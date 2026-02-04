//! Collision detection module
//!
//! This module provides collision detection functionality for the
//! custom physics system. Uses ray-AABB intersection for hex-prism collision.
//!
//! # Ray-AABB Intersection
//!
//! The slab method is used for ray-AABB intersection, which finds the
//! intersection points by computing entry and exit times for each axis.
//!
//! # Example
//!
//! ```ignore
//! use battle_tok_engine::physics::collision::{ray_aabb_intersect, HitInfo, HexPrism, HexPrismGrid};
//! use glam::Vec3;
//!
//! // Ray-AABB intersection
//! let origin = Vec3::new(0.0, 0.0, -5.0);
//! let direction = Vec3::new(0.0, 0.0, 1.0);
//! let aabb_min = Vec3::new(-1.0, -1.0, -1.0);
//! let aabb_max = Vec3::new(1.0, 1.0, 1.0);
//!
//! if let Some(t) = ray_aabb_intersect(origin, direction, aabb_min, aabb_max) {
//!     let hit_point = origin + direction * t;
//!     println!("Hit at distance {}: {:?}", t, hit_point);
//! }
//!
//! // HexPrism collision
//! let mut grid = HexPrismGrid::new(1.0, 1.0);
//! let prism = HexPrism::new(Vec3::ZERO, 1.0, 0.5, 0);
//! grid.insert(0, 0, 0, prism);
//!
//! if let Some(hit) = grid.ray_cast(origin, direction, 100.0) {
//!     println!("Hit prism at {:?}", hit.prism_coord);
//! }
//! ```

use glam::Vec3;
use std::collections::HashMap;

/// Information about a ray-hex collision.
///
/// Contains the position, surface normal, and grid coordinates of the hit hex-prism.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitInfo {
    /// World-space position where the collision occurred
    pub position: Vec3,
    /// Surface normal at the hit point (normalized)
    pub normal: Vec3,
    /// Grid coordinates of the hit prism (q, r, level)
    pub prism_coord: (i32, i32, i32),
    /// Distance from ray origin to hit point
    pub distance: f32,
}

impl HitInfo {
    /// Creates a new HitInfo with the given parameters.
    pub fn new(position: Vec3, normal: Vec3, prism_coord: (i32, i32, i32), distance: f32) -> Self {
        Self {
            position,
            normal,
            prism_coord,
            distance,
        }
    }
}

/// Performs ray-AABB (Axis-Aligned Bounding Box) intersection test using the slab method.
///
/// The slab method works by finding the intersection of the ray with each pair of
/// axis-aligned planes that make up the AABB. If the ray enters and exits the AABB
/// at valid times (t_enter < t_exit and t_exit > 0), there is an intersection.
///
/// # Arguments
///
/// * `ray_origin` - Starting point of the ray
/// * `ray_dir` - Direction of the ray (must be normalized)
/// * `aabb_min` - Minimum corner of the AABB
/// * `aabb_max` - Maximum corner of the AABB
///
/// # Returns
///
/// * `Some(t)` - Distance along the ray to the intersection point (t >= 0)
/// * `None` - No intersection or intersection is behind the ray origin
pub fn ray_aabb_intersect(
    ray_origin: Vec3,
    ray_dir: Vec3,
    aabb_min: Vec3,
    aabb_max: Vec3,
) -> Option<f32> {
    // Compute inverse direction for efficient division
    // Handle near-zero directions by using large values
    let inv_dir = Vec3::new(
        if ray_dir.x.abs() > 1e-10 { 1.0 / ray_dir.x } else { f32::MAX * ray_dir.x.signum() },
        if ray_dir.y.abs() > 1e-10 { 1.0 / ray_dir.y } else { f32::MAX * ray_dir.y.signum() },
        if ray_dir.z.abs() > 1e-10 { 1.0 / ray_dir.z } else { f32::MAX * ray_dir.z.signum() },
    );

    // Compute intersection times with the two YZ planes (x = aabb_min.x and x = aabb_max.x)
    let t1 = (aabb_min.x - ray_origin.x) * inv_dir.x;
    let t2 = (aabb_max.x - ray_origin.x) * inv_dir.x;

    let mut t_min = t1.min(t2);
    let mut t_max = t1.max(t2);

    // Compute intersection times with the two XZ planes (y = aabb_min.y and y = aabb_max.y)
    let t3 = (aabb_min.y - ray_origin.y) * inv_dir.y;
    let t4 = (aabb_max.y - ray_origin.y) * inv_dir.y;

    t_min = t_min.max(t3.min(t4));
    t_max = t_max.min(t3.max(t4));

    // Compute intersection times with the two XY planes (z = aabb_min.z and z = aabb_max.z)
    let t5 = (aabb_min.z - ray_origin.z) * inv_dir.z;
    let t6 = (aabb_max.z - ray_origin.z) * inv_dir.z;

    t_min = t_min.max(t5.min(t6));
    t_max = t_max.min(t5.max(t6));

    // Check if there's a valid intersection
    if t_max >= t_min && t_max >= 0.0 {
        // Return the nearest positive intersection
        if t_min >= 0.0 {
            Some(t_min)
        } else {
            // Ray starts inside the AABB
            Some(t_max)
        }
    } else {
        None
    }
}

/// Computes the surface normal for a point on an AABB surface.
///
/// Determines which face of the AABB the point is on and returns the outward normal.
///
/// # Arguments
///
/// * `point` - Point on the AABB surface
/// * `aabb_min` - Minimum corner of the AABB
/// * `aabb_max` - Maximum corner of the AABB
///
/// # Returns
///
/// Normalized outward normal vector
pub fn aabb_surface_normal(point: Vec3, aabb_min: Vec3, aabb_max: Vec3) -> Vec3 {
    let center = (aabb_min + aabb_max) * 0.5;
    let half_extents = (aabb_max - aabb_min) * 0.5;
    let local = point - center;

    // Normalize to unit cube space
    let normalized = Vec3::new(
        local.x / half_extents.x,
        local.y / half_extents.y,
        local.z / half_extents.z,
    );

    // Find which face we're closest to (highest absolute normalized coordinate)
    let abs_normalized = normalized.abs();

    if abs_normalized.x >= abs_normalized.y && abs_normalized.x >= abs_normalized.z {
        Vec3::new(normalized.x.signum(), 0.0, 0.0)
    } else if abs_normalized.y >= abs_normalized.x && abs_normalized.y >= abs_normalized.z {
        Vec3::new(0.0, normalized.y.signum(), 0.0)
    } else {
        Vec3::new(0.0, 0.0, normalized.z.signum())
    }
}

// =============================================================================
// HexPrism - Hexagonal prism voxel for building walls
// =============================================================================

/// A hexagonal prism voxel used for building walls and fortifications.
///
/// The hex prism is stored with axial coordinates for efficient grid operations,
/// but uses an AABB approximation for fast collision detection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HexPrism {
    /// Center position of the hex prism base in world space
    pub center: Vec3,
    /// Height of the prism (extends upward from center.y)
    pub height: f32,
    /// Radius from center to hex vertex (circumradius)
    pub radius: f32,
    /// Material type identifier (for rendering/physics properties)
    pub material: u8,
}

impl HexPrism {
    /// Creates a new hex prism at the given position.
    ///
    /// # Arguments
    ///
    /// * `center` - World position of the prism base center
    /// * `height` - Height of the prism
    /// * `radius` - Circumradius (center to vertex distance)
    /// * `material` - Material type identifier
    pub fn new(center: Vec3, height: f32, radius: f32, material: u8) -> Self {
        Self {
            center,
            height,
            radius,
            material,
        }
    }

    /// Returns the axis-aligned bounding box that contains this hex prism.
    ///
    /// The AABB is slightly larger than the hex to ensure complete coverage.
    /// For a regular hexagon, the circumradius equals the distance from center to vertex.
    ///
    /// # Returns
    ///
    /// Tuple of (min_corner, max_corner) defining the AABB
    pub fn get_aabb(&self) -> (Vec3, Vec3) {
        // For a regular hexagon with circumradius R:
        // - Width (flat-to-flat): 2 * R * cos(30°) ≈ 1.732 * R
        // - But we use R for the AABB to fully contain all vertices
        let min = Vec3::new(
            self.center.x - self.radius,
            self.center.y,
            self.center.z - self.radius,
        );
        let max = Vec3::new(
            self.center.x + self.radius,
            self.center.y + self.height,
            self.center.z + self.radius,
        );
        (min, max)
    }

    /// Returns the world-space center of the prism (at mid-height).
    pub fn center_3d(&self) -> Vec3 {
        Vec3::new(
            self.center.x,
            self.center.y + self.height * 0.5,
            self.center.z,
        )
    }
}

impl Default for HexPrism {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            height: 1.0,
            radius: 0.5,
            material: 0,
        }
    }
}

// =============================================================================
// HexPrismGrid - Collection of hex prisms using axial coordinates
// =============================================================================

/// A grid of hex prisms stored using axial coordinates.
///
/// Uses a HashMap for sparse storage, allowing efficient insertion and removal
/// of individual prisms. Supports raycasting for collision detection.
///
/// # Coordinate System
///
/// Uses axial coordinates (q, r) plus a vertical level index:
/// - `q` increases to the right
/// - `r` increases down-right (pointy-top orientation)
/// - `level` is the vertical stack index (0 = ground level)
#[derive(Debug, Clone, Default)]
pub struct HexPrismGrid {
    /// Stored prisms indexed by (q, r, level)
    prisms: HashMap<(i32, i32, i32), HexPrism>,
    /// Horizontal spacing between adjacent hex centers
    pub hex_spacing: f32,
    /// Vertical height of each level
    pub level_height: f32,
    /// Radius of each hex prism
    pub prism_radius: f32,
}

impl HexPrismGrid {
    /// Creates a new empty hex prism grid.
    ///
    /// # Arguments
    ///
    /// * `hex_spacing` - Distance between adjacent hex centers
    /// * `level_height` - Height of each vertical level
    pub fn new(hex_spacing: f32, level_height: f32) -> Self {
        Self {
            prisms: HashMap::new(),
            hex_spacing,
            level_height,
            prism_radius: hex_spacing * 0.5,
        }
    }

    /// Creates a grid with custom prism radius.
    pub fn with_radius(hex_spacing: f32, level_height: f32, prism_radius: f32) -> Self {
        Self {
            prisms: HashMap::new(),
            hex_spacing,
            level_height,
            prism_radius,
        }
    }

    /// Converts axial grid coordinates to world position.
    ///
    /// Uses pointy-top hex orientation where:
    /// - q increases to the right
    /// - r increases down-right
    ///
    /// # Arguments
    ///
    /// * `q` - Axial q coordinate
    /// * `r` - Axial r coordinate
    /// * `level` - Vertical stack level
    ///
    /// # Returns
    ///
    /// World-space position of the hex center
    pub fn axial_to_world(&self, q: i32, r: i32, level: i32) -> Vec3 {
        // Pointy-top hex layout formulas
        let sqrt3 = 3.0_f32.sqrt();
        let x = self.hex_spacing * (sqrt3 * q as f32 + sqrt3 / 2.0 * r as f32);
        let z = self.hex_spacing * (1.5 * r as f32);
        let y = level as f32 * self.level_height;
        Vec3::new(x, y, z)
    }

    /// Inserts a hex prism at the given grid coordinate.
    ///
    /// The prism's world position is computed from the grid coordinates.
    /// If a prism already exists at this coordinate, it is replaced.
    ///
    /// # Arguments
    ///
    /// * `q`, `r` - Axial coordinates
    /// * `level` - Vertical stack level
    /// * `prism` - The hex prism to insert
    pub fn insert(&mut self, q: i32, r: i32, level: i32, prism: HexPrism) {
        self.prisms.insert((q, r, level), prism);
    }

    /// Inserts a hex prism at the given coordinate, computing position automatically.
    ///
    /// # Arguments
    ///
    /// * `q`, `r` - Axial coordinates
    /// * `level` - Vertical stack level
    /// * `material` - Material type for the new prism
    pub fn insert_auto(&mut self, q: i32, r: i32, level: i32, material: u8) {
        let center = self.axial_to_world(q, r, level);
        let prism = HexPrism::new(center, self.level_height, self.prism_radius, material);
        self.prisms.insert((q, r, level), prism);
    }

    /// Gets a reference to the prism at the given coordinate, if it exists.
    pub fn get(&self, q: i32, r: i32, level: i32) -> Option<&HexPrism> {
        self.prisms.get(&(q, r, level))
    }

    /// Gets a mutable reference to the prism at the given coordinate.
    pub fn get_mut(&mut self, q: i32, r: i32, level: i32) -> Option<&mut HexPrism> {
        self.prisms.get_mut(&(q, r, level))
    }

    /// Removes the prism at the given coordinate.
    ///
    /// # Returns
    ///
    /// The removed prism, if one existed at this coordinate
    pub fn remove(&mut self, q: i32, r: i32, level: i32) -> Option<HexPrism> {
        self.prisms.remove(&(q, r, level))
    }

    /// Returns the number of prisms in the grid.
    pub fn len(&self) -> usize {
        self.prisms.len()
    }

    /// Returns true if the grid contains no prisms.
    pub fn is_empty(&self) -> bool {
        self.prisms.is_empty()
    }

    /// Clears all prisms from the grid.
    pub fn clear(&mut self) {
        self.prisms.clear();
    }

    /// Returns an iterator over all (coordinate, prism) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&(i32, i32, i32), &HexPrism)> {
        self.prisms.iter()
    }

    /// Casts a ray against all prisms in the grid and returns the closest hit.
    ///
    /// This uses brute-force iteration over all prisms with AABB collision.
    /// For large grids, consider implementing spatial partitioning (octree, BVH).
    ///
    /// # Arguments
    ///
    /// * `origin` - Ray starting position
    /// * `direction` - Ray direction (should be normalized)
    /// * `max_dist` - Maximum distance to check for intersections
    ///
    /// # Returns
    ///
    /// `Some(HitInfo)` for the closest hit, or `None` if no intersection
    pub fn ray_cast(&self, origin: Vec3, direction: Vec3, max_dist: f32) -> Option<HitInfo> {
        let mut closest: Option<HitInfo> = None;
        let mut closest_dist = max_dist;

        for (&(q, r, level), prism) in &self.prisms {
            let (aabb_min, aabb_max) = prism.get_aabb();

            if let Some(t) = ray_aabb_intersect(origin, direction, aabb_min, aabb_max) {
                // Only consider hits in front and within range
                if t >= 0.0 && t < closest_dist {
                    let hit_position = origin + direction * t;
                    let normal = aabb_surface_normal(hit_position, aabb_min, aabb_max);

                    closest = Some(HitInfo {
                        position: hit_position,
                        normal,
                        prism_coord: (q, r, level),
                        distance: t,
                    });
                    closest_dist = t;
                }
            }
        }

        closest
    }

    /// Checks if a ray intersects any prism in the grid.
    ///
    /// Faster than `ray_cast` when you only need to know if a hit occurred.
    pub fn ray_test(&self, origin: Vec3, direction: Vec3, max_dist: f32) -> bool {
        for prism in self.prisms.values() {
            let (aabb_min, aabb_max) = prism.get_aabb();
            if let Some(t) = ray_aabb_intersect(origin, direction, aabb_min, aabb_max) {
                if t >= 0.0 && t < max_dist {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_hits_aabb_from_front() {
        let origin = Vec3::new(0.0, 0.0, -5.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);
        let aabb_min = Vec3::new(-1.0, -1.0, -1.0);
        let aabb_max = Vec3::new(1.0, 1.0, 1.0);

        let result = ray_aabb_intersect(origin, dir, aabb_min, aabb_max);
        assert!(result.is_some());
        let t = result.unwrap();
        assert!((t - 4.0).abs() < 0.001, "Expected t=4.0, got t={}", t);
    }

    #[test]
    fn test_ray_misses_aabb() {
        let origin = Vec3::new(0.0, 5.0, -5.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);
        let aabb_min = Vec3::new(-1.0, -1.0, -1.0);
        let aabb_max = Vec3::new(1.0, 1.0, 1.0);

        let result = ray_aabb_intersect(origin, dir, aabb_min, aabb_max);
        assert!(result.is_none());
    }

    #[test]
    fn test_ray_starts_inside_aabb() {
        let origin = Vec3::new(0.0, 0.0, 0.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);
        let aabb_min = Vec3::new(-1.0, -1.0, -1.0);
        let aabb_max = Vec3::new(1.0, 1.0, 1.0);

        let result = ray_aabb_intersect(origin, dir, aabb_min, aabb_max);
        assert!(result.is_some());
        let t = result.unwrap();
        // Should hit the exit face at z=1
        assert!((t - 1.0).abs() < 0.001, "Expected t=1.0, got t={}", t);
    }

    #[test]
    fn test_ray_aabb_behind_origin() {
        let origin = Vec3::new(0.0, 0.0, 5.0);
        let dir = Vec3::new(0.0, 0.0, 1.0);
        let aabb_min = Vec3::new(-1.0, -1.0, -1.0);
        let aabb_max = Vec3::new(1.0, 1.0, 1.0);

        // AABB is behind the ray origin
        let result = ray_aabb_intersect(origin, dir, aabb_min, aabb_max);
        assert!(result.is_none());
    }

    #[test]
    fn test_surface_normal_x_face() {
        let aabb_min = Vec3::new(-1.0, -1.0, -1.0);
        let aabb_max = Vec3::new(1.0, 1.0, 1.0);

        let point = Vec3::new(1.0, 0.0, 0.0);
        let normal = aabb_surface_normal(point, aabb_min, aabb_max);
        assert_eq!(normal, Vec3::X);

        let point = Vec3::new(-1.0, 0.0, 0.0);
        let normal = aabb_surface_normal(point, aabb_min, aabb_max);
        assert_eq!(normal, Vec3::NEG_X);
    }

    #[test]
    fn test_surface_normal_y_face() {
        let aabb_min = Vec3::new(-1.0, -1.0, -1.0);
        let aabb_max = Vec3::new(1.0, 1.0, 1.0);

        let point = Vec3::new(0.0, 1.0, 0.0);
        let normal = aabb_surface_normal(point, aabb_min, aabb_max);
        assert_eq!(normal, Vec3::Y);

        let point = Vec3::new(0.0, -1.0, 0.0);
        let normal = aabb_surface_normal(point, aabb_min, aabb_max);
        assert_eq!(normal, Vec3::NEG_Y);
    }

    #[test]
    fn test_hit_info_new() {
        let hit = HitInfo::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::Y,
            (0, 1, 2),
            5.0,
        );
        assert_eq!(hit.position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(hit.normal, Vec3::Y);
        assert_eq!(hit.prism_coord, (0, 1, 2));
        assert_eq!(hit.distance, 5.0);
    }
}
