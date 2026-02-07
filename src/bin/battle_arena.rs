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

use std::collections::{BTreeMap, HashMap, HashSet};
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
#[cfg(not(target_arch = "wasm32"))]
use std::thread;
use std::time::Instant;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

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
    BrickTreeGpuBuffers, FogPostConfig, FogPostPass, LavaSteamConfig, MaterialSystem,
    ParticleSystem, PointLightManager, SceneConfig,
};

// Import building block types for GPU operations
use battle_tok_engine::render::{BuildingBlock, BuildingBlockShape};

// Import game module types
use battle_tok_engine::game::ProjectileKind;
use battle_tok_engine::game::config::{ArenaConfig, VisualConfig};
use battle_tok_engine::game::terrain::terrain_height_at_island;
use battle_tok_engine::game::{
    BattleScene, BridgeConfig, BuildMode, BuilderMode, Camera, FloatingIslandConfig, LavaParams,
    Mesh, MovementKeys, PLAYER_EYE_HEIGHT, SHADER_SOURCE, SdfCannonData, SdfCannonUniforms,
    StartOverlay, TerrainEditorUI, TerrainParams, Uniforms, Vertex, VoxelCoord, VoxelHudState,
    VoxelMaterialId, WeaponMode, CastleToolParams, draw_text, add_quad,
    generate_all_trees_mesh, generate_bridge, generate_floating_island, generate_lava_ocean,
    generate_trees_on_terrain, is_inside_hexagon, set_terrain_params,
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
    _center: Vec3,
    _radius: f32,
    _face_dir: VoxelFaceDir,
    first_instance: u32,
    instance_count: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawIndexedIndirectCommand {
    index_count: u32,
    instance_count: u32,
    first_index: u32,
    base_vertex: i32,
    first_instance: u32,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BlockQuadVertex {
    uv: [f32; 2],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct PackedBlockFaceInstance {
    /// local x5 | y5 | z5 | dir3 | width5 | height5 | spare4
    pack0: u32,
    /// chunk_x10 | chunk_y10 | chunk_z10 (biased by +512) | spare2
    pack1: u32,
    /// RGBA8 tint.
    color_rgba8: u32,
}

#[derive(Clone, Debug, Default)]
struct BlockChunkCpuData {
    center: Vec3,
    radius: f32,
    faces: [Vec<PackedBlockFaceInstance>; 6],
}

#[derive(Clone, Copy, Debug)]
struct BlockSnapshotCell {
    x: i32,
    y: i32,
    z: i32,
    material: u8,
    crack_stage: u8,
}

#[derive(Clone, Debug)]
struct BlockChunkBuildInput {
    key: (i32, i32, i32),
    cells: Vec<BlockSnapshotCell>,
}

#[derive(Debug)]
struct BlockChunkBuildOutput {
    key: (i32, i32, i32),
    chunk: Option<BlockChunkCpuData>,
}

#[derive(Debug)]
struct BlockChunkBuildJob {
    id: u64,
    chunks: Vec<BlockChunkBuildInput>,
}

#[derive(Debug)]
struct BlockChunkBuildResult {
    id: u64,
    outputs: Vec<BlockChunkBuildOutput>,
}

#[cfg(not(target_arch = "wasm32"))]
struct BlockChunkMeshWorker {
    tx_job: Sender<BlockChunkBuildJob>,
    rx_result: Receiver<BlockChunkBuildResult>,
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
const BLOCK_RENDER_CHUNK_SIZE: i32 = 12;
const INITIAL_BLOCK_CHUNK_INDIRECT_CAPACITY: u32 = 256;
const INITIAL_BLOCK_INSTANCE_CAPACITY: u32 = 1024;
const BLOCK_CHUNK_MESH_JOB_BATCH: usize = 128;
const VOXEL_SIZE_METERS: f32 = 0.25;
const VOXEL_SHELL_SHADER_SOURCE: &str = include_str!("../../shaders/voxel_shell.wgsl");
const BLOCK_FACE_SHADER_SOURCE: &str = include_str!("../shaders/block_faces.wgsl");
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

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct VoxelShellUniforms {
    inv_view_proj: [[f32; 4]; 4],
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    _pad0: f32,
    resolution: [f32; 2],
    node_count: u32,
    leaf_count: u32,
    sun_dir: [f32; 3],
    _pad1: f32,
}

struct BloomMip {
    _texture: wgpu::Texture,
    view: wgpu::TextureView,
    width: u32,
    height: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum VoxelFaceDir {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

fn greedy_rects_from_tiles(
    tiles: &std::collections::BTreeMap<(i32, i32), u16>,
) -> Vec<(i32, i32, i32, i32, u16)> {
    let mut remaining = tiles.clone();
    let mut rects = Vec::new();

    while let Some((&(u0, v0), &material)) = remaining.iter().next() {
        let mut width = 1i32;
        while remaining.get(&(u0 + width, v0)) == Some(&material) {
            width += 1;
        }

        let mut height = 1i32;
        'expand_height: loop {
            let next_v = v0 + height;
            for du in 0..width {
                if remaining.get(&(u0 + du, next_v)) != Some(&material) {
                    break 'expand_height;
                }
            }
            height += 1;
        }

        for dv in 0..height {
            for du in 0..width {
                remaining.remove(&(u0 + du, v0 + dv));
            }
        }

        rects.push((u0, v0, width, height, material));
    }

    rects
}

fn voxel_face_dir_index(dir: VoxelFaceDir) -> usize {
    match dir {
        VoxelFaceDir::PosX => 0,
        VoxelFaceDir::NegX => 1,
        VoxelFaceDir::PosY => 2,
        VoxelFaceDir::NegY => 3,
        VoxelFaceDir::PosZ => 4,
        VoxelFaceDir::NegZ => 5,
    }
}

fn block_material_color(material: u8, crack_stage: u8) -> [f32; 4] {
    let base = match material {
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
    };
    if crack_stage == 0 {
        return base;
    }
    if crack_stage >= 6 {
        return [1.0, 0.18, 0.05, 1.0];
    }
    let severity = (crack_stage as f32 / 5.0).clamp(0.0, 1.0);
    let darken = 1.0 - severity * 0.42;
    [
        (base[0] * darken + severity * 0.28).clamp(0.0, 1.0),
        (base[1] * darken + severity * 0.05).clamp(0.0, 1.0),
        (base[2] * darken + severity * 0.05).clamp(0.0, 1.0),
        base[3],
    ]
}

fn crack_stage_from_hp(hp: u16, max_hp: u16) -> u8 {
    if max_hp == 0 || hp >= max_hp {
        return 0;
    }
    let ratio = 1.0 - (hp as f32 / max_hp as f32);
    (ratio * 5.0).round().clamp(0.0, 6.0) as u8
}

fn pack_rgba8(color: [f32; 4]) -> u32 {
    let r = (color[0].clamp(0.0, 1.0) * 255.0).round() as u32;
    let g = (color[1].clamp(0.0, 1.0) * 255.0).round() as u32;
    let b = (color[2].clamp(0.0, 1.0) * 255.0).round() as u32;
    let a = (color[3].clamp(0.0, 1.0) * 255.0).round() as u32;
    r | (g << 8) | (b << 16) | (a << 24)
}

fn pack_block_face_instance(
    local: (i32, i32, i32),
    dir: VoxelFaceDir,
    width: i32,
    height: i32,
    material: u8,
    chunk_key: (i32, i32, i32),
    color: [f32; 4],
) -> PackedBlockFaceInstance {
    let lx = local.0.clamp(0, 31) as u32;
    let ly = local.1.clamp(0, 31) as u32;
    let lz = local.2.clamp(0, 31) as u32;
    let dir_u = voxel_face_dir_index(dir) as u32;
    let w = width.clamp(1, 31) as u32;
    let h = height.clamp(1, 31) as u32;

    let mat = (material as u32) & 0xF;
    let pack0 = lx | (ly << 5) | (lz << 10) | (dir_u << 15) | (w << 18) | (h << 23) | (mat << 28);

    let cx = (chunk_key.0 + 512).clamp(0, 1023) as u32;
    let cy = (chunk_key.1 + 512).clamp(0, 1023) as u32;
    let cz = (chunk_key.2 + 512).clamp(0, 1023) as u32;
    let pack1 = cx | (cy << 10) | (cz << 20);

    PackedBlockFaceInstance {
        pack0,
        pack1,
        color_rgba8: pack_rgba8(color),
    }
}

fn build_packed_chunk_faces(input: &BlockChunkBuildInput) -> Option<BlockChunkCpuData> {
    let chunk_origin = (
        input.key.0 * BLOCK_RENDER_CHUNK_SIZE,
        input.key.1 * BLOCK_RENDER_CHUNK_SIZE,
        input.key.2 * BLOCK_RENDER_CHUNK_SIZE,
    );
    let chunk_max = (
        chunk_origin.0 + BLOCK_RENDER_CHUNK_SIZE,
        chunk_origin.1 + BLOCK_RENDER_CHUNK_SIZE,
        chunk_origin.2 + BLOCK_RENDER_CHUNK_SIZE,
    );

    let mut occupied: HashMap<(i32, i32, i32), (u8, u8)> = HashMap::new();
    for c in &input.cells {
        occupied.insert((c.x, c.y, c.z), (c.material, c.crack_stage));
    }

    let in_core = |x: i32, y: i32, z: i32| {
        x >= chunk_origin.0
            && x < chunk_max.0
            && y >= chunk_origin.1
            && y < chunk_max.1
            && z >= chunk_origin.2
            && z < chunk_max.2
    };

    let mut min = Vec3::ZERO;
    let mut max = Vec3::ZERO;
    let mut initialized = false;
    let mut planes: BTreeMap<(VoxelFaceDir, i32), BTreeMap<(i32, i32), u16>> = BTreeMap::new();

    for (&(x, y, z), &(material, crack_stage)) in &occupied {
        if !in_core(x, y, z) {
            continue;
        }

        let cube_min = Vec3::new(
            x as f32 * VOXEL_SIZE_METERS,
            y as f32 * VOXEL_SIZE_METERS,
            z as f32 * VOXEL_SIZE_METERS,
        );
        let cube_max = cube_min + Vec3::splat(VOXEL_SIZE_METERS);
        if initialized {
            min = min.min(cube_min);
            max = max.max(cube_max);
        } else {
            min = cube_min;
            max = cube_max;
            initialized = true;
        }

        let encoded = ((material as u16) << 8) | crack_stage as u16;
        if !occupied.contains_key(&(x + 1, y, z)) {
            planes
                .entry((VoxelFaceDir::PosX, x + 1))
                .or_default()
                .insert((y, z), encoded);
        }
        if !occupied.contains_key(&(x - 1, y, z)) {
            planes
                .entry((VoxelFaceDir::NegX, x))
                .or_default()
                .insert((y, z), encoded);
        }
        if !occupied.contains_key(&(x, y + 1, z)) {
            planes
                .entry((VoxelFaceDir::PosY, y + 1))
                .or_default()
                .insert((x, z), encoded);
        }
        if !occupied.contains_key(&(x, y - 1, z)) {
            planes
                .entry((VoxelFaceDir::NegY, y))
                .or_default()
                .insert((x, z), encoded);
        }
        if !occupied.contains_key(&(x, y, z + 1)) {
            planes
                .entry((VoxelFaceDir::PosZ, z + 1))
                .or_default()
                .insert((x, y), encoded);
        }
        if !occupied.contains_key(&(x, y, z - 1)) {
            planes
                .entry((VoxelFaceDir::NegZ, z))
                .or_default()
                .insert((x, y), encoded);
        }
    }

    if !initialized {
        return None;
    }

    let mut chunk = BlockChunkCpuData {
        center: (min + max) * 0.5,
        radius: (max - ((min + max) * 0.5)).length().max(1.0),
        faces: std::array::from_fn(|_| Vec::new()),
    };

    for ((dir, plane), tiles) in planes {
        for (u0, v0, width, height, encoded) in greedy_rects_from_tiles(&tiles) {
            let material = ((encoded >> 8) & 0xFF) as u8;
            let crack_stage = (encoded & 0xFF) as u8;
            let color = block_material_color(material, crack_stage);
            let local = match dir {
                VoxelFaceDir::PosX | VoxelFaceDir::NegX => (
                    plane - chunk_origin.0,
                    u0 - chunk_origin.1,
                    v0 - chunk_origin.2,
                ),
                VoxelFaceDir::PosY | VoxelFaceDir::NegY => (
                    u0 - chunk_origin.0,
                    plane - chunk_origin.1,
                    v0 - chunk_origin.2,
                ),
                VoxelFaceDir::PosZ | VoxelFaceDir::NegZ => (
                    u0 - chunk_origin.0,
                    v0 - chunk_origin.1,
                    plane - chunk_origin.2,
                ),
            };
            let packed =
                pack_block_face_instance(local, dir, width, height, material, input.key, color);
            chunk.faces[voxel_face_dir_index(dir)].push(packed);
        }
    }

    Some(chunk)
}

fn build_block_chunk_outputs(inputs: Vec<BlockChunkBuildInput>) -> Vec<BlockChunkBuildOutput> {
    inputs
        .into_iter()
        .map(|chunk_in| BlockChunkBuildOutput {
            key: chunk_in.key,
            chunk: build_packed_chunk_faces(&chunk_in),
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
impl BlockChunkMeshWorker {
    fn spawn() -> Option<Self> {
        let (tx_job, rx_job) = mpsc::channel::<BlockChunkBuildJob>();
        let (tx_result, rx_result) = mpsc::channel::<BlockChunkBuildResult>();
        let builder = thread::Builder::new().name("block-chunk-mesh-worker".to_string());
        let spawn_result = builder.spawn(move || {
            if let Some(core_ids) = core_affinity::get_core_ids()
                && !core_ids.is_empty()
            {
                let preferred = core_ids.get(1).or_else(|| core_ids.first()).copied();
                if let Some(core_id) = preferred {
                    let _ = core_affinity::set_for_current(core_id);
                }
            }

            while let Ok(job) = rx_job.recv() {
                let outputs = build_block_chunk_outputs(job.chunks);
                if tx_result.send(BlockChunkBuildResult { id: job.id, outputs }).is_err() {
                    break;
                }
            }
        });
        match spawn_result {
            Ok(_) => Some(Self { tx_job, rx_result }),
            Err(_) => None,
        }
    }
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
    block_chunk_pipeline: wgpu::RenderPipeline,
    preview_pipeline: wgpu::RenderPipeline,
    voxel_shell_pipeline: wgpu::RenderPipeline,
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
    block_chunk_vertex_buffer: wgpu::Buffer,
    block_chunk_index_buffer: wgpu::Buffer,
    block_chunk_instance_buffer: wgpu::Buffer,
    block_chunk_instance_capacity: u32,
    block_chunk_buffers: Vec<BlockChunkBuffers>,
    block_chunk_indirect_buffer: wgpu::Buffer,
    block_chunk_indirect_capacity: u32,
    block_chunk_use_multi_draw: bool,
    brick_tree_buffers: BrickTreeGpuBuffers,
    voxel_shell_uniform_buffer: wgpu::Buffer,
    voxel_shell_bind_group_layout: wgpu::BindGroupLayout,
    voxel_shell_bind_group: wgpu::BindGroup,
    voxel_shell_enabled: bool,

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
    voxel_hud: VoxelHudState,

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
    block_chunk_cache: HashMap<(i32, i32, i32), BlockChunkCpuData>,
    pending_block_chunk_keys: HashSet<(i32, i32, i32)>,
    full_block_chunk_rebuild_pending: bool,
    block_mesh_job_next_id: u64,
    block_mesh_job_in_flight: bool,
    block_mesh_last_applied_id: u64,
    #[cfg(not(target_arch = "wasm32"))]
    block_chunk_mesh_worker: Option<BlockChunkMeshWorker>,

    // PostFx frame state
    prev_view_proj: Mat4,
    current_view_proj: Mat4,
    current_jitter: [f32; 2],
    taa_history_use_a: bool,
    postfx_enabled: bool,
    taa_enabled: bool,
    bloom_enabled: bool,
    use_voxel_shell: bool,
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
            voxel_hud: VoxelHudState::default(),
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
            block_chunk_cache: HashMap::new(),
            pending_block_chunk_keys: HashSet::new(),
            full_block_chunk_rebuild_pending: true,
            block_mesh_job_next_id: 1,
            block_mesh_job_in_flight: false,
            block_mesh_last_applied_id: 0,
            #[cfg(not(target_arch = "wasm32"))]
            block_chunk_mesh_worker: None,
            prev_view_proj: Mat4::IDENTITY,
            current_view_proj: Mat4::IDENTITY,
            current_jitter: [0.0, 0.0],
            taa_history_use_a: true,
            postfx_enabled: true,
            taa_enabled: true,
            bloom_enabled: true,
            use_voxel_shell: false,
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
                required_features: {
                    let supported = adapter.features();
                    let mut features = wgpu::Features::empty();
                    if supported.contains(wgpu::Features::INDIRECT_FIRST_INSTANCE) {
                        features |= wgpu::Features::INDIRECT_FIRST_INSTANCE;
                    }
                    features
                },
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

        let block_chunk_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Block Face Instance Shader"),
            source: wgpu::ShaderSource::Wgsl(BLOCK_FACE_SHADER_SOURCE.into()),
        });
        let block_chunk_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Block Face Instance Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &block_chunk_shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<BlockQuadVertex>() as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                    wgpu::VertexBufferLayout {
                        array_stride: std::mem::size_of::<PackedBlockFaceInstance>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint32,
                                offset: 0,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint32,
                                offset: 4,
                                shader_location: 2,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Uint32,
                                offset: 8,
                                shader_location: 3,
                            },
                        ],
                    },
                ],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &block_chunk_shader,
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

        let voxel_shell_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Voxel Shell Shader"),
            source: wgpu::ShaderSource::Wgsl(VOXEL_SHELL_SHADER_SOURCE.into()),
        });
        let voxel_shell_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Voxel Shell Uniform Buffer"),
            size: std::mem::size_of::<VoxelShellUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let brick_tree_buffers = BrickTreeGpuBuffers::create_empty(&device);
        let voxel_shell_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Voxel Shell Bind Group Layout"),
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
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Storage { read_only: true },
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });
        let voxel_shell_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Voxel Shell Bind Group"),
            layout: &voxel_shell_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: voxel_shell_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: brick_tree_buffers.node_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: brick_tree_buffers.leaf_buffer.as_entire_binding(),
                },
            ],
        });
        let voxel_shell_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Voxel Shell Pipeline Layout"),
                bind_group_layouts: &[&voxel_shell_bind_group_layout],
                push_constant_ranges: &[],
            });
        let voxel_shell_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Voxel Shell Pipeline"),
            layout: Some(&voxel_shell_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &voxel_shell_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &voxel_shell_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_SCENE_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
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

        let quad_vertices = [
            BlockQuadVertex { uv: [0.0, 0.0] },
            BlockQuadVertex { uv: [1.0, 0.0] },
            BlockQuadVertex { uv: [1.0, 1.0] },
            BlockQuadVertex { uv: [0.0, 1.0] },
        ];
        let quad_indices: [u32; 6] = [0, 1, 2, 0, 2, 3];
        let block_chunk_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Block Chunk Quad Vertex Buffer"),
            contents: bytemuck::cast_slice(&quad_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let block_chunk_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Block Chunk Quad Index Buffer"),
            contents: bytemuck::cast_slice(&quad_indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let block_chunk_instance_capacity = INITIAL_BLOCK_INSTANCE_CAPACITY.max(1);
        let block_chunk_instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Block Chunk Instance Buffer"),
            size: (block_chunk_instance_capacity as u64)
                * (std::mem::size_of::<PackedBlockFaceInstance>() as u64),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let block_chunk_indirect_capacity = INITIAL_BLOCK_CHUNK_INDIRECT_CAPACITY.max(1);
        let block_chunk_indirect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Block Chunk Indirect Buffer"),
            size: (block_chunk_indirect_capacity as u64)
                * (std::mem::size_of::<DrawIndexedIndirectCommand>() as u64),
            usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let block_chunk_use_multi_draw = device
            .features()
            .contains(wgpu::Features::INDIRECT_FIRST_INSTANCE);

        // Store everything
        self.window = Some(window);
        self.scene = Some(scene);
        self.gpu = Some(GpuResources {
            device,
            queue,
            surface,
            surface_config,
            pipeline,
            block_chunk_pipeline,
            preview_pipeline,
            voxel_shell_pipeline,
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
            block_chunk_vertex_buffer,
            block_chunk_index_buffer,
            block_chunk_instance_buffer,
            block_chunk_instance_capacity,
            block_chunk_buffers: Vec::new(),
            block_chunk_indirect_buffer,
            block_chunk_indirect_capacity,
            block_chunk_use_multi_draw,
            brick_tree_buffers,
            voxel_shell_uniform_buffer,
            voxel_shell_bind_group_layout,
            voxel_shell_bind_group,
            voxel_shell_enabled: false,
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
        self.block_chunk_cache.clear();
        self.pending_block_chunk_keys.clear();
        self.full_block_chunk_rebuild_pending = true;
        self.block_mesh_job_in_flight = false;
        self.block_mesh_last_applied_id = 0;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.block_chunk_mesh_worker = BlockChunkMeshWorker::spawn();
        }

        println!(
            "[Battle Arena] Hex-prism walls: {} vertices, {} indices",
            hex_wall_vertices.len(),
            hex_indices.len()
        );
    }

    fn update(&mut self, delta_time: f32) {
        let mut pending_brick_upload: Option<(
            Vec<battle_tok_engine::game::systems::voxel_building::BrickNode>,
            Vec<battle_tok_engine::game::systems::voxel_building::BrickLeaf64>,
        )> = None;
        let (dirty_voxel_chunks, block_mesh_dirty);

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

            let render_deltas = scene.building.drain_render_deltas();
            let local_dirty_voxel_chunks = render_deltas.dirty_chunks.clone();
            let needs_shell_rebuild = !render_deltas.dirty_chunks.is_empty()
                || !render_deltas.bake_jobs.is_empty()
                || !render_deltas.bake_results.is_empty();
            if needs_shell_rebuild && self.use_voxel_shell {
                scene.building.voxel_runtime.rebuild_brick_tree();
                pending_brick_upload = Some((
                    scene.building.voxel_runtime.brick_tree.nodes.clone(),
                    scene.building.voxel_runtime.brick_tree.leaves.clone(),
                ));
            }

            let local_block_mesh_dirty = scene.building.block_manager.needs_mesh_update();
            scene.building.block_manager.clear_dirty();
            let _ = scene.building.drain_audio_events();
            dirty_voxel_chunks = local_dirty_voxel_chunks;
            block_mesh_dirty = local_block_mesh_dirty;
        }

        if !dirty_voxel_chunks.is_empty() {
            self.mark_dirty_block_chunks_from_voxel_chunks(&dirty_voxel_chunks);
        } else if block_mesh_dirty {
            self.full_block_chunk_rebuild_pending = true;
        }

        self.collect_block_mesh_results();
        self.dispatch_block_mesh_job();
        self.emergency_sync_block_chunk_rebuild_if_needed();

        if let Some((nodes, leaves)) = pending_brick_upload
            && let Some(gpu) = self.gpu.as_mut()
        {
            gpu.brick_tree_buffers
                .upload(&gpu.device, &gpu.queue, &nodes, &leaves);
            gpu.voxel_shell_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Voxel Shell Bind Group"),
                layout: &gpu.voxel_shell_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: gpu.voxel_shell_uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: gpu.brick_tree_buffers.node_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: gpu.brick_tree_buffers.leaf_buffer.as_entire_binding(),
                    },
                ],
            });
            gpu.voxel_shell_enabled = !nodes.is_empty();
        } else if let Some(gpu) = self.gpu.as_mut()
            && !self.use_voxel_shell
        {
            gpu.voxel_shell_enabled = false;
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

        // Legacy hex-builder path stays disabled in battle runtime.
        self.update_builder_cursor();
        self.update_voxel_target();

        // Building physics now updates in BattleScene fixed-step.
    }

    fn block_render_chunk_key_from_voxel(x: i32, y: i32, z: i32) -> (i32, i32, i32) {
        (
            x.div_euclid(BLOCK_RENDER_CHUNK_SIZE),
            y.div_euclid(BLOCK_RENDER_CHUNK_SIZE),
            z.div_euclid(BLOCK_RENDER_CHUNK_SIZE),
        )
    }

    fn mark_dirty_block_chunks_from_voxel_chunks(&mut self, dirty_chunks: &[glam::IVec3]) {
        for chunk in dirty_chunks {
            let x0 = chunk.x * 16;
            let y0 = chunk.y * 16;
            let z0 = chunk.z * 16;
            let x1 = x0 + 15;
            let y1 = y0 + 15;
            let z1 = z0 + 15;

            let bx0 = x0.div_euclid(BLOCK_RENDER_CHUNK_SIZE);
            let by0 = y0.div_euclid(BLOCK_RENDER_CHUNK_SIZE);
            let bz0 = z0.div_euclid(BLOCK_RENDER_CHUNK_SIZE);
            let bx1 = x1.div_euclid(BLOCK_RENDER_CHUNK_SIZE);
            let by1 = y1.div_euclid(BLOCK_RENDER_CHUNK_SIZE);
            let bz1 = z1.div_euclid(BLOCK_RENDER_CHUNK_SIZE);

            for bz in bz0..=bz1 {
                for by in by0..=by1 {
                    for bx in bx0..=bx1 {
                        for dz in -1..=1 {
                            for dy in -1..=1 {
                                for dx in -1..=1 {
                                    self.pending_block_chunk_keys
                                        .insert((bx + dx, by + dy, bz + dz));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn snapshot_block_chunk(&self, key: (i32, i32, i32)) -> Option<BlockChunkBuildInput> {
        let scene = self.scene.as_ref()?;
        let world = &scene.building.voxel_runtime.world;
        let origin = (
            key.0 * BLOCK_RENDER_CHUNK_SIZE,
            key.1 * BLOCK_RENDER_CHUNK_SIZE,
            key.2 * BLOCK_RENDER_CHUNK_SIZE,
        );
        let min = (origin.0 - 1, origin.1 - 1, origin.2 - 1);
        let max = (
            origin.0 + BLOCK_RENDER_CHUNK_SIZE,
            origin.1 + BLOCK_RENDER_CHUNK_SIZE,
            origin.2 + BLOCK_RENDER_CHUNK_SIZE,
        );
        let mut cells = Vec::new();
        for z in min.2..=max.2 {
            for y in min.1..=max.1 {
                for x in min.0..=max.0 {
                    let coord = VoxelCoord::new(x, y, z);
                    let Some(cell) = world.get(coord) else {
                        continue;
                    };
                    cells.push(BlockSnapshotCell {
                        x,
                        y,
                        z,
                        material: cell.material,
                        crack_stage: crack_stage_from_hp(cell.hp, cell.max_hp),
                    });
                }
            }
        }
        Some(BlockChunkBuildInput { key, cells })
    }

    fn apply_block_chunk_build_result(&mut self, result: BlockChunkBuildResult) {
        if result.id < self.block_mesh_last_applied_id {
            return;
        }
        self.block_mesh_last_applied_id = result.id;
        for output in result.outputs {
            match output.chunk {
                Some(chunk) if chunk.faces.iter().any(|faces| !faces.is_empty()) => {
                    self.block_chunk_cache.insert(output.key, chunk);
                }
                _ => {
                    self.block_chunk_cache.remove(&output.key);
                }
            }
        }
        self.rebuild_block_chunk_gpu_buffers();
    }

    fn collect_block_mesh_results(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut drained = Vec::new();
            if let Some(worker) = &self.block_chunk_mesh_worker {
                while let Ok(result) = worker.rx_result.try_recv() {
                    drained.push(result);
                }
            }
            if !drained.is_empty() {
                self.block_mesh_job_in_flight = false;
                for result in drained {
                    self.apply_block_chunk_build_result(result);
                }
            }
        }
    }

    fn dispatch_block_mesh_job(&mut self) {
        if self.block_mesh_job_in_flight {
            return;
        }

        if self.full_block_chunk_rebuild_pending {
            self.block_chunk_cache.clear();
            self.pending_block_chunk_keys.clear();
            if let Some(scene) = self.scene.as_ref() {
                let occupied = scene.building.voxel_runtime.world.occupied_coords();
                for coord in occupied {
                    self.pending_block_chunk_keys.insert(Self::block_render_chunk_key_from_voxel(
                        coord.x, coord.y, coord.z,
                    ));
                }
            }
            self.full_block_chunk_rebuild_pending = false;
            if self.pending_block_chunk_keys.is_empty() {
                self.rebuild_block_chunk_gpu_buffers();
                return;
            }
        }

        if self.pending_block_chunk_keys.is_empty() {
            return;
        }

        let selected: Vec<(i32, i32, i32)> = self
            .pending_block_chunk_keys
            .iter()
            .copied()
            .take(BLOCK_CHUNK_MESH_JOB_BATCH)
            .collect();
        for key in &selected {
            self.pending_block_chunk_keys.remove(key);
        }

        let mut snapshots = Vec::with_capacity(selected.len());
        for key in selected {
            if let Some(snapshot) = self.snapshot_block_chunk(key) {
                snapshots.push(snapshot);
            }
        }
        if snapshots.is_empty() {
            return;
        }

        let job_id = self.block_mesh_job_next_id;
        self.block_mesh_job_next_id += 1;
        self.block_mesh_job_in_flight = true;

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(worker) = &self.block_chunk_mesh_worker {
            if worker
                .tx_job
                .send(BlockChunkBuildJob {
                    id: job_id,
                    chunks: snapshots.clone(),
                })
                .is_ok()
            {
                return;
            }
        }

        let outputs = build_block_chunk_outputs(snapshots);
        self.block_mesh_job_in_flight = false;
        self.apply_block_chunk_build_result(BlockChunkBuildResult { id: job_id, outputs });
    }

    fn rebuild_block_chunk_gpu_buffers(&mut self) {
        let Some(gpu) = self.gpu.as_mut() else {
            return;
        };

        let mut entries: Vec<(&(i32, i32, i32), &BlockChunkCpuData)> =
            self.block_chunk_cache.iter().collect();
        entries.sort_by_key(|(key, _)| **key);

        let dirs = [
            VoxelFaceDir::PosX,
            VoxelFaceDir::NegX,
            VoxelFaceDir::PosY,
            VoxelFaceDir::NegY,
            VoxelFaceDir::PosZ,
            VoxelFaceDir::NegZ,
        ];

        let mut all_instances = Vec::<PackedBlockFaceInstance>::new();
        let mut chunk_buffers = Vec::<BlockChunkBuffers>::new();
        for (_, chunk) in entries {
            for (dir_idx, dir) in dirs.iter().enumerate() {
                let faces = &chunk.faces[dir_idx];
                if faces.is_empty() {
                    continue;
                }
                let first_instance = all_instances.len() as u32;
                all_instances.extend_from_slice(faces);
                chunk_buffers.push(BlockChunkBuffers {
                    _center: chunk.center,
                    _radius: chunk.radius,
                    _face_dir: *dir,
                    first_instance,
                    instance_count: faces.len() as u32,
                });
            }
        }

        if all_instances.is_empty() {
            gpu.block_chunk_buffers.clear();
            return;
        }

        let needed = all_instances.len() as u32;
        if needed > gpu.block_chunk_instance_capacity {
            gpu.block_chunk_instance_capacity = needed.next_power_of_two();
            gpu.block_chunk_instance_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Block Chunk Instance Buffer"),
                size: (gpu.block_chunk_instance_capacity as u64)
                    * (std::mem::size_of::<PackedBlockFaceInstance>() as u64),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        gpu.queue.write_buffer(
            &gpu.block_chunk_instance_buffer,
            0,
            bytemuck::cast_slice(&all_instances),
        );
        gpu.block_chunk_buffers = chunk_buffers;
    }

    fn emergency_sync_block_chunk_rebuild_if_needed(&mut self) {
        if !self.block_chunk_cache.is_empty() {
            return;
        }
        let Some(scene) = self.scene.as_ref() else {
            return;
        };
        let occupied = scene.building.voxel_runtime.world.occupied_coords();
        if occupied.is_empty() {
            return;
        }

        let mut keys: HashSet<(i32, i32, i32)> = HashSet::new();
        for coord in occupied {
            keys.insert(Self::block_render_chunk_key_from_voxel(
                coord.x, coord.y, coord.z,
            ));
        }

        let mut inputs = Vec::with_capacity(keys.len());
        for key in keys {
            if let Some(snapshot) = self.snapshot_block_chunk(key) {
                inputs.push(snapshot);
            }
        }
        if inputs.is_empty() {
            return;
        }

        let job_id = self.block_mesh_job_next_id;
        self.block_mesh_job_next_id += 1;
        let outputs = build_block_chunk_outputs(inputs);
        self.apply_block_chunk_build_result(BlockChunkBuildResult { id: job_id, outputs });
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

    /// Place a building block at the preview position
    fn place_building_block(&mut self) {
        let _ = self.apply_voxel_primary_action();
    }

    fn update_voxel_target(&mut self) {
        if !self.voxel_hud.visible {
            self.voxel_hud.target_hit = None;
            return;
        }
        let Some(scene) = self.scene.as_ref() else {
            self.voxel_hud.target_hit = None;
            return;
        };
        let Some(gpu) = self.gpu.as_ref() else {
            self.voxel_hud.target_hit = None;
            return;
        };
        let mouse_pos = (
            gpu.surface_config.width as f32 * 0.5,
            gpu.surface_config.height as f32 * 0.5,
        );
        let (ray_origin, ray_dir) = self.screen_to_ray(mouse_pos.0, mouse_pos.1);
        self.voxel_hud.target_hit = scene.building.raycast_voxel(ray_origin, ray_dir, 96.0);
    }

    fn apply_voxel_primary_action(&mut self) -> bool {
        let Some(gpu) = self.gpu.as_ref() else {
            return false;
        };
        let mouse_pos = (
            gpu.surface_config.width as f32 * 0.5,
            gpu.surface_config.height as f32 * 0.5,
        );
        let (ray_origin, ray_dir) = self.screen_to_ray(mouse_pos.0, mouse_pos.1);

        let hit = self
            .scene
            .as_ref()
            .and_then(|s| s.building.raycast_voxel(ray_origin, ray_dir, 96.0));
        let ground_coord = self.find_ground_voxel_coord(ray_origin, ray_dir, 96.0);
        let mode = self.voxel_hud.mode;
        let material = VoxelMaterialId(self.voxel_hud.selected_material());
        let params = self.castle_tool_params();
        let Some(scene) = self.scene.as_mut() else {
            return false;
        };

        match mode {
            BuildMode::Place => {
                if let Some(hit) = hit {
                    let coord = VoxelCoord::new(
                        hit.coord.x + hit.normal.x,
                        hit.coord.y + hit.normal.y,
                        hit.coord.z + hit.normal.z,
                    );
                    scene.building.place_voxel(coord, material)
                } else if let Some(coord) = ground_coord {
                    scene.building.place_voxel(coord, material)
                } else {
                    false
                }
            }
            BuildMode::Remove => {
                if let Some(hit) = hit {
                    scene.building.remove_voxel(hit.coord)
                } else {
                    false
                }
            }
            BuildMode::CornerBrush => {
                if let Some(hit) = hit {
                    let anchor = VoxelCoord::new(
                        hit.coord.x + hit.normal.x,
                        hit.coord.y + hit.normal.y,
                        hit.coord.z + hit.normal.z,
                    );
                    scene.building.place_corner_brush(
                        anchor,
                        hit.normal,
                        self.voxel_hud.corner_radius_vox,
                        material,
                    ) > 0
                } else {
                    false
                }
            }
            BuildMode::BasePlateRect | BuildMode::WallLine => {
                let target = hit
                    .map(|h| VoxelCoord::new(h.coord.x + h.normal.x, h.coord.y + h.normal.y, h.coord.z + h.normal.z))
                    .or(ground_coord);
                let Some(target) = target else {
                    return false;
                };
                if self.voxel_hud.tool_anchor_a.is_none() {
                    self.voxel_hud.tool_anchor_a = Some(target);
                    return false;
                }
                let anchor_a = self.voxel_hud.tool_anchor_a.take().unwrap_or(target);
                match mode {
                    BuildMode::BasePlateRect => {
                        let res = scene
                            .building
                            .build_base_plate_rect(anchor_a, target, material, params);
                        res.applied > 0
                    }
                    BuildMode::WallLine => {
                        let res = scene
                            .building
                            .build_wall_line(anchor_a, target, material, params);
                        res.applied > 0
                    }
                    _ => false,
                }
            }
            BuildMode::BasePlateCircle => {
                let target = hit
                    .map(|h| VoxelCoord::new(h.coord.x + h.normal.x, h.coord.y + h.normal.y, h.coord.z + h.normal.z))
                    .or(ground_coord);
                let Some(target) = target else {
                    return false;
                };
                if self.voxel_hud.tool_anchor_a.is_none() {
                    self.voxel_hud.tool_anchor_a = Some(target);
                    return false;
                }
                let center = self.voxel_hud.tool_anchor_a.take().unwrap_or(target);
                let radius = Self::compute_radius_from_anchor(center, target);
                let res = scene
                    .building
                    .build_base_plate_circle(center, radius, material, params);
                res.applied > 0
            }
            BuildMode::WallRing => {
                let target = hit
                    .map(|h| VoxelCoord::new(h.coord.x + h.normal.x, h.coord.y + h.normal.y, h.coord.z + h.normal.z))
                    .or(ground_coord);
                let Some(target) = target else {
                    return false;
                };
                if self.voxel_hud.tool_anchor_a.is_none() {
                    self.voxel_hud.tool_anchor_a = Some(target);
                    return false;
                }
                let center = self.voxel_hud.tool_anchor_a.take().unwrap_or(target);
                let radius = Self::compute_radius_from_anchor(center, target);
                let res = scene
                    .building
                    .build_wall_ring(center, radius, material, params);
                res.applied > 0
            }
            BuildMode::JointColumn => {
                let anchor = hit
                    .map(|h| VoxelCoord::new(h.coord.x + h.normal.x, h.coord.y + h.normal.y, h.coord.z + h.normal.z))
                    .or(ground_coord);
                let Some(anchor) = anchor else {
                    return false;
                };
                let res = scene.building.build_joint_column(
                    anchor,
                    self.voxel_hud.wall_height_vox,
                    self.voxel_hud.joint_radius_vox,
                    material,
                );
                res.applied > 0
            }
        }
    }

    fn apply_voxel_secondary_action(&mut self) -> bool {
        let Some(gpu) = self.gpu.as_ref() else {
            return false;
        };
        let mouse_pos = (
            gpu.surface_config.width as f32 * 0.5,
            gpu.surface_config.height as f32 * 0.5,
        );
        let (ray_origin, ray_dir) = self.screen_to_ray(mouse_pos.0, mouse_pos.1);
        let hit = self
            .scene
            .as_ref()
            .and_then(|s| s.building.raycast_voxel(ray_origin, ray_dir, 96.0));
        let Some(scene) = self.scene.as_mut() else {
            return false;
        };
        self.voxel_hud.tool_anchor_a = None;
        if let Some(hit) = hit {
            scene.building.remove_voxel(hit.coord)
        } else {
            false
        }
    }

    fn castle_tool_params(&self) -> CastleToolParams {
        CastleToolParams {
            wall_height_vox: self.voxel_hud.wall_height_vox,
            wall_thickness_vox: self.voxel_hud.wall_thickness_vox,
            plate_thickness_vox: self.voxel_hud.plate_thickness_vox,
            joint_spacing_vox: self.voxel_hud.joint_spacing_vox,
            joint_radius_vox: self.voxel_hud.joint_radius_vox,
            rib_spacing_vox: self.voxel_hud.rib_spacing_vox,
            rib_levels: [0.33, 0.66],
        }
    }

    fn compute_radius_from_anchor(center: VoxelCoord, edge: VoxelCoord) -> u8 {
        let dx = (edge.x - center.x) as f32;
        let dz = (edge.z - center.z) as f32;
        let d = (dx * dx + dz * dz).sqrt().round() as i32;
        d.clamp(1, 64) as u8
    }

    fn estimate_projected_voxel_count(&self) -> usize {
        let Some(hit) = self.voxel_hud.target_hit else {
            return 0;
        };
        let target = VoxelCoord::new(
            hit.coord.x + hit.normal.x,
            hit.coord.y + hit.normal.y,
            hit.coord.z + hit.normal.z,
        );
        let h = self.voxel_hud.wall_height_vox.max(1) as usize;
        let t = self.voxel_hud.wall_thickness_vox.max(1) as usize;
        let p = self.voxel_hud.plate_thickness_vox.max(1) as usize;
        let mut r = self.voxel_hud.ring_radius_vox.max(1) as usize;
        let jr = self.voxel_hud.joint_radius_vox.max(1) as usize;
        if let Some(center) = self.voxel_hud.tool_anchor_a
            && matches!(self.voxel_hud.mode, BuildMode::BasePlateCircle | BuildMode::WallRing)
        {
            r = Self::compute_radius_from_anchor(center, target) as usize;
        }

        match self.voxel_hud.mode {
            BuildMode::Place | BuildMode::Remove => 1,
            BuildMode::CornerBrush => {
                let rr = self.voxel_hud.corner_radius_vox.max(1) as f32;
                ((4.0 / 3.0) * std::f32::consts::PI * rr * rr * rr * 0.5) as usize
            }
            BuildMode::BasePlateRect => {
                if let Some(a) = self.voxel_hud.tool_anchor_a {
                    let dx = (a.x - target.x).unsigned_abs() as usize + 1;
                    let dz = (a.z - target.z).unsigned_abs() as usize + 1;
                    dx * dz * p
                } else {
                    p
                }
            }
            BuildMode::BasePlateCircle => (std::f32::consts::PI * (r * r) as f32) as usize * p,
            BuildMode::WallLine => {
                if let Some(a) = self.voxel_hud.tool_anchor_a {
                    let dx = (a.x - target.x).unsigned_abs() as f32;
                    let dz = (a.z - target.z).unsigned_abs() as f32;
                    let len = dx.max(dz).max(1.0) as usize;
                    len * t.max(1) * h.max(1)
                } else {
                    t * h
                }
            }
            BuildMode::WallRing => ((2.0 * std::f32::consts::PI * r as f32) as usize) * t * h,
            BuildMode::JointColumn => {
                (std::f32::consts::PI * (jr * jr) as f32).ceil() as usize * h
            }
        }
    }

    fn find_ground_voxel_coord(&self, ray_origin: Vec3, ray_dir: Vec3, max_dist: f32) -> Option<VoxelCoord> {
        let mut t = 0.25f32;
        while t <= max_dist {
            let p = ray_origin + ray_dir * t;
            if let Some(terrain_y) = self.sample_build_ground_height(p.x, p.z)
                && p.y <= terrain_y + VOXEL_SIZE_METERS * 0.5
            {
                let x = (p.x / VOXEL_SIZE_METERS).floor() as i32;
                let z = (p.z / VOXEL_SIZE_METERS).floor() as i32;
                let y = (terrain_y / VOXEL_SIZE_METERS).floor() as i32;
                return Some(VoxelCoord::new(x, y, z));
            }
            t += VOXEL_SIZE_METERS.max(0.05);
        }
        None
    }

    fn generate_block_preview_mesh(
        &self,
        instances: &[(Vec3, BuildingBlockShape)],
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
        let block_chunk_draw_count = self.prepare_block_chunk_draw_commands();

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
        self.render_meshes(
            &mut encoder,
            &gpu.scene_hdr_view,
            dynamic_index_count,
            block_chunk_draw_count,
        );
        self.render_voxel_shell(&mut encoder, &gpu.scene_hdr_view);
        if gpu.voxel_shell_enabled {
            draw_calls += 1;
        }
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

        let shell_uniforms = VoxelShellUniforms {
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: self.camera.position.to_array(),
            _pad0: 0.0,
            resolution: [config.width as f32, config.height as f32],
            node_count: gpu.brick_tree_buffers.node_count,
            leaf_count: gpu.brick_tree_buffers.leaf_count,
            sun_dir: vis.sun_direction.to_array(),
            _pad1: 0.0,
        };
        queue.write_buffer(
            &gpu.voxel_shell_uniform_buffer,
            0,
            bytemuck::cast_slice(&[shell_uniforms]),
        );

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

    fn prepare_block_chunk_draw_commands(&mut self) -> u32 {
        let Some(gpu) = self.gpu.as_mut() else {
            return 0;
        };

        let mut commands: Vec<DrawIndexedIndirectCommand> =
            Vec::with_capacity(gpu.block_chunk_buffers.len());

        for chunk in &gpu.block_chunk_buffers {
            if chunk.instance_count == 0 {
                continue;
            }
            commands.push(DrawIndexedIndirectCommand {
                index_count: 6,
                instance_count: chunk.instance_count,
                first_index: 0,
                base_vertex: 0,
                first_instance: chunk.first_instance,
            });
        }

        let visible_count = commands.len() as u32;
        if visible_count == 0 && !gpu.block_chunk_buffers.is_empty() {
            // Fail-safe: if culling/pathing logic regresses, draw a small front slice anyway.
            let fallback_count = gpu
                .block_chunk_buffers
                .len()
                .min(128);
            commands.reserve(fallback_count);
            for chunk in gpu.block_chunk_buffers.iter().take(fallback_count) {
                if chunk.instance_count == 0 {
                    continue;
                }
                commands.push(DrawIndexedIndirectCommand {
                    index_count: 6,
                    instance_count: chunk.instance_count,
                    first_index: 0,
                    base_vertex: 0,
                    first_instance: chunk.first_instance,
                });
            }
            if commands.is_empty() {
                return 0;
            }
        }
        let visible_count = commands.len() as u32;

        if visible_count > gpu.block_chunk_indirect_capacity {
            gpu.block_chunk_indirect_capacity = visible_count.next_power_of_two();
            gpu.block_chunk_indirect_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Block Chunk Indirect Buffer"),
                size: (gpu.block_chunk_indirect_capacity as u64)
                    * (std::mem::size_of::<DrawIndexedIndirectCommand>() as u64),
                usage: wgpu::BufferUsages::INDIRECT | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }

        gpu.queue.write_buffer(
            &gpu.block_chunk_indirect_buffer,
            0,
            bytemuck::cast_slice(&commands),
        );
        visible_count
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
        block_chunk_draw_count: u32,
    ) {
        let gpu = self.gpu.as_ref().unwrap();

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

        if block_chunk_draw_count > 0 && !gpu.voxel_shell_enabled {
            render_pass.set_pipeline(&gpu.block_chunk_pipeline);
            render_pass.set_bind_group(0, &gpu.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, gpu.block_chunk_vertex_buffer.slice(..));
            render_pass.set_vertex_buffer(1, gpu.block_chunk_instance_buffer.slice(..));
            render_pass.set_index_buffer(
                gpu.block_chunk_index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );

            if gpu.block_chunk_use_multi_draw {
                render_pass.multi_draw_indexed_indirect(
                    &gpu.block_chunk_indirect_buffer,
                    0,
                    block_chunk_draw_count,
                );
            } else {
                for chunk in &gpu.block_chunk_buffers {
                    if chunk.instance_count == 0 {
                        continue;
                    }
                    let start = chunk.first_instance;
                    let end = chunk.first_instance + chunk.instance_count;
                    render_pass.draw_indexed(0..6, 0, start..end);
                }
            }
            render_pass.set_pipeline(&gpu.pipeline);
            render_pass.set_bind_group(0, &gpu.uniform_bind_group, &[]);
        }

        // Block placement preview / drag hologram
        let mut preview_positions = Vec::new();
        if self.voxel_hud.visible
            && let Some(hit) = self.voxel_hud.target_hit
        {
            let center = match self.voxel_hud.mode {
                BuildMode::Place
                | BuildMode::CornerBrush
                | BuildMode::BasePlateRect
                | BuildMode::BasePlateCircle
                | BuildMode::WallLine
                | BuildMode::WallRing
                | BuildMode::JointColumn => Vec3::new(
                    (hit.coord.x + hit.normal.x) as f32 * VOXEL_SIZE_METERS + VOXEL_SIZE_METERS * 0.5,
                    (hit.coord.y + hit.normal.y) as f32 * VOXEL_SIZE_METERS + VOXEL_SIZE_METERS * 0.5,
                    (hit.coord.z + hit.normal.z) as f32 * VOXEL_SIZE_METERS + VOXEL_SIZE_METERS * 0.5,
                ),
                BuildMode::Remove => Vec3::new(
                    hit.coord.x as f32 * VOXEL_SIZE_METERS + VOXEL_SIZE_METERS * 0.5,
                    hit.coord.y as f32 * VOXEL_SIZE_METERS + VOXEL_SIZE_METERS * 0.5,
                    hit.coord.z as f32 * VOXEL_SIZE_METERS + VOXEL_SIZE_METERS * 0.5,
                ),
            };
            preview_positions.push(center);
            if let Some(anchor) = self.voxel_hud.tool_anchor_a {
                preview_positions.push(Vec3::new(
                    anchor.x as f32 * VOXEL_SIZE_METERS + VOXEL_SIZE_METERS * 0.5,
                    anchor.y as f32 * VOXEL_SIZE_METERS + VOXEL_SIZE_METERS * 0.5,
                    anchor.z as f32 * VOXEL_SIZE_METERS + VOXEL_SIZE_METERS * 0.5,
                ));
            }
        }

        if !preview_positions.is_empty() {
            let mut preview_instances: Vec<(Vec3, battle_tok_engine::render::BuildingBlockShape)> =
                Vec::new();
            let shape = battle_tok_engine::render::BuildingBlockShape::Cube {
                half_extents: Vec3::splat(VOXEL_SIZE_METERS * 0.5),
            };
            preview_instances.extend(preview_positions.iter().copied().map(|p| (p, shape)));

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

    fn render_voxel_shell(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let gpu = self.gpu.as_ref().unwrap();
        if !gpu.voxel_shell_enabled || gpu.brick_tree_buffers.node_count == 0 {
            return;
        }

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Voxel Shell Pass"),
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
        render_pass.set_pipeline(&gpu.voxel_shell_pipeline);
        render_pass.set_bind_group(0, &gpu.voxel_shell_bind_group, &[]);
        render_pass.draw(0..3, 0..1);
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
        if self.voxel_hud.visible && !self.start_overlay.visible {
            let crosshair_mesh = self.generate_crosshair_mesh(w, h);
            self.draw_ui_mesh(encoder, view, "Crosshair Pass", &crosshair_mesh);
            let hud_mesh = self.generate_voxel_hud_mesh(w, h);
            self.draw_ui_mesh(encoder, view, "Voxel HUD Pass", &hud_mesh);
        }

        // Top bar HUD
        if scene.game_state.top_bar.visible && !self.start_overlay.visible && !self.voxel_hud.visible {
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
        let (cx, cy) = (w / 2.0, h / 2.0);
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

    fn generate_voxel_hud_mesh(&self, w: f32, h: f32) -> Mesh {
        let mut verts = Vec::new();
        let mut idxs = Vec::new();
        let to_ndc =
            |x: f32, y: f32| -> [f32; 3] { [(x / w) * 2.0 - 1.0, 1.0 - (y / h) * 2.0, 0.0] };

        let panel_w = 420.0;
        let panel_h = 74.0;
        let px = 18.0;
        let py = h - panel_h - 18.0;
        add_quad(
            &mut verts,
            &mut idxs,
            to_ndc(px, py),
            to_ndc(px + panel_w, py),
            to_ndc(px + panel_w, py + panel_h),
            to_ndc(px, py + panel_h),
            [0.02, 0.03, 0.04, 0.22],
        );

        let mode_text = format!(
            "MODE {:?}  MAT {}  PROJ {}",
            self.voxel_hud.mode,
            self.voxel_hud.selected_material(),
            self.estimate_projected_voxel_count()
        );
        let params_text = format!(
            "H {}  T {}  PLATE {}  R {}  JR {}",
            self.voxel_hud.wall_height_vox,
            self.voxel_hud.wall_thickness_vox,
            self.voxel_hud.plate_thickness_vox,
            self.voxel_hud.ring_radius_vox,
            self.voxel_hud.joint_radius_vox
        );
        let anchor_text = if let Some(anchor) = self.voxel_hud.tool_anchor_a {
            format!("ANCHOR A {},{},{}", anchor.x, anchor.y, anchor.z)
        } else {
            "ANCHOR A -".to_string()
        };
        let target_text = if let Some(hit) = self.voxel_hud.target_hit {
            let hp = self
                .scene
                .as_ref()
                .and_then(|s| s.building.voxel_runtime.world.get(hit.coord))
                .map(|c| format!("{}/{}", c.hp, c.max_hp))
                .unwrap_or_else(|| "-".to_string());
            format!(
                "TARGET {},{},{} HP {}",
                hit.coord.x, hit.coord.y, hit.coord.z, hp
            )
        } else {
            "TARGET -".to_string()
        };

        draw_text(
            &mut verts,
            &mut idxs,
            &mode_text,
            px + 14.0,
            py + 10.0,
            2.2,
            [0.90, 0.95, 1.0, 1.0],
            w,
            h,
        );
        draw_text(
            &mut verts,
            &mut idxs,
            &params_text,
            px + 14.0,
            py + 31.0,
            2.0,
            [0.82, 1.0, 0.87, 1.0],
            w,
            h,
        );
        draw_text(
            &mut verts,
            &mut idxs,
            &anchor_text,
            px + 14.0,
            py + 51.0,
            1.8,
            [0.97, 0.91, 0.74, 1.0],
            w,
            h,
        );
        draw_text(
            &mut verts,
            &mut idxs,
            &target_text,
            px + 220.0,
            py + 51.0,
            1.8,
            [1.0, 1.0, 1.0, 1.0],
            w,
            h,
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
                // Battle runtime build UI is voxel-only.
                self.builder_mode.enabled = false;
                self.builder_mode.cursor_coord = None;
                self.voxel_hud.toggle();
                self.voxel_hud.tool_anchor_a = None;
                if let Some(window) = &self.window {
                    if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                        let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                    }
                    window.set_cursor_visible(false);
                }
            }

            KeyCode::Tab if pressed && self.voxel_hud.visible => self.voxel_hud.cycle_mode(),
            KeyCode::KeyQ | KeyCode::KeyM if pressed && self.voxel_hud.visible => {
                self.voxel_hud.cycle_mode()
            }
            KeyCode::Digit1 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(0),
            KeyCode::Digit2 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(1),
            KeyCode::Digit3 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(2),
            KeyCode::Digit4 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(3),
            KeyCode::Digit5 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(4),
            KeyCode::Digit6 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(5),
            KeyCode::Digit7 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(6),
            KeyCode::Digit8 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(7),
            KeyCode::Digit9 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(8),
            KeyCode::Digit0 if pressed && self.voxel_hud.visible => self.voxel_hud.select_slot(9),
            KeyCode::ArrowUp if pressed && self.voxel_hud.visible => {
                if self.movement.sprint {
                    self.voxel_hud.adjust_height_param(1);
                } else {
                    self.voxel_hud.adjust_primary_param(1);
                }
            }
            KeyCode::ArrowDown if pressed && self.voxel_hud.visible => {
                if self.movement.sprint {
                    self.voxel_hud.adjust_height_param(-1);
                } else {
                    self.voxel_hud.adjust_primary_param(-1);
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
            KeyCode::F6 if pressed => {
                self.use_voxel_shell = !self.use_voxel_shell;
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.voxel_shell_enabled = false;
                }
                println!(
                    "[VoxelShell] {}",
                    if self.use_voxel_shell {
                        "enabled"
                    } else {
                        "disabled (proxy voxel mesh)"
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

                        if self.voxel_hud.visible && pressed {
                            self.place_building_block();
                        }
                    }
                    MouseButton::Right => {
                        let pressed = state == ElementState::Pressed;
                        if self.voxel_hud.visible {
                            if pressed {
                                let _ = self.apply_voxel_secondary_action();
                            }
                            self.mouse_pressed = false;
                        } else {
                            self.mouse_pressed = pressed;
                        }
                    }
                    MouseButton::Middle => {
                        if state == ElementState::Pressed && self.voxel_hud.visible {
                            self.voxel_hud.adjust_primary_param(1);
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
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                };

                if self.voxel_hud.visible {
                    let delta = if scroll > 0.0 {
                        1
                    } else if scroll < 0.0 {
                        -1
                    } else {
                        0
                    };
                    if delta != 0 {
                        if self.movement.sprint {
                            self.voxel_hud.adjust_height_param(delta);
                        } else {
                            self.voxel_hud.adjust_primary_param(delta);
                        }
                    }
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
                        let mode_str = if self.voxel_hud.visible {
                            let proj = self.estimate_projected_voxel_count();
                            format!(
                                "Build-{:?}-mat{}-r{}-h{}-proj{}",
                                self.voxel_hud.mode,
                                self.voxel_hud.selected_material(),
                                self.voxel_hud.ring_radius_vox,
                                self.voxel_hud.wall_height_vox,
                                proj
                            )
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
        if let DeviceEvent::MouseMotion { delta } = event {
            self.camera
                .handle_mouse_look(delta.0 as f32, delta.1 as f32);
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
