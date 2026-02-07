//! Battle Arena - Combat Prototype
//!
//! Run with: `cargo run --bin battle_arena`
//!
//! Controls:
//! - WASD: Move (first-person or camera)
//! - Mouse right-drag: Look around (FPS style)
//! - Space: Jump (first-person mode) / Fire cannon (free camera)
//! - F: Fire cannon (aims where you look)
//! - X: Toggle weapon (cannonball / rocket launcher)
//! - G: Grab/release cannon (walk to reposition)
//! - Shift: Sprint when moving
//! - V: Toggle first-person / free camera mode
//! - R: Reset camera
//! - C: Clear all projectiles
//! - B: Toggle builder mode
//! - T: Terrain editor UI
//! - ESC: Exit
//!
//! Browser (wasm): build with `cargo build --bin battle_arena --target wasm32-unknown-unknown`,
//! then run `wasm-bindgen` and serve. Enables AI agents to test the game in the browser.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

use battle_tok_engine::render::hex_prism::DEFAULT_HEX_RADIUS;
use glam::{IVec3, Mat4, Vec3};
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
    FogPostConfig, FogPostPass, LavaSteamConfig, MaterialSystem, ParticleSystem, PointLightManager,
    SceneConfig,
};

// Import building block types for GPU operations
use battle_tok_engine::render::{BuildingBlock, MergedMesh};

// Import game module types
use battle_tok_engine::game::ProjectileKind;
use battle_tok_engine::game::config::{ArenaConfig, VisualConfig};
use battle_tok_engine::game::terrain::terrain_height_at_island;
use battle_tok_engine::game::{
    BLOCK_GRID_SIZE, BattleScene, BridgeConfig, BuilderMode, Camera, FloatingIslandConfig,
    LavaParams, Mesh, MovementKeys, PLAYER_EYE_HEIGHT, SHADER_SOURCE, SdfCannonData,
    SdfCannonUniforms, SelectedFace, StartOverlay, TerrainEditorUI, TerrainParams, Uniforms,
    Vertex, WeaponMode, generate_all_trees_mesh, generate_bridge, generate_floating_island,
    generate_lava_ocean, generate_trees_on_terrain, get_material_color, is_inside_hexagon,
    set_terrain_params,
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

/// GPU buffers for one cullable building chunk.
struct BlockChunkBuffers {
    center: Vec3,
    radius: f32,
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

const HDR_SCENE_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const BLOOM_MIP_COUNT: usize = 5;
/// Disabled by default: double-click SDF merge causes large frame-time spikes.
const ENABLE_SDF_DOUBLE_CLICK_MERGE: bool = false;
const BLOCK_RENDER_CHUNK_SIZE: i32 = 12;
const BLOCK_RENDER_MAX_DISTANCE: f32 = 260.0;
const BLOCK_REAR_CULL_DOT: f32 = -0.30;
const STRUCTURE_ROCK_TEXTURE_BYTES: &[u8] =
    include_bytes!("../../Assets/textures/rock_stone_tile.png");

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TaaParams {
    inv_curr_view_proj: [[f32; 4]; 4],
    prev_view_proj: [[f32; 4]; 4],
    inv_resolution: [f32; 2],
    jitter: [f32; 2],
    history_weight: f32,
    new_weight: f32,
    reject_threshold: f32,
    enabled: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BloomExtractParams {
    threshold: f32,
    knee: f32,
    _pad0: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BloomBlurParams {
    texel_size: [f32; 2],
    direction: [f32; 2],
    intensity: f32,
    mode: u32,
    _pad0: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TonemapCompositeParams {
    exposure: f32,
    saturation: f32,
    contrast: f32,
    bloom_intensity: f32,
}

struct BloomMip {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
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
    preview_pipeline: wgpu::RenderPipeline,
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
    dynamic_vertex_capacity: u64,
    dynamic_index_capacity: u64,

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
    _structure_texture: wgpu::Texture,
    _structure_texture_view: wgpu::TextureView,
    _structure_texture_sampler: wgpu::Sampler,

    // Building blocks (split into cullable chunks)
    block_chunk_buffers: Vec<BlockChunkBuffers>,

    // Merged mesh GPU buffers
    merged_mesh_buffers: Vec<MergedMeshBuffers>,

    // Trees
    tree_vertex_buffer: wgpu::Buffer,
    tree_index_buffer: wgpu::Buffer,
    tree_index_count: u32,

    // HDR scene/post textures
    scene_hdr_texture: wgpu::Texture,
    scene_hdr_view: wgpu::TextureView,
    post_temp_texture: wgpu::Texture,
    post_temp_view: wgpu::TextureView,
    taa_history_a_texture: wgpu::Texture,
    taa_history_a_view: wgpu::TextureView,
    taa_history_b_texture: wgpu::Texture,
    taa_history_b_view: wgpu::TextureView,
    bloom_mips: Vec<BloomMip>,
    bloom_ping_texture: wgpu::Texture,
    bloom_ping_view: wgpu::TextureView,

    // Lava ocean (rendered with animated lava.wgsl shader)
    lava_pipeline: wgpu::RenderPipeline,
    lava_bind_group: wgpu::BindGroup,
    lava_scene_uniform_buffer: wgpu::Buffer,
    lava_params_buffer: wgpu::Buffer,
    lava_vertex_buffer: wgpu::Buffer,
    lava_index_buffer: wgpu::Buffer,
    lava_index_count: u32,

    // Post process
    post_sampler: wgpu::Sampler,
    taa_pipeline: wgpu::RenderPipeline,
    taa_bind_group_layout: wgpu::BindGroupLayout,
    taa_params_buffer: wgpu::Buffer,
    bloom_extract_pipeline: wgpu::RenderPipeline,
    bloom_extract_bind_group_layout: wgpu::BindGroupLayout,
    bloom_extract_params_buffer: wgpu::Buffer,
    bloom_blur_pipeline: wgpu::RenderPipeline,
    bloom_blur_bind_group_layout: wgpu::BindGroupLayout,
    bloom_blur_params_buffer: wgpu::Buffer,
    tonemap_pipeline: wgpu::RenderPipeline,
    tonemap_bind_group_layout: wgpu::BindGroupLayout,
    tonemap_params_buffer: wgpu::Buffer,
}

impl GpuResources {
    fn create_color_target(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        format: wgpu::TextureFormat,
        label: &'static str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    fn create_bloom_mips(device: &wgpu::Device, width: u32, height: u32) -> Vec<BloomMip> {
        let mut mips = Vec::with_capacity(BLOOM_MIP_COUNT);
        let mut w = (width / 2).max(1);
        let mut h = (height / 2).max(1);
        for i in 0..BLOOM_MIP_COUNT {
            let label = format!("Bloom Mip {}", i);
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(&label),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_SCENE_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            mips.push(BloomMip {
                _texture: texture,
                view,
                width: w,
                height: h,
            });
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        mips
    }

    fn create_structure_rock_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
    ) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
        let decoded = image::load_from_memory(STRUCTURE_ROCK_TEXTURE_BYTES)
            .expect("Failed to decode embedded structure rock texture")
            .to_rgba8();
        let (width, height) = decoded.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Structure Rock Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            decoded.as_raw(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Structure Rock Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        (texture, view, sampler)
    }

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

        let (scene_hdr_texture, scene_hdr_view) = Self::create_color_target(
            &self.device,
            self.surface_config.width,
            self.surface_config.height,
            HDR_SCENE_FORMAT,
            "Scene HDR Texture",
        );
        self.scene_hdr_texture = scene_hdr_texture;
        self.scene_hdr_view = scene_hdr_view;

        let (post_temp_texture, post_temp_view) = Self::create_color_target(
            &self.device,
            self.surface_config.width,
            self.surface_config.height,
            HDR_SCENE_FORMAT,
            "Post Temp Texture",
        );
        self.post_temp_texture = post_temp_texture;
        self.post_temp_view = post_temp_view;

        let (taa_history_a_texture, taa_history_a_view) = Self::create_color_target(
            &self.device,
            self.surface_config.width,
            self.surface_config.height,
            HDR_SCENE_FORMAT,
            "TAA History A",
        );
        self.taa_history_a_texture = taa_history_a_texture;
        self.taa_history_a_view = taa_history_a_view;

        let (taa_history_b_texture, taa_history_b_view) = Self::create_color_target(
            &self.device,
            self.surface_config.width,
            self.surface_config.height,
            HDR_SCENE_FORMAT,
            "TAA History B",
        );
        self.taa_history_b_texture = taa_history_b_texture;
        self.taa_history_b_view = taa_history_b_view;

        self.bloom_mips = Self::create_bloom_mips(
            &self.device,
            self.surface_config.width,
            self.surface_config.height,
        );

        let ping_w = (self.surface_config.width / 2).max(1);
        let ping_h = (self.surface_config.height / 2).max(1);
        let (bloom_ping_texture, bloom_ping_view) =
            Self::create_color_target(&self.device, ping_w, ping_h, HDR_SCENE_FORMAT, "Bloom Ping");
        self.bloom_ping_texture = bloom_ping_texture;
        self.bloom_ping_view = bloom_ping_view;
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
    shift_drag_build_active: bool,
    shift_drag_start: Option<Vec3>,
    shift_drag_preview_positions: Vec<Vec3>,
    preview_update_accumulator: f32,

    // Windows focus overlay
    start_overlay: StartOverlay,

    // Terrain editor UI (on-screen sliders)
    terrain_ui: TerrainEditorUI,

    // Timing
    start_time: Instant,
    last_frame: Instant,
    frame_count: u64,
    render_frame_index: u64,
    fps: f32,
    last_fps_update: Instant,
    frame_time_ms: f32,
    draw_calls_estimate: u32,

    // PostFx frame state
    prev_view_proj: Mat4,
    current_view_proj: Mat4,
    current_jitter: [f32; 2],
    taa_history_use_a: bool,
    postfx_enabled: bool,
    taa_enabled: bool,
    bloom_enabled: bool,
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
            shift_drag_build_active: false,
            shift_drag_start: None,
            shift_drag_preview_positions: Vec::new(),
            preview_update_accumulator: 0.0,
            start_overlay: StartOverlay::default(),
            terrain_ui: TerrainEditorUI::default(),
            start_time: Instant::now(),
            last_frame: Instant::now(),
            frame_count: 0,
            render_frame_index: 0,
            fps: 0.0,
            last_fps_update: Instant::now(),
            frame_time_ms: 0.0,
            draw_calls_estimate: 0,
            prev_view_proj: Mat4::IDENTITY,
            current_view_proj: Mat4::IDENTITY,
            current_jitter: [0.0, 0.0],
            taa_history_use_a: true,
            postfx_enabled: true,
            taa_enabled: true,
            bloom_enabled: true,
        }
    }

    /// Async GPU request (adapter + device). Used on native with block_on, on wasm with spawn_local.
    async fn request_gpu_async(
        instance: wgpu::Instance,
        surface: wgpu::Surface<'static>,
    ) -> (
        wgpu::Instance,
        wgpu::Surface<'static>,
        wgpu::Adapter,
        wgpu::Device,
        wgpu::Queue,
    ) {
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("Battle Arena Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                ..Default::default()
            })
            .await
            .expect("Failed to create device");

        (instance, surface, adapter, device, queue)
    }

    fn initialize(&mut self, window: Arc<Window>) {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(Arc::clone(&window)).unwrap();
        // Safe: we keep the window in self.window and never drop it before surface
        let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface) };

        let (instance, surface, adapter, device, queue) =
            pollster::block_on(Self::request_gpu_async(instance, surface));

        self.initialize_from_gpu(window, instance, adapter, device, queue, surface);
    }

    /// Finishes initialization using an already-created GPU (used by native after block_on and by wasm after async init).
    fn initialize_from_gpu(
        &mut self,
        window: Arc<Window>,
        _instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
    ) {
        let size = window.inner_size();

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
        let (structure_texture, structure_texture_view, structure_texture_sampler) =
            GpuResources::create_structure_rock_texture(&device, &queue);

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
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
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&structure_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&structure_texture_sampler),
                },
            ],
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
                    format: HDR_SCENE_FORMAT,
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

        // Preview pipeline: alpha-blended hologram with depth test but no depth writes.
        let preview_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Preview Pipeline"),
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
                    format: HDR_SCENE_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -2,
                    slope_scale: -1.0,
                    clamp: 0.0,
                },
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
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: ui_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&structure_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&structure_texture_sampler),
                },
            ],
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
        let scene_hdr_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scene HDR Texture"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_SCENE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let scene_hdr_view = scene_hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let post_temp_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Post Temp Texture"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_SCENE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let post_temp_view = post_temp_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let taa_history_a_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA History A"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_SCENE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let taa_history_a_view =
            taa_history_a_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let taa_history_b_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("TAA History B"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: HDR_SCENE_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let taa_history_b_view =
            taa_history_b_texture.create_view(&wgpu::TextureViewDescriptor::default());

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
                    format: HDR_SCENE_FORMAT,
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
        let lava_shader_source =
            std::fs::read_to_string("shaders/lava.wgsl").expect("Failed to load lava.wgsl shader");
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

        let lava_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
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
                    format: HDR_SCENE_FORMAT,
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
        self.postfx_enabled = scene.visuals.postfx.debug_toggles.postfx_enabled;
        self.taa_enabled = scene.visuals.postfx.debug_toggles.taa_enabled;
        self.bloom_enabled = scene.visuals.postfx.debug_toggles.bloom_enabled;

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
        let dynamic_vertex_capacity = 1024 * 1024;
        let dynamic_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Vertex Buffer"),
            size: dynamic_vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dynamic_index_capacity = 256 * 1024;
        let dynamic_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Index Buffer"),
            size: dynamic_index_capacity,
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
            HDR_SCENE_FORMAT,
            "Assets/Skybox/sky_26_2k/sky_26_cubemap_2k", // day sky
            "Assets/Skybox/sky_16_2k/sky_16_cubemap_2k", // night sky
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

        let mut particle_system = ParticleSystem::new(&device, HDR_SCENE_FORMAT);
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

        let mut fog_post = FogPostPass::with_config(
            &device,
            &queue,
            HDR_SCENE_FORMAT,
            FogPostConfig::battle_arena(),
        );
        let haze_cfg = &scene.visuals.postfx.haze;
        fog_post.set_config(FogPostConfig {
            fog_color: scene.visuals.fog_color,
            density: if haze_cfg.enabled {
                haze_cfg.density
            } else {
                0.0
            },
            height_fog_start: haze_cfg.height_fog_start,
            height_fog_density: haze_cfg.height_fog_density,
            max_opacity: haze_cfg.max_opacity,
            horizon_boost: haze_cfg.horizon_boost,
        });

        // Configure lava steam boundary wall around islands
        fog_post.set_steam_config(LavaSteamConfig::battle_arena(
            attacker_center,
            defender_center,
            island_radius,
            lava_ocean_level,
        ));

        let post_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Post Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let bloom_mips =
            GpuResources::create_bloom_mips(&device, size.width.max(1), size.height.max(1));
        let (bloom_ping_texture, bloom_ping_view) = GpuResources::create_color_target(
            &device,
            (size.width.max(1) / 2).max(1),
            (size.height.max(1) / 2).max(1),
            HDR_SCENE_FORMAT,
            "Bloom Ping",
        );

        let taa_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("TAA Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/taa.wgsl").into()),
        });
        let bloom_extract_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Extract Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/bloom_extract.wgsl").into(),
            ),
        });
        let bloom_blur_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Bloom Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../shaders/bloom_blur.wgsl").into()),
        });
        let tonemap_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tonemap Composite Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/tonemap_composite.wgsl").into(),
            ),
        });

        let taa_params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("TAA Params Buffer"),
            size: std::mem::size_of::<TaaParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let taa_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("TAA Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
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
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let taa_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("TAA Pipeline Layout"),
            bind_group_layouts: &[&taa_bind_group_layout],
            push_constant_ranges: &[],
        });
        let taa_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("TAA Pipeline"),
            layout: Some(&taa_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &taa_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &taa_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_SCENE_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let bloom_extract_params_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Bloom Extract Params"),
                contents: bytemuck::bytes_of(&BloomExtractParams {
                    threshold: scene.visuals.postfx.bloom.threshold,
                    knee: scene.visuals.postfx.bloom.knee,
                    _pad0: [0.0; 2],
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bloom_extract_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Bloom Extract BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
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
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let bloom_extract_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Bloom Extract Pipeline Layout"),
                bind_group_layouts: &[&bloom_extract_bind_group_layout],
                push_constant_ranges: &[],
            });
        let bloom_extract_pipeline =
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Bloom Extract Pipeline"),
                layout: Some(&bloom_extract_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &bloom_extract_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &bloom_extract_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: HDR_SCENE_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let bloom_blur_params_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Bloom Blur Params"),
                contents: bytemuck::bytes_of(&BloomBlurParams {
                    texel_size: [
                        1.0 / size.width.max(1) as f32,
                        1.0 / size.height.max(1) as f32,
                    ],
                    direction: [1.0, 0.0],
                    intensity: 1.0,
                    mode: 1,
                    _pad0: [0.0; 2],
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bloom_blur_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Bloom Blur BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
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
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let bloom_blur_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Bloom Blur Pipeline Layout"),
                bind_group_layouts: &[&bloom_blur_bind_group_layout],
                push_constant_ranges: &[],
            });
        let bloom_blur_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Bloom Blur Pipeline"),
            layout: Some(&bloom_blur_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &bloom_blur_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &bloom_blur_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_SCENE_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let tonemap_params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tonemap Params"),
            contents: bytemuck::bytes_of(&TonemapCompositeParams {
                exposure: scene.visuals.postfx.tonemap.exposure,
                saturation: scene.visuals.postfx.tonemap.saturation,
                contrast: scene.visuals.postfx.tonemap.contrast,
                bloom_intensity: scene.visuals.postfx.bloom.intensity,
            }),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let tonemap_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Tonemap Composite BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
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
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let tonemap_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Tonemap Composite Pipeline Layout"),
                bind_group_layouts: &[&tonemap_bind_group_layout],
                push_constant_ranges: &[],
            });
        let tonemap_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Tonemap Composite Pipeline"),
            layout: Some(&tonemap_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &tonemap_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &tonemap_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Store everything
        self.window = Some(window);
        self.scene = Some(scene);
        self.gpu = Some(GpuResources {
            device,
            queue,
            surface,
            surface_config,
            pipeline,
            preview_pipeline,
            sdf_cannon_pipeline,
            ui_pipeline,
            uniform_buffer,
            uniform_bind_group,
            static_vertex_buffer,
            static_index_buffer,
            static_index_count: static_mesh.indices.len() as u32,
            dynamic_vertex_buffer,
            dynamic_index_buffer,
            dynamic_vertex_capacity,
            dynamic_index_capacity,
            hex_wall_vertex_buffer,
            hex_wall_index_buffer,
            hex_wall_index_count,
            sdf_cannon_uniform_buffer,
            sdf_cannon_data_buffer,
            sdf_cannon_bind_group,
            depth_texture: depth_texture_view,
            depth_texture_raw: depth_texture,
            scene_hdr_texture,
            scene_hdr_view,
            post_temp_texture,
            post_temp_view,
            taa_history_a_texture,
            taa_history_a_view,
            taa_history_b_texture,
            taa_history_b_view,
            bloom_mips,
            bloom_ping_texture,
            bloom_ping_view,
            ui_uniform_buffer,
            ui_bind_group,
            _structure_texture: structure_texture,
            _structure_texture_view: structure_texture_view,
            _structure_texture_sampler: structure_texture_sampler,
            block_chunk_buffers: Vec::new(),
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
            post_sampler,
            taa_pipeline,
            taa_bind_group_layout,
            taa_params_buffer,
            bloom_extract_pipeline,
            bloom_extract_bind_group_layout,
            bloom_extract_params_buffer,
            bloom_blur_pipeline,
            bloom_blur_bind_group_layout,
            bloom_blur_params_buffer,
            tonemap_pipeline,
            tonemap_bind_group_layout,
            tonemap_params_buffer,
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
        let projectile_trails: Vec<(Vec3, Vec3, ProjectileKind)> = self
            .scene
            .as_ref()
            .unwrap()
            .projectiles
            .iter_with_kind()
            .map(|(proj, kind)| (proj.position, proj.velocity, kind))
            .collect();
        let explosion_events = self.scene.as_mut().unwrap().drain_explosion_events();
        if let Some(ref mut particle_system) = self.particle_system {
            for (proj_pos, proj_vel, kind) in projectile_trails {
                if kind == ProjectileKind::Rocket {
                    Self::spawn_rocket_trail_embers(particle_system, proj_pos, proj_vel);
                } else {
                    Self::spawn_cannonball_trail_embers(particle_system, proj_pos, proj_vel);
                }
            }
            for event in explosion_events {
                Self::spawn_explosion_embers(particle_system, event.position, event.ember_count);
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
        if self
            .scene
            .as_ref()
            .unwrap()
            .building
            .block_manager
            .needs_mesh_update()
        {
            self.regenerate_block_mesh();
        }

        // Update builder cursor and block preview
        self.update_builder_cursor();
        let toolbar_visible = self
            .scene
            .as_ref()
            .is_some_and(|s| s.building.toolbar().visible);
        if !toolbar_visible {
            self.preview_update_accumulator = 0.0;
            self.update_block_preview();
        } else if self.shift_drag_build_active || self.left_mouse_pressed {
            // Keep drag placement fully responsive.
            self.preview_update_accumulator = 0.0;
            self.update_block_preview();
        } else {
            // Throttle expensive placement raycasts while idle in build mode.
            self.preview_update_accumulator += delta_time;
            if self.preview_update_accumulator >= (1.0 / 30.0) {
                self.preview_update_accumulator = 0.0;
                self.update_block_preview();
            }
        }
        self.update_shift_drag_build_preview();

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
                            scene.building.remove_block(block_id);
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
                    }
                    scene.building.remove_block(*block_id);
                }
            }
            needs_update
        };
        if needs_mesh_update {
            self.regenerate_block_mesh();
        }
    }

    fn spawn_explosion_embers(particle_system: &mut ParticleSystem, center: Vec3, count: usize) {
        let count = count.clamp(8, 128);
        let inv_count = 1.0 / count as f32;

        for i in 0..count {
            let t = i as f32 * inv_count;
            let angle = t * std::f32::consts::TAU + (i as f32 * 0.377).sin();
            let ring = ((i as f32 * 0.618_034).fract() * 1.8) + 0.2;
            let y = center.y + ((i as f32 * 0.414).fract() * 1.2) + 0.1;
            let spawn = [
                center.x + angle.cos() * ring,
                y,
                center.z + angle.sin() * ring,
            ];
            particle_system.spawn_ember(spawn);
        }
    }

    fn spawn_rocket_trail_embers(
        particle_system: &mut ParticleSystem,
        position: Vec3,
        velocity: Vec3,
    ) {
        let back = (-velocity).normalize_or_zero();
        let back = if back.length_squared() > 1e-6 {
            back
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let center = position + back * 0.32;
        let seed = position.x * 12.9898 + position.y * 37.719 + position.z * 78.233;

        for i in 0..2 {
            let t = i as f32 * 0.73 + seed;
            let angle = t * std::f32::consts::TAU;
            let radius = 0.05 + (t * 0.618).fract() * 0.06;
            let spawn = [
                center.x + angle.cos() * radius,
                center.y + 0.03 + (t * 0.414).fract() * 0.08,
                center.z + angle.sin() * radius,
            ];
            particle_system.spawn_ember(spawn);
        }
    }

    fn spawn_cannonball_trail_embers(
        particle_system: &mut ParticleSystem,
        position: Vec3,
        velocity: Vec3,
    ) {
        let back = (-velocity).normalize_or_zero();
        let back = if back.length_squared() > 1e-6 {
            back
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };
        let center = position + back * 0.22;
        let seed = position.x * 19.173 + position.y * 7.341 + position.z * 31.289;
        let angle = (seed * 2.319).fract() * std::f32::consts::TAU;
        let radius = 0.05 + (seed * 0.618).fract() * 0.05;
        let spawn = [
            center.x + angle.cos() * radius,
            center.y + 0.03 + (seed * 0.414).fract() * 0.05,
            center.z + angle.sin() * radius,
        ];
        particle_system.spawn_ember(spawn);
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
        gpu.lava_vertex_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Lava Vertex Buffer"),
                contents: bytemuck::cast_slice(&lava_ocean.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        gpu.lava_index_buffer = gpu
            .device
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

    /// Sample build-ground height on the active islands.
    ///
    /// Returns `None` outside buildable island surfaces.
    fn sample_build_ground_height(&self, x: f32, z: f32) -> Option<f32> {
        let scene = self.scene.as_ref()?;

        let attacker = &scene.config.island_attacker;
        let defender = &scene.config.island_defender;

        let in_attacker = is_inside_hexagon(
            x - attacker.position.x,
            z - attacker.position.z,
            attacker.radius,
        );
        let in_defender = is_inside_hexagon(
            x - defender.position.x,
            z - defender.position.z,
            defender.radius,
        );

        if !in_attacker && !in_defender {
            return None;
        }

        let use_attacker = if in_attacker && in_defender {
            let da2 = (x - attacker.position.x).powi(2) + (z - attacker.position.z).powi(2);
            let dd2 = (x - defender.position.x).powi(2) + (z - defender.position.z).powi(2);
            da2 <= dd2
        } else {
            in_attacker
        };

        let island = if use_attacker { attacker } else { defender };
        let base_y = island.position.y + island.surface_height;

        Some(terrain_height_at_island(
            x,
            z,
            base_y,
            island.position.x,
            island.position.z,
            island.radius,
        ))
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

        let max_dist = 200.0;
        let step_size = 0.5;
        let mut t = 0.0;
        while t < max_dist {
            let point = ray_origin + ray_dir * t;
            if let Some(terrain_y) = self.sample_build_ground_height(point.x, point.z) {
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
            .calculate_placement(ray_origin, ray_dir, &|x, z| {
                self.sample_build_ground_height(x, z)
            })
    }

    /// Place a building block at the preview position
    fn place_building_block(&mut self) {
        let quick_mode = self
            .scene
            .as_ref()
            .is_some_and(|s| s.building.toolbar().quick_mode);

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

        let placed = if quick_mode {
            self.place_structure_at_anchor(position)
        } else {
            let ground_hint = self.sample_build_ground_height(position.x, position.z);
            if self
                .scene
                .as_mut()
                .unwrap()
                .building
                .place_block_with_ground_hint(position, ground_hint)
                .is_some()
            {
                1
            } else {
                0
            }
        };

        if placed > 0 {
            self.regenerate_block_mesh();
        }
    }

    fn start_shift_drag_build(&mut self) {
        let Some(start_pos) = self.calculate_block_placement_position() else {
            return;
        };
        self.shift_drag_build_active = true;
        self.shift_drag_start = Some(start_pos);
        self.shift_drag_preview_positions = vec![start_pos];
    }

    fn update_shift_drag_build_preview(&mut self) {
        if !self.shift_drag_build_active || !self.left_mouse_pressed {
            return;
        }
        let Some(start) = self.shift_drag_start else {
            return;
        };
        let Some(end) = self.calculate_block_placement_position() else {
            return;
        };
        self.shift_drag_preview_positions = Self::line_build_positions(start, end);
    }

    fn commit_shift_drag_build(&mut self) {
        if !self.shift_drag_build_active {
            return;
        }

        let positions = std::mem::take(&mut self.shift_drag_preview_positions);
        self.shift_drag_build_active = false;
        self.shift_drag_start = None;

        if positions.is_empty() {
            return;
        }

        let placements: Vec<(Vec3, Option<f32>)> = positions
            .into_iter()
            .map(|pos| (pos, self.sample_build_ground_height(pos.x, pos.z)))
            .collect();

        let mut placed_count = 0u32;
        let quick_mode = self
            .scene
            .as_ref()
            .is_some_and(|s| s.building.toolbar().quick_mode);
        if quick_mode {
            let mut all_instances = Vec::new();
            for (anchor, _) in placements {
                all_instances.extend(self.structure_instances_at_anchor(anchor));
            }
            placed_count += self.place_structure_instances(all_instances);
        } else {
            let scene = self.scene.as_mut().unwrap();
            for (position, ground_hint) in placements {
                if scene
                    .building
                    .place_block_with_ground_hint(position, ground_hint)
                    .is_some()
                {
                    placed_count += 1;
                }
            }
        }

        if placed_count > 0 {
            self.regenerate_block_mesh();
        }
    }

    fn structure_instances_at_anchor(
        &self,
        anchor: Vec3,
    ) -> Vec<(Vec3, battle_tok_engine::render::BuildingBlockShape)> {
        let Some(scene) = self.scene.as_ref() else {
            return Vec::new();
        };
        let layout = scene.building.toolbar().selected_structure_layout();
        layout
            .into_iter()
            .map(|(offset, shape)| {
                (
                    Vec3::new(
                        anchor.x + offset.x as f32 * BLOCK_GRID_SIZE,
                        anchor.y + offset.y as f32 * BLOCK_GRID_SIZE,
                        anchor.z + offset.z as f32 * BLOCK_GRID_SIZE,
                    ),
                    shape,
                )
            })
            .collect()
    }

    fn place_structure_at_anchor(&mut self, anchor: Vec3) -> u32 {
        self.place_structure_instances(self.structure_instances_at_anchor(anchor))
    }

    fn world_to_build_cell(position: Vec3) -> IVec3 {
        IVec3::new(
            (position.x / BLOCK_GRID_SIZE).round() as i32,
            (position.y / BLOCK_GRID_SIZE).round() as i32,
            (position.z / BLOCK_GRID_SIZE).round() as i32,
        )
    }

    fn place_structure_instances(
        &mut self,
        mut instances: Vec<(Vec3, battle_tok_engine::render::BuildingBlockShape)>,
    ) -> u32 {
        if instances.is_empty() {
            return 0;
        }

        // Deduplicate by snapped build cell before placement attempts.
        let mut seen_cells = HashSet::<IVec3>::new();
        instances.retain(|(position, _)| seen_cells.insert(Self::world_to_build_cell(*position)));
        instances.sort_by(|a, b| {
            a.0.y
                .total_cmp(&b.0.y)
                .then_with(|| a.0.x.total_cmp(&b.0.x))
                .then_with(|| a.0.z.total_cmp(&b.0.z))
        });
        let placements: Vec<(
            Vec3,
            battle_tok_engine::render::BuildingBlockShape,
            IVec3,
            Option<f32>,
        )> = instances
            .into_iter()
            .map(|(position, shape)| {
                (
                    position,
                    shape,
                    Self::world_to_build_cell(position),
                    self.sample_build_ground_height(position.x, position.z),
                )
            })
            .collect();

        let material = self
            .scene
            .as_ref()
            .map(|s| s.building.toolbar().selected_material)
            .unwrap_or(0);

        let mut placed = 0u32;
        let mut skipped_occupied = 0u32;
        let mut blocked_other = 0u32;

        if let Some(scene) = self.scene.as_mut() {
            for (position, shape, cell, ground_hint) in placements {
                if scene.building.statics_v2.is_occupied(cell) {
                    skipped_occupied += 1;
                    continue;
                }

                if scene
                    .building
                    .place_block_shape_with_ground_hint(shape, position, material, ground_hint)
                    .is_some()
                {
                    placed += 1;
                } else {
                    blocked_other += 1;
                }
            }
        }

        if skipped_occupied > 0 || blocked_other > 0 {
            println!(
                "[Build] Template placement: +{} | skipped occupied {} | blocked {}",
                placed, skipped_occupied, blocked_other
            );
        }

        placed
    }

    fn line_build_positions(start: Vec3, end: Vec3) -> Vec<Vec3> {
        let sx = (start.x / BLOCK_GRID_SIZE).round() as i32;
        let sy = (start.y / BLOCK_GRID_SIZE).round() as i32;
        let sz = (start.z / BLOCK_GRID_SIZE).round() as i32;

        let ex = (end.x / BLOCK_GRID_SIZE).round() as i32;
        let ey = (end.y / BLOCK_GRID_SIZE).round() as i32;
        let ez = (end.z / BLOCK_GRID_SIZE).round() as i32;

        let dx = ex - sx;
        let dy = ey - sy;
        let dz = ez - sz;
        let steps = dx.abs().max(dy.abs()).max(dz.abs()).max(1);

        let mut cells = Vec::with_capacity((steps + 1) as usize);
        let mut seen = HashSet::new();
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let gx = (sx as f32 + dx as f32 * t).round() as i32;
            let gy = (sy as f32 + dy as f32 * t).round() as i32;
            let gz = (sz as f32 + dz as f32 * t).round() as i32;
            if seen.insert((gx, gy, gz)) {
                cells.push(Vec3::new(
                    gx as f32 * BLOCK_GRID_SIZE,
                    gy as f32 * BLOCK_GRID_SIZE,
                    gz as f32 * BLOCK_GRID_SIZE,
                ));
            }
        }
        cells
    }

    fn generate_block_preview_mesh(
        &self,
        instances: &[(Vec3, battle_tok_engine::render::BuildingBlockShape)],
        pulse_alpha: f32,
        color: [f32; 3],
    ) -> Option<(Vec<Vertex>, Vec<u32>)> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let preview_color = [color[0], color[1], color[2], pulse_alpha];

        for (pos, shape) in instances {
            let block = BuildingBlock::new(*shape, *pos, 0);
            let (block_verts, block_indices) = block.generate_mesh();
            let base = vertices.len() as u32;
            vertices.extend(block_verts.iter().map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: preview_color,
            }));
            indices.extend(block_indices.iter().map(|i| i + base));
        }

        if vertices.is_empty() {
            None
        } else {
            Some((vertices, indices))
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
            let material = scene.building.toolbar().selected_material;
            let block = BuildingBlock::new(shape, pos, material);
            let block_id = scene.building.block_manager.add_block(block);
            scene
                .building
                .block_physics
                .register_grounded_block(block_id);
            scene
                .building
                .register_external_grounded_block(block_id, pos, material);
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
        fn is_voxel_cube_for_chunking(block: &battle_tok_engine::render::BuildingBlock) -> bool {
            let battle_tok_engine::render::BuildingBlockShape::Cube { half_extents } = block.shape
            else {
                return false;
            };
            let is_unit = (half_extents.x - 0.5).abs() <= 1e-4
                && (half_extents.y - 0.5).abs() <= 1e-4
                && (half_extents.z - 0.5).abs() <= 1e-4;
            if !is_unit {
                return false;
            }
            block.rotation.dot(glam::Quat::IDENTITY).abs() >= 0.9999
        }

        fn block_material_color(material: u8) -> [f32; 4] {
            match material {
                0 => [0.6, 0.6, 0.6, 1.0],
                1 => [0.7, 0.5, 0.3, 1.0],
                2 => [0.4, 0.4, 0.45, 1.0],
                3 => [0.8, 0.7, 0.5, 1.0],
                4 => [0.3, 0.3, 0.35, 1.0],
                5 => [0.6, 0.3, 0.2, 1.0],
                6 => [0.2, 0.4, 0.2, 1.0],
                7 => [0.5, 0.5, 0.6, 1.0],
                8 => [0.9, 0.9, 0.85, 1.0],
                9 => [0.2, 0.2, 0.3, 1.0],
                _ => [0.5, 0.5, 0.5, 1.0],
            }
        }

        let gpu = match &mut self.gpu {
            Some(g) => g,
            None => return,
        };
        let scene = self.scene.as_ref().unwrap();
        let blocks = scene.building.block_manager.blocks();
        if blocks.is_empty() {
            gpu.block_chunk_buffers.clear();
            if let Some(scene_mut) = self.scene.as_mut() {
                scene_mut.building.block_manager.clear_dirty();
            }
            return;
        }

        #[derive(Default)]
        struct ChunkBuildData {
            vertices: Vec<Vertex>,
            indices: Vec<u32>,
            min: Vec3,
            max: Vec3,
            initialized: bool,
        }

        let mut chunks: HashMap<(i32, i32, i32), ChunkBuildData> = HashMap::new();
        let chunk_size = BLOCK_RENDER_CHUNK_SIZE as f32;

        let mut voxel_cubes: HashMap<(i32, i32, i32), (Vec3, u8)> = HashMap::new();
        let mut fallback_blocks: Vec<&battle_tok_engine::render::BuildingBlock> = Vec::new();

        for block in blocks {
            if is_voxel_cube_for_chunking(block) {
                let cell = (
                    block.position.x.round() as i32,
                    block.position.y.round() as i32,
                    block.position.z.round() as i32,
                );
                voxel_cubes
                    .entry(cell)
                    .or_insert((block.position, block.material));
            } else {
                fallback_blocks.push(block);
            }
        }

        if !voxel_cubes.is_empty() {
            let faces: [([i32; 3], [f32; 3], [[f32; 3]; 4]); 6] = [
                (
                    [1, 0, 0],
                    [1.0, 0.0, 0.0],
                    [
                        [1.0, -1.0, -1.0],
                        [1.0, 1.0, -1.0],
                        [1.0, 1.0, 1.0],
                        [1.0, -1.0, 1.0],
                    ],
                ),
                (
                    [-1, 0, 0],
                    [-1.0, 0.0, 0.0],
                    [
                        [-1.0, -1.0, 1.0],
                        [-1.0, 1.0, 1.0],
                        [-1.0, 1.0, -1.0],
                        [-1.0, -1.0, -1.0],
                    ],
                ),
                (
                    [0, 1, 0],
                    [0.0, 1.0, 0.0],
                    [
                        [-1.0, 1.0, -1.0],
                        [-1.0, 1.0, 1.0],
                        [1.0, 1.0, 1.0],
                        [1.0, 1.0, -1.0],
                    ],
                ),
                (
                    [0, -1, 0],
                    [0.0, -1.0, 0.0],
                    [
                        [-1.0, -1.0, 1.0],
                        [-1.0, -1.0, -1.0],
                        [1.0, -1.0, -1.0],
                        [1.0, -1.0, 1.0],
                    ],
                ),
                (
                    [0, 0, 1],
                    [0.0, 0.0, 1.0],
                    [
                        [-1.0, -1.0, 1.0],
                        [1.0, -1.0, 1.0],
                        [1.0, 1.0, 1.0],
                        [-1.0, 1.0, 1.0],
                    ],
                ),
                (
                    [0, 0, -1],
                    [0.0, 0.0, -1.0],
                    [
                        [1.0, -1.0, -1.0],
                        [-1.0, -1.0, -1.0],
                        [-1.0, 1.0, -1.0],
                        [1.0, 1.0, -1.0],
                    ],
                ),
            ];

            for (&cell, &(position, material)) in &voxel_cubes {
                let key = (
                    (position.x / chunk_size).floor() as i32,
                    (position.y / chunk_size).floor() as i32,
                    (position.z / chunk_size).floor() as i32,
                );
                let chunk = chunks.entry(key).or_default();

                let cube_min = position - Vec3::splat(0.5);
                let cube_max = position + Vec3::splat(0.5);
                if chunk.initialized {
                    chunk.min = chunk.min.min(cube_min);
                    chunk.max = chunk.max.max(cube_max);
                } else {
                    chunk.min = cube_min;
                    chunk.max = cube_max;
                    chunk.initialized = true;
                }

                let color = block_material_color(material);
                for (offset, normal_arr, corners) in &faces {
                    let neighbor = (cell.0 + offset[0], cell.1 + offset[1], cell.2 + offset[2]);
                    if voxel_cubes.contains_key(&neighbor) {
                        continue;
                    }

                    let base = chunk.vertices.len() as u32;
                    let normal = [normal_arr[0], normal_arr[1], normal_arr[2]];
                    for corner in corners {
                        let local = Vec3::new(corner[0], corner[1], corner[2]) * 0.5;
                        let world = position + local;
                        chunk.vertices.push(Vertex {
                            position: [world.x, world.y, world.z],
                            normal,
                            color,
                        });
                    }
                    chunk.indices.extend_from_slice(&[
                        base,
                        base + 1,
                        base + 2,
                        base,
                        base + 2,
                        base + 3,
                    ]);
                }
            }
        }

        for block in fallback_blocks {
            let key = (
                (block.position.x / chunk_size).floor() as i32,
                (block.position.y / chunk_size).floor() as i32,
                (block.position.z / chunk_size).floor() as i32,
            );
            let chunk = chunks.entry(key).or_default();

            let aabb = block.aabb();
            if chunk.initialized {
                chunk.min = chunk.min.min(aabb.min);
                chunk.max = chunk.max.max(aabb.max);
            } else {
                chunk.min = aabb.min;
                chunk.max = aabb.max;
                chunk.initialized = true;
            }

            let (block_vertices, block_indices) = block.generate_mesh();
            let base = chunk.vertices.len() as u32;
            chunk.vertices.extend(block_vertices.iter().map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            }));
            chunk.indices.extend(block_indices.iter().map(|i| i + base));
        }

        let mut sorted_chunks: Vec<((i32, i32, i32), ChunkBuildData)> =
            chunks.into_iter().collect();
        sorted_chunks.sort_by_key(|(key, _)| *key);

        let mut new_buffers = Vec::with_capacity(sorted_chunks.len());
        for (_key, chunk) in sorted_chunks {
            if chunk.indices.is_empty() {
                continue;
            }

            let vertex_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Block Chunk Vertex Buffer"),
                    contents: bytemuck::cast_slice(&chunk.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });
            let index_buffer = gpu
                .device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Block Chunk Index Buffer"),
                    contents: bytemuck::cast_slice(&chunk.indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

            let center = (chunk.min + chunk.max) * 0.5;
            let radius = (chunk.max - center).length().max(1.0);
            new_buffers.push(BlockChunkBuffers {
                center,
                radius,
                vertex_buffer,
                index_buffer,
                index_count: chunk.indices.len() as u32,
            });
        }

        gpu.block_chunk_buffers = new_buffers;
        if let Some(scene_mut) = self.scene.as_mut() {
            scene_mut.building.block_manager.clear_dirty();
        }
    }

    /// Generate ghost preview mesh for builder mode
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
                    let y1 = self
                        .sample_build_ground_height(x1, z1)
                        .map(|y| y + 0.1)
                        .unwrap_or(world_pos.y + 0.1);
                    let y2 = self
                        .sample_build_ground_height(x2, z2)
                        .map(|y| y + 0.1)
                        .unwrap_or(world_pos.y + 0.1);

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

        let scene = self.scene.as_ref().unwrap();
        let gpu = self.gpu.as_ref().unwrap();
        let mut hdr_source_view = &gpu.scene_hdr_view;
        let mut draw_calls = 5u32; // sky, meshes, lava, sdf, particles

        // 1) Base scene -> HDR target
        self.render_sky(&mut encoder, &gpu.scene_hdr_view);
        self.render_meshes(&mut encoder, &gpu.scene_hdr_view, dynamic_index_count);
        self.render_lava(&mut encoder, &gpu.scene_hdr_view);
        self.render_sdf_cannon(&mut encoder, &gpu.scene_hdr_view);
        self.render_particles(&mut encoder, &gpu.scene_hdr_view);

        // 2) Haze post (HDR -> HDR temp)
        if self.postfx_enabled && scene.visuals.postfx.haze.enabled {
            if let Some(ref fog_post) = self.fog_post {
                fog_post.render_to_view(
                    &gpu.device,
                    &mut encoder,
                    &gpu.scene_hdr_view,
                    &gpu.depth_texture,
                    &gpu.post_temp_view,
                );
                hdr_source_view = &gpu.post_temp_view;
                draw_calls += 1;
            }
        }

        // 3) TAA (HDR source -> history ping-pong)
        if self.postfx_enabled && self.taa_enabled && scene.visuals.postfx.taa.enabled {
            let (history_read, history_write) = if self.taa_history_use_a {
                (&gpu.taa_history_a_view, &gpu.taa_history_b_view)
            } else {
                (&gpu.taa_history_b_view, &gpu.taa_history_a_view)
            };
            self.render_taa(
                &gpu.device,
                &mut encoder,
                hdr_source_view,
                history_read,
                history_write,
            );
            hdr_source_view = history_write;
            self.taa_history_use_a = !self.taa_history_use_a;
            draw_calls += 1;
        }

        // 4) Bloom chain
        if self.postfx_enabled && self.bloom_enabled && scene.visuals.postfx.bloom.enabled {
            self.render_bloom_chain(&gpu.device, &gpu.queue, &mut encoder, hdr_source_view);
            draw_calls += BLOOM_MIP_COUNT as u32;
        } else {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &gpu.bloom_mips[0].view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
        }

        // 5) Tonemap+composite to swapchain
        let bloom_view = &gpu.bloom_mips[0].view;
        self.render_tonemap_composite(
            &gpu.device,
            &mut encoder,
            hdr_source_view,
            bloom_view,
            &view,
        );
        draw_calls += 1;

        // 6) UI on top
        self.render_ui(&mut encoder, &view);
        draw_calls += 1;

        self.gpu
            .as_ref()
            .unwrap()
            .queue
            .submit(std::iter::once(encoder.finish()));
        output.present();

        self.prev_view_proj = self.current_view_proj;
        self.render_frame_index = self.render_frame_index.wrapping_add(1);
        self.draw_calls_estimate = draw_calls;
    }

    fn halton(mut index: u32, base: u32) -> f32 {
        let mut result = 0.0;
        let mut f = 1.0;
        while index > 0 {
            f /= base as f32;
            result += f * (index % base) as f32;
            index /= base;
        }
        result
    }

    fn compute_taa_jitter(&self, width: u32, height: u32) -> [f32; 2] {
        let idx = (self.render_frame_index % 8) as u32 + 1;
        let hx = Self::halton(idx, 2) - 0.5;
        let hy = Self::halton(idx, 3) - 0.5;
        let jx = (hx * 2.0) / width.max(1) as f32;
        let jy = (hy * 2.0) / height.max(1) as f32;
        [jx, jy]
    }

    fn render_taa(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        current_view: &wgpu::TextureView,
        history_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
    ) {
        let gpu = self.gpu.as_ref().unwrap();
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("TAA Bind Group"),
            layout: &gpu.taa_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu.taa_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(current_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(history_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&gpu.depth_texture),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&gpu.post_sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("TAA Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&gpu.taa_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn render_bloom_chain(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        source_view: &wgpu::TextureView,
    ) {
        let gpu = self.gpu.as_ref().unwrap();
        if gpu.bloom_mips.is_empty() {
            return;
        }

        // Extract bright areas -> mip0
        let extract_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Extract BG"),
            layout: &gpu.bloom_extract_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu.bloom_extract_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(source_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&gpu.post_sampler),
                },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Extract Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &gpu.bloom_mips[0].view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&gpu.bloom_extract_pipeline);
            pass.set_bind_group(0, &extract_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // Downsample mips
        for i in 1..gpu.bloom_mips.len() {
            let src = &gpu.bloom_mips[i - 1];
            let dst = &gpu.bloom_mips[i];
            queue.write_buffer(
                &gpu.bloom_blur_params_buffer,
                0,
                bytemuck::cast_slice(&[BloomBlurParams {
                    texel_size: [1.0 / src.width as f32, 1.0 / src.height as f32],
                    direction: [0.0, 0.0],
                    intensity: 1.0,
                    mode: 0,
                    _pad0: [0.0; 2],
                }]),
            );
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bloom Downsample BG"),
                layout: &gpu.bloom_blur_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: gpu.bloom_blur_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&src.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&gpu.post_sampler),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Downsample Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&gpu.bloom_blur_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // Upsample and accumulate back into larger mips.
        for i in (1..gpu.bloom_mips.len()).rev() {
            let src = &gpu.bloom_mips[i];
            let dst = &gpu.bloom_mips[i - 1];
            queue.write_buffer(
                &gpu.bloom_blur_params_buffer,
                0,
                bytemuck::cast_slice(&[BloomBlurParams {
                    texel_size: [1.0 / src.width as f32, 1.0 / src.height as f32],
                    direction: [0.0, 0.0],
                    intensity: 0.35,
                    mode: 2,
                    _pad0: [0.0; 2],
                }]),
            );
            let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Bloom Upsample BG"),
                layout: &gpu.bloom_blur_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: gpu.bloom_blur_params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&src.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&gpu.post_sampler),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Upsample Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &dst.view,
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
            pass.set_pipeline(&gpu.bloom_blur_pipeline);
            pass.set_bind_group(0, &bg, &[]);
            pass.draw(0..3, 0..1);
        }

        // Blur bloom mip0 (horizontal -> ping, vertical -> mip0)
        let base = &gpu.bloom_mips[0];
        queue.write_buffer(
            &gpu.bloom_blur_params_buffer,
            0,
            bytemuck::cast_slice(&[BloomBlurParams {
                texel_size: [1.0 / base.width as f32, 1.0 / base.height as f32],
                direction: [1.0, 0.0],
                intensity: 1.0,
                mode: 1,
                _pad0: [0.0; 2],
            }]),
        );
        let blur_h_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Blur H BG"),
            layout: &gpu.bloom_blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu.bloom_blur_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&base.view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&gpu.post_sampler),
                },
            ],
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Bloom Blur Horizontal"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &gpu.bloom_ping_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&gpu.bloom_blur_pipeline);
            pass.set_bind_group(0, &blur_h_bg, &[]);
            pass.draw(0..3, 0..1);
        }

        queue.write_buffer(
            &gpu.bloom_blur_params_buffer,
            0,
            bytemuck::cast_slice(&[BloomBlurParams {
                texel_size: [1.0 / base.width as f32, 1.0 / base.height as f32],
                direction: [0.0, 1.0],
                intensity: 1.0,
                mode: 1,
                _pad0: [0.0; 2],
            }]),
        );
        let blur_v_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bloom Blur V BG"),
            layout: &gpu.bloom_blur_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu.bloom_blur_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&gpu.bloom_ping_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&gpu.post_sampler),
                },
            ],
        });
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Bloom Blur Vertical"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &gpu.bloom_mips[0].view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&gpu.bloom_blur_pipeline);
        pass.set_bind_group(0, &blur_v_bg, &[]);
        pass.draw(0..3, 0..1);
    }

    fn render_tonemap_composite(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        hdr_scene_view: &wgpu::TextureView,
        bloom_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
    ) {
        let gpu = self.gpu.as_ref().unwrap();
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tonemap Composite BG"),
            layout: &gpu.tonemap_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: gpu.tonemap_params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(hdr_scene_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(bloom_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&gpu.post_sampler),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Tonemap Composite Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&gpu.tonemap_pipeline);
        pass.set_bind_group(0, &bg, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Prepare all GPU buffer data for the current frame: dynamic meshes, uniforms,
    /// SDF cannon data, and visual system updates. Returns the dynamic index count.
    fn prepare_frame_data(&mut self, time: f32, _delta_time: f32) -> u32 {
        fn grow_capacity(current: u64, required: u64) -> u64 {
            let mut capacity = current.max(64 * 1024);
            while capacity < required {
                let next = capacity.saturating_mul(2);
                if next <= capacity {
                    return required;
                }
                capacity = next;
            }
            capacity
        }

        // Build dynamic mesh from scene data (needs &self.scene, then &self for ghost/grid)
        let dynamic_mesh = self.scene.as_ref().unwrap().generate_dynamic_mesh();
        let dynamic_indices: Vec<u32> = (0..dynamic_mesh.len() as u32).collect();

        // Legacy hex-builder ghost/grid previews are intentionally disabled in battle_arena.

        let dynamic_index_count = dynamic_indices.len() as u32;

        {
            let gpu = self.gpu.as_mut().unwrap();
            let vertex_bytes = (dynamic_mesh.len() * std::mem::size_of::<Vertex>()) as u64;
            let index_bytes = (dynamic_indices.len() * std::mem::size_of::<u32>()) as u64;

            if vertex_bytes > gpu.dynamic_vertex_capacity {
                let new_capacity = grow_capacity(gpu.dynamic_vertex_capacity, vertex_bytes);
                gpu.dynamic_vertex_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Dynamic Vertex Buffer"),
                    size: new_capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                gpu.dynamic_vertex_capacity = new_capacity;
                println!(
                    "[DynamicMesh] Grew vertex buffer to {} bytes",
                    gpu.dynamic_vertex_capacity
                );
            }

            if index_bytes > gpu.dynamic_index_capacity {
                let new_capacity = grow_capacity(gpu.dynamic_index_capacity, index_bytes);
                gpu.dynamic_index_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("Dynamic Index Buffer"),
                    size: new_capacity,
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                gpu.dynamic_index_capacity = new_capacity;
                println!(
                    "[DynamicMesh] Grew index buffer to {} bytes",
                    gpu.dynamic_index_capacity
                );
            }

            if !dynamic_mesh.is_empty() {
                gpu.queue.write_buffer(
                    &gpu.dynamic_vertex_buffer,
                    0,
                    bytemuck::cast_slice(&dynamic_mesh),
                );
                gpu.queue.write_buffer(
                    &gpu.dynamic_index_buffer,
                    0,
                    bytemuck::cast_slice(&dynamic_indices),
                );
            }
        }

        let gpu = self.gpu.as_ref().unwrap();
        let queue = &gpu.queue;
        let config = &gpu.surface_config;
        let scene = self.scene.as_ref().unwrap();

        // Update uniforms
        let aspect = config.width as f32 / config.height as f32;
        let view_mat = self.camera.get_view_matrix();
        let proj_mat = self.camera.get_projection_matrix(aspect);
        let jitter = if self.postfx_enabled && self.taa_enabled {
            self.compute_taa_jitter(config.width, config.height)
        } else {
            [0.0, 0.0]
        };
        let jitter_mat = Mat4::from_cols_array(&[
            1.0, 0.0, 0.0, 0.0, //
            0.0, 1.0, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            jitter[0], jitter[1], 0.0, 1.0,
        ]);
        let view_proj = jitter_mat * proj_mat * view_mat;
        self.current_view_proj = view_proj;
        self.current_jitter = jitter;

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

        let taa_cfg = &scene.visuals.postfx.taa;
        let taa_params = TaaParams {
            inv_curr_view_proj: view_proj.inverse().to_cols_array_2d(),
            prev_view_proj: self.prev_view_proj.to_cols_array_2d(),
            inv_resolution: [
                1.0 / config.width.max(1) as f32,
                1.0 / config.height.max(1) as f32,
            ],
            jitter,
            history_weight: taa_cfg.history_weight,
            new_weight: taa_cfg.new_weight,
            reject_threshold: taa_cfg.depth_reject_threshold,
            enabled: if self.postfx_enabled && self.taa_enabled && taa_cfg.enabled {
                1
            } else {
                0
            },
        };
        queue.write_buffer(
            &gpu.taa_params_buffer,
            0,
            bytemuck::cast_slice(&[taa_params]),
        );

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
                    let len =
                        (cross.x * cross.x + cross.y * cross.y + cross.z * cross.z + w * w).sqrt();
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

        let bloom_cfg = &scene.visuals.postfx.bloom;
        queue.write_buffer(
            &gpu.bloom_extract_params_buffer,
            0,
            bytemuck::cast_slice(&[BloomExtractParams {
                threshold: bloom_cfg.threshold,
                knee: bloom_cfg.knee,
                _pad0: [0.0; 2],
            }]),
        );

        let bloom_intensity = if self.postfx_enabled && self.bloom_enabled && bloom_cfg.enabled {
            bloom_cfg.intensity
        } else {
            0.0
        };
        let tone_cfg = &scene.visuals.postfx.tonemap;
        queue.write_buffer(
            &gpu.tonemap_params_buffer,
            0,
            bytemuck::cast_slice(&[TonemapCompositeParams {
                exposure: tone_cfg.exposure,
                saturation: tone_cfg.saturation,
                contrast: tone_cfg.contrast,
                bloom_intensity,
            }]),
        );

        // Update visual systems — skybox with day/night crossfade
        if let Some(ref cubemap_skybox) = self.cubemap_skybox {
            // Compute blend factor from DayCycle time (0-1)
            // Dawn (0.0-0.15): night → day transition
            // Day  (0.15-0.65): pure day
            // Dusk (0.65-0.8): day → night transition
            // Night(0.8-1.0): pure night
            let day_time = if scene.visuals.postfx.lock_midday {
                0.40
            } else {
                scene.game_state.day_cycle.time()
            };
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

        // Building blocks (camera-aware chunk culling).
        let cam_pos = self.camera.position;
        let cam_forward = self.camera.get_forward();
        for chunk in &gpu.block_chunk_buffers {
            if chunk.index_count == 0 {
                continue;
            }

            let to_chunk = chunk.center - cam_pos;
            let dist = to_chunk.length();
            if dist > BLOCK_RENDER_MAX_DISTANCE + chunk.radius {
                continue;
            }
            if dist > 20.0 {
                let dir = to_chunk / dist.max(1e-6);
                if cam_forward.dot(dir) < BLOCK_REAR_CULL_DOT {
                    continue;
                }
            }

            render_pass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
            render_pass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..chunk.index_count, 0, 0..1);
        }

        // Block placement preview / drag hologram
        let mut preview_positions = Vec::new();
        if scene.building.toolbar().visible
            && scene.building.toolbar().show_preview
            && !scene.building.toolbar().is_bridge_mode()
        {
            if self.shift_drag_build_active && !self.shift_drag_preview_positions.is_empty() {
                preview_positions.extend(self.shift_drag_preview_positions.iter().copied());
            } else if let Some(preview_pos) = scene.building.toolbar().preview_position {
                preview_positions.push(preview_pos);
            }
        }

        if !preview_positions.is_empty() {
            let quick_mode = scene.building.toolbar().quick_mode;
            let mut preview_instances: Vec<(Vec3, battle_tok_engine::render::BuildingBlockShape)> =
                Vec::new();
            if quick_mode {
                for anchor in &preview_positions {
                    preview_instances.extend(self.structure_instances_at_anchor(*anchor));
                }
            } else {
                let shape = scene.building.toolbar().get_selected_shape();
                preview_instances.extend(preview_positions.iter().copied().map(|p| (p, shape)));
            }

            let pulse = (self.start_time.elapsed().as_secs_f32() * 3.0).sin() * 0.5 + 0.5;
            let alpha = 0.22 + pulse * 0.18;
            if let Some((preview_vertices, preview_indices)) =
                self.generate_block_preview_mesh(&preview_instances, alpha, [0.15, 1.0, 0.30])
            {
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
                render_pass.set_pipeline(&gpu.preview_pipeline);
                render_pass.set_bind_group(0, &gpu.uniform_bind_group, &[]);
                render_pass.set_vertex_buffer(0, preview_vb.slice(..));
                render_pass.set_index_buffer(preview_ib.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..preview_indices.len() as u32, 0, 0..1);
                render_pass.set_pipeline(&gpu.pipeline);
                render_pass.set_bind_group(0, &gpu.uniform_bind_group, &[]);
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
                        // In first-person mode: jump + fire
                        scene.player.request_jump();
                    }
                    if !self.builder_mode.enabled && !scene.fire_cannon() {
                        println!("[Weapon] Cannot fire: projectile limit reached");
                    }
                }
                self.movement.up = pressed;
            }
            KeyCode::KeyF if pressed => {
                // F key: Fire active weapon
                if !scene.fire_cannon() {
                    println!("[Weapon] Cannot fire: projectile limit reached");
                }
            }
            KeyCode::KeyX if pressed => {
                let mode = scene.toggle_weapon_mode();
                println!(
                    "[Weapon] {}",
                    match mode {
                        WeaponMode::Cannonball => "Cannonball mode",
                        WeaponMode::RocketLauncher =>
                            "Rocket launcher mode (blast affects both sides)",
                    }
                );
            }
            KeyCode::KeyG if pressed => {
                // G key: Grab/release cannon
                let changed = scene.toggle_cannon_grab();
                if changed {
                    let grabbed = scene.cannon.is_grabbed();
                    println!(
                        "[Cannon] {}",
                        if grabbed {
                            "Grabbed! Walk to move it, F to fire, X to swap weapon, G to release"
                        } else {
                            "Released at current position"
                        }
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
                // Keep legacy hex-builder path disabled to avoid overlay/input conflicts.
                self.builder_mode.enabled = false;
                self.builder_mode.cursor_coord = None;
                scene.building.toolbar_mut().toggle();
                if !scene.building.toolbar().visible {
                    self.shift_drag_build_active = false;
                    self.shift_drag_start = None;
                    self.shift_drag_preview_positions.clear();
                }
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
                    if scene.building.toolbar().quick_mode {
                        scene.building.toolbar_mut().next_structure();
                    } else {
                        scene.building.toolbar_mut().next_shape();
                    }
                }
            }

            KeyCode::KeyQ | KeyCode::KeyM if pressed => {
                let toolbar_was_visible = scene.building.toolbar().visible;
                if toolbar_was_visible {
                    scene.building.toolbar_mut().toggle_quick_mode();
                } else {
                    scene.building.toolbar_mut().toggle();
                    if let Some(window) = &self.window {
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                    }
                    let mode_name = if scene.building.toolbar().quick_mode {
                        "quick-build"
                    } else {
                        "primitive"
                    };
                    println!(
                        "[BuildToolbar] Opened via Q/M in {} mode (press Q/M again to toggle)",
                        mode_name
                    );
                }
                self.update_block_preview();
            }

            KeyCode::Digit1 if pressed && scene.building.toolbar().visible => {
                if scene.building.toolbar().quick_mode {
                    scene.building.toolbar_mut().select_structure(0)
                } else {
                    scene.building.toolbar_mut().select_shape(0)
                }
            }
            KeyCode::Digit2 if pressed && scene.building.toolbar().visible => {
                if scene.building.toolbar().quick_mode {
                    scene.building.toolbar_mut().select_structure(1)
                } else {
                    scene.building.toolbar_mut().select_shape(1)
                }
            }
            KeyCode::Digit3 if pressed && scene.building.toolbar().visible => {
                if scene.building.toolbar().quick_mode {
                    scene.building.toolbar_mut().select_structure(2)
                } else {
                    scene.building.toolbar_mut().select_shape(2)
                }
            }
            KeyCode::Digit4 if pressed && scene.building.toolbar().visible => {
                if scene.building.toolbar().quick_mode {
                    scene.building.toolbar_mut().select_structure(3)
                } else {
                    scene.building.toolbar_mut().select_shape(3)
                }
            }
            KeyCode::Digit5 if pressed && scene.building.toolbar().visible => {
                if scene.building.toolbar().quick_mode {
                    scene.building.toolbar_mut().select_structure(4)
                } else {
                    scene.building.toolbar_mut().select_shape(4)
                }
            }
            KeyCode::Digit6 if pressed && scene.building.toolbar().visible => {
                if scene.building.toolbar().quick_mode {
                    scene.building.toolbar_mut().select_structure(5)
                } else {
                    scene.building.toolbar_mut().select_shape(5)
                }
            }
            KeyCode::Digit7 if pressed && scene.building.toolbar().visible => {
                if scene.building.toolbar().quick_mode {
                    scene.building.toolbar_mut().select_structure(6)
                } else {
                    scene.building.toolbar_mut().select_shape(6)
                }
            }

            KeyCode::ArrowUp if pressed && scene.building.toolbar().visible => {
                if scene.building.toolbar().quick_mode {
                    scene.building.toolbar_mut().prev_structure()
                } else {
                    scene.building.toolbar_mut().prev_shape()
                }
            }
            KeyCode::ArrowDown if pressed && scene.building.toolbar().visible => {
                if scene.building.toolbar().quick_mode {
                    scene.building.toolbar_mut().next_structure()
                } else {
                    scene.building.toolbar_mut().next_shape()
                }
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
            KeyCode::F7 if pressed => {
                self.postfx_enabled = !self.postfx_enabled;
                println!(
                    "[PostFx] {}",
                    if self.postfx_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            KeyCode::F8 if pressed => {
                self.taa_enabled = !self.taa_enabled;
                println!(
                    "[PostFx] TAA {}",
                    if self.taa_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
            KeyCode::F9 if pressed => {
                self.bloom_enabled = !self.bloom_enabled;
                println!(
                    "[PostFx] Bloom {}",
                    if self.bloom_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
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
                        } else if self.terrain_ui.on_mouse_release(mouse_pos.0, mouse_pos.1)
                            && let Some(scene) = self.scene.as_mut()
                        {
                            scene.terrain_needs_rebuild = true;
                        }

                        if !pressed && self.shift_drag_build_active {
                            self.commit_shift_drag_build();
                        }

                        let toolbar_visible = self
                            .scene
                            .as_ref()
                            .is_some_and(|s| s.building.toolbar().visible);
                        let bridge_mode = self
                            .scene
                            .as_ref()
                            .is_some_and(|s| s.building.toolbar().is_bridge_mode());

                        if toolbar_visible && pressed {
                            if self.movement.sprint && !bridge_mode {
                                self.start_shift_drag_build();
                            } else if bridge_mode {
                                self.handle_bridge_click();
                            } else if !ENABLE_SDF_DOUBLE_CLICK_MERGE || !self.handle_block_click() {
                                self.place_building_block();
                            }
                        }
                    }
                    MouseButton::Right => {
                        self.mouse_pressed = state == ElementState::Pressed;
                    }
                    MouseButton::Middle => {
                        if state == ElementState::Pressed
                            && let Some(scene) = self.scene.as_mut()
                            && scene.building.toolbar().visible
                        {
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
                self.frame_time_ms = delta_time * 1000.0;

                self.frame_count += 1;
                if now.duration_since(self.last_fps_update).as_secs_f32() >= 1.0 {
                    self.fps = self.frame_count as f32
                        / now.duration_since(self.last_fps_update).as_secs_f32();
                    self.frame_count = 0;
                    self.last_fps_update = now;

                    if let (Some(window), Some(scene)) = (&self.window, &self.scene) {
                        let mode_str = if scene.building.toolbar().visible {
                            if scene.building.toolbar().quick_mode {
                                format!(
                                    "Build-Q:{}",
                                    scene.building.toolbar().quick_structure_name()
                                )
                            } else {
                                "Build-Primitive".to_string()
                            }
                        } else {
                            "Combat".to_string()
                        };
                        let weapon_str = match scene.weapon_mode() {
                            WeaponMode::Cannonball => "Cannonball",
                            WeaponMode::RocketLauncher => "Rocket",
                        };
                        let postfx_str = format!(
                            "P:{} TAA:{} B:{}",
                            if self.postfx_enabled { "on" } else { "off" },
                            if self.taa_enabled { "on" } else { "off" },
                            if self.bloom_enabled { "on" } else { "off" }
                        );
                        window.set_title(&format!(
                            "Battle Sphere - {} | Weapon: {} | FPS: {:.0} | {:.2}ms | Draws~{} | {} | Prisms: {}",
                            mode_str,
                            weapon_str,
                            self.fps,
                            self.frame_time_ms,
                            self.draw_calls_estimate,
                            postfx_str,
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

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    println!("===========================================");
    println!("   Battle Sphere - Combat Arena");
    println!("===========================================");
    println!();
    println!("*** Click anywhere to start ***");
    println!();
    println!("Controls: WASD Move, Space Jump, V Toggle FPS/Free");
    println!("G: Grab/Release Cannon, F: Fire, X: Toggle Rocket Launcher");
    println!(
        "B: Builder, T: Terrain Editor, F7/F8/F9: PostFx/TAA/Bloom, F11: Fullscreen, ESC: Exit"
    );
    println!();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = BattleArenaApp::new();
    event_loop.run_app(&mut app).unwrap();
}

#[cfg(target_arch = "wasm32")]
fn main() {}

/// Browser entry point. Async init (request_adapter/request_device) then runs the game loop.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub fn wasm_start() {
    console_error_panic_hook::set_once();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let attrs = WindowAttributes::default()
        .with_title("Battle Sphere - Combat Arena")
        .with_inner_size(PhysicalSize::new(1920, 1080));

    let window = Arc::new(event_loop.create_window(attrs).expect("create window"));

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let surface = instance.create_surface(Arc::clone(&window)).unwrap();
    let surface: wgpu::Surface<'static> = unsafe { std::mem::transmute(surface) };

    spawn_local(async move {
        let (instance, surface, adapter, device, queue) =
            BattleArenaApp::request_gpu_async(instance, surface).await;

        let mut app = BattleArenaApp::new();
        app.initialize_from_gpu(window, instance, adapter, device, queue, surface);

        event_loop.run_app(&mut app).unwrap();
    });
}
