//! Froxel Assignment Shader Bindings (US-033)
//!
//! This module defines the bind group layout and supporting structures for the
//! SDF-to-froxel assignment compute shader. The assignment shader determines
//! which SDFs potentially intersect each froxel, enabling efficient spatial
//! culling during raymarching.
//!
//! ## Bind Group Layout
//!
//! The assignment compute shader uses the following bindings:
//!
//! | Binding | Type | Access | Description |
//! |---------|------|--------|-------------|
//! | 0 | Storage Buffer | Read-only | SDF bounds (AABB per creature) |
//! | 1 | Storage Buffer | Read-only | Froxel bounds (precomputed world-space AABBs) |
//! | 2 | Storage Buffer | Read-write | Froxel SDF lists (output: which SDFs per froxel) |
//! | 3 | Uniform Buffer | Read-only | Assignment uniforms (creature_count, etc.) |
//!
//! ## Memory Layout
//!
//! - SDF Bounds: 32 bytes per creature × 1024 max creatures = 32,768 bytes (~32 KB)
//! - Froxel Bounds: Already defined in froxel_buffers.rs (~192 KB)
//! - Froxel SDF Lists: Already defined in froxel_buffers.rs (~1.6 MB)
//! - Uniforms: 16 bytes

use super::froxel_buffers::{FroxelBoundsBuffer, FroxelSDFListBuffer};

// ============================================================================
// SDF Bounds - World-space AABB for each creature/SDF
// ============================================================================

/// Maximum number of SDFs (creatures/entities) that can be processed.
pub const MAX_SDF_COUNT: u32 = 1024;

/// World-space axis-aligned bounding box (AABB) for a single SDF/creature.
///
/// This bounds is used to determine which froxels an SDF potentially intersects.
/// The bounds should encompass the entire SDF, including any animations or
/// deformations that might occur.
///
/// WGSL Layout (32 bytes, 2 rows of 16 bytes):
///   Row 0 (offset 0-15):  min_x, min_y, min_z, _pad0
///   Row 1 (offset 16-31): max_x, max_y, max_z, _pad1
///
/// Byte offset summary:
///   offset  0: min_x (f32) - Minimum X coordinate in world space
///   offset  4: min_y (f32) - Minimum Y coordinate in world space
///   offset  8: min_z (f32) - Minimum Z coordinate in world space
///   offset 12: _pad0 (u32) - Padding for 16-byte alignment
///   offset 16: max_x (f32) - Maximum X coordinate in world space
///   offset 20: max_y (f32) - Maximum Y coordinate in world space
///   offset 24: max_z (f32) - Maximum Z coordinate in world space
///   offset 28: _pad1 (u32) - Padding for 16-byte alignment
///   TOTAL: 32 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SdfBounds {
    /// Minimum X coordinate in world space (meters)
    pub min_x: f32,
    /// Minimum Y coordinate in world space (meters)
    pub min_y: f32,
    /// Minimum Z coordinate in world space (meters)
    pub min_z: f32,
    /// Padding for 16-byte row alignment
    pub _pad0: u32,
    /// Maximum X coordinate in world space (meters)
    pub max_x: f32,
    /// Maximum Y coordinate in world space (meters)
    pub max_y: f32,
    /// Maximum Z coordinate in world space (meters)
    pub max_z: f32,
    /// Padding for 16-byte row alignment
    pub _pad1: u32,
}

impl Default for SdfBounds {
    fn default() -> Self {
        Self {
            min_x: 0.0,
            min_y: 0.0,
            min_z: 0.0,
            _pad0: 0,
            max_x: 0.0,
            max_y: 0.0,
            max_z: 0.0,
            _pad1: 0,
        }
    }
}

impl SdfBounds {
    /// Create new SDF bounds from min/max positions.
    pub fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self {
            min_x: min[0],
            min_y: min[1],
            min_z: min[2],
            _pad0: 0,
            max_x: max[0],
            max_y: max[1],
            max_z: max[2],
            _pad1: 0,
        }
    }

    /// Create bounds from center position and half-extents.
    pub fn from_center_extents(center: [f32; 3], half_extents: [f32; 3]) -> Self {
        Self::new(
            [
                center[0] - half_extents[0],
                center[1] - half_extents[1],
                center[2] - half_extents[2],
            ],
            [
                center[0] + half_extents[0],
                center[1] + half_extents[1],
                center[2] + half_extents[2],
            ],
        )
    }

    /// Get the minimum corner as an array.
    #[inline]
    pub fn min(&self) -> [f32; 3] {
        [self.min_x, self.min_y, self.min_z]
    }

    /// Get the maximum corner as an array.
    #[inline]
    pub fn max(&self) -> [f32; 3] {
        [self.max_x, self.max_y, self.max_z]
    }

    /// Get the center of the bounds.
    #[inline]
    pub fn center(&self) -> [f32; 3] {
        [
            (self.min_x + self.max_x) * 0.5,
            (self.min_y + self.max_y) * 0.5,
            (self.min_z + self.max_z) * 0.5,
        ]
    }

    /// Get the size (extents) of the bounds.
    #[inline]
    pub fn size(&self) -> [f32; 3] {
        [
            self.max_x - self.min_x,
            self.max_y - self.min_y,
            self.max_z - self.min_z,
        ]
    }
}

// ============================================================================
// SDF Bounds Buffer - Contains bounds for all SDFs
// ============================================================================

/// Buffer containing bounds for all SDFs/creatures.
///
/// Memory: 32 bytes × 1024 SDFs = 32,768 bytes (~32 KB)
///
/// Buffer layout:
/// - count: 4 bytes (u32) - Number of valid SDF bounds
/// - padding: 12 bytes (3 × u32 for 16-byte alignment)
/// - bounds: 1024 × 32 bytes = 32,768 bytes
/// Total: 16 + 32,768 = 32,784 bytes
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SdfBoundsBuffer {
    /// Number of valid SDF bounds (0 to MAX_SDF_COUNT)
    pub count: u32,
    /// Padding for 16-byte alignment
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    /// Bounds for each SDF, indexed by SDF/creature ID
    pub bounds: [SdfBounds; MAX_SDF_COUNT as usize],
}

impl Default for SdfBoundsBuffer {
    fn default() -> Self {
        Self {
            count: 0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            bounds: [SdfBounds::default(); MAX_SDF_COUNT as usize],
        }
    }
}

impl SdfBoundsBuffer {
    /// Create a new empty SDF bounds buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the bounds for an SDF at the given index.
    pub fn set(&mut self, index: u32, bounds: SdfBounds) {
        if index < MAX_SDF_COUNT {
            self.bounds[index as usize] = bounds;
        }
    }

    /// Get the bounds for an SDF at the given index.
    pub fn get(&self, index: u32) -> Option<&SdfBounds> {
        if index < self.count && index < MAX_SDF_COUNT {
            Some(&self.bounds[index as usize])
        } else {
            None
        }
    }

    /// Clear all bounds and reset count to 0.
    pub fn clear(&mut self) {
        self.count = 0;
    }
}

// ============================================================================
// Assignment Uniforms - Parameters for the assignment compute shader
// ============================================================================

/// Uniforms for the froxel assignment compute shader.
///
/// WGSL Layout (16 bytes):
///   offset  0: creature_count (u32) - Number of creatures/SDFs to process
///   offset  4: froxel_count (u32) - Number of froxels (always TOTAL_FROXELS)
///   offset  8: _pad0 (u32) - Reserved for future use
///   offset 12: _pad1 (u32) - Reserved for future use
///   TOTAL: 16 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AssignmentUniforms {
    /// Number of creatures/SDFs to process (0 to MAX_SDF_COUNT)
    pub creature_count: u32,
    /// Number of froxels in the grid (always TOTAL_FROXELS = 6144)
    pub froxel_count: u32,
    /// Reserved for future use (e.g., assignment flags)
    pub _pad0: u32,
    /// Reserved for future use
    pub _pad1: u32,
}

impl Default for AssignmentUniforms {
    fn default() -> Self {
        use super::froxel_config::TOTAL_FROXELS;
        Self {
            creature_count: 0,
            froxel_count: TOTAL_FROXELS,
            _pad0: 0,
            _pad1: 0,
        }
    }
}

impl AssignmentUniforms {
    /// Create new assignment uniforms with the given creature count.
    pub fn new(creature_count: u32) -> Self {
        use super::froxel_config::TOTAL_FROXELS;
        Self {
            creature_count,
            froxel_count: TOTAL_FROXELS,
            _pad0: 0,
            _pad1: 0,
        }
    }
}

// ============================================================================
// Buffer Size Constants
// ============================================================================

/// Size of SdfBounds struct in bytes (32 bytes)
pub const SDF_BOUNDS_SIZE: usize = std::mem::size_of::<SdfBounds>();

/// Total size of SdfBoundsBuffer in bytes (~32 KB)
pub const SDF_BOUNDS_BUFFER_SIZE: usize = std::mem::size_of::<SdfBoundsBuffer>();

/// Size of AssignmentUniforms struct in bytes (16 bytes)
pub const ASSIGNMENT_UNIFORMS_SIZE: usize = std::mem::size_of::<AssignmentUniforms>();

// ============================================================================
// Bind Group Layout and Creation
// ============================================================================

/// Create the bind group layout for the froxel assignment compute shader.
///
/// Layout:
/// - Binding 0: Read-only storage buffer (SDF bounds)
/// - Binding 1: Read-only storage buffer (Froxel bounds)
/// - Binding 2: Read-write storage buffer (Froxel SDF lists)
/// - Binding 3: Uniform buffer (Assignment uniforms)
pub fn create_assignment_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Froxel Assignment Bind Group Layout"),
        entries: &[
            // Binding 0: SDF bounds buffer (read-only)
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Binding 1: Froxel bounds buffer (read-only)
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Binding 2: Froxel SDF lists buffer (read-write)
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Binding 3: Assignment uniforms (read-only)
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    })
}

/// Create GPU buffers for the froxel assignment system.
///
/// Returns a tuple of (sdf_bounds_buffer, froxel_bounds_buffer, froxel_sdf_lists_buffer, uniforms_buffer).
pub fn create_assignment_buffers(
    device: &wgpu::Device,
) -> (wgpu::Buffer, wgpu::Buffer, wgpu::Buffer, wgpu::Buffer) {
    use wgpu::util::DeviceExt;

    // SDF bounds buffer (read-only storage, written from CPU)
    let sdf_bounds_data = SdfBoundsBuffer::default();
    let sdf_bounds_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("SDF Bounds Buffer"),
        contents: bytemuck::bytes_of(&sdf_bounds_data),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    // Froxel bounds buffer (read-only storage, written from CPU)
    let froxel_bounds_data = Box::new(FroxelBoundsBuffer::default());
    let froxel_bounds_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Froxel Bounds Buffer (Assignment)"),
        contents: bytemuck::bytes_of(froxel_bounds_data.as_ref()),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    // Froxel SDF lists buffer (read-write storage, written by compute shader)
    let froxel_sdf_lists_data = Box::new(FroxelSDFListBuffer::default());
    let froxel_sdf_lists_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Froxel SDF Lists Buffer"),
        contents: bytemuck::bytes_of(froxel_sdf_lists_data.as_ref()),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    // Assignment uniforms buffer
    let uniforms_data = AssignmentUniforms::default();
    let uniforms_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Assignment Uniforms Buffer"),
        contents: bytemuck::bytes_of(&uniforms_data),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    (
        sdf_bounds_buffer,
        froxel_bounds_buffer,
        froxel_sdf_lists_buffer,
        uniforms_buffer,
    )
}

/// Create the bind group for the froxel assignment compute shader.
///
/// The bind group connects the buffers to the shader bindings.
pub fn create_assignment_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sdf_bounds_buffer: &wgpu::Buffer,
    froxel_bounds_buffer: &wgpu::Buffer,
    froxel_sdf_lists_buffer: &wgpu::Buffer,
    uniforms_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Froxel Assignment Bind Group"),
        layout,
        entries: &[
            // Binding 0: SDF bounds buffer
            wgpu::BindGroupEntry {
                binding: 0,
                resource: sdf_bounds_buffer.as_entire_binding(),
            },
            // Binding 1: Froxel bounds buffer
            wgpu::BindGroupEntry {
                binding: 1,
                resource: froxel_bounds_buffer.as_entire_binding(),
            },
            // Binding 2: Froxel SDF lists buffer
            wgpu::BindGroupEntry {
                binding: 2,
                resource: froxel_sdf_lists_buffer.as_entire_binding(),
            },
            // Binding 3: Assignment uniforms
            wgpu::BindGroupEntry {
                binding: 3,
                resource: uniforms_buffer.as_entire_binding(),
            },
        ],
    })
}

/// Write updated SDF bounds to the GPU buffer.
pub fn write_sdf_bounds(queue: &wgpu::Queue, buffer: &wgpu::Buffer, data: &SdfBoundsBuffer) {
    queue.write_buffer(buffer, 0, bytemuck::bytes_of(data));
}

/// Write updated assignment uniforms to the GPU buffer.
pub fn write_assignment_uniforms(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    data: &AssignmentUniforms,
) {
    queue.write_buffer(buffer, 0, bytemuck::bytes_of(data));
}

// ============================================================================
// Compile-time Size Assertions
// ============================================================================

const _: () = {
    // SdfBounds: 32 bytes (2 × 16-byte rows)
    assert!(
        std::mem::size_of::<SdfBounds>() == 32,
        "SdfBounds must be 32 bytes for GPU alignment"
    );

    // SdfBoundsBuffer: 16 (header) + 32 × 1024 = 32,784 bytes
    assert!(
        std::mem::size_of::<SdfBoundsBuffer>() == 16 + 32 * 1024,
        "SdfBoundsBuffer size mismatch"
    );

    // AssignmentUniforms: 16 bytes
    assert!(
        std::mem::size_of::<AssignmentUniforms>() == 16,
        "AssignmentUniforms must be 16 bytes"
    );
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdf_bounds_size() {
        assert_eq!(std::mem::size_of::<SdfBounds>(), 32);
    }

    #[test]
    fn test_sdf_bounds_buffer_size() {
        assert_eq!(
            std::mem::size_of::<SdfBoundsBuffer>(),
            16 + 32 * MAX_SDF_COUNT as usize
        );
    }

    #[test]
    fn test_assignment_uniforms_size() {
        assert_eq!(std::mem::size_of::<AssignmentUniforms>(), 16);
    }

    #[test]
    fn test_sdf_bounds_new() {
        let bounds = SdfBounds::new([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);
        assert_eq!(bounds.min(), [1.0, 2.0, 3.0]);
        assert_eq!(bounds.max(), [4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_sdf_bounds_from_center_extents() {
        let bounds = SdfBounds::from_center_extents([5.0, 5.0, 5.0], [2.0, 3.0, 4.0]);
        assert_eq!(bounds.min(), [3.0, 2.0, 1.0]);
        assert_eq!(bounds.max(), [7.0, 8.0, 9.0]);
    }

    #[test]
    fn test_sdf_bounds_center() {
        let bounds = SdfBounds::new([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);
        assert_eq!(bounds.center(), [5.0, 10.0, 15.0]);
    }

    #[test]
    fn test_sdf_bounds_size_method() {
        let bounds = SdfBounds::new([1.0, 2.0, 3.0], [5.0, 7.0, 10.0]);
        assert_eq!(bounds.size(), [4.0, 5.0, 7.0]);
    }

    #[test]
    fn test_sdf_bounds_buffer_set_get() {
        let mut buffer = SdfBoundsBuffer::new();
        buffer.count = 5;

        let bounds = SdfBounds::new([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);
        buffer.set(3, bounds);

        let retrieved = buffer.get(3).unwrap();
        assert_eq!(retrieved.min(), [1.0, 2.0, 3.0]);
        assert_eq!(retrieved.max(), [4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_sdf_bounds_buffer_get_out_of_range() {
        let buffer = SdfBoundsBuffer::new();
        // count is 0, so any index should return None
        assert!(buffer.get(0).is_none());
        assert!(buffer.get(100).is_none());
    }

    #[test]
    fn test_assignment_uniforms_new() {
        let uniforms = AssignmentUniforms::new(42);
        assert_eq!(uniforms.creature_count, 42);
        assert_eq!(
            uniforms.froxel_count,
            super::super::froxel_config::TOTAL_FROXELS
        );
    }

    #[test]
    fn test_assignment_uniforms_default() {
        let uniforms = AssignmentUniforms::default();
        assert_eq!(uniforms.creature_count, 0);
        assert_eq!(
            uniforms.froxel_count,
            super::super::froxel_config::TOTAL_FROXELS
        );
    }

    #[test]
    fn test_buffer_size_constants() {
        assert_eq!(SDF_BOUNDS_SIZE, 32);
        assert_eq!(SDF_BOUNDS_BUFFER_SIZE, 16 + 32 * 1024);
        assert_eq!(ASSIGNMENT_UNIFORMS_SIZE, 16);
    }
}
