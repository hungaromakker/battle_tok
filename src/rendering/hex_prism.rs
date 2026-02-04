//! Hex-Prism Voxel Data Structures
//!
//! This module provides data structures for stackable hexagonal prism voxels,
//! designed for building walls and fortifications in the Battle Sphere game.
//!
//! Hex-prisms use axial coordinates (q, r) for the horizontal plane plus a
//! level coordinate for vertical stacking. This system naturally matches
//! the game's hex grid and allows organic-looking structures.
//!
//! # Coordinate System
//!
//! - **Axial coordinates (q, r)**: Position on the hex grid
//! - **Level**: Vertical stacking index (0 = ground level)
//! - World Y position = level * height
//!
//! # Example
//!
//! ```ignore
//! use magic_engine::rendering::hex_prism::{HexPrism, HexPrismGrid};
//! use glam::Vec3;
//!
//! let mut grid = HexPrismGrid::new(0.5, 0.3); // radius=0.5, height=0.3
//!
//! // Insert hex-prisms to build a wall
//! grid.insert(0, 0, 0, 1); // material 1 at origin, ground level
//! grid.insert(0, 0, 1, 1); // stack another on top
//! grid.insert(1, 0, 0, 1); // adjacent hex
//!
//! // Query prisms
//! if let Some(prism) = grid.get(0, 0, 0) {
//!     println!("Found prism at {:?}", prism.center);
//! }
//! ```

use glam::Vec3;
use std::collections::HashMap;

/// A single hexagonal prism voxel.
///
/// Hex-prisms are flat-topped hexagons extruded vertically. They stack
/// cleanly and tessellate without gaps, making them ideal for building
/// walls and fortifications that don't show obvious grid artifacts.
#[derive(Clone, Debug, PartialEq)]
pub struct HexPrism {
    /// Center position in world coordinates
    pub center: Vec3,
    /// Height of the prism (vertical extent)
    pub height: f32,
    /// Circumradius of the hexagon (distance from center to vertex)
    pub radius: f32,
    /// Material identifier (determines appearance/properties)
    pub material: u8,
}

impl HexPrism {
    /// Creates a new hex-prism with the given properties.
    pub fn new(center: Vec3, height: f32, radius: f32, material: u8) -> Self {
        Self {
            center,
            height,
            radius,
            material,
        }
    }
}

/// A grid of stackable hexagonal prism voxels.
///
/// Uses axial coordinates (q, r) plus a level index to address each hex-prism.
/// The grid stores prisms in a sparse HashMap, making it memory-efficient
/// for structures with many empty spaces.
///
/// # Coordinate System
///
/// The hex grid uses "pointy-top" orientation with axial coordinates:
/// - q: column (increases to the right)
/// - r: row (increases down-right in a staggered pattern)
/// - level: vertical stack index (0 = ground)
///
/// For flat-topped hexagons (used here), the conversion to world space uses:
/// - x = radius * (3/2 * q)
/// - y = level * height
/// - z = radius * (sqrt(3) * (r + q/2))
#[derive(Clone, Debug)]
pub struct HexPrismGrid {
    /// Sparse storage of hex-prisms indexed by (q, r, level)
    prisms: HashMap<(i32, i32, i32), HexPrism>,
    /// Default radius for new prisms
    pub default_radius: f32,
    /// Default height for new prisms
    pub default_height: f32,
}

impl Default for HexPrismGrid {
    fn default() -> Self {
        Self::new(0.5, 0.3)
    }
}

impl HexPrismGrid {
    /// Creates a new empty hex-prism grid with specified default dimensions.
    ///
    /// # Arguments
    ///
    /// * `default_radius` - Default circumradius for hex-prisms (center to vertex)
    /// * `default_height` - Default height for hex-prisms
    pub fn new(default_radius: f32, default_height: f32) -> Self {
        Self {
            prisms: HashMap::new(),
            default_radius,
            default_height,
        }
    }

    /// Inserts a hex-prism at the specified axial coordinates and level.
    ///
    /// Creates a new hex-prism with the grid's default radius and height,
    /// positions it in world space, and stores it in the grid.
    ///
    /// # Arguments
    ///
    /// * `q` - Axial q coordinate (column)
    /// * `r` - Axial r coordinate (row)
    /// * `level` - Vertical stack level (0 = ground)
    /// * `material` - Material identifier for the prism
    ///
    /// # Returns
    ///
    /// The previous prism at this location, if any.
    pub fn insert(&mut self, q: i32, r: i32, level: i32, material: u8) -> Option<HexPrism> {
        let center = axial_to_world(q, r, level, self.default_radius, self.default_height);
        let prism = HexPrism::new(center, self.default_height, self.default_radius, material);
        self.prisms.insert((q, r, level), prism)
    }

    /// Retrieves a reference to the hex-prism at the specified coordinates.
    ///
    /// # Returns
    ///
    /// `Some(&HexPrism)` if a prism exists at the location, `None` otherwise.
    pub fn get(&self, q: i32, r: i32, level: i32) -> Option<&HexPrism> {
        self.prisms.get(&(q, r, level))
    }

    /// Retrieves a mutable reference to the hex-prism at the specified coordinates.
    ///
    /// # Returns
    ///
    /// `Some(&mut HexPrism)` if a prism exists at the location, `None` otherwise.
    pub fn get_mut(&mut self, q: i32, r: i32, level: i32) -> Option<&mut HexPrism> {
        self.prisms.get_mut(&(q, r, level))
    }

    /// Removes and returns the hex-prism at the specified coordinates.
    ///
    /// # Returns
    ///
    /// The removed prism if it existed, `None` otherwise.
    pub fn remove(&mut self, q: i32, r: i32, level: i32) -> Option<HexPrism> {
        self.prisms.remove(&(q, r, level))
    }

    /// Returns the number of hex-prisms in the grid.
    pub fn len(&self) -> usize {
        self.prisms.len()
    }

    /// Returns `true` if the grid contains no hex-prisms.
    pub fn is_empty(&self) -> bool {
        self.prisms.is_empty()
    }

    /// Returns an iterator over all (coordinates, prism) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&(i32, i32, i32), &HexPrism)> {
        self.prisms.iter()
    }

    /// Returns an iterator over all prisms.
    pub fn prisms(&self) -> impl Iterator<Item = &HexPrism> {
        self.prisms.values()
    }

    /// Clears all prisms from the grid.
    pub fn clear(&mut self) {
        self.prisms.clear();
    }

    /// Checks if a prism exists at the specified coordinates.
    pub fn contains(&self, q: i32, r: i32, level: i32) -> bool {
        self.prisms.contains_key(&(q, r, level))
    }
}

/// Converts axial hex coordinates to world space position.
///
/// Uses the "flat-topped" hexagon orientation where:
/// - x axis goes right
/// - y axis goes up (stacking direction)
/// - z axis goes forward (into the screen)
///
/// # Arguments
///
/// * `q` - Axial q coordinate (column)
/// * `r` - Axial r coordinate (row)
/// * `level` - Vertical stack level (0 = ground)
///
/// # Returns
///
/// World space `Vec3` position for the center of the hex-prism.
pub fn axial_to_world(q: i32, r: i32, level: i32, radius: f32, height: f32) -> Vec3 {
    // Flat-topped hexagon layout:
    // x = radius * 3/2 * q
    // z = radius * sqrt(3) * (r + q/2)
    // y = level * height (centered in the prism)
    let sqrt3 = 3.0_f32.sqrt();

    let x = radius * 1.5 * q as f32;
    let z = radius * sqrt3 * (r as f32 + q as f32 / 2.0);
    let y = level as f32 * height + height / 2.0; // Center of the prism vertically

    Vec3::new(x, y, z)
}

/// Converts world space position to the nearest axial coordinates.
///
/// This is useful for determining which hex cell a world position falls into.
///
/// # Arguments
///
/// * `world_pos` - World space position
/// * `radius` - Hex circumradius
/// * `height` - Hex prism height
///
/// # Returns
///
/// Tuple of (q, r, level) representing the nearest hex-prism coordinates.
pub fn world_to_axial(world_pos: Vec3, radius: f32, height: f32) -> (i32, i32, i32) {
    let sqrt3 = 3.0_f32.sqrt();

    // Inverse of the axial_to_world formulas
    let q_float = world_pos.x / (radius * 1.5);
    let r_float = (world_pos.z / (radius * sqrt3)) - q_float / 2.0;
    let level_float = (world_pos.y - height / 2.0) / height;

    // Round to nearest hex using cube coordinate rounding
    // Convert to cube coordinates for proper rounding
    let x = q_float;
    let z = r_float;
    let y = -x - z;

    let mut rx = x.round();
    let ry = y.round();
    let mut rz = z.round();

    let x_diff = (rx - x).abs();
    let y_diff = (ry - y).abs();
    let z_diff = (rz - z).abs();

    // Cube coordinate rounding: reset the component with largest rounding error
    if x_diff > y_diff && x_diff > z_diff {
        rx = -ry - rz;
    } else if y_diff <= z_diff {
        rz = -rx - ry;
    }
    // Note: if y_diff > z_diff, ry would be recalculated but we don't use it

    let q = rx as i32;
    let r = rz as i32;
    let level = level_float.round() as i32;

    (q, r, level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_prism_creation() {
        let prism = HexPrism::new(Vec3::new(1.0, 2.0, 3.0), 0.5, 0.3, 1);
        assert_eq!(prism.center, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(prism.height, 0.5);
        assert_eq!(prism.radius, 0.3);
        assert_eq!(prism.material, 1);
    }

    #[test]
    fn test_grid_creation() {
        let grid = HexPrismGrid::new(0.5, 0.3);
        assert!(grid.is_empty());
        assert_eq!(grid.len(), 0);
        assert_eq!(grid.default_radius, 0.5);
        assert_eq!(grid.default_height, 0.3);
    }

    #[test]
    fn test_grid_insert_and_get() {
        let mut grid = HexPrismGrid::new(0.5, 0.3);

        // Insert a prism
        let replaced = grid.insert(0, 0, 0, 1);
        assert!(replaced.is_none());
        assert_eq!(grid.len(), 1);

        // Get it back
        let prism = grid.get(0, 0, 0);
        assert!(prism.is_some());
        let prism = prism.unwrap();
        assert_eq!(prism.material, 1);
        assert_eq!(prism.radius, 0.5);
        assert_eq!(prism.height, 0.3);
    }

    #[test]
    fn test_grid_replace() {
        let mut grid = HexPrismGrid::new(0.5, 0.3);

        grid.insert(0, 0, 0, 1);
        let replaced = grid.insert(0, 0, 0, 2);

        assert!(replaced.is_some());
        assert_eq!(replaced.unwrap().material, 1);

        let prism = grid.get(0, 0, 0).unwrap();
        assert_eq!(prism.material, 2);
    }

    #[test]
    fn test_grid_remove() {
        let mut grid = HexPrismGrid::new(0.5, 0.3);

        grid.insert(0, 0, 0, 1);
        assert_eq!(grid.len(), 1);

        let removed = grid.remove(0, 0, 0);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().material, 1);
        assert!(grid.is_empty());

        // Remove non-existent
        let removed = grid.remove(1, 1, 1);
        assert!(removed.is_none());
    }

    #[test]
    fn test_grid_contains() {
        let mut grid = HexPrismGrid::new(0.5, 0.3);

        assert!(!grid.contains(0, 0, 0));
        grid.insert(0, 0, 0, 1);
        assert!(grid.contains(0, 0, 0));
        assert!(!grid.contains(1, 0, 0));
    }

    #[test]
    fn test_axial_to_world_origin() {
        let pos = axial_to_world(0, 0, 0, 1.0, 1.0);
        // Origin hex, level 0, centered at height/2
        assert_eq!(pos.x, 0.0);
        assert_eq!(pos.y, 0.5); // height/2
        assert_eq!(pos.z, 0.0);
    }

    #[test]
    fn test_axial_to_world_q_direction() {
        let pos = axial_to_world(1, 0, 0, 1.0, 1.0);
        // Moving in q direction: x = 1.5 * q * radius
        assert!((pos.x - 1.5).abs() < 0.001);
        assert!((pos.z - 3.0_f32.sqrt() / 2.0).abs() < 0.001);
    }

    #[test]
    fn test_axial_to_world_stacking() {
        let level0 = axial_to_world(0, 0, 0, 1.0, 0.5);
        let level1 = axial_to_world(0, 0, 1, 1.0, 0.5);
        let level2 = axial_to_world(0, 0, 2, 1.0, 0.5);

        // Each level should be height apart
        assert!((level1.y - level0.y - 0.5).abs() < 0.001);
        assert!((level2.y - level1.y - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_world_to_axial_roundtrip() {
        let radius = 0.5;
        let height = 0.3;

        // Test several coordinates
        for q in -2..=2 {
            for r in -2..=2 {
                for level in 0..=3 {
                    let world = axial_to_world(q, r, level, radius, height);
                    let (q2, r2, level2) = world_to_axial(world, radius, height);
                    assert_eq!((q, r, level), (q2, r2, level2), "Roundtrip failed for ({}, {}, {})", q, r, level);
                }
            }
        }
    }

    #[test]
    fn test_grid_iteration() {
        let mut grid = HexPrismGrid::new(0.5, 0.3);

        grid.insert(0, 0, 0, 1);
        grid.insert(1, 0, 0, 2);
        grid.insert(0, 1, 0, 3);

        // Count prisms
        assert_eq!(grid.prisms().count(), 3);

        // Sum materials
        let total_material: u8 = grid.prisms().map(|p| p.material).sum();
        assert_eq!(total_material, 6);
    }

    #[test]
    fn test_build_wall() {
        let mut grid = HexPrismGrid::new(0.1, 0.1); // Micro-voxels

        // Build a simple wall segment: 3 wide, 2 high
        for q in 0..3 {
            for level in 0..2 {
                grid.insert(q, 0, level, 1);
            }
        }

        assert_eq!(grid.len(), 6);

        // Verify all positions exist
        for q in 0..3 {
            for level in 0..2 {
                assert!(grid.contains(q, 0, level));
            }
        }
    }
}
