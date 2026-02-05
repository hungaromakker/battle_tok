//! GPU Uniform Buffers
//!
//! Data structures for GPU uniform buffers in the arena shader pipeline.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

/// Uniforms sent to GPU - includes projectile data
/// Matches WGSL struct layout (std140 alignment rules):
/// - vec3<f32> followed by f32 packs into 16 bytes
/// - vec3<f32> followed by vec3 needs alignment padding
/// - array<vec4> needs 16-byte alignment
/// Total: 656 bytes
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [[f32; 4]; 4], // 64 bytes (offset 0)
    pub camera_pos: [f32; 3],     // 12 bytes (offset 64)
    pub time: f32,                // 4 bytes (offset 76) - packs with camera_pos
    pub sun_dir: [f32; 3],        // 12 bytes (offset 80)
    pub fog_density: f32,         // 4 bytes (offset 92) - packs with sun_dir
    pub fog_color: [f32; 3],      // 12 bytes (offset 96)
    pub ambient: f32,             // 4 bytes (offset 108) - packs with fog_color
    // Projectile data (up to 32 projectiles)
    pub projectile_count: u32,          // 4 bytes (offset 112)
    pub _pad_before_padding1: [f32; 3], // 12 bytes (offset 116) - align to 16-byte boundary
    pub _padding1: [f32; 3],            // 12 bytes (offset 128)
    pub _pad_before_array: f32,         // 4 bytes (offset 140) - align array to 16-byte boundary
    pub projectile_positions: [[f32; 4]; 32], // 512 bytes (offset 144)
                                        // Total: 656 bytes
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            time: 0.0,
            sun_dir: [0.577, 0.577, 0.577],
            fog_density: 0.0002,
            fog_color: [0.7, 0.8, 0.95],
            ambient: 0.3,
            projectile_count: 0,
            _pad_before_padding1: [0.0; 3],
            _padding1: [0.0; 3],
            _pad_before_array: 0.0,
            projectile_positions: [[0.0; 4]; 32],
        }
    }
}

// Compile-time size check for Uniforms (must match WGSL shader expectation)
const _: () = assert!(std::mem::size_of::<Uniforms>() == 656);

/// Model uniforms for hex-prism rendering (US-012)
/// Matches the ModelUniforms struct in hex_prism.wgsl
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct HexPrismModelUniforms {
    pub model: [[f32; 4]; 4],         // 64 bytes - model transform
    pub normal_matrix: [[f32; 3]; 3], // 36 bytes - normal transform (3x3)
    pub _padding: [f32; 3],           // 12 bytes - pad to 112 bytes total
}

impl Default for HexPrismModelUniforms {
    fn default() -> Self {
        Self {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            normal_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            _padding: [0.0; 3],
        }
    }
}

/// SDF Cannon Uniforms (US-013)
/// Matches the Uniforms struct in sdf_cannon.wgsl
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SdfCannonUniforms {
    pub view_proj: [[f32; 4]; 4],     // 64 bytes
    pub inv_view_proj: [[f32; 4]; 4], // 64 bytes
    pub camera_pos: [f32; 3],         // 12 bytes
    pub time: f32,                    // 4 bytes
    pub sun_dir: [f32; 3],            // 12 bytes
    pub fog_density: f32,             // 4 bytes
    pub fog_color: [f32; 3],          // 12 bytes
    pub ambient: f32,                 // 4 bytes
}

impl Default for SdfCannonUniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            time: 0.0,
            sun_dir: [0.577, 0.577, 0.577],
            fog_density: 0.0002,
            fog_color: [0.7, 0.8, 0.95],
            ambient: 0.3,
        }
    }
}

/// SDF Cannon Data (US-013)
/// Matches the CannonData struct in sdf_cannon.wgsl
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SdfCannonData {
    pub world_pos: [f32; 3],       // 12 bytes
    pub _pad0: f32,                // 4 bytes
    pub barrel_rotation: [f32; 4], // 16 bytes (quaternion)
    pub color: [f32; 3],           // 12 bytes
    pub _pad1: f32,                // 4 bytes
}

impl Default for SdfCannonData {
    fn default() -> Self {
        Self {
            world_pos: [0.0, 0.0, 0.0],
            _pad0: 0.0,
            barrel_rotation: [0.0, 0.0, 0.0, 1.0], // identity quaternion
            color: [0.4, 0.35, 0.3],               // bronze/metallic color
            _pad1: 0.0,
        }
    }
}

// ============================================================================
// BATCH 1: Visual Upgrade Uniform Structs
// ============================================================================

/// Enhanced Terrain Params (terrain_enhanced.wgsl)
/// Height-based material bands + slope detection + noise variation
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct TerrainParams {
    // Material colors
    pub grass: [f32; 3],
    pub _pad0: f32,
    pub dirt: [f32; 3],
    pub _pad1: f32,
    pub rock: [f32; 3],
    pub _pad2: f32,
    pub snow: [f32; 3],
    pub _pad3: f32,
    // Height band thresholds (world units)
    pub dirt_start: f32,
    pub dirt_end: f32,
    pub rock_start: f32,
    pub rock_end: f32,
    pub snow_start: f32,
    pub snow_end: f32,
    // Noise params
    pub noise_scale: f32,
    pub noise_strength: f32,
}

impl Default for TerrainParams {
    fn default() -> Self {
        Self {
            // Apocalyptic scorched terrain palette (US-P2-012)
            grass: [0.15, 0.18, 0.10], // Scorched olive (was bright green)
            _pad0: 0.0,
            dirt: [0.28, 0.20, 0.14], // Ashen brown (volcanic ash)
            _pad1: 0.0,
            rock: [0.25, 0.22, 0.24], // Dark volcanic with purple tint
            _pad2: 0.0,
            snow: [0.50, 0.48, 0.45], // Ash/dust peaks (was white snow)
            _pad3: 0.0,
            // Height bands tuned for terrain scale
            dirt_start: 0.6,
            dirt_end: 1.6,
            rock_start: 1.4,
            rock_end: 2.8,
            snow_start: 2.6,
            snow_end: 3.6,
            // Noise variation
            noise_scale: 0.25,
            noise_strength: 0.12,
        }
    }
}

/// Lava Params (lava.wgsl)
/// Animated flowing lava with emissive cracks
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct LavaParams {
    pub time: f32,
    pub emissive_strength: f32, // 0.8..2.5 (HDR)
    pub scale: f32,             // 0.15..0.6
    pub speed: f32,             // 0.1..0.6
    pub crack_sharpness: f32,   // 0.78..0.95
    pub normal_strength: f32,   // 0.3..1.0
    pub _pad0: [f32; 2],
    // Colors
    pub core_color: [f32; 3], // Bright molten
    pub _pad1: f32,
    pub crust_color: [f32; 3], // Dark cooled crust
    pub _pad2: f32,
}

impl Default for LavaParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            emissive_strength: 1.8,
            scale: 0.25,
            speed: 0.35,
            crack_sharpness: 0.85,
            normal_strength: 0.6,
            _pad0: [0.0; 2],
            core_color: [2.4, 0.65, 0.08], // HDR orange-yellow
            _pad1: 0.0,
            crust_color: [0.05, 0.01, 0.01], // Dark red-black
            _pad2: 0.0,
        }
    }
}

/// Storm Sky Params (sky_storm_procedural.wgsl)
/// Multi-color roguelike palette with swirling clouds and lightning
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SkyStormParams {
    pub time: f32,
    pub cloud_speed: f32,         // 0.02..0.1
    pub cloud_scale: f32,         // 2.0..6.0
    pub cloud_density: f32,       // 0.3..0.8
    pub lightning_intensity: f32, // 0.0..1.0
    pub lightning_frequency: f32, // 5.0..10.0 seconds
    pub _pad0: [f32; 2],
    // Colors
    pub col_top: [f32; 3], // Dark sky
    pub _pad1: f32,
    pub col_mid: [f32; 3], // Purple mid
    pub _pad2: f32,
    pub col_horizon: [f32; 3], // Orange-red horizon
    pub _pad3: f32,
    pub col_magic: [f32; 3], // Magic veins
    pub _pad4: f32,
}

impl Default for SkyStormParams {
    fn default() -> Self {
        Self {
            time: 0.0,
            cloud_speed: 0.05,
            cloud_scale: 4.0,
            cloud_density: 0.5,
            lightning_intensity: 0.6,
            lightning_frequency: 7.0,
            _pad0: [0.0; 2],
            col_top: [0.08, 0.06, 0.14], // Dark purple-black
            _pad1: 0.0,
            col_mid: [0.22, 0.12, 0.30], // Purple mid
            _pad2: 0.0,
            col_horizon: [0.70, 0.22, 0.18], // Orange-red fire
            _pad3: 0.0,
            col_magic: [0.25, 0.45, 0.95], // Magic blue
            _pad4: 0.0,
        }
    }
}

/// Fog Post-Process Params (fog_post.wgsl)
/// Distance-based + height-based atmospheric fog
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct FogPostParams {
    pub fog_color: [f32; 3],     // Stormy purple
    pub density: f32,            // 0.015..0.04
    pub height_fog_start: f32,   // Y level where height fog starts
    pub height_fog_density: f32, // 0.05..0.15
    pub _pad0: [f32; 2],
    // Camera matrices for world pos reconstruction
    pub inv_view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _pad1: f32,
}

impl Default for FogPostParams {
    fn default() -> Self {
        Self {
            fog_color: [0.55, 0.45, 0.70], // Stormy purple
            density: 0.025,
            height_fog_start: 2.0,
            height_fog_density: 0.08,
            _pad0: [0.0; 2],
            inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            _pad1: 0.0,
        }
    }
}

/// ACES Tonemap Params (tonemap_aces.wgsl)
/// Cinematic HDR to LDR conversion
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct TonemapParams {
    pub exposure: f32,   // 1.0 default
    pub gamma: f32,      // 2.2 default (sRGB)
    pub saturation: f32, // 1.0 default
    pub contrast: f32,   // 1.0 default
}

impl Default for TonemapParams {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            gamma: 2.2,
            saturation: 1.0,
            contrast: 1.0,
        }
    }
}
