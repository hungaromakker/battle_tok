//! Froxel Compute Dispatchers (US-0M04, US-0M05)
//!
//! Dispatches froxel compute passes each frame:
//! - `dispatch_froxel_clear`: Zeros all froxel SDF counts before assignment
//! - `dispatch_froxel_assign`: GPU-based SDF-to-froxel mapping via atomic operations

use wgpu::util::DeviceExt;

use super::compute_pipelines::ComputePipelines;
#[allow(unused_imports)]
use super::froxel_assignment::{AssignmentUniforms, SdfBoundsBuffer};
use super::froxel_config::TOTAL_FROXELS;

/// Workgroup size used in froxel_clear.wgsl (must match shader).
const CLEAR_WORKGROUP_SIZE: u32 = 64;

/// Number of workgroups to dispatch: ceil(TOTAL_FROXELS / CLEAR_WORKGROUP_SIZE).
/// 6144 / 64 = 96 (exact).
const CLEAR_WORKGROUP_COUNT: u32 =
    (TOTAL_FROXELS + CLEAR_WORKGROUP_SIZE - 1) / CLEAR_WORKGROUP_SIZE;

// Compile-time assertion that workgroup count is 96.
const _: () = assert!(CLEAR_WORKGROUP_COUNT == 96);

/// Dispatch the froxel clear compute pass, zeroing all froxel SDF counts.
///
/// This must run BEFORE the froxel assignment pass every frame so that
/// stale SDF lists from the previous frame are cleared.
///
/// # Arguments
/// * `encoder` - Command encoder to record the compute pass into
/// * `device` - GPU device (used to create the bind group)
/// * `pipelines` - Compute pipelines (provides the froxel clear pipeline + layout)
/// * `froxel_sdf_list_buffer` - The GPU buffer holding froxel SDF lists (read_write storage)
pub fn dispatch_froxel_clear(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    pipelines: &ComputePipelines,
    froxel_sdf_list_buffer: &wgpu::Buffer,
) {
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("froxel_clear_bind_group"),
        layout: &pipelines.froxel_clear_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: froxel_sdf_list_buffer.as_entire_binding(),
        }],
    });

    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("froxel_clear_pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&pipelines.froxel_clear_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(CLEAR_WORKGROUP_COUNT, 1, 1);
}

/// Workgroup size used in froxel_assign.wgsl `cs_assign_sdfs` (must match shader).
const ASSIGN_WORKGROUP_SIZE: u32 = 256;

/// Dispatch the froxel assignment compute pass, mapping SDFs to froxels.
///
/// This must run AFTER the froxel clear pass each frame so that the SDF lists
/// start empty before assignment populates them.
///
/// The function uploads the SDF bounds array and assignment uniforms to the GPU,
/// then dispatches the compute shader with `ceil(sdf_count / 256)` workgroups.
/// Each thread processes one SDF and atomically adds its index to all
/// intersecting froxels.
///
/// # Arguments
/// * `encoder` - Command encoder to record the compute pass into
/// * `device` - GPU device (used to create the bind group)
/// * `queue` - GPU queue (used to write buffer data)
/// * `pipelines` - Compute pipelines (provides the froxel assign pipeline + layout)
/// * `sdf_bounds` - SDF bounds data to upload (position + bounding box per entity)
/// * `froxel_bounds_buffer` - The GPU buffer holding precomputed froxel world-space AABBs
/// * `froxel_sdf_list_buffer` - The GPU buffer holding froxel SDF lists (read_write storage)
pub fn dispatch_froxel_assign(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    _queue: &wgpu::Queue,
    pipelines: &ComputePipelines,
    sdf_bounds: &SdfBoundsBuffer,
    froxel_bounds_buffer: &wgpu::Buffer,
    froxel_sdf_list_buffer: &wgpu::Buffer,
) {
    let sdf_count = sdf_bounds
        .count
        .min(super::froxel_assignment::MAX_SDF_COUNT);

    // Early out: nothing to assign if there are no SDFs
    if sdf_count == 0 {
        return;
    }

    // Upload SDF bounds to a temporary GPU buffer
    let sdf_bounds_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("froxel_assign_sdf_bounds"),
        contents: bytemuck::bytes_of(sdf_bounds),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    // Upload assignment uniforms
    let uniforms = AssignmentUniforms::new(sdf_count);
    let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("froxel_assign_uniforms"),
        contents: bytemuck::bytes_of(&uniforms),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    // Create bind group matching the shader layout:
    // binding 0: SDF bounds (read), binding 1: froxel bounds (read),
    // binding 2: froxel SDF lists (read_write), binding 3: uniforms
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("froxel_assign_bind_group"),
        layout: &pipelines.froxel_assign_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: sdf_bounds_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: froxel_bounds_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: froxel_sdf_list_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: uniforms_buffer.as_entire_binding(),
            },
        ],
    });

    // Dispatch: ceil(sdf_count / 256) workgroups
    let workgroup_count = (sdf_count + ASSIGN_WORKGROUP_SIZE - 1) / ASSIGN_WORKGROUP_SIZE;

    let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
        label: Some("froxel_assign_pass"),
        timestamp_writes: None,
    });
    pass.set_pipeline(&pipelines.froxel_assign_pipeline);
    pass.set_bind_group(0, &bind_group, &[]);
    pass.dispatch_workgroups(workgroup_count, 1, 1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workgroup_count() {
        assert_eq!(CLEAR_WORKGROUP_COUNT, 96);
    }

    #[test]
    fn test_covers_all_froxels() {
        assert!(CLEAR_WORKGROUP_COUNT * CLEAR_WORKGROUP_SIZE >= TOTAL_FROXELS);
    }

    #[test]
    fn test_assign_workgroup_size_matches_shader() {
        // froxel_assign.wgsl uses @workgroup_size(256, 1, 1)
        assert_eq!(ASSIGN_WORKGROUP_SIZE, 256);
    }

    #[test]
    fn test_assign_workgroup_count_single() {
        // 1 SDF → 1 workgroup
        let count = (1 + ASSIGN_WORKGROUP_SIZE - 1) / ASSIGN_WORKGROUP_SIZE;
        assert_eq!(count, 1);
    }

    #[test]
    fn test_assign_workgroup_count_exact() {
        // 256 SDFs → 1 workgroup
        let count = (256 + ASSIGN_WORKGROUP_SIZE - 1) / ASSIGN_WORKGROUP_SIZE;
        assert_eq!(count, 1);
    }

    #[test]
    fn test_assign_workgroup_count_overflow() {
        // 257 SDFs → 2 workgroups
        let count = (257 + ASSIGN_WORKGROUP_SIZE - 1) / ASSIGN_WORKGROUP_SIZE;
        assert_eq!(count, 2);
    }

    #[test]
    fn test_assign_workgroup_count_max() {
        // 1024 SDFs → 4 workgroups
        let count = (1024 + ASSIGN_WORKGROUP_SIZE - 1) / ASSIGN_WORKGROUP_SIZE;
        assert_eq!(count, 4);
    }
}
