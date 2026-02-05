//! Hex-Prism Voxel Data Structures
//!
//! This module provides data structures for stackable hexagonal prism voxels
//! used for building walls and fortifications. Hex-prisms match the planet's
//! hex grid naturally and avoid the visible grid artifacts of cube voxels.
//!
//! # Coordinate System
//!
//! Uses axial coordinates (q, r) for the hex grid with level (vertical stacking).
//! - q: axial q-coordinate
//! - r: axial r-coordinate
//! - level: vertical stack level (0 = ground level)
//!
//! # Example
//!
//! ```ignore
//! use battle_tok_engine::render::hex_prism::{HexPrism, HexPrismGrid, axial_to_world};
//!
//! let mut grid = HexPrismGrid::new();
//!
//! // Insert a hex-prism at axial coords (0, 0) at ground level
//! let prism = HexPrism::new(0.5, 0.3, 1);  // height=0.5, radius=0.3, material=1
//! grid.insert(0, 0, 0, prism);
//!
//! // Query it back
//! if let Some(p) = grid.get(0, 0, 0) {
//!     println!("Found prism with height: {}", p.height);
//! }
//!
//! // Get world position for coordinates
//! let world_pos = axial_to_world(0, 0, 0);
//! ```

use bytemuck::{Pod, Zeroable};
use glam::Vec3;
use std::collections::HashMap;
use std::f32::consts::PI;

// Import collision functions for ray-prism intersection (US-015)
use crate::physics::collision::{HitInfo, aabb_surface_normal, ray_aabb_intersect};

/// Default hex-prism radius for micro-voxels (0.1-0.5 units appear smooth)
pub const DEFAULT_HEX_RADIUS: f32 = 0.3;

/// Default hex-prism height for stackable voxels
pub const DEFAULT_HEX_HEIGHT: f32 = 0.5;

/// Horizontal spacing multiplier for hex grid (sqrt(3))
pub const HEX_HORIZONTAL_SPACING: f32 = 1.732_050_8; // sqrt(3)

/// A stackable hexagonal prism voxel.
///
/// Hex-prisms are the fundamental building blocks for walls and fortifications.
/// They stack vertically and tile horizontally in a hex grid pattern.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HexPrism {
    /// Center position in world space (computed from grid coordinates)
    pub center: Vec3,
    /// Height of the prism along the Y axis (in meters, SI units)
    pub height: f32,
    /// Radius of the hexagonal base (distance from center to vertex)
    pub radius: f32,
    /// Material type identifier
    pub material: u8,
}

impl HexPrism {
    /// Creates a new HexPrism with default center at origin.
    ///
    /// The center will be updated when inserted into a HexPrismGrid.
    ///
    /// # Arguments
    ///
    /// * `height` - Height of the prism (Y axis)
    /// * `radius` - Radius of the hexagonal base
    /// * `material` - Material type identifier
    pub fn new(height: f32, radius: f32, material: u8) -> Self {
        Self {
            center: Vec3::ZERO,
            height,
            radius,
            material,
        }
    }

    /// Creates a new HexPrism with explicit center position.
    ///
    /// # Arguments
    ///
    /// * `center` - World-space center position
    /// * `height` - Height of the prism (Y axis)
    /// * `radius` - Radius of the hexagonal base
    /// * `material` - Material type identifier
    pub fn with_center(center: Vec3, height: f32, radius: f32, material: u8) -> Self {
        Self {
            center,
            height,
            radius,
            material,
        }
    }

    /// Creates a default-sized HexPrism at the origin.
    ///
    /// Uses DEFAULT_HEX_HEIGHT and DEFAULT_HEX_RADIUS.
    pub fn default_at_origin(material: u8) -> Self {
        Self::new(DEFAULT_HEX_HEIGHT, DEFAULT_HEX_RADIUS, material)
    }

    /// Returns the axis-aligned bounding box (AABB) for this hex-prism.
    ///
    /// The AABB is computed as the smallest box that completely contains the hex prism.
    /// For a hexagon with radius r, the circumscribed box has half-width = r (vertex distance)
    /// and the apothem (edge distance) is r * cos(30°) = r * sqrt(3)/2.
    ///
    /// # Returns
    ///
    /// Tuple of (min_corner, max_corner) for the bounding box.
    pub fn get_aabb(&self) -> (Vec3, Vec3) {
        let half_height = self.height / 2.0;
        // For a regular hexagon, the bounding box extends by the radius in X and Z
        // (pointy-top orientation: vertices at top/bottom, so X extends by radius)
        let min = Vec3::new(
            self.center.x - self.radius,
            self.center.y - half_height,
            self.center.z - self.radius,
        );
        let max = Vec3::new(
            self.center.x + self.radius,
            self.center.y + half_height,
            self.center.z + self.radius,
        );
        (min, max)
    }
}

impl Default for HexPrism {
    fn default() -> Self {
        Self {
            center: Vec3::ZERO,
            height: DEFAULT_HEX_HEIGHT,
            radius: DEFAULT_HEX_RADIUS,
            material: 0,
        }
    }
}

/// A grid of hex-prism voxels using axial coordinates.
///
/// The grid uses a HashMap with (q, r, level) keys for efficient sparse storage.
/// This is ideal for building structures where most of space is empty.
///
/// # Coordinate System
///
/// - q, r: Axial hex coordinates (pointy-top orientation)
/// - level: Vertical stack level (0 = ground, positive = upward)
///
/// # Mesh Regeneration
///
/// The grid tracks a `mesh_dirty` flag that is set whenever prisms are added or removed.
/// Call `needs_mesh_update()` to check if the mesh should be regenerated, and
/// `clear_mesh_dirty()` after regenerating.
#[derive(Debug, Clone)]
pub struct HexPrismGrid {
    /// Sparse storage of hex-prisms keyed by (q, r, level)
    prisms: HashMap<(i32, i32, i32), HexPrism>,
    /// Flag indicating the mesh needs to be regenerated
    mesh_dirty: bool,
}

impl Default for HexPrismGrid {
    fn default() -> Self {
        Self {
            prisms: HashMap::new(),
            mesh_dirty: false,
        }
    }
}

impl HexPrismGrid {
    /// Creates a new empty HexPrismGrid.
    pub fn new() -> Self {
        Self {
            prisms: HashMap::new(),
            mesh_dirty: false,
        }
    }

    /// Creates a new HexPrismGrid with pre-allocated capacity.
    ///
    /// # Arguments
    ///
    /// * `capacity` - Expected number of hex-prisms to store
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            prisms: HashMap::with_capacity(capacity),
            mesh_dirty: false,
        }
    }

    /// Inserts a hex-prism at the specified axial coordinates and level.
    ///
    /// The prism's center will be automatically computed from the coordinates.
    /// Sets the mesh_dirty flag to indicate the mesh needs regeneration.
    /// Returns the previous prism at this location if one existed.
    ///
    /// # Arguments
    ///
    /// * `q` - Axial q-coordinate
    /// * `r` - Axial r-coordinate
    /// * `level` - Vertical stack level
    /// * `prism` - The hex-prism to insert
    pub fn insert(&mut self, q: i32, r: i32, level: i32, mut prism: HexPrism) -> Option<HexPrism> {
        // Compute world-space center from axial coordinates
        prism.center = axial_to_world(q, r, level);
        self.mesh_dirty = true;
        self.prisms.insert((q, r, level), prism)
    }

    /// Gets a reference to the hex-prism at the specified coordinates.
    ///
    /// Returns None if no prism exists at this location.
    ///
    /// # Arguments
    ///
    /// * `q` - Axial q-coordinate
    /// * `r` - Axial r-coordinate
    /// * `level` - Vertical stack level
    pub fn get(&self, q: i32, r: i32, level: i32) -> Option<&HexPrism> {
        self.prisms.get(&(q, r, level))
    }

    /// Gets a mutable reference to the hex-prism at the specified coordinates.
    ///
    /// Returns None if no prism exists at this location.
    ///
    /// # Arguments
    ///
    /// * `q` - Axial q-coordinate
    /// * `r` - Axial r-coordinate
    /// * `level` - Vertical stack level
    pub fn get_mut(&mut self, q: i32, r: i32, level: i32) -> Option<&mut HexPrism> {
        self.prisms.get_mut(&(q, r, level))
    }

    /// Removes and returns the hex-prism at the specified coordinates.
    ///
    /// Sets the mesh_dirty flag if a prism was actually removed.
    /// Returns None if no prism existed at this location.
    ///
    /// # Arguments
    ///
    /// * `q` - Axial q-coordinate
    /// * `r` - Axial r-coordinate
    /// * `level` - Vertical stack level
    pub fn remove(&mut self, q: i32, r: i32, level: i32) -> Option<HexPrism> {
        let removed = self.prisms.remove(&(q, r, level));
        if removed.is_some() {
            self.mesh_dirty = true;
        }
        removed
    }

    /// Removes a hex-prism using coordinates from HitInfo.
    ///
    /// Convenience method for destruction system. Sets mesh_dirty flag.
    ///
    /// # Arguments
    ///
    /// * `coord` - Tuple of (q, r, level) from HitInfo.prism_coord
    pub fn remove_by_coord(&mut self, coord: (i32, i32, i32)) -> Option<HexPrism> {
        self.remove(coord.0, coord.1, coord.2)
    }

    /// Returns the number of hex-prisms in the grid.
    pub fn len(&self) -> usize {
        self.prisms.len()
    }

    /// Returns true if the grid contains no hex-prisms.
    pub fn is_empty(&self) -> bool {
        self.prisms.is_empty()
    }

    /// Checks if a hex-prism exists at the specified coordinates.
    ///
    /// # Arguments
    ///
    /// * `q` - Axial q-coordinate
    /// * `r` - Axial r-coordinate
    /// * `level` - Vertical stack level
    pub fn contains(&self, q: i32, r: i32, level: i32) -> bool {
        self.prisms.contains_key(&(q, r, level))
    }

    /// Returns an iterator over all (coordinates, prism) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (&(i32, i32, i32), &HexPrism)> {
        self.prisms.iter()
    }

    /// Returns an iterator over all hex-prisms.
    pub fn prisms(&self) -> impl Iterator<Item = &HexPrism> {
        self.prisms.values()
    }

    /// Clears all hex-prisms from the grid.
    pub fn clear(&mut self) {
        self.prisms.clear();
    }

    /// Returns true if mesh needs to be regenerated.
    pub fn needs_mesh_update(&self) -> bool {
        self.mesh_dirty
    }

    /// Clears the mesh dirty flag after regenerating.
    pub fn clear_mesh_dirty(&mut self) {
        self.mesh_dirty = false;
    }

    /// Casts a ray against all hex-prisms in the grid and returns the closest hit.
    ///
    /// Uses AABB collision for fast rejection. For Phase 1, this is brute-force.
    /// Future phases may add spatial partitioning for large grids.
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
                // Only consider hits in front of ray and within max distance
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

    /// Checks if any hex-prism in the grid is hit by a ray.
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

/// Converts axial hex coordinates to world-space position.
///
/// Uses pointy-top hex orientation with the following layout:
/// - X axis: horizontal (positive = right)
/// - Y axis: vertical (positive = up)
/// - Z axis: depth (positive = forward)
///
/// The conversion uses standard hex-grid spacing:
/// - Horizontal spacing: radius * sqrt(3)
/// - Vertical spacing: radius * 1.5 (for pointy-top)
/// - Level spacing: height of prism
///
/// # Arguments
///
/// * `q` - Axial q-coordinate
/// * `r` - Axial r-coordinate
/// * `level` - Vertical stack level (0 = ground)
///
/// # Returns
///
/// World-space position as Vec3 (center of the hex-prism)
pub fn axial_to_world(q: i32, r: i32, level: i32) -> Vec3 {
    let q_f = q as f32;
    let r_f = r as f32;
    let level_f = level as f32;

    // Pointy-top hex grid conversion to world coordinates
    // X = radius * sqrt(3) * (q + r/2)
    // Z = radius * 3/2 * r
    // Y = level * height
    let x = DEFAULT_HEX_RADIUS * HEX_HORIZONTAL_SPACING * (q_f + r_f / 2.0);
    let z = DEFAULT_HEX_RADIUS * 1.5 * r_f;
    let y = level_f * DEFAULT_HEX_HEIGHT + DEFAULT_HEX_HEIGHT / 2.0; // Center of prism

    Vec3::new(x, y, z)
}

/// Converts world-space position to the nearest axial coordinates.
///
/// This is the inverse of `axial_to_world`. Note that due to floating-point
/// precision, round-tripping may not be exact.
///
/// # Arguments
///
/// * `pos` - World-space position
///
/// # Returns
///
/// Tuple of (q, r, level) axial coordinates
pub fn world_to_axial(pos: Vec3) -> (i32, i32, i32) {
    // Inverse of the axial_to_world conversion
    let r_f = pos.z / (DEFAULT_HEX_RADIUS * 1.5);
    let q_f = pos.x / (DEFAULT_HEX_RADIUS * HEX_HORIZONTAL_SPACING) - r_f / 2.0;
    let level_f = (pos.y - DEFAULT_HEX_HEIGHT / 2.0) / DEFAULT_HEX_HEIGHT;

    // Use cube coordinate rounding for accurate hex cell selection
    let (q, r) = axial_round(q_f, r_f);
    let level = level_f.round() as i32;

    (q, r, level)
}

/// Rounds fractional axial coordinates to the nearest hex cell.
///
/// Uses cube coordinate rounding for accuracy.
fn axial_round(q: f32, r: f32) -> (i32, i32) {
    // Convert to cube coordinates
    let s = -q - r;

    // Round each coordinate
    let mut rq = q.round();
    let mut rr = r.round();
    let rs = s.round();

    // Find the coordinate with the largest rounding error and fix it
    let q_diff = (rq - q).abs();
    let r_diff = (rr - r).abs();
    let s_diff = (rs - s).abs();

    if q_diff > r_diff && q_diff > s_diff {
        rq = -rr - rs;
    } else if r_diff > s_diff {
        rr = -rq - rs;
    }

    (rq as i32, rr as i32)
}

// ============================================================================
// MESH GENERATION
// ============================================================================

/// Vertex format for hex-prism meshes.
///
/// Matches the VertexInput struct in hex_prism.wgsl:
/// - position: vec3<f32> at @location(0)
/// - normal: vec3<f32> at @location(1)
/// - color: vec4<f32> at @location(2)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct HexPrismVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

impl HexPrismVertex {
    /// Creates a new vertex with the given position, normal, and color.
    pub fn new(position: Vec3, normal: Vec3, color: [f32; 4]) -> Self {
        Self {
            position: position.to_array(),
            normal: normal.to_array(),
            color,
        }
    }
}

/// Material preset colors for hex-prism walls.
pub mod materials {
    /// Stone gray - weathered fortress walls
    pub const STONE_GRAY: [f32; 4] = [0.45, 0.42, 0.40, 1.0];
    /// Stone gray (light) - newer/cleaner stone
    pub const STONE_LIGHT: [f32; 4] = [0.55, 0.52, 0.50, 1.0];
    /// Stone gray (dark) - aged/mossy stone
    pub const STONE_DARK: [f32; 4] = [0.32, 0.30, 0.28, 1.0];
    /// Wood brown - wooden palisades
    pub const WOOD_BROWN: [f32; 4] = [0.45, 0.32, 0.20, 1.0];
    /// Wood brown (light) - fresh lumber
    pub const WOOD_LIGHT: [f32; 4] = [0.55, 0.40, 0.25, 1.0];
    /// Wood brown (dark) - weathered wood
    pub const WOOD_DARK: [f32; 4] = [0.30, 0.22, 0.15, 1.0];
    /// Metal (iron) - reinforced sections
    pub const METAL_IRON: [f32; 4] = [0.35, 0.35, 0.38, 1.0];
    /// Metal (bronze) - decorative elements
    pub const METAL_BRONZE: [f32; 4] = [0.55, 0.42, 0.25, 1.0];

    /// Returns a color for a given material ID.
    pub fn color_for_material(material: u8) -> [f32; 4] {
        match material {
            0 => STONE_GRAY,
            1 => STONE_LIGHT,
            2 => STONE_DARK,
            3 => WOOD_BROWN,
            4 => WOOD_LIGHT,
            5 => WOOD_DARK,
            6 => METAL_IRON,
            7 => METAL_BRONZE,
            _ => STONE_GRAY,
        }
    }
}

impl HexPrism {
    /// Generates a triangle mesh for this hex-prism.
    ///
    /// Creates a hexagonal prism with:
    /// - 6 vertices on top, 6 on bottom (12 total for sides)
    /// - 1 center vertex for top fan, 1 for bottom fan
    /// - Side faces: 6 quads (12 triangles)
    /// - Top face: 6 triangles (fan from center)
    /// - Bottom face: 6 triangles (fan from center)
    ///
    /// Total: 14 vertices, 24 triangles (72 indices)
    ///
    /// # Returns
    ///
    /// Tuple of (vertices, indices) for the mesh.
    pub fn generate_mesh(&self) -> (Vec<HexPrismVertex>, Vec<u32>) {
        let mut vertices = Vec::with_capacity(14);
        let mut indices = Vec::with_capacity(72);

        let color = materials::color_for_material(self.material);
        let half_height = self.height / 2.0;
        let center = self.center;

        // Generate 6 hex vertices around the center at top and bottom
        // Pointy-top hex: first vertex at +Z direction
        let mut hex_offsets = [[0.0_f32; 2]; 6];
        for i in 0..6 {
            let angle = (i as f32) * PI / 3.0; // 60 degrees apart, starting at 0
            hex_offsets[i] = [
                self.radius * angle.sin(), // X
                self.radius * angle.cos(), // Z
            ];
        }

        // Normals
        let up = Vec3::Y;
        let down = Vec3::NEG_Y;

        // ========================================
        // TOP FACE (6 triangles from center)
        // ========================================
        let top_center_idx = vertices.len() as u32;
        let top_y = center.y + half_height;
        vertices.push(HexPrismVertex::new(
            Vec3::new(center.x, top_y, center.z),
            up,
            color,
        ));

        // 6 top edge vertices
        for i in 0..6 {
            let [ox, oz] = hex_offsets[i];
            vertices.push(HexPrismVertex::new(
                Vec3::new(center.x + ox, top_y, center.z + oz),
                up,
                color,
            ));
        }
        let top_first_edge = top_center_idx + 1;

        // Top face triangles (fan from center, CCW winding for upward normal)
        for i in 0..6 {
            let i1 = top_first_edge + i as u32;
            let i2 = top_first_edge + ((i + 1) % 6) as u32;
            indices.extend_from_slice(&[top_center_idx, i1, i2]);
        }

        // ========================================
        // BOTTOM FACE (6 triangles from center)
        // ========================================
        let bottom_center_idx = vertices.len() as u32;
        let bottom_y = center.y - half_height;
        vertices.push(HexPrismVertex::new(
            Vec3::new(center.x, bottom_y, center.z),
            down,
            color,
        ));

        // 6 bottom edge vertices
        for i in 0..6 {
            let [ox, oz] = hex_offsets[i];
            vertices.push(HexPrismVertex::new(
                Vec3::new(center.x + ox, bottom_y, center.z + oz),
                down,
                color,
            ));
        }
        let bottom_first_edge = bottom_center_idx + 1;

        // Bottom face triangles (fan from center, CW winding for downward normal)
        for i in 0..6 {
            let i1 = bottom_first_edge + i as u32;
            let i2 = bottom_first_edge + ((i + 1) % 6) as u32;
            indices.extend_from_slice(&[bottom_center_idx, i2, i1]);
        }

        // ========================================
        // SIDE FACES (6 quads = 12 triangles)
        // ========================================
        // Each side needs 4 vertices with outward-facing normals
        for i in 0..6 {
            let [ox1, oz1] = hex_offsets[i];
            let [ox2, oz2] = hex_offsets[(i + 1) % 6];

            // Compute outward normal for this side
            let mid_x = (ox1 + ox2) / 2.0;
            let mid_z = (oz1 + oz2) / 2.0;
            let side_normal = Vec3::new(mid_x, 0.0, mid_z).normalize();

            let side_base = vertices.len() as u32;

            // 4 vertices for this quad (top-left, top-right, bottom-right, bottom-left)
            vertices.push(HexPrismVertex::new(
                Vec3::new(center.x + ox1, top_y, center.z + oz1),
                side_normal,
                color,
            ));
            vertices.push(HexPrismVertex::new(
                Vec3::new(center.x + ox2, top_y, center.z + oz2),
                side_normal,
                color,
            ));
            vertices.push(HexPrismVertex::new(
                Vec3::new(center.x + ox2, bottom_y, center.z + oz2),
                side_normal,
                color,
            ));
            vertices.push(HexPrismVertex::new(
                Vec3::new(center.x + ox1, bottom_y, center.z + oz1),
                side_normal,
                color,
            ));

            // Two triangles for this quad (CCW winding)
            // Triangle 1: top-left, bottom-left, bottom-right
            indices.extend_from_slice(&[side_base, side_base + 3, side_base + 2]);
            // Triangle 2: top-left, bottom-right, top-right
            indices.extend_from_slice(&[side_base, side_base + 2, side_base + 1]);
        }

        (vertices, indices)
    }
}

impl HexPrismGrid {
    /// Generates a combined mesh for all hex-prisms in the grid.
    ///
    /// Merges all individual prism meshes into a single vertex/index buffer
    /// for efficient rendering with a single draw call.
    ///
    /// # Returns
    ///
    /// Tuple of (vertices, indices) for the combined mesh.
    /// Returns empty vectors if the grid is empty.
    pub fn generate_combined_mesh(&self) -> (Vec<HexPrismVertex>, Vec<u32>) {
        if self.is_empty() {
            return (Vec::new(), Vec::new());
        }

        // Pre-allocate based on expected size
        // Each prism: ~38 vertices (14 base + 24 for sides with separate normals), ~72 indices
        let prism_count = self.len();
        let mut all_vertices = Vec::with_capacity(prism_count * 38);
        let mut all_indices = Vec::with_capacity(prism_count * 72);

        // Merge all prism meshes
        for prism in self.prisms() {
            let (vertices, indices) = prism.generate_mesh();
            let base_index = all_vertices.len() as u32;

            all_vertices.extend(vertices);
            all_indices.extend(indices.iter().map(|i| i + base_index));
        }

        (all_vertices, all_indices)
    }

    /// Creates a simple wall from hex-prisms.
    ///
    /// Builds a wall of `width` prisms horizontally and `height` prisms vertically,
    /// starting at the given axial coordinate.
    ///
    /// # Arguments
    ///
    /// * `start_q` - Starting axial q-coordinate
    /// * `start_r` - Starting axial r-coordinate
    /// * `width` - Number of prisms wide
    /// * `height` - Number of prisms tall (vertical layers)
    /// * `material` - Material type for all prisms
    pub fn create_wall(
        &mut self,
        start_q: i32,
        start_r: i32,
        width: i32,
        height: i32,
        material: u8,
    ) {
        for q_offset in 0..width {
            for level in 0..height {
                let prism = HexPrism::new(DEFAULT_HEX_HEIGHT, DEFAULT_HEX_RADIUS, material);
                self.insert(start_q + q_offset, start_r, level, prism);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_prism_new() {
        let prism = HexPrism::new(1.0, 0.5, 2);
        assert_eq!(prism.height, 1.0);
        assert_eq!(prism.radius, 0.5);
        assert_eq!(prism.material, 2);
        assert_eq!(prism.center, Vec3::ZERO);
    }

    #[test]
    fn test_hex_prism_with_center() {
        let center = Vec3::new(1.0, 2.0, 3.0);
        let prism = HexPrism::with_center(center, 1.0, 0.5, 3);
        assert_eq!(prism.center, center);
    }

    #[test]
    fn test_hex_prism_grid_insert_get() {
        let mut grid = HexPrismGrid::new();
        let prism = HexPrism::new(0.5, 0.3, 1);

        // Insert and verify
        let prev = grid.insert(0, 0, 0, prism);
        assert!(prev.is_none());

        // Get and verify
        let retrieved = grid.get(0, 0, 0);
        assert!(retrieved.is_some());
        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.height, 0.5);
        assert_eq!(retrieved.radius, 0.3);
        assert_eq!(retrieved.material, 1);
    }

    #[test]
    fn test_hex_prism_grid_remove() {
        let mut grid = HexPrismGrid::new();
        grid.insert(1, 2, 3, HexPrism::new(1.0, 0.5, 1));

        assert!(grid.contains(1, 2, 3));
        assert_eq!(grid.len(), 1);

        let removed = grid.remove(1, 2, 3);
        assert!(removed.is_some());
        assert!(!grid.contains(1, 2, 3));
        assert!(grid.is_empty());
    }

    #[test]
    fn test_axial_to_world_origin() {
        let pos = axial_to_world(0, 0, 0);
        assert_eq!(pos.x, 0.0);
        assert_eq!(pos.z, 0.0);
        // Y should be at the center of the first prism
        assert!((pos.y - DEFAULT_HEX_HEIGHT / 2.0).abs() < 0.001);
    }

    #[test]
    fn test_axial_to_world_stacking() {
        let level0 = axial_to_world(0, 0, 0);
        let level1 = axial_to_world(0, 0, 1);
        let level2 = axial_to_world(0, 0, 2);

        // Each level should be DEFAULT_HEX_HEIGHT apart
        assert!((level1.y - level0.y - DEFAULT_HEX_HEIGHT).abs() < 0.001);
        assert!((level2.y - level1.y - DEFAULT_HEX_HEIGHT).abs() < 0.001);
    }

    #[test]
    fn test_world_to_axial_roundtrip() {
        // Test roundtrip conversion for various coordinates
        let test_coords = [(0, 0, 0), (1, 0, 0), (0, 1, 0), (1, 1, 1), (-1, 2, 3)];

        for (q, r, level) in test_coords {
            let world = axial_to_world(q, r, level);
            let (rq, rr, rlevel) = world_to_axial(world);
            assert_eq!(
                (q, r, level),
                (rq, rr, rlevel),
                "Roundtrip failed for ({}, {}, {})",
                q,
                r,
                level
            );
        }
    }

    #[test]
    fn test_grid_center_is_set_on_insert() {
        let mut grid = HexPrismGrid::new();
        let prism = HexPrism::new(0.5, 0.3, 1);

        grid.insert(2, 3, 1, prism);

        let retrieved = grid.get(2, 3, 1).unwrap();
        let expected_center = axial_to_world(2, 3, 1);
        assert_eq!(retrieved.center, expected_center);
    }

    #[test]
    fn test_hex_prism_generate_mesh() {
        let prism = HexPrism::with_center(Vec3::new(0.0, 0.5, 0.0), 1.0, 0.5, 0);
        let (vertices, indices) = prism.generate_mesh();

        // Check vertex count: 1 top center + 6 top edge + 1 bottom center + 6 bottom edge + 24 side (4 per quad × 6)
        assert_eq!(vertices.len(), 38);

        // Check index count: 6 top triangles + 6 bottom triangles + 12 side triangles = 24 triangles × 3
        assert_eq!(indices.len(), 72);

        // All indices should be valid
        for &idx in &indices {
            assert!((idx as usize) < vertices.len(), "Invalid index: {}", idx);
        }
    }

    #[test]
    fn test_hex_prism_grid_generate_combined_mesh() {
        let mut grid = HexPrismGrid::new();

        // Create a 2×2 wall
        grid.create_wall(0, 0, 2, 2, 0);
        assert_eq!(grid.len(), 4);

        let (vertices, indices) = grid.generate_combined_mesh();

        // Should have 4 prisms × 38 vertices each = 152 vertices
        assert_eq!(vertices.len(), 152);

        // Should have 4 prisms × 72 indices each = 288 indices
        assert_eq!(indices.len(), 288);

        // All indices should be valid
        for &idx in &indices {
            assert!((idx as usize) < vertices.len(), "Invalid index: {}", idx);
        }
    }

    #[test]
    fn test_empty_grid_combined_mesh() {
        let grid = HexPrismGrid::new();
        let (vertices, indices) = grid.generate_combined_mesh();

        assert!(vertices.is_empty());
        assert!(indices.is_empty());
    }

    #[test]
    fn test_hex_prism_vertex_size() {
        // Ensure vertex size is correct for GPU alignment
        // position: 12 bytes, normal: 12 bytes, color: 16 bytes = 40 bytes
        assert_eq!(std::mem::size_of::<HexPrismVertex>(), 40);
    }
}
