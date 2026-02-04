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
    pub view_proj: [[f32; 4]; 4],    // 64 bytes (offset 0)
    pub camera_pos: [f32; 3],        // 12 bytes (offset 64)
    pub time: f32,                   // 4 bytes (offset 76) - packs with camera_pos
    pub sun_dir: [f32; 3],           // 12 bytes (offset 80)
    pub fog_density: f32,            // 4 bytes (offset 92) - packs with sun_dir
    pub fog_color: [f32; 3],         // 12 bytes (offset 96)
    pub ambient: f32,                // 4 bytes (offset 108) - packs with fog_color
    // Projectile data (up to 32 projectiles)
    pub projectile_count: u32,       // 4 bytes (offset 112)
    pub _pad_before_padding1: [f32; 3], // 12 bytes (offset 116) - align to 16-byte boundary
    pub _padding1: [f32; 3],         // 12 bytes (offset 128)
    pub _pad_before_array: f32,      // 4 bytes (offset 140) - align array to 16-byte boundary
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
    pub model: [[f32; 4]; 4],           // 64 bytes - model transform
    pub normal_matrix: [[f32; 3]; 3],   // 36 bytes - normal transform (3x3)
    pub _padding: [f32; 3],             // 12 bytes - pad to 112 bytes total
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
    pub view_proj: [[f32; 4]; 4],       // 64 bytes
    pub inv_view_proj: [[f32; 4]; 4],   // 64 bytes
    pub camera_pos: [f32; 3],           // 12 bytes
    pub time: f32,                      // 4 bytes
    pub sun_dir: [f32; 3],              // 12 bytes
    pub fog_density: f32,               // 4 bytes
    pub fog_color: [f32; 3],            // 12 bytes
    pub ambient: f32,                   // 4 bytes
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
    pub world_pos: [f32; 3],            // 12 bytes
    pub _pad0: f32,                     // 4 bytes
    pub barrel_rotation: [f32; 4],      // 16 bytes (quaternion)
    pub color: [f32; 3],                // 12 bytes
    pub _pad1: f32,                     // 4 bytes
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
