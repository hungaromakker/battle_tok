//! Froxel GPU Buffer Structures (US-029)
//!
//! This module defines Rust structs and GPU buffer layouts for the froxel (frustum + voxel)
//! culling system. Froxels are 3D cells that subdivide the view frustum for efficient
//! spatial queries during raymarching.
//!
//! ## Buffer Structure
//!
//! Two main buffers are used:
//! 1. **FroxelBoundsBuffer**: Stores world-space AABB bounds for each froxel (6,144 froxels)
//! 2. **FroxelSDFListBuffer**: Stores per-froxel lists of SDF indices (max 64 SDFs per froxel)
//!
//! ## Memory Layout
//!
//! FroxelBounds: 32 bytes each × 6,144 froxels = 196,608 bytes (~192 KB)
//! FroxelSDFList: 260 bytes each × 6,144 froxels = 1,597,440 bytes (~1.5 MB)
//! Total GPU memory: ~1.7 MB for froxel culling

use super::froxel_config::{MAX_SDFS_PER_FROXEL, TOTAL_FROXELS};

// ============================================================================
// FroxelBounds - World-space AABB for each froxel
// ============================================================================

/// World-space axis-aligned bounding box (AABB) for a single froxel.
///
/// Each froxel's bounds are computed from the view frustum and stored here
/// for intersection tests during culling.
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
pub struct FroxelBounds {
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

impl Default for FroxelBounds {
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

impl FroxelBounds {
    /// Create new froxel bounds from min/max positions.
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

    /// Get the center of the froxel bounds.
    #[inline]
    pub fn center(&self) -> [f32; 3] {
        [
            (self.min_x + self.max_x) * 0.5,
            (self.min_y + self.max_y) * 0.5,
            (self.min_z + self.max_z) * 0.5,
        ]
    }

    /// Get the size (extents) of the froxel bounds.
    #[inline]
    pub fn size(&self) -> [f32; 3] {
        [
            self.max_x - self.min_x,
            self.max_y - self.min_y,
            self.max_z - self.min_z,
        ]
    }

    /// Check if a point is inside the froxel bounds.
    #[inline]
    pub fn contains_point(&self, point: [f32; 3]) -> bool {
        point[0] >= self.min_x
            && point[0] <= self.max_x
            && point[1] >= self.min_y
            && point[1] <= self.max_y
            && point[2] >= self.min_z
            && point[2] <= self.max_z
    }

    /// Check if this froxel bounds intersects with another AABB.
    #[inline]
    pub fn intersects_aabb(&self, other_min: [f32; 3], other_max: [f32; 3]) -> bool {
        self.min_x <= other_max[0]
            && self.max_x >= other_min[0]
            && self.min_y <= other_max[1]
            && self.max_y >= other_min[1]
            && self.min_z <= other_max[2]
            && self.max_z >= other_min[2]
    }
}

// ============================================================================
// FroxelSDFList - Per-froxel list of SDF indices
// ============================================================================

/// List of SDF indices assigned to a single froxel.
///
/// During culling, SDFs that potentially intersect a froxel are added to that
/// froxel's list. During raymarching, only SDFs in the current froxel are evaluated.
///
/// WGSL Layout (260 bytes):
///   offset 0:   count (u32) - Number of valid SDF indices in the array
///   offset 4:   sdf_indices[64] (array<u32, 64>) - SDF indices (256 bytes)
///   TOTAL: 260 bytes
///
/// Note: We add 3 padding u32s after count to ensure the array starts at a
/// 16-byte boundary, making the total 272 bytes for proper alignment.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FroxelSDFList {
    /// Number of valid SDF indices in this froxel (0 to MAX_SDFS_PER_FROXEL)
    pub count: u32,
    /// Padding for 16-byte alignment before the array
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    /// Array of SDF indices that intersect this froxel
    /// Only the first `count` entries are valid
    pub sdf_indices: [u32; 64],
}

impl Default for FroxelSDFList {
    fn default() -> Self {
        Self {
            count: 0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            sdf_indices: [0u32; 64],
        }
    }
}

impl FroxelSDFList {
    /// Create an empty SDF list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an SDF index to the list.
    /// Returns true if added, false if the list is full.
    pub fn add(&mut self, sdf_index: u32) -> bool {
        if self.count >= MAX_SDFS_PER_FROXEL {
            return false;
        }
        self.sdf_indices[self.count as usize] = sdf_index;
        self.count += 1;
        true
    }

    /// Clear all SDF indices from the list.
    pub fn clear(&mut self) {
        self.count = 0;
        // Note: We don't need to zero the array - count tracks valid entries
    }

    /// Check if the list is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Check if the list is full.
    #[inline]
    pub fn is_full(&self) -> bool {
        self.count >= MAX_SDFS_PER_FROXEL
    }

    /// Get the number of SDFs in this froxel.
    #[inline]
    pub fn len(&self) -> u32 {
        self.count
    }

    /// Iterate over valid SDF indices.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.sdf_indices[..self.count as usize].iter().copied()
    }
}

// ============================================================================
// GPU Buffer Structures
// ============================================================================

/// Buffer containing bounds for all froxels.
///
/// Memory: 32 bytes × 6,144 froxels = 196,608 bytes (~192 KB)
///
/// Buffer layout:
/// - count: 4 bytes (u32) - Always equals TOTAL_FROXELS
/// - padding: 12 bytes (3 × u32 for 16-byte alignment)
/// - bounds: 6,144 × 32 bytes = 196,608 bytes
/// Total: 16 + 196,608 = 196,624 bytes
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FroxelBoundsBuffer {
    /// Number of froxels (always TOTAL_FROXELS = 6144)
    pub count: u32,
    /// Padding for 16-byte alignment
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    /// Bounds for each froxel, indexed by:
    /// index = z * (FROXEL_TILES_X * FROXEL_TILES_Y) + y * FROXEL_TILES_X + x
    pub bounds: [FroxelBounds; TOTAL_FROXELS as usize],
}

impl Default for FroxelBoundsBuffer {
    fn default() -> Self {
        Self {
            count: TOTAL_FROXELS,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            bounds: [FroxelBounds::default(); TOTAL_FROXELS as usize],
        }
    }
}

impl std::fmt::Debug for FroxelBoundsBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FroxelBoundsBuffer")
            .field("count", &self.count)
            .field("bounds", &format_args!("[FroxelBounds; {}]", self.bounds.len()))
            .finish()
    }
}

impl FroxelBoundsBuffer {
    /// Create a new froxel bounds buffer with default (zeroed) bounds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate the linear index for a froxel at (x, y, z) grid coordinates.
    ///
    /// Grid layout: X varies fastest, then Y, then Z (row-major order)
    /// index = z * (TILES_X * TILES_Y) + y * TILES_X + x
    #[inline]
    pub fn froxel_index(x: u32, y: u32, z: u32) -> usize {
        use super::froxel_config::{FROXEL_TILES_X, FROXEL_TILES_Y};
        (z * FROXEL_TILES_X * FROXEL_TILES_Y + y * FROXEL_TILES_X + x) as usize
    }

    /// Get the bounds for a froxel at (x, y, z) grid coordinates.
    #[inline]
    pub fn get(&self, x: u32, y: u32, z: u32) -> &FroxelBounds {
        &self.bounds[Self::froxel_index(x, y, z)]
    }

    /// Get mutable bounds for a froxel at (x, y, z) grid coordinates.
    #[inline]
    pub fn get_mut(&mut self, x: u32, y: u32, z: u32) -> &mut FroxelBounds {
        let idx = Self::froxel_index(x, y, z);
        &mut self.bounds[idx]
    }

    /// Set the bounds for a froxel at (x, y, z) grid coordinates.
    #[inline]
    pub fn set(&mut self, x: u32, y: u32, z: u32, bounds: FroxelBounds) {
        let idx = Self::froxel_index(x, y, z);
        self.bounds[idx] = bounds;
    }
}

/// Buffer containing SDF lists for all froxels.
///
/// Memory: 272 bytes × 6,144 froxels = 1,671,168 bytes (~1.6 MB)
///
/// Buffer layout:
/// - count: 4 bytes (u32) - Always equals TOTAL_FROXELS
/// - padding: 12 bytes (3 × u32 for 16-byte alignment)
/// - lists: 6,144 × 272 bytes = 1,671,168 bytes
/// Total: 16 + 1,671,168 = 1,671,184 bytes
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FroxelSDFListBuffer {
    /// Number of froxels (always TOTAL_FROXELS = 6144)
    pub count: u32,
    /// Padding for 16-byte alignment
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    /// SDF list for each froxel, indexed same as FroxelBoundsBuffer
    pub lists: [FroxelSDFList; TOTAL_FROXELS as usize],
}

impl Default for FroxelSDFListBuffer {
    fn default() -> Self {
        Self {
            count: TOTAL_FROXELS,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            lists: [FroxelSDFList::default(); TOTAL_FROXELS as usize],
        }
    }
}

impl FroxelSDFListBuffer {
    /// Create a new froxel SDF list buffer with empty lists.
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate the linear index for a froxel at (x, y, z) grid coordinates.
    #[inline]
    pub fn froxel_index(x: u32, y: u32, z: u32) -> usize {
        FroxelBoundsBuffer::froxel_index(x, y, z)
    }

    /// Get the SDF list for a froxel at (x, y, z) grid coordinates.
    #[inline]
    pub fn get(&self, x: u32, y: u32, z: u32) -> &FroxelSDFList {
        &self.lists[Self::froxel_index(x, y, z)]
    }

    /// Get mutable SDF list for a froxel at (x, y, z) grid coordinates.
    #[inline]
    pub fn get_mut(&mut self, x: u32, y: u32, z: u32) -> &mut FroxelSDFList {
        let idx = Self::froxel_index(x, y, z);
        &mut self.lists[idx]
    }

    /// Clear all SDF lists (reset counts to 0).
    pub fn clear_all(&mut self) {
        for list in self.lists.iter_mut() {
            list.clear();
        }
    }

    /// Add an SDF index to the froxel at (x, y, z).
    /// Returns true if added, false if that froxel's list is full.
    pub fn add_sdf(&mut self, x: u32, y: u32, z: u32, sdf_index: u32) -> bool {
        self.get_mut(x, y, z).add(sdf_index)
    }
}

// ============================================================================
// Buffer Size Constants
// ============================================================================

/// Size of FroxelBounds struct in bytes (32 bytes)
pub const FROXEL_BOUNDS_SIZE: usize = std::mem::size_of::<FroxelBounds>();

/// Size of FroxelSDFList struct in bytes (272 bytes)
pub const FROXEL_SDF_LIST_SIZE: usize = std::mem::size_of::<FroxelSDFList>();

/// Total size of FroxelBoundsBuffer in bytes (~192 KB)
pub const FROXEL_BOUNDS_BUFFER_SIZE: usize = std::mem::size_of::<FroxelBoundsBuffer>();

/// Total size of FroxelSDFListBuffer in bytes (~1.6 MB)
pub const FROXEL_SDF_LIST_BUFFER_SIZE: usize = std::mem::size_of::<FroxelSDFListBuffer>();

// ============================================================================
// wgpu Buffer Creation Functions
// ============================================================================

/// Create a wgpu buffer for froxel bounds.
///
/// Buffer usage: STORAGE | COPY_DST (writable from CPU, readable in shader)
pub fn create_froxel_bounds_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    // Heap-allocate to avoid ~192 KB stack usage
    let buffer_data = Box::new(FroxelBoundsBuffer::default());

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Froxel Bounds Buffer"),
        contents: bytemuck::bytes_of(buffer_data.as_ref()),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

/// Create a wgpu buffer for froxel SDF lists.
///
/// Buffer usage: STORAGE | COPY_DST (writable from CPU, readable in shader)
pub fn create_froxel_sdf_list_buffer(device: &wgpu::Device) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    // Heap-allocate to avoid ~1.6 MB stack usage
    let buffer_data = Box::new(FroxelSDFListBuffer::default());

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Froxel SDF List Buffer"),
        contents: bytemuck::bytes_of(buffer_data.as_ref()),
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    })
}

/// Write updated froxel bounds to the GPU buffer.
pub fn write_froxel_bounds(queue: &wgpu::Queue, buffer: &wgpu::Buffer, data: &FroxelBoundsBuffer) {
    queue.write_buffer(buffer, 0, bytemuck::bytes_of(data));
}

/// Write updated froxel SDF lists to the GPU buffer.
pub fn write_froxel_sdf_lists(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    data: &FroxelSDFListBuffer,
) {
    queue.write_buffer(buffer, 0, bytemuck::bytes_of(data));
}

// ============================================================================
// Compile-time Size Assertions
// ============================================================================

const _: () = {
    // FroxelBounds: 32 bytes (2 × 16-byte rows)
    assert!(
        std::mem::size_of::<FroxelBounds>() == 32,
        "FroxelBounds must be 32 bytes for GPU alignment"
    );

    // FroxelSDFList: 16 (header + padding) + 256 (64 × u32) = 272 bytes
    assert!(
        std::mem::size_of::<FroxelSDFList>() == 272,
        "FroxelSDFList must be 272 bytes"
    );

    // FroxelBoundsBuffer: 16 (header) + 32 × 6144 = 196,624 bytes
    assert!(
        std::mem::size_of::<FroxelBoundsBuffer>() == 16 + 32 * 6144,
        "FroxelBoundsBuffer size mismatch"
    );

    // FroxelSDFListBuffer: 16 (header) + 272 × 6144 = 1,671,184 bytes
    assert!(
        std::mem::size_of::<FroxelSDFListBuffer>() == 16 + 272 * 6144,
        "FroxelSDFListBuffer size mismatch"
    );
};

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::froxel_config::*;

    #[test]
    fn test_froxel_bounds_size() {
        assert_eq!(std::mem::size_of::<FroxelBounds>(), 32);
    }

    #[test]
    fn test_froxel_sdf_list_size() {
        assert_eq!(std::mem::size_of::<FroxelSDFList>(), 272);
    }

    #[test]
    fn test_froxel_bounds_buffer_size() {
        assert_eq!(
            std::mem::size_of::<FroxelBoundsBuffer>(),
            16 + 32 * TOTAL_FROXELS as usize
        );
    }

    #[test]
    fn test_froxel_sdf_list_buffer_size() {
        assert_eq!(
            std::mem::size_of::<FroxelSDFListBuffer>(),
            16 + 272 * TOTAL_FROXELS as usize
        );
    }

    #[test]
    fn test_froxel_bounds_new() {
        let bounds = FroxelBounds::new([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);
        assert_eq!(bounds.min(), [1.0, 2.0, 3.0]);
        assert_eq!(bounds.max(), [4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_froxel_bounds_center() {
        let bounds = FroxelBounds::new([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]);
        assert_eq!(bounds.center(), [5.0, 10.0, 15.0]);
    }

    #[test]
    fn test_froxel_bounds_extents() {
        let bounds = FroxelBounds::new([1.0, 2.0, 3.0], [5.0, 7.0, 10.0]);
        assert_eq!(bounds.size(), [4.0, 5.0, 7.0]);
    }

    #[test]
    fn test_froxel_bounds_contains_point() {
        let bounds = FroxelBounds::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);

        assert!(bounds.contains_point([5.0, 5.0, 5.0]));
        assert!(bounds.contains_point([0.0, 0.0, 0.0]));
        assert!(bounds.contains_point([10.0, 10.0, 10.0]));
        assert!(!bounds.contains_point([11.0, 5.0, 5.0]));
        assert!(!bounds.contains_point([-1.0, 5.0, 5.0]));
    }

    #[test]
    fn test_froxel_bounds_intersects_aabb() {
        let bounds = FroxelBounds::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);

        // Overlapping
        assert!(bounds.intersects_aabb([5.0, 5.0, 5.0], [15.0, 15.0, 15.0]));
        // Touching
        assert!(bounds.intersects_aabb([10.0, 0.0, 0.0], [20.0, 10.0, 10.0]));
        // Inside
        assert!(bounds.intersects_aabb([2.0, 2.0, 2.0], [8.0, 8.0, 8.0]));
        // Outside
        assert!(!bounds.intersects_aabb([20.0, 20.0, 20.0], [30.0, 30.0, 30.0]));
    }

    #[test]
    fn test_froxel_sdf_list_add() {
        let mut list = FroxelSDFList::new();

        assert!(list.is_empty());
        assert!(!list.is_full());
        assert_eq!(list.len(), 0);

        // Add some indices
        assert!(list.add(5));
        assert!(list.add(10));
        assert!(list.add(15));

        assert!(!list.is_empty());
        assert_eq!(list.len(), 3);

        // Verify indices via iterator
        let indices: Vec<u32> = list.iter().collect();
        assert_eq!(indices, vec![5, 10, 15]);
    }

    #[test]
    fn test_froxel_sdf_list_full() {
        let mut list = FroxelSDFList::new();

        // Fill the list
        for i in 0..MAX_SDFS_PER_FROXEL {
            assert!(list.add(i), "Should add index {}", i);
        }

        assert!(list.is_full());
        assert_eq!(list.len(), MAX_SDFS_PER_FROXEL);

        // Should fail to add more
        assert!(!list.add(999));
        assert_eq!(list.len(), MAX_SDFS_PER_FROXEL);
    }

    #[test]
    fn test_froxel_sdf_list_clear() {
        let mut list = FroxelSDFList::new();

        list.add(1);
        list.add(2);
        list.add(3);
        assert_eq!(list.len(), 3);

        list.clear();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn test_froxel_index_calculation() {
        // Test linear index calculation
        // index = z * (TILES_X * TILES_Y) + y * TILES_X + x
        // With 16×16×24: tiles_per_slice = 256

        assert_eq!(FroxelBoundsBuffer::froxel_index(0, 0, 0), 0);
        assert_eq!(FroxelBoundsBuffer::froxel_index(1, 0, 0), 1);
        assert_eq!(FroxelBoundsBuffer::froxel_index(15, 0, 0), 15);
        assert_eq!(FroxelBoundsBuffer::froxel_index(0, 1, 0), 16); // TILES_X
        assert_eq!(FroxelBoundsBuffer::froxel_index(0, 0, 1), 256); // TILES_X * TILES_Y
        assert_eq!(
            FroxelBoundsBuffer::froxel_index(15, 15, 23),
            (TOTAL_FROXELS - 1) as usize
        );
    }

    #[test]
    fn test_froxel_bounds_buffer_access() {
        let mut buffer = FroxelBoundsBuffer::new();

        // Set bounds at a specific position
        let bounds = FroxelBounds::new([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]);
        buffer.set(5, 7, 12, bounds);

        // Read back
        let read_bounds = buffer.get(5, 7, 12);
        assert_eq!(read_bounds.min(), [1.0, 2.0, 3.0]);
        assert_eq!(read_bounds.max(), [4.0, 5.0, 6.0]);
    }

    #[test]
    fn test_froxel_sdf_list_buffer_access() {
        let mut buffer = FroxelSDFListBuffer::new();

        // Add SDFs to a specific froxel
        assert!(buffer.add_sdf(3, 4, 5, 42));
        assert!(buffer.add_sdf(3, 4, 5, 100));

        // Read back
        let list = buffer.get(3, 4, 5);
        assert_eq!(list.len(), 2);
        let indices: Vec<u32> = list.iter().collect();
        assert_eq!(indices, vec![42, 100]);

        // Other froxels should be empty
        assert!(buffer.get(0, 0, 0).is_empty());
    }

    #[test]
    fn test_froxel_sdf_list_buffer_clear_all() {
        let mut buffer = FroxelSDFListBuffer::new();

        // Add SDFs to multiple froxels
        buffer.add_sdf(0, 0, 0, 1);
        buffer.add_sdf(1, 1, 1, 2);
        buffer.add_sdf(5, 5, 5, 3);

        assert!(!buffer.get(0, 0, 0).is_empty());
        assert!(!buffer.get(1, 1, 1).is_empty());
        assert!(!buffer.get(5, 5, 5).is_empty());

        // Clear all
        buffer.clear_all();

        assert!(buffer.get(0, 0, 0).is_empty());
        assert!(buffer.get(1, 1, 1).is_empty());
        assert!(buffer.get(5, 5, 5).is_empty());
    }

    #[test]
    fn test_buffer_size_constants() {
        assert_eq!(FROXEL_BOUNDS_SIZE, 32);
        assert_eq!(FROXEL_SDF_LIST_SIZE, 272);
        assert_eq!(FROXEL_BOUNDS_BUFFER_SIZE, 16 + 32 * 6144);
        assert_eq!(FROXEL_SDF_LIST_BUFFER_SIZE, 16 + 272 * 6144);
    }

    #[test]
    fn test_total_froxels_matches() {
        // Verify TOTAL_FROXELS = 16 * 16 * 24 = 6144
        assert_eq!(
            TOTAL_FROXELS,
            FROXEL_TILES_X * FROXEL_TILES_Y * FROXEL_DEPTH_SLICES
        );
        assert_eq!(TOTAL_FROXELS, 6144);
    }
}
