//! Dual Grid System - Oscar Stalberg's technique for organic building
//!
//! The dual grid approach defines types at CORNERS rather than cell centers,
//! which allows smooth transitions between materials and organic-looking
//! structures instead of blocky voxel aesthetics.
//!
//! Key concepts:
//! - Primary grid: 1 dm (0.1m) cells for block placement
//! - Dual grid: Offset by half a cell, types defined at corners
//! - Corner interpolation creates smooth transitions
//! - Multiple overlapping layers for complex structures

use glam::{IVec3, Vec3};
use std::collections::HashMap;

/// Block size in meters (1 dm = 0.1m)
pub const BLOCK_SIZE: f32 = 0.1;

/// Half block size for dual grid offset
pub const HALF_BLOCK: f32 = BLOCK_SIZE / 2.0;

/// Type of a grid corner (what material/empty)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CornerType {
    #[default]
    Empty,
    Wood,
    Stone,
    Iron,
    Thatch,
    Mortar,
    Earth,
}

impl CornerType {
    /// Is this corner solid (not empty)?
    pub fn is_solid(&self) -> bool {
        !matches!(self, CornerType::Empty)
    }

    /// Get material color for this corner type
    pub fn color(&self) -> Vec3 {
        match self {
            CornerType::Empty => Vec3::ZERO,
            CornerType::Wood => Vec3::new(0.55, 0.35, 0.15),
            CornerType::Stone => Vec3::new(0.5, 0.5, 0.5),
            CornerType::Iron => Vec3::new(0.3, 0.3, 0.35),
            CornerType::Thatch => Vec3::new(0.7, 0.65, 0.3),
            CornerType::Mortar => Vec3::new(0.85, 0.82, 0.75),
            CornerType::Earth => Vec3::new(0.4, 0.3, 0.2),
        }
    }
}

/// A corner in the dual grid
#[derive(Debug, Clone, Copy)]
pub struct GridCorner {
    /// Position in grid coordinates
    pub pos: IVec3,
    /// Type of this corner
    pub corner_type: CornerType,
    /// Deformation offset for organic look (Stalberg's squash/stretch)
    pub deform: Vec3,
}

impl GridCorner {
    pub fn new(pos: IVec3, corner_type: CornerType) -> Self {
        Self {
            pos,
            corner_type,
            deform: Vec3::ZERO,
        }
    }

    /// World position including deformation
    pub fn world_pos(&self) -> Vec3 {
        Vec3::new(
            self.pos.x as f32 * BLOCK_SIZE + HALF_BLOCK,
            self.pos.y as f32 * BLOCK_SIZE + HALF_BLOCK,
            self.pos.z as f32 * BLOCK_SIZE + HALF_BLOCK,
        ) + self.deform
    }
}

/// A cell in the primary grid (defined by 8 corners)
#[derive(Debug, Clone)]
pub struct GridCell {
    /// Position in grid coordinates
    pub pos: IVec3,
    /// The 8 corner types (indexed by [z*4 + y*2 + x])
    /// Corner 0: (0,0,0), Corner 1: (1,0,0), etc.
    pub corners: [CornerType; 8],
}

impl GridCell {
    /// Create a new cell with all empty corners
    pub fn empty(pos: IVec3) -> Self {
        Self {
            pos,
            corners: [CornerType::Empty; 8],
        }
    }

    /// Create a solid cell of a single type
    pub fn solid(pos: IVec3, corner_type: CornerType) -> Self {
        Self {
            pos,
            corners: [corner_type; 8],
        }
    }

    /// Is this cell completely empty?
    pub fn is_empty(&self) -> bool {
        self.corners.iter().all(|c| !c.is_solid())
    }

    /// Is this cell completely solid?
    pub fn is_solid(&self) -> bool {
        self.corners.iter().all(|c| c.is_solid())
    }

    /// Get corner index from local offset
    pub fn corner_index(dx: i32, dy: i32, dz: i32) -> usize {
        (dz.clamp(0, 1) * 4 + dy.clamp(0, 1) * 2 + dx.clamp(0, 1)) as usize
    }

    /// World position of cell center
    pub fn world_center(&self) -> Vec3 {
        Vec3::new(
            self.pos.x as f32 * BLOCK_SIZE + HALF_BLOCK,
            self.pos.y as f32 * BLOCK_SIZE + HALF_BLOCK,
            self.pos.z as f32 * BLOCK_SIZE + HALF_BLOCK,
        )
    }

    /// Get the dominant material in this cell
    pub fn dominant_type(&self) -> CornerType {
        let mut counts: HashMap<CornerType, u32> = HashMap::new();
        for corner in &self.corners {
            if corner.is_solid() {
                *counts.entry(*corner).or_insert(0) += 1;
            }
        }
        counts
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(t, _)| t)
            .unwrap_or(CornerType::Empty)
    }
}

/// The dual grid system for building
#[derive(Debug, Clone)]
pub struct DualGrid {
    /// Cells indexed by grid position
    cells: HashMap<IVec3, GridCell>,
    /// Corner deformations for organic look
    corner_deforms: HashMap<IVec3, Vec3>,
    /// Grid bounds (min corner)
    pub min_bound: IVec3,
    /// Grid bounds (max corner)
    pub max_bound: IVec3,
    /// Whether deformation is enabled
    pub organic_mode: bool,
    /// Deformation intensity (0-1)
    pub deform_intensity: f32,
}

impl Default for DualGrid {
    fn default() -> Self {
        Self::new()
    }
}

impl DualGrid {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
            corner_deforms: HashMap::new(),
            min_bound: IVec3::ZERO,
            max_bound: IVec3::ZERO,
            organic_mode: true,
            deform_intensity: 0.3,
        }
    }

    /// Convert world position to grid coordinates
    pub fn world_to_grid(&self, world_pos: Vec3) -> IVec3 {
        IVec3::new(
            (world_pos.x / BLOCK_SIZE).floor() as i32,
            (world_pos.y / BLOCK_SIZE).floor() as i32,
            (world_pos.z / BLOCK_SIZE).floor() as i32,
        )
    }

    /// Convert grid coordinates to world position (cell center)
    pub fn grid_to_world(&self, grid_pos: IVec3) -> Vec3 {
        Vec3::new(
            grid_pos.x as f32 * BLOCK_SIZE + HALF_BLOCK,
            grid_pos.y as f32 * BLOCK_SIZE + HALF_BLOCK,
            grid_pos.z as f32 * BLOCK_SIZE + HALF_BLOCK,
        )
    }

    /// Get or create a cell at position
    pub fn get_or_create_cell(&mut self, pos: IVec3) -> &mut GridCell {
        self.update_bounds(pos);
        self.cells.entry(pos).or_insert_with(|| GridCell::empty(pos))
    }

    /// Get cell at position (if exists)
    pub fn get_cell(&self, pos: IVec3) -> Option<&GridCell> {
        self.cells.get(&pos)
    }

    /// Set a cell to be fully solid with a material
    pub fn set_solid(&mut self, pos: IVec3, corner_type: CornerType) {
        self.update_bounds(pos);
        self.cells.insert(pos, GridCell::solid(pos, corner_type));
        self.update_deformations_around(pos);
    }

    /// Set a cell to empty
    pub fn clear_cell(&mut self, pos: IVec3) {
        self.cells.remove(&pos);
        self.update_deformations_around(pos);
    }

    /// Set a specific corner of a cell
    pub fn set_corner(&mut self, cell_pos: IVec3, corner_idx: usize, corner_type: CornerType) {
        let cell = self.get_or_create_cell(cell_pos);
        cell.corners[corner_idx] = corner_type;
        self.update_deformations_around(cell_pos);
    }

    /// Place a block at world position
    pub fn place_block_at_world(&mut self, world_pos: Vec3, corner_type: CornerType) {
        let grid_pos = self.world_to_grid(world_pos);
        self.set_solid(grid_pos, corner_type);
    }

    /// Remove block at world position
    pub fn remove_block_at_world(&mut self, world_pos: Vec3) {
        let grid_pos = self.world_to_grid(world_pos);
        self.clear_cell(grid_pos);
    }

    /// Update bounds when adding a cell
    fn update_bounds(&mut self, pos: IVec3) {
        self.min_bound = self.min_bound.min(pos);
        self.max_bound = self.max_bound.max(pos + IVec3::ONE);
    }

    /// Update deformations around a modified cell (for organic look)
    fn update_deformations_around(&mut self, pos: IVec3) {
        if !self.organic_mode {
            return;
        }

        // Update deformations for corners in a 3x3x3 neighborhood
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let corner_pos = pos + IVec3::new(dx, dy, dz);
                    self.update_corner_deformation(corner_pos);
                }
            }
        }
    }

    /// Calculate deformation for a corner based on neighbors
    fn update_corner_deformation(&mut self, corner_pos: IVec3) {
        // Use a hash function to generate pseudo-random but consistent deformation
        let hash = Self::hash_pos(corner_pos);
        let deform = Vec3::new(
            ((hash & 0xFF) as f32 / 255.0 - 0.5) * BLOCK_SIZE * self.deform_intensity,
            (((hash >> 8) & 0xFF) as f32 / 255.0 - 0.5) * BLOCK_SIZE * self.deform_intensity,
            (((hash >> 16) & 0xFF) as f32 / 255.0 - 0.5) * BLOCK_SIZE * self.deform_intensity,
        );

        if deform.length() > 0.001 {
            self.corner_deforms.insert(corner_pos, deform);
        } else {
            self.corner_deforms.remove(&corner_pos);
        }
    }

    /// Hash function for consistent pseudo-random deformation
    fn hash_pos(pos: IVec3) -> u32 {
        let mut h = (pos.x as u32).wrapping_mul(73856093);
        h ^= (pos.y as u32).wrapping_mul(19349663);
        h ^= (pos.z as u32).wrapping_mul(83492791);
        h
    }

    /// Get deformation at a corner
    pub fn get_deformation(&self, corner_pos: IVec3) -> Vec3 {
        self.corner_deforms.get(&corner_pos).copied().unwrap_or(Vec3::ZERO)
    }

    /// Get world position of a corner including deformation
    pub fn corner_world_pos(&self, corner_pos: IVec3) -> Vec3 {
        Vec3::new(
            corner_pos.x as f32 * BLOCK_SIZE,
            corner_pos.y as f32 * BLOCK_SIZE,
            corner_pos.z as f32 * BLOCK_SIZE,
        ) + self.get_deformation(corner_pos)
    }

    /// Check if a position has structural support
    pub fn has_support(&self, pos: IVec3) -> bool {
        // Check if there's a solid cell below
        if let Some(below) = self.get_cell(pos - IVec3::Y) {
            if below.is_solid() {
                return true;
            }
        }

        // Check if there are enough neighbors for lateral support
        let mut neighbor_count = 0;
        for offset in [
            IVec3::new(-1, 0, 0),
            IVec3::new(1, 0, 0),
            IVec3::new(0, 0, -1),
            IVec3::new(0, 0, 1),
        ] {
            if let Some(neighbor) = self.get_cell(pos + offset) {
                if neighbor.is_solid() {
                    neighbor_count += 1;
                }
            }
        }

        neighbor_count >= 2
    }

    /// Find all unsupported cells (for physics cascade)
    pub fn find_unsupported(&self) -> Vec<IVec3> {
        let mut unsupported = Vec::new();
        for (pos, cell) in &self.cells {
            if !cell.is_empty() && !self.has_support(*pos) {
                // Ground level cells are always supported
                if pos.y > 0 {
                    unsupported.push(*pos);
                }
            }
        }
        unsupported
    }

    /// Get all solid cells
    pub fn solid_cells(&self) -> impl Iterator<Item = (&IVec3, &GridCell)> {
        self.cells.iter().filter(|(_, cell)| !cell.is_empty())
    }

    /// Get cell count
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Clear all cells
    pub fn clear(&mut self) {
        self.cells.clear();
        self.corner_deforms.clear();
        self.min_bound = IVec3::ZERO;
        self.max_bound = IVec3::ZERO;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_to_grid() {
        let grid = DualGrid::new();

        // Test center of first cell
        let pos = Vec3::new(0.05, 0.05, 0.05);
        assert_eq!(grid.world_to_grid(pos), IVec3::ZERO);

        // Test second cell
        let pos = Vec3::new(0.15, 0.05, 0.05);
        assert_eq!(grid.world_to_grid(pos), IVec3::new(1, 0, 0));

        // Test negative
        let pos = Vec3::new(-0.05, 0.05, 0.05);
        assert_eq!(grid.world_to_grid(pos), IVec3::new(-1, 0, 0));
    }

    #[test]
    fn test_set_and_get_cell() {
        let mut grid = DualGrid::new();

        grid.set_solid(IVec3::ZERO, CornerType::Stone);

        let cell = grid.get_cell(IVec3::ZERO).unwrap();
        assert!(cell.is_solid());
        assert_eq!(cell.dominant_type(), CornerType::Stone);
    }

    #[test]
    fn test_support() {
        let mut grid = DualGrid::new();

        // Place ground block
        grid.set_solid(IVec3::new(0, 0, 0), CornerType::Stone);

        // Place block above - should be supported
        grid.set_solid(IVec3::new(0, 1, 0), CornerType::Stone);
        assert!(grid.has_support(IVec3::new(0, 1, 0)));

        // Place floating block - no support
        grid.set_solid(IVec3::new(5, 5, 5), CornerType::Wood);
        assert!(!grid.has_support(IVec3::new(5, 5, 5)));
    }
}
