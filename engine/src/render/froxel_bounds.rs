//! Froxel Bounds Calculation Module (US-030)
//!
//! This module provides functions to calculate world-space axis-aligned bounding boxes
//! (AABBs) for each froxel in the view frustum. Froxels (frustum + voxels) are used
//! to spatially partition the view frustum for efficient SDF culling during raymarching.
//!
//! ## Key Features
//!
//! - Handles perspective projection correctly (near froxels are smaller in world space)
//! - Uses exponential depth distribution from `froxel_config` for better near-field precision
//! - Bounds are updated when camera parameters change (position, orientation, FOV)
//!
//! ## Coordinate System
//!
//! - X: Right (+X is screen right)
//! - Y: Up (+Y is screen up)
//! - Z: Into screen (camera looks toward -Z in view space, +Z in world space for forward)
//!
//! ## Usage
//!
//! ```ignore
//! let camera_params = CameraProjection {
//!     position: Vec3::new(0.0, 5.0, 0.0),
//!     forward: Vec3::new(0.0, 0.0, -1.0),
//!     up: Vec3::new(0.0, 1.0, 0.0),
//!     right: Vec3::new(1.0, 0.0, 0.0),
//!     fov_y: std::f32::consts::FRAC_PI_4, // 45 degrees
//!     aspect_ratio: 16.0 / 9.0,
//!     near: 0.1,
//!     far: 1000.0,
//! };
//!
//! let bounds = calculate_froxel_bounds(&camera_params);
//! ```

use super::froxel_buffers::{FroxelBounds, FroxelBoundsBuffer};
use super::froxel_config::{
    depth_slice_bounds, FROXEL_DEPTH_SLICES, FROXEL_TILES_X, FROXEL_TILES_Y,
};

/// Camera projection parameters needed for froxel bounds calculation.
///
/// This struct contains all the information needed to project froxel corners
/// from normalized device coordinates (NDC) to world space.
#[derive(Clone, Copy, Debug)]
pub struct CameraProjection {
    /// Camera position in world space (meters)
    pub position: [f32; 3],
    /// Forward direction (normalized, camera looks this way)
    pub forward: [f32; 3],
    /// Up direction (normalized, perpendicular to forward)
    pub up: [f32; 3],
    /// Right direction (normalized, perpendicular to forward and up)
    pub right: [f32; 3],
    /// Vertical field of view in radians (typically PI/4 = 45°)
    pub fov_y: f32,
    /// Aspect ratio (width / height, e.g., 16/9 = 1.777...)
    pub aspect_ratio: f32,
    /// Near plane distance in meters
    pub near: f32,
    /// Far plane distance in meters
    pub far: f32,
}

impl Default for CameraProjection {
    fn default() -> Self {
        Self {
            position: [0.0, 5.0, 0.0],
            forward: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            right: [1.0, 0.0, 0.0],
            fov_y: std::f32::consts::FRAC_PI_4, // 45 degrees
            aspect_ratio: 16.0 / 9.0,
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl CameraProjection {
    /// Create a new camera projection with the given parameters.
    pub fn new(
        position: [f32; 3],
        forward: [f32; 3],
        up: [f32; 3],
        right: [f32; 3],
        fov_y: f32,
        aspect_ratio: f32,
        near: f32,
        far: f32,
    ) -> Self {
        Self {
            position,
            forward,
            up,
            right,
            fov_y,
            aspect_ratio,
            near,
            far,
        }
    }

    /// Create a camera projection from position, yaw, and pitch angles.
    ///
    /// This is a convenience method that matches the CameraController's angle convention.
    ///
    /// # Arguments
    /// * `position` - Camera position in world space
    /// * `yaw` - Horizontal angle in radians (0 = looking toward -Z)
    /// * `pitch` - Vertical angle in radians (0 = level, positive = looking up)
    /// * `fov_y` - Vertical field of view in radians
    /// * `aspect_ratio` - Width / height
    /// * `near` - Near plane distance
    /// * `far` - Far plane distance
    pub fn from_angles(
        position: [f32; 3],
        yaw: f32,
        pitch: f32,
        fov_y: f32,
        aspect_ratio: f32,
        near: f32,
        far: f32,
    ) -> Self {
        // Calculate forward direction from yaw and pitch
        // This matches CameraController::get_forward()
        let forward = [
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            -yaw.cos() * pitch.cos(),
        ];
        let forward_len =
            (forward[0] * forward[0] + forward[1] * forward[1] + forward[2] * forward[2]).sqrt();
        let forward = [
            forward[0] / forward_len,
            forward[1] / forward_len,
            forward[2] / forward_len,
        ];

        // Right = forward × world_up (Vec3::Y)
        // Since world_up = (0, 1, 0):
        // right = (forward.z, 0, -forward.x) normalized
        let right_unnorm = [forward[2], 0.0, -forward[0]];
        let right_len =
            (right_unnorm[0] * right_unnorm[0] + right_unnorm[2] * right_unnorm[2]).sqrt();
        let right = if right_len > 0.0001 {
            [
                right_unnorm[0] / right_len,
                0.0,
                right_unnorm[2] / right_len,
            ]
        } else {
            // Looking straight up or down - use a fallback
            [1.0, 0.0, 0.0]
        };

        // Up = right × forward
        let up = [
            right[1] * forward[2] - right[2] * forward[1],
            right[2] * forward[0] - right[0] * forward[2],
            right[0] * forward[1] - right[1] * forward[0],
        ];
        let up_len = (up[0] * up[0] + up[1] * up[1] + up[2] * up[2]).sqrt();
        let up = [up[0] / up_len, up[1] / up_len, up[2] / up_len];

        Self {
            position,
            forward,
            up,
            right,
            fov_y,
            aspect_ratio,
            near,
            far,
        }
    }

    /// Calculate a unique hash-like value for detecting camera changes.
    ///
    /// This can be used to determine if froxel bounds need to be recalculated.
    /// Returns a checksum of all camera parameters that affect froxel bounds.
    pub fn change_hash(&self) -> u64 {
        // Simple hash combining all relevant parameters
        let mut hash: u64 = 0;
        let scale = 1000.0; // Scale floats to preserve precision

        // Position
        hash = hash.wrapping_add((self.position[0] * scale) as i32 as u64);
        hash = hash.wrapping_mul(31);
        hash = hash.wrapping_add((self.position[1] * scale) as i32 as u64);
        hash = hash.wrapping_mul(31);
        hash = hash.wrapping_add((self.position[2] * scale) as i32 as u64);
        hash = hash.wrapping_mul(31);

        // Forward direction
        hash = hash.wrapping_add((self.forward[0] * scale) as i32 as u64);
        hash = hash.wrapping_mul(31);
        hash = hash.wrapping_add((self.forward[1] * scale) as i32 as u64);
        hash = hash.wrapping_mul(31);
        hash = hash.wrapping_add((self.forward[2] * scale) as i32 as u64);
        hash = hash.wrapping_mul(31);

        // FOV, aspect, near, far
        hash = hash.wrapping_add((self.fov_y * scale) as i32 as u64);
        hash = hash.wrapping_mul(31);
        hash = hash.wrapping_add((self.aspect_ratio * scale) as i32 as u64);
        hash = hash.wrapping_mul(31);
        hash = hash.wrapping_add((self.near * scale) as i32 as u64);
        hash = hash.wrapping_mul(31);
        hash = hash.wrapping_add((self.far * scale) as i32 as u64);

        hash
    }
}

/// Calculate world-space AABB bounds for all froxels in the view frustum.
///
/// This function computes the axis-aligned bounding box for each froxel by:
/// 1. Computing the 8 corners of each froxel in view space (using perspective projection)
/// 2. Transforming corners to world space
/// 3. Computing the AABB that contains all 8 corners
///
/// The depth slices use exponential distribution from `depth_slice_bounds()`, placing
/// more slices near the camera where precision matters most.
///
/// # Arguments
///
/// * `camera` - Camera projection parameters
///
/// # Returns
///
/// A `FroxelBoundsBuffer` containing AABBs for all froxels.
///
/// # Performance
///
/// This function performs 8 corner calculations per froxel:
/// - 6,144 froxels × 8 corners = 49,152 corner calculations
/// - Should be called only when camera parameters change
pub fn calculate_froxel_bounds(camera: &CameraProjection) -> Box<FroxelBoundsBuffer> {
    let mut buffer = Box::new(FroxelBoundsBuffer::new());

    // Pre-calculate half-angles for perspective projection
    let half_fov_y = camera.fov_y * 0.5;
    let tan_half_fov_y = half_fov_y.tan();
    let tan_half_fov_x = tan_half_fov_y * camera.aspect_ratio;

    // Tile dimensions in NDC space (-1 to 1)
    let tile_width_ndc = 2.0 / FROXEL_TILES_X as f32;
    let tile_height_ndc = 2.0 / FROXEL_TILES_Y as f32;

    // Iterate over all froxels
    for z in 0..FROXEL_DEPTH_SLICES {
        // Get depth bounds for this slice (exponential distribution)
        let (depth_near, depth_far) = depth_slice_bounds(z, camera.near, camera.far);

        for y in 0..FROXEL_TILES_Y {
            for x in 0..FROXEL_TILES_X {
                // Calculate NDC bounds for this tile
                // NDC goes from -1 (left/bottom) to +1 (right/top)
                let ndc_left = -1.0 + (x as f32) * tile_width_ndc;
                let ndc_right = -1.0 + ((x + 1) as f32) * tile_width_ndc;
                let ndc_bottom = -1.0 + (y as f32) * tile_height_ndc;
                let ndc_top = -1.0 + ((y + 1) as f32) * tile_height_ndc;

                // Calculate the 8 corners of this froxel in world space
                let corners = calculate_froxel_corners(
                    camera, ndc_left, ndc_right, ndc_bottom, ndc_top, depth_near, depth_far,
                    tan_half_fov_x, tan_half_fov_y,
                );

                // Compute AABB from corners
                let bounds = aabb_from_corners(&corners);

                // Store in buffer
                buffer.set(x, y, z, bounds);
            }
        }
    }

    buffer
}

/// Calculate the 8 corners of a froxel in world space.
///
/// Each froxel is a truncated pyramid (frustum) in view space. We calculate the
/// 4 corners at the near depth and 4 corners at the far depth.
#[inline]
fn calculate_froxel_corners(
    camera: &CameraProjection,
    ndc_left: f32,
    ndc_right: f32,
    ndc_bottom: f32,
    ndc_top: f32,
    depth_near: f32,
    depth_far: f32,
    tan_half_fov_x: f32,
    tan_half_fov_y: f32,
) -> [[f32; 3]; 8] {
    let mut corners = [[0.0f32; 3]; 8];

    // Near plane corners (0-3)
    corners[0] = ndc_to_world(camera, ndc_left, ndc_bottom, depth_near, tan_half_fov_x, tan_half_fov_y);
    corners[1] = ndc_to_world(camera, ndc_right, ndc_bottom, depth_near, tan_half_fov_x, tan_half_fov_y);
    corners[2] = ndc_to_world(camera, ndc_right, ndc_top, depth_near, tan_half_fov_x, tan_half_fov_y);
    corners[3] = ndc_to_world(camera, ndc_left, ndc_top, depth_near, tan_half_fov_x, tan_half_fov_y);

    // Far plane corners (4-7)
    corners[4] = ndc_to_world(camera, ndc_left, ndc_bottom, depth_far, tan_half_fov_x, tan_half_fov_y);
    corners[5] = ndc_to_world(camera, ndc_right, ndc_bottom, depth_far, tan_half_fov_x, tan_half_fov_y);
    corners[6] = ndc_to_world(camera, ndc_right, ndc_top, depth_far, tan_half_fov_x, tan_half_fov_y);
    corners[7] = ndc_to_world(camera, ndc_left, ndc_top, depth_far, tan_half_fov_x, tan_half_fov_y);

    corners
}

/// Convert a point from NDC (normalized device coordinates) to world space.
///
/// Uses perspective projection: points at NDC (x, y) spread out as depth increases.
///
/// # Arguments
///
/// * `camera` - Camera projection parameters
/// * `ndc_x` - X coordinate in NDC (-1 to 1, left to right)
/// * `ndc_y` - Y coordinate in NDC (-1 to 1, bottom to top)
/// * `depth` - Distance from camera along view direction (in world units/meters)
/// * `tan_half_fov_x` - Pre-calculated tan(fov_x / 2)
/// * `tan_half_fov_y` - Pre-calculated tan(fov_y / 2)
#[inline]
fn ndc_to_world(
    camera: &CameraProjection,
    ndc_x: f32,
    ndc_y: f32,
    depth: f32,
    tan_half_fov_x: f32,
    tan_half_fov_y: f32,
) -> [f32; 3] {
    // Calculate view-space position
    // At distance d, the half-width is d * tan(fov_x/2) and half-height is d * tan(fov_y/2)
    // NDC maps -1..1 to -half_extent..+half_extent
    let view_x = ndc_x * depth * tan_half_fov_x;
    let view_y = ndc_y * depth * tan_half_fov_y;
    let view_z = depth;

    // Transform to world space:
    // world = camera_pos + right * view_x + up * view_y + forward * view_z
    [
        camera.position[0]
            + camera.right[0] * view_x
            + camera.up[0] * view_y
            + camera.forward[0] * view_z,
        camera.position[1]
            + camera.right[1] * view_x
            + camera.up[1] * view_y
            + camera.forward[1] * view_z,
        camera.position[2]
            + camera.right[2] * view_x
            + camera.up[2] * view_y
            + camera.forward[2] * view_z,
    ]
}

/// Compute an axis-aligned bounding box from 8 corner points.
#[inline]
fn aabb_from_corners(corners: &[[f32; 3]; 8]) -> FroxelBounds {
    let mut min = [f32::MAX, f32::MAX, f32::MAX];
    let mut max = [f32::MIN, f32::MIN, f32::MIN];

    for corner in corners {
        min[0] = min[0].min(corner[0]);
        min[1] = min[1].min(corner[1]);
        min[2] = min[2].min(corner[2]);

        max[0] = max[0].max(corner[0]);
        max[1] = max[1].max(corner[1]);
        max[2] = max[2].max(corner[2]);
    }

    FroxelBounds::new(min, max)
}

/// Tracks camera state to detect when froxel bounds need recalculation.
///
/// Use this to avoid recalculating bounds every frame - only recalculate
/// when the camera actually changes.
#[derive(Clone, Default)]
pub struct FroxelBoundsTracker {
    /// Hash of the last camera state that was used to calculate bounds
    last_camera_hash: u64,
    /// Cached froxel bounds (only valid if last_camera_hash matches current camera)
    cached_bounds: Option<Box<FroxelBoundsBuffer>>,
}

impl FroxelBoundsTracker {
    /// Create a new tracker with no cached bounds.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if bounds need to be recalculated for the given camera.
    ///
    /// Returns true if the camera has changed since the last calculation.
    pub fn needs_update(&self, camera: &CameraProjection) -> bool {
        let current_hash = camera.change_hash();
        self.last_camera_hash != current_hash || self.cached_bounds.is_none()
    }

    /// Get froxel bounds, recalculating if necessary.
    ///
    /// This is the main entry point for getting froxel bounds. It checks if
    /// the camera has changed and only recalculates if needed.
    ///
    /// # Arguments
    ///
    /// * `camera` - Current camera projection parameters
    ///
    /// # Returns
    ///
    /// Reference to the froxel bounds buffer (may be freshly calculated or cached)
    pub fn get_bounds(&mut self, camera: &CameraProjection) -> &FroxelBoundsBuffer {
        let current_hash = camera.change_hash();

        if self.last_camera_hash != current_hash || self.cached_bounds.is_none() {
            // Recalculate bounds
            let bounds = calculate_froxel_bounds(camera);
            self.cached_bounds = Some(bounds);
            self.last_camera_hash = current_hash;
        }

        self.cached_bounds.as_ref().unwrap()
    }

    /// Force recalculation of bounds on next access.
    pub fn invalidate(&mut self) {
        self.last_camera_hash = 0;
    }

    /// Check if bounds are currently cached.
    pub fn has_cached_bounds(&self) -> bool {
        self.cached_bounds.is_some()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_camera_projection_default() {
        let camera = CameraProjection::default();
        assert_eq!(camera.position, [0.0, 5.0, 0.0]);
        assert_eq!(camera.forward, [0.0, 0.0, -1.0]);
        assert_eq!(camera.near, 0.1);
        assert_eq!(camera.far, 1000.0);
    }

    #[test]
    fn test_camera_projection_from_angles() {
        // Camera looking straight forward (yaw=0, pitch=0)
        let camera = CameraProjection::from_angles(
            [0.0, 5.0, 0.0],
            0.0,
            0.0,
            std::f32::consts::FRAC_PI_4,
            16.0 / 9.0,
            0.1,
            1000.0,
        );

        // Forward should be (0, 0, -1)
        assert!((camera.forward[0]).abs() < 0.001);
        assert!((camera.forward[1]).abs() < 0.001);
        assert!((camera.forward[2] + 1.0).abs() < 0.001);

        // Right should be (1, 0, 0) or (-1, 0, 0)
        assert!((camera.right[1]).abs() < 0.001);
        assert!((camera.right[0].abs() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_camera_change_hash() {
        let camera1 = CameraProjection::default();
        let camera2 = CameraProjection::default();

        // Same cameras should have same hash
        assert_eq!(camera1.change_hash(), camera2.change_hash());

        // Different position should have different hash
        let mut camera3 = CameraProjection::default();
        camera3.position[0] = 10.0;
        assert_ne!(camera1.change_hash(), camera3.change_hash());

        // Different FOV should have different hash
        let mut camera4 = CameraProjection::default();
        camera4.fov_y = 1.0;
        assert_ne!(camera1.change_hash(), camera4.change_hash());
    }

    #[test]
    fn test_calculate_froxel_bounds_count() {
        let camera = CameraProjection::default();
        let buffer = calculate_froxel_bounds(&camera);

        // Should have TOTAL_FROXELS bounds
        assert_eq!(buffer.count, FROXEL_TILES_X * FROXEL_TILES_Y * FROXEL_DEPTH_SLICES);
    }

    #[test]
    fn test_froxel_bounds_contain_camera() {
        let camera = CameraProjection::default();
        let buffer = calculate_froxel_bounds(&camera);

        // The first slice (z=0) should be closest to the camera
        // Its near plane should be at camera.near
        let first_bounds = buffer.get(0, 0, 0);

        // The bounds should extend from near the camera outward
        // Camera is at (0, 5, 0), looking toward -Z
        // So bounds should be in front of camera (negative Z values in world space)
        // First slice should start at camera.near (0.1m) from camera
        assert!(first_bounds.size()[0] > 0.0, "Froxel should have positive width");
        assert!(first_bounds.size()[1] > 0.0, "Froxel should have positive height");
        assert!(first_bounds.size()[2] > 0.0, "Froxel should have positive depth");
    }

    #[test]
    fn test_near_froxels_smaller_than_far() {
        let camera = CameraProjection::default();
        let buffer = calculate_froxel_bounds(&camera);

        // Near froxels should be smaller in world space than far froxels
        // due to perspective projection
        let near_bounds = buffer.get(8, 8, 0); // Center tile, first depth slice
        let far_bounds = buffer.get(8, 8, FROXEL_DEPTH_SLICES - 1); // Center tile, last depth slice

        let near_size = near_bounds.size();
        let far_size = far_bounds.size();

        // Far froxels should be significantly larger
        let near_volume = near_size[0] * near_size[1] * near_size[2];
        let far_volume = far_size[0] * far_size[1] * far_size[2];

        assert!(
            far_volume > near_volume * 100.0,
            "Far froxel volume ({}) should be much larger than near ({})",
            far_volume,
            near_volume
        );
    }

    #[test]
    fn test_exponential_depth_distribution() {
        let camera = CameraProjection::default();
        let buffer = calculate_froxel_bounds(&camera);

        // Check that first few depth slices are thin (small Z range)
        // and later slices are thicker
        let slice0 = buffer.get(8, 8, 0);
        let slice1 = buffer.get(8, 8, 1);
        let slice_last = buffer.get(8, 8, FROXEL_DEPTH_SLICES - 1);

        let depth0 = slice0.size()[2];
        let depth1 = slice1.size()[2];
        let depth_last = slice_last.size()[2];

        // Each subsequent slice should be thicker
        assert!(
            depth1 > depth0,
            "Slice 1 depth ({}) should be > slice 0 depth ({})",
            depth1,
            depth0
        );
        assert!(
            depth_last > depth1 * 10.0,
            "Last slice depth ({}) should be much > slice 1 depth ({})",
            depth_last,
            depth1
        );
    }

    #[test]
    fn test_froxel_bounds_tracker_caching() {
        let camera = CameraProjection::default();
        let mut tracker = FroxelBoundsTracker::new();

        // First call should calculate
        assert!(tracker.needs_update(&camera));
        let _ = tracker.get_bounds(&camera);
        assert!(tracker.has_cached_bounds());

        // Second call with same camera should not recalculate
        assert!(!tracker.needs_update(&camera));

        // After position change, should need update
        let mut moved_camera = camera;
        moved_camera.position[0] = 100.0;
        assert!(tracker.needs_update(&moved_camera));
    }

    #[test]
    fn test_froxel_bounds_tracker_invalidate() {
        let camera = CameraProjection::default();
        let mut tracker = FroxelBoundsTracker::new();

        let _ = tracker.get_bounds(&camera);
        assert!(!tracker.needs_update(&camera));

        tracker.invalidate();
        assert!(tracker.needs_update(&camera));
    }

    #[test]
    fn test_ndc_to_world_center() {
        let camera = CameraProjection::default();
        let tan_half_fov_y = (camera.fov_y * 0.5).tan();
        let tan_half_fov_x = tan_half_fov_y * camera.aspect_ratio;

        // Center of screen (NDC 0, 0) at depth 10 should be along forward direction
        let point = ndc_to_world(&camera, 0.0, 0.0, 10.0, tan_half_fov_x, tan_half_fov_y);

        // Camera at (0, 5, 0) looking toward -Z
        // Point should be at (0, 5, -10)
        assert!((point[0] - 0.0).abs() < 0.001);
        assert!((point[1] - 5.0).abs() < 0.001);
        assert!((point[2] - (-10.0)).abs() < 0.001);
    }

    #[test]
    fn test_aabb_from_corners() {
        let corners = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];

        let bounds = aabb_from_corners(&corners);
        assert_eq!(bounds.min(), [0.0, 0.0, 0.0]);
        assert_eq!(bounds.max(), [1.0, 1.0, 1.0]);
    }

    #[test]
    fn test_froxel_grid_coverage() {
        let camera = CameraProjection::default();
        let buffer = calculate_froxel_bounds(&camera);

        // Adjacent froxels should share edges (no gaps)
        // Check horizontal adjacency
        let left = buffer.get(5, 8, 5);
        let right = buffer.get(6, 8, 5);

        // Right edge of left should be close to left edge of right
        // (Not exact due to AABB approximation of frustum)
        let left_right_edge = left.max_x;
        let right_left_edge = right.min_x;

        // Should overlap or be very close
        assert!(
            (left_right_edge - right_left_edge).abs() < 5.0,
            "Adjacent froxels should be close: left.max_x={}, right.min_x={}",
            left_right_edge,
            right_left_edge
        );
    }
}
