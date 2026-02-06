//! Battle Arena - Combat Prototype
//!
//! Run with: `cargo run --bin battle_arena`
//!
//! Controls:
//! - WASD: Move (first-person or camera)
//! - Mouse right-drag: Look around (FPS style)
//! - Space: Jump (first-person mode) / Fire cannon (free camera)
//! - F: Fire cannon (aims where you look)
//! - G: Grab/release cannon (walk to reposition)
//! - Shift: Sprint when moving
//! - V: Toggle first-person / free camera mode
//! - R: Reset camera
//! - C: Clear all projectiles
//! - B: Toggle builder mode
//! - T: Terrain editor UI
//! - ESC: Exit

use std::sync::Arc;
use std::time::Instant;

use battle_tok_engine::render::hex_prism::DEFAULT_HEX_RADIUS;
use glam::{Mat4, Vec3};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Fullscreen, Window, WindowAttributes, WindowId};

// Import skybox
use battle_tok_engine::render::CubemapSkybox;

// Import Phase 2 visual upgrade systems
use battle_tok_engine::render::{
    FogPostConfig, FogPostPass, LavaSteamConfig, MaterialSystem, ParticleSystem,
    PointLightManager, SceneConfig,
};

// Import building block types for GPU operations
use battle_tok_engine::render::{BuildingBlock, MergedMesh};

// Import game module types
use battle_tok_engine::game::config::{ArenaConfig, VisualConfig};
use battle_tok_engine::game::{
    BattleScene, BridgeConfig, BuilderMode, Camera, FloatingIslandConfig, LavaParams,
    Mesh, MovementKeys, PLAYER_EYE_HEIGHT, SHADER_SOURCE, SdfCannonData, SdfCannonUniforms,
    SelectedFace, StartOverlay, TerrainEditorUI, TerrainParams, Uniforms, Vertex,
    generate_all_trees_mesh, generate_bridge, generate_floating_island, generate_lava_ocean,
    generate_trees_on_terrain, get_material_color, set_terrain_params, terrain_height_at,
};
use battle_tok_engine::render::hex_prism::DEFAULT_HEX_HEIGHT;

use battle_tok_engine::game::MovementState;

// Buffer init helper
use wgpu::util::DeviceExt;

// GPU-specific types (need wgpu::Buffer, can't be in module)
/// GPU buffers for a merged mesh (baked from SDF)
struct MergedMeshBuffers {
    _id: u32,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

/// Scene uniforms for the lava shader (matches lava.wgsl Uniforms struct).
/// Subset of the main Uniforms — only what the lava shader needs.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LavaSceneUniforms {
    view_proj: [[f32; 4]; 4], // 64 bytes
    camera_pos: [f32; 3],     // 12 bytes
    time: f32,                // 4 bytes  (total 80)
    sun_dir: [f32; 3],        // 12 bytes
    fog_density: f32,         // 4 bytes  (total 96)
    fog_color: [f32; 3],      // 12 bytes
    ambient: f32,             // 4 bytes  (total 112)
}

// ============================================================================
// GPU RESOURCES
// ============================================================================

/// All GPU-related state grouped together.
/// Constructed during `initialize()` and wrapped in `Option<GpuResources>` on the app.
#[allow(dead_code)]
struct GpuResources {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // Pipelines
    pipeline: wgpu::RenderPipeline,
    sdf_cannon_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,

    // Main uniform
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    // Static mesh (terrain)
    static_vertex_buffer: wgpu::Buffer,
    static_index_buffer: wgpu::Buffer,
    static_index_count: u32,

    // Dynamic mesh (projectiles, debris)
    dynamic_vertex_buffer: wgpu::Buffer,
    dynamic_index_buffer: wgpu::Buffer,

    // Hex walls
    hex_wall_vertex_buffer: wgpu::Buffer,
    hex_wall_index_buffer: wgpu::Buffer,
    hex_wall_index_count: u32,

    // SDF cannon
    sdf_cannon_uniform_buffer: wgpu::Buffer,
    sdf_cannon_data_buffer: wgpu::Buffer,
    sdf_cannon_bind_group: wgpu::BindGroup,

    // Depth
    depth_texture: wgpu::TextureView,
    depth_texture_raw: wgpu::Texture,

    // UI
    ui_uniform_buffer: wgpu::Buffer,
    ui_bind_group: wgpu::BindGroup,

    // Building blocks
    block_vertex_buffer: Option<wgpu::Buffer>,
    block_index_buffer: Option<wgpu::Buffer>,
    block_index_count: u32,

    // Merged mesh GPU buffers
    merged_mesh_buffers: Vec<MergedMeshBuffers>,

    // Trees
    tree_vertex_buffer: wgpu::Buffer,
    tree_index_buffer: wgpu::Buffer,
    tree_index_count: u32,

    // Offscreen scene color texture (for fog post-process)
    scene_color_texture: wgpu::Texture,
    scene_color_view: wgpu::TextureView,

    // Lava ocean (rendered with animated lava.wgsl shader)
    lava_pipeline: wgpu::RenderPipeline,
    lava_bind_group: wgpu::BindGroup,
    lava_scene_uniform_buffer: wgpu::Buffer,
    lava_params_buffer: wgpu::Buffer,
    lava_vertex_buffer: wgpu::Buffer,
    lava_index_buffer: wgpu::Buffer,
    lava_index_count: u32,
}

impl GpuResources {
    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.surface_config.width = new_size.width.max(1);
        self.surface_config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        let depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: self.surface_config.width,
                height: self.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.depth_texture = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.depth_texture_raw = depth_texture;

        // Recreate offscreen scene color texture for fog post-process
        let scene_color_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scene Color Texture"),
            size: wgpu::Extent3d {
                width: self.surface_config.width,
                height: self.surface_config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.scene_color_view =
            scene_color_texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.scene_color_texture = scene_color_texture;
    }
}

// ============================================================================
// APPLICATION
// ============================================================================

struct BattleArenaApp {
    window: Option<Arc<Window>>,

    // Scene (holds ALL game state)
    scene: Option<BattleScene>,

    // GPU resources (replaces ~25 individual fields)
    gpu: Option<GpuResources>,

    // Cubemap skybox with day/night crossfade
    cubemap_skybox: Option<CubemapSkybox>,

    // Phase 2 Visual Systems
    point_lights: Option<PointLightManager>,
    particle_system: Option<ParticleSystem>,
    material_system: Option<MaterialSystem>,
    fog_post: Option<FogPostPass>,

    // Camera and input (stays here — winit types)
    camera: Camera,
    movement: MovementKeys,
    mouse_pressed: bool,
    left_mouse_pressed: bool,
    _last_mouse_pos: Option<(f32, f32)>,
    current_mouse_pos: Option<(f32, f32)>,

    // Builder mode (stays here — mixes with winit cursor control)
    builder_mode: BuilderMode,

    // Windows focus overlay
    start_overlay: StartOverlay,

    // Terrain editor UI (on-screen sliders)
    terrain_ui: TerrainEditorUI,

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
            scene: None,
            gpu: None,
            cubemap_skybox: None,
            point_lights: None,
            particle_system: None,
            material_system: None,
            fog_post: None,
            camera: Camera::default(),
            movement: MovementKeys::default(),
            mouse_pressed: false,
            left_mouse_pressed: false,
            _last_mouse_pos: None,
            current_mouse_pos: None,
            builder_mode: BuilderMode::default(),
            start_overlay: StartOverlay::default(),
            terrain_ui: TerrainEditorUI::default(),
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

        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Battle Arena Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        }))
        .expect("Failed to create device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let present_mode = if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Immediate)
        {
            wgpu::PresentMode::Immediate
        } else if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Mailbox)
        {
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

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

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

        // UI PIPELINE (no depth testing)
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
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let ui_uniforms = Uniforms {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            time: 0.0,
            sun_dir: [0.0, 1.0, 0.0],
            fog_density: 0.0,
            fog_color: [0.0, 0.0, 0.0],
            ambient: 1.0,
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

        let ui_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ui_uniform_buffer.as_entire_binding(),
            }],
        });

        // SDF CANNON RENDERING SETUP (US-013)
        let sdf_cannon_shader_source = std::fs::read_to_string("shaders/sdf_cannon.wgsl")
            .expect("Failed to load sdf_cannon.wgsl shader");
        let sdf_cannon_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SDF Cannon Shader"),
            source: wgpu::ShaderSource::Wgsl(sdf_cannon_shader_source.into()),
        });

        let sdf_cannon_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SDF Cannon Uniform Buffer"),
            size: std::mem::size_of::<SdfCannonUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sdf_cannon_data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SDF Cannon Data Buffer"),
            size: std::mem::size_of::<SdfCannonData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sdf_cannon_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let sdf_cannon_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("SDF Cannon Pipeline Layout"),
                bind_group_layouts: &[&sdf_cannon_bind_group_layout],
                push_constant_ranges: &[],
            });

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

        // Offscreen scene color texture (for fog post-process to read from)
        let scene_color_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scene Color Texture"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let scene_color_view =
            scene_color_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sdf_cannon_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SDF Cannon Pipeline"),
            layout: Some(&sdf_cannon_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sdf_cannon_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
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
                cull_mode: None,
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

        // ============================================
        // LAVA OCEAN PIPELINE (animated lava.wgsl shader)
        // ============================================
        let lava_shader_source = std::fs::read_to_string("shaders/lava.wgsl")
            .expect("Failed to load lava.wgsl shader");
        let lava_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Lava Ocean Shader"),
            source: wgpu::ShaderSource::Wgsl(lava_shader_source.into()),
        });

        let lava_scene_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lava Scene Uniform Buffer"),
            size: std::mem::size_of::<LavaSceneUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lava_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Lava Params Buffer"),
            size: std::mem::size_of::<LavaParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let lava_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Lava Bind Group Layout"),
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

        let lava_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Lava Bind Group"),
            layout: &lava_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: lava_scene_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lava_params_buffer.as_entire_binding(),
                },
            ],
        });

        let lava_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Lava Pipeline Layout"),
                bind_group_layouts: &[&lava_bind_group_layout],
                push_constant_ranges: &[],
            });

        let lava_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Lava Ocean Pipeline"),
            layout: Some(&lava_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &lava_shader,
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
                module: &lava_shader,
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
                cull_mode: None, // No culling — lava visible from below too
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

        println!("[Lava] Animated lava ocean pipeline initialized");

        // ============================================
        // CREATE BATTLE SCENE (all game state)
        // ============================================
        let mut scene = BattleScene::new(ArenaConfig::default(), VisualConfig::default());

        // ============================================
        // Generate static mesh from scene's terrain config
        // ============================================
        let config = &scene.config;
        let attacker_center = config.island_attacker.position;
        let defender_center = config.island_defender.position;
        let island_radius = config.island_attacker.radius;
        // Lava at terrain level (slightly below island surface) — NOT deep underground
        let lava_ocean_level = -0.5;

        let mut static_mesh = Mesh::new();

        let island_config = FloatingIslandConfig {
            radius: island_radius,
            surface_height: config.island_attacker.surface_height,
            island_thickness: config.island_attacker.thickness,
            taper_amount: config.island_attacker.taper_amount,
            num_layers: 5,
            edge_noise: 3.0,
        };

        let attacker_platform = generate_floating_island(attacker_center, island_config);
        static_mesh.merge(&attacker_platform);

        let defender_platform = generate_floating_island(defender_center, island_config);
        static_mesh.merge(&defender_platform);

        // Lava ocean generated separately — rendered with animated lava shader
        let lava_ocean = generate_lava_ocean(config.lava_size, lava_ocean_level);

        println!(
            "[Floating Islands] Generated 2 islands + lava ocean at y={:.1} (terrain level)",
            lava_ocean_level,
        );

        // Generate bridge connecting the two floating islands
        let surface_min_y = attacker_center.y;

        let mut bridge_start = Vec3::new(0.0, surface_min_y, attacker_center.z - island_radius);
        let mut best_dist_start = f32::MAX;
        for v in &attacker_platform.vertices {
            let (vx, vy, vz) = (v.position[0], v.position[1], v.position[2]);
            if vy >= surface_min_y {
                let dist = vx.abs() + (vz - (attacker_center.z - island_radius)).abs() * 0.5;
                if dist < best_dist_start && vz < attacker_center.z {
                    best_dist_start = dist;
                    bridge_start = Vec3::new(vx, vy, vz);
                }
            }
        }

        let mut bridge_end = Vec3::new(0.0, surface_min_y, defender_center.z + island_radius);
        let mut best_dist_end = f32::MAX;
        for v in &defender_platform.vertices {
            let (vx, vy, vz) = (v.position[0], v.position[1], v.position[2]);
            if vy >= surface_min_y {
                let dist = vx.abs() + ((defender_center.z + island_radius) - vz).abs() * 0.5;
                if dist < best_dist_end && vz > defender_center.z {
                    best_dist_end = dist;
                    bridge_end = Vec3::new(vx, vy, vz);
                }
            }
        }

        println!(
            "[Bridge] Connecting floating islands from {:?} to {:?}",
            bridge_start, bridge_end
        );
        let bridge_config = BridgeConfig::default();
        let bridge_mesh = generate_bridge(bridge_start, bridge_end, &bridge_config);
        static_mesh.merge(&bridge_mesh);

        // Register bridge with scene for player ground collision
        scene.set_bridge(bridge_start, bridge_end);

        println!("[Floating Islands] Bridge chain connects the two floating battle platforms");

        // TREE MESH from scene data
        let mut all_trees = scene.trees_attacker.clone();
        all_trees.extend(scene.trees_defender.clone());
        let tree_mesh = generate_all_trees_mesh(&all_trees);

        let max_tree_vertices = 2000 * 50;
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
            queue.write_buffer(
                &tree_vertex_buffer,
                0,
                bytemuck::cast_slice(&tree_mesh.vertices),
            );
            queue.write_buffer(
                &tree_index_buffer,
                0,
                bytemuck::cast_slice(&tree_mesh.indices),
            );
        }

        let tree_index_count = tree_mesh.indices.len() as u32;
        println!(
            "[Trees] Generated {} trees ({} attacker, {} defender)",
            scene.trees_attacker.len() + scene.trees_defender.len(),
            scene.trees_attacker.len(),
            scene.trees_defender.len()
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

        // Lava ocean GPU buffers (separate from static for animated shader)
        let lava_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lava Vertex Buffer"),
            contents: bytemuck::cast_slice(&lava_ocean.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let lava_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lava Index Buffer"),
            contents: bytemuck::cast_slice(&lava_ocean.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let lava_index_count = lava_ocean.indices.len() as u32;
        println!(
            "[Lava] Ocean mesh: {} verts, {} indices",
            lava_ocean.vertices.len(),
            lava_ocean.indices.len()
        );

        // Dynamic buffers
        let dynamic_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Vertex Buffer"),
            size: 1024 * 1024,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dynamic_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Index Buffer"),
            size: 256 * 1024,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // HEX-PRISM WALLS
        let (hex_vertices, hex_indices) = scene.hex_grid.generate_combined_mesh();
        let hex_wall_vertices: Vec<Vertex> = hex_vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            })
            .collect();
        let hex_wall_index_count = hex_indices.len() as u32;

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

        if !hex_wall_vertices.is_empty() {
            queue.write_buffer(
                &hex_wall_vertex_buffer,
                0,
                bytemuck::cast_slice(&hex_wall_vertices),
            );
            queue.write_buffer(
                &hex_wall_index_buffer,
                0,
                bytemuck::cast_slice(&hex_indices),
            );
        }

        // Cubemap skybox with day/night crossfade
        let cubemap_skybox = CubemapSkybox::new(
            &device,
            &queue,
            surface_format,
            "Assets/Skybox/sky_26_2k/sky_26_cubemap_2k",  // day sky
            "Assets/Skybox/sky_16_2k/sky_16_cubemap_2k",  // night sky
        );

        // Phase 2 Visual Systems (torch params from VisualConfig)
        let mut point_lights = PointLightManager::new(&device);
        let torch_color = scene.visuals.torch_color.to_array();
        let torch_radius = scene.visuals.torch_radius;
        let torch_y = 3.0; // Above ground level (islands at Y=0)
        point_lights.add_torch([10.0, torch_y, 55.0], torch_color, torch_radius);
        point_lights.add_torch([-10.0, torch_y, 55.0], torch_color, torch_radius);
        point_lights.add_torch([10.0, torch_y, 35.0], torch_color, torch_radius);
        point_lights.add_torch([-10.0, torch_y, 35.0], torch_color, torch_radius);
        point_lights.add_torch([10.0, torch_y, -55.0], torch_color, torch_radius);
        point_lights.add_torch([-10.0, torch_y, -55.0], torch_color, torch_radius);
        point_lights.add_torch([10.0, torch_y, -35.0], torch_color, torch_radius);
        point_lights.add_torch([-10.0, torch_y, -35.0], torch_color, torch_radius);
        let bridge_torch_y = 2.0; // Bridge at ground level
        point_lights.add_torch([3.0, bridge_torch_y, 15.0], torch_color, 8.0);
        point_lights.add_torch([-3.0, bridge_torch_y, 15.0], torch_color, 8.0);
        point_lights.add_torch([3.0, bridge_torch_y, -15.0], torch_color, 8.0);
        point_lights.add_torch([-3.0, bridge_torch_y, -15.0], torch_color, 8.0);

        let mut particle_system = ParticleSystem::new(&device, surface_format);
        let ember_y = lava_ocean_level + 1.0;
        particle_system.add_spawn_position([0.0, ember_y, 0.0]);
        particle_system.add_spawn_position([15.0, ember_y, 15.0]);
        particle_system.add_spawn_position([-15.0, ember_y, 15.0]);
        particle_system.add_spawn_position([15.0, ember_y, -15.0]);
        particle_system.add_spawn_position([-15.0, ember_y, -15.0]);
        particle_system.add_spawn_position([25.0, ember_y, 0.0]);
        particle_system.add_spawn_position([-25.0, ember_y, 0.0]);
        particle_system.add_spawn_position([0.0, ember_y, 30.0]);
        particle_system.add_spawn_position([0.0, ember_y, -30.0]);
        particle_system.set_spawn_rate(100.0);

        let mut material_system = MaterialSystem::new(&device);
        material_system.set_scene_config(SceneConfig::battle_arena());

        let mut fog_post =
            FogPostPass::with_config(&device, &queue, surface_format, FogPostConfig::battle_arena());

        // Configure lava steam boundary wall around islands
        fog_post.set_steam_config(LavaSteamConfig::battle_arena(
            attacker_center,
            defender_center,
            island_radius,
            lava_ocean_level,
        ));

        // Store everything
        self.window = Some(window);
        self.scene = Some(scene);
        self.gpu = Some(GpuResources {
            device,
            queue,
            surface,
            surface_config,
            pipeline,
            sdf_cannon_pipeline,
            ui_pipeline,
            uniform_buffer,
            uniform_bind_group,
            static_vertex_buffer,
            static_index_buffer,
            static_index_count: static_mesh.indices.len() as u32,
            dynamic_vertex_buffer,
            dynamic_index_buffer,
            hex_wall_vertex_buffer,
            hex_wall_index_buffer,
            hex_wall_index_count,
            sdf_cannon_uniform_buffer,
            sdf_cannon_data_buffer,
            sdf_cannon_bind_group,
            depth_texture: depth_texture_view,
            depth_texture_raw: depth_texture,
            scene_color_texture,
            scene_color_view,
            ui_uniform_buffer,
            ui_bind_group,
            block_vertex_buffer: None,
            block_index_buffer: None,
            block_index_count: 0,
            merged_mesh_buffers: Vec::new(),
            tree_vertex_buffer,
            tree_index_buffer,
            tree_index_count,
            // Lava ocean
            lava_pipeline,
            lava_bind_group,
            lava_scene_uniform_buffer,
            lava_params_buffer,
            lava_vertex_buffer,
            lava_index_buffer,
            lava_index_count,
        });
        self.cubemap_skybox = Some(cubemap_skybox);
        self.point_lights = Some(point_lights);
        self.particle_system = Some(particle_system);
        self.material_system = Some(material_system);
        self.fog_post = Some(fog_post);

        println!(
            "[Battle Arena] Hex-prism walls: {} vertices, {} indices",
            hex_wall_vertices.len(),
            hex_indices.len()
        );
    }

    fn update(&mut self, delta_time: f32) {
        // Build input state structs from raw keys
        let movement = MovementState {
            forward: self.movement.forward,
            backward: self.movement.backward,
            left: self.movement.left,
            right: self.movement.right,
            up: self.movement.up,
            down: self.movement.down,
            sprint: self.movement.sprint,
        };
        let camera_forward = self.camera.get_forward();

        // Scene update: delegate all game logic
        {
            let scene = self.scene.as_mut().unwrap();
            scene.camera_yaw = self.camera.yaw;
            scene.update(delta_time, &movement, camera_forward);

            if scene.first_person_mode {
                self.camera.position = scene.player.get_eye_position();
            }
        }

        // Free camera mode (no scene borrow needed)
        if !self.scene.as_ref().unwrap().first_person_mode {
            let forward = if self.movement.forward { 1.0 } else { 0.0 }
                - if self.movement.backward { 1.0 } else { 0.0 };
            let right = if self.movement.right { 1.0 } else { 0.0 }
                - if self.movement.left { 1.0 } else { 0.0 };
            let up = if self.movement.up { 1.0 } else { 0.0 }
                - if self.movement.down { 1.0 } else { 0.0 };
            self.camera
                .update_movement(forward, right, up, delta_time, self.movement.sprint);
        }

        // Update Phase 2 visual systems
        let time = self.start_time.elapsed().as_secs_f32();
        if let (Some(point_lights), Some(gpu)) = (&mut self.point_lights, &self.gpu) {
            point_lights.update(&gpu.queue, time);
        }
        if let Some(ref mut particle_system) = self.particle_system {
            particle_system.update(delta_time);
        }

        // Rebuild terrain if scene flagged it (drop scene borrow before calling self methods)
        if self.scene.as_ref().unwrap().terrain_needs_rebuild {
            self.rebuild_terrain();
            self.scene.as_mut().unwrap().terrain_needs_rebuild = false;
        }

        // Regenerate hex-prism mesh if dirty
        if self.scene.as_ref().unwrap().hex_grid.needs_mesh_update() {
            self.regenerate_hex_wall_mesh();
        }

        // Update builder cursor and block preview
        self.update_builder_cursor();
        self.update_block_preview();

        // Block pickup system
        const PICKUP_HOLD_TIME: f32 = 0.5;
        let toolbar_visible = self.scene.as_ref().unwrap().building.toolbar().visible;
        if self.left_mouse_pressed && toolbar_visible {
            self.scene
                .as_mut()
                .unwrap()
                .building
                .toolbar_mut()
                .mouse_hold_time += delta_time;

            let hold_time = self
                .scene
                .as_ref()
                .unwrap()
                .building
                .toolbar()
                .mouse_hold_time;
            let pickup_in_progress = self
                .scene
                .as_ref()
                .unwrap()
                .building
                .toolbar()
                .pickup_in_progress;

            if hold_time >= PICKUP_HOLD_TIME && !pickup_in_progress {
                self.scene
                    .as_mut()
                    .unwrap()
                    .building
                    .toolbar_mut()
                    .pickup_in_progress = true;

                // Raycast without scene borrow
                if let Some((block_id, shape, material)) = self.raycast_to_loose_block() {
                    let mut did_stash = false;
                    let mut count = 0;
                    let mut max = 0;
                    {
                        let scene = self.scene.as_mut().unwrap();
                        if scene
                            .building
                            .toolbar_mut()
                            .inventory
                            .stash(shape, material)
                        {
                            scene.building.block_physics.unregister_block(block_id);
                            scene.building.block_manager.remove_block(block_id);
                            count = scene.building.toolbar().inventory.count();
                            max = scene.building.toolbar().inventory.max_capacity;
                            did_stash = true;
                        }
                    }
                    if did_stash {
                        self.regenerate_block_mesh();
                        println!("[Pickup] Stashed block (inventory: {}/{})", count, max);
                    }
                }
            }
        } else {
            self.scene
                .as_mut()
                .unwrap()
                .building
                .toolbar_mut()
                .mouse_hold_time = 0.0;
            self.scene
                .as_mut()
                .unwrap()
                .building
                .toolbar_mut()
                .pickup_in_progress = false;
        }

        // Update building physics mesh if blocks changed
        let needs_mesh_update = {
            let scene = self.scene.as_mut().unwrap();
            let mut needs_update = false;
            for block in scene.building.block_manager.blocks() {
                if scene.building.block_physics.is_falling(block.id) {
                    needs_update = true;
                    break;
                }
            }
            let removed = scene.building.update_physics(delta_time);
            if !removed.is_empty() {
                needs_update = true;
                for block_id in &removed {
                    if let Some(block) = scene.building.block_manager.get_block(*block_id) {
                        let debris = battle_tok_engine::game::spawn_debris(
                            block.position,
                            block.material,
                            8,
                        );
                        scene.destruction.add_debris(debris);
                        scene.building.block_physics.unregister_block(*block_id);
                    }
                    scene.building.block_manager.remove_block(*block_id);
                }
            }
            needs_update
        };
        if needs_mesh_update {
            self.regenerate_block_mesh();
        }
    }

    /// Regenerate hex-prism wall mesh and update GPU buffers
    fn regenerate_hex_wall_mesh(&mut self) {
        let Some(ref mut gpu) = self.gpu else { return };
        let scene = self.scene.as_mut().unwrap();

        let (hex_vertices, hex_indices) = scene.hex_grid.generate_combined_mesh();
        let hex_wall_vertices: Vec<Vertex> = hex_vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            })
            .collect();

        gpu.hex_wall_index_count = hex_indices.len() as u32;
        if !hex_wall_vertices.is_empty() {
            gpu.queue.write_buffer(
                &gpu.hex_wall_vertex_buffer,
                0,
                bytemuck::cast_slice(&hex_wall_vertices),
            );
            gpu.queue.write_buffer(
                &gpu.hex_wall_index_buffer,
                0,
                bytemuck::cast_slice(&hex_indices),
            );
        }
        scene.hex_grid.clear_mesh_dirty();
    }

    fn rebuild_terrain(&mut self) {
        let gpu = self.gpu.as_mut().expect("GPU not initialized");
        let scene = self.scene.as_mut().unwrap();
        let config = &scene.config;

        let attacker_center = config.island_attacker.position;
        let defender_center = config.island_defender.position;
        let island_radius = config.island_attacker.radius;

        let island_cfg = FloatingIslandConfig {
            radius: island_radius,
            surface_height: config.island_attacker.surface_height,
            island_thickness: config.island_attacker.thickness,
            taper_amount: config.island_attacker.taper_amount,
            num_layers: 5,
            edge_noise: 3.0,
        };

        let mut static_mesh = Mesh::new();
        let attacker_platform = generate_floating_island(attacker_center, island_cfg);
        static_mesh.merge(&attacker_platform);
        let defender_platform = generate_floating_island(defender_center, island_cfg);
        static_mesh.merge(&defender_platform);
        // Lava ocean NOT merged — rendered separately with animated lava shader
        let lava_ocean = generate_lava_ocean(config.lava_size, -0.5);

        // Rebuild lava GPU buffers
        gpu.lava_vertex_buffer =
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Lava Vertex Buffer"),
                    contents: bytemuck::cast_slice(&lava_ocean.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
        gpu.lava_index_buffer =
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Lava Index Buffer"),
                    contents: bytemuck::cast_slice(&lava_ocean.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
        gpu.lava_index_count = lava_ocean.indices.len() as u32;

        // Bridge
        let surface_min_y = attacker_center.y;
        let mut bridge_start = Vec3::new(0.0, surface_min_y, attacker_center.z - island_radius);
        let mut best_dist_start = f32::MAX;
        for v in &attacker_platform.vertices {
            let (vx, vy, vz) = (v.position[0], v.position[1], v.position[2]);
            if vy >= surface_min_y {
                let dist = vx.abs() + (vz - (attacker_center.z - island_radius)).abs() * 0.5;
                if dist < best_dist_start && vz < attacker_center.z {
                    best_dist_start = dist;
                    bridge_start = Vec3::new(vx, vy, vz);
                }
            }
        }
        let mut bridge_end = Vec3::new(0.0, surface_min_y, defender_center.z + island_radius);
        let mut best_dist_end = f32::MAX;
        for v in &defender_platform.vertices {
            let (vx, vy, vz) = (v.position[0], v.position[1], v.position[2]);
            if vy >= surface_min_y {
                let dist = vx.abs() + ((defender_center.z + island_radius) - vz).abs() * 0.5;
                if dist < best_dist_end && vz > defender_center.z {
                    best_dist_end = dist;
                    bridge_end = Vec3::new(vx, vy, vz);
                }
            }
        }
        let bridge_cfg = BridgeConfig::default();
        let bridge_mesh = generate_bridge(bridge_start, bridge_end, &bridge_cfg);
        static_mesh.merge(&bridge_mesh);

        // Update bridge for player ground collision
        scene.set_bridge(bridge_start, bridge_end);

        gpu.static_vertex_buffer =
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Static Vertex Buffer"),
                    contents: bytemuck::cast_slice(&static_mesh.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
        gpu.static_index_buffer =
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Static Index Buffer"),
                    contents: bytemuck::cast_slice(&static_mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
        gpu.static_index_count = static_mesh.indices.len() as u32;

        // Regenerate trees
        scene.trees_attacker = generate_trees_on_terrain(attacker_center, 28.0, 0.3, 0.0);
        scene.trees_defender = generate_trees_on_terrain(defender_center, 28.0, 0.35, 100.0);
        let mut all_trees = scene.trees_attacker.clone();
        all_trees.extend(scene.trees_defender.clone());
        let tree_mesh = generate_all_trees_mesh(&all_trees);

        if !tree_mesh.vertices.is_empty() {
            gpu.tree_vertex_buffer =
                gpu.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Tree Vertex Buffer"),
                        contents: bytemuck::cast_slice(&tree_mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
            gpu.tree_index_buffer =
                gpu.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Tree Index Buffer"),
                        contents: bytemuck::cast_slice(&tree_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
            gpu.tree_index_count = tree_mesh.indices.len() as u32;
        }
    }

    /// Raycast from camera to find a loose (physics-detached) block
    fn raycast_to_loose_block(
        &self,
    ) -> Option<(u32, battle_tok_engine::render::BuildingBlockShape, u8)> {
        let mouse_pos = self.current_mouse_pos?;
        let _gpu = self.gpu.as_ref()?;
        let scene = self.scene.as_ref()?;

        let (ray_origin, ray_dir) = self.screen_to_ray(mouse_pos.0, mouse_pos.1);
        let max_dist = 20.0;
        let step_size = 0.25;
        let mut t = 0.5;

        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            if let Some((block_id, dist)) = scene.building.block_manager.find_closest(p, 0.5)
                && dist < 0.5
                && scene.building.block_physics.is_loose(block_id)
                && let Some(block) = scene.building.block_manager.get_block(block_id)
            {
                return Some((block_id, block.shape, block.material));
            }
            t += step_size;
        }
        None
    }

    /// Convert screen coordinates to a world-space ray
    fn screen_to_ray(&self, screen_x: f32, screen_y: f32) -> (Vec3, Vec3) {
        let Some(ref gpu) = self.gpu else {
            return (self.camera.position, self.camera.get_forward());
        };
        let width = gpu.surface_config.width as f32;
        let height = gpu.surface_config.height as f32;
        let ndc_x = (2.0 * screen_x / width) - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_y / height);
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = right.cross(forward).normalize();
        let aspect = width / height;
        let half_fov_tan = (self.camera.fov / 2.0).tan();
        let ray_dir = (forward + right * ndc_x * half_fov_tan * aspect + up * ndc_y * half_fov_tan)
            .normalize();
        (self.camera.position, ray_dir)
    }

    /// Update builder mode cursor position
    fn update_builder_cursor(&mut self) {
        if !self.builder_mode.enabled {
            self.builder_mode.cursor_coord = None;
            return;
        }
        if let Some((mx, my)) = self.current_mouse_pos {
            self.builder_mode.cursor_coord = self.raycast_to_build_position(mx, my);
        }
    }

    /// Raycast from screen position to find build position
    fn raycast_to_build_position(&self, screen_x: f32, screen_y: f32) -> Option<(i32, i32, i32)> {
        let scene = self.scene.as_ref()?;
        let (ray_origin, ray_dir) = self.screen_to_ray(screen_x, screen_y);

        if let Some(hit) = scene.hex_grid.ray_cast(ray_origin, ray_dir, 200.0) {
            let above = (hit.prism_coord.0, hit.prism_coord.1, hit.prism_coord.2 + 1);
            return Some(above);
        }

        let attacker_center = scene.config.island_attacker.position;
        let defender_center = scene.config.island_defender.position;
        let platform_radius = scene.config.island_attacker.radius;

        let max_dist = 200.0;
        let step_size = 0.5;
        let mut t = 0.0;
        while t < max_dist {
            let point = ray_origin + ray_dir * t;
            let in_attacker = (point.x - attacker_center.x).powi(2)
                + (point.z - attacker_center.z).powi(2)
                < platform_radius * platform_radius;
            let in_defender = (point.x - defender_center.x).powi(2)
                + (point.z - defender_center.z).powi(2)
                < platform_radius * platform_radius;

            if in_attacker || in_defender {
                let base_y = if in_attacker {
                    attacker_center.y
                } else {
                    defender_center.y
                };
                let terrain_y = terrain_height_at(point.x, point.z, base_y);
                if point.y <= terrain_y + 0.1 {
                    let build_level = self.builder_mode.build_level;
                    let (q, r, _) = battle_tok_engine::render::hex_prism::world_to_axial(point);
                    return Some((q, r, build_level));
                }
            }
            t += step_size;
        }
        None
    }

    /// Update the preview position for block placement
    fn update_block_preview(&mut self) {
        let visible = self.scene.as_ref().unwrap().building.toolbar().visible;
        if visible {
            let pos = self.calculate_block_placement_position();
            self.scene
                .as_mut()
                .unwrap()
                .building
                .toolbar_mut()
                .preview_position = pos;
        } else {
            self.scene
                .as_mut()
                .unwrap()
                .building
                .toolbar_mut()
                .preview_position = None;
        }
    }

    /// Calculate the snapped position for block placement
    fn calculate_block_placement_position(&self) -> Option<Vec3> {
        let gpu = self.gpu.as_ref()?;
        let scene = self.scene.as_ref()?;

        let mouse_pos = self.current_mouse_pos.unwrap_or((
            gpu.surface_config.width as f32 / 2.0,
            gpu.surface_config.height as f32 / 2.0,
        ));

        let (ray_origin, ray_dir) = self.screen_to_ray(mouse_pos.0, mouse_pos.1);
        scene
            .building
            .calculate_placement(ray_origin, ray_dir, &|x, z| terrain_height_at(x, z, 0.0))
    }

    /// Place a building block at the preview position
    fn place_building_block(&mut self) {
        let preview_pos = self
            .scene
            .as_ref()
            .unwrap()
            .building
            .toolbar()
            .preview_position;
        let position = match preview_pos {
            Some(pos) => pos,
            None => match self.calculate_block_placement_position() {
                Some(pos) => pos,
                None => return,
            },
        };

        if let Some(_block_id) = self.scene.as_mut().unwrap().building.place_block(position) {
            self.regenerate_block_mesh();
        }
    }

    /// Handle block click for double-click merging
    fn handle_block_click(&mut self) -> bool {
        // Raycast without scene borrow
        let block_id = match self.raycast_to_block() {
            Some(id) => id,
            None => return false,
        };

        let merge_result = {
            let scene = self.scene.as_mut().unwrap();
            if let Some(blocks_to_merge) = scene
                .building
                .merge_workflow
                .on_block_click(block_id, &scene.building.block_manager)
            {
                let color = if let Some(block) =
                    scene.building.block_manager.get_block(blocks_to_merge[0])
                {
                    get_material_color(block.material)
                } else {
                    [0.5, 0.5, 0.5, 1.0]
                };

                let merged = scene.building.merge_workflow.merge_blocks(
                    &blocks_to_merge,
                    &mut scene.building.block_manager,
                    color,
                );
                Some(merged)
            } else {
                None
            }
        };
        if let Some(merged) = merge_result {
            if let Some(merged) = merged {
                self.create_merged_mesh_buffers(merged);
            }
            self.regenerate_block_mesh();
            return true;
        }
        false
    }

    /// Raycast from camera to find which building block is hit
    fn raycast_to_block(&self) -> Option<u32> {
        let mouse_pos = self.current_mouse_pos?;
        let scene = self.scene.as_ref()?;
        let (ray_origin, ray_dir) = self.screen_to_ray(mouse_pos.0, mouse_pos.1);
        let max_dist = 100.0;
        let step_size = 0.25;
        let mut t = 0.5;
        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            if let Some((block_id, dist)) = scene.building.block_manager.find_closest(p, 0.5)
                && dist < 0.5
            {
                return Some(block_id);
            }
            t += step_size;
        }
        None
    }

    /// Raycast to find which face of a block is pointed at
    fn raycast_to_face(&self) -> Option<SelectedFace> {
        let mouse_pos = self.current_mouse_pos?;
        let scene = self.scene.as_ref()?;
        let (ray_origin, ray_dir) = self.screen_to_ray(mouse_pos.0, mouse_pos.1);
        let max_dist = 50.0;
        let step_size = 0.1;
        let mut t = 0.5;
        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            if let Some((block_id, dist)) = scene.building.block_manager.find_closest(p, 1.0)
                && dist < 0.3
                && let Some(block) = scene.building.block_manager.get_block(block_id)
            {
                let aabb = block.aabb();
                let center = (aabb.min + aabb.max) * 0.5;
                let half_size = (aabb.max - aabb.min) * 0.5;
                let offset = p - center;
                let abs_offset = Vec3::new(offset.x.abs(), offset.y.abs(), offset.z.abs());

                let (normal, face_pos, size) =
                    if abs_offset.x > abs_offset.y && abs_offset.x > abs_offset.z {
                        let n = Vec3::new(offset.x.signum(), 0.0, 0.0);
                        let pos = center + n * half_size.x;
                        (n, pos, (half_size.z * 2.0, half_size.y * 2.0))
                    } else if abs_offset.y > abs_offset.z {
                        let n = Vec3::new(0.0, offset.y.signum(), 0.0);
                        let pos = center + n * half_size.y;
                        (n, pos, (half_size.x * 2.0, half_size.z * 2.0))
                    } else {
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
            t += step_size;
        }
        None
    }

    /// Handle click in bridge mode
    fn handle_bridge_click(&mut self) -> bool {
        if !self
            .scene
            .as_ref()
            .unwrap()
            .building
            .toolbar()
            .is_bridge_mode()
        {
            return false;
        }
        // Raycast without scene borrow
        if let Some(face) = self.raycast_to_face() {
            self.scene
                .as_mut()
                .unwrap()
                .building
                .toolbar_mut()
                .bridge_tool
                .select_face(face);
            let is_ready = self
                .scene
                .as_ref()
                .unwrap()
                .building
                .toolbar()
                .bridge_tool
                .is_ready();
            if is_ready {
                self.create_bridge();
                self.scene
                    .as_mut()
                    .unwrap()
                    .building
                    .toolbar_mut()
                    .bridge_tool
                    .clear();
            }
            return true;
        }
        false
    }

    /// Create a bridge between two selected faces
    fn create_bridge(&mut self) {
        let scene = self.scene.as_mut().unwrap();
        let first = match scene.building.toolbar().bridge_tool.first_face {
            Some(f) => f,
            None => return,
        };
        let second = match scene.building.toolbar().bridge_tool.second_face {
            Some(f) => f,
            None => return,
        };

        let start = first.position;
        let end = second.position;
        let direction = end - start;
        let length = direction.length();
        if length < 0.1 {
            return;
        }

        let num_segments = (length / battle_tok_engine::game::BLOCK_GRID_SIZE).ceil() as i32;
        let segment_length = length / num_segments as f32;

        for i in 0..=num_segments {
            let t = i as f32 / num_segments as f32;
            let pos = start + direction * t;
            let w = first.size.0 * (1.0 - t) + second.size.0 * t;
            let h = first.size.1 * (1.0 - t) + second.size.1 * t;
            let half_extents = Vec3::new(segment_length * 0.6, h * 0.5, w * 0.5);
            let shape = battle_tok_engine::render::BuildingBlockShape::Cube { half_extents };
            let block = BuildingBlock::new(shape, pos, scene.building.toolbar().selected_material);
            let block_id = scene.building.block_manager.add_block(block);
            scene.building.block_physics.register_block(block_id);
        }
        self.regenerate_block_mesh();
    }

    /// Create GPU buffers for a merged mesh
    fn create_merged_mesh_buffers(&mut self, merged: MergedMesh) {
        let gpu = match &mut self.gpu {
            Some(g) => g,
            None => return,
        };
        if merged.vertices.is_empty() {
            return;
        }

        let mesh_vertices: Vec<Vertex> = merged
            .vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            })
            .collect();

        let vertex_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Merged Mesh Vertex Buffer"),
                contents: bytemuck::cast_slice(&mesh_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Merged Mesh Index Buffer"),
                contents: bytemuck::cast_slice(&merged.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        gpu.merged_mesh_buffers.push(MergedMeshBuffers {
            _id: merged.id,
            vertex_buffer,
            index_buffer,
            index_count: merged.indices.len() as u32,
        });
    }

    /// Regenerate the building block mesh buffer
    fn regenerate_block_mesh(&mut self) {
        let gpu = match &mut self.gpu {
            Some(g) => g,
            None => return,
        };
        let scene = self.scene.as_ref().unwrap();

        let (vertices, indices) = scene.building.block_manager.generate_combined_mesh();
        if vertices.is_empty() {
            gpu.block_index_count = 0;
            return;
        }

        let mesh_vertices: Vec<Vertex> = vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            })
            .collect();

        gpu.block_vertex_buffer = Some(gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Block Vertex Buffer"),
                contents: bytemuck::cast_slice(&mesh_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));
        gpu.block_index_buffer = Some(gpu.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Block Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            },
        ));
        gpu.block_index_count = indices.len() as u32;
    }

    /// Generate ghost preview mesh for builder mode
    fn generate_ghost_preview_mesh(&self, time: f32) -> Option<Mesh> {
        if !self.builder_mode.enabled || !self.builder_mode.show_preview {
            return None;
        }
        let scene = self.scene.as_ref()?;
        let coord = self.builder_mode.cursor_coord?;
        if scene.hex_grid.contains(coord.0, coord.1, coord.2) {
            return None;
        }

        let world_pos =
            battle_tok_engine::render::hex_prism::axial_to_world(coord.0, coord.1, coord.2);
        let prism = battle_tok_engine::render::HexPrism::with_center(
            world_pos,
            DEFAULT_HEX_HEIGHT,
            DEFAULT_HEX_RADIUS,
            self.builder_mode.selected_material,
        );
        let (hex_vertices, hex_indices) = prism.generate_mesh();
        let pulse = 0.5 + (time * 4.0).sin() * 0.3;
        let ghost_color = [0.3, 0.9, 0.4, pulse];
        let vertices: Vec<Vertex> = hex_vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: ghost_color,
            })
            .collect();
        Some(Mesh {
            vertices,
            indices: hex_indices,
        })
    }

    /// Generate grid overlay mesh for builder mode
    fn generate_grid_overlay_mesh(&self) -> Option<Mesh> {
        if !self.builder_mode.enabled {
            return None;
        }
        let coord = self.builder_mode.cursor_coord?;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let line_thickness = 0.04;
        let grid_radius: i32 = 3;
        let hex_radius = DEFAULT_HEX_RADIUS;

        for dq in -grid_radius..=grid_radius {
            for dr in -grid_radius..=grid_radius {
                let ds = -dq - dr;
                let hex_dist = (dq.abs() + dr.abs() + ds.abs()) / 2;
                if hex_dist > grid_radius {
                    continue;
                }

                let q = coord.0 + dq;
                let r = coord.1 + dr;
                let world_pos = battle_tok_engine::render::hex_prism::axial_to_world(q, r, coord.2);
                let is_cursor = dq == 0 && dr == 0;
                let cell_color = if is_cursor {
                    [0.2, 1.0, 0.4, 0.9]
                } else {
                    [0.3, 0.7, 1.0, 0.4]
                };

                for i in 0..6 {
                    let angle1 = (i as f32) * std::f32::consts::PI / 3.0;
                    let angle2 = ((i + 1) % 6) as f32 * std::f32::consts::PI / 3.0;
                    let x1 = world_pos.x + hex_radius * angle1.sin();
                    let z1 = world_pos.z + hex_radius * angle1.cos();
                    let x2 = world_pos.x + hex_radius * angle2.sin();
                    let z2 = world_pos.z + hex_radius * angle2.cos();
                    let y1 = terrain_height_at(x1, z1, 0.0) + 0.1;
                    let y2 = terrain_height_at(x2, z2, 0.0) + 0.1;

                    let edge_dx = x2 - x1;
                    let edge_dz = z2 - z1;
                    let edge_len = (edge_dx * edge_dx + edge_dz * edge_dz).sqrt();
                    if edge_len < 0.001 {
                        continue;
                    }
                    let perp_x = -edge_dz / edge_len * line_thickness;
                    let perp_z = edge_dx / edge_len * line_thickness;

                    let base_idx = vertices.len() as u32;
                    vertices.push(Vertex {
                        position: [x1 - perp_x, y1, z1 - perp_z],
                        normal: [0.0, 1.0, 0.0],
                        color: cell_color,
                    });
                    vertices.push(Vertex {
                        position: [x1 + perp_x, y1, z1 + perp_z],
                        normal: [0.0, 1.0, 0.0],
                        color: cell_color,
                    });
                    vertices.push(Vertex {
                        position: [x2 + perp_x, y2, z2 + perp_z],
                        normal: [0.0, 1.0, 0.0],
                        color: cell_color,
                    });
                    vertices.push(Vertex {
                        position: [x2 - perp_x, y2, z2 - perp_z],
                        normal: [0.0, 1.0, 0.0],
                        color: cell_color,
                    });
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
        // Acquire surface texture (scoped to release borrow of self.gpu)
        let output = {
            let Some(ref gpu) = self.gpu else { return };
            match gpu.surface.get_current_texture() {
                Ok(t) => t,
                Err(_) => return,
            }
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let time = self.start_time.elapsed().as_secs_f32();
        let now = std::time::Instant::now();
        let delta_time = now.duration_since(self.last_frame).as_secs_f32().min(0.1);

        let dynamic_index_count = self.prepare_frame_data(time, delta_time);

        let mut encoder = self.gpu.as_ref().unwrap().device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            },
        );

        // Phase 1: Render scene to offscreen texture (for fog post-process)
        let gpu = self.gpu.as_ref().unwrap();
        let scene_view = &gpu.scene_color_view;

        self.render_sky(&mut encoder, scene_view);
        self.render_meshes(&mut encoder, scene_view, dynamic_index_count);
        self.render_lava(&mut encoder, scene_view);
        self.render_sdf_cannon(&mut encoder, scene_view);
        self.render_particles(&mut encoder, scene_view);

        // Phase 2: Fog post-process (reads scene color + depth → writes to swapchain)
        if let Some(ref fog_post) = self.fog_post {
            let gpu = self.gpu.as_ref().unwrap();
            fog_post.render_to_view(
                &gpu.device,
                &mut encoder,
                &gpu.scene_color_view,
                &gpu.depth_texture,
                &view,
            );
        }

        // Phase 3: UI on top (no fog applied to UI)
        self.render_ui(&mut encoder, &view);

        self.gpu
            .as_ref()
            .unwrap()
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();
    }

    /// Prepare all GPU buffer data for the current frame: dynamic meshes, uniforms,
    /// SDF cannon data, and visual system updates. Returns the dynamic index count.
    fn prepare_frame_data(&mut self, time: f32, _delta_time: f32) -> u32 {
        // Build dynamic mesh from scene data (needs &self.scene, then &self for ghost/grid)
        let mut dynamic_mesh = self.scene.as_ref().unwrap().generate_dynamic_mesh();
        let mut dynamic_indices: Vec<u32> = (0..dynamic_mesh.len() as u32).collect();

        if let Some(ghost_mesh) = self.generate_ghost_preview_mesh(time) {
            let base = dynamic_mesh.len() as u32;
            dynamic_mesh.extend(ghost_mesh.vertices);
            dynamic_indices.extend(ghost_mesh.indices.iter().map(|i| i + base));
        }

        if let Some(grid_mesh) = self.generate_grid_overlay_mesh() {
            let base = dynamic_mesh.len() as u32;
            dynamic_mesh.extend(grid_mesh.vertices);
            dynamic_indices.extend(grid_mesh.indices.iter().map(|i| i + base));
        }

        let dynamic_index_count = dynamic_indices.len() as u32;

        let gpu = self.gpu.as_ref().unwrap();
        let queue = &gpu.queue;
        let config = &gpu.surface_config;
        let scene = self.scene.as_ref().unwrap();

        // Update dynamic buffers
        if !dynamic_mesh.is_empty() {
            queue.write_buffer(
                &gpu.dynamic_vertex_buffer,
                0,
                bytemuck::cast_slice(&dynamic_mesh),
            );
            queue.write_buffer(
                &gpu.dynamic_index_buffer,
                0,
                bytemuck::cast_slice(&dynamic_indices),
            );
        }

        // Update uniforms
        let aspect = config.width as f32 / config.height as f32;
        let view_mat = self.camera.get_view_matrix();
        let proj_mat = self.camera.get_projection_matrix(aspect);
        let view_proj = proj_mat * view_mat;

        let vis = &scene.visuals;
        let mut uniforms = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: self.camera.position.to_array(),
            time,
            sun_dir: vis.sun_direction.to_array(),
            fog_density: vis.fog_density,
            fog_color: vis.fog_color.to_array(),
            ambient: vis.ambient_intensity,
            projectile_count: scene.projectiles.active_count() as u32,
            ..Default::default()
        };

        for (i, projectile) in scene.projectiles.iter().enumerate().take(32) {
            uniforms.projectile_positions[i] = [
                projectile.position.x,
                projectile.position.y,
                projectile.position.z,
                projectile.radius,
            ];
        }

        queue.write_buffer(&gpu.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // SDF cannon uniforms
        {
            let inv_view_proj = view_proj.inverse();
            let sdf_uniforms = SdfCannonUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                inv_view_proj: inv_view_proj.to_cols_array_2d(),
                camera_pos: self.camera.position.to_array(),
                time,
                sun_dir: vis.sun_direction.to_array(),
                fog_density: vis.fog_density,
                fog_color: vis.fog_color.to_array(),
                ambient: vis.ambient_intensity,
            };
            queue.write_buffer(
                &gpu.sdf_cannon_uniform_buffer,
                0,
                bytemuck::cast_slice(&[sdf_uniforms]),
            );

            let cannon = scene.cannon.cannon();
            // Compute barrel rotation quaternion from look direction.
            // The barrel points along -Z by default, so we need the rotation
            // from -Z to the current barrel direction.
            let barrel_dir = cannon.get_barrel_direction();
            let default_dir = Vec3::new(0.0, 0.0, -1.0);
            // Quaternion from default_dir to barrel_dir
            let barrel_rotation = {
                let dot = default_dir.dot(barrel_dir);
                if dot > 0.9999 {
                    [0.0_f32, 0.0, 0.0, 1.0] // identity
                } else if dot < -0.9999 {
                    [0.0_f32, 1.0, 0.0, 0.0] // 180° around Y
                } else {
                    let cross = default_dir.cross(barrel_dir);
                    let w = 1.0 + dot;
                    let len = (cross.x * cross.x + cross.y * cross.y + cross.z * cross.z + w * w).sqrt();
                    [cross.x / len, cross.y / len, cross.z / len, w / len]
                }
            };

            // Color: golden highlight when grabbed, bronze otherwise
            let color = if cannon.grabbed {
                [0.6, 0.5, 0.2]
            } else {
                [0.4, 0.35, 0.3]
            };

            let sdf_cannon_data = SdfCannonData {
                world_pos: cannon.position.to_array(),
                _pad0: 0.0,
                barrel_rotation,
                color,
                _pad1: 0.0,
            };
            queue.write_buffer(
                &gpu.sdf_cannon_data_buffer,
                0,
                bytemuck::cast_slice(&[sdf_cannon_data]),
            );
        }

        // Lava ocean uniforms (animated shader)
        {
            let lava_scene = LavaSceneUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                camera_pos: self.camera.position.to_array(),
                time,
                sun_dir: vis.sun_direction.to_array(),
                fog_density: vis.fog_density,
                fog_color: vis.fog_color.to_array(),
                ambient: vis.ambient_intensity,
            };
            queue.write_buffer(
                &gpu.lava_scene_uniform_buffer,
                0,
                bytemuck::cast_slice(&[lava_scene]),
            );

            let lava_params = LavaParams {
                time,
                ..LavaParams::default()
            };
            queue.write_buffer(
                &gpu.lava_params_buffer,
                0,
                bytemuck::cast_slice(&[lava_params]),
            );
        }

        // Update visual systems — skybox with day/night crossfade
        if let Some(ref cubemap_skybox) = self.cubemap_skybox {
            // Compute blend factor from DayCycle time (0-1)
            // Dawn (0.0-0.15): night → day transition
            // Day  (0.15-0.65): pure day
            // Dusk (0.65-0.8): day → night transition
            // Night(0.8-1.0): pure night
            let day_time = scene.game_state.day_cycle.time();
            let blend = if day_time < 0.15 {
                // Dawn: fade from night (1.0) to day (0.0)
                1.0 - (day_time / 0.15)
            } else if day_time < 0.65 {
                // Day: pure day
                0.0
            } else if day_time < 0.80 {
                // Dusk: fade from day (0.0) to night (1.0)
                (day_time - 0.65) / 0.15
            } else {
                // Night: pure night
                1.0
            };
            cubemap_skybox.update(queue, view_proj, blend);
        }
        if let Some(ref material_system) = self.material_system {
            material_system.update_scene_uniforms(queue, view_proj, self.camera.position, time);
        }
        if let Some(ref fog_post) = self.fog_post {
            fog_post.update(queue, view_proj, self.camera.position, time);
        }

        // Update particle uniforms
        if let Some(ref particle_system) = self.particle_system {
            particle_system.upload_particles(queue);
            particle_system.update_uniforms(
                queue,
                view_mat.to_cols_array_2d(),
                proj_mat.to_cols_array_2d(),
            );
        }

        dynamic_index_count
    }

    /// Render the cubemap skybox background (no depth test).
    fn render_sky(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if let Some(ref cubemap_skybox) = self.cubemap_skybox {
            cubemap_skybox.render_to_view(encoder, view);
        }
    }

    /// Render all static and dynamic meshes with depth testing: terrain, hex walls,
    /// building blocks, block placement preview, merged meshes, and trees.
    fn render_meshes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        dynamic_index_count: u32,
    ) {
        let gpu = self.gpu.as_ref().unwrap();
        let scene = self.scene.as_ref().unwrap();

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Mesh Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &gpu.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&gpu.pipeline);
        render_pass.set_bind_group(0, &gpu.uniform_bind_group, &[]);

        // Static terrain
        render_pass.set_vertex_buffer(0, gpu.static_vertex_buffer.slice(..));
        render_pass.set_index_buffer(gpu.static_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..gpu.static_index_count, 0, 0..1);

        // Dynamic mesh (projectiles, debris, ghost preview, grid overlay)
        if dynamic_index_count > 0 {
            render_pass.set_vertex_buffer(0, gpu.dynamic_vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                gpu.dynamic_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(0..dynamic_index_count, 0, 0..1);
        }

        // Hex walls
        if gpu.hex_wall_index_count > 0 {
            render_pass.set_vertex_buffer(0, gpu.hex_wall_vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                gpu.hex_wall_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(0..gpu.hex_wall_index_count, 0, 0..1);
        }

        // Building blocks
        if let (Some(block_vb), Some(block_ib)) =
            (&gpu.block_vertex_buffer, &gpu.block_index_buffer)
            && gpu.block_index_count > 0
        {
            render_pass.set_vertex_buffer(0, block_vb.slice(..));
            render_pass.set_index_buffer(block_ib.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..gpu.block_index_count, 0, 0..1);
        }

        // Block placement preview
        if scene.building.toolbar().visible
            && scene.building.toolbar().show_preview
            && !scene.building.toolbar().is_bridge_mode()
            && let Some(preview_pos) = scene.building.toolbar().preview_position
        {
            let shape = scene.building.toolbar().get_selected_shape();
            let preview_block = BuildingBlock::new(shape, preview_pos, 0);
            let (preview_verts, preview_indices) = preview_block.generate_mesh();

            let pulse = (self.start_time.elapsed().as_secs_f32() * 3.0).sin() * 0.5 + 0.5;
            let highlight_color = [0.2 + pulse * 0.3, 0.9, 0.2 + pulse * 0.2, 0.85];

            let preview_vertices: Vec<Vertex> = preview_verts
                .iter()
                .map(|v| Vertex {
                    position: v.position,
                    normal: v.normal,
                    color: highlight_color,
                })
                .collect();

            if !preview_vertices.is_empty() && !preview_indices.is_empty() {
                let preview_vb = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Preview VB"),
                        contents: bytemuck::cast_slice(&preview_vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                let preview_ib = gpu
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Preview IB"),
                        contents: bytemuck::cast_slice(&preview_indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                render_pass.set_vertex_buffer(0, preview_vb.slice(..));
                render_pass.set_index_buffer(preview_ib.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..preview_indices.len() as u32, 0, 0..1);
            }
        }

        // Merged meshes
        for merged in &gpu.merged_mesh_buffers {
            if merged.index_count > 0 {
                render_pass.set_vertex_buffer(0, merged.vertex_buffer.slice(..));
                render_pass
                    .set_index_buffer(merged.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..merged.index_count, 0, 0..1);
            }
        }

        // Trees
        if gpu.tree_index_count > 0 {
            render_pass.set_vertex_buffer(0, gpu.tree_vertex_buffer.slice(..));
            render_pass
                .set_index_buffer(gpu.tree_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..gpu.tree_index_count, 0, 0..1);
        }
    }

    /// Render the animated lava ocean with the dedicated lava.wgsl shader.
    /// Uses depth testing (loads existing depth from terrain pass).
    fn render_lava(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let gpu = self.gpu.as_ref().unwrap();
        if gpu.lava_index_count == 0 {
            return;
        }

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Lava Ocean Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &gpu.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Preserve depth from terrain
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&gpu.lava_pipeline);
        render_pass.set_bind_group(0, &gpu.lava_bind_group, &[]);
        render_pass.set_vertex_buffer(0, gpu.lava_vertex_buffer.slice(..));
        render_pass.set_index_buffer(gpu.lava_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..gpu.lava_index_count, 0, 0..1);
    }

    /// Render the SDF ray-marched cannon with its own pipeline and depth testing.
    fn render_sdf_cannon(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let gpu = self.gpu.as_ref().unwrap();

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("SDF Cannon Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &gpu.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        render_pass.set_pipeline(&gpu.sdf_cannon_pipeline);
        render_pass.set_bind_group(0, &gpu.sdf_cannon_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Render billboard particles (explosions, sparks) with depth testing.
    fn render_particles(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let Some(ref particle_system) = self.particle_system else {
            return;
        };
        let gpu = self.gpu.as_ref().unwrap();

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Particle Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &gpu.depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        particle_system.render(&mut render_pass);
    }

    /// Render all 2D UI overlays (no depth test): terrain editor, crosshair,
    /// build toolbar, top bar HUD, and start overlay.
    fn render_ui(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let gpu = self.gpu.as_ref().unwrap();
        let scene = self.scene.as_ref().unwrap();
        let config = &gpu.surface_config;
        let w = config.width as f32;
        let h = config.height as f32;

        // Terrain editor UI
        if self.terrain_ui.visible {
            let ui_mesh = self.terrain_ui.generate_ui_mesh(w, h);
            self.draw_ui_mesh(encoder, view, "UI Pass", &ui_mesh);
        }

        // Selection crosshair
        if scene.building.toolbar().visible && !self.start_overlay.visible {
            let crosshair_mesh = self.generate_crosshair_mesh(w, h);
            self.draw_ui_mesh(encoder, view, "Crosshair Pass", &crosshair_mesh);
        }

        // Build toolbar
        if scene.building.toolbar().visible {
            let toolbar_mesh = scene.building.toolbar().generate_ui_mesh(w, h);
            self.draw_ui_mesh(encoder, view, "Toolbar Pass", &toolbar_mesh);
        }

        // Top bar HUD
        if scene.game_state.top_bar.visible && !self.start_overlay.visible {
            let (resources, day_cycle, population) = scene.game_state.ui_data();
            let top_bar_mesh = scene
                .game_state
                .top_bar
                .generate_ui_mesh(w, h, resources, day_cycle, population);
            self.draw_ui_mesh(encoder, view, "Top Bar Pass", &top_bar_mesh);
        }

        // Start overlay
        if self.start_overlay.visible {
            let overlay_mesh = self.start_overlay.generate_ui_mesh(w, h);
            self.draw_ui_mesh(encoder, view, "Start Overlay Pass", &overlay_mesh);
        }
    }

    /// Helper: draw a UI mesh (2D overlay, no depth test) in its own render pass.
    fn draw_ui_mesh(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        label: &str,
        mesh: &Mesh,
    ) {
        if mesh.vertices.is_empty() {
            return;
        }
        let gpu = self.gpu.as_ref().unwrap();
        let vb = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let ib = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
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
        rp.set_pipeline(&gpu.ui_pipeline);
        rp.set_bind_group(0, &gpu.ui_bind_group, &[]);
        rp.set_vertex_buffer(0, vb.slice(..));
        rp.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
        rp.draw_indexed(0..mesh.indices.len() as u32, 0, 0..1);
    }

    /// Generate crosshair mesh for builder mode targeting.
    fn generate_crosshair_mesh(&self, w: f32, h: f32) -> Mesh {
        let (cx, cy) = self.current_mouse_pos.unwrap_or((w / 2.0, h / 2.0));
        let size = 25.0;
        let thickness = 4.0;
        let gap = 8.0;
        let pulse = (self.start_time.elapsed().as_secs_f32() * 3.0).sin() * 0.3 + 0.7;
        let crosshair_color = [pulse, 1.0, 0.2, 0.95];
        let to_ndc =
            |x: f32, y: f32| -> [f32; 3] { [(x / w) * 2.0 - 1.0, 1.0 - (y / h) * 2.0, 0.0] };
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        let add_quad = |verts: &mut Vec<Vertex>,
                        idxs: &mut Vec<u32>,
                        x1: f32,
                        y1: f32,
                        x2: f32,
                        y2: f32,
                        color: [f32; 4]| {
            let base = verts.len() as u32;
            let n = [0.0, 0.0, 1.0];
            verts.push(Vertex {
                position: to_ndc(x1, y1),
                normal: n,
                color,
            });
            verts.push(Vertex {
                position: to_ndc(x2, y1),
                normal: n,
                color,
            });
            verts.push(Vertex {
                position: to_ndc(x2, y2),
                normal: n,
                color,
            });
            verts.push(Vertex {
                position: to_ndc(x1, y2),
                normal: n,
                color,
            });
            idxs.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        };
        add_quad(
            &mut verts,
            &mut idxs,
            cx - size - gap,
            cy - thickness / 2.0,
            cx - gap,
            cy + thickness / 2.0,
            crosshair_color,
        );
        add_quad(
            &mut verts,
            &mut idxs,
            cx + gap,
            cy - thickness / 2.0,
            cx + size + gap,
            cy + thickness / 2.0,
            crosshair_color,
        );
        add_quad(
            &mut verts,
            &mut idxs,
            cx - thickness / 2.0,
            cy - size - gap,
            cx + thickness / 2.0,
            cy - gap,
            crosshair_color,
        );
        add_quad(
            &mut verts,
            &mut idxs,
            cx - thickness / 2.0,
            cy + gap,
            cx + thickness / 2.0,
            cy + size + gap,
            crosshair_color,
        );
        add_quad(
            &mut verts,
            &mut idxs,
            cx - 2.0,
            cy - 2.0,
            cx + 2.0,
            cy + 2.0,
            [1.0, 1.0, 1.0, 1.0],
        );

        Mesh {
            vertices: verts,
            indices: idxs,
        }
    }

    fn handle_key(&mut self, key: KeyCode, pressed: bool) {
        let scene = self.scene.as_mut().unwrap();

        match key {
            // Movement
            KeyCode::KeyW => self.movement.forward = pressed,
            KeyCode::KeyS => self.movement.backward = pressed,
            KeyCode::KeyA => self.movement.left = pressed,
            KeyCode::KeyD => self.movement.right = pressed,
            KeyCode::Space => {
                if pressed {
                    if scene.first_person_mode {
                        // In first-person mode: jump first, but also fire if near cannon
                        scene.player.request_jump();
                    } else if !self.builder_mode.enabled {
                        scene.fire_cannon();
                    }
                }
                self.movement.up = pressed;
            }
            KeyCode::KeyF if pressed => {
                // F key: Fire cannon (works in any mode if close enough)
                scene.fire_cannon();
            }
            KeyCode::KeyG if pressed => {
                // G key: Grab/release cannon
                let changed = scene.toggle_cannon_grab();
                if changed {
                    let grabbed = scene.cannon.is_grabbed();
                    println!(
                        "[Cannon] {}",
                        if grabbed { "Grabbed! Walk to move it, F to fire, G to release" }
                        else { "Released at current position" }
                    );
                } else {
                    println!("[Cannon] Too far away to grab (walk closer)");
                }
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.movement.sprint = pressed;
                self.movement.down = pressed;
            }

            KeyCode::ControlLeft | KeyCode::ControlRight => {
                self.builder_mode.ctrl_held = pressed;
            }

            KeyCode::KeyB if pressed => {
                self.builder_mode.toggle();
                scene.building.toolbar_mut().toggle();
                if let Some(window) = &self.window {
                    if scene.building.toolbar().visible {
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                    } else {
                        if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                        }
                        window.set_cursor_visible(false);
                    }
                }
            }

            KeyCode::Tab if pressed => {
                if scene.building.toolbar().visible {
                    scene.building.toolbar_mut().next_shape();
                }
            }

            KeyCode::Digit1 if pressed && scene.building.toolbar().visible => {
                scene.building.toolbar_mut().select_shape(0)
            }
            KeyCode::Digit2 if pressed && scene.building.toolbar().visible => {
                scene.building.toolbar_mut().select_shape(1)
            }
            KeyCode::Digit3 if pressed && scene.building.toolbar().visible => {
                scene.building.toolbar_mut().select_shape(2)
            }
            KeyCode::Digit4 if pressed && scene.building.toolbar().visible => {
                scene.building.toolbar_mut().select_shape(3)
            }
            KeyCode::Digit5 if pressed && scene.building.toolbar().visible => {
                scene.building.toolbar_mut().select_shape(4)
            }
            KeyCode::Digit6 if pressed && scene.building.toolbar().visible => {
                scene.building.toolbar_mut().select_shape(5)
            }
            KeyCode::Digit7 if pressed && scene.building.toolbar().visible => {
                scene.building.toolbar_mut().select_shape(6)
            }

            KeyCode::ArrowUp if pressed && scene.building.toolbar().visible => {
                scene.building.toolbar_mut().prev_shape()
            }
            KeyCode::ArrowDown if pressed && scene.building.toolbar().visible => {
                scene.building.toolbar_mut().next_shape()
            }

            KeyCode::F11 if pressed => {
                if let Some(window) = &self.window {
                    let current = window.fullscreen();
                    window.set_fullscreen(if current.is_some() {
                        None
                    } else {
                        Some(Fullscreen::Borderless(None))
                    });
                }
            }

            KeyCode::KeyT if pressed => {
                self.terrain_ui.toggle();
            }

            KeyCode::KeyV if pressed => {
                scene.first_person_mode = !scene.first_person_mode;
                if scene.first_person_mode {
                    scene.player.position =
                        self.camera.position - Vec3::new(0.0, PLAYER_EYE_HEIGHT, 0.0);
                }
            }

            KeyCode::F1 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 0.1,
                    mountains: 0.0,
                    rocks: 0.0,
                    hills: 0.2,
                    detail: 0.1,
                    water: 0.0,
                });
                scene.terrain_needs_rebuild = true;
            }
            KeyCode::F2 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 0.3,
                    mountains: 0.1,
                    rocks: 0.1,
                    hills: 0.4,
                    detail: 0.2,
                    water: 0.0,
                });
                scene.terrain_needs_rebuild = true;
            }
            KeyCode::F3 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 0.5,
                    mountains: 0.3,
                    rocks: 0.5,
                    hills: 0.3,
                    detail: 0.4,
                    water: 0.2,
                });
                scene.terrain_needs_rebuild = true;
            }
            KeyCode::F4 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 1.0,
                    mountains: 0.8,
                    rocks: 0.6,
                    hills: 0.3,
                    detail: 0.5,
                    water: 0.3,
                });
                scene.terrain_needs_rebuild = true;
            }

            KeyCode::Digit1 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(0)
            }
            KeyCode::Digit2 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(1)
            }
            KeyCode::Digit3 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(2)
            }
            KeyCode::Digit4 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(3)
            }
            KeyCode::Digit5 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(4)
            }
            KeyCode::Digit6 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(5)
            }
            KeyCode::Digit7 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(6)
            }
            KeyCode::Digit8 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(7)
            }

            KeyCode::KeyZ
                if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled =>
            {
                self.builder_mode.undo(&mut scene.hex_grid);
            }
            KeyCode::KeyY
                if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled =>
            {
                self.builder_mode.redo(&mut scene.hex_grid);
            }
            KeyCode::KeyC
                if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled =>
            {
                self.builder_mode.copy_area(&scene.hex_grid, 3);
            }
            KeyCode::KeyV
                if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled =>
            {
                self.builder_mode.paste(&mut scene.hex_grid);
            }
            KeyCode::KeyR if pressed && self.builder_mode.enabled => {
                self.builder_mode.rotate_selection();
            }

            KeyCode::KeyR if pressed && !self.builder_mode.enabled => self.camera.reset(),
            KeyCode::KeyC if pressed && !self.builder_mode.ctrl_held => {
                scene.clear_projectiles();
            }

            // Arrow keys: no longer used for cannon aiming (cannon now aims with camera)
            // Kept for builder toolbar navigation above
            KeyCode::ArrowUp | KeyCode::ArrowDown | KeyCode::ArrowLeft | KeyCode::ArrowRight => {}
            _ => {}
        }
    }
}

impl ApplicationHandler for BattleArenaApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title("Battle Sphere - Combat Arena [T: Terrain Editor]")
                .with_inner_size(PhysicalSize::new(1920, 1080));
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
                let scene = self.scene.as_mut().unwrap();
                let mouse_pos = self.current_mouse_pos.unwrap_or((0.0, 0.0));

                if self.start_overlay.visible && state == ElementState::Pressed {
                    self.start_overlay.visible = false;
                    if let Some(window) = &self.window {
                        if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                        }
                        window.set_cursor_visible(false);
                    }
                    return;
                }

                match button {
                    MouseButton::Left => {
                        let pressed = state == ElementState::Pressed;
                        self.left_mouse_pressed = pressed;

                        if pressed {
                            if self.terrain_ui.on_mouse_press(mouse_pos.0, mouse_pos.1) {
                                return;
                            }
                        } else if self.terrain_ui.on_mouse_release(mouse_pos.0, mouse_pos.1) {
                            scene.terrain_needs_rebuild = true;
                        }

                        if scene.building.toolbar().visible && pressed {
                            if scene.building.toolbar().is_bridge_mode() {
                                self.handle_bridge_click();
                            } else if !self.handle_block_click() {
                                self.place_building_block();
                            }
                        } else if self.builder_mode.enabled && pressed {
                            self.builder_mode.place_at_cursor(&mut scene.hex_grid);
                        }
                    }
                    MouseButton::Right => {
                        if self.builder_mode.enabled && state == ElementState::Pressed {
                            self.builder_mode.remove_at_cursor(&mut scene.hex_grid);
                        }
                        self.mouse_pressed = state == ElementState::Pressed;
                    }
                    MouseButton::Middle => {
                        if state == ElementState::Pressed && scene.building.toolbar().visible {
                            scene.building.toolbar_mut().next_material();
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let current = (position.x as f32, position.y as f32);
                self.current_mouse_pos = Some(current);
                if self.left_mouse_pressed && self.terrain_ui.visible {
                    self.terrain_ui.on_mouse_move(current.0, current.1);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scene = self.scene.as_mut().unwrap();
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                };

                if scene.building.toolbar().visible {
                    scene.building.toolbar_mut().adjust_height(scroll);
                    self.update_block_preview();
                } else if self.builder_mode.enabled {
                    let delta = if scroll > 0.0 { 1 } else { -1 };
                    self.builder_mode.adjust_level(delta);
                } else {
                    self.camera.position += self.camera.get_forward() * scroll * 5.0;
                }
            }
            WindowEvent::Resized(new_size) => {
                if let Some(ref mut gpu) = self.gpu {
                    gpu.resize(new_size);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let delta_time = now.duration_since(self.last_frame).as_secs_f32();
                self.last_frame = now;

                self.frame_count += 1;
                if now.duration_since(self.last_fps_update).as_secs_f32() >= 1.0 {
                    self.fps = self.frame_count as f32
                        / now.duration_since(self.last_fps_update).as_secs_f32();
                    self.frame_count = 0;
                    self.last_fps_update = now;

                    if let (Some(window), Some(scene)) = (&self.window, &self.scene) {
                        let mode_str = if self.builder_mode.enabled {
                            format!("BUILDER (Mat: {})", self.builder_mode.selected_material + 1)
                        } else {
                            "Combat".to_string()
                        };
                        window.set_title(&format!(
                            "Battle Sphere - {} | FPS: {:.0} | Prisms: {}",
                            mode_str,
                            self.fps,
                            scene.hex_grid.len()
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

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if self.start_overlay.visible {
            return;
        }
        let scene = self.scene.as_ref();
        if let DeviceEvent::MouseMotion { delta } = event {
            let toolbar_visible = scene.is_some_and(|s| s.building.toolbar().visible);
            if toolbar_visible {
                if self.mouse_pressed {
                    self.camera
                        .handle_mouse_look(delta.0 as f32, delta.1 as f32);
                }
            } else {
                self.camera
                    .handle_mouse_look(delta.0 as f32, delta.1 as f32);
            }
        }
    }
}

fn main() {
    println!("===========================================");
    println!("   Battle Sphere - Combat Arena");
    println!("===========================================");
    println!();
    println!("*** Click anywhere to start ***");
    println!();
    println!("Controls: WASD Move, Space Jump, V Toggle FPS/Free");
    println!("G: Grab/Release Cannon, F: Fire Cannon");
    println!("B: Builder, T: Terrain Editor, F11: Fullscreen, ESC: Exit");
    println!();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = BattleArenaApp::new();
    event_loop.run_app(&mut app).unwrap();
}
