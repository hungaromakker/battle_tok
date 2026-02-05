//! Battle Arena - Combat Prototype
//!
//! Run with: `cargo run --bin battle_arena`
//!
//! Phase 1C implementation: Projectile spawning and rendering demo.
//! Press SPACE to spawn projectiles that follow ballistic arcs.
//!
//! Controls:
//! - WASD: Move (first-person or camera)
//! - Mouse right-drag: Look around (FPS style)
//! - Space: Jump (first-person mode) / Fire (free camera mode)
//! - Shift: Sprint when moving
//! - V: Toggle first-person / free camera mode
//! - Arrow Up/Down: Adjust barrel elevation
//! - Arrow Left/Right: Adjust barrel azimuth
//! - R: Reset camera
//! - C: Clear all projectiles
//! - B: Toggle builder mode
//! - T: Terrain editor UI
//! - ESC: Exit
//!
//! ## Performance Optimizations (US-019)
//!
//! ### Identified Bottlenecks (Profile Analysis):
//! 1. **VSync capping FPS** - AutoVsync limited FPS to monitor refresh rate (60Hz)
//! 2. **Per-frame mesh regeneration** - Cannon + projectile spheres rebuilt every frame
//! 3. **High-poly sphere generation** - 8 segments = 144 triangles per projectile sphere
//!
//! ### Implemented Optimizations:
//! 1. **Disable VSync** - Changed `PresentMode::AutoVsync` to `PresentMode::Immediate`
//!    - Before: Capped at ~60 FPS (monitor refresh rate)
//!    - After: Uncapped, limited only by GPU/CPU performance
//! 2. **Reduce sphere segments** - 4 segments (32 triangles) provides same visual at distance
//!    - Before: 8 segments = 144 triangles per sphere
//!    - After: 4 segments = 32 triangles per sphere (4.5x triangle reduction)
//! 3. **Cache cannon mesh** - Only regenerate when barrel direction changes
//!    - Before: Regenerate ~200 vertices per frame unconditionally
//!    - After: Cache last direction, skip if unchanged (saves trig calculations)
//! 4. **Pre-allocated buffers** - Dynamic buffers sized for max expected content
//!    - Avoids reallocation during gameplay
//!
//! ### Performance Results:
//! - Baseline (VSync on): ~60 FPS (capped by monitor)
//! - After VSync removal: 1500-3000+ FPS (GPU limited)
//! - With 32 projectiles: 1000+ FPS maintained

use std::sync::Arc;
use std::time::Instant;

// bytemuck used by wgpu
use glam::{Mat4, Vec3};
use battle_tok_engine::render::HexPrismGrid;
use battle_tok_engine::render::hex_prism::{DEFAULT_HEX_HEIGHT, DEFAULT_HEX_RADIUS};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowAttributes, WindowId};

// Import physics from engine
use battle_tok_engine::physics::ballistics::{BallisticsConfig, Projectile, ProjectileState};

// Import stormy skybox
use battle_tok_engine::render::{StormySky, StormySkyConfig};

// Import Phase 2 visual upgrade systems
use battle_tok_engine::render::{
    PointLightManager,
    ParticleSystem,
    MaterialSystem, SceneConfig,
    FogPostPass, FogPostConfig,
};

// Import building block system (Phase 2-4)
use battle_tok_engine::render::{
    BuildingBlockManager, BuildingBlockShape, BuildingBlock,
    MergeWorkflowManager, MergedMesh,
    SculptingManager,
    // Building physics (Phase 5)
    BuildingPhysics,
};

// Import fullscreen for F11 toggle
use winit::window::Fullscreen;

// Import game module types (modularized code)
use battle_tok_engine::game::{
    // Core types
    Vertex, Mesh, Camera,
    // Player types
    Player, MovementKeys, AimingKeys, PLAYER_EYE_HEIGHT,
    // Uniforms
    Uniforms, SdfCannonUniforms, SdfCannonData,
    // Visual upgrade uniforms (Batch 1)
    TerrainShaderParams, LavaParams, SkyStormParams, FogPostParams, TonemapParams,
    // Builder types
    BuilderMode, BuildToolbar, SelectedFace,
    SHAPE_NAMES, BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, PHYSICS_CHECK_INTERVAL,
    // Builder raycast & placement
    screen_to_ray, determine_hit_face, calculate_adjacent_block_position,
    snap_to_grid, find_snap_position, ray_terrain_intersection,
    check_block_support, calculate_bridge_segments, PlacementResult,
    // Terrain
    TerrainParams, get_terrain_params, set_terrain_params,
    terrain_height_at, generate_elevated_hex_terrain, generate_lava_plane,
    generate_bridge, BridgeConfig,
    // Trees
    PlacedTree, generate_trees_on_terrain, generate_all_trees_mesh,
    // Destruction
    FallingPrism, DebrisParticle, spawn_debris, get_material_color,
    // Meteors
    Meteor, MeteorSpawner, spawn_meteor_impact,
    // Mesh generators
    generate_rotated_box, generate_sphere,
    // UI
    StartOverlay, TerrainEditorUI, TopBar,
    // Game state
    GameState,
    // Render
    SHADER_SOURCE, create_test_walls,
    generate_hex_grid_overlay, calculate_ghost_color, GHOST_PREVIEW_COLOR, generate_block_preview_mesh,
    // Physics
    CollisionResult, AABB, check_capsule_aabb_collision, check_capsule_hex_collision,
    hex_to_world_position, world_to_hex_coords,
    HEX_NEIGHBORS, has_support, find_unsupported_cascade, check_falling_prism_collision,
    // Input
    InputAction, InputContext, MovementState, AimingState, MovementKey, AimingKey, map_key_to_action,
    // Cannon
    ArenaCannon as Cannon, CANNON_ROTATION_SPEED,
};

// Player constants not in module
const COYOTE_TIME: f32 = 0.1;

// GPU-specific types (need wgpu::Buffer, can't be in module)
/// GPU buffers for a merged mesh (baked from SDF)
struct MergedMeshBuffers {
    /// Unique ID matching the MergedMesh
    _id: u32,
    /// Vertex buffer
    vertex_buffer: wgpu::Buffer,
    /// Index buffer
    index_buffer: wgpu::Buffer,
    /// Number of indices
    index_count: u32,
}
// ============================================================================
// APPLICATION
// ============================================================================

struct BattleArenaApp {
    window: Option<Arc<Window>>,

    // GPU resources
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_buffer: Option<wgpu::Buffer>,
    uniform_bind_group: Option<wgpu::BindGroup>,

    // Static mesh buffers (terrain, cannon base)
    static_vertex_buffer: Option<wgpu::Buffer>,
    static_index_buffer: Option<wgpu::Buffer>,
    static_index_count: u32,

    // Dynamic mesh buffers (cannon barrel, projectiles)
    dynamic_vertex_buffer: Option<wgpu::Buffer>,
    dynamic_index_buffer: Option<wgpu::Buffer>,
    dynamic_index_count: u32,

    // Hex-prism walls for collision detection (US-015) and destruction (US-016)
    hex_wall_grid: HexPrismGrid,
    hex_wall_vertex_buffer: Option<wgpu::Buffer>,
    hex_wall_index_buffer: Option<wgpu::Buffer>,
    hex_wall_index_count: u32,
    prisms_destroyed: u32, // Total prisms destroyed (US-016)

    // SDF cannon rendering (US-013)
    sdf_cannon_pipeline: Option<wgpu::RenderPipeline>,
    sdf_cannon_uniform_buffer: Option<wgpu::Buffer>,
    sdf_cannon_data_buffer: Option<wgpu::Buffer>,
    sdf_cannon_bind_group: Option<wgpu::BindGroup>,
    depth_texture: Option<wgpu::TextureView>,
    depth_texture_raw: Option<wgpu::Texture>,
    
    // UI pipeline (no depth testing, for 2D overlay)
    ui_pipeline: Option<wgpu::RenderPipeline>,
    ui_uniform_buffer: Option<wgpu::Buffer>,
    ui_bind_group: Option<wgpu::BindGroup>,

    // Dark and stormy skybox
    stormy_sky: Option<StormySky>,
    lightning_timer: f32, // Timer for periodic lightning (3-8 sec intervals)

    // Phase 2 Visual Systems
    point_lights: Option<PointLightManager>,  // Torch lighting with flicker
    particle_system: Option<ParticleSystem>,  // Ember/ash particles
    material_system: Option<MaterialSystem>,  // Material coordination
    fog_post: Option<FogPostPass>,            // Depth-based fog post-process

    // First-person player controller (Phase 1)
    player: Player,
    first_person_mode: bool, // Toggle between FPS mode and free camera
    
    // Camera and input
    camera: Camera,
    movement: MovementKeys,
    aiming: AimingKeys, // Cannon aiming keys (US-017)
    mouse_pressed: bool,
    left_mouse_pressed: bool, // For builder mode placement
    _last_mouse_pos: Option<(f32, f32)>,
    current_mouse_pos: Option<(f32, f32)>, // For raycast

    // Builder mode (Fallout 4-style building)
    builder_mode: BuilderMode,
    // Ghost and grid rendering uses dynamic_mesh buffer for simplicity
    
    // New building block system (Phase 2-5)
    build_toolbar: BuildToolbar,
    block_manager: BuildingBlockManager,
    block_physics: BuildingPhysics,
    merge_workflow: MergeWorkflowManager,
    _sculpting: SculptingManager,
    merged_mesh_buffers: Vec<MergedMeshBuffers>,
    block_vertex_buffer: Option<wgpu::Buffer>,
    block_index_buffer: Option<wgpu::Buffer>,
    block_index_count: u32,
    
    // Windows focus overlay
    start_overlay: StartOverlay,

    // Procedural trees (harvestable for building materials)
    trees_attacker: Vec<PlacedTree>,
    trees_defender: Vec<PlacedTree>,
    tree_vertex_buffer: Option<wgpu::Buffer>,
    tree_index_buffer: Option<wgpu::Buffer>,
    tree_index_count: u32,
    _wood_harvested: u32, // Total wood collected

    // Cannon and projectiles
    cannon: Cannon,
    projectiles: Vec<Projectile>,
    ballistics_config: BallisticsConfig,

    // Physics-based destruction system
    falling_prisms: Vec<FallingPrism>,
    debris_particles: Vec<DebrisParticle>,

    // Meteor system (atmospheric fireballs)
    meteors: Vec<Meteor>,
    meteor_spawner: MeteorSpawner,

    // Terrain editor UI (on-screen sliders)
    terrain_ui: TerrainEditorUI,
    terrain_needs_rebuild: bool,

    // Game state (economy, population, day cycle)
    game_state: GameState,

    // Timing
    start_time: Instant,
    last_frame: Instant,
    frame_count: u64,
    fps: f32,
    last_fps_update: Instant,
}

impl BattleArenaApp {
    fn new() -> Self {
        Self {
            window: None,
            device: None,
            queue: None,
            surface: None,
            surface_config: None,
            pipeline: None,
            uniform_buffer: None,
            uniform_bind_group: None,
            static_vertex_buffer: None,
            static_index_buffer: None,
            static_index_count: 0,
            dynamic_vertex_buffer: None,
            dynamic_index_buffer: None,
            dynamic_index_count: 0,
            hex_wall_grid: create_test_walls(),
            hex_wall_vertex_buffer: None,
            hex_wall_index_buffer: None,
            hex_wall_index_count: 0,
            prisms_destroyed: 0,
            sdf_cannon_pipeline: None,
            sdf_cannon_uniform_buffer: None,
            sdf_cannon_data_buffer: None,
            sdf_cannon_bind_group: None,
            depth_texture: None,
            depth_texture_raw: None,
            ui_pipeline: None,
            ui_uniform_buffer: None,
            ui_bind_group: None,
            stormy_sky: None,
            lightning_timer: 3.0, // First lightning in 3 seconds
            // Phase 2 Visual Systems
            point_lights: None,
            particle_system: None,
            material_system: None,
            fog_post: None,
            player: Player::default(),
            first_person_mode: true, // Start in first-person mode
            camera: Camera::default(),
            movement: MovementKeys::default(),
            aiming: AimingKeys::default(),
            mouse_pressed: false,
            left_mouse_pressed: false,
            _last_mouse_pos: None,
            current_mouse_pos: None,
            builder_mode: BuilderMode::default(),
            build_toolbar: BuildToolbar::default(),
            block_manager: BuildingBlockManager::new(),
            block_physics: BuildingPhysics::new(),
            merge_workflow: MergeWorkflowManager::new(),
            _sculpting: SculptingManager::new(),
            merged_mesh_buffers: Vec::new(),
            block_vertex_buffer: None,
            block_index_buffer: None,
            block_index_count: 0,
            start_overlay: StartOverlay::default(),
            trees_attacker: Vec::new(),
            trees_defender: Vec::new(),
            tree_vertex_buffer: None,
            tree_index_buffer: None,
            tree_index_count: 0,
            _wood_harvested: 0,
            cannon: Cannon::default(),
            projectiles: Vec::new(),
            ballistics_config: BallisticsConfig::default(),
            falling_prisms: Vec::new(),
            debris_particles: Vec::new(),
            // Meteor spawner: centered between the two islands, covers full arena
            meteors: Vec::new(),
            meteor_spawner: MeteorSpawner::new(Vec3::new(0.0, 0.0, 0.0), 60.0),
            terrain_ui: TerrainEditorUI::default(),
            terrain_needs_rebuild: false,
            game_state: GameState::new(),
            start_time: Instant::now(),
            last_frame: Instant::now(),
            frame_count: 0,
            fps: 0.0,
            last_fps_update: Instant::now(),
        }
    }

    fn initialize(&mut self, window: Arc<Window>) {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        // Request adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find adapter");

        // Create device and queue
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Battle Arena Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                ..Default::default()
            },
        ))
        .expect("Failed to create device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Prefer Immediate (uncapped FPS) for performance testing, fallback to Mailbox
        let present_mode = if surface_caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::AutoVsync
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Battle Arena Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create bind group
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        
        // ============================================
        // UI PIPELINE (no depth testing, for 2D overlay)
        // ============================================
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for UI
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None, // NO depth testing for UI overlay
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        
        // Create UI uniform buffer with identity matrix (pre-initialized, never changes)
        let ui_uniforms = Uniforms {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            time: 0.0,
            sun_dir: [0.0, 1.0, 0.0],
            fog_density: 0.0, // No fog for UI
            fog_color: [0.0, 0.0, 0.0],
            ambient: 1.0, // Full brightness for UI
            projectile_count: 0,
            _pad_before_padding1: [0.0; 3],
            _padding1: [0.0; 3],
            _pad_before_array: 0.0,
            projectile_positions: [[0.0; 4]; 32],
        };
        let ui_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Uniform Buffer"),
            contents: bytemuck::bytes_of(&ui_uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        
        // Create UI bind group (uses same layout as main pipeline)
        let ui_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ui_uniform_buffer.as_entire_binding(),
            }],
        });

        // ============================================
        // SDF CANNON RENDERING SETUP (US-013)
        // ============================================
        // Load SDF cannon shader from file
        let sdf_cannon_shader_source = std::fs::read_to_string("shaders/sdf_cannon.wgsl")
            .expect("Failed to load sdf_cannon.wgsl shader");
        let sdf_cannon_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SDF Cannon Shader"),
            source: wgpu::ShaderSource::Wgsl(sdf_cannon_shader_source.into()),
        });

        // Create SDF cannon uniform buffer (camera, time, lighting)
        let sdf_cannon_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SDF Cannon Uniform Buffer"),
            size: std::mem::size_of::<SdfCannonUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create SDF cannon data buffer (position, rotation, color)
        let sdf_cannon_data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SDF Cannon Data Buffer"),
            size: std::mem::size_of::<SdfCannonData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create SDF cannon bind group layout
        let sdf_cannon_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SDF Cannon Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create SDF cannon bind group
        let sdf_cannon_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SDF Cannon Bind Group"),
            layout: &sdf_cannon_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sdf_cannon_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: sdf_cannon_data_buffer.as_entire_binding(),
                },
            ],
        });

        // Create SDF cannon pipeline layout
        let sdf_cannon_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SDF Cannon Pipeline Layout"),
            bind_group_layouts: &[&sdf_cannon_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create depth texture for proper depth testing
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create SDF cannon render pipeline (fullscreen triangle for ray marching)
        let sdf_cannon_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SDF Cannon Pipeline"),
            layout: Some(&sdf_cannon_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sdf_cannon_shader,
                entry_point: Some("vs_main"),
                buffers: &[], // Fullscreen triangle uses vertex_index
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sdf_cannon_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for fullscreen triangle
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        println!("[US-013] SDF cannon pipeline initialized");

        // Create static mesh (terrain platforms with procedural elevation)
        let mut static_mesh = Mesh::new();

        // Island positions - separated to create a gap for the bridge
        // With radius 30 and centers 90 units apart, the gap is ~30 meters
        const ATTACKER_CENTER: Vec3 = Vec3::new(0.0, 0.0, 45.0);
        const DEFENDER_CENTER: Vec3 = Vec3::new(0.0, 0.0, -45.0);
        const ISLAND_RADIUS: f32 = 30.0;
        
        // Attacker hex platform (where cannon is) - detailed mountainous terrain
        let attacker_platform = generate_elevated_hex_terrain(
            ATTACKER_CENTER,
            ISLAND_RADIUS,
            [0.4, 0.5, 0.3, 1.0], // Color determined dynamically by height
            64, // High subdivision for detailed mountains
        );
        static_mesh.merge(&attacker_platform);
        
        // Water plane for attacker platform
        let attacker_water = generate_lava_plane(
            ATTACKER_CENTER,
            ISLAND_RADIUS,
        );
        static_mesh.merge(&attacker_water);

        // Defender hex platform (target) - detailed mountainous terrain
        let defender_platform = generate_elevated_hex_terrain(
            DEFENDER_CENTER,
            ISLAND_RADIUS,
            [0.5, 0.4, 0.35, 1.0], // Color determined dynamically by height
            64, // High subdivision for detailed mountains
        );
        static_mesh.merge(&defender_platform);
        
        // Water plane for defender platform
        let defender_water = generate_lava_plane(
            DEFENDER_CENTER,
            ISLAND_RADIUS,
        );
        static_mesh.merge(&defender_water);
        
        // Generate bridge connecting the two islands
        // Find actual terrain edge vertices by scanning the generated meshes
        
        // Find the attacker terrain vertex closest to the defender (smallest Z, near X=0)
        let mut bridge_start = Vec3::new(0.0, 0.0, ATTACKER_CENTER.z);
        let mut best_dist_start = f32::MAX;
        for v in &attacker_platform.vertices {
            let vx = v.position[0];
            let vz = v.position[2];
            // Looking for vertex near X=0 with smallest Z (toward defender)
            let dist = vx.abs() + (vz - (ATTACKER_CENTER.z - ISLAND_RADIUS)).abs() * 0.5;
            if dist < best_dist_start && vz < ATTACKER_CENTER.z {
                best_dist_start = dist;
                bridge_start = Vec3::new(v.position[0], v.position[1], v.position[2]);
            }
        }
        
        // Find the defender terrain vertex closest to the attacker (largest Z, near X=0)
        let mut bridge_end = Vec3::new(0.0, 0.0, DEFENDER_CENTER.z);
        let mut best_dist_end = f32::MAX;
        for v in &defender_platform.vertices {
            let vx = v.position[0];
            let vz = v.position[2];
            // Looking for vertex near X=0 with largest Z (toward attacker)
            let dist = vx.abs() + ((DEFENDER_CENTER.z + ISLAND_RADIUS) - vz).abs() * 0.5;
            if dist < best_dist_end && vz > DEFENDER_CENTER.z {
                best_dist_end = dist;
                bridge_end = Vec3::new(v.position[0], v.position[1], v.position[2]);
            }
        }
        
        println!("[Bridge] Connecting from {:?} to {:?}", bridge_start, bridge_end);
        
        let bridge_config = BridgeConfig::default();
        let bridge_mesh = generate_bridge(bridge_start, bridge_end, &bridge_config);
        static_mesh.merge(&bridge_mesh);
        
        println!("[Builder Mode] Generated detailed terrain with mountains, rocks, and water");
        
        // ============================================
        // PROCEDURAL TREES (harvestable)
        // ============================================
        // Generate trees on both platforms using noise-based distribution
        self.trees_attacker = generate_trees_on_terrain(
            ATTACKER_CENTER,
            28.0, // Slightly smaller than terrain
            0.3,  // Density threshold (higher = more trees)
            0.0,  // Seed offset
        );
        self.trees_defender = generate_trees_on_terrain(
            DEFENDER_CENTER,
            28.0,
            0.35, // Slightly more trees on defender side
            100.0, // Different seed for variety
        );
        
        // Generate combined tree mesh
        let mut all_trees = self.trees_attacker.clone();
        all_trees.extend(self.trees_defender.clone());
        let tree_mesh = generate_all_trees_mesh(&all_trees);
        
        // Create tree buffers
        let max_tree_vertices = 2000 * 50; // Max ~2000 trees, ~50 vertices each
        let max_tree_indices = 2000 * 80;
        
        let tree_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tree Vertex Buffer"),
            size: (max_tree_vertices * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let tree_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tree Index Buffer"),
            size: (max_tree_indices * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        if !tree_mesh.vertices.is_empty() {
            queue.write_buffer(&tree_vertex_buffer, 0, bytemuck::cast_slice(&tree_mesh.vertices));
            queue.write_buffer(&tree_index_buffer, 0, bytemuck::cast_slice(&tree_mesh.indices));
        }
        
        let tree_index_count = tree_mesh.indices.len() as u32;
        println!("[Trees] Generated {} trees ({} attacker, {} defender)", 
            self.trees_attacker.len() + self.trees_defender.len(),
            self.trees_attacker.len(),
            self.trees_defender.len()
        );

        // Create static buffers
        let static_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Static Vertex Buffer"),
            contents: bytemuck::cast_slice(&static_mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let static_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Static Index Buffer"),
            contents: bytemuck::cast_slice(&static_mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Create dynamic buffers (will be updated each frame)
        let dynamic_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Vertex Buffer"),
            size: 1024 * 1024, // 1MB should be plenty
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dynamic_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Index Buffer"),
            size: 256 * 1024, // 256KB
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ============================================
        // HEX-PRISM WALLS (US-012, US-016)
        // ============================================
        // Generate mesh from hex-prism wall grid
        let (hex_vertices, hex_indices) = self.hex_wall_grid.generate_combined_mesh();

        // Convert HexPrismVertex to Vertex (same layout: position, normal, color)
        let hex_wall_vertices: Vec<Vertex> = hex_vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            })
            .collect();

        let hex_wall_index_count = hex_indices.len() as u32;

        // US-016: Create buffers with COPY_DST to allow updates when prisms are destroyed
        // Pre-allocate space for max 500 prisms (each = 38 vertices, 72 indices)
        let max_hex_vertex_bytes = 500 * 38 * std::mem::size_of::<Vertex>();
        let max_hex_index_bytes = 500 * 72 * std::mem::size_of::<u32>();

        let hex_wall_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Hex Wall Vertex Buffer"),
            size: max_hex_vertex_bytes as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let hex_wall_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Hex Wall Index Buffer"),
            size: max_hex_index_bytes as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Write initial mesh data to buffers
        if !hex_wall_vertices.is_empty() {
            queue.write_buffer(&hex_wall_vertex_buffer, 0, bytemuck::cast_slice(&hex_wall_vertices));
            queue.write_buffer(&hex_wall_index_buffer, 0, bytemuck::cast_slice(&hex_indices));
        }

        // Initialize apocalyptic battle arena skybox (before device is moved)
        // Using battle_arena preset: purple sky, orange horizon, lightning, dramatic atmosphere
        let stormy_sky = StormySky::with_config(&device, surface_format, StormySkyConfig::battle_arena());

        // ============================================
        // Phase 2 Visual Systems Initialization
        // ============================================

        // Point light manager for torch lighting with flickering
        let mut point_lights = PointLightManager::new(&device);
        // Add torches on both castle platforms (8 torches each, 16 total)
        // Attacker castle torches (at Z=45, in a rough rectangle around the castle)
        let torch_color = [1.0, 0.6, 0.2]; // Warm orange
        let torch_radius = 10.0;
        point_lights.add_torch([10.0, 6.0, 55.0], torch_color, torch_radius);
        point_lights.add_torch([-10.0, 6.0, 55.0], torch_color, torch_radius);
        point_lights.add_torch([10.0, 6.0, 35.0], torch_color, torch_radius);
        point_lights.add_torch([-10.0, 6.0, 35.0], torch_color, torch_radius);
        // Defender castle torches (at Z=-45)
        point_lights.add_torch([10.0, 6.0, -55.0], torch_color, torch_radius);
        point_lights.add_torch([-10.0, 6.0, -55.0], torch_color, torch_radius);
        point_lights.add_torch([10.0, 6.0, -35.0], torch_color, torch_radius);
        point_lights.add_torch([-10.0, 6.0, -35.0], torch_color, torch_radius);
        // Bridge torches (at the ends)
        point_lights.add_torch([3.0, 4.0, 15.0], torch_color, 8.0);
        point_lights.add_torch([-3.0, 4.0, 15.0], torch_color, 8.0);
        point_lights.add_torch([3.0, 4.0, -15.0], torch_color, 8.0);
        point_lights.add_torch([-3.0, 4.0, -15.0], torch_color, 8.0);
        println!("[US-P2-014] Point lights initialized: {} torches", point_lights.light_count());

        // Particle system for ember/ash effects
        let mut particle_system = ParticleSystem::new(&device, surface_format);
        // Add spawn positions above lava areas (between the islands)
        // Lava is in the gap around Z=0 (between Z=-15 and Z=15)
        particle_system.add_spawn_position([0.0, 0.5, 0.0]);
        particle_system.add_spawn_position([5.0, 0.5, 5.0]);
        particle_system.add_spawn_position([-5.0, 0.5, 5.0]);
        particle_system.add_spawn_position([5.0, 0.5, -5.0]);
        particle_system.add_spawn_position([-5.0, 0.5, -5.0]);
        particle_system.add_spawn_position([10.0, 0.5, 0.0]);
        particle_system.add_spawn_position([-10.0, 0.5, 0.0]);
        // Also spawn near the island edges where lava pools are
        particle_system.add_spawn_position([0.0, 0.5, 20.0]);
        particle_system.add_spawn_position([0.0, 0.5, -20.0]);
        particle_system.set_spawn_rate(80.0); // 80 embers per second for dramatic effect
        println!("[US-P2-014] Particle system initialized with {} spawn positions", 9);

        // Material system for unified material management
        let mut material_system = MaterialSystem::new(&device);
        material_system.set_scene_config(SceneConfig::battle_arena());
        println!("[US-P2-014] Material system initialized with battle_arena scene config");

        // Fog post-processing pass
        let fog_post = FogPostPass::with_config(&device, surface_format, FogPostConfig::battle_arena());
        println!("[US-P2-014] Fog post-pass initialized with battle_arena preset");

        self.window = Some(window);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface = Some(surface);
        self.surface_config = Some(surface_config);
        self.pipeline = Some(pipeline);
        self.uniform_buffer = Some(uniform_buffer);
        self.uniform_bind_group = Some(uniform_bind_group);
        self.static_vertex_buffer = Some(static_vertex_buffer);
        self.static_index_buffer = Some(static_index_buffer);
        self.static_index_count = static_mesh.indices.len() as u32;
        self.dynamic_vertex_buffer = Some(dynamic_vertex_buffer);
        self.dynamic_index_buffer = Some(dynamic_index_buffer);
        self.hex_wall_vertex_buffer = Some(hex_wall_vertex_buffer);
        self.hex_wall_index_buffer = Some(hex_wall_index_buffer);
        self.hex_wall_index_count = hex_wall_index_count;

        // Store SDF cannon resources (US-013)
        self.sdf_cannon_pipeline = Some(sdf_cannon_pipeline);
        self.sdf_cannon_uniform_buffer = Some(sdf_cannon_uniform_buffer);
        self.sdf_cannon_data_buffer = Some(sdf_cannon_data_buffer);
        self.sdf_cannon_bind_group = Some(sdf_cannon_bind_group);
        self.depth_texture = Some(depth_texture_view);
        self.depth_texture_raw = Some(depth_texture);
        self.ui_pipeline = Some(ui_pipeline);
        self.ui_uniform_buffer = Some(ui_uniform_buffer);
        self.ui_bind_group = Some(ui_bind_group);
        self.stormy_sky = Some(stormy_sky);

        // Store Phase 2 visual systems
        self.point_lights = Some(point_lights);
        self.particle_system = Some(particle_system);
        self.material_system = Some(material_system);
        self.fog_post = Some(fog_post);

        // Store tree buffers
        self.tree_vertex_buffer = Some(tree_vertex_buffer);
        self.tree_index_buffer = Some(tree_index_buffer);
        self.tree_index_count = tree_index_count;

        println!(
            "[Battle Arena] Hex-prism walls: {} vertices, {} indices",
            hex_wall_vertices.len(),
            hex_indices.len()
        );
    }

    fn update(&mut self, delta_time: f32) {
        // Update game state (day cycle, economy, population)
        let new_day = self.game_state.update(delta_time);
        if new_day {
            // Day changed - could trigger notifications here
            println!("Day {} has begun!", self.game_state.day_cycle.day());
        }

        // Update lightning timer (triggers every 3-8 seconds randomly)
        self.lightning_timer -= delta_time;
        if self.lightning_timer <= 0.0 {
            // Trigger lightning flash
            if let Some(ref mut stormy_sky) = self.stormy_sky {
                stormy_sky.trigger_lightning();
            }
            // Reset timer to random value between 3.0 and 8.0 seconds
            // Using pseudo-random based on current time for variety
            let time = self.start_time.elapsed().as_secs_f32();
            let rand_val = ((time * 12.9898).sin() * 43758.5453).fract(); // 0.0 to 1.0
            self.lightning_timer = 3.0 + rand_val * 5.0; // 3.0 to 8.0 seconds
        }

        // Update Phase 2 visual systems
        let time = self.start_time.elapsed().as_secs_f32();

        // Update point lights (flickering effect)
        if let (Some(point_lights), Some(queue)) = (&mut self.point_lights, &self.queue) {
            point_lights.update(queue, time);
        }

        // Update particle system (embers rising from lava)
        if let Some(ref mut particle_system) = self.particle_system {
            particle_system.update(delta_time);
        }

        // Check if terrain needs rebuilding
        if self.terrain_needs_rebuild {
            self.rebuild_terrain();
            self.terrain_needs_rebuild = false;
        }
        
        // Update block placement preview
        self.update_block_preview();
        
        // Building block physics: runs every frame for smooth falling
        // Support graph check runs every N seconds, but physics simulation runs continuously
        self.build_toolbar.physics_check_timer += delta_time;
        let do_support_check = self.build_toolbar.physics_check_timer >= PHYSICS_CHECK_INTERVAL;
        if do_support_check {
            self.build_toolbar.physics_check_timer = 0.0;
        }
        self.update_building_physics(delta_time, do_support_check);
        
        // === BLOCK PICKUP SYSTEM ===
        // Hold left-click on a loose block to pick it up
        const PICKUP_HOLD_TIME: f32 = 0.5; // Seconds to hold before pickup
        
        if self.left_mouse_pressed && self.build_toolbar.visible {
            self.build_toolbar.mouse_hold_time += delta_time;
            
            // Check if we should pick up a loose block
            if self.build_toolbar.mouse_hold_time >= PICKUP_HOLD_TIME && !self.build_toolbar.pickup_in_progress {
                self.build_toolbar.pickup_in_progress = true;
                
                // Try to pick up a loose block
                if let Some((block_id, shape, material)) = self.raycast_to_loose_block() {
                    // Stash the block in inventory
                    if self.build_toolbar.inventory.stash(shape, material) {
                        // Remove from world
                        self.block_physics.unregister_block(block_id);
                        self.block_manager.remove_block(block_id);
                        self.regenerate_block_mesh();
                        
                        println!("[Pickup] Stashed block (inventory: {}/{})", 
                            self.build_toolbar.inventory.count(),
                            self.build_toolbar.inventory.max_capacity);
                    } else {
                        println!("[Pickup] Inventory full!");
                    }
                }
            }
        } else {
            // Reset hold timer when mouse is released
            self.build_toolbar.mouse_hold_time = 0.0;
            self.build_toolbar.pickup_in_progress = false;
        }
        
        // Update movement based on mode
        if self.first_person_mode {
            // First-person mode: Player physics with terrain collision
            self.player.update(&self.movement, self.camera.yaw, delta_time);
            
            // Check collision with building blocks
            self.player_block_collision();
            
            // Check collision with hex prism walls
            self.player_hex_collision();
            
            // Camera follows player's eye position
            self.camera.position = self.player.get_eye_position();
        } else {
            // Free camera mode: Direct camera movement (original behavior)
            let forward = if self.movement.forward { 1.0 } else { 0.0 }
                - if self.movement.backward { 1.0 } else { 0.0 };
            let right = if self.movement.right { 1.0 } else { 0.0 }
                - if self.movement.left { 1.0 } else { 0.0 };
            let up = if self.movement.up { 1.0 } else { 0.0 }
                - if self.movement.down { 1.0 } else { 0.0 };

            self.camera
                .update_movement(forward, right, up, delta_time, self.movement.sprint);
        }

        // Update cannon aiming based on held arrow keys (US-017: smooth movement)
        let aim_delta = CANNON_ROTATION_SPEED * delta_time;
        if self.aiming.aim_up {
            self.cannon.adjust_elevation(aim_delta);
        }
        if self.aiming.aim_down {
            self.cannon.adjust_elevation(-aim_delta);
        }
        if self.aiming.aim_left {
            self.cannon.adjust_azimuth(-aim_delta);
        }
        if self.aiming.aim_right {
            self.cannon.adjust_azimuth(aim_delta);
        }

        // Smooth interpolation for cannon barrel movement
        self.cannon.update(delta_time);

        // Update projectiles with ballistics and collision detection (US-015)
        // Collect hit information first, then apply destruction (US-016)
        let mut prisms_to_destroy: Vec<(i32, i32, i32)> = Vec::new();

        self.projectiles.retain_mut(|projectile| {
            // Store position before integration for collision ray
            let prev_position = projectile.position;

            // Integrate physics
            let state = projectile.integrate(&self.ballistics_config, delta_time);

            // If still flying, check for collision with hex-prism walls
            if matches!(state, ProjectileState::Flying) {
                // Cast ray from previous position to current position
                let ray_dir = projectile.position - prev_position;
                let ray_length = ray_dir.length();

                if ray_length > 0.001 {
                    let ray_dir_normalized = ray_dir / ray_length;

                    // Check collision against hex-prism grid
                    if let Some(hit) = self.hex_wall_grid.ray_cast(prev_position, ray_dir_normalized, ray_length) {
                        // Projectile hit a wall!
                        projectile.position = hit.position;
                        projectile.active = false;

                        // US-016: Queue prism for destruction
                        prisms_to_destroy.push(hit.prism_coord);

                        println!(
                            "[US-016] Projectile HIT wall at ({:.2}, {:.2}, {:.2}), destroying prism {:?}",
                            hit.position.x, hit.position.y, hit.position.z, hit.prism_coord
                        );
                        return false; // Remove projectile (state = Hit)
                    }
                }

                true // Keep flying
            } else {
                // Ground hit or expired
                false
            }
        });

        // US-016: Apply hex-prism destruction with physics cascade for all hits this frame
        for coord in prisms_to_destroy {
            self.destroy_prism_with_physics(coord);
        }

        // Physics simulation for falling prisms
        self.update_falling_prisms(delta_time);

        // Physics simulation for debris particles
        self.update_debris_particles(delta_time);

        // Update meteor system (apocalyptic fireballs)
        self.update_meteors(delta_time);

        // US-016: Regenerate hex-prism mesh if any prisms were destroyed or modified
        if self.hex_wall_grid.needs_mesh_update() {
            self.regenerate_hex_wall_mesh();
        }
        
        // Builder mode: Update cursor position from mouse
        self.update_builder_cursor();
    }
    
    /// Destroy a prism and check for unsupported prisms above/behind it
    fn destroy_prism_with_physics(&mut self, coord: (i32, i32, i32)) {
        // Remove the hit prism and spawn debris
        if let Some(prism) = self.hex_wall_grid.remove_by_coord(coord) {
            self.prisms_destroyed += 1;
            
            // Spawn debris particles at the destroyed prism's location
            let debris = spawn_debris(prism.center, prism.material, 8);
            self.debris_particles.extend(debris);
            
            println!(
                "[Physics] Prism DESTROYED at {:?}, spawned {} debris (total destroyed: {})",
                coord, 8, self.prisms_destroyed
            );
            
            // Check for cascade - prisms that lost support
            self.check_support_cascade(coord);
        }
    }
    
    /// Check if prisms above or nearby have lost support and should fall
    fn check_support_cascade(&mut self, destroyed_coord: (i32, i32, i32)) {
        let (q, r, level) = destroyed_coord;
        
        // Collect all prisms that need to fall (can't modify grid while iterating)
        let mut prisms_to_fall: Vec<(i32, i32, i32)> = Vec::new();
        
        // Check prism directly above
        if self.hex_wall_grid.contains(q, r, level + 1) {
            if !self.has_support(q, r, level + 1) {
                prisms_to_fall.push((q, r, level + 1));
            }
        }
        
        // Check prisms in adjacent columns that might have lost support
        // Hex neighbors (axial coordinates)
        let neighbors = [
            (q + 1, r),     // E
            (q - 1, r),     // W
            (q, r + 1),     // SE
            (q, r - 1),     // NW
            (q + 1, r - 1), // NE
            (q - 1, r + 1), // SW
        ];
        
        for (nq, nr) in neighbors {
            // Check if neighbor at same level or above has lost support
            for check_level in level..=level + 3 {
                if self.hex_wall_grid.contains(nq, nr, check_level) {
                    if !self.has_support(nq, nr, check_level) {
                        prisms_to_fall.push((nq, nr, check_level));
                    }
                }
            }
        }
        
        // Convert unsupported prisms to falling prisms
        for coord in prisms_to_fall {
            if let Some(prism) = self.hex_wall_grid.remove_by_coord(coord) {
                println!("[Physics] Prism at {:?} lost support - FALLING!", coord);
                self.falling_prisms.push(FallingPrism::new(coord, prism.center, prism.material));
                
                // Recursively check for more cascade
                self.check_support_cascade(coord);
            }
        }
    }
    
    /// Check if a prism has support (something below it or on ground level)
    fn has_support(&self, q: i32, r: i32, level: i32) -> bool {
        // Ground level always has support
        if level <= 0 {
            return true;
        }
        
        // Check if there's a prism directly below
        if self.hex_wall_grid.contains(q, r, level - 1) {
            return true;
        }
        
        // Check for diagonal support from neighbors (structural support)
        // A prism can be supported if at least 2 adjacent neighbors at same level exist
        let neighbors = [
            (q + 1, r),
            (q - 1, r),
            (q, r + 1),
            (q, r - 1),
            (q + 1, r - 1),
            (q - 1, r + 1),
        ];
        
        let mut neighbor_support = 0;
        for (nq, nr) in neighbors {
            // Check neighbor at same level (horizontal structural support)
            if self.hex_wall_grid.contains(nq, nr, level) {
                neighbor_support += 1;
            }
            // Check neighbor below (diagonal support)
            if self.hex_wall_grid.contains(nq, nr, level - 1) {
                neighbor_support += 1;
            }
        }
        
        // Need at least 2 supports to stay standing (structural integrity)
        neighbor_support >= 2
    }
    
    /// Update falling prisms physics
    fn update_falling_prisms(&mut self, delta_time: f32) {
        // Update physics for each falling prism
        for prism in &mut self.falling_prisms {
            prism.update(delta_time);
        }
        
        // Check for collisions with remaining wall prisms
        let mut new_debris: Vec<DebrisParticle> = Vec::new();
        let mut prisms_to_destroy: Vec<(i32, i32, i32)> = Vec::new();
        
        self.falling_prisms.retain(|prism| {
            if prism.grounded {
                // Prism hit the ground - convert to debris
                new_debris.extend(spawn_debris(prism.position, prism.material, 12));
                println!("[Physics] Falling prism IMPACTED ground at ({:.1}, {:.1}, {:.1})", 
                    prism.position.x, prism.position.y, prism.position.z);
                return false; // Remove falling prism
            }
            
            // Check if falling prism collides with another wall prism
            // Convert world position to approximate grid coords
            let approx_q = (prism.position.x / (DEFAULT_HEX_RADIUS * 1.732)).round() as i32;
            let approx_r = (prism.position.z / (DEFAULT_HEX_RADIUS * 1.5)).round() as i32;
            let approx_level = (prism.position.y / DEFAULT_HEX_HEIGHT).floor() as i32;
            
            // Check collision with nearby prisms
            for dq in -1..=1 {
                for dr in -1..=1 {
                    for dl in -1..=1 {
                        let check_coord = (approx_q + dq, approx_r + dr, approx_level + dl);
                        if self.hex_wall_grid.contains(check_coord.0, check_coord.1, check_coord.2) {
                            // Collision! Destroy the stationary prism too
                            prisms_to_destroy.push(check_coord);
                            new_debris.extend(spawn_debris(prism.position, prism.material, 6));
                            return false;
                        }
                    }
                }
            }
            
            // Keep falling if lifetime < 10 seconds (safety limit)
            prism.lifetime < 10.0
        });
        
        // Add spawned debris
        self.debris_particles.extend(new_debris);
        
        // Destroy prisms that were hit by falling debris
        for coord in prisms_to_destroy {
            self.destroy_prism_with_physics(coord);
        }
    }
    
    /// Update debris particles
    fn update_debris_particles(&mut self, delta_time: f32) {
        for particle in &mut self.debris_particles {
            particle.update(delta_time);
        }
        
        // Remove dead particles
        self.debris_particles.retain(|p| p.is_alive());
    }

    /// Update meteor system - spawn new meteors and handle impacts
    fn update_meteors(&mut self, delta_time: f32) {
        // Try to spawn new meteor
        if let Some(new_meteor) = self.meteor_spawner.update(delta_time, self.meteors.len()) {
            self.meteors.push(new_meteor);
        }

        // Update existing meteors and collect impacts
        let mut impacts: Vec<Vec3> = Vec::new();

        for meteor in &mut self.meteors {
            if let Some(impact_pos) = meteor.update(delta_time) {
                impacts.push(impact_pos);
            }
        }

        // Remove dead meteors
        self.meteors.retain(|m| m.is_alive());

        // Spawn debris at impact points
        for impact in impacts {
            let fire_debris = spawn_meteor_impact(impact, 15);
            self.debris_particles.extend(fire_debris);
        }
    }

    /// US-016: Regenerate hex-prism wall mesh and update GPU buffers after destruction
    fn regenerate_hex_wall_mesh(&mut self) {
        let Some(ref queue) = self.queue else { return };
        let Some(ref hex_wall_vb) = self.hex_wall_vertex_buffer else { return };
        let Some(ref hex_wall_ib) = self.hex_wall_index_buffer else { return };

        // Generate new mesh from remaining prisms
        let (hex_vertices, hex_indices) = self.hex_wall_grid.generate_combined_mesh();

        // Convert HexPrismVertex to Vertex
        let hex_wall_vertices: Vec<Vertex> = hex_vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            })
            .collect();

        // Update index count for rendering
        self.hex_wall_index_count = hex_indices.len() as u32;

        // Write updated mesh data to GPU buffers
        if !hex_wall_vertices.is_empty() {
            queue.write_buffer(hex_wall_vb, 0, bytemuck::cast_slice(&hex_wall_vertices));
            queue.write_buffer(hex_wall_ib, 0, bytemuck::cast_slice(&hex_indices));
        }

        // Clear the dirty flag
        self.hex_wall_grid.clear_mesh_dirty();

        println!(
            "[US-016] Mesh regenerated: {} prisms remaining ({} vertices, {} indices)",
            self.hex_wall_grid.len(),
            hex_wall_vertices.len(),
            hex_indices.len()
        );
    }

    fn fire_projectile(&mut self) {
        if self.projectiles.len() < 32 {
            let projectile = self.cannon.fire();
            self.projectiles.push(projectile);
            println!(
                "Fired projectile! Elevation: {:.1}°, Azimuth: {:.1}°, Active: {}",
                self.cannon.barrel_elevation.to_degrees(),
                self.cannon.barrel_azimuth.to_degrees(),
                self.projectiles.len()
            );
        }
    }
    
    /// Calculate the snapped position for block placement
    fn calculate_block_placement_position(&self) -> Option<Vec3> {
        let config = self.surface_config.as_ref()?;
        
        // Use mouse cursor position for placement (like an editor)
        // This allows precise selection of where to place
        let mouse_pos = self.current_mouse_pos.unwrap_or((
            config.width as f32 / 2.0,
            config.height as f32 / 2.0
        ));
        
        // Convert screen position to ray
        let aspect = config.width as f32 / config.height as f32;
        let half_fov = (self.camera.fov / 2.0).tan();
        
        let ndc_x = (mouse_pos.0 / config.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (mouse_pos.1 / config.height as f32) * 2.0;
        
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = right.cross(forward).normalize(); // Fixed: correct cross product order
        
        // Use same formula as hex grid's screen_to_ray (which works)
        let ray_dir = (forward + right * ndc_x * half_fov * aspect + up * ndc_y * half_fov)
            .normalize();
        
        let ray_origin = self.camera.position;
        
        // Find intersection with terrain or existing blocks
        let max_dist = 50.0;
        let step_size = 0.25;
        let mut t = 1.0;
        
        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            let ground_height = terrain_height_at(p.x, p.z, 0.0);
            
            // Check if we hit terrain
            if p.y <= ground_height {
                // Snap to grid
                let snapped_x = (p.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                let snapped_z = (p.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                let snapped_ground = terrain_height_at(snapped_x, snapped_z, 0.0);
                
                // Apply build height offset
                let y = snapped_ground + 0.5 + self.build_toolbar.build_height;
                let mut position = Vec3::new(snapped_x, y, snapped_z);
                
                // Try to snap to nearby existing blocks
                position = self.snap_to_nearby_blocks(position);
                
                return Some(position);
            }
            
            // Check if we hit an existing block (for stacking)
            if let Some((block_id, dist)) = self.block_manager.find_closest(p, 1.0) {
                if dist < 0.5 {
                    // Hit a block - place on top/side of it
                    if let Some(block) = self.block_manager.get_block(block_id) {
                        let aabb = block.aabb();
                        
                        // Determine which face we hit
                        let block_center = (aabb.min + aabb.max) * 0.5;
                        let hit_offset = p - block_center;
                        
                        // Find dominant axis
                        let abs_offset = Vec3::new(hit_offset.x.abs(), hit_offset.y.abs(), hit_offset.z.abs());
                        
                        let mut placement_pos;
                        if abs_offset.y > abs_offset.x && abs_offset.y > abs_offset.z {
                            // Top or bottom
                            if hit_offset.y > 0.0 {
                                placement_pos = Vec3::new(block_center.x, aabb.max.y + 0.5, block_center.z);
                            } else {
                                placement_pos = Vec3::new(block_center.x, aabb.min.y - 0.5, block_center.z);
                            }
                        } else if abs_offset.x > abs_offset.z {
                            // Left or right
                            if hit_offset.x > 0.0 {
                                placement_pos = Vec3::new(aabb.max.x + 0.5, block_center.y, block_center.z);
                            } else {
                                placement_pos = Vec3::new(aabb.min.x - 0.5, block_center.y, block_center.z);
                            }
                        } else {
                            // Front or back
                            if hit_offset.z > 0.0 {
                                placement_pos = Vec3::new(block_center.x, block_center.y, aabb.max.z + 0.5);
                            } else {
                                placement_pos = Vec3::new(block_center.x, block_center.y, aabb.min.z - 0.5);
                            }
                        }
                        
                        // Snap to grid
                        placement_pos.x = (placement_pos.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                        placement_pos.z = (placement_pos.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                        
                        return Some(placement_pos);
                    }
                }
            }
            
            t += step_size;
        }
        
        // If nothing hit, place at a reasonable distance
        let p = ray_origin + ray_dir * 10.0;
        let snapped_x = (p.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
        let snapped_z = (p.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
        let ground = terrain_height_at(snapped_x, snapped_z, 0.0);
        
        Some(Vec3::new(snapped_x, ground + 0.5 + self.build_toolbar.build_height, snapped_z))
    }
    
    /// Snap position to nearby existing blocks if close enough
    fn snap_to_nearby_blocks(&self, position: Vec3) -> Vec3 {
        let mut best_pos = position;
        let mut best_dist = BLOCK_SNAP_DISTANCE;
        
        for block in self.block_manager.blocks() {
            let aabb = block.aabb();
            let block_center = (aabb.min + aabb.max) * 0.5;
            
            // Check if we should snap to this block's grid
            let _dx = position.x - block_center.x;
            let _dz = position.z - block_center.z;
            
            // Possible snap positions (adjacent to this block)
            let snap_positions = [
                Vec3::new(block_center.x + BLOCK_GRID_SIZE, position.y, block_center.z), // Right
                Vec3::new(block_center.x - BLOCK_GRID_SIZE, position.y, block_center.z), // Left
                Vec3::new(block_center.x, position.y, block_center.z + BLOCK_GRID_SIZE), // Front
                Vec3::new(block_center.x, position.y, block_center.z - BLOCK_GRID_SIZE), // Back
                Vec3::new(block_center.x, aabb.max.y + 0.5, block_center.z), // Top
            ];
            
            for snap_pos in snap_positions {
                let dist = (snap_pos - position).length();
                if dist < best_dist {
                    best_dist = dist;
                    best_pos = snap_pos;
                }
            }
        }
        
        best_pos
    }
    
    /// Update the preview position for block placement
    fn update_block_preview(&mut self) {
        if self.build_toolbar.visible {
            self.build_toolbar.preview_position = self.calculate_block_placement_position();
        } else {
            self.build_toolbar.preview_position = None;
        }
    }
    
    /// Place a building block at the preview position
    fn place_building_block(&mut self) {
        // Use the preview position if available
        let position = match self.build_toolbar.preview_position {
            Some(pos) => pos,
            None => {
                // Fallback: calculate position now
                match self.calculate_block_placement_position() {
                    Some(pos) => pos,
                    None => return,
                }
            }
        };
        
        // Create block with selected shape
        let shape = self.build_toolbar.get_selected_shape();
        let block = BuildingBlock::new(shape, position, self.build_toolbar.selected_material);
        let block_id = self.block_manager.add_block(block);

        // Register with physics system
        self.block_physics.register_block(block_id);

        println!("[Build] Placed {} at ({:.1}, {:.1}, {:.1}) ID={}",
            SHAPE_NAMES[self.build_toolbar.selected_shape],
            position.x, position.y, position.z, block_id);

        self.regenerate_block_mesh();
    }
    
    /// Handle click in bridge mode - select faces
    fn handle_bridge_click(&mut self) -> bool {
        if !self.build_toolbar.is_bridge_mode() {
            return false;
        }
        
        // Raycast to find which block face is clicked
        if let Some(face) = self.raycast_to_face() {
            self.build_toolbar.bridge_tool.select_face(face);
            
            // If both faces selected, create the bridge
            if self.build_toolbar.bridge_tool.is_ready() {
                self.create_bridge();
                self.build_toolbar.bridge_tool.clear();
            }
            return true;
        }
        false
    }
    
    /// Raycast to find which face of a block is pointed at
    fn raycast_to_face(&self) -> Option<SelectedFace> {
        let mouse_pos = self.current_mouse_pos?;
        let config = self.surface_config.as_ref()?;
        
        let aspect = config.width as f32 / config.height as f32;
        let half_fov = (self.camera.fov / 2.0).tan();
        
        let ndc_x = (mouse_pos.0 / config.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (mouse_pos.1 / config.height as f32) * 2.0;
        
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = right.cross(forward).normalize(); // Fixed: correct cross product order
        
        // Use same formula as hex grid's screen_to_ray (which works)
        let ray_dir = (forward + right * ndc_x * half_fov * aspect + up * ndc_y * half_fov)
            .normalize();
        
        let ray_origin = self.camera.position;
        let max_dist = 50.0;
        let step_size = 0.1;
        let mut t = 0.5;
        
        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            
            // Check if we hit a block
            if let Some((block_id, dist)) = self.block_manager.find_closest(p, 1.0) {
                if dist < 0.3 {
                    if let Some(block) = self.block_manager.get_block(block_id) {
                        let aabb = block.aabb();
                        let center = (aabb.min + aabb.max) * 0.5;
                        let half_size = (aabb.max - aabb.min) * 0.5;
                        
                        // Determine which face was hit
                        let offset = p - center;
                        let abs_offset = Vec3::new(offset.x.abs(), offset.y.abs(), offset.z.abs());
                        
                        let (normal, face_pos, size) = if abs_offset.x > abs_offset.y && abs_offset.x > abs_offset.z {
                            // X face (left/right)
                            let n = Vec3::new(offset.x.signum(), 0.0, 0.0);
                            let pos = center + n * half_size.x;
                            (n, pos, (half_size.z * 2.0, half_size.y * 2.0))
                        } else if abs_offset.y > abs_offset.z {
                            // Y face (top/bottom)
                            let n = Vec3::new(0.0, offset.y.signum(), 0.0);
                            let pos = center + n * half_size.y;
                            (n, pos, (half_size.x * 2.0, half_size.z * 2.0))
                        } else {
                            // Z face (front/back)
                            let n = Vec3::new(0.0, 0.0, offset.z.signum());
                            let pos = center + n * half_size.z;
                            (n, pos, (half_size.x * 2.0, half_size.y * 2.0))
                        };
                        
                        return Some(SelectedFace {
                            block_id,
                            position: face_pos,
                            _normal: normal,
                            size,
                        });
                    }
                }
            }
            t += step_size;
        }
        None
    }
    
    /// Create a bridge between two selected faces
    fn create_bridge(&mut self) {
        let first = match self.build_toolbar.bridge_tool.first_face {
            Some(f) => f,
            None => return,
        };
        let second = match self.build_toolbar.bridge_tool.second_face {
            Some(f) => f,
            None => return,
        };
        
        // Calculate bridge parameters
        let start = first.position;
        let end = second.position;
        let direction = end - start;
        let length = direction.length();
        
        if length < 0.1 {
            println!("[Bridge] Faces too close together!");
            return;
        }
        
        // Create multiple connected blocks along the bridge
        let num_segments = (length / BLOCK_GRID_SIZE).ceil() as i32;
        let segment_length = length / num_segments as f32;
        let _dir_normalized = direction.normalize();
        
        // Average the face sizes for the bridge cross-section
        let _bridge_width = (first.size.0 + second.size.0) * 0.5;
        let _bridge_height = (first.size.1 + second.size.1) * 0.5;
        
        println!("[Bridge] Creating bridge with {} segments, length={:.1}", num_segments, length);
        
        for i in 0..=num_segments {
            let t = i as f32 / num_segments as f32;
            let pos = start + direction * t;
            
            // Interpolate size from first to second face
            let w = first.size.0 * (1.0 - t) + second.size.0 * t;
            let h = first.size.1 * (1.0 - t) + second.size.1 * t;
            
            // Create a cube block at this position with interpolated size
            let half_extents = Vec3::new(
                segment_length * 0.6,
                h * 0.5,
                w * 0.5,
            );
            
            let shape = BuildingBlockShape::Cube { half_extents };
            let block = BuildingBlock::new(shape, pos, self.build_toolbar.selected_material);
            let block_id = self.block_manager.add_block(block);
            // Register with physics system
            self.block_physics.register_block(block_id);
        }

        println!("[Bridge] Bridge created between blocks {} and {}", first.block_id, second.block_id);
        self.regenerate_block_mesh();
    }
    
    /// Update building physics - runs every frame for smooth falling
    /// do_support_check: when true, also runs the expensive support graph check
    fn update_building_physics(&mut self, dt: f32, do_support_check: bool) {
        if self.block_manager.blocks().is_empty() {
            return;
        }
        
        // Log support check
        if do_support_check {
            let block_count = self.block_manager.blocks().len();
            println!("[Physics] Running support check on {} blocks...", block_count);
        }
        
        // Update physics simulation (this handles both physics AND support checks via pending_checks)
        self.block_physics.update(dt, &mut self.block_manager);

        // Get blocks that need to be removed (disintegrated)
        let blocks_to_remove = self.block_physics.take_blocks_to_remove();

        // Remove disintegrated blocks and spawn debris
        for block_id in &blocks_to_remove {
            if let Some(block) = self.block_manager.get_block(*block_id) {
                let position = block.position;
                let material = block.material;

                // Create debris particles for visual effect
                let debris = spawn_debris(position, material, 8);
                self.debris_particles.extend(debris);

                println!("[Physics] Block {} disintegrated!", block_id);

                // Unregister from physics
                self.block_physics.unregister_block(*block_id);
            }

            self.block_manager.remove_block(*block_id);
        }

        // Check for blocks that are currently falling and create visual effects
        let mut needs_mesh_update = !blocks_to_remove.is_empty();
        for block in self.block_manager.blocks() {
            if self.block_physics.is_falling(block.id) {
                needs_mesh_update = true;
            }
        }

        // Regenerate mesh if physics changed anything
        if needs_mesh_update {
            self.regenerate_block_mesh();
        }
    }
    
    /// Raycast from camera to find which building block is hit
    fn raycast_to_block(&self) -> Option<u32> {
        let mouse_pos = self.current_mouse_pos?;
        let config = self.surface_config.as_ref()?;
        
        // Convert screen position to ray
        let aspect = config.width as f32 / config.height as f32;
        let half_fov = (self.camera.fov / 2.0).tan();
        
        // NDC coordinates (-1 to 1)
        let ndc_x = (mouse_pos.0 / config.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (mouse_pos.1 / config.height as f32) * 2.0;
        
        // Calculate ray direction
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = right.cross(forward).normalize(); // Fixed: correct cross product order
        
        // Use same formula as hex grid's screen_to_ray (which works)
        let ray_dir = (forward + right * ndc_x * half_fov * aspect + up * ndc_y * half_fov)
            .normalize();
        
        let ray_origin = self.camera.position;
        
        // Find closest block by marching along ray and checking SDF
        let max_dist = 100.0;
        let step_size = 0.25;
        let mut t = 0.5;
        
        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            
            // Check if we're inside any block
            if let Some((block_id, dist)) = self.block_manager.find_closest(p, 0.5) {
                if dist < 0.5 {
                    return Some(block_id);
                }
            }
            
            t += step_size;
        }
        
        None
    }
    
    /// Raycast from camera to find a loose (physics-detached) block
    /// Returns (block_id, shape, material) if found
    fn raycast_to_loose_block(&self) -> Option<(u32, BuildingBlockShape, u8)> {
        let mouse_pos = self.current_mouse_pos?;
        let config = self.surface_config.as_ref()?;
        
        // Convert screen position to ray
        let aspect = config.width as f32 / config.height as f32;
        let half_fov = (self.camera.fov / 2.0).tan();
        
        // NDC coordinates (-1 to 1)
        let ndc_x = (mouse_pos.0 / config.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (mouse_pos.1 / config.height as f32) * 2.0;
        
        // Calculate ray direction
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = right.cross(forward).normalize();
        
        let ray_dir = (forward + right * ndc_x * half_fov * aspect + up * ndc_y * half_fov)
            .normalize();
        
        let ray_origin = self.camera.position;
        
        // Find closest loose block by marching along ray
        let max_dist = 20.0; // Shorter range for pickup
        let step_size = 0.25;
        let mut t = 0.5;
        
        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            
            // Check if we're inside any block
            if let Some((block_id, dist)) = self.block_manager.find_closest(p, 0.5) {
                if dist < 0.5 {
                    // Check if this block is loose (detached from structure)
                    if self.block_physics.is_loose(block_id) {
                        if let Some(block) = self.block_manager.get_block(block_id) {
                            return Some((block_id, block.shape, block.material));
                        }
                    }
                }
            }
            
            t += step_size;
        }
        
        None
    }
    
    /// Handle block click for double-click merging
    fn handle_block_click(&mut self) -> bool {
        if let Some(block_id) = self.raycast_to_block() {
            // Check for double-click
            if let Some(blocks_to_merge) = self.merge_workflow.on_block_click(block_id, &self.block_manager) {
                // Double-click detected! Merge connected blocks
                println!("[Merge] Double-click on block {}, merging {} connected blocks", 
                    block_id, blocks_to_merge.len());
                
                // Get material color based on first block
                let color = if let Some(block) = self.block_manager.get_block(blocks_to_merge[0]) {
                    get_material_color(block.material)
                } else {
                    [0.5, 0.5, 0.5, 1.0]
                };
                
                // Perform merge
                if let Some(merged) = self.merge_workflow.merge_blocks(
                    &blocks_to_merge, 
                    &mut self.block_manager,
                    color
                ) {
                    println!("[Merge] Created merged mesh with {} vertices", merged.vertices.len());
                    self.create_merged_mesh_buffers(merged);
                }
                
                // Regenerate remaining block mesh
                self.regenerate_block_mesh();
                return true;
            }
        }
        false
    }
    
    /// Check player collision with building blocks and push out if overlapping
    fn player_block_collision(&mut self) {
        let player_pos = self.player.position;
        let player_top = player_pos.y + PLAYER_EYE_HEIGHT + 0.2; // Player capsule top
        
        // Check each block for collision
        for block in self.block_manager.blocks() {
            let aabb = block.aabb();
            
            // Simple capsule-AABB collision
            // Check if player cylinder overlaps with block AABB
            let closest_x = player_pos.x.clamp(aabb.min.x, aabb.max.x);
            let closest_z = player_pos.z.clamp(aabb.min.z, aabb.max.z);
            
            let dx = player_pos.x - closest_x;
            let dz = player_pos.z - closest_z;
            let horizontal_dist = (dx * dx + dz * dz).sqrt();
            
            // Player capsule radius
            let player_radius = 0.3;
            
            // Check horizontal overlap
            if horizontal_dist < player_radius {
                // Check vertical overlap
                let in_vertical_range = player_pos.y < aabb.max.y && player_top > aabb.min.y;
                
                if in_vertical_range {
                    // Collision! Push player out horizontally
                    if horizontal_dist > 0.001 {
                        let push_dir = Vec3::new(dx, 0.0, dz).normalize();
                        let penetration = player_radius - horizontal_dist;
                        self.player.position += push_dir * (penetration + 0.01);
                        
                        // Calculate impact force from player velocity
                        let vel_dot = self.player.velocity.dot(push_dir);
                        if vel_dot < 0.0 {
                            // Apply impulse to loose blocks (player bumps them)
                            if self.block_physics.is_loose(block.id) {
                                let player_mass = 70.0; // kg
                                let impulse = -push_dir * vel_dot.abs() * player_mass * 0.3;
                                self.block_physics.apply_impulse(block.id, impulse);
                            }
                            
                            // Trigger support check on collision
                            self.block_physics.trigger_support_check(block.id);
                            
                            // Stop velocity in that direction
                            self.player.velocity -= push_dir * vel_dot;
                        }
                    } else {
                        // Player is inside block, push toward center of AABB's nearest face
                        let block_center = (aabb.min + aabb.max) * 0.5;
                        let to_player = player_pos - block_center;
                        let push_dir = Vec3::new(to_player.x.signum(), 0.0, to_player.z.signum()).normalize_or_zero();
                        self.player.position += push_dir * (player_radius + 0.1);
                    }
                }
                
                // Check if player is standing on top of block
                if player_pos.y >= aabb.max.y - 0.1 && player_pos.y <= aabb.max.y + 0.5 {
                    let on_top_xz = player_pos.x >= aabb.min.x - player_radius 
                        && player_pos.x <= aabb.max.x + player_radius
                        && player_pos.z >= aabb.min.z - player_radius 
                        && player_pos.z <= aabb.max.z + player_radius;
                    
                    if on_top_xz && self.player.vertical_velocity <= 0.0 {
                        // Land on top of block
                        self.player.position.y = aabb.max.y;
                        self.player.vertical_velocity = 0.0;
                        self.player.is_grounded = true;
                        self.player.coyote_time_remaining = COYOTE_TIME;
                    }
                }
            }
        }
    }
    
    /// Check player collision with hex prism walls
    fn player_hex_collision(&mut self) {
        let player_pos = self.player.position;
        let player_radius = 0.3;
        
        // Iterate through nearby hex prisms
        for ((q, r, level), _prism) in self.hex_wall_grid.iter() {
            // Convert hex coordinates to world position
            let hex_x = (*q as f32) * DEFAULT_HEX_RADIUS * 1.5;
            let hex_z = (*r as f32) * DEFAULT_HEX_RADIUS * 3.0_f32.sqrt() 
                + (*q as f32).abs() % 2.0 * DEFAULT_HEX_RADIUS * 3.0_f32.sqrt() * 0.5;
            let hex_y = (*level as f32) * DEFAULT_HEX_HEIGHT;
            
            // Simple distance check for collision
            let dx = player_pos.x - hex_x;
            let dz = player_pos.z - hex_z;
            let horizontal_dist = (dx * dx + dz * dz).sqrt();
            
            // Hex prism collision radius (inscribed circle)
            let hex_collision_radius = DEFAULT_HEX_RADIUS * 0.866; // cos(30°) ≈ 0.866
            
            if horizontal_dist < hex_collision_radius + player_radius {
                // Check vertical overlap
                let hex_bottom = hex_y;
                let hex_top = hex_y + DEFAULT_HEX_HEIGHT;
                let player_top = player_pos.y + PLAYER_EYE_HEIGHT + 0.2;
                
                let in_vertical_range = player_pos.y < hex_top && player_top > hex_bottom;
                
                if in_vertical_range {
                    // Push player out
                    if horizontal_dist > 0.001 {
                        let push_dir = Vec3::new(dx, 0.0, dz).normalize();
                        let penetration = (hex_collision_radius + player_radius) - horizontal_dist;
                        self.player.position += push_dir * (penetration + 0.01);
                        
                        // Stop velocity in push direction
                        let vel_dot = self.player.velocity.dot(push_dir);
                        if vel_dot < 0.0 {
                            self.player.velocity -= push_dir * vel_dot;
                        }
                    }
                }
                
                // Check if player is standing on top of hex prism
                if player_pos.y >= hex_top - 0.1 && player_pos.y <= hex_top + 0.5 {
                    if horizontal_dist < hex_collision_radius + player_radius 
                        && self.player.vertical_velocity <= 0.0 
                    {
                        // Land on top of hex prism
                        self.player.position.y = hex_top;
                        self.player.vertical_velocity = 0.0;
                        self.player.is_grounded = true;
                        self.player.coyote_time_remaining = COYOTE_TIME;
                    }
                }
            }
        }
    }
    
    /// Create GPU buffers for a merged mesh
    fn create_merged_mesh_buffers(&mut self, merged: MergedMesh) {
        let device = match &self.device {
            Some(d) => d,
            None => return,
        };
        
        if merged.vertices.is_empty() {
            return;
        }
        
        // Convert to Vertex format
        let mesh_vertices: Vec<Vertex> = merged.vertices.iter().map(|v| Vertex {
            position: v.position,
            normal: v.normal,
            color: v.color,
        }).collect();
        
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Merged Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Merged Mesh Index Buffer"),
            contents: bytemuck::cast_slice(&merged.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        
        self.merged_mesh_buffers.push(MergedMeshBuffers {
            _id: merged.id,
            vertex_buffer,
            index_buffer,
            index_count: merged.indices.len() as u32,
        });
        
        println!("[Merge] Created GPU buffers for merged mesh {}", merged.id);
    }
    
    /// Regenerate the building block mesh buffer
    fn regenerate_block_mesh(&mut self) {
        let device = match &self.device {
            Some(d) => d,
            None => return,
        };
        let _queue = match &self.queue {
            Some(q) => q,
            None => return,
        };
        
        // Generate combined mesh from all blocks
        let (vertices, indices) = self.block_manager.generate_combined_mesh();
        
        if vertices.is_empty() {
            self.block_index_count = 0;
            return;
        }
        
        // Convert BlockVertex to Vertex (same layout)
        let mesh_vertices: Vec<Vertex> = vertices.iter().map(|v| Vertex {
            position: v.position,
            normal: v.normal,
            color: v.color,
        }).collect();
        
        // Create or update buffers
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Block Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Block Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        
        self.block_vertex_buffer = Some(vertex_buffer);
        self.block_index_buffer = Some(index_buffer);
        self.block_index_count = indices.len() as u32;
        
        println!("[Build] Mesh updated: {} vertices, {} indices", 
            mesh_vertices.len(), indices.len());
    }
    
    // ========================================================================
    // TERRAIN REBUILD (called when UI applies changes)
    // ========================================================================
    
    fn rebuild_terrain(&mut self) {
        let device = self.device.as_ref().expect("Device not initialized");
        let _queue = self.queue.as_ref().expect("Queue not initialized");
        
        // Island positions - must match initial generation
        const ATTACKER_CENTER: Vec3 = Vec3::new(0.0, 0.0, 45.0);
        const DEFENDER_CENTER: Vec3 = Vec3::new(0.0, 0.0, -45.0);
        const ISLAND_RADIUS: f32 = 30.0;
        
        // Regenerate terrain mesh
        let mut static_mesh = Mesh::new();
        
        // Attacker platform
        let attacker_platform = generate_elevated_hex_terrain(
            ATTACKER_CENTER,
            ISLAND_RADIUS,
            [0.4, 0.5, 0.3, 1.0],
            64,
        );
        static_mesh.merge(&attacker_platform);
        
        // Water for attacker
        let params = get_terrain_params();
        if params.water > 0.01 {
            let attacker_water = generate_lava_plane(ATTACKER_CENTER, ISLAND_RADIUS);
            static_mesh.merge(&attacker_water);
        }
        
        // Defender platform
        let defender_platform = generate_elevated_hex_terrain(
            DEFENDER_CENTER,
            ISLAND_RADIUS,
            [0.5, 0.4, 0.35, 1.0],
            64,
        );
        static_mesh.merge(&defender_platform);
        
        // Water for defender
        if params.water > 0.01 {
            let defender_water = generate_lava_plane(DEFENDER_CENTER, ISLAND_RADIUS);
            static_mesh.merge(&defender_water);
        }
        
        // Generate bridge - find actual terrain edge vertices
        let mut bridge_start = Vec3::new(0.0, 0.0, ATTACKER_CENTER.z);
        let mut best_dist_start = f32::MAX;
        for v in &attacker_platform.vertices {
            let vx = v.position[0];
            let vz = v.position[2];
            let dist = vx.abs() + (vz - (ATTACKER_CENTER.z - ISLAND_RADIUS)).abs() * 0.5;
            if dist < best_dist_start && vz < ATTACKER_CENTER.z {
                best_dist_start = dist;
                bridge_start = Vec3::new(v.position[0], v.position[1], v.position[2]);
            }
        }
        
        let mut bridge_end = Vec3::new(0.0, 0.0, DEFENDER_CENTER.z);
        let mut best_dist_end = f32::MAX;
        for v in &defender_platform.vertices {
            let vx = v.position[0];
            let vz = v.position[2];
            let dist = vx.abs() + ((DEFENDER_CENTER.z + ISLAND_RADIUS) - vz).abs() * 0.5;
            if dist < best_dist_end && vz > DEFENDER_CENTER.z {
                best_dist_end = dist;
                bridge_end = Vec3::new(v.position[0], v.position[1], v.position[2]);
            }
        }
        
        let bridge_config = BridgeConfig::default();
        let bridge_mesh = generate_bridge(bridge_start, bridge_end, &bridge_config);
        static_mesh.merge(&bridge_mesh);
        
        // Update buffers
        self.static_vertex_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Static Vertex Buffer"),
            contents: bytemuck::cast_slice(&static_mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }));
        
        self.static_index_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Static Index Buffer"),
            contents: bytemuck::cast_slice(&static_mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        }));
        
        self.static_index_count = static_mesh.indices.len() as u32;
        
        println!("Terrain rebuilt! {} vertices, {} triangles", 
            static_mesh.vertices.len(), 
            static_mesh.indices.len() / 3);
        
        // Also regenerate trees (they depend on terrain height)
        self.trees_attacker = generate_trees_on_terrain(
            ATTACKER_CENTER, 28.0, 0.3, 0.0,
        );
        self.trees_defender = generate_trees_on_terrain(
            DEFENDER_CENTER, 28.0, 0.35, 100.0,
        );
        
        let mut all_trees = self.trees_attacker.clone();
        all_trees.extend(self.trees_defender.clone());
        let tree_mesh = generate_all_trees_mesh(&all_trees);
        
        if !tree_mesh.vertices.is_empty() {
            self.tree_vertex_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tree Vertex Buffer"),
                contents: bytemuck::cast_slice(&tree_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }));
            self.tree_index_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tree Index Buffer"),
                contents: bytemuck::cast_slice(&tree_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            }));
            self.tree_index_count = tree_mesh.indices.len() as u32;
        }
        
        println!("[Trees] Regenerated {} trees", all_trees.len());
    }
    
    // ========================================================================
    // BUILDER MODE: Raycast and grid positioning
    // ========================================================================
    
    /// Convert screen coordinates to a world-space ray
    fn screen_to_ray(&self, screen_x: f32, screen_y: f32) -> (Vec3, Vec3) {
        let Some(ref config) = self.surface_config else {
            return (self.camera.position, self.camera.get_forward());
        };
        
        let width = config.width as f32;
        let height = config.height as f32;
        
        // Convert to normalized device coordinates (-1 to 1)
        let ndc_x = (2.0 * screen_x / width) - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_y / height); // Flip Y
        
        // Get camera vectors
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = right.cross(forward).normalize();
        
        // Calculate ray direction based on FOV and aspect ratio
        let aspect = width / height;
        let half_fov_tan = (self.camera.fov / 2.0).tan();
        
        let ray_dir = (forward + right * ndc_x * half_fov_tan * aspect + up * ndc_y * half_fov_tan).normalize();
        
        (self.camera.position, ray_dir)
    }
    
    /// Raycast from screen position to find build position on terrain or existing prisms
    fn raycast_to_build_position(&self, screen_x: f32, screen_y: f32) -> Option<(i32, i32, i32)> {
        let (ray_origin, ray_dir) = self.screen_to_ray(screen_x, screen_y);
        
        // First, check if we hit an existing prism (to build on top)
        if let Some(hit) = self.hex_wall_grid.ray_cast(ray_origin, ray_dir, 200.0) {
            // Place on top of the hit prism
            let above = (hit.prism_coord.0, hit.prism_coord.1, hit.prism_coord.2 + 1);
            return Some(above);
        }
        
        // Otherwise, raycast against the terrain (both hex platforms)
        // Use an iterative approach to find where the ray intersects the terrain
        let attacker_center = Vec3::new(0.0, 0.0, 45.0);
        let defender_center = Vec3::new(0.0, 0.0, -45.0);
        let platform_radius = 30.0;
        
        // March along the ray to find terrain intersection
        let max_dist = 200.0;
        let step_size = 0.5;
        let mut t = 0.0;
        
        while t < max_dist {
            let point = ray_origin + ray_dir * t;
            
            // Check if point is within either hex platform bounds
            let in_attacker = (point.x - attacker_center.x).powi(2) + (point.z - attacker_center.z).powi(2) < platform_radius * platform_radius;
            let in_defender = (point.x - defender_center.x).powi(2) + (point.z - defender_center.z).powi(2) < platform_radius * platform_radius;
            
            if in_attacker || in_defender {
                let base_y = if in_attacker { attacker_center.y } else { defender_center.y };
                let terrain_y = terrain_height_at(point.x, point.z, base_y);
                
                // Check if ray has crossed below terrain
                if point.y <= terrain_y + 0.1 {
                    // Found intersection! Convert to grid coordinates
                    let build_level = self.builder_mode.build_level;
                    let (q, r, _) = battle_tok_engine::render::hex_prism::world_to_axial(point);
                    return Some((q, r, build_level));
                }
            }
            
            t += step_size;
        }
        
        None
    }
    
    /// Update builder mode cursor position from current mouse position
    fn update_builder_cursor(&mut self) {
        if !self.builder_mode.enabled {
            self.builder_mode.cursor_coord = None;
            return;
        }
        
        if let Some((mx, my)) = self.current_mouse_pos {
            self.builder_mode.cursor_coord = self.raycast_to_build_position(mx, my);
        }
    }
    
    /// Generate ghost preview mesh for builder mode
    fn generate_ghost_preview_mesh(&self, time: f32) -> Option<Mesh> {
        if !self.builder_mode.enabled || !self.builder_mode.show_preview {
            return None;
        }
        
        let coord = self.builder_mode.cursor_coord?;
        
        // Don't show ghost if position is already occupied
        if self.hex_wall_grid.contains(coord.0, coord.1, coord.2) {
            return None;
        }
        
        // Create a ghost prism at the cursor position
        let world_pos = battle_tok_engine::render::hex_prism::axial_to_world(coord.0, coord.1, coord.2);
        let prism = battle_tok_engine::render::HexPrism::with_center(
            world_pos,
            battle_tok_engine::render::hex_prism::DEFAULT_HEX_HEIGHT,
            battle_tok_engine::render::hex_prism::DEFAULT_HEX_RADIUS,
            self.builder_mode.selected_material,
        );
        
        let (hex_vertices, hex_indices) = prism.generate_mesh();
        
        // Convert to our Vertex format with pulsing green color
        let pulse = 0.5 + (time * 4.0).sin() * 0.3;
        let ghost_color = [0.3, 0.9, 0.4, pulse]; // Green with pulsing alpha
        
        let vertices: Vec<Vertex> = hex_vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: ghost_color,
            })
            .collect();
        
        Some(Mesh { vertices, indices: hex_indices })
    }
    
    /// Generate grid overlay mesh for builder mode
    /// Shows hex grid cells around the cursor position
    fn generate_grid_overlay_mesh(&self) -> Option<Mesh> {
        if !self.builder_mode.enabled {
            return None;
        }
        
        let Some(coord) = self.builder_mode.cursor_coord else {
            return None;
        };
        
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        
        // Grid parameters
        let line_thickness = 0.04;
        let grid_radius: i32 = 3;
        let hex_radius = battle_tok_engine::render::hex_prism::DEFAULT_HEX_RADIUS;
        
        // Generate hex outlines around cursor
        for dq in -grid_radius..=grid_radius {
            for dr in -grid_radius..=grid_radius {
                // Hex distance check (axial coordinates)
                let ds = -dq - dr;
                let hex_dist = (dq.abs() + dr.abs() + ds.abs()) / 2;
                if hex_dist > grid_radius {
                    continue;
                }
                
                let q = coord.0 + dq;
                let r = coord.1 + dr;
                let world_pos = battle_tok_engine::render::hex_prism::axial_to_world(q, r, coord.2);
                
                // Color - highlight cursor cell differently
                let is_cursor = dq == 0 && dr == 0;
                let cell_color = if is_cursor {
                    [0.2, 1.0, 0.4, 0.9] // Bright green for cursor
                } else {
                    [0.3, 0.7, 1.0, 0.4] // Cyan for others
                };
                
                // Generate 6 edge quads for the hex outline
                for i in 0..6 {
                    let angle1 = (i as f32) * std::f32::consts::PI / 3.0;
                    let angle2 = ((i + 1) % 6) as f32 * std::f32::consts::PI / 3.0;
                    
                    // Hex vertices (pointy-top orientation)
                    let x1 = world_pos.x + hex_radius * angle1.sin();
                    let z1 = world_pos.z + hex_radius * angle1.cos();
                    let x2 = world_pos.x + hex_radius * angle2.sin();
                    let z2 = world_pos.z + hex_radius * angle2.cos();
                    
                    // Sample terrain height + small offset above terrain
                    let y1 = terrain_height_at(x1, z1, 0.0) + 0.1;
                    let y2 = terrain_height_at(x2, z2, 0.0) + 0.1;
                    
                    // Calculate perpendicular offset for line thickness
                    let edge_dx = x2 - x1;
                    let edge_dz = z2 - z1;
                    let edge_len = (edge_dx * edge_dx + edge_dz * edge_dz).sqrt();
                    if edge_len < 0.001 { continue; }
                    
                    let perp_x = -edge_dz / edge_len * line_thickness;
                    let perp_z = edge_dx / edge_len * line_thickness;
                    
                    // Create quad for this edge (4 vertices)
                    let base_idx = vertices.len() as u32;
                    
                    vertices.push(Vertex { 
                        position: [x1 - perp_x, y1, z1 - perp_z], 
                        normal: [0.0, 1.0, 0.0], 
                        color: cell_color 
                    });
                    vertices.push(Vertex { 
                        position: [x1 + perp_x, y1, z1 + perp_z], 
                        normal: [0.0, 1.0, 0.0], 
                        color: cell_color 
                    });
                    vertices.push(Vertex { 
                        position: [x2 + perp_x, y2, z2 + perp_z], 
                        normal: [0.0, 1.0, 0.0], 
                        color: cell_color 
                    });
                    vertices.push(Vertex { 
                        position: [x2 - perp_x, y2, z2 - perp_z], 
                        normal: [0.0, 1.0, 0.0], 
                        color: cell_color 
                    });
                    
                    // Two triangles per quad
                    indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
                    indices.extend_from_slice(&[base_idx, base_idx + 2, base_idx + 3]);
                }
            }
        }
        
        if vertices.is_empty() {
            None
        } else {
            Some(Mesh { vertices, indices })
        }
    }

    fn render(&mut self) {
        let Some(ref device) = self.device else { return };
        let Some(ref queue) = self.queue else { return };
        let Some(ref surface) = self.surface else { return };
        let Some(ref config) = self.surface_config else { return };
        let Some(ref pipeline) = self.pipeline else { return };
        let Some(ref uniform_buffer) = self.uniform_buffer else { return };
        let Some(ref bind_group) = self.uniform_bind_group else { return };
        let Some(ref static_vb) = self.static_vertex_buffer else { return };
        let Some(ref static_ib) = self.static_index_buffer else { return };
        let Some(ref dynamic_vb) = self.dynamic_vertex_buffer else { return };
        let Some(ref dynamic_ib) = self.dynamic_index_buffer else { return };
        let Some(ref hex_wall_vb) = self.hex_wall_vertex_buffer else { return };
        let Some(ref hex_wall_ib) = self.hex_wall_index_buffer else { return };
        let Some(ref depth_view) = self.depth_texture else { return };

        // Get surface texture
        let output = match surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Get time for animations
        let time = self.start_time.elapsed().as_secs_f32();
        
        // Build dynamic mesh (projectiles + builder mode preview)
        let mut dynamic_mesh = Mesh::new();

        // Projectile meshes (spheres)
        for projectile in &self.projectiles {
            let sphere = generate_sphere(projectile.position, projectile.radius, [0.2, 0.2, 0.2, 1.0], 4); // US-019: Reduced from 8 to 4 segments (4.5x fewer triangles)
            dynamic_mesh.merge(&sphere);
        }
        
        // Falling prisms (physics-based destruction)
        for falling in &self.falling_prisms {
            let color = get_material_color(falling.material);
            // Render as a simple box for performance (actual hex would need rotation math)
            let half_size = Vec3::new(DEFAULT_HEX_RADIUS * 0.8, DEFAULT_HEX_HEIGHT * 0.5, DEFAULT_HEX_RADIUS * 0.8);
            let box_mesh = generate_rotated_box(falling.position, half_size, falling.rotation, color);
            dynamic_mesh.merge(&box_mesh);
        }
        
        // Debris particles (small cubes for visibility)
        for particle in &self.debris_particles {
            if particle.is_alive() {
                // Fade out based on lifetime
                let alpha = (particle.lifetime / 3.0).min(1.0);
                let mut color = particle.color;
                color[3] = alpha;
                let cube = generate_sphere(particle.position, particle.size, color, 2); // Low-poly sphere
                dynamic_mesh.merge(&cube);
            }
        }

        // Falling meteors (HDR emissive fireballs)
        for meteor in &self.meteors {
            if meteor.is_alive() {
                // Meteor core (bright HDR emissive)
                let fireball = generate_sphere(meteor.position, meteor.size, meteor.color, 4);
                dynamic_mesh.merge(&fireball);

                // Optional: trail effect (smaller spheres behind)
                if meteor.velocity.length() > 1.0 {
                    let trail_dir = -meteor.velocity.normalize();
                    for i in 1..4 {
                        let trail_pos = meteor.position + trail_dir * (i as f32) * meteor.size * 0.8;
                        let trail_size = meteor.size * (0.8 - i as f32 * 0.15);
                        let trail_alpha = 1.0 - (i as f32 * 0.25);
                        let trail_color = [
                            meteor.color[0] * trail_alpha,
                            meteor.color[1] * trail_alpha,
                            meteor.color[2] * trail_alpha * 0.5,
                            trail_alpha,
                        ];
                        let trail = generate_sphere(trail_pos, trail_size, trail_color, 3);
                        dynamic_mesh.merge(&trail);
                    }
                }
            }
        }

        // Builder mode: Ghost preview at cursor position
        if let Some(ghost_mesh) = self.generate_ghost_preview_mesh(time) {
            dynamic_mesh.merge(&ghost_mesh);
        }
        
        // Builder mode: Grid overlay around cursor
        if let Some(grid_mesh) = self.generate_grid_overlay_mesh() {
            dynamic_mesh.merge(&grid_mesh);
        }

        // Update dynamic buffers
        if !dynamic_mesh.vertices.is_empty() {
            queue.write_buffer(dynamic_vb, 0, bytemuck::cast_slice(&dynamic_mesh.vertices));
            queue.write_buffer(dynamic_ib, 0, bytemuck::cast_slice(&dynamic_mesh.indices));
        }
        self.dynamic_index_count = dynamic_mesh.indices.len() as u32;

        // Update uniforms
        let aspect = config.width as f32 / config.height as f32;
        let view_mat = self.camera.get_view_matrix();
        let proj_mat = self.camera.get_projection_matrix(aspect);
        let view_proj = proj_mat * view_mat;

        let mut uniforms = Uniforms::default();
        uniforms.view_proj = view_proj.to_cols_array_2d();
        uniforms.camera_pos = self.camera.position.to_array();
        uniforms.time = time;
        uniforms.projectile_count = self.projectiles.len() as u32;

        for (i, projectile) in self.projectiles.iter().enumerate().take(32) {
            uniforms.projectile_positions[i] = [
                projectile.position.x,
                projectile.position.y,
                projectile.position.z,
                projectile.radius,
            ];
        }

        queue.write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // ============================================
        // US-013: Update SDF cannon uniforms
        // ============================================
        if let (Some(sdf_uniform_buffer), Some(sdf_data_buffer)) =
            (&self.sdf_cannon_uniform_buffer, &self.sdf_cannon_data_buffer)
        {
            let inv_view_proj = view_proj.inverse();

            // SDF cannon uniforms (camera, time, lighting)
            // Low sun angle for dramatic rim lighting (apocalyptic battle atmosphere)
            let sdf_uniforms = SdfCannonUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                inv_view_proj: inv_view_proj.to_cols_array_2d(),
                camera_pos: self.camera.position.to_array(),
                time,
                sun_dir: [0.3, 0.25, -0.92],  // Slightly higher sun for better lighting
                fog_density: 0.004,           // Lighter fog to preserve colors
                fog_color: [0.65, 0.45, 0.55], // Warmer, more saturated fog
                ambient: 0.25,                // Higher ambient for richer colors
            };
            queue.write_buffer(sdf_uniform_buffer, 0, bytemuck::cast_slice(&[sdf_uniforms]));

            // SDF cannon data (position, rotation, color)
            // Convert barrel rotation from euler angles to quaternion
            let (sin_az, cos_az) = (self.cannon.barrel_azimuth * 0.5).sin_cos();
            let (sin_elev, cos_elev) = (self.cannon.barrel_elevation * 0.5).sin_cos();

            // Quaternion for Y rotation (azimuth) then X rotation (elevation)
            // q = q_y * q_x (apply elevation first, then azimuth)
            let quat_x = [sin_elev, 0.0, 0.0, cos_elev]; // Rotation around X
            let quat_y = [0.0, sin_az, 0.0, cos_az];     // Rotation around Y

            // Quaternion multiplication: q_y * q_x
            let barrel_rotation = [
                quat_y[3] * quat_x[0] + quat_y[0] * quat_x[3] + quat_y[1] * quat_x[2] - quat_y[2] * quat_x[1],
                quat_y[3] * quat_x[1] - quat_y[0] * quat_x[2] + quat_y[1] * quat_x[3] + quat_y[2] * quat_x[0],
                quat_y[3] * quat_x[2] + quat_y[0] * quat_x[1] - quat_y[1] * quat_x[0] + quat_y[2] * quat_x[3],
                quat_y[3] * quat_x[3] - quat_y[0] * quat_x[0] - quat_y[1] * quat_x[1] - quat_y[2] * quat_x[2],
            ];

            let sdf_cannon_data = SdfCannonData {
                world_pos: self.cannon.position.to_array(),
                _pad0: 0.0,
                barrel_rotation,
                color: [0.4, 0.35, 0.3], // Bronze/metallic color
                _pad1: 0.0,
            };
            queue.write_buffer(sdf_data_buffer, 0, bytemuck::cast_slice(&[sdf_cannon_data]));
        }

        // ============================================
        // Update stormy skybox uniforms
        // ============================================
        if let Some(ref mut stormy_sky) = self.stormy_sky {
            stormy_sky.update(
                queue,
                view_proj,
                self.camera.position,
                time,
                (config.width, config.height),
            );
        }

        // ============================================
        // Update material system scene uniforms
        // ============================================
        if let Some(ref material_system) = self.material_system {
            material_system.update_scene_uniforms(
                queue,
                view_proj,
                self.camera.position,
                time,
            );
        }

        // ============================================
        // Update fog post-pass uniforms
        // ============================================
        if let Some(ref fog_post) = self.fog_post {
            fog_post.update(queue, view_proj, self.camera.position);
        }

        // Create command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // ============================================
        // PASS 0: Stormy skybox (background)
        // ============================================
        if let Some(ref stormy_sky) = self.stormy_sky {
            stormy_sky.render_to_view(&mut encoder, &view);
        }

        // ============================================
        // PASS 1: Mesh rendering (terrain, walls, projectiles)
        // ============================================
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Mesh Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Load sky background (don't clear)
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);

            // Draw static mesh (terrain)
            render_pass.set_vertex_buffer(0, static_vb.slice(..));
            render_pass.set_index_buffer(static_ib.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.static_index_count, 0, 0..1);

            // Draw dynamic mesh (projectiles)
            if self.dynamic_index_count > 0 {
                render_pass.set_vertex_buffer(0, dynamic_vb.slice(..));
                render_pass.set_index_buffer(dynamic_ib.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..self.dynamic_index_count, 0, 0..1);
            }

            // Draw hex-prism walls (US-012)
            if self.hex_wall_index_count > 0 {
                render_pass.set_vertex_buffer(0, hex_wall_vb.slice(..));
                render_pass.set_index_buffer(hex_wall_ib.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..self.hex_wall_index_count, 0, 0..1);
            }
            
            // Draw building blocks (new system)
            if let (Some(block_vb), Some(block_ib)) = (&self.block_vertex_buffer, &self.block_index_buffer) {
                if self.block_index_count > 0 {
                    render_pass.set_vertex_buffer(0, block_vb.slice(..));
                    render_pass.set_index_buffer(block_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.block_index_count, 0, 0..1);
                }
            }
            
            // Draw block placement preview (Fallout 4 / Fortnite style ghost)
            if self.build_toolbar.visible && self.build_toolbar.show_preview && !self.build_toolbar.is_bridge_mode() {
                if let Some(preview_pos) = self.build_toolbar.preview_position {
                    // Generate preview mesh
                    let shape = self.build_toolbar.get_selected_shape();
                    let preview_block = BuildingBlock::new(shape, preview_pos, 0);
                    let (preview_verts, preview_indices) = preview_block.generate_mesh();
                    
                    // Pulsing effect based on time
                    let pulse = (self.start_time.elapsed().as_secs_f32() * 3.0).sin() * 0.5 + 0.5;
                    
                    // Bright green/yellow for valid placement (Fortnite style)
                    let r = 0.2 + pulse * 0.3;
                    let g = 0.9;
                    let b = 0.2 + pulse * 0.2;
                    let highlight_color = [r, g, b, 0.85];
                    
                    let preview_vertices: Vec<Vertex> = preview_verts.iter().map(|v| Vertex {
                        position: v.position,
                        normal: v.normal,
                        color: highlight_color,
                    }).collect();
                    
                    if !preview_vertices.is_empty() && !preview_indices.is_empty() {
                        let preview_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Preview Vertex Buffer"),
                            contents: bytemuck::cast_slice(&preview_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                        let preview_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Preview Index Buffer"),
                            contents: bytemuck::cast_slice(&preview_indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                        
                        render_pass.set_vertex_buffer(0, preview_vb.slice(..));
                        render_pass.set_index_buffer(preview_ib.slice(..), wgpu::IndexFormat::Uint32);
                        render_pass.draw_indexed(0..preview_indices.len() as u32, 0, 0..1);
                    }
                    
                    // Draw wireframe outline (slightly larger, bright white edges)
                    let outline_scale = 1.05;
                    let outline_color = [1.0, 1.0, 1.0, 0.9];
                    let outline_vertices: Vec<Vertex> = preview_verts.iter().map(|v| {
                        // Scale position slightly outward from center
                        let dir = Vec3::from(v.position) - preview_pos;
                        let scaled_pos = preview_pos + dir * outline_scale;
                        Vertex {
                            position: scaled_pos.to_array(),
                            normal: v.normal,
                            color: outline_color,
                        }
                    }).collect();
                    
                    // Generate wireframe indices (edges only)
                    let mut wireframe_indices: Vec<u32> = Vec::new();
                    for chunk in preview_indices.chunks(3) {
                        if chunk.len() == 3 {
                            // Triangle edges
                            wireframe_indices.push(chunk[0]);
                            wireframe_indices.push(chunk[1]);
                            wireframe_indices.push(chunk[1]);
                            wireframe_indices.push(chunk[2]);
                            wireframe_indices.push(chunk[2]);
                            wireframe_indices.push(chunk[0]);
                        }
                    }
                    
                    if !outline_vertices.is_empty() {
                        let outline_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Outline Vertex Buffer"),
                            contents: bytemuck::cast_slice(&outline_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                        
                        // For wireframe, we still use the triangle indices but render as lines
                        // Note: wgpu doesn't have LINE mode in the main pipeline, so we render as triangles
                        // but with the outline effect from scaling
                        render_pass.set_vertex_buffer(0, outline_vb.slice(..));
                        render_pass.draw_indexed(0..preview_indices.len() as u32, 0, 0..1);
                    }
                }
            }
            
            // Draw merged meshes (baked from SDF)
            for merged in &self.merged_mesh_buffers {
                if merged.index_count > 0 {
                    render_pass.set_vertex_buffer(0, merged.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(merged.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..merged.index_count, 0, 0..1);
                }
            }
            
            // Draw trees
            if let (Some(tree_vb), Some(tree_ib)) = (&self.tree_vertex_buffer, &self.tree_index_buffer) {
                if self.tree_index_count > 0 {
                    render_pass.set_vertex_buffer(0, tree_vb.slice(..));
                    render_pass.set_index_buffer(tree_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.tree_index_count, 0, 0..1);
                }
            }
        }

        // ============================================
        // PASS 2: SDF Cannon rendering (US-013)
        // ============================================
        if let (Some(sdf_pipeline), Some(sdf_bind_group)) =
            (&self.sdf_cannon_pipeline, &self.sdf_cannon_bind_group)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SDF Cannon Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve previous pass
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Use existing depth
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(sdf_pipeline);
            render_pass.set_bind_group(0, sdf_bind_group, &[]);
            // Fullscreen triangle - 3 vertices, no vertex buffer needed
            render_pass.draw(0..3, 0..1);
        }

        // ============================================
        // PASS 2.5: Ember Particles (additive blend)
        // ============================================
        // Particles render after opaque geometry but before UI
        // Uses additive blending for glowing effect
        if let Some(ref particle_system) = self.particle_system {
            // Upload particle data to GPU
            particle_system.upload_particles(queue);

            // Update particle uniforms with current camera matrices
            particle_system.update_uniforms(queue, view_mat.to_cols_array_2d(), proj_mat.to_cols_array_2d());

            // Render particles
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Particle Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve previous passes
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Use existing depth (particles read but don't write)
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            particle_system.render(&mut render_pass);
        }

        // ============================================
        // PASS 3: UI Overlay (terrain editor sliders)
        // ============================================
        if self.terrain_ui.visible {
            if let (Some(ui_pipeline), Some(ui_bg)) = (&self.ui_pipeline, &self.ui_bind_group) {
                let config = self.surface_config.as_ref().unwrap();
                let ui_mesh = self.terrain_ui.generate_ui_mesh(
                    config.width as f32,
                    config.height as f32
                );
                
                if !ui_mesh.vertices.is_empty() {
                    // Create temporary UI vertex/index buffers
                    let ui_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("UI Vertex Buffer"),
                        contents: bytemuck::cast_slice(&ui_mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let ui_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("UI Index Buffer"),
                        contents: bytemuck::cast_slice(&ui_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    
                    // Render UI with depth testing disabled (render on top)
                    // Uses separate UI bind group with identity matrix (no 3D transform)
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("UI Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load, // Preserve previous passes
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None, // No depth testing for UI
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    
                    render_pass.set_pipeline(ui_pipeline);
                    render_pass.set_bind_group(0, ui_bg, &[]); // Use UI bind group with identity matrix
                    render_pass.set_vertex_buffer(0, ui_vertex_buffer.slice(..));
                    render_pass.set_index_buffer(ui_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..ui_mesh.indices.len() as u32, 0, 0..1);
                }
            }
        }
        
        // ============================================
        // PASS 4: Selection Cursor (when in build mode)
        // ============================================
        if self.build_toolbar.visible && !self.start_overlay.visible {
            if let (Some(ui_pipeline), Some(ui_bg)) = (&self.ui_pipeline, &self.ui_bind_group) {
                let config = self.surface_config.as_ref().unwrap();
                let w = config.width as f32;
                let h = config.height as f32;
                
                // Use mouse position for cursor, fallback to center
                let (cx, cy) = self.current_mouse_pos.unwrap_or((w / 2.0, h / 2.0));
                
                // Selection cursor dimensions (larger for visibility)
                let size = 25.0;
                let thickness = 4.0;
                let gap = 8.0;
                
                // Pulsing bright green/yellow color
                let pulse = (self.start_time.elapsed().as_secs_f32() * 3.0).sin() * 0.3 + 0.7;
                let crosshair_color = [pulse, 1.0, 0.2, 0.95];
                
                let to_ndc = |x: f32, y: f32| -> [f32; 3] {
                    [(x / w) * 2.0 - 1.0, 1.0 - (y / h) * 2.0, 0.0]
                };
                
                let mut cross_verts = Vec::new();
                let mut cross_indices = Vec::new();
                
                // Helper to add a quad
                let add_cross_quad = |verts: &mut Vec<Vertex>, indices: &mut Vec<u32>, 
                    x1: f32, y1: f32, x2: f32, y2: f32, color: [f32; 4]| {
                    let base = verts.len() as u32;
                    let normal = [0.0, 0.0, 1.0];
                    verts.push(Vertex { position: to_ndc(x1, y1), normal, color });
                    verts.push(Vertex { position: to_ndc(x2, y1), normal, color });
                    verts.push(Vertex { position: to_ndc(x2, y2), normal, color });
                    verts.push(Vertex { position: to_ndc(x1, y2), normal, color });
                    indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
                };
                
                // Horizontal line (left part)
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx - size - gap, cy - thickness/2.0,
                    cx - gap, cy + thickness/2.0, crosshair_color);
                // Horizontal line (right part)
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx + gap, cy - thickness/2.0,
                    cx + size + gap, cy + thickness/2.0, crosshair_color);
                // Vertical line (top part)
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx - thickness/2.0, cy - size - gap,
                    cx + thickness/2.0, cy - gap, crosshair_color);
                // Vertical line (bottom part)
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx - thickness/2.0, cy + gap,
                    cx + thickness/2.0, cy + size + gap, crosshair_color);
                // Center dot
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx - 2.0, cy - 2.0, cx + 2.0, cy + 2.0, [1.0, 1.0, 1.0, 1.0]);
                
                if !cross_verts.is_empty() {
                    let cross_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Crosshair VB"),
                        contents: bytemuck::cast_slice(&cross_verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let cross_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Crosshair IB"),
                        contents: bytemuck::cast_slice(&cross_indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Crosshair Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store, },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    
                    render_pass.set_pipeline(ui_pipeline);
                    render_pass.set_bind_group(0, ui_bg, &[]);
                    render_pass.set_vertex_buffer(0, cross_vb.slice(..));
                    render_pass.set_index_buffer(cross_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..cross_indices.len() as u32, 0, 0..1);
                }
            }
        }
        
        // ============================================
        // PASS 5: Build Toolbar UI
        // ============================================
        if self.build_toolbar.visible {
            if let (Some(ui_pipeline), Some(ui_bg)) = (&self.ui_pipeline, &self.ui_bind_group) {
                let config = self.surface_config.as_ref().unwrap();
                let toolbar_mesh = self.build_toolbar.generate_ui_mesh(
                    config.width as f32,
                    config.height as f32
                );
                
                if !toolbar_mesh.vertices.is_empty() {
                    let toolbar_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Toolbar Vertex Buffer"),
                        contents: bytemuck::cast_slice(&toolbar_mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let toolbar_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Toolbar Index Buffer"),
                        contents: bytemuck::cast_slice(&toolbar_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Build Toolbar Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    
                    render_pass.set_pipeline(ui_pipeline);
                    render_pass.set_bind_group(0, ui_bg, &[]);
                    render_pass.set_vertex_buffer(0, toolbar_vb.slice(..));
                    render_pass.set_index_buffer(toolbar_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..toolbar_mesh.indices.len() as u32, 0, 0..1);
                }
            }
        }

        // ============================================
        // PASS 6: Top Bar (resources, day, population)
        // ============================================
        if self.game_state.top_bar.visible && !self.start_overlay.visible {
            if let (Some(ui_pipeline), Some(ui_bg)) = (&self.ui_pipeline, &self.ui_bind_group) {
                let config = self.surface_config.as_ref().unwrap();
                let (resources, day_cycle, population) = self.game_state.ui_data();
                let top_bar_mesh = self.game_state.top_bar.generate_ui_mesh(
                    config.width as f32,
                    config.height as f32,
                    resources,
                    day_cycle,
                    population,
                );

                if !top_bar_mesh.vertices.is_empty() {
                    let top_bar_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Top Bar Vertex Buffer"),
                        contents: bytemuck::cast_slice(&top_bar_mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let top_bar_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Top Bar Index Buffer"),
                        contents: bytemuck::cast_slice(&top_bar_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });

                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Top Bar Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });

                    render_pass.set_pipeline(ui_pipeline);
                    render_pass.set_bind_group(0, ui_bg, &[]);
                    render_pass.set_vertex_buffer(0, top_bar_vb.slice(..));
                    render_pass.set_index_buffer(top_bar_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..top_bar_mesh.indices.len() as u32, 0, 0..1);
                }
            }
        }

        // ============================================
        // PASS 7: Start Overlay (Windows focus)
        // ============================================
        if self.start_overlay.visible {
            if let (Some(ui_pipeline), Some(ui_bg)) = (&self.ui_pipeline, &self.ui_bind_group) {
                let config = self.surface_config.as_ref().unwrap();
                let overlay_mesh = self.start_overlay.generate_ui_mesh(
                    config.width as f32,
                    config.height as f32
                );
                
                if !overlay_mesh.vertices.is_empty() {
                    let overlay_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Overlay Vertex Buffer"),
                        contents: bytemuck::cast_slice(&overlay_mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let overlay_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Overlay Index Buffer"),
                        contents: bytemuck::cast_slice(&overlay_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Start Overlay Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    
                    render_pass.set_pipeline(ui_pipeline);
                    render_pass.set_bind_group(0, ui_bg, &[]);
                    render_pass.set_vertex_buffer(0, overlay_vb.slice(..));
                    render_pass.set_index_buffer(overlay_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..overlay_mesh.indices.len() as u32, 0, 0..1);
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn handle_key(&mut self, key: KeyCode, pressed: bool) {
        match key {
            // Movement
            KeyCode::KeyW => self.movement.forward = pressed,
            KeyCode::KeyS => self.movement.backward = pressed,
            KeyCode::KeyA => self.movement.left = pressed,
            KeyCode::KeyD => self.movement.right = pressed,
            KeyCode::Space => {
                if pressed {
                    if self.first_person_mode {
                        // Jump in first-person mode
                        self.player.request_jump();
                    } else if !self.builder_mode.enabled {
                        // Fire projectile in free camera mode
                        self.fire_projectile();
                    }
                }
                self.movement.up = pressed;
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.movement.sprint = pressed;
                self.movement.down = pressed;
            }
            
            // Ctrl key for copy/paste/undo
            KeyCode::ControlLeft | KeyCode::ControlRight => {
                self.builder_mode.ctrl_held = pressed;
            }
            
            // Builder mode toggle (B key)
            KeyCode::KeyB if pressed => {
                self.builder_mode.toggle();
                self.build_toolbar.toggle();
                
                // In build mode: show cursor for selection
                // Out of build mode: hide cursor for FPS look
                if let Some(window) = &self.window {
                    if self.build_toolbar.visible {
                        // Show cursor for selection
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                        println!("[Build Mode] Cursor visible - click to place blocks");
                    } else {
                        // Hide cursor for FPS mode
                        if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                        }
                        window.set_cursor_visible(false);
                    }
                }
            }
            
            // Tab - cycle through shapes in build toolbar
            KeyCode::Tab if pressed => {
                if self.build_toolbar.visible {
                    self.build_toolbar.next_shape();
                }
            }
            
            // Number keys 1-6 for direct shape selection
            KeyCode::Digit1 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(0);
            }
            KeyCode::Digit2 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(1);
            }
            KeyCode::Digit3 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(2);
            }
            KeyCode::Digit4 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(3);
            }
            KeyCode::Digit5 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(4);
            }
            KeyCode::Digit6 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(5);
            }
            KeyCode::Digit7 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(6); // Bridge tool
            }
            
            // Arrow keys for shape selection in build mode
            KeyCode::ArrowUp if pressed && self.build_toolbar.visible => {
                self.build_toolbar.prev_shape();
            }
            KeyCode::ArrowDown if pressed && self.build_toolbar.visible => {
                self.build_toolbar.next_shape();
            }
            
            // F11 - Fullscreen toggle
            KeyCode::F11 if pressed => {
                if let Some(window) = &self.window {
                    let current = window.fullscreen();
                    window.set_fullscreen(if current.is_some() {
                        None // Back to windowed
                    } else {
                        Some(Fullscreen::Borderless(None)) // Borderless fullscreen
                    });
                    println!("Fullscreen: {}", current.is_none());
                }
            }
            
            // Terrain editor toggle (T key) - shows on-screen UI sliders
            KeyCode::KeyT if pressed => {
                self.terrain_ui.toggle();
                if self.terrain_ui.visible {
                    println!("=== TERRAIN EDITOR UI OPEN ===");
                    println!("Click and drag sliders to adjust terrain");
                    println!("Click APPLY button to rebuild terrain");
                } else {
                    println!("Terrain editor closed");
                }
            }
            
            // Toggle first-person mode (V key)
            KeyCode::KeyV if pressed => {
                self.first_person_mode = !self.first_person_mode;
                if self.first_person_mode {
                    // Sync player position to current camera when entering FPS mode
                    self.player.position = self.camera.position - Vec3::new(0.0, PLAYER_EYE_HEIGHT, 0.0);
                    println!("=== FIRST-PERSON MODE ===");
                    println!("WASD: Move, Mouse: Look, Space: Jump, Shift: Sprint");
                } else {
                    println!("=== FREE CAMERA MODE ===");
                    println!("WASD: Move, QE: Up/Down, Mouse: Look, Space: Fire");
                }
            }
            
            // Terrain presets (F1-F4) work anytime
            KeyCode::F1 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 0.1, mountains: 0.0, rocks: 0.0, hills: 0.2, detail: 0.1, water: 0.0,
                });
                self.terrain_needs_rebuild = true;
                println!("PRESET: FLAT - Perfect for building!");
            }
            KeyCode::F2 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 0.3, mountains: 0.1, rocks: 0.1, hills: 0.4, detail: 0.2, water: 0.0,
                });
                self.terrain_needs_rebuild = true;
                println!("PRESET: GENTLE HILLS");
            }
            KeyCode::F3 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 0.5, mountains: 0.3, rocks: 0.5, hills: 0.3, detail: 0.4, water: 0.2,
                });
                self.terrain_needs_rebuild = true;
                println!("PRESET: ROCKY TERRAIN");
            }
            KeyCode::F4 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 1.0, mountains: 0.8, rocks: 0.6, hills: 0.3, detail: 0.5, water: 0.3,
                });
                self.terrain_needs_rebuild = true;
                println!("PRESET: MOUNTAINS");
            }
            
            // Material selection (1-8 keys) - only when builder mode active AND terrain UI NOT visible
            KeyCode::Digit1 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(0);
            }
            KeyCode::Digit2 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(1);
            }
            KeyCode::Digit3 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(2);
            }
            KeyCode::Digit4 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(3);
            }
            KeyCode::Digit5 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(4);
            }
            KeyCode::Digit6 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(5);
            }
            KeyCode::Digit7 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(6);
            }
            KeyCode::Digit8 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(7);
            }
            
            // Undo/Redo (Ctrl+Z / Ctrl+Y)
            KeyCode::KeyZ if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled => {
                self.builder_mode.undo(&mut self.hex_wall_grid);
            }
            KeyCode::KeyY if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled => {
                self.builder_mode.redo(&mut self.hex_wall_grid);
            }
            
            // Copy/Paste (Ctrl+C / Ctrl+V)
            KeyCode::KeyC if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled => {
                self.builder_mode.copy_area(&self.hex_wall_grid, 3);
            }
            KeyCode::KeyV if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled => {
                self.builder_mode.paste(&mut self.hex_wall_grid);
            }
            
            // Rotate paste selection
            KeyCode::KeyR if pressed && self.builder_mode.enabled => {
                self.builder_mode.rotate_selection();
            }
            
            // Camera/cannon controls (when NOT in builder mode or Ctrl not held)
            KeyCode::KeyR if pressed && !self.builder_mode.enabled => self.camera.reset(),
            KeyCode::KeyC if pressed && !self.builder_mode.ctrl_held => {
                self.projectiles.clear();
                println!("Cleared all projectiles");
            }
            
            // Arrow keys: continuous aiming with smooth movement (US-017)
            KeyCode::ArrowUp => {
                self.aiming.aim_up = pressed;
                if pressed && !self.cannon.is_aiming() {
                    println!("Aiming UP (hold for continuous movement)");
                }
            }
            KeyCode::ArrowDown => {
                self.aiming.aim_down = pressed;
                if pressed && !self.cannon.is_aiming() {
                    println!("Aiming DOWN (hold for continuous movement)");
                }
            }
            KeyCode::ArrowLeft => {
                self.aiming.aim_left = pressed;
                if pressed && !self.cannon.is_aiming() {
                    println!("Aiming LEFT (hold for continuous movement)");
                }
            }
            KeyCode::ArrowRight => {
                self.aiming.aim_right = pressed;
                if pressed && !self.cannon.is_aiming() {
                    println!("Aiming RIGHT (hold for continuous movement)");
                }
            }
            _ => {}
        }
    }
}

impl ApplicationHandler for BattleArenaApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title("Battle Sphere - Combat Arena [T: Terrain Editor]")
                .with_inner_size(PhysicalSize::new(1920, 1080)); // Full HD
            let window = Arc::new(event_loop.create_window(attrs).unwrap());
            self.initialize(window);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if key == KeyCode::Escape && event.state == ElementState::Pressed {
                        event_loop.exit();
                        return;
                    }
                    self.handle_key(key, event.state == ElementState::Pressed);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let mouse_pos = self.current_mouse_pos.unwrap_or((0.0, 0.0));
                
                // Handle start overlay - dismiss on any click
                if self.start_overlay.visible && state == ElementState::Pressed {
                    self.start_overlay.visible = false;
                    // Grab cursor on Windows
                    if let Some(window) = &self.window {
                        if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                        }
                        window.set_cursor_visible(false);
                    }
                    println!("=== GAME STARTED ===");
                    return; // Consume this click
                }
                
                match button {
                    MouseButton::Left => {
                        let pressed = state == ElementState::Pressed;
                        self.left_mouse_pressed = pressed;
                        
                        // Check if terrain UI wants the event
                        if pressed {
                            if self.terrain_ui.on_mouse_press(mouse_pos.0, mouse_pos.1) {
                                // UI consumed the event
                                return;
                            }
                        } else {
                            // Mouse release
                            if self.terrain_ui.on_mouse_release(mouse_pos.0, mouse_pos.1) {
                                // Rebuild terrain
                                self.terrain_needs_rebuild = true;
                            }
                        }
                        
                        // New building block system
                        if self.build_toolbar.visible && pressed {
                            // Bridge mode: select faces
                            if self.build_toolbar.is_bridge_mode() {
                                self.handle_bridge_click();
                            }
                            // Normal mode: First check for double-click (merge), then place
                            else if !self.handle_block_click() {
                                self.place_building_block();
                            }
                        }
                        // Legacy hex prism builder mode
                        else if self.builder_mode.enabled && pressed {
                            if self.builder_mode.place_at_cursor(&mut self.hex_wall_grid) {
                                // Mesh needs regeneration
                            }
                        }
                    }
                    MouseButton::Right => {
                        // Right-click removes blocks in builder mode
                        if self.builder_mode.enabled && state == ElementState::Pressed {
                            if self.builder_mode.remove_at_cursor(&mut self.hex_wall_grid) {
                                // Mesh needs regeneration
                            }
                        }
                        // Track right mouse state (for legacy compatibility)
                        self.mouse_pressed = state == ElementState::Pressed;
                    }
                    MouseButton::Middle => {
                        if state == ElementState::Pressed {
                            if self.build_toolbar.visible {
                                // Middle-click in build mode: cycle material
                                self.build_toolbar.next_material();
                            }
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let current = (position.x as f32, position.y as f32);
                
                // Always track current mouse position for builder raycast
                self.current_mouse_pos = Some(current);
                
                // Update terrain UI slider dragging
                if self.left_mouse_pressed && self.terrain_ui.visible {
                    self.terrain_ui.on_mouse_move(current.0, current.1);
                }
                
                // Note: Camera look is now handled by DeviceEvent::MouseMotion 
                // for proper cross-platform support (especially Windows)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                };
                
                // Build toolbar: scroll adjusts height
                if self.build_toolbar.visible {
                    self.build_toolbar.adjust_height(scroll);
                    // Update preview immediately
                    self.update_block_preview();
                }
                // Builder mode (hex prisms): scroll adjusts build height level
                else if self.builder_mode.enabled {
                    let delta = if scroll > 0.0 { 1 } else { -1 };
                    self.builder_mode.adjust_level(delta);
                } else {
                    // Normal mode: scroll zooms camera
                    self.camera.position += self.camera.get_forward() * scroll * 5.0;
                }
            }
            WindowEvent::Resized(new_size) => {
                if let (Some(surface), Some(config), Some(device)) =
                    (&self.surface, &mut self.surface_config, &self.device)
                {
                    config.width = new_size.width.max(1);
                    config.height = new_size.height.max(1);
                    surface.configure(device, config);

                    // Recreate depth texture for new size (US-013)
                    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Depth Texture"),
                        size: wgpu::Extent3d {
                            width: config.width,
                            height: config.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Depth32Float,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    });
                    self.depth_texture = Some(depth_texture.create_view(&wgpu::TextureViewDescriptor::default()));
                    self.depth_texture_raw = Some(depth_texture);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let delta_time = now.duration_since(self.last_frame).as_secs_f32();
                self.last_frame = now;

                // Update FPS counter
                self.frame_count += 1;
                if now.duration_since(self.last_fps_update).as_secs_f32() >= 1.0 {
                    self.fps = self.frame_count as f32
                        / now.duration_since(self.last_fps_update).as_secs_f32();
                    self.frame_count = 0;
                    self.last_fps_update = now;

                    if let Some(window) = &self.window {
                        let mode_str = if self.builder_mode.enabled {
                            format!("BUILDER (Mat: {})", self.builder_mode.selected_material + 1)
                        } else {
                            "Combat".to_string()
                        };
                        window.set_title(&format!(
                            "Battle Sphere - {} | FPS: {:.0} | Prisms: {}",
                            mode_str,
                            self.fps,
                            self.hex_wall_grid.len()
                        ));
                    }
                }

                self.update(delta_time);
                self.render();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    // Handle raw device events - crucial for Windows mouse look
    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        // Skip if start overlay is visible (not yet focused)
        if self.start_overlay.visible {
            return;
        }
        
        if let DeviceEvent::MouseMotion { delta } = event {
            // In build mode: mouse moves cursor for selection (camera doesn't rotate)
            // Camera can still rotate with right-click held
            if self.build_toolbar.visible {
                // Only rotate camera if right mouse is held
                if self.mouse_pressed {
                    self.camera.handle_mouse_look(delta.0 as f32, delta.1 as f32);
                }
            } else {
                // Normal mode: free camera look
                self.camera.handle_mouse_look(delta.0 as f32, delta.1 as f32);
            }
        }
    }
}

// Buffer init helper
use wgpu::util::DeviceExt;

fn main() {
    println!("===========================================");
    println!("   Battle Sphere - Combat Arena");
    println!("===========================================");
    println!();
    println!("*** Click anywhere to start ***");
    println!();
    println!("General Controls:");
    println!("  Mouse       - Look around (free look)");
    println!("  WASD        - Move");
    println!("  SPACE       - Jump (FPS) / Fire (Free)");
    println!("  V           - Toggle First-Person / Free Camera");
    println!("  F11         - Toggle Fullscreen");
    println!("  ESC         - Exit");
    println!();
    println!("Build Toolbar (press B to toggle):");
    println!("  Tab/Up/Down - Cycle through shapes");
    println!("  1-7         - Select shape (7=Bridge tool)");
    println!("  Scroll      - Adjust height level");
    println!("  Middle-click- Change material");
    println!("  Left-click  - Place block / Select face (Bridge)");
    println!("  Double-click- Merge connected blocks into one");
    println!();
    println!("  Bridge Tool: Click 2 faces to connect them!");
    println!("  Physics: Unsupported blocks fall every 5 sec");
    println!();
    println!("Combat Controls:");
    println!("  Arrow Keys  - Aim cannon");
    println!("  C           - Clear projectiles");
    println!("  R           - Reset camera");
    println!();
    println!("Legacy Builder (hex prisms):");
    println!("  Left-click  - Place prism");
    println!("  Right-click - Remove prism");
    println!("  Scroll      - Adjust build height");
    println!("  Ctrl+Z/Y    - Undo/Redo");
    println!();
    println!("Terrain Editor (press T):");
    println!("  F1 = FLAT | F2 = HILLS | F3 = ROCKY | F4 = MOUNTAINS");
    println!();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = BattleArenaApp::new();
    event_loop.run_app(&mut app).unwrap();
}
