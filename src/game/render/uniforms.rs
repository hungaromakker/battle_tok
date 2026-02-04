//! GPU Uniform Buffers
//!
//! Data structures for GPU uniform buffers in the arena shader pipeline.

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

/// Main uniforms for the arena shader
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Uniforms {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
    pub time: f32,
    pub resolution: [f32; 2],
    pub _padding: f32,
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0, 0.0],
            time: 0.0,
            resolution: [1920.0, 1080.0],
            _padding: 0.0,
        }
    }
}

// Ensure size is multiple of 16 bytes
const _: () = assert!(std::mem::size_of::<Uniforms>() % 16 == 0);

/// Model uniforms for hex prism rendering
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct HexPrismModelUniforms {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 4],
}

impl Default for HexPrismModelUniforms {
    fn default() -> Self {
        Self {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            color: [1.0, 1.0, 1.0, 1.0],
        }
    }
}

/// SDF cannon uniforms for raymarching
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SdfCannonUniforms {
    pub camera_pos: [f32; 4],
    pub camera_dir: [f32; 4],
    pub camera_up: [f32; 4],
    pub camera_right: [f32; 4],
    pub resolution: [f32; 2],
    pub time: f32,
    pub fov: f32,
}

impl Default for SdfCannonUniforms {
    fn default() -> Self {
        Self {
            camera_pos: [0.0, 5.0, 10.0, 0.0],
            camera_dir: [0.0, 0.0, -1.0, 0.0],
            camera_up: [0.0, 1.0, 0.0, 0.0],
            camera_right: [1.0, 0.0, 0.0, 0.0],
            resolution: [1920.0, 1080.0],
            time: 0.0,
            fov: 60.0_f32.to_radians(),
        }
    }
}

/// SDF cannon data for projectile rendering
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SdfCannonData {
    pub barrel_pos: [f32; 4],
    pub barrel_dir: [f32; 4],
    pub projectile_pos: [f32; 4],
    pub projectile_active: u32,
    pub _padding: [u32; 3],
}

impl Default for SdfCannonData {
    fn default() -> Self {
        Self {
            barrel_pos: [0.0, 2.0, 0.0, 0.0],
            barrel_dir: [0.0, 0.5, -1.0, 0.0],
            projectile_pos: [0.0, 0.0, 0.0, 0.0],
            projectile_active: 0,
            _padding: [0; 3],
        }
    }
}
