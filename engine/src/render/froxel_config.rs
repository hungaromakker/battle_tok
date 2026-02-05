//! Froxel Configuration Module
//!
//! This module defines constants and configuration for the froxel (frustum + voxel)
//! grid system used for spatial culling of SDFs during raymarching.
//!
//! Froxels divide the view frustum into a 3D grid:
//! - X/Y: Screen-space tiles (FROXEL_TILES_X × FROXEL_TILES_Y)
//! - Z: Exponentially distributed depth slices (FROXEL_DEPTH_SLICES)
//!
//! The exponential depth distribution places more slices near the camera where
//! precision matters most, and fewer slices in the distance.

/// Number of froxel tiles in the X (horizontal) direction.
pub const FROXEL_TILES_X: u32 = 16;

/// Number of froxel tiles in the Y (vertical) direction.
pub const FROXEL_TILES_Y: u32 = 16;

/// Number of depth slices for froxel partitioning.
/// Uses exponential distribution for better near-field precision.
pub const FROXEL_DEPTH_SLICES: u32 = 24;

/// Maximum number of SDFs that can be assigned to a single froxel.
/// This limits memory usage per froxel while allowing reasonable entity density.
pub const MAX_SDFS_PER_FROXEL: u32 = 64;

/// Total number of froxels in the grid (tiles_x * tiles_y * depth_slices).
pub const TOTAL_FROXELS: u32 = FROXEL_TILES_X * FROXEL_TILES_Y * FROXEL_DEPTH_SLICES;

/// Calculate the near and far depth bounds for a given depth slice.
///
/// Uses exponential distribution: `depth = near * (far/near)^(slice/total_slices)`
///
/// This places more slices near the camera where detail matters, and fewer
/// slices in the distance where objects appear smaller.
///
/// # Arguments
///
/// * `slice` - The depth slice index (0 to FROXEL_DEPTH_SLICES - 1)
/// * `near` - The near plane distance
/// * `far` - The far plane distance
///
/// # Returns
///
/// A tuple `(slice_near, slice_far)` containing the near and far bounds of the slice.
///
/// # Example
///
/// ```
/// use battle_tok_engine::render::froxel_config::depth_slice_bounds;
///
/// // Get bounds for slice 0 (closest to camera)
/// let (near, far) = depth_slice_bounds(0, 0.1, 1000.0);
/// assert!(near >= 0.1);
/// assert!(far > near);
/// ```
pub fn depth_slice_bounds(slice: u32, near: f32, far: f32) -> (f32, f32) {
    let total_slices = FROXEL_DEPTH_SLICES as f32;
    let slice_f = slice as f32;

    // Exponential depth distribution: d = near * (far/near)^(t)
    // where t goes from 0 to 1 across slices
    let ratio = far / near;

    let t_near = slice_f / total_slices;
    let t_far = (slice_f + 1.0) / total_slices;

    let slice_near = near * ratio.powf(t_near);
    let slice_far = near * ratio.powf(t_far);

    (slice_near, slice_far)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(FROXEL_TILES_X, 16);
        assert_eq!(FROXEL_TILES_Y, 16);
        assert_eq!(FROXEL_DEPTH_SLICES, 24);
        assert_eq!(MAX_SDFS_PER_FROXEL, 64);
        assert_eq!(TOTAL_FROXELS, 16 * 16 * 24);
    }

    #[test]
    fn test_depth_slice_bounds_first_slice() {
        let (slice_near, slice_far) = depth_slice_bounds(0, 0.1, 1000.0);
        // First slice should start at near plane
        assert!((slice_near - 0.1).abs() < 0.0001);
        // Far should be greater than near
        assert!(slice_far > slice_near);
    }

    #[test]
    fn test_depth_slice_bounds_last_slice() {
        let (slice_near, slice_far) = depth_slice_bounds(FROXEL_DEPTH_SLICES - 1, 0.1, 1000.0);
        // Last slice should end at far plane
        assert!((slice_far - 1000.0).abs() < 0.01);
        assert!(slice_far > slice_near);
    }

    #[test]
    fn test_depth_slice_bounds_continuity() {
        let near = 0.1;
        let far = 1000.0;

        // Each slice's far should equal the next slice's near
        for i in 0..FROXEL_DEPTH_SLICES - 1 {
            let (_, this_far) = depth_slice_bounds(i, near, far);
            let (next_near, _) = depth_slice_bounds(i + 1, near, far);
            assert!(
                (this_far - next_near).abs() < 0.0001,
                "Slice {} far ({}) should match slice {} near ({})",
                i,
                this_far,
                i + 1,
                next_near
            );
        }
    }

    #[test]
    fn test_depth_slice_bounds_exponential_distribution() {
        let near = 0.1;
        let far = 1000.0;

        // Near slices should be smaller (cover less depth range)
        let (near0, far0) = depth_slice_bounds(0, near, far);
        let (near_last, far_last) = depth_slice_bounds(FROXEL_DEPTH_SLICES - 1, near, far);

        let first_slice_range = far0 - near0;
        let last_slice_range = far_last - near_last;

        // Last slice should cover a much larger range than first
        assert!(
            last_slice_range > first_slice_range * 10.0,
            "Exponential distribution: last slice range ({}) should be >> first ({})",
            last_slice_range,
            first_slice_range
        );
    }
}
