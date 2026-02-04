//! Tile Culling Compute Dispatcher (US-0M06)
//!
//! Dispatches tile_culling.wgsl per frame to build per-tile entity lists for
//! screen-space culling. Uses a single compute dispatch for efficiency:
//! tile entity counts are cleared via `encoder.clear_buffer()` before the
//! cull pass runs, avoiding a separate clear compute dispatch.

use wgpu;
use wgpu::util::DeviceExt;

use super::compute_pipelines::ComputePipelines;

/// GPU-side culling uniforms matching `CullingUniforms` in tile_culling.wgsl.
///
/// Layout (80 bytes):
/// - view_proj: mat4x4<f32>  (64 bytes)
/// - resolution: vec2<f32>   (8 bytes)
/// - _padding: vec2<f32>     (8 bytes)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileCullUniforms {
    /// View-projection matrix for projecting world positions to clip space.
    pub view_proj: [f32; 16],
    /// Screen resolution (width, height) in pixels.
    pub resolution: [f32; 2],
    /// Padding for 16-byte alignment.
    pub _padding: [f32; 2],
}

const TILE_CULL_UNIFORMS_SIZE: usize = 80;
const _: () = assert!(std::mem::size_of::<TileCullUniforms>() == TILE_CULL_UNIFORMS_SIZE);

/// Tile size in pixels (must match TILE_SIZE in tile_culling.wgsl and culling.rs).
#[allow(dead_code)]
const TILE_SIZE: u32 = 16;

/// Dispatch tile culling for the current frame.
///
/// Clears the tile buffer and dispatches the tile culling compute shader to
/// project entity bounding spheres to screen space and populate per-tile entity
/// lists. Each tile accumulates up to 32 entity indices via atomics.
///
/// Uses a single compute dispatch (clear is done via `clear_buffer` on the
/// encoder for efficiency).
///
/// Workgroup count: `ceil(tile_count_x / 16) × ceil(tile_count_y / 16)` for
/// the clear pass, and `ceil(entity_count / 256)` for the cull pass.
///
/// # Arguments
/// - `encoder`: Command encoder to record compute passes into
/// - `device`: GPU device for creating temporary buffers
/// - `pipelines`: Compute pipelines (tile_culling_pipeline + layout)
/// - `tile_gpu_buffer`: Storage buffer for tile data (read_write)
/// - `entity_gpu_buffer`: Storage buffer for entity data (read-only)
/// - `view_proj`: 4×4 view-projection matrix (column-major f32 array)
/// - `screen_width`: Screen width in pixels
/// - `screen_height`: Screen height in pixels
/// - `entity_count`: Number of active entities in the entity buffer
pub fn dispatch_tile_culling(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    pipelines: &ComputePipelines,
    tile_gpu_buffer: &wgpu::Buffer,
    entity_gpu_buffer: &wgpu::Buffer,
    view_proj: [f32; 16],
    screen_width: u32,
    screen_height: u32,
    entity_count: u32,
) {
    // Clear the tile buffer (zeros all entity_count atomics and indices).
    // This replaces a separate clear compute pass for single-pass efficiency.
    encoder.clear_buffer(tile_gpu_buffer, 0, None);

    if entity_count == 0 {
        return;
    }

    // Upload culling uniforms
    let uniforms = TileCullUniforms {
        view_proj,
        resolution: [screen_width as f32, screen_height as f32],
        _padding: [0.0; 2],
    };

    let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("tile_cull_uniforms_buffer"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    // Create bind group matching tile_culling.wgsl:
    //   @binding(0): storage<read_write> TileBuffer
    //   @binding(1): storage<read> EntityBuffer
    //   @binding(2): uniform CullingUniforms
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("tile_cull_bind_group"),
        layout: &pipelines.tile_culling_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: tile_gpu_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: entity_gpu_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: uniforms_buffer.as_entire_binding(),
            },
        ],
    });

    // Dispatch cull pass.
    // The shader uses @workgroup_size(16, 16, 1) = 256 threads per workgroup.
    // Entity index = global_id.x + global_id.y * 16, so each workgroup handles
    // 256 entities. We dispatch ceil(entity_count / 256) workgroups in X.
    let entities_per_workgroup: u32 = 16 * 16; // 256
    let cull_wg_x = (entity_count + entities_per_workgroup - 1) / entities_per_workgroup;

    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("tile_cull_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&pipelines.tile_culling_pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(cull_wg_x, 1, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_cull_uniforms_size() {
        assert_eq!(std::mem::size_of::<TileCullUniforms>(), 80);
    }

    #[test]
    fn test_workgroup_calculation_for_tiles() {
        // 1080p: 120×68 tiles → ceil(120/16)=8, ceil(68/16)=5
        let tiles_x = (1920 + TILE_SIZE - 1) / TILE_SIZE;
        let tiles_y = (1080 + TILE_SIZE - 1) / TILE_SIZE;
        assert_eq!(tiles_x, 120);
        assert_eq!(tiles_y, 68);
        let wg_x = (tiles_x + 15) / 16;
        let wg_y = (tiles_y + 15) / 16;
        assert_eq!(wg_x, 8);
        assert_eq!(wg_y, 5);
    }

    #[test]
    fn test_cull_workgroup_calculation() {
        // 1000 entities / 256 = 4 workgroups
        assert_eq!((1000u32 + 255) / 256, 4);
        // 0 entities = 0 workgroups
        assert_eq!((0u32 + 255) / 256, 0);
        // 1 entity = 1 workgroup
        assert_eq!((1u32 + 255) / 256, 1);
        // 256 entities = 1 workgroup
        assert_eq!((256u32 + 255) / 256, 1);
        // 257 entities = 2 workgroups
        assert_eq!((257u32 + 255) / 256, 2);
    }
}
