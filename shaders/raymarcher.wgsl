// Sketch Engine - Main Ray Marcher Shader
// This shader performs fullscreen ray marching against SDF entities

// ============================================================================
// UNIFORM BINDINGS
// ============================================================================

// Uniforms struct layout (192 bytes total):
// - view_proj:          mat4x4<f32> @ offset 0   (64 bytes)
// - inv_view_proj:      mat4x4<f32> @ offset 64  (64 bytes)
// - camera_pos:         vec3<f32>   @ offset 128 (12 bytes)
// - time:               f32         @ offset 140 (4 bytes)
// - resolution:         vec2<f32>   @ offset 144 (8 bytes)
// - step_count:         u32         @ offset 152 (4 bytes)
// - lod_debug_mode:     u32         @ offset 156 (4 bytes)
// - preview_position:   vec3<f32>   @ offset 160 (12 bytes)
// - preview_sdf_type:   u32         @ offset 172 (4 bytes)
// - preview_enabled:    u32         @ offset 176 (4 bytes)
// - entity_debug_mode:  u32         @ offset 180 (4 bytes) - US-0M04: 0=off, 1=position RGB, 2=count brightness
// - terrain_visible:    u32         @ offset 184 (4 bytes) - US-0N01: 1=visible, 0=hidden (F1 toggle)
// - camera_mode:        u32         @ offset 188 (4 bytes) - US-019: 0=third-person, 1=first-person
// Total: 192 bytes
//
// IMPORTANT: We use separate u32 fields for padding instead of vec3<u32> or array<u32, 3>
// because:
// - vec3<u32> has 16-byte alignment in WGSL, causing a size mismatch (208 vs 192 bytes)
// - array<u32, 3> in uniform buffers requires 16-byte stride alignment
// The Rust struct uses [u32; 2] which has 4-byte alignment, so separate u32 fields match.
struct Uniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,  // Inverse view-projection for ray direction calculation
    camera_pos: vec3<f32>,
    time: f32,
    resolution: vec2<f32>,
    step_count: u32,
    lod_debug_mode: u32,  // 1 = enabled, 0 = disabled (also used for SDF debug modes 2-7)
    preview_position: vec3<f32>,  // World-space position for placement preview
    preview_sdf_type: u32,        // SDF type for preview (0 = sphere, 1 = box, 2 = capsule)
    preview_enabled: u32,         // Whether preview is enabled (1 = enabled, 0 = disabled)
    entity_debug_mode: u32,       // US-0M04: Entity debug mode (0=off, 1=position as RGB, 2=count as brightness)
    terrain_visible: u32,         // US-0N01: Whether terrain is visible (1 = visible, 0 = hidden via F1 toggle)
    camera_mode: u32,             // US-019: Camera mode (0 = third-person, 1 = first-person)
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// ============================================================================
// ENTITY STORAGE BUFFER
// ============================================================================

// SDF type constants
const SDF_SPHERE: u32 = 0u;
const SDF_BOX: u32 = 1u;
const SDF_CAPSULE: u32 = 2u;

// GpuEntity layout (96 bytes total, matches Rust GpuEntity):
// IMPORTANT: We use scalar fields (position_x, position_y, position_z) instead of vec3<f32>
// because in WGSL storage buffers, vec3<f32> has 16-byte alignment which would cause
// the struct to be 128 bytes instead of 96 bytes.
// The Rust struct uses [f32; 3] which has 4-byte alignment, so scalar fields match.
//
// Layout (total 96 bytes, 6 rows of 16 bytes each):
// Row 0 (offset 0-15):  position_x, position_y, position_z, sdf_type
// Row 1 (offset 16-31): scale_x, scale_y, scale_z, seed
// Row 2 (offset 32-47): rotation (vec4)
// Row 3 (offset 48-63): color_r, color_g, color_b, roughness
// Row 4 (offset 64-79): metallic, selected, lod_octaves, use_noise
// Row 5 (offset 80-95): noise_amplitude, noise_frequency, noise_octaves, _padding
//
// Byte offset summary (for debugging buffer mismatches):
//   offset  0: position_x (f32)
//   offset  4: position_y (f32)
//   offset  8: position_z (f32)
//   offset 12: sdf_type (u32)
//   offset 16: scale_x (f32)
//   offset 20: scale_y (f32)
//   offset 24: scale_z (f32)
//   offset 28: seed (f32)
//   offset 32: rotation.x (f32)
//   offset 36: rotation.y (f32)
//   offset 40: rotation.z (f32)
//   offset 44: rotation.w (f32)
//   offset 48: color_r (f32)
//   offset 52: color_g (f32)
//   offset 56: color_b (f32)
//   offset 60: roughness (f32)
//   offset 64: metallic (f32)
//   offset 68: selected (f32)
//   offset 72: lod_octaves (u32)
//   offset 76: use_noise (u32)
//   offset 80: noise_amplitude (f32)
//   offset 84: noise_frequency (f32)
//   offset 88: noise_octaves (u32)
//   offset 92: baked_sdf_id (u32) - Baked SDF slot ID (0 = not baked, use equation evaluation)
//   offset 96: bake_blend (f32) - Blend factor for smooth transition (US-023)
//   offset 100: _pad6_0 (u32) - Padding
//   offset 104: _pad6_1 (u32) - Padding
//   offset 108: _pad6_2 (u32) - Padding
//   TOTAL: 112 bytes
struct GpuEntity {
    // Row 0: position (3 floats) + sdf_type = 16 bytes (offset 0-15)
    position_x: f32,      // offset 0
    position_y: f32,      // offset 4
    position_z: f32,      // offset 8
    sdf_type: u32,        // offset 12
    // Row 1: scale (3 floats) + seed = 16 bytes (offset 16-31)
    scale_x: f32,         // offset 16
    scale_y: f32,         // offset 20
    scale_z: f32,         // offset 24
    seed: f32,            // offset 28
    // Row 2: rotation quaternion (vec4 is OK, has 16-byte alignment) = 16 bytes (offset 32-47)
    rotation: vec4<f32>,  // offset 32 (vec4 has consistent 16-byte alignment)
    // Row 3: color (3 floats) + roughness = 16 bytes (offset 48-63)
    color_r: f32,         // offset 48
    color_g: f32,         // offset 52
    color_b: f32,         // offset 56
    roughness: f32,       // offset 60
    // Row 4: metallic + selected + lod_octaves + use_noise = 16 bytes (offset 64-79)
    metallic: f32,        // offset 64
    selected: f32,        // offset 68 (1.0 if selected, 0.0 otherwise)
    lod_octaves: u32,     // offset 72 (LOD level octave count 1-8 for noise functions)
    use_noise: u32,       // offset 76 (1 if noise displacement enabled, 0 otherwise)
    // Row 5: noise_amplitude + noise_frequency + noise_octaves + baked_sdf_id = 16 bytes (offset 80-95)
    noise_amplitude: f32, // offset 80 (amplitude of noise displacement)
    noise_frequency: f32, // offset 84 (frequency multiplier for noise)
    noise_octaves: u32,   // offset 88 (number of octaves for FBM noise 1-8)
    baked_sdf_id: u32,    // offset 92 (baked SDF slot ID, 0 = not baked, use equation evaluation)
    // Row 6: bake transition + precision + padding = 16 bytes (offset 96-111) (US-023, US-031)
    bake_blend: f32,      // offset 96 (blend factor: 0.0 = equation only, 1.0 = baked only)
    precision_class: u32, // offset 100 (US-031: 0=Player(1.0x), 1=Interactive(1.2x), 2=Static(1.5x), 3=Terrain(2.0x))
    _pad6_1: u32,         // offset 104 (padding)
    _pad6_2: u32,         // offset 108 (padding)
}

// Helper functions to extract vec3 from GpuEntity scalar fields
fn get_entity_position(entity: GpuEntity) -> vec3<f32> {
    return vec3<f32>(entity.position_x, entity.position_y, entity.position_z);
}

fn get_entity_scale(entity: GpuEntity) -> vec3<f32> {
    return vec3<f32>(entity.scale_x, entity.scale_y, entity.scale_z);
}

fn get_entity_color(entity: GpuEntity) -> vec3<f32> {
    return vec3<f32>(entity.color_r, entity.color_g, entity.color_b);
}

// ============================================================================
// PRECISION CLASS AND ADAPTIVE STEP (US-031, US-032, US-038)
// ============================================================================

/// Get precision multiplier from precision class enum value (US-031).
/// 0=Player(1.0x), 1=Interactive(1.2x), 2=Static(1.5x), 3=Terrain(2.0x)
fn get_precision_multiplier(precision_class: u32) -> f32 {
    switch precision_class {
        case 0u: { return 1.0; }  // Player - highest precision
        case 1u: { return 1.2; }  // Interactive
        case 2u: { return 1.5; }  // Static (default)
        case 3u: { return 2.0; }  // Terrain - lowest precision
        default: { return 1.5; }  // Default to Static
    }
}

/// Calculate base step size for ray marching based on distance from camera (US-032).
///
/// Distance bands with smooth linear interpolation:
/// - < 5m:    0.1 to 0.5 (high precision for nearby objects)
/// - 5-50m:  0.5 to 2.0 (medium precision for mid-range)
/// - > 50m:  2.0 to 5.0 (lower precision for distant objects, clamped at 200m)
fn base_step_for_distance(distance: f32) -> f32 {
    let d = max(distance, 0.0);

    if (d < 5.0) {
        // Close range: 0.1 at 0m, 0.5 at 5m
        let t = d / 5.0;
        return mix(0.1, 0.5, t);
    } else if (d < 50.0) {
        // Mid range: 0.5 at 5m, 2.0 at 50m
        let t = (d - 5.0) / 45.0;
        return mix(0.5, 2.0, t);
    } else {
        // Far range: 2.0 at 50m, 5.0 at 200m (clamped)
        let t = min((d - 50.0) / 150.0, 1.0);
        return mix(2.0, 5.0, t);
    }
}

/// Adaptive step size calculation combining distance-based minimum with SDF distance (US-038).
///
/// The step size is computed as:
///   step = max(base_step_for_distance(distance), sdf_dist) * precision_multiplier
///
/// This ensures:
/// 1. A minimum step size based on distance from camera (prevents over-sampling distant objects)
/// 2. Larger steps when far from surfaces (SDF distance is large)
/// 3. Precision class multiplier adjusts for entity type (player needs higher precision than terrain)
///
/// Arguments:
/// - distance: Current ray distance from camera (in meters)
/// - sdf_dist: Distance to nearest SDF surface (in meters)
/// - precision: Precision class multiplier (1.0 = highest, 2.0 = lowest)
///
/// Returns: Adaptive step size in meters
fn adaptive_step(distance: f32, sdf_dist: f32, precision: f32) -> f32 {
    // Get the distance-based minimum step size
    let min_step = base_step_for_distance(distance);
    // Combine with SDF distance - use whichever is larger
    let combined_step = max(min_step, sdf_dist);
    // Apply precision class multiplier
    return combined_step * precision;
}

// EntityBuffer layout (matches Rust EntityBufferHeader + array<GpuEntity>):
// - count: u32 (4 bytes)
// - _padding0-2: 3x u32 (12 bytes) - NOT vec3<u32> to avoid 16-byte alignment issues
// Total header: 16 bytes
// Note: We use separate u32 fields instead of vec3<u32> because vec3 has 16-byte
// alignment in WGSL, which would push the array start to offset 32 instead of 16.
struct EntityBuffer {
    count: u32,
    _padding0: u32,
    _padding1: u32,
    _padding2: u32,
    entities: array<GpuEntity>,
}

@group(0) @binding(1)
var<storage, read> entity_buffer: EntityBuffer;

// ============================================================================
// TERRAIN CONFIGURATION
// ============================================================================

// TerrainConfig layout (matches Rust GpuTerrainConfig):
// - amplitude: f32 (4 bytes)   offset 0
// - frequency: f32 (4 bytes)   offset 4
// - octaves: u32 (4 bytes)     offset 8
// - enabled: u32 (4 bytes)     offset 12
// - seed: f32 (4 bytes)        offset 16
// - _pad1_0-2: 3x u32 (12 bytes) offset 20
// - _padding_0-2: 3x u32 (12 bytes) offset 32
// - _pad2: u32 (4 bytes)       offset 44
// Total: 48 bytes
struct TerrainConfig {
    amplitude: f32,
    frequency: f32,
    octaves: u32,
    enabled: u32,
    seed: f32,
    _pad1_0: u32,
    _pad1_1: u32,
    _pad1_2: u32,
    _padding_0: u32,
    _padding_1: u32,
    _padding_2: u32,
    _pad2: u32,
}

@group(0) @binding(2)
var<uniform> terrain_config: TerrainConfig;

// ============================================================================
// BAKED SDF TEXTURE ARRAY
// ============================================================================
//
// Baked SDFs are stored as a 2D texture array where each SDF occupies 64 consecutive layers.
// - Texture dimensions: 64 × 64 (XY slices)
// - Array layers: 64 per SDF × 256 max SDFs = 16384 total layers
// - Format: R16Float (signed distance values)
// - Each SDF ID maps to layers [id * 64, id * 64 + 63]
//
// Trilinear interpolation is performed manually:
// 1. Hardware bilinear interpolation on XY (via sampler)
// 2. Manual linear interpolation on Z (between adjacent layers)

// Resolution of each baked SDF volume (64³ voxels)
const BAKED_SDF_RESOLUTION: f32 = 64.0;

// Baked SDF brick cache SSBO (group 1 to avoid conflicts with main bind group)
@group(1) @binding(0)
var<storage, read> brick_cache: array<f32>;

// ============================================================================
// TILE CULLING BUFFER (US-017)
// ============================================================================
//
// Tile-based culling data from compute shader (US-010).
// The screen is divided into 16×16 pixel tiles, and each tile stores
// up to 32 entity indices that potentially overlap that tile.
//
// This allows the raymarcher to only evaluate entities relevant to
// each pixel's tile, dramatically reducing SDF evaluations.
//
// Binding group 2 is used to separate from main render and baked SDF bindings.

const TILE_SIZE: u32 = 16u;
const MAX_ENTITIES_PER_TILE: u32 = 32u;

// TileData for read-only access (no atomics in fragment shader)
// Matches WGSL TileData struct layout from culling.wgsl
struct TileDataReadOnly {
    entity_count: u32,
    _padding: u32,
    entity_indices: array<u32, 32>,
}

// TileBufferReadOnly: Complete tile buffer for reading in fragment shader
struct TileBufferReadOnly {
    tiles_x: u32,
    tiles_y: u32,
    tile_size: u32,
    total_tiles: u32,
    tiles: array<TileDataReadOnly>,
}

@group(2) @binding(0)
var<storage, read> tile_buffer: TileBufferReadOnly;

// Global flag to enable/disable tile culling (set via uniform in future, currently always on)
// When tile_buffer.total_tiles == 0, fallback to full entity list
fn is_tile_culling_enabled() -> bool {
    return tile_buffer.total_tiles > 0u;
}

// Get tile index for a given pixel coordinate
fn get_tile_index_for_pixel(pixel_x: u32, pixel_y: u32) -> u32 {
    let tile_x = pixel_x / TILE_SIZE;
    let tile_y = pixel_y / TILE_SIZE;
    return tile_y * tile_buffer.tiles_x + tile_x;
}

// Get entity count for a tile (clamped to max)
fn get_tile_entity_count(tile_index: u32) -> u32 {
    if (tile_index >= tile_buffer.total_tiles) {
        return 0u;
    }
    return min(tile_buffer.tiles[tile_index].entity_count, MAX_ENTITIES_PER_TILE);
}

// Get entity index from a tile's entity list
fn get_tile_entity_index(tile_index: u32, slot: u32) -> u32 {
    return tile_buffer.tiles[tile_index].entity_indices[slot];
}

// ============================================================================
// FROXEL CULLING BUFFER (US-037)
// ============================================================================
//
// Froxel (frustum + voxel) based 3D culling for efficient raymarching.
// The view frustum is divided into a 3D grid:
// - X/Y: 16×16 screen-space tiles (matching tile culling)
// - Z: 24 exponentially distributed depth slices
// - Total: 6,144 froxels
//
// Each froxel contains a list of SDF indices that potentially intersect it.
// During raymarching, only SDFs in the current froxel are evaluated.

// Froxel grid dimensions (must match froxel_config.rs and froxel_lookup.wgsl)
const FROXEL_TILES_X: u32 = 16u;
const FROXEL_TILES_Y: u32 = 16u;
const FROXEL_DEPTH_SLICES: u32 = 24u;
const TOTAL_FROXELS: u32 = 6144u;  // 16 * 16 * 24
const MAX_SDFS_PER_FROXEL: u32 = 64u;
const FROXEL_INVALID: u32 = 0xFFFFFFFFu;

// FroxelBounds: World-space AABB for a single froxel (32 bytes)
struct FroxelBounds {
    min_x: f32,
    min_y: f32,
    min_z: f32,
    _pad0: u32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
    _pad1: u32,
}

// FroxelBoundsBuffer: Contains bounds for all froxels
struct FroxelBoundsBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    bounds: array<FroxelBounds, 6144>,
}

// FroxelSDFList: Per-froxel list of SDF indices (272 bytes)
// Note: In read-only context, we don't use atomic<u32> for count
struct FroxelSDFListReadOnly {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    sdf_indices: array<u32, 64>,
}

// FroxelSDFListBuffer: Contains SDF lists for all froxels
struct FroxelSDFListBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    lists: array<FroxelSDFListReadOnly, 6144>,
}

// FroxelUniforms: Per-frame froxel system parameters
struct FroxelUniforms {
    // Camera parameters for froxel calculations
    camera_forward_x: f32,
    camera_forward_y: f32,
    camera_forward_z: f32,
    fov_y: f32,
    camera_up_x: f32,
    camera_up_y: f32,
    camera_up_z: f32,
    aspect_ratio: f32,
    camera_right_x: f32,
    camera_right_y: f32,
    camera_right_z: f32,
    near_plane: f32,
    far_plane: f32,
    froxel_enabled: u32,  // 1 = enabled, 0 = disabled
    _pad0: u32,
    _pad1: u32,
}

// Binding group 3 for froxel culling buffers
@group(3) @binding(0)
var<storage, read> froxel_bounds: FroxelBoundsBuffer;

@group(3) @binding(1)
var<storage, read> froxel_sdf_lists: FroxelSDFListBuffer;

@group(3) @binding(2)
var<uniform> froxel_uniforms: FroxelUniforms;

// ============================================================================
// FROXEL HELPER FUNCTIONS
// ============================================================================

// Check if froxel system is enabled
fn is_froxel_culling_enabled() -> bool {
    return froxel_uniforms.froxel_enabled == 1u;
}

// Get camera vectors from froxel uniforms
fn get_froxel_camera_forward() -> vec3<f32> {
    return vec3<f32>(
        froxel_uniforms.camera_forward_x,
        froxel_uniforms.camera_forward_y,
        froxel_uniforms.camera_forward_z
    );
}

fn get_froxel_camera_up() -> vec3<f32> {
    return vec3<f32>(
        froxel_uniforms.camera_up_x,
        froxel_uniforms.camera_up_y,
        froxel_uniforms.camera_up_z
    );
}

fn get_froxel_camera_right() -> vec3<f32> {
    return vec3<f32>(
        froxel_uniforms.camera_right_x,
        froxel_uniforms.camera_right_y,
        froxel_uniforms.camera_right_z
    );
}

// Calculate froxel coordinates from a linear index
fn froxel_index_to_coords(index: u32) -> vec3<u32> {
    let tiles_per_slice = FROXEL_TILES_X * FROXEL_TILES_Y;
    let z = index / tiles_per_slice;
    let remaining = index % tiles_per_slice;
    let y = remaining / FROXEL_TILES_X;
    let x = remaining % FROXEL_TILES_X;
    return vec3<u32>(x, y, z);
}

// Calculate linear froxel index from 3D coordinates
fn froxel_coords_to_index(x: u32, y: u32, z: u32) -> u32 {
    return z * (FROXEL_TILES_X * FROXEL_TILES_Y) + y * FROXEL_TILES_X + x;
}

// Get depth slice near boundary using exponential distribution
fn froxel_slice_near_depth(slice: u32) -> f32 {
    let near = froxel_uniforms.near_plane;
    let far = froxel_uniforms.far_plane;
    let total_slices = f32(FROXEL_DEPTH_SLICES);
    let t = f32(slice) / total_slices;
    let ratio = far / near;
    return near * pow(ratio, t);
}

// Get depth slice far boundary using exponential distribution
fn froxel_slice_far_depth(slice: u32) -> f32 {
    let near = froxel_uniforms.near_plane;
    let far = froxel_uniforms.far_plane;
    let total_slices = f32(FROXEL_DEPTH_SLICES);
    let t = f32(slice + 1u) / total_slices;
    let ratio = far / near;
    return near * pow(ratio, t);
}

// Convert depth to slice index
fn depth_to_froxel_slice(depth: f32) -> u32 {
    let near = froxel_uniforms.near_plane;
    let far = froxel_uniforms.far_plane;

    if depth < near {
        return FROXEL_DEPTH_SLICES; // Invalid
    }
    if depth >= far {
        return FROXEL_DEPTH_SLICES; // Invalid
    }

    let ratio = far / near;
    let t = log(depth / near) / log(ratio);
    let slice = u32(t * f32(FROXEL_DEPTH_SLICES));
    return min(slice, FROXEL_DEPTH_SLICES - 1u);
}

// Get froxel index for a world position
fn get_froxel_for_position(world_pos: vec3<f32>) -> u32 {
    let camera_forward = get_froxel_camera_forward();
    let camera_up = get_froxel_camera_up();
    let camera_right = get_froxel_camera_right();

    // Transform to view space
    let offset = world_pos - uniforms.camera_pos;
    let view_x = dot(offset, camera_right);
    let view_y = dot(offset, camera_up);
    let depth = dot(offset, camera_forward);

    // Check depth bounds
    let near = froxel_uniforms.near_plane;
    let far = froxel_uniforms.far_plane;
    if depth < near || depth >= far {
        return FROXEL_INVALID;
    }

    // Calculate half-extents at this depth
    let half_fov_y = froxel_uniforms.fov_y * 0.5;
    let half_height = depth * tan(half_fov_y);
    let half_width = half_height * froxel_uniforms.aspect_ratio;

    // Convert to NDC (-1 to +1)
    let ndc_x = view_x / half_width;
    let ndc_y = view_y / half_height;

    // Check NDC bounds
    if ndc_x < -1.0 || ndc_x > 1.0 || ndc_y < -1.0 || ndc_y > 1.0 {
        return FROXEL_INVALID;
    }

    // Map NDC to tile indices
    let tile_x = u32((ndc_x + 1.0) * 0.5 * f32(FROXEL_TILES_X));
    let tile_y = u32((ndc_y + 1.0) * 0.5 * f32(FROXEL_TILES_Y));
    let clamped_x = min(tile_x, FROXEL_TILES_X - 1u);
    let clamped_y = min(tile_y, FROXEL_TILES_Y - 1u);

    // Get depth slice
    let slice = depth_to_froxel_slice(depth);
    if slice >= FROXEL_DEPTH_SLICES {
        return FROXEL_INVALID;
    }

    return froxel_coords_to_index(clamped_x, clamped_y, slice);
}

// Get distance along ray to exit current froxel
fn get_froxel_exit_distance(ray_origin: vec3<f32>, ray_dir: vec3<f32>, froxel_idx: u32) -> f32 {
    if froxel_idx >= TOTAL_FROXELS {
        return 10000.0;
    }

    let coords = froxel_index_to_coords(froxel_idx);
    let depth_near = froxel_slice_near_depth(coords.z);
    let depth_far = froxel_slice_far_depth(coords.z);

    let camera_forward = get_froxel_camera_forward();
    let camera_up = get_froxel_camera_up();
    let camera_right = get_froxel_camera_right();

    // Transform ray to view space
    let ray_offset = ray_origin - uniforms.camera_pos;
    let ray_view_x = dot(ray_offset, camera_right);
    let ray_view_y = dot(ray_offset, camera_up);
    let ray_view_z = dot(ray_offset, camera_forward);

    let dir_view_x = dot(ray_dir, camera_right);
    let dir_view_y = dot(ray_dir, camera_up);
    let dir_view_z = dot(ray_dir, camera_forward);

    var min_exit_dist: f32 = 10000.0;

    // Check depth plane intersections
    if abs(dir_view_z) > 0.0001 {
        let t_near = (depth_near - ray_view_z) / dir_view_z;
        if t_near > 0.001 {
            min_exit_dist = min(min_exit_dist, t_near);
        }

        let t_far = (depth_far - ray_view_z) / dir_view_z;
        if t_far > 0.001 {
            min_exit_dist = min(min_exit_dist, t_far);
        }
    }

    // Calculate tile NDC bounds
    let tile_width_ndc = 2.0 / f32(FROXEL_TILES_X);
    let tile_height_ndc = 2.0 / f32(FROXEL_TILES_Y);
    let ndc_left = -1.0 + f32(coords.x) * tile_width_ndc;
    let ndc_right = -1.0 + f32(coords.x + 1u) * tile_width_ndc;
    let ndc_bottom = -1.0 + f32(coords.y) * tile_height_ndc;
    let ndc_top = -1.0 + f32(coords.y + 1u) * tile_height_ndc;

    // Calculate half-angle tangents
    let half_fov_y = froxel_uniforms.fov_y * 0.5;
    let tan_half_fov_y = tan(half_fov_y);
    let tan_half_fov_x = tan_half_fov_y * froxel_uniforms.aspect_ratio;

    // Check frustum plane intersections (left, right, bottom, top)
    // Left plane
    let plane_coeff_left = ndc_left * tan_half_fov_x;
    let denom_left = dir_view_x - dir_view_z * plane_coeff_left;
    if abs(denom_left) > 0.0001 {
        let t = -(ray_view_x - ray_view_z * plane_coeff_left) / denom_left;
        if t > 0.001 {
            min_exit_dist = min(min_exit_dist, t);
        }
    }

    // Right plane
    let plane_coeff_right = ndc_right * tan_half_fov_x;
    let denom_right = dir_view_x - dir_view_z * plane_coeff_right;
    if abs(denom_right) > 0.0001 {
        let t = -(ray_view_x - ray_view_z * plane_coeff_right) / denom_right;
        if t > 0.001 {
            min_exit_dist = min(min_exit_dist, t);
        }
    }

    // Bottom plane
    let plane_coeff_bottom = ndc_bottom * tan_half_fov_y;
    let denom_bottom = dir_view_y - dir_view_z * plane_coeff_bottom;
    if abs(denom_bottom) > 0.0001 {
        let t = -(ray_view_y - ray_view_z * plane_coeff_bottom) / denom_bottom;
        if t > 0.001 {
            min_exit_dist = min(min_exit_dist, t);
        }
    }

    // Top plane
    let plane_coeff_top = ndc_top * tan_half_fov_y;
    let denom_top = dir_view_y - dir_view_z * plane_coeff_top;
    if abs(denom_top) > 0.0001 {
        let t = -(ray_view_y - ray_view_z * plane_coeff_top) / denom_top;
        if t > 0.001 {
            min_exit_dist = min(min_exit_dist, t);
        }
    }

    return min_exit_dist;
}

// Check if a froxel is empty (has no SDFs)
fn is_froxel_empty(froxel_idx: u32) -> bool {
    if froxel_idx >= TOTAL_FROXELS {
        return true;
    }
    return froxel_sdf_lists.lists[froxel_idx].count == 0u;
}

// Get SDF count for a froxel
fn get_froxel_sdf_count(froxel_idx: u32) -> u32 {
    if froxel_idx >= TOTAL_FROXELS {
        return 0u;
    }
    return min(froxel_sdf_lists.lists[froxel_idx].count, MAX_SDFS_PER_FROXEL);
}

// Get SDF index from a froxel's list
fn get_froxel_sdf_index(froxel_idx: u32, slot: u32) -> u32 {
    return froxel_sdf_lists.lists[froxel_idx].sdf_indices[slot];
}

// ============================================================================
// RAY MARCHING CONSTANTS
// ============================================================================

// NOTE: 1 unit = 1 meter (SI units)
const MAX_DIST: f32 = 5000.0;  // 5km visibility for spherical world
const SURFACE_DIST: f32 = 0.001;
const NORMAL_EPSILON: f32 = 0.001;

// ============================================================================
// SPHERICAL WORLD CONSTANTS
// ============================================================================

// World size: 10km x 10km (map_size = 5000, so -5000 to +5000)
const WORLD_MAP_SIZE: f32 = 5000.0;
const WORLD_DIAMETER: f32 = 10000.0;  // 10km

// Planet radius for curvature calculation
// For a 10km "circumference" world: circumference = 20000m (wrap in both X and Z)
// radius = circumference / (2π) ≈ 3183m
const PLANET_RADIUS: f32 = 3183.0;

// Curvature drop formula: drop = distance² / (2 * radius)
// At 100m: drop ≈ 1.57m
// At 500m: drop ≈ 39m
// At 1km: drop ≈ 157m
fn curvature_drop(distance: f32) -> f32 {
    return (distance * distance) / (2.0 * PLANET_RADIUS);
}

// Wrap position for spherical world (like walking around a planet)
fn wrap_world_position(pos: vec3<f32>) -> vec3<f32> {
    var p = pos;
    // Wrap X
    if (p.x > WORLD_MAP_SIZE) {
        p.x = p.x - WORLD_DIAMETER;
    } else if (p.x < -WORLD_MAP_SIZE) {
        p.x = p.x + WORLD_DIAMETER;
    }
    // Wrap Z
    if (p.z > WORLD_MAP_SIZE) {
        p.z = p.z - WORLD_DIAMETER;
    } else if (p.z < -WORLD_MAP_SIZE) {
        p.z = p.z + WORLD_DIAMETER;
    }
    return p;
}

// ============================================================================
// LOD CONSTANTS
// ============================================================================

// LOD level octave counts for adaptive detail rendering
const LOD_FULL_OCTAVES: u32 = 8u;       // Near objects (< 10 units)
const LOD_MEDIUM_OCTAVES: u32 = 4u;     // Medium distance (10-50 units)
const LOD_LOW_OCTAVES: u32 = 2u;        // Far objects (50-200 units)
const LOD_SILHOUETTE_OCTAVES: u32 = 1u; // Very far (>= 200 units)

// Distance thresholds for LOD transitions
const LOD_SILHOUETTE_DISTANCE: f32 = 200.0;  // Distance at which silhouette mode activates
const LOD_SILHOUETTE_BLEND_RANGE: f32 = 50.0; // Blend range for smooth transition to silhouette

// LOD debug visualization colors (specified in task requirements)
// Green = Full detail (< 10 units, 8 octaves)
// Yellow = Medium detail (10-50 units, 4 octaves)
// Orange = Low detail (50-200 units, 2 octaves)
// Red = Silhouette mode (>= 200 units, 1 octave)
const LOD_DEBUG_COLOR_FULL: vec3<f32> = vec3<f32>(0.2, 0.9, 0.2);       // Green
const LOD_DEBUG_COLOR_MEDIUM: vec3<f32> = vec3<f32>(0.95, 0.9, 0.2);    // Yellow
const LOD_DEBUG_COLOR_LOW: vec3<f32> = vec3<f32>(1.0, 0.6, 0.1);        // Orange
const LOD_DEBUG_COLOR_SILHOUETTE: vec3<f32> = vec3<f32>(0.95, 0.2, 0.2); // Red

// Distance thresholds for LOD debug coloring (matching LodConfig defaults)
const LOD_NEAR_DISTANCE: f32 = 10.0;    // Full detail threshold
const LOD_MEDIUM_DISTANCE: f32 = 50.0;  // Medium detail threshold
const LOD_FAR_DISTANCE: f32 = 200.0;    // Low detail threshold (same as silhouette)

// Get LOD debug color based on distance from camera
fn get_lod_debug_color(distance: f32) -> vec3<f32> {
    if (distance < LOD_NEAR_DISTANCE) {
        return LOD_DEBUG_COLOR_FULL;
    } else if (distance < LOD_MEDIUM_DISTANCE) {
        return LOD_DEBUG_COLOR_MEDIUM;
    } else if (distance < LOD_FAR_DISTANCE) {
        return LOD_DEBUG_COLOR_LOW;
    } else {
        return LOD_DEBUG_COLOR_SILHOUETTE;
    }
}

// ============================================================================
// SDF PRIMITIVES (imported from sdf_primitives.wgsl at compile time)
// ============================================================================

fn sdf_sphere(p: vec3<f32>, radius: f32) -> f32 {
    return length(p) - radius;
}

fn sdf_box(p: vec3<f32>, half_extents: vec3<f32>) -> f32 {
    let q = abs(p) - half_extents;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

/// Analytical axis-aligned normal for a box SDF (sharp block look).
/// p: point in local space, half_extents: box half sizes.
/// Returns normalized outward normal; dominant axis chosen for crisp edges.
fn sdf_box_normal_local(p: vec3<f32>, half_extents: vec3<f32>) -> vec3<f32> {
    let closest = clamp(p, -half_extents, half_extents);
    let d = p - closest;
    let len = length(d);
    if (len < 1e-6) {
        return vec3<f32>(0.0, 1.0, 0.0);
    }
    let ad = abs(d);
    if (ad.x >= ad.y && ad.x >= ad.z) {
        return vec3<f32>(sign(d.x), 0.0, 0.0);
    }
    if (ad.y >= ad.z) {
        return vec3<f32>(0.0, sign(d.y), 0.0);
    }
    return vec3<f32>(0.0, 0.0, sign(d.z));
}

fn sdf_capsule(p: vec3<f32>, height: f32, radius: f32) -> f32 {
    let half_height = height * 0.5;
    let p_clamped = vec3<f32>(p.x, clamp(p.y, -half_height, half_height), p.z);
    return length(p - p_clamped) - radius;
}

// ============================================================================
// BAKED SDF SAMPLING
// ============================================================================
//
// sample_baked_sdf performs trilinear interpolation from the baked SDF texture array.
//
// The baked SDF uses a normalized coordinate system:
// - Local position (0, 0, 0) maps to texture center (0.5, 0.5, 0.5)
// - Local position range [-0.5, 0.5] maps to texture range [0.0, 1.0]
// - Positions outside this range return MAX_DIST (out of bounds)
//
// The 3D volume is stored as a 2D texture array:
// - Each SDF slot has 64 consecutive layers (Z slices)
// - Slot ID maps to base layer: slot_id * 64
//
// Trilinear interpolation:
// - XY: Hardware bilinear via sampler (linear filtering)
// - Z: Manual linear interpolation between adjacent layers

/// Sample a baked SDF from the 3D texture array.
///
/// Parameters:
/// - id: The baked SDF slot ID (0-255)
/// - local_pos: Position in entity local space, normalized to [-0.5, 0.5] range
///
/// Returns:
/// - Signed distance value, or MAX_DIST if out of bounds or id is 0 (unbaked)
fn sample_baked_sdf(id: u32, local_pos: vec3<f32>) -> f32 {
    // Bounds check: id 0 means unbaked, id >= 256 is out of range
    if (id == 0u || id >= 256u) {
        return MAX_DIST;
    }

    // Each brick is 64^3 = 262144 f32 values
    let brick_offset = id * 262144u;

    // Convert local_pos from [-0.5, 0.5] to grid [0, 63]
    let grid_pos = (local_pos + vec3<f32>(0.5)) * 63.0;
    let grid_clamped = clamp(grid_pos, vec3<f32>(0.0), vec3<f32>(62.0));

    // Integer corners
    let p0 = vec3<u32>(grid_clamped);
    let p1 = p0 + vec3<u32>(1u);

    // Fractional part for interpolation
    let f = fract(grid_clamped);

    // 8 corner reads: index = brick_offset + z * 4096 + y * 64 + x
    let c000 = brick_cache[brick_offset + p0.z * 4096u + p0.y * 64u + p0.x];
    let c100 = brick_cache[brick_offset + p0.z * 4096u + p0.y * 64u + p1.x];
    let c010 = brick_cache[brick_offset + p0.z * 4096u + p1.y * 64u + p0.x];
    let c110 = brick_cache[brick_offset + p0.z * 4096u + p1.y * 64u + p1.x];
    let c001 = brick_cache[brick_offset + p1.z * 4096u + p0.y * 64u + p0.x];
    let c101 = brick_cache[brick_offset + p1.z * 4096u + p0.y * 64u + p1.x];
    let c011 = brick_cache[brick_offset + p1.z * 4096u + p1.y * 64u + p0.x];
    let c111 = brick_cache[brick_offset + p1.z * 4096u + p1.y * 64u + p1.x];

    // Trilinear interpolation
    let c00 = mix(c000, c100, f.x);
    let c10 = mix(c010, c110, f.x);
    let c01 = mix(c001, c101, f.x);
    let c11 = mix(c011, c111, f.x);

    let c0 = mix(c00, c10, f.y);
    let c1 = mix(c01, c11, f.y);

    return mix(c0, c1, f.z);
}

/// Transform world position to entity local space for baked SDF sampling.
///
/// This function handles the transformation from world coordinates to the
/// normalized local space used by baked SDFs.
///
/// Parameters:
/// - world_pos: Position in world space
/// - entity_pos: Entity's world position (center)
/// - entity_rotation: Entity's rotation quaternion
/// - entity_scale: Entity's uniform scale factor
///
/// Returns:
/// - Position in normalized local space [-0.5, 0.5] suitable for sample_baked_sdf
fn world_to_baked_local(
    world_pos: vec3<f32>,
    entity_pos: vec3<f32>,
    entity_rotation: vec4<f32>,
    entity_scale: f32
) -> vec3<f32> {
    // Translate to entity origin
    let relative_pos = world_pos - entity_pos;

    // Rotate to entity local space (inverse rotation)
    let inv_rotation = quat_inverse(entity_rotation);
    let rotated_pos = quat_rotate(inv_rotation, relative_pos);

    // Apply inverse scale and normalize to [-0.5, 0.5] range
    // Baked SDFs are normalized to unit cube centered at origin
    // Scale determines the world-space size of the baked volume
    return rotated_pos / entity_scale;
}

// ============================================================================
// BLENDING OPERATIONS
// ============================================================================

// Smooth minimum (polynomial smooth min)
// Creates smooth blending/union between two SDF values
// a, b: distance values to blend
// k: smoothness factor (larger = smoother blend, 0 = hard union)
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 0.0) {
        return min(a, b);
    }
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// Alias for smooth_min
fn smin(a: f32, b: f32, k: f32) -> f32 {
    return smooth_min(a, b, k);
}

// LOD-aware smooth minimum
// Adapts smoothness based on distance for performance optimization
// a, b: distance values to blend
// k: base smoothness factor
// distance: distance from camera to the evaluation point
// Near objects (distance < 100) use specified k for full quality
// Far objects (distance > 100) use larger k for cheaper, less detailed blending
fn smin_lod(a: f32, b: f32, k: f32, distance: f32) -> f32 {
    // Scale k based on distance: k * clamp(distance / 100.0, 0.1, 2.0)
    // - At distance 10: k * 0.1 (tighter blend, more detail)
    // - At distance 100: k * 1.0 (base smoothness)
    // - At distance 200+: k * 2.0 (looser blend, cheaper)
    let lod_scale = clamp(distance / 100.0, 0.1, 2.0);
    let lod_k = k * lod_scale;
    return smooth_min(a, b, lod_k);
}

// ============================================================================
// QUATERNION MATH
// ============================================================================

fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let qv = vec3<f32>(q.x, q.y, q.z);
    let uv = cross(qv, v);
    let uuv = cross(qv, uv);
    return v + ((uv * q.w) + uuv) * 2.0;
}

fn quat_inverse(q: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(-q.x, -q.y, -q.z, q.w);
}

// ============================================================================
// NOISE FUNCTIONS FOR DISPLACEMENT
// ============================================================================

// 3D to 3D hash function for gradient noise
fn hash33(p: vec3<f32>) -> vec3<f32> {
    var p3 = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

// 3D Gradient noise with smoothstep interpolation
fn noise_3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Smoothstep interpolation (3t^2 - 2t^3)
    let u = f * f * (3.0 - 2.0 * f);

    // Sample gradients at 8 corners and compute dot products
    return mix(
        mix(
            mix(dot(hash33(i + vec3<f32>(0.0, 0.0, 0.0)) * 2.0 - 1.0, f - vec3<f32>(0.0, 0.0, 0.0)),
                dot(hash33(i + vec3<f32>(1.0, 0.0, 0.0)) * 2.0 - 1.0, f - vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(dot(hash33(i + vec3<f32>(0.0, 1.0, 0.0)) * 2.0 - 1.0, f - vec3<f32>(0.0, 1.0, 0.0)),
                dot(hash33(i + vec3<f32>(1.0, 1.0, 0.0)) * 2.0 - 1.0, f - vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(dot(hash33(i + vec3<f32>(0.0, 0.0, 1.0)) * 2.0 - 1.0, f - vec3<f32>(0.0, 0.0, 1.0)),
                dot(hash33(i + vec3<f32>(1.0, 0.0, 1.0)) * 2.0 - 1.0, f - vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(dot(hash33(i + vec3<f32>(0.0, 1.0, 1.0)) * 2.0 - 1.0, f - vec3<f32>(0.0, 1.0, 1.0)),
                dot(hash33(i + vec3<f32>(1.0, 1.0, 1.0)) * 2.0 - 1.0, f - vec3<f32>(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

// Standard FBM with configurable octaves for noise displacement
fn fbm_displacement(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;

    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * noise_3d(pos * frequency);
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    return value;
}

// LOD-aware FBM with smooth octave blending
// Reduces octaves based on distance to prevent popping artifacts
// distance: distance from camera to the sample point
// base_octaves: maximum number of octaves at full detail (typically 4-8)
// Returns FBM value with dynamically adjusted octave count
fn fbm_lod(p: vec3<f32>, distance: f32, base_octaves: i32) -> f32 {
    // Calculate effective octave count based on distance
    // Formula: octaves = max(1, base_octaves - floor(distance / 50))
    let octave_reduction = i32(floor(distance / 50.0));
    let effective_octaves_i = max(1, base_octaves - octave_reduction);

    // Calculate fractional part for smooth blending between LOD levels
    // This prevents popping when transitioning between octave counts
    let distance_in_band = distance - floor(distance / 50.0) * 50.0;
    let blend_factor = distance_in_band / 50.0;  // 0.0 to 1.0 within each 50-unit band

    // Compute FBM at current octave level
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;

    for (var i = 0; i < effective_octaves_i; i = i + 1) {
        value = value + amplitude * noise_3d(p * frequency);
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    // Add partial contribution from the next octave for smooth blending
    // This creates a gradual fade-out of detail rather than abrupt pops
    if (effective_octaves_i < base_octaves && blend_factor > 0.0) {
        // Inverse blend: stronger at start of band, fades toward end
        let octave_blend = 1.0 - blend_factor;
        value = value + amplitude * noise_3d(p * frequency) * octave_blend;
    }

    return value;
}

// LOD-aware FBM with custom parameters
// Provides full control over lacunarity and gain for specialized noise patterns
fn fbm_lod_params(p: vec3<f32>, distance: f32, base_octaves: i32, lacunarity: f32, gain: f32) -> f32 {
    let octave_reduction = i32(floor(distance / 50.0));
    let effective_octaves_i = max(1, base_octaves - octave_reduction);

    let distance_in_band = distance - floor(distance / 50.0) * 50.0;
    let blend_factor = distance_in_band / 50.0;

    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;

    for (var i = 0; i < effective_octaves_i; i = i + 1) {
        value = value + amplitude * noise_3d(p * frequency);
        amplitude = amplitude * gain;
        frequency = frequency * lacunarity;
    }

    // Smooth blend for next octave
    if (effective_octaves_i < base_octaves && blend_factor > 0.0) {
        let octave_blend = 1.0 - blend_factor;
        value = value + amplitude * noise_3d(p * frequency) * octave_blend;
    }

    return value;
}

// ============================================================================
// TERRAIN SDF
// ============================================================================

// 2D to 2D hash function for terrain noise
fn hash22(p: vec2<f32>) -> vec2<f32> {
    var p3 = fract(vec3<f32>(p.xyx) * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// 2D gradient noise for terrain heightmap
fn noise_2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Smoothstep interpolation
    let u = f * f * (3.0 - 2.0 * f);

    // Sample 4 corners and interpolate
    let a = hash22(i + vec2<f32>(0.0, 0.0)).x;
    let b = hash22(i + vec2<f32>(1.0, 0.0)).x;
    let c = hash22(i + vec2<f32>(0.0, 1.0)).x;
    let d = hash22(i + vec2<f32>(1.0, 1.0)).x;

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y) * 2.0 - 1.0;
}

// FBM for terrain heightmap using 2D noise on the xz plane
// Returns height value based on FBM of the xz coordinates
fn fbm_terrain(p: vec2<f32>, octaves: i32, seed: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p + vec2<f32>(seed * 17.3, seed * 31.7);

    for (var i = 0; i < octaves; i = i + 1) {
        value = value + amplitude * noise_2d(pos * frequency);
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.0;
    }

    return value;
}

// Terrain SDF: ground plane with height variation from FBM noise and planet curvature
// Formula: terrain_sdf(p) = p.y - fbm(p.xz * frequency, octaves) * amplitude - curvature_drop(distance)
//
// The curvature drop makes distant terrain "sink" below the horizon, creating the illusion
// of standing on a spherical planet. This is especially visible at distances > 500m.
fn terrain_sdf(p: vec3<f32>) -> f32 {
    // Base terrain height from noise
    let height = fbm_terrain(
        p.xz * terrain_config.frequency,
        i32(terrain_config.octaves),
        terrain_config.seed
    ) * terrain_config.amplitude;

    // Calculate distance from camera for curvature drop
    // This creates the "over the horizon" effect on a spherical world
    let dist_from_camera = length(p.xz - uniforms.camera_pos.xz);
    let drop = curvature_drop(dist_from_camera);

    // Terrain surface = base height minus curvature drop
    // As distance increases, the terrain "drops" below the horizon
    return p.y - height + drop;
}

// ============================================================================
// UE5-STYLE TERRAIN MATERIALS
// ============================================================================
// Multi-layer PBR materials with procedural detail at multiple scales

// --- Base Material Colors (photorealistic, slightly desaturated) ---
// Grass layers (with variation)
const GRASS_BASE: vec3<f32> = vec3<f32>(0.15, 0.28, 0.08);        // Dark grass base
const GRASS_MID: vec3<f32> = vec3<f32>(0.22, 0.38, 0.12);         // Medium grass
const GRASS_TIP: vec3<f32> = vec3<f32>(0.35, 0.48, 0.18);         // Grass tips (lighter)
const GRASS_DRY: vec3<f32> = vec3<f32>(0.42, 0.40, 0.22);         // Dry/dead grass

// Rock layers
const ROCK_DARK: vec3<f32> = vec3<f32>(0.18, 0.16, 0.14);         // Dark rock crevices
const ROCK_MID: vec3<f32> = vec3<f32>(0.38, 0.35, 0.32);          // Mid-tone rock
const ROCK_LIGHT: vec3<f32> = vec3<f32>(0.55, 0.52, 0.48);        // Exposed rock faces
const ROCK_MOSS: vec3<f32> = vec3<f32>(0.25, 0.32, 0.18);         // Mossy rock

// Sand/dirt layers
const SAND_WET: vec3<f32> = vec3<f32>(0.35, 0.30, 0.22);          // Wet sand (darker)
const SAND_DRY: vec3<f32> = vec3<f32>(0.62, 0.55, 0.42);          // Dry sand
const DIRT_BASE: vec3<f32> = vec3<f32>(0.28, 0.22, 0.15);         // Rich soil
const MUD_WET: vec3<f32> = vec3<f32>(0.20, 0.16, 0.12);           // Wet mud

// Water edge
const WATER_SHALLOW: vec3<f32> = vec3<f32>(0.15, 0.35, 0.40);     // Shallow water tint

// --- Terrain Detail Noise Functions ---

// 2D hash for terrain detail (faster than 3D)
fn hash2d_terrain(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// 2D gradient noise for detail
fn noise2d_detail(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash2d_terrain(i + vec2<f32>(0.0, 0.0)), hash2d_terrain(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash2d_terrain(i + vec2<f32>(0.0, 1.0)), hash2d_terrain(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    ) * 2.0 - 1.0;
}

// Multi-octave detail noise with LOD
fn terrain_detail_noise(p: vec2<f32>, octaves: i32, distance_lod: f32) -> f32 {
    // Reduce octaves based on distance for performance
    let effective_octaves = max(1, min(octaves, i32(mix(f32(octaves), 1.0, saturate(distance_lod / 200.0)))));
    
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;
    
    for (var i = 0; i < effective_octaves; i = i + 1) {
        value = value + amplitude * noise2d_detail(pos * frequency);
        amplitude = amplitude * 0.5;
        frequency = frequency * 2.1; // Slightly irregular lacunarity for more organic look
    }
    return value;
}

// Voronoi-based crack pattern for rocks
fn voronoi2d_terrain(p: vec2<f32>) -> vec2<f32> {
    let i = floor(p);
    let f = fract(p);
    
    var f1 = 1.0;
    var f2 = 1.0;
    
    for (var y = -1; y <= 1; y = y + 1) {
        for (var x = -1; x <= 1; x = x + 1) {
            let neighbor = vec2<f32>(f32(x), f32(y));
            let cell_hash = hash22(i + neighbor);
            let cell_pos = neighbor + cell_hash - f;
            let dist = length(cell_pos);
            
            if (dist < f1) {
                f2 = f1;
                f1 = dist;
            } else if (dist < f2) {
                f2 = dist;
            }
        }
    }
    return vec2<f32>(f1, f2);
}

// --- Terrain Material Calculation ---

struct TerrainMaterial {
    albedo: vec3<f32>,
    roughness: f32,
    metallic: f32,
    normal_offset: vec3<f32>,  // For micro-detail normal perturbation
    subsurface: f32,           // Subsurface scattering amount (for grass)
}

// Calculate terrain slope from position (0 = flat, 1 = vertical cliff)
fn calculate_terrain_slope(p: vec3<f32>) -> f32 {
    let eps = 0.5;
    let h0 = fbm_terrain(p.xz * terrain_config.frequency, i32(terrain_config.octaves), terrain_config.seed);
    let hx = fbm_terrain((p.xz + vec2<f32>(eps, 0.0)) * terrain_config.frequency, i32(terrain_config.octaves), terrain_config.seed);
    let hz = fbm_terrain((p.xz + vec2<f32>(0.0, eps)) * terrain_config.frequency, i32(terrain_config.octaves), terrain_config.seed);
    
    let dx = (hx - h0) / eps * terrain_config.amplitude;
    let dz = (hz - h0) / eps * terrain_config.amplitude;
    
    let slope_vec = vec3<f32>(-dx, 1.0, -dz);
    let normal = normalize(slope_vec);
    
    // Return slope as 1 - dot(normal, up)
    return 1.0 - normal.y;
}

// Main terrain material function - UE5 style
fn get_terrain_material(p: vec3<f32>, normal: vec3<f32>, distance: f32) -> TerrainMaterial {
    var mat: TerrainMaterial;
    mat.metallic = 0.0;
    mat.normal_offset = vec3<f32>(0.0);
    mat.subsurface = 0.0;
    
    // --- Calculate terrain parameters ---
    let height = p.y;
    let slope = 1.0 - max(normal.y, 0.0);  // 0 = flat, 1 = vertical
    let slope_sharp = smoothstep(0.3, 0.7, slope);  // Sharp transition for rock
    
    // World-space coordinates for tiling
    let world_uv = p.xz;
    
    // Distance-based LOD for detail noise
    let lod_factor = distance;
    
    // --- Multi-scale noise sampling ---
    // Macro scale (large terrain features)
    let macro_noise = terrain_detail_noise(world_uv * 0.01, 3, lod_factor);
    // Meso scale (medium features like bumps, patches)
    let meso_noise = terrain_detail_noise(world_uv * 0.08, 4, lod_factor);
    // Micro scale (fine detail - grass blades, pebbles)
    let micro_noise = terrain_detail_noise(world_uv * 0.5, 5, lod_factor);
    // Very fine detail (only visible close up)
    let fine_noise = select(0.0, terrain_detail_noise(world_uv * 2.0, 3, lod_factor), distance < 50.0);
    
    // Voronoi for rock cracks (only computed for rocky areas)
    let rock_voronoi = select(vec2<f32>(0.5, 0.5), voronoi2d_terrain(world_uv * 0.3), slope_sharp > 0.1);
    let crack_factor = smoothstep(0.0, 0.15, rock_voronoi.y - rock_voronoi.x);
    
    // --- Height and moisture zones ---
    let height_normalized = (height + terrain_config.amplitude) / (terrain_config.amplitude * 2.0);
    let water_level = -terrain_config.amplitude * 0.3;  // Water line
    let beach_level = water_level + terrain_config.amplitude * 0.15;
    
    // Moisture simulation (low areas are wetter)
    let moisture = saturate(1.0 - (height - water_level) / (terrain_config.amplitude * 0.5));
    let moisture_varied = moisture + meso_noise * 0.2;
    
    // --- Material blending ---
    
    // 1. GRASS (flat areas, above beach)
    var grass_weight = (1.0 - slope_sharp) * smoothstep(beach_level, beach_level + 2.0, height);
    
    // Grass color with variation
    let grass_variation = macro_noise * 0.5 + 0.5;
    let grass_detail = meso_noise * 0.3 + fine_noise * 0.15;
    var grass_color = mix(GRASS_BASE, GRASS_MID, grass_variation);
    grass_color = mix(grass_color, GRASS_TIP, saturate(grass_detail + 0.3));
    // Add dry patches
    let dry_patches = smoothstep(0.4, 0.7, macro_noise + meso_noise * 0.5);
    grass_color = mix(grass_color, GRASS_DRY, dry_patches * (1.0 - moisture_varied) * 0.6);
    
    // 2. ROCK (steep slopes)
    var rock_weight = slope_sharp;
    
    // Rock color with cracks and variation
    var rock_color = mix(ROCK_DARK, ROCK_MID, macro_noise * 0.5 + 0.5);
    rock_color = mix(rock_color, ROCK_LIGHT, saturate(meso_noise + 0.2));
    // Darken cracks
    rock_color = mix(rock_color, ROCK_DARK * 0.5, (1.0 - crack_factor) * 0.5);
    // Add moss on north-facing rock
    let north_facing = saturate(-normal.z * 0.5 + 0.5);
    let moss_amount = north_facing * moisture_varied * (1.0 - slope_sharp * 0.5) * 0.4;
    rock_color = mix(rock_color, ROCK_MOSS, moss_amount);
    
    // 3. SAND/DIRT (beach level and dry areas)
    let beach_zone = smoothstep(water_level - 1.0, water_level + 0.5, height) * 
                     (1.0 - smoothstep(beach_level, beach_level + 3.0, height));
    var sand_weight = beach_zone * (1.0 - slope_sharp);
    
    // Sand color with wet/dry variation
    var sand_color = mix(SAND_WET, SAND_DRY, saturate(height - water_level) / 3.0);
    sand_color = mix(sand_color, sand_color * (1.0 + meso_noise * 0.2), 1.0);
    
    // 4. MUD (very low wet areas)
    let mud_zone = smoothstep(beach_level, water_level, height) * moisture_varied;
    var mud_weight = mud_zone * (1.0 - slope_sharp);
    
    var mud_color = mix(MUD_WET, DIRT_BASE, meso_noise * 0.5 + 0.5);
    
    // 5. WATER EDGE TINT (underwater or at water level)
    let water_edge = smoothstep(water_level + 1.0, water_level - 1.0, height);
    
    // --- Normalize weights and blend ---
    let total_weight = grass_weight + rock_weight + sand_weight + mud_weight + 0.001;
    grass_weight = grass_weight / total_weight;
    rock_weight = rock_weight / total_weight;
    sand_weight = sand_weight / total_weight;
    mud_weight = mud_weight / total_weight;
    
    // Final albedo blend
    mat.albedo = grass_color * grass_weight +
                 rock_color * rock_weight +
                 sand_color * sand_weight +
                 mud_color * mud_weight;
    
    // Water edge tinting
    mat.albedo = mix(mat.albedo, mat.albedo * WATER_SHALLOW * 2.0, water_edge * 0.5);
    
    // --- Roughness per material ---
    let grass_roughness = 0.85 + micro_noise * 0.1;
    let rock_roughness = 0.6 + meso_noise * 0.2 - crack_factor * 0.2;
    let sand_roughness = 0.9 + fine_noise * 0.05;
    let mud_roughness = 0.4 + moisture_varied * 0.2;  // Wet = shiny
    
    mat.roughness = grass_roughness * grass_weight +
                    rock_roughness * rock_weight +
                    sand_roughness * sand_weight +
                    mud_roughness * mud_weight;
    
    // --- Micro-normal perturbation (adds fine surface detail) ---
    if (distance < 100.0) {
        let detail_strength = smoothstep(100.0, 20.0, distance) * 0.15;
        let nx = terrain_detail_noise(world_uv * 1.5 + vec2<f32>(7.3, 0.0), 2, lod_factor);
        let nz = terrain_detail_noise(world_uv * 1.5 + vec2<f32>(0.0, 13.7), 2, lod_factor);
        mat.normal_offset = vec3<f32>(nx, 0.0, nz) * detail_strength;
    }
    
    // --- Subsurface scattering for grass ---
    mat.subsurface = grass_weight * 0.4 * (1.0 - slope_sharp);
    
    return mat;
}

// Legacy terrain colors (kept for compatibility)
const TERRAIN_COLOR_LOW: vec3<f32> = vec3<f32>(0.2, 0.35, 0.15);   // Grass/lowlands
const TERRAIN_COLOR_HIGH: vec3<f32> = vec3<f32>(0.45, 0.4, 0.35);  // Rocky/highlands

// ============================================================================
// SDF WITH NOISE DISPLACEMENT
// ============================================================================

/// Applies noise displacement to an SDF value.
///
/// Formula: sdf(p) + fbm(p * frequency, octaves) * amplitude
///
/// Parameters:
/// - base_sdf: The base SDF value at point p
/// - world_p: World-space position for noise sampling (ensures consistent noise across transforms)
/// - frequency: Noise frequency multiplier (higher = more detail)
/// - octaves: Number of FBM octaves (1-8, more = more detail but slower)
/// - amplitude: Noise amplitude (strength of displacement)
fn sdf_with_noise(base_sdf: f32, world_p: vec3<f32>, frequency: f32, octaves: i32, amplitude: f32) -> f32 {
    let noise_value = fbm_displacement(world_p * frequency, octaves);
    return base_sdf + noise_value * amplitude;
}

/// Applies LOD-aware noise displacement to an SDF value.
/// Reduces octaves based on distance from camera for performance optimization.
///
/// Parameters:
/// - base_sdf: The base SDF value at point p
/// - world_p: World-space position for noise sampling
/// - frequency: Noise frequency multiplier
/// - base_octaves: Maximum number of FBM octaves at full detail
/// - amplitude: Noise amplitude (strength of displacement)
/// - camera_distance: Distance from camera to the sample point
fn sdf_with_noise_lod(base_sdf: f32, world_p: vec3<f32>, frequency: f32, base_octaves: i32, amplitude: f32, camera_distance: f32) -> f32 {
    // In silhouette mode, skip noise entirely for maximum performance
    if (camera_distance >= LOD_SILHOUETTE_DISTANCE) {
        return base_sdf;
    }

    // Use LOD-aware FBM with smooth octave blending
    let noise_value = fbm_lod(world_p * frequency, camera_distance, base_octaves);

    // Fade out noise amplitude as we approach silhouette distance
    // This ensures smooth transition to no-noise silhouette mode
    let silhouette_blend_start = LOD_SILHOUETTE_DISTANCE - LOD_SILHOUETTE_BLEND_RANGE;
    var amplitude_scale = 1.0;
    if (camera_distance > silhouette_blend_start) {
        let blend_factor = (camera_distance - silhouette_blend_start) / LOD_SILHOUETTE_BLEND_RANGE;
        amplitude_scale = 1.0 - smoothstep(0.0, 1.0, blend_factor);
    }

    return base_sdf + noise_value * amplitude * amplitude_scale;
}

// ============================================================================
// PREVIEW SDF EVALUATION
// ============================================================================

// Preview ghost appearance constants
const PREVIEW_SCALE: f32 = 1.0;            // Default scale for preview objects
const PREVIEW_COLOR: vec3<f32> = vec3<f32>(0.4, 0.8, 1.0);  // Cyan ghost color
const PREVIEW_ALPHA: f32 = 0.5;            // Semi-transparency

/// Evaluate the preview SDF at a given point.
/// Returns the signed distance to the preview object, or MAX_DIST if preview is disabled.
fn evaluate_preview_sdf(p: vec3<f32>) -> f32 {
    if (uniforms.preview_enabled == 0u) {
        return MAX_DIST;
    }

    // Transform point to preview local space (centered at preview_position)
    let local_p = p - uniforms.preview_position;

    var d: f32 = MAX_DIST;

    switch uniforms.preview_sdf_type {
        case SDF_SPHERE: {
            // Sphere with radius 1.0 (default scale)
            d = sdf_sphere(local_p, PREVIEW_SCALE);
        }
        case SDF_BOX: {
            // Box with half extents 1.0
            d = sdf_box(local_p, vec3<f32>(PREVIEW_SCALE));
        }
        case SDF_CAPSULE: {
            // Capsule with height 2.0 and radius 1.0
            d = sdf_capsule(local_p, 2.0 * PREVIEW_SCALE, PREVIEW_SCALE);
        }
        default: {
            d = MAX_DIST;
        }
    }

    return d;
}

// ============================================================================
// SCENE SDF EVALUATION
// ============================================================================

struct HitResult {
    dist: f32,
    entity_index: i32,
    lod_octaves: u32,  // LOD octave count from the closest entity
    is_terrain: u32,   // 1 if terrain hit, 0 otherwise
    is_preview: u32,   // 1 if preview hit, 0 otherwise
    is_marker: u32,    // 1 if distance reference marker, 0 otherwise
    is_hands: u32,     // US-019: 1 if first-person hand was hit, 0 otherwise
    marker_distance: f32,  // Distance of the marker from origin (10, 25, 50, 100, 1000)
    precision_class: u32,  // US-039: 0=Player, 1=Interactive, 2=Static, 3=Terrain (for adaptive stepping)
}

// ============================================================================
// DISTANCE REFERENCE MARKERS
// ============================================================================
// Trees/poles at 10m, 25m, 50m, 100m, 1000m in all 4 cardinal directions (N, S, E, W)
// Each marker is a tree: trunk (capsule) + foliage (sphere on top)
// Tree heights scale with distance: 10m marker = 5m tree, 1km marker = 50m tree

// Tree SDF: trunk (capsule) + foliage (sphere) smoothly blended
fn sdf_tree(p: vec3<f32>, height: f32) -> f32 {
    // Trunk: capsule from ground to 70% of height
    let trunk_height = height * 0.7;
    let trunk_radius = height * 0.05;
    let trunk_center = vec3<f32>(0.0, trunk_height * 0.5, 0.0);
    let trunk_p = p - trunk_center;
    let trunk = sdf_capsule(trunk_p, trunk_height, trunk_radius);

    // Foliage: sphere at top, radius = 30% of height
    let foliage_center = vec3<f32>(0.0, trunk_height + height * 0.15, 0.0);
    let foliage_radius = height * 0.25;
    let foliage = sdf_sphere(p - foliage_center, foliage_radius);

    // Smooth blend trunk and foliage
    return smooth_min(trunk, foliage, height * 0.1);
}

// Reference marker distances in meters
const MARKER_DISTANCES: array<f32, 5> = array<f32, 5>(10.0, 25.0, 50.0, 100.0, 1000.0);
// Tree heights for each distance marker (proportional to distance)
const MARKER_HEIGHTS: array<f32, 5> = array<f32, 5>(5.0, 8.0, 12.0, 20.0, 50.0);

// Evaluate all distance reference markers with planet curvature
// Returns (distance to nearest marker, marker's distance from origin)
// Trees are placed relative to the camera, and their Y position is adjusted
// for planet curvature (they "sink" below the horizon at distance)
fn evaluate_markers(p: vec3<f32>) -> vec2<f32> {
    var min_d: f32 = MAX_DIST;
    var marker_dist: f32 = 0.0;

    // Markers are placed relative to camera XZ position
    let cam_xz = uniforms.camera_pos.xz;

    // For each marker distance
    for (var i: i32 = 0; i < 5; i = i + 1) {
        let dist = MARKER_DISTANCES[i];
        let height = MARKER_HEIGHTS[i];

        // Curvature drop at this distance - trees sink below horizon
        let drop = curvature_drop(dist);

        // North (+Z) - relative to camera
        let north_pos = vec3<f32>(cam_xz.x, -drop, cam_xz.y + dist);
        let north_p = p - north_pos;
        let d_north = sdf_tree(north_p, height);
        if (d_north < min_d) {
            min_d = d_north;
            marker_dist = dist;
        }

        // South (-Z)
        let south_pos = vec3<f32>(cam_xz.x, -drop, cam_xz.y - dist);
        let south_p = p - south_pos;
        let d_south = sdf_tree(south_p, height);
        if (d_south < min_d) {
            min_d = d_south;
            marker_dist = dist;
        }

        // East (+X)
        let east_pos = vec3<f32>(cam_xz.x + dist, -drop, cam_xz.y);
        let east_p = p - east_pos;
        let d_east = sdf_tree(east_p, height);
        if (d_east < min_d) {
            min_d = d_east;
            marker_dist = dist;
        }

        // West (-X)
        let west_pos = vec3<f32>(cam_xz.x - dist, -drop, cam_xz.y);
        let west_p = p - west_pos;
        let d_west = sdf_tree(west_p, height);
        if (d_west < min_d) {
            min_d = d_west;
            marker_dist = dist;
        }
    }

    return vec2<f32>(min_d, marker_dist);
}

fn evaluate_entity_sdf(p: vec3<f32>, entity: GpuEntity) -> f32 {
    // Reconstruct vec3 from scalar fields (to match 112-byte struct layout)
    let entity_position = vec3<f32>(entity.position_x, entity.position_y, entity.position_z);
    let entity_scale = vec3<f32>(entity.scale_x, entity.scale_y, entity.scale_z);

    // Transform point to entity local space
    let local_p = quat_rotate(quat_inverse(entity.rotation), p - entity_position);
    let scaled_p = local_p / entity_scale;

    // Use minimum scale component for consistent distance scaling
    let min_scale = min(entity_scale.x, min(entity_scale.y, entity_scale.z));

    var d: f32 = MAX_DIST;

    // US-023: Smooth transition from equation to baked SDF
    // bake_blend: 0.0 = equation only, 1.0 = baked only, 0.0-1.0 = blend both
    let blend = entity.bake_blend;

    // Check if this entity uses a baked SDF (baked_sdf_id != 0)
    // Baked SDFs provide O(1) lookup via texture sampling vs O(n) equation evaluation
    // Performance target: 5x faster than equation evaluation per entity
    if (entity.baked_sdf_id != 0u && blend > 0.0) {
        // BAKED SDF PATH: Use texture sampling with trilinear interpolation
        //
        // Transform world position to normalized local space for baked SDF sampling.
        // Baked SDFs use a normalized [-0.5, 0.5] coordinate system centered at entity origin.
        // The baking process captures the SDF in this normalized space.
        //
        // For uniform scale entities, we use the average scale to determine the sampling volume.
        // For non-uniform scale, the min_scale determines the bounding volume size.
        let baked_local_pos = world_to_baked_local(
            p,
            entity_position,
            entity.rotation,
            min_scale  // Use min_scale as the baked volume size
        );

        // Sample the baked SDF using trilinear interpolation
        // Returns MAX_DIST if position is outside the baked volume bounds
        let baked_d = sample_baked_sdf(entity.baked_sdf_id, baked_local_pos);

        // If within baked volume bounds, use baked distance (potentially blended)
        // The baked SDF stores normalized distances, so we need to scale by entity size
        if (baked_d < MAX_DIST) {
            let baked_scaled = baked_d * min_scale;

            // US-023: Smooth transition - blend between equation and baked
            if (blend >= 1.0) {
                // Fully baked - use baked SDF only (most common case after transition)
                d = baked_scaled;
            } else {
                // During transition - blend equation and baked for smooth visual transition
                let equation_d = evaluate_entity_sdf_equation(scaled_p, entity.sdf_type) * min_scale;
                // Use smoothstep for perceptually smooth transition (no visual pop)
                let smooth_blend = smoothstep(0.0, 1.0, blend);
                d = mix(equation_d, baked_scaled, smooth_blend);
            }
        } else {
            // Outside baked volume - fall back to equation for this sample
            // This handles cases where rays extend beyond the baked region
            d = evaluate_entity_sdf_equation(scaled_p, entity.sdf_type) * min_scale;
        }
    } else {
        // EQUATION FALLBACK PATH: Use traditional SDF equation evaluation
        // Used when entity hasn't been baked (baked_sdf_id == 0) or bake_blend == 0.0
        d = evaluate_entity_sdf_equation(scaled_p, entity.sdf_type) * min_scale;
    }

    // Apply noise displacement if enabled for this entity
    // Note: Noise is applied AFTER baked/equation evaluation for consistent results
    if (entity.use_noise == 1u) {
        // Use world-space position for consistent noise across transforms
        d = sdf_with_noise(
            d,
            p,
            entity.noise_frequency,
            i32(entity.noise_octaves),
            entity.noise_amplitude
        );
    }

    return d;
}

/// Evaluate SDF using equation-based primitives.
/// This is the fallback when no baked SDF is available (baked_sdf_id == 0)
/// or when sampling outside the baked volume bounds.
fn evaluate_entity_sdf_equation(scaled_p: vec3<f32>, sdf_type: u32) -> f32 {
    switch sdf_type {
        case SDF_SPHERE: {
            // Sphere uses scale.x as radius
            return sdf_sphere(scaled_p, 1.0);
        }
        case SDF_BOX: {
            // Box uses scale as half extents
            return sdf_box(scaled_p, vec3<f32>(1.0));
        }
        case SDF_CAPSULE: {
            // Capsule uses scale.y as height, scale.x as radius
            return sdf_capsule(scaled_p, 2.0, 1.0);
        }
        default: {
            return MAX_DIST;
        }
    }
}

// ============================================================================
// FIRST-PERSON HANDS SDF (US-019)
// ============================================================================
//
// First-person hands are rendered as capsules positioned relative to the camera.
// They are only visible in first-person mode (camera_mode == 1).
//
// Hand positions in camera space (relative to camera looking direction):
// - Left hand:  (-0.3, -0.3, 0.5) - 30cm left, 30cm down, 50cm forward
// - Right hand: ( 0.3, -0.3, 0.5) - 30cm right, 30cm down, 50cm forward
//
// The hands have a subtle idle bob animation based on time.

// First-person hand constants
const FP_HAND_RADIUS: f32 = 0.05;        // Hand capsule radius (5cm - realistic palm width)
const FP_HAND_LENGTH: f32 = 0.12;        // Hand capsule length (12cm - hand length)
const FP_HAND_BOB_AMPLITUDE: f32 = 0.01; // Subtle bob amplitude (1cm)
const FP_HAND_BOB_SPEED: f32 = 2.0;      // Bob animation speed
const FP_HAND_COLOR: vec3<f32> = vec3<f32>(0.9, 0.75, 0.6); // Skin tone (matches player body)

// Left hand offset in camera space (X=-0.3, Y=-0.3, Z=0.5)
const FP_LEFT_HAND_OFFSET: vec3<f32> = vec3<f32>(-0.3, -0.3, 0.5);
// Right hand offset in camera space (X=0.3, Y=-0.3, Z=0.5)
const FP_RIGHT_HAND_OFFSET: vec3<f32> = vec3<f32>(0.3, -0.3, 0.5);

/// Extract camera orientation vectors from the inverse view-projection matrix.
/// Returns (right, up, forward) vectors in world space.
fn get_camera_orientation() -> mat3x3<f32> {
    // The inverse view matrix (upper-left 3x3 of inv_view_proj) contains:
    // Column 0: camera right vector
    // Column 1: camera up vector
    // Column 2: camera forward vector (negative look direction)
    //
    // Note: We extract from inv_view_proj which includes perspective, so
    // we need to normalize the columns for accurate direction vectors.
    let right = normalize(vec3<f32>(
        uniforms.inv_view_proj[0][0],
        uniforms.inv_view_proj[0][1],
        uniforms.inv_view_proj[0][2]
    ));
    let up = normalize(vec3<f32>(
        uniforms.inv_view_proj[1][0],
        uniforms.inv_view_proj[1][1],
        uniforms.inv_view_proj[1][2]
    ));
    // Forward is the negative of the view direction in OpenGL convention
    let forward = normalize(vec3<f32>(
        -uniforms.inv_view_proj[2][0],
        -uniforms.inv_view_proj[2][1],
        -uniforms.inv_view_proj[2][2]
    ));
    return mat3x3<f32>(right, up, forward);
}

/// Transform a position from camera space to world space.
/// camera_local: position in camera space (X=right, Y=up, Z=forward)
/// Returns: position in world space
fn camera_to_world(camera_local: vec3<f32>) -> vec3<f32> {
    let orientation = get_camera_orientation();
    // Transform local position by camera orientation and add camera world position
    return uniforms.camera_pos
         + orientation[0] * camera_local.x  // right
         + orientation[1] * camera_local.y  // up
         + orientation[2] * camera_local.z; // forward
}

/// Calculate idle bob offset for hands.
/// Uses a sine wave with slight phase difference between hands.
fn hand_bob_offset(time: f32, is_left: bool) -> f32 {
    let phase = select(0.0, 0.5, is_left);
    return sin((time + phase) * FP_HAND_BOB_SPEED) * FP_HAND_BOB_AMPLITUDE;
}

/// Signed distance to a hand capsule.
/// p: world-space point to evaluate
/// hand_center: world-space center of the hand
fn sdf_hand_capsule(p: vec3<f32>, hand_center: vec3<f32>) -> f32 {
    // Simple sphere for the hand (capsule would need orientation which adds complexity)
    // Using a sphere here for simplicity, can be upgraded to capsule later
    return length(p - hand_center) - FP_HAND_RADIUS;
}

/// Evaluate first-person hands SDF.
/// Returns distance to nearest hand, or MAX_DIST if hands are not visible.
/// Only evaluates if camera_mode == 1 (first-person).
fn evaluate_first_person_hands(p: vec3<f32>) -> f32 {
    // Only render hands in first-person mode
    if (uniforms.camera_mode != 1u) {
        return MAX_DIST;
    }

    // Calculate bob offset for subtle idle animation
    let left_bob = hand_bob_offset(uniforms.time, true);
    let right_bob = hand_bob_offset(uniforms.time, false);

    // Calculate hand positions in camera space with bob animation
    let left_cam = FP_LEFT_HAND_OFFSET + vec3<f32>(0.0, left_bob, 0.0);
    let right_cam = FP_RIGHT_HAND_OFFSET + vec3<f32>(0.0, right_bob, 0.0);

    // Transform to world space
    let left_world = camera_to_world(left_cam);
    let right_world = camera_to_world(right_cam);

    // Evaluate SDF for both hands
    let d_left = sdf_hand_capsule(p, left_world);
    let d_right = sdf_hand_capsule(p, right_world);

    // Return minimum distance (closest hand)
    return min(d_left, d_right);
}

// ============================================================================
// SCENE SDF EVALUATION - TILE-AWARE VERSION (US-017)
// ============================================================================
//
// scene_sdf_tiled: Uses tile culling to only evaluate entities overlapping the
// current pixel's tile. This dramatically reduces SDF evaluations from O(n) to O(k)
// where k is the number of entities in the current tile (typically 2-10).
//
// scene_sdf: Legacy version that evaluates all entities (for normal calculation
// and when tile culling is unavailable).

/// Tile-aware scene SDF evaluation.
/// Uses tile culling data to only evaluate entities overlapping the current pixel's tile.
/// Falls back to evaluating all entities if tile culling is unavailable.
///
/// Parameters:
/// - p: World-space position to evaluate
/// - pixel_coords: Screen-space pixel coordinates (x, y) for tile lookup
fn scene_sdf_tiled(p: vec3<f32>, pixel_coords: vec2<u32>) -> HitResult {
    var result: HitResult;
    result.dist = MAX_DIST;
    result.entity_index = -1;
    result.lod_octaves = LOD_FULL_OCTAVES;
    result.is_terrain = 0u;
    result.is_preview = 0u;
    result.is_marker = 0u;
    result.is_hands = 0u;
    result.marker_distance = 0.0;
    result.precision_class = 2u;  // Default to Static precision

    // US-019: Evaluate first-person hands (highest priority - closest to camera)
    let hands_d = evaluate_first_person_hands(p);
    if (hands_d < result.dist) {
        result.dist = hands_d;
        result.entity_index = -4;  // Special index for first-person hands
        result.is_hands = 1u;
        result.precision_class = 0u;  // Player precision for hands
    }

    // Evaluate distance reference markers (trees at 10m, 25m, 50m, 100m, 1000m)
    let marker_result = evaluate_markers(p);
    if (marker_result.x < result.dist) {
        result.dist = marker_result.x;
        result.entity_index = -3;  // Special index for markers
        result.is_marker = 1u;
        result.marker_distance = marker_result.y;
        result.precision_class = 2u;  // Static precision for markers
    }

    // US-0P04: Two-sided terrain SDF evaluation
    if (terrain_config.enabled == 1u && uniforms.terrain_visible == 1u) {
        let terrain_d = terrain_sdf(p);

        let camera_terrain_height = fbm_terrain(
            uniforms.camera_pos.xz * terrain_config.frequency,
            i32(terrain_config.octaves),
            terrain_config.seed
        ) * terrain_config.amplitude;

        let camera_above_terrain = uniforms.camera_pos.y > camera_terrain_height;

        var should_include_terrain = false;

        if (camera_above_terrain) {
            should_include_terrain = terrain_d < result.dist;
        } else {
            let abs_terrain_d = abs(terrain_d);
            should_include_terrain = abs_terrain_d < result.dist * 0.5 && terrain_d < 0.0;
        }

        if (should_include_terrain) {
            result.dist = select(terrain_d, abs(terrain_d), !camera_above_terrain);
            result.entity_index = -1;  // No entity, it's terrain
            result.is_terrain = 1u;
            result.is_preview = 0u;
            result.is_marker = 0u;
            result.is_hands = 0u;
            result.precision_class = 3u;  // Terrain precision (lowest)
        }
    }

    // US-017: Tile-based entity evaluation
    // Check if tile culling is enabled (total_tiles > 0)
    if (is_tile_culling_enabled()) {
        // Get the tile index for the current pixel
        let tile_index = get_tile_index_for_pixel(pixel_coords.x, pixel_coords.y);

        // Get the number of entities in this tile (clamped to max)
        let tile_entity_count = get_tile_entity_count(tile_index);

        // Only evaluate entities that overlap this tile
        for (var slot: u32 = 0u; slot < tile_entity_count; slot = slot + 1u) {
            let entity_index = get_tile_entity_index(tile_index, slot);

            // Bounds check (should always pass, but be safe)
            if (entity_index >= entity_buffer.count) {
                continue;
            }

            let entity = entity_buffer.entities[entity_index];
            let d = evaluate_entity_sdf(p, entity);

            if (d < result.dist) {
                result.dist = d;
                result.entity_index = i32(entity_index);
                result.lod_octaves = entity.lod_octaves;
                result.is_terrain = 0u;
                result.is_preview = 0u;
                result.is_marker = 0u;
                result.is_hands = 0u;
                result.precision_class = entity.precision_class;  // US-039: Read precision from entity
            }
        }
    } else {
        // Fallback: Evaluate all entities when tile culling is unavailable
        let count = entity_buffer.count;
        for (var i: u32 = 0u; i < count; i = i + 1u) {
            let entity = entity_buffer.entities[i];
            let d = evaluate_entity_sdf(p, entity);

            if (d < result.dist) {
                result.dist = d;
                result.entity_index = i32(i);
                result.lod_octaves = entity.lod_octaves;
                result.is_terrain = 0u;
                result.is_preview = 0u;
                result.is_marker = 0u;
                result.is_hands = 0u;
                result.precision_class = entity.precision_class;  // US-039: Read precision from entity
            }
        }
    }

    // Evaluate preview object (always evaluated last so we can render it semi-transparently)
    let preview_d = evaluate_preview_sdf(p);
    if (preview_d < result.dist) {
        result.dist = preview_d;
        result.entity_index = -2;  // Special index for preview
        result.lod_octaves = LOD_FULL_OCTAVES;
        result.is_terrain = 0u;
        result.is_preview = 1u;
        result.is_marker = 0u;
        result.is_hands = 0u;
        result.precision_class = 1u;  // Interactive precision for preview
    }

    return result;
}

/// Froxel-aware scene SDF evaluation (US-037).
/// Only evaluates SDFs that are assigned to the current froxel.
/// Falls back to tile-based or full evaluation if froxel system is disabled.
///
/// Parameters:
/// - p: World-space position to evaluate
/// - froxel_idx: The froxel index for this position (pre-computed by ray march loop)
/// - pixel_coords: Screen-space pixel coordinates for tile-based fallback
fn scene_sdf_froxel(p: vec3<f32>, froxel_idx: u32, pixel_coords: vec2<u32>) -> HitResult {
    var result: HitResult;
    result.dist = MAX_DIST;
    result.entity_index = -1;
    result.lod_octaves = LOD_FULL_OCTAVES;
    result.is_terrain = 0u;
    result.is_preview = 0u;
    result.is_marker = 0u;
    result.is_hands = 0u;
    result.marker_distance = 0.0;
    result.precision_class = 2u;  // Default to Static precision

    // US-019: Evaluate first-person hands (highest priority - closest to camera)
    let hands_d = evaluate_first_person_hands(p);
    if (hands_d < result.dist) {
        result.dist = hands_d;
        result.entity_index = -4;  // Special index for first-person hands
        result.is_hands = 1u;
        result.precision_class = 0u;  // Player precision for hands
    }

    // Evaluate distance reference markers (trees at 10m, 25m, 50m, 100m, 1000m)
    let marker_result = evaluate_markers(p);
    if (marker_result.x < result.dist) {
        result.dist = marker_result.x;
        result.entity_index = -3;  // Special index for markers
        result.is_marker = 1u;
        result.marker_distance = marker_result.y;
        result.precision_class = 2u;  // Static precision for markers
    }

    // US-0P04: Two-sided terrain SDF evaluation
    if (terrain_config.enabled == 1u && uniforms.terrain_visible == 1u) {
        let terrain_d = terrain_sdf(p);

        let camera_terrain_height = fbm_terrain(
            uniforms.camera_pos.xz * terrain_config.frequency,
            i32(terrain_config.octaves),
            terrain_config.seed
        ) * terrain_config.amplitude;

        let camera_above_terrain = uniforms.camera_pos.y > camera_terrain_height;

        var should_include_terrain = false;

        if (camera_above_terrain) {
            should_include_terrain = terrain_d < result.dist;
        } else {
            let abs_terrain_d = abs(terrain_d);
            should_include_terrain = abs_terrain_d < result.dist * 0.5 && terrain_d < 0.0;
        }

        if (should_include_terrain) {
            result.dist = select(terrain_d, abs(terrain_d), !camera_above_terrain);
            result.entity_index = -1;  // No entity, it's terrain
            result.is_terrain = 1u;
            result.is_preview = 0u;
            result.is_marker = 0u;
            result.is_hands = 0u;
            result.precision_class = 3u;  // Terrain precision (lowest)
        }
    }

    // US-037: Froxel-based entity evaluation
    // Only evaluate entities that are assigned to this froxel
    if (froxel_idx < TOTAL_FROXELS) {
        let sdf_count = get_froxel_sdf_count(froxel_idx);

        for (var slot: u32 = 0u; slot < sdf_count; slot = slot + 1u) {
            let entity_index = get_froxel_sdf_index(froxel_idx, slot);

            // Bounds check
            if (entity_index >= entity_buffer.count) {
                continue;
            }

            let entity = entity_buffer.entities[entity_index];
            let d = evaluate_entity_sdf(p, entity);

            if (d < result.dist) {
                result.dist = d;
                result.entity_index = i32(entity_index);
                result.lod_octaves = entity.lod_octaves;
                result.is_terrain = 0u;
                result.is_preview = 0u;
                result.is_marker = 0u;
                result.is_hands = 0u;
                result.precision_class = entity.precision_class;  // US-039: Read precision from entity
            }
        }
    } else {
        // Froxel is invalid (outside frustum) - fallback to tile-based or full evaluation
        if (is_tile_culling_enabled()) {
            let tile_index = get_tile_index_for_pixel(pixel_coords.x, pixel_coords.y);
            let tile_entity_count = get_tile_entity_count(tile_index);

            for (var slot: u32 = 0u; slot < tile_entity_count; slot = slot + 1u) {
                let entity_index = get_tile_entity_index(tile_index, slot);

                if (entity_index >= entity_buffer.count) {
                    continue;
                }

                let entity = entity_buffer.entities[entity_index];
                let d = evaluate_entity_sdf(p, entity);

                if (d < result.dist) {
                    result.dist = d;
                    result.entity_index = i32(entity_index);
                    result.lod_octaves = entity.lod_octaves;
                    result.is_terrain = 0u;
                    result.is_preview = 0u;
                    result.is_marker = 0u;
                    result.is_hands = 0u;
                    result.precision_class = entity.precision_class;  // US-039: Read precision from entity
                }
            }
        } else {
            // Evaluate all entities when both systems are unavailable
            let count = entity_buffer.count;
            for (var i: u32 = 0u; i < count; i = i + 1u) {
                let entity = entity_buffer.entities[i];
                let d = evaluate_entity_sdf(p, entity);

                if (d < result.dist) {
                    result.dist = d;
                    result.entity_index = i32(i);
                    result.lod_octaves = entity.lod_octaves;
                    result.is_terrain = 0u;
                    result.is_preview = 0u;
                    result.is_marker = 0u;
                    result.is_hands = 0u;
                    result.precision_class = entity.precision_class;  // US-039: Read precision from entity
                }
            }
        }
    }

    // Evaluate preview object (always evaluated last so we can render it semi-transparently)
    let preview_d = evaluate_preview_sdf(p);
    if (preview_d < result.dist) {
        result.dist = preview_d;
        result.entity_index = -2;  // Special index for preview
        result.lod_octaves = LOD_FULL_OCTAVES;
        result.is_terrain = 0u;
        result.is_preview = 1u;
        result.is_marker = 0u;
        result.is_hands = 0u;
        result.precision_class = 1u;  // Interactive precision for preview
    }

    return result;
}

/// Legacy scene SDF evaluation (for backward compatibility and normal calculation).
/// Evaluates ALL entities without tile culling.
/// Used by calculate_normal() which doesn't have pixel coordinates available.
fn scene_sdf(p: vec3<f32>) -> HitResult {
    var result: HitResult;
    result.dist = MAX_DIST;
    result.entity_index = -1;
    result.lod_octaves = LOD_FULL_OCTAVES;
    result.is_terrain = 0u;
    result.is_preview = 0u;
    result.is_marker = 0u;
    result.is_hands = 0u;
    result.marker_distance = 0.0;
    result.precision_class = 2u;  // Default to Static precision

    // US-019: Evaluate first-person hands (highest priority - closest to camera)
    // Hands are only visible in first-person mode (camera_mode == 1)
    let hands_d = evaluate_first_person_hands(p);
    if (hands_d < result.dist) {
        result.dist = hands_d;
        result.entity_index = -4;  // Special index for first-person hands
        result.is_hands = 1u;
        result.precision_class = 0u;  // Player precision for hands
    }

    // Evaluate distance reference markers (trees at 10m, 25m, 50m, 100m, 1000m)
    let marker_result = evaluate_markers(p);
    if (marker_result.x < result.dist) {
        result.dist = marker_result.x;
        result.entity_index = -3;  // Special index for markers
        result.is_marker = 1u;
        result.is_hands = 0u;
        result.marker_distance = marker_result.y;
        result.precision_class = 2u;  // Static precision for markers
    }

    // US-0P04: Two-sided terrain SDF evaluation
    // Terrain is ALWAYS evaluated when enabled and visible, regardless of camera position.
    // This ensures entities are visible from any camera angle (above, below, or at terrain level).
    //
    // Two-sided terrain means:
    // - When camera is above terrain: rays hit terrain surface from above (normal case)
    // - When camera is below terrain: rays pass through terrain to reach entities
    //   (terrain rendered as semi-transparent or skipped based on ray direction)
    //
    // The key insight: we use the terrain SDF distance but DON'T let it block entity rays
    // when the camera is below terrain. This is achieved by only considering terrain hits
    // when the ray is traveling toward the terrain surface (not away from it).
    if (terrain_config.enabled == 1u && uniforms.terrain_visible == 1u) {
        let terrain_d = terrain_sdf(p);

        // Calculate terrain height at camera position for underground detection
        let camera_terrain_height = fbm_terrain(
            uniforms.camera_pos.xz * terrain_config.frequency,
            i32(terrain_config.octaves),
            terrain_config.seed
        ) * terrain_config.amplitude;

        // Determine if camera is above or below terrain at its current XZ position
        let camera_above_terrain = uniforms.camera_pos.y > camera_terrain_height;

        // Two-sided terrain handling:
        // - If camera is above terrain: use terrain_d normally (blocks rays from above)
        // - If camera is below terrain: only block rays if terrain_d is negative
        //   (meaning the sample point is inside/below terrain surface)
        //   This allows rays to pass through terrain to reach entities above
        var should_include_terrain = false;

        if (camera_above_terrain) {
            // Normal case: include terrain if it's closer than current result
            should_include_terrain = terrain_d < result.dist;
        } else {
            // Underground camera: only include terrain if we're looking at it from below
            // Use absolute distance for two-sided effect, but with reduced priority
            // so entities are still visible through the terrain
            let abs_terrain_d = abs(terrain_d);
            // Only include terrain if it's significantly closer and we're inside it
            should_include_terrain = abs_terrain_d < result.dist * 0.5 && terrain_d < 0.0;
        }

        if (should_include_terrain) {
            result.dist = select(terrain_d, abs(terrain_d), !camera_above_terrain);
            result.entity_index = -1;  // No entity, it's terrain
            result.is_terrain = 1u;
            result.is_preview = 0u;
            result.is_marker = 0u;
            result.is_hands = 0u;
            result.precision_class = 3u;  // Terrain precision (lowest)
        }
    }

    // Evaluate entities
    let count = entity_buffer.count;
    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let entity = entity_buffer.entities[i];
        let d = evaluate_entity_sdf(p, entity);

        if (d < result.dist) {
            result.dist = d;
            result.entity_index = i32(i);
            result.lod_octaves = entity.lod_octaves;
            result.is_terrain = 0u;
            result.is_preview = 0u;
            result.is_marker = 0u;
            result.is_hands = 0u;
            result.precision_class = entity.precision_class;  // US-039: Read precision from entity
        }
    }

    // Evaluate preview object (always evaluated last so we can render it semi-transparently)
    let preview_d = evaluate_preview_sdf(p);
    if (preview_d < result.dist) {
        result.dist = preview_d;
        result.entity_index = -2;  // Special index for preview
        result.lod_octaves = LOD_FULL_OCTAVES;
        result.is_terrain = 0u;
        result.is_preview = 1u;
        result.is_marker = 0u;
        result.is_hands = 0u;
        result.precision_class = 1u;  // Interactive precision for preview
    }

    return result;
}

// ============================================================================
// RAY MARCHING
// ============================================================================

struct RayMarchResult {
    hit: bool,
    position: vec3<f32>,
    distance: f32,
    steps: u32,
    entity_index: i32,
    lod_octaves: u32,  // LOD octave count of the hit entity
    is_terrain: bool,  // True if terrain was hit
    is_preview: bool,  // True if preview object was hit
    is_marker: bool,   // True if distance reference marker was hit
    is_hands: bool,    // US-019: True if first-person hand was hit
    marker_distance: f32,  // Distance of the hit marker from origin (10, 25, 50, 100, 1000)
}

/// Legacy ray marching with adaptive stepping (US-039).
/// Uses scene_sdf() without culling. Used for normal calculation and backward compatibility.
fn ray_march(ray_origin: vec3<f32>, ray_dir: vec3<f32>, max_steps: u32) -> RayMarchResult {
    var result: RayMarchResult;
    result.hit = false;
    result.position = ray_origin;
    result.distance = 0.0;
    result.steps = 0u;
    result.entity_index = -1;
    result.lod_octaves = LOD_FULL_OCTAVES;
    result.is_terrain = false;
    result.is_preview = false;
    result.is_marker = false;
    result.is_hands = false;
    result.marker_distance = 0.0;

    var t: f32 = 0.0;

    for (var i: u32 = 0u; i < max_steps; i = i + 1u) {
        let p = ray_origin + ray_dir * t;
        let hit = scene_sdf(p);

        result.steps = i + 1u;

        if (hit.dist < SURFACE_DIST) {
            result.hit = true;
            result.position = p;
            result.distance = t;
            result.entity_index = hit.entity_index;
            result.lod_octaves = hit.lod_octaves;
            result.is_terrain = hit.is_terrain == 1u;
            result.is_preview = hit.is_preview == 1u;
            result.is_marker = hit.is_marker == 1u;
            result.is_hands = hit.is_hands == 1u;
            result.marker_distance = hit.marker_distance;
            return result;
        }

        if (t > MAX_DIST) {
            break;
        }

        // US-039: Adaptive step size calculation
        let precision = get_precision_multiplier(hit.precision_class);
        let step_size = adaptive_step(t, hit.dist, precision);
        t = t + step_size;
    }

    result.distance = t;
    return result;
}

/// Tile-aware ray marching with adaptive stepping (US-017, US-039).
/// This is the main ray march function for rendering with tile culling enabled.
///
/// US-039: Uses adaptive_step() for step size calculation:
/// - Combines distance-based minimum step with SDF distance
/// - Precision class from nearest SDF modulates step size
///
/// Parameters:
/// - ray_origin: Starting point of the ray (camera position)
/// - ray_dir: Normalized ray direction
/// - max_steps: Maximum number of ray march steps
/// - pixel_coords: Screen-space pixel coordinates for tile lookup
fn ray_march_tiled(ray_origin: vec3<f32>, ray_dir: vec3<f32>, max_steps: u32, pixel_coords: vec2<u32>) -> RayMarchResult {
    var result: RayMarchResult;
    result.hit = false;
    result.position = ray_origin;
    result.distance = 0.0;
    result.steps = 0u;
    result.entity_index = -1;
    result.lod_octaves = LOD_FULL_OCTAVES;
    result.is_terrain = false;
    result.is_preview = false;
    result.is_marker = false;
    result.is_hands = false;
    result.marker_distance = 0.0;

    var t: f32 = 0.0;

    for (var i: u32 = 0u; i < max_steps; i = i + 1u) {
        let p = ray_origin + ray_dir * t;
        let hit = scene_sdf_tiled(p, pixel_coords);

        result.steps = i + 1u;

        if (hit.dist < SURFACE_DIST) {
            result.hit = true;
            result.position = p;
            result.distance = t;
            result.entity_index = hit.entity_index;
            result.lod_octaves = hit.lod_octaves;
            result.is_terrain = hit.is_terrain == 1u;
            result.is_preview = hit.is_preview == 1u;
            result.is_marker = hit.is_marker == 1u;
            result.is_hands = hit.is_hands == 1u;
            result.marker_distance = hit.marker_distance;
            return result;
        }

        if (t > MAX_DIST) {
            break;
        }

        // US-039: Adaptive step size calculation
        let precision = get_precision_multiplier(hit.precision_class);
        let step_size = adaptive_step(t, hit.dist, precision);
        t = t + step_size;
    }

    result.distance = t;
    return result;
}

/// Froxel-aware ray marching with adaptive stepping (US-037, US-039).
///
/// This function tracks the current froxel as the ray progresses through 3D space.
/// It switches SDF lists when crossing froxel boundaries and skips empty froxels
/// by advancing directly to their exit distance.
///
/// US-039: Adaptive step size integration:
/// - Uses adaptive_step() instead of fixed SDF distance for stepping
/// - Step size combines distance-based minimum with SDF distance
/// - Precision class from nearest SDF adjusts step size (Player=1x, Terrain=2x)
/// - Provides smooth transitions at distance band boundaries (5m, 50m, 200m)
///
/// Key optimizations:
/// 1. Only evaluate SDFs in the current froxel's list (not all entities)
/// 2. Skip empty froxels by jumping to exit distance
/// 3. Switch SDF list when crossing froxel boundaries
/// 4. Adaptive stepping based on distance from camera and entity precision
/// 5. Fallback to tile-based culling when froxel system is disabled
///
/// Parameters:
/// - ray_origin: Starting point of the ray (camera position)
/// - ray_dir: Normalized ray direction
/// - max_steps: Maximum number of ray march steps
/// - pixel_coords: Screen-space pixel coordinates for fallback
fn ray_march_froxel(ray_origin: vec3<f32>, ray_dir: vec3<f32>, max_steps: u32, pixel_coords: vec2<u32>) -> RayMarchResult {
    var result: RayMarchResult;
    result.hit = false;
    result.position = ray_origin;
    result.distance = 0.0;
    result.steps = 0u;
    result.entity_index = -1;
    result.lod_octaves = LOD_FULL_OCTAVES;
    result.is_terrain = false;
    result.is_preview = false;
    result.is_marker = false;
    result.is_hands = false;
    result.marker_distance = 0.0;

    // Fallback to tile-based ray marching if froxel system is disabled
    if (!is_froxel_culling_enabled()) {
        return ray_march_tiled(ray_origin, ray_dir, max_steps, pixel_coords);
    }

    var t: f32 = 0.0;
    var current_froxel: u32 = FROXEL_INVALID;
    var steps_in_froxel: u32 = 0u;
    let max_steps_per_froxel: u32 = 32u;  // Prevent infinite loops within a froxel

    // US-039: Debug counters for performance measurement
    // These track how adaptive stepping affects ray march performance
    var total_adaptive_steps: u32 = 0u;  // Steps where adaptive_step > sdf_dist
    var total_sdf_steps: u32 = 0u;       // Steps where sdf_dist was used directly

    for (var i: u32 = 0u; i < max_steps; i = i + 1u) {
        let p = ray_origin + ray_dir * t;

        // Get the froxel for the current position
        let new_froxel = get_froxel_for_position(p);

        // Check if we've crossed into a new froxel
        if (new_froxel != current_froxel) {
            current_froxel = new_froxel;
            steps_in_froxel = 0u;

            // If the new froxel is empty, skip to its exit distance
            if (current_froxel != FROXEL_INVALID && is_froxel_empty(current_froxel)) {
                let exit_dist = get_froxel_exit_distance(p, ray_dir, current_froxel);
                // Advance past the empty froxel with a small epsilon
                t = t + exit_dist + 0.01;
                continue;
            }
        }

        result.steps = i + 1u;
        steps_in_froxel = steps_in_froxel + 1u;

        // Evaluate the scene SDF using froxel-based culling
        let hit = scene_sdf_froxel(p, current_froxel, pixel_coords);

        if (hit.dist < SURFACE_DIST) {
            result.hit = true;
            result.position = p;
            result.distance = t;
            result.entity_index = hit.entity_index;
            result.lod_octaves = hit.lod_octaves;
            result.is_terrain = hit.is_terrain == 1u;
            result.is_preview = hit.is_preview == 1u;
            result.is_marker = hit.is_marker == 1u;
            result.is_hands = hit.is_hands == 1u;
            result.marker_distance = hit.marker_distance;
            return result;
        }

        if (t > MAX_DIST) {
            break;
        }

        // If we're spending too many steps in one froxel, skip to exit
        // This prevents getting stuck in pathological cases
        if (steps_in_froxel >= max_steps_per_froxel && current_froxel != FROXEL_INVALID) {
            let exit_dist = get_froxel_exit_distance(p, ray_dir, current_froxel);
            if (exit_dist > hit.dist && exit_dist < MAX_DIST) {
                t = t + exit_dist + 0.01;
                current_froxel = FROXEL_INVALID;  // Force re-evaluation
                continue;
            }
        }

        // US-039: Adaptive step size calculation
        // Get precision multiplier from the closest SDF's precision class
        let precision = get_precision_multiplier(hit.precision_class);

        // Calculate adaptive step: max(distance-based minimum, sdf_dist) * precision
        let step_size = adaptive_step(t, hit.dist, precision);

        // Debug counter: track when adaptive stepping provides larger steps than raw SDF distance
        let base_step = base_step_for_distance(t);
        if (base_step > hit.dist) {
            total_adaptive_steps = total_adaptive_steps + 1u;
        } else {
            total_sdf_steps = total_sdf_steps + 1u;
        }

        t = t + step_size;
    }

    result.distance = t;
    return result;
}

// ============================================================================
// NORMAL CALCULATION
// ============================================================================

fn calculate_normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(NORMAL_EPSILON, 0.0);

    let n = vec3<f32>(
        scene_sdf(p + e.xyy).dist - scene_sdf(p - e.xyy).dist,
        scene_sdf(p + e.yxy).dist - scene_sdf(p - e.yxy).dist,
        scene_sdf(p + e.yyx).dist - scene_sdf(p - e.yyx).dist
    );

    return normalize(n);
}

/// Analytical world-space normal for box entities (sharp block look).
/// Returns the normal if entity is SDF_BOX; otherwise returns (0,0,0) so caller uses calculate_normal.
fn get_entity_normal_world(p: vec3<f32>, entity: GpuEntity) -> vec3<f32> {
    if (entity.sdf_type != SDF_BOX) {
        return vec3<f32>(0.0, 0.0, 0.0);
    }
    let entity_position = get_entity_position(entity);
    let entity_scale = get_entity_scale(entity);
    let local_p = quat_rotate(quat_inverse(entity.rotation), p - entity_position);
    let scaled_p = local_p / entity_scale;
    let half_extents = vec3<f32>(1.0, 1.0, 1.0);
    let local_n = sdf_box_normal_local(scaled_p, half_extents);
    return quat_rotate(entity.rotation, local_n);
}

// ============================================================================
// SOFT SHADOWS (Unreal-like quality)
// ============================================================================

/// Soft shadow calculation using SDF sphere tracing.
/// Returns shadow factor (0.0 = full shadow, 1.0 = fully lit)
/// k controls softness: higher = softer penumbra (16-64 typical for Unreal-like)
fn soft_shadow(ro: vec3<f32>, rd: vec3<f32>, mint: f32, maxt: f32, k: f32) -> f32 {
    var res = 1.0;
    var t = mint;
    var ph = 1e10;  // Previous hit distance for improved penumbra
    
    for (var i = 0; i < 32; i++) {
        let p = ro + rd * t;
        let h = scene_sdf(p).dist;
        
        if (h < 0.001) {
            return 0.0;  // Hard shadow
        }
        
        // Improved soft shadow with better penumbra estimation
        // Based on IQ's improved soft shadows technique
        let y = h * h / (2.0 * ph);
        let d = sqrt(h * h - y * y);
        res = min(res, k * d / max(0.0, t - y));
        ph = h;
        
        t += h;
        if (t > maxt) {
            break;
        }
    }
    
    return clamp(res, 0.0, 1.0);
}

// ============================================================================
// AMBIENT OCCLUSION
// ============================================================================

/// Calculate ambient occlusion using SDF sampling.
/// Samples along the normal to detect nearby geometry that occludes ambient light.
fn ambient_occlusion(p: vec3<f32>, n: vec3<f32>) -> f32 {
    var occ = 0.0;
    var sca = 1.0;
    
    // 5 samples at increasing distances
    for (var i = 0; i < 5; i++) {
        let h = 0.01 + 0.15 * f32(i) / 4.0;  // Sample distances: 0.01, 0.0475, 0.085, 0.1225, 0.16
        let d = scene_sdf(p + n * h).dist;
        occ += (h - d) * sca;  // Accumulate occlusion when SDF is less than expected
        sca *= 0.75;  // Reduce weight for farther samples
    }
    
    return clamp(1.0 - 2.0 * occ, 0.0, 1.0);
}

// ============================================================================
// PBR LIGHTING (Physically Based Rendering)
// ============================================================================

/// Fresnel-Schlick approximation for PBR
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (1.0 - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

/// GGX/Trowbridge-Reitz normal distribution function
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let n_dot_h2 = n_dot_h * n_dot_h;
    
    let num = a2;
    var denom = (n_dot_h2 * (a2 - 1.0) + 1.0);
    denom = 3.14159265 * denom * denom;
    
    return num / denom;
}

/// Schlick-GGX geometry function
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;
    
    let num = n_dot_v;
    let denom = n_dot_v * (1.0 - k) + k;
    
    return num / denom;
}

/// Smith's method for geometry occlusion (combines view and light)
fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx2 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx1 = geometry_schlick_ggx(n_dot_l, roughness);
    
    return ggx1 * ggx2;
}

/// Full PBR lighting calculation for a single light
fn pbr_light(
    p: vec3<f32>,
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    light_color: vec3<f32>,
    light_intensity: f32,
    albedo: vec3<f32>,
    metallic: f32,
    roughness: f32
) -> vec3<f32> {
    let h = normalize(v + l);
    
    let n_dot_l = max(dot(n, l), 0.0);
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_h = max(dot(n, h), 0.0);
    let h_dot_v = max(dot(h, v), 0.0);
    
    // Base reflectivity (dielectric = 0.04, metallic = albedo color)
    let f0 = mix(vec3<f32>(0.04), albedo, metallic);
    
    // Cook-Torrance BRDF components
    let ndf = distribution_ggx(n_dot_h, roughness);
    let g = geometry_smith(n, v, l, roughness);
    let f = fresnel_schlick(h_dot_v, f0);
    
    // Specular reflection
    let numerator = ndf * g * f;
    let denominator = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = numerator / denominator;
    
    // Energy conservation
    let ks = f;
    var kd = vec3<f32>(1.0) - ks;
    kd = kd * (1.0 - metallic);  // Metals have no diffuse
    
    // Final radiance
    let radiance = light_color * light_intensity;
    return (kd * albedo / 3.14159265 + specular) * radiance * n_dot_l;
}

// ============================================================================
// TONEMAPPING & POST-PROCESSING
// ============================================================================

/// ACES Filmic Tonemapping (Unreal Engine's default)
/// Converts HDR to LDR with cinematic color response
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

/// Apply gamma correction (linear to sRGB)
fn gamma_correct(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(1.0 / 2.2));
}

/// Vignette effect (darkening corners)
fn apply_vignette(color: vec3<f32>, uv: vec2<f32>, intensity: f32) -> vec3<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(uv, center);
    let vignette = 1.0 - smoothstep(0.3, 0.9, dist * intensity);
    return color * vignette;
}

/// Film grain for cinematic feel
fn film_grain(color: vec3<f32>, uv: vec2<f32>, time: f32, intensity: f32) -> vec3<f32> {
    let noise = fract(sin(dot(uv + time, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    return color + (noise - 0.5) * intensity;
}

/// Color grading - lift/gamma/gain
fn color_grade(color: vec3<f32>, lift: vec3<f32>, gamma: vec3<f32>, gain: vec3<f32>) -> vec3<f32> {
    var c = color;
    c = c * gain + lift;
    c = pow(max(c, vec3<f32>(0.0)), 1.0 / gamma);
    return c;
}

/// Chromatic aberration effect
fn chromatic_aberration(uv: vec2<f32>, intensity: f32) -> vec2<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dir = uv - center;
    return dir * intensity;
}

// ============================================================================
// MAIN LIGHTING FUNCTION (PBR + Shadows + AO)
// ============================================================================

fn calculate_lighting(p: vec3<f32>, normal: vec3<f32>, color: vec3<f32>) -> vec3<f32> {
    let view_dir = normalize(uniforms.camera_pos - p);
    
    // Material properties (use defaults, will be overridden by entity data later)
    let roughness = 0.5;
    let metallic = 0.0;
    
    // === SUN LIGHT (Primary directional light) ===
    let sun_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let sun_color = vec3<f32>(1.0, 0.95, 0.85);  // Warm sunlight
    let sun_intensity = 3.0;  // HDR intensity
    
    // Calculate soft shadow for sun
    let shadow_origin = p + normal * 0.02;  // Bias to avoid self-shadowing
    let shadow = soft_shadow(shadow_origin, sun_dir, 0.1, 50.0, 32.0);
    
    // PBR sun contribution
    var final_color = pbr_light(p, normal, view_dir, sun_dir, sun_color, sun_intensity * shadow, color, metallic, roughness);
    
    // === SKY LIGHT (Ambient/environment light) ===
    let sky_color = vec3<f32>(0.4, 0.6, 0.9);  // Blue sky
    let ground_color = vec3<f32>(0.3, 0.25, 0.2);  // Earth bounce
    
    // Hemisphere lighting for ambient
    let sky_factor = normal.y * 0.5 + 0.5;
    let ambient_color = mix(ground_color, sky_color, sky_factor);
    
    // Calculate ambient occlusion
    let ao = ambient_occlusion(p, normal);
    
    // Add ambient contribution (scaled by AO)
    let ambient_intensity = 0.3;
    final_color = final_color + color * ambient_color * ambient_intensity * ao;
    
    // === FILL LIGHT (Camera-relative for visibility) ===
    let fill_dir = normalize(view_dir + vec3<f32>(0.0, 0.3, 0.0));
    let fill_color = vec3<f32>(0.6, 0.7, 0.9);
    let fill_intensity = 0.5;
    final_color = final_color + pbr_light(p, normal, view_dir, fill_dir, fill_color, fill_intensity, color, metallic, roughness);
    
    // === RIM LIGHT (Unreal-style subsurface scattering approximation) ===
    let rim_power = 3.0;
    let rim = pow(1.0 - max(dot(view_dir, normal), 0.0), rim_power);
    let rim_color = vec3<f32>(0.8, 0.9, 1.0);
    final_color = final_color + rim * rim_color * 0.3;
    
    return final_color;
}

/// Enhanced lighting with PBR parameters from entity
fn calculate_lighting_pbr(
    p: vec3<f32>,
    normal: vec3<f32>,
    albedo: vec3<f32>,
    roughness: f32,
    metallic: f32
) -> vec3<f32> {
    let view_dir = normalize(uniforms.camera_pos - p);
    
    // === SUN LIGHT ===
    let sun_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
    let sun_color = vec3<f32>(1.0, 0.95, 0.85);
    let sun_intensity = 3.0;
    
    // Soft shadow
    let shadow_origin = p + normal * 0.02;
    let shadow = soft_shadow(shadow_origin, sun_dir, 0.1, 50.0, 32.0);
    
    var final_color = pbr_light(p, normal, view_dir, sun_dir, sun_color, sun_intensity * shadow, albedo, metallic, roughness);
    
    // === SKY AMBIENT ===
    let sky_color = vec3<f32>(0.4, 0.6, 0.9);
    let ground_color = vec3<f32>(0.3, 0.25, 0.2);
    let sky_factor = normal.y * 0.5 + 0.5;
    let ambient_color = mix(ground_color, sky_color, sky_factor);
    
    let ao = ambient_occlusion(p, normal);
    let ambient_intensity = 0.3;
    final_color = final_color + albedo * ambient_color * ambient_intensity * ao;
    
    // === FILL LIGHT ===
    let fill_dir = normalize(view_dir + vec3<f32>(0.0, 0.3, 0.0));
    let fill_color = vec3<f32>(0.6, 0.7, 0.9);
    let fill_intensity = 0.5;
    final_color = final_color + pbr_light(p, normal, view_dir, fill_dir, fill_color, fill_intensity, albedo, metallic, roughness);
    
    // === RIM/FRESNEL ===
    let rim = pow(1.0 - max(dot(view_dir, normal), 0.0), 3.0);
    let rim_color = vec3<f32>(0.8, 0.9, 1.0);
    final_color = final_color + rim * rim_color * 0.3;
    
    return final_color;
}

// Silhouette mode lighting - minimal computation, single color output
// Used for very distant objects (>= 200 units) to reduce GPU workload
// Returns a flat color with subtle depth variation for visual coherence
fn calculate_silhouette_lighting(color: vec3<f32>, distance: f32) -> vec3<f32> {
    // Simple depth-based darkening for distant silhouettes
    // No normal calculation needed - just use base color with distance falloff
    let depth_factor = clamp(1.0 - (distance - LOD_SILHOUETTE_DISTANCE) * 0.002, 0.3, 0.8);
    return color * depth_factor;
}

// ============================================================================
// VERTEX SHADER
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = positions[vertex_index] * 0.5 + 0.5;
    output.uv.y = 1.0 - output.uv.y;  // Flip Y for screen coords
    return output;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

fn get_ray_direction(uv: vec2<f32>) -> vec3<f32> {
    // Convert UV [0,1] to NDC [-1,1]
    // UV.y was flipped in vertex shader, so we need to un-flip for clip space
    // Clip space: (-1,-1) is bottom-left, (1,1) is top-right
    let ndc_x = uv.x * 2.0 - 1.0;
    let ndc_y = 1.0 - uv.y * 2.0;  // Un-flip Y for correct clip space mapping

    // Use two points on the ray (near and far planes) to compute direction
    // glam::perspective_rh uses standard OpenGL depth: near=-1, far=+1 (NOT reverse-Z)
    let near_clip = vec4<f32>(ndc_x, ndc_y, -1.0, 1.0);
    let far_clip = vec4<f32>(ndc_x, ndc_y, 1.0, 1.0);

    // Transform both points from clip space to world space
    let near_world_h = uniforms.inv_view_proj * near_clip;
    let far_world_h = uniforms.inv_view_proj * far_clip;

    // Perspective divide
    let near_world = near_world_h.xyz / near_world_h.w;
    let far_world = far_world_h.xyz / far_world_h.w;

    // Ray direction is from near to far
    return normalize(far_world - near_world);
}

// Selection highlight color (bright cyan/teal)
const SELECTION_COLOR: vec3<f32> = vec3<f32>(0.2, 0.8, 1.0);
const SELECTION_OUTLINE_THICKNESS: f32 = 0.015;
const SELECTION_GLOW_INTENSITY: f32 = 0.4;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let uv = input.uv;

    // ========================================================================
    // DEBUG MODE 2: Bypass all SDF rendering, output diagnostic colors
    // Use this to verify the shader pipeline and surface presentation work
    // ========================================================================
    if (uniforms.lod_debug_mode == 2u) {
        // Quadrant-based coloring for visibility testing
        // Red = top-left, Green = top-right, Blue = bottom-left, Yellow = bottom-right
        var debug_color = vec3<f32>(0.0, 0.0, 0.0);
        if (uv.x < 0.5 && uv.y < 0.5) {
            debug_color = vec3<f32>(1.0, 0.0, 0.0); // Red - top-left
        } else if (uv.x >= 0.5 && uv.y < 0.5) {
            debug_color = vec3<f32>(0.0, 1.0, 0.0); // Green - top-right
        } else if (uv.x < 0.5 && uv.y >= 0.5) {
            debug_color = vec3<f32>(0.0, 0.0, 1.0); // Blue - bottom-left
        } else {
            debug_color = vec3<f32>(1.0, 1.0, 0.0); // Yellow - bottom-right
        }
        // Overlay entity count visualization: white tint shows entities reached GPU
        // Each entity adds 5% brightness (max 50% for 10+ entities)
        let entity_tint = f32(min(entity_buffer.count, 10u)) * 0.05;
        debug_color = debug_color + vec3<f32>(entity_tint);
        return vec4<f32>(debug_color, 1.0);
    }

    // ========================================================================
    // DEBUG MODE 3: Solid color ray march test (US-0C05)
    // Tests ray marching with hardcoded sphere at origin + solid color output
    // If solid color visible - SDF/lighting is the issue
    // If solid color NOT visible - pipeline/present is the issue
    // ========================================================================
    if (uniforms.lod_debug_mode == 3u) {
        let ray_origin = uniforms.camera_pos;
        let ray_dir = get_ray_direction(uv);

        // Simple ray march against a hardcoded sphere at origin with radius 2.0
        var t: f32 = 0.0;
        let max_steps = 64u;
        let sphere_radius = 2.0;

        for (var i: u32 = 0u; i < max_steps; i = i + 1u) {
            let p = ray_origin + ray_dir * t;
            // Simple sphere SDF at origin
            let d = length(p) - sphere_radius;

            if (d < SURFACE_DIST) {
                // HIT: Return bright magenta for visibility
                return vec4<f32>(1.0, 0.0, 1.0, 1.0);
            }

            if (t > MAX_DIST) {
                break;
            }

            t = t + d;
        }

        // MISS: Return dark gray background
        return vec4<f32>(0.1, 0.1, 0.1, 1.0);
    }

    // ========================================================================
    // DEBUG MODE 4: Test scene SDF with solid color output (US-0C05)
    // Uses the actual scene_sdf() but returns solid color on hit
    // This tests the entity buffer and SDF evaluation without lighting complexity
    // ========================================================================
    if (uniforms.lod_debug_mode == 4u) {
        let ray_origin = uniforms.camera_pos;
        let ray_dir = get_ray_direction(uv);

        // Ray march using the actual scene SDF
        let result = ray_march(ray_origin, ray_dir, uniforms.step_count);

        if (result.hit) {
            // HIT: Return bright cyan for visibility (different from mode 3)
            return vec4<f32>(0.0, 1.0, 1.0, 1.0);
        }

        // MISS: Return dark gray background
        return vec4<f32>(0.1, 0.1, 0.1, 1.0);
    }

    // ========================================================================
    // DEBUG MODE 5: Visualize ray directions (US-0C05)
    // Tests if ray direction calculation is correct
    // R = positive X component, G = positive Y component, B = positive Z component
    // ========================================================================
    if (uniforms.lod_debug_mode == 5u) {
        let ray_origin = uniforms.camera_pos;
        let ray_dir = get_ray_direction(uv);

        // Visualize ray direction as RGB
        let debug_color = ray_dir * 0.5 + 0.5; // Map -1..1 to 0..1
        return vec4<f32>(debug_color, 1.0);
    }

    // ========================================================================
    // DEBUG MODE 6: Uniform Buffer Verification (US-0G05)
    // Visualizes uniform values directly to verify GPU buffer updates:
    // - Left column: Camera position (R=X, G=Y, B=Z normalized)
    // - Middle column: Resolution verification (gradient based on pixel coords)
    // - Right column: Step count visualization + time pulse
    // If you see correct gradients/colors, uniforms are reaching the GPU correctly
    // ========================================================================
    if (uniforms.lod_debug_mode == 6u) {
        var out_color = vec3<f32>(0.0, 0.0, 0.0);

        // Divide screen into thirds vertically
        let third = 1.0 / 3.0;

        if (uv.x < third) {
            // LEFT COLUMN: Camera position visualization
            // Map camera_pos components to color channels
            // Assuming reasonable camera range of -50 to 50 for each axis
            let cam_x = clamp((uniforms.camera_pos.x + 50.0) / 100.0, 0.0, 1.0);
            let cam_y = clamp((uniforms.camera_pos.y + 50.0) / 100.0, 0.0, 1.0);
            let cam_z = clamp((uniforms.camera_pos.z + 50.0) / 100.0, 0.0, 1.0);

            // Top region shows X position (red gradient)
            // Middle region shows Y position (green gradient)
            // Bottom region shows Z position (blue gradient)
            if (uv.y < 0.33) {
                // Z position - blue
                out_color = vec3<f32>(0.0, 0.0, cam_z);
                // Add stripe pattern to show Z value numerically
                let stripe = step(0.5, fract(uniforms.camera_pos.z * 0.2 + uv.y * 10.0));
                out_color = out_color + vec3<f32>(stripe * 0.2);
            } else if (uv.y < 0.66) {
                // Y position - green
                out_color = vec3<f32>(0.0, cam_y, 0.0);
                let stripe = step(0.5, fract(uniforms.camera_pos.y * 0.2 + uv.y * 10.0));
                out_color = out_color + vec3<f32>(stripe * 0.2);
            } else {
                // X position - red
                out_color = vec3<f32>(cam_x, 0.0, 0.0);
                let stripe = step(0.5, fract(uniforms.camera_pos.x * 0.2 + uv.y * 10.0));
                out_color = out_color + vec3<f32>(stripe * 0.2);
            }

            // Warn if camera is at origin (0,0,0) - bright magenta warning
            let cam_at_origin = abs(uniforms.camera_pos.x) < 0.001
                             && abs(uniforms.camera_pos.y) < 0.001
                             && abs(uniforms.camera_pos.z) < 0.001;
            if (cam_at_origin) {
                out_color = vec3<f32>(1.0, 0.0, 1.0); // Magenta warning
            }

        } else if (uv.x < 2.0 * third) {
            // MIDDLE COLUMN: Resolution verification
            // Creates a gradient that should match pixel coords exactly
            // If resolution is wrong, this will look distorted

            // Expected pixel coordinates
            let expected_x = uv.x * uniforms.resolution.x;
            let expected_y = uv.y * uniforms.resolution.y;

            // Create checkerboard pattern based on actual pixel coords
            // Each square is 32 pixels wide
            let checker_x = step(0.5, fract(expected_x / 32.0));
            let checker_y = step(0.5, fract(expected_y / 32.0));
            let checker = abs(checker_x - checker_y);

            // Color based on which quadrant we're in
            let in_right = f32(uv.x > 0.5);
            let in_bottom = f32(uv.y > 0.5);

            out_color = vec3<f32>(
                checker * 0.3 + in_right * 0.4,
                checker * 0.3 + in_bottom * 0.4,
                0.2
            );

            // Add resolution text hint: brightness shows if resolution is reasonable
            // Bright = good resolution (800-2000), dim = too small or too large
            let res_quality = clamp(
                min(uniforms.resolution.x, uniforms.resolution.y) / 1000.0,
                0.2,
                1.0
            );
            out_color = out_color * res_quality;

        } else {
            // RIGHT COLUMN: Step count and time verification
            // Top: step count visualization (horizontal bars)
            // Bottom: time-based animation (proves uniforms update each frame)

            if (uv.y > 0.5) {
                // Step count visualization
                // Map step_count (typical range 32-256) to visual bars
                let normalized_steps = clamp(f32(uniforms.step_count) / 256.0, 0.0, 1.0);

                // Create horizontal bar that fills based on step count
                let bar_fill = f32(uv.y - 0.5) * 2.0; // 0-1 in this region
                if (bar_fill < normalized_steps) {
                    // Gradient from red (low) to green (high step count)
                    out_color = vec3<f32>(
                        1.0 - normalized_steps,
                        normalized_steps,
                        0.2
                    );
                } else {
                    out_color = vec3<f32>(0.1, 0.1, 0.1); // Empty bar background
                }

                // If step_count is 0, show warning
                if (uniforms.step_count == 0u) {
                    out_color = vec3<f32>(1.0, 0.0, 0.0); // Red warning
                }

            } else {
                // Time-based animation (proves uniforms update)
                // Cycles through colors based on time
                let time_phase = uniforms.time * 0.5;
                let r = sin(time_phase) * 0.5 + 0.5;
                let g = sin(time_phase + 2.094) * 0.5 + 0.5; // +2π/3
                let b = sin(time_phase + 4.189) * 0.5 + 0.5; // +4π/3

                out_color = vec3<f32>(r, g, b);

                // Add pulsing ring to show time updates
                let ring_center = vec2<f32>(0.833, 0.25); // Center of this region
                let dist_to_center = length(uv - ring_center);
                let ring_radius = 0.1 + sin(uniforms.time * 3.0) * 0.05;
                let ring = smoothstep(ring_radius - 0.02, ring_radius, dist_to_center)
                         - smoothstep(ring_radius, ring_radius + 0.02, dist_to_center);
                out_color = out_color + vec3<f32>(ring * 0.5);
            }
        }

        return vec4<f32>(out_color, 1.0);
    }

    // ========================================================================
    // DEBUG MODE 7: Simple Clear Color Only (US-0K04)
    // Renders only a solid color without any shader processing.
    // This is the most minimal debug mode to verify:
    // - Surface/texture copy to iced works
    // - Shader output is visible on screen
    // - No shader code path issues
    // If you see solid bright lime green, the texture copy to iced surface works.
    // If you see nothing/black, there's an issue with surface presentation.
    // ========================================================================
    if (uniforms.lod_debug_mode == 7u) {
        // Return a very distinctive bright lime green color
        // This is intentionally chosen to be visible and unmistakable
        return vec4<f32>(0.2, 1.0, 0.2, 1.0);
    }

    // ========================================================================
    // ENTITY DEBUG MODE 1: Entity Position as RGB Color (US-0M04)
    // Renders entity positions as colors to verify entity data reaches the shader.
    // R = X position, G = Y position, B = Z position (normalized to view range)
    // Press F9 to cycle through entity debug modes.
    // ========================================================================
    if (uniforms.entity_debug_mode == 1u) {
        let ray_origin = uniforms.camera_pos;
        let ray_dir = get_ray_direction(uv);
        let result = ray_march(ray_origin, ray_dir, uniforms.step_count);

        if (result.hit && result.entity_index >= 0) {
            let entity = entity_buffer.entities[u32(result.entity_index)];
            let entity_pos = get_entity_position(entity);

            // Normalize position to 0-1 range for visualization
            // Assumes entities are within -10 to 10 range for each axis
            let normalized_pos = (entity_pos + vec3<f32>(10.0)) / 20.0;
            let clamped_pos = clamp(normalized_pos, vec3<f32>(0.0), vec3<f32>(1.0));

            // Log first entity data (only for pixel near center to avoid spam)
            // This creates a visible "marker" in the center showing debug info
            let center_dist = length(uv - vec2<f32>(0.5, 0.5));
            if (center_dist < 0.02) {
                // Center marker: show white to indicate entity was hit
                return vec4<f32>(1.0, 1.0, 1.0, 1.0);
            }

            // Return position as RGB color
            return vec4<f32>(clamped_pos, 1.0);
        }

        // No entity hit - show dark background
        return vec4<f32>(0.1, 0.1, 0.15, 1.0);
    }

    // ========================================================================
    // ENTITY DEBUG MODE 2: Entity Count as Brightness (US-0M04)
    // Renders brightness based on entity count to verify entity buffer count.
    // More entities = brighter color.
    // Also shows entity count visually in the corner.
    // ========================================================================
    if (uniforms.entity_debug_mode == 2u) {
        let count = entity_buffer.count;

        // Normalize count to brightness (0-10 entities = 0-1 brightness)
        let brightness = clamp(f32(count) / 10.0, 0.0, 1.0);

        // Base color based on entity count
        var base_color = vec3<f32>(brightness * 0.5, brightness * 0.8, brightness);

        // Add visual indicator bars for entity count (each bar = 1 entity)
        // Displayed at bottom of screen
        if (uv.y > 0.95) {
            let bar_index = u32(uv.x * 20.0); // 20 possible bars
            if (bar_index < count) {
                // Entity bar - bright cyan
                base_color = vec3<f32>(0.2, 1.0, 1.0);
            } else {
                // Empty slot - dark
                base_color = vec3<f32>(0.1, 0.1, 0.1);
            }
        }

        // Show ray march result with count-based tinting
        let ray_origin = uniforms.camera_pos;
        let ray_dir = get_ray_direction(uv);
        let result = ray_march(ray_origin, ray_dir, uniforms.step_count);

        if (result.hit && uv.y <= 0.95) {
            // Hit an entity - show count-tinted white
            let hit_brightness = 0.5 + brightness * 0.5;
            return vec4<f32>(hit_brightness, hit_brightness, hit_brightness, 1.0);
        }

        return vec4<f32>(base_color, 1.0);
    }

    // Get ray from camera
    let ray_origin = uniforms.camera_pos;
    let ray_dir = get_ray_direction(uv);

    // Use step count from uniforms (configurable quality)
    let max_steps = uniforms.step_count;

    // US-017: Get pixel coordinates for tile culling
    // input.position.xy contains fragment position in pixel coordinates
    let pixel_coords = vec2<u32>(u32(input.position.x), u32(input.position.y));

    // Ray march the scene using tile-aware version for performance
    // This only evaluates entities that overlap the current pixel's tile
    let result = ray_march_tiled(ray_origin, ray_dir, max_steps, pixel_coords);

    // Background color (bg-deep from visual design, converted to linear)
    let bg_color = vec3<f32>(0.05, 0.05, 0.06);

    if (!result.hit) {
        // ====================================================================
        // ATMOSPHERIC SKY (Unreal-style sky gradient with sun)
        // ====================================================================
        let sun_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
        
        // Base sky gradient (darker at top, lighter at horizon)
        let horizon_color = vec3<f32>(0.5, 0.6, 0.8);  // Light blue horizon
        let zenith_color = vec3<f32>(0.2, 0.35, 0.65);  // Deep blue zenith
        let ground_color = vec3<f32>(0.15, 0.12, 0.1);  // Dark brown below horizon
        
        // Calculate sky color based on ray direction
        let up_amount = ray_dir.y;
        var sky_color: vec3<f32>;
        
        if (up_amount > 0.0) {
            // Above horizon - blue gradient
            let sky_t = pow(up_amount, 0.5);  // Non-linear for more horizon color
            sky_color = mix(horizon_color, zenith_color, sky_t);
        } else {
            // Below horizon - dark ground
            let ground_t = pow(-up_amount, 0.7);
            sky_color = mix(horizon_color * 0.3, ground_color, ground_t);
        }
        
        // Sun disk and glow
        let sun_dot = max(dot(ray_dir, sun_dir), 0.0);
        
        // Sharp sun disk
        let sun_disk = smoothstep(0.9995, 0.9998, sun_dot);
        let sun_disk_color = vec3<f32>(1.0, 0.95, 0.8) * 5.0;  // HDR sun
        
        // Sun glow (large soft halo)
        let sun_glow = pow(sun_dot, 8.0) * 0.5;
        let sun_glow_color = vec3<f32>(1.0, 0.8, 0.5);
        
        // Sun haze (atmospheric scattering near sun)
        let sun_haze = pow(sun_dot, 2.0) * 0.15;
        let sun_haze_color = vec3<f32>(1.0, 0.9, 0.7);
        
        // Combine sky + sun effects
        sky_color = sky_color + sun_haze_color * sun_haze;
        sky_color = sky_color + sun_glow_color * sun_glow;
        sky_color = mix(sky_color, sun_disk_color, sun_disk);
        
        // Apply tonemapping and post-processing to sky
        sky_color = aces_tonemap(sky_color);
        sky_color = apply_vignette(sky_color, uv, 1.2);
        sky_color = gamma_correct(sky_color);
        
        return vec4<f32>(sky_color, 1.0);
    }

    // Get surface color based on hit type (terrain, entity, or preview)
    var surface_color = vec3<f32>(0.7, 0.7, 0.7);
    var is_selected = 0.0;
    var lod_octaves = result.lod_octaves;
    var is_preview_hit = result.is_preview;

    if (result.is_preview) {
        // Preview object: use the preview ghost color
        surface_color = PREVIEW_COLOR;
    } else if (result.is_hands) {
        // US-019: First-person hands - use skin tone color matching player body
        surface_color = FP_HAND_COLOR;
    } else if (result.is_marker) {
        // Distance reference marker trees - color-coded by distance
        // Trunk is brown, foliage is green with distance-based tint
        let hit_height = result.position.y;
        let is_trunk = hit_height < result.marker_distance * 0.02;  // Lower part is trunk

        if (is_trunk) {
            // Brown trunk color
            surface_color = vec3<f32>(0.4, 0.25, 0.1);
        } else {
            // Foliage color varies by distance marker:
            // 10m = bright green, 25m = yellow-green, 50m = yellow, 100m = orange, 1000m = red
            if (result.marker_distance < 15.0) {
                // 10m marker - bright green
                surface_color = vec3<f32>(0.2, 0.8, 0.2);
            } else if (result.marker_distance < 30.0) {
                // 25m marker - yellow-green
                surface_color = vec3<f32>(0.5, 0.8, 0.2);
            } else if (result.marker_distance < 75.0) {
                // 50m marker - yellow
                surface_color = vec3<f32>(0.8, 0.8, 0.2);
            } else if (result.marker_distance < 500.0) {
                // 100m marker - orange
                surface_color = vec3<f32>(0.9, 0.5, 0.1);
            } else {
                // 1000m marker - red (most distant, easy to spot)
                surface_color = vec3<f32>(0.9, 0.2, 0.2);
            }
        }
    } else if (result.is_terrain) {
        // UE5-style terrain materials - compute early, use in lighting
        // Note: Full material is computed below in lighting section for PBR
        let terrain_normal = calculate_normal(result.position);
        let terrain_mat = get_terrain_material(result.position, terrain_normal, result.distance);
        surface_color = terrain_mat.albedo;
    } else if (result.entity_index >= 0) {
        let entity = entity_buffer.entities[u32(result.entity_index)];
        // Use helper function to get vec3 color from scalar fields
        surface_color = get_entity_color(entity);
        is_selected = entity.selected;
        lod_octaves = entity.lod_octaves;
    }
    // LOD octaves are available for use with FBM/noise functions:
    // - LOD_FULL_OCTAVES (8): Full detail for near objects (< 10 units)
    // - LOD_MEDIUM_OCTAVES (4): Medium detail (10-50 units)
    // - LOD_LOW_OCTAVES (2): Low detail (50-200 units)
    // - LOD_SILHOUETTE_OCTAVES (1): Minimal detail (>= 200 units)
    // Use fbm_lod(p, distance, base_octaves) for LOD-aware noise

    // Determine if we're in silhouette mode or transitioning to it
    let silhouette_blend_start = LOD_SILHOUETTE_DISTANCE - LOD_SILHOUETTE_BLEND_RANGE;
    let is_silhouette_distance = result.distance >= LOD_SILHOUETTE_DISTANCE;
    let is_blending_to_silhouette = result.distance >= silhouette_blend_start && result.distance < LOD_SILHOUETTE_DISTANCE;

    var final_lit_color: vec3<f32>;

    if (is_silhouette_distance) {
        // Full silhouette mode: single color, no normal calculation, minimal SDF ops
        final_lit_color = calculate_silhouette_lighting(surface_color, result.distance);
    } else if (is_blending_to_silhouette) {
        // Smooth blend between full lighting and silhouette mode
        // This prevents visual popping when transitioning
        let blend_factor = (result.distance - silhouette_blend_start) / LOD_SILHOUETTE_BLEND_RANGE;

        // Calculate full lighting
        var normal = calculate_normal(result.position);
        if (result.entity_index >= 0) {
            let entity = entity_buffer.entities[u32(result.entity_index)];
            let entity_n = get_entity_normal_world(result.position, entity);
            if (length(entity_n) > 0.5) {
                normal = entity_n;
            }
        }
        var full_lit_color: vec3<f32>;

        if (result.is_terrain) {
            // Use terrain PBR material even during blend
            let terrain_mat = get_terrain_material(result.position, normal, result.distance);
            let perturbed_normal = normalize(normal + terrain_mat.normal_offset);
            full_lit_color = calculate_lighting_pbr(
                result.position,
                perturbed_normal,
                terrain_mat.albedo,
                terrain_mat.roughness,
                terrain_mat.metallic
            );
        } else {
            full_lit_color = calculate_lighting(result.position, normal, surface_color);
        }

        // Calculate silhouette color
        let silhouette_color = calculate_silhouette_lighting(surface_color, result.distance);

        // Smooth blend using smoothstep for gradual transition
        let smooth_blend = smoothstep(0.0, 1.0, blend_factor);
        final_lit_color = mix(full_lit_color, silhouette_color, smooth_blend);

        // Apply selection highlight if needed (with reduced intensity during blend)
        if (is_selected > 0.5) {
            let view_dir = normalize(uniforms.camera_pos - result.position);
            let fresnel = pow(1.0 - max(dot(view_dir, normal), 0.0), 2.5);
            let rim_glow = fresnel * SELECTION_GLOW_INTENSITY * (1.0 - smooth_blend);
            final_lit_color = mix(final_lit_color, final_lit_color + SELECTION_COLOR * rim_glow, 1.0);
            final_lit_color = mix(final_lit_color, final_lit_color * vec3<f32>(1.1, 1.2, 1.3), 0.15 * (1.0 - smooth_blend));
        }
    } else {
        // Full detail mode: calculate normal and apply full lighting
        var normal = calculate_normal(result.position);
        if (result.entity_index >= 0) {
            let entity = entity_buffer.entities[u32(result.entity_index)];
            let entity_n = get_entity_normal_world(result.position, entity);
            if (length(entity_n) > 0.5) {
                normal = entity_n;
            }
        }
        var lit_color: vec3<f32>;

        if (result.is_terrain) {
            // UE5-style terrain with full PBR materials
            let terrain_mat = get_terrain_material(result.position, normal, result.distance);
            
            // Apply micro-normal perturbation for surface detail
            let perturbed_normal = normalize(normal + terrain_mat.normal_offset);
            
            // Full PBR lighting with terrain material properties
            lit_color = calculate_lighting_pbr(
                result.position,
                perturbed_normal,
                terrain_mat.albedo,
                terrain_mat.roughness,
                terrain_mat.metallic
            );
            
            // Subsurface scattering approximation for grass
            // Light passes through thin grass blades from behind
            if (terrain_mat.subsurface > 0.0) {
                let view_dir = normalize(uniforms.camera_pos - result.position);
                let sun_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
                
                // SSS: light coming through from behind (view aligned with light)
                let sss_dot = saturate(dot(-view_dir, sun_dir));
                let sss_power = pow(sss_dot, 3.0);
                
                // Warm subsurface color for vegetation
                let sss_color = vec3<f32>(0.4, 0.55, 0.15) * 2.0;  // Bright green-yellow
                let sss_contribution = sss_color * sss_power * terrain_mat.subsurface;
                
                lit_color = lit_color + sss_contribution;
            }
        } else {
            lit_color = calculate_lighting(result.position, normal, surface_color);
        }
        
        final_lit_color = lit_color;

        // Apply selection highlight effect
        if (is_selected > 0.5) {
            // Add rim lighting effect for selected objects
            let view_dir = normalize(uniforms.camera_pos - result.position);
            let fresnel = pow(1.0 - max(dot(view_dir, normal), 0.0), 2.5);
            let rim_glow = fresnel * SELECTION_GLOW_INTENSITY;

            // Blend selection color with surface
            final_lit_color = mix(lit_color, lit_color + SELECTION_COLOR * rim_glow, 1.0);

            // Add subtle overall tint to selected objects
            final_lit_color = mix(final_lit_color, final_lit_color * vec3<f32>(1.1, 1.2, 1.3), 0.15);
        }
    }

    // ========================================================================
    // ATMOSPHERIC FOG (Unreal-style exponential height fog)
    // ========================================================================
    // Distance-based fog with height falloff for cinematic atmosphere
    let fog_density = 0.00015;  // Slightly denser for more atmosphere
    let fog_height_falloff = 0.02;  // How quickly fog density decreases with height
    let fog_base_height = 0.0;  // Fog is densest at ground level
    
    // Height-based fog density modification
    let height_above_fog = max(result.position.y - fog_base_height, 0.0);
    let height_fog_factor = exp(-height_above_fog * fog_height_falloff);
    
    // Combined distance and height fog
    let fog_amount = 1.0 - exp(-result.distance * fog_density * height_fog_factor);
    
    // Atmospheric fog color (bluish haze for distance)
    let fog_near_color = vec3<f32>(0.6, 0.7, 0.9);  // Blue-ish near fog
    let fog_far_color = vec3<f32>(0.5, 0.55, 0.65);  // Desaturated far fog
    let fog_blend = smoothstep(0.0, 500.0, result.distance);
    let fog_color = mix(fog_near_color, fog_far_color, fog_blend);
    
    var final_color = mix(final_lit_color, fog_color * 0.3, fog_amount);

    // LOD Debug Mode: Override color with LOD-based debug visualization
    if (uniforms.lod_debug_mode == 1u) {
        // Get LOD debug color based on distance from camera
        let lod_color = get_lod_debug_color(result.distance);
        // Apply lighting to the debug color to maintain some depth perception
        // Use a simplified lighting model with the debug color
        let normal = calculate_normal(result.position);
        let light_dir = normalize(vec3<f32>(0.5, 1.0, 0.3));
        let diff = max(dot(normal, light_dir), 0.0);
        let ambient = 0.3;
        let lit_lod_color = lod_color * (ambient + diff * 0.7);
        // Apply fog to debug color as well
        final_color = mix(lit_lod_color, bg_color, fog_amount * 0.5);
    }

    // Apply semi-transparency for preview objects
    // Blend with a pulsing glow effect for visibility
    if (is_preview_hit) {
        // Add pulsing glow effect based on time
        let pulse = 0.3 + 0.2 * sin(uniforms.time * 3.0);
        let glow_color = PREVIEW_COLOR * (1.0 + pulse);

        // Blend final color with background for semi-transparency
        // The preview appears as a ghostly overlay
        let preview_blend = PREVIEW_ALPHA + pulse * 0.1;
        final_color = mix(bg_color, glow_color, preview_blend);

        // Add edge glow effect using fresnel
        let normal = calculate_normal(result.position);
        let view_dir = normalize(uniforms.camera_pos - result.position);
        let fresnel = pow(1.0 - max(dot(view_dir, normal), 0.0), 2.0);
        final_color = final_color + PREVIEW_COLOR * fresnel * 0.5;
    }

    // ========================================================================
    // POST-PROCESSING PIPELINE (Unreal-style cinematic look)
    // ========================================================================
    
    // 1. ACES Filmic Tonemapping (converts HDR to LDR with cinematic response)
    final_color = aces_tonemap(final_color);
    
    // 2. Subtle color grading (warm shadows, cool highlights)
    let lift = vec3<f32>(0.02, 0.015, 0.025);   // Slight blue lift in shadows
    let gamma_adj = vec3<f32>(1.0, 1.0, 1.0);   // Neutral gamma
    let gain = vec3<f32>(1.05, 1.02, 1.0);      // Slight warm gain
    final_color = color_grade(final_color, lift, gamma_adj, gain);
    
    // 3. Vignette (subtle darkening at edges for focus)
    final_color = apply_vignette(final_color, uv, 1.2);
    
    // 4. Film grain (very subtle for cinematic texture)
    final_color = film_grain(final_color, uv, uniforms.time, 0.02);
    
    // 5. Gamma correction (linear to sRGB)
    final_color = gamma_correct(final_color);

    return vec4<f32>(final_color, 1.0);
}
