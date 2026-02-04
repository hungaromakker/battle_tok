//! Building Blocks - Reusable block templates
//!
//! Players can save combinations of blocks as templates
//! to reuse in later games.

use glam::{IVec3, Vec3};
use std::collections::HashMap;

use super::dual_grid::{CornerType, DualGrid, BLOCK_SIZE};

/// Shape of a building block
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockShape {
    /// Single 1 dm³ cube
    Cube,
    /// Wall segment (multiple cubes in a line)
    Wall,
    /// Floor/ceiling plane
    Floor,
    /// Ramp (angled)
    Ramp,
    /// Stairs (stepped)
    Stairs,
    /// Arch
    Arch,
    /// Column/pillar
    Column,
    /// Custom (defined by grid)
    Custom,
}

/// A single building block (can be single cube or compound)
#[derive(Debug, Clone)]
pub struct BuildingBlock {
    /// Unique identifier
    pub id: u32,
    /// Display name
    pub name: String,
    /// Shape type
    pub shape: BlockShape,
    /// Material type
    pub material: CornerType,
    /// Grid positions relative to anchor (0,0,0)
    pub cells: Vec<IVec3>,
    /// Anchor point for placement
    pub anchor: IVec3,
    /// Rotation (0, 90, 180, 270 degrees around Y)
    pub rotation: i32,
    /// Is this a player-saved template?
    pub is_template: bool,
}

impl BuildingBlock {
    /// Create a single cube block
    pub fn cube(material: CornerType) -> Self {
        Self {
            id: 0,
            name: String::from("Cube"),
            shape: BlockShape::Cube,
            material,
            cells: vec![IVec3::ZERO],
            anchor: IVec3::ZERO,
            rotation: 0,
            is_template: false,
        }
    }

    /// Create a wall segment (length x height x 1)
    pub fn wall(length: i32, height: i32, material: CornerType) -> Self {
        let mut cells = Vec::new();
        for x in 0..length {
            for y in 0..height {
                cells.push(IVec3::new(x, y, 0));
            }
        }
        Self {
            id: 0,
            name: format!("Wall {}x{}", length, height),
            shape: BlockShape::Wall,
            material,
            cells,
            anchor: IVec3::ZERO,
            rotation: 0,
            is_template: false,
        }
    }

    /// Create a floor segment (width x 1 x depth)
    pub fn floor(width: i32, depth: i32, material: CornerType) -> Self {
        let mut cells = Vec::new();
        for x in 0..width {
            for z in 0..depth {
                cells.push(IVec3::new(x, 0, z));
            }
        }
        Self {
            id: 0,
            name: format!("Floor {}x{}", width, depth),
            shape: BlockShape::Floor,
            material,
            cells,
            anchor: IVec3::ZERO,
            rotation: 0,
            is_template: false,
        }
    }

    /// Create a ramp (rises in X direction)
    pub fn ramp(length: i32, height: i32, material: CornerType) -> Self {
        let mut cells = Vec::new();
        let slope = height as f32 / length as f32;
        for x in 0..length {
            let y_max = ((x as f32 + 1.0) * slope).ceil() as i32;
            for y in 0..y_max {
                cells.push(IVec3::new(x, y, 0));
            }
        }
        Self {
            id: 0,
            name: format!("Ramp {}x{}", length, height),
            shape: BlockShape::Ramp,
            material,
            cells,
            anchor: IVec3::ZERO,
            rotation: 0,
            is_template: false,
        }
    }

    /// Create stairs (rises in X direction, step size = 1)
    pub fn stairs(count: i32, material: CornerType) -> Self {
        let mut cells = Vec::new();
        for step in 0..count {
            // Each step is 2 blocks wide and 1 block tall
            for x in 0..2 {
                for y in 0..=step {
                    cells.push(IVec3::new(step * 2 + x, y, 0));
                }
            }
        }
        Self {
            id: 0,
            name: format!("Stairs x{}", count),
            shape: BlockShape::Stairs,
            material,
            cells,
            anchor: IVec3::ZERO,
            rotation: 0,
            is_template: false,
        }
    }

    /// Create a column/pillar
    pub fn column(height: i32, material: CornerType) -> Self {
        let mut cells = Vec::new();
        for y in 0..height {
            cells.push(IVec3::new(0, y, 0));
        }
        Self {
            id: 0,
            name: format!("Column h{}", height),
            shape: BlockShape::Column,
            material,
            cells,
            anchor: IVec3::ZERO,
            rotation: 0,
            is_template: false,
        }
    }

    /// Get world positions for this block at a given placement position
    pub fn world_positions(&self, placement_pos: Vec3) -> Vec<Vec3> {
        let base_grid = IVec3::new(
            (placement_pos.x / BLOCK_SIZE).floor() as i32,
            (placement_pos.y / BLOCK_SIZE).floor() as i32,
            (placement_pos.z / BLOCK_SIZE).floor() as i32,
        );

        self.cells
            .iter()
            .map(|cell| {
                let rotated = self.rotate_cell(*cell);
                let grid_pos = base_grid + rotated - self.anchor;
                Vec3::new(
                    grid_pos.x as f32 * BLOCK_SIZE + BLOCK_SIZE / 2.0,
                    grid_pos.y as f32 * BLOCK_SIZE + BLOCK_SIZE / 2.0,
                    grid_pos.z as f32 * BLOCK_SIZE + BLOCK_SIZE / 2.0,
                )
            })
            .collect()
    }

    /// Get grid positions for this block at a given placement grid position
    pub fn grid_positions(&self, placement_grid: IVec3) -> Vec<IVec3> {
        self.cells
            .iter()
            .map(|cell| {
                let rotated = self.rotate_cell(*cell);
                placement_grid + rotated - self.anchor
            })
            .collect()
    }

    /// Rotate a cell position around Y axis based on current rotation
    fn rotate_cell(&self, cell: IVec3) -> IVec3 {
        match self.rotation {
            0 => cell,
            90 => IVec3::new(-cell.z, cell.y, cell.x),
            180 => IVec3::new(-cell.x, cell.y, -cell.z),
            270 => IVec3::new(cell.z, cell.y, -cell.x),
            _ => cell,
        }
    }

    /// Rotate block 90 degrees clockwise
    pub fn rotate_cw(&mut self) {
        self.rotation = (self.rotation + 90) % 360;
    }

    /// Rotate block 90 degrees counter-clockwise
    pub fn rotate_ccw(&mut self) {
        self.rotation = (self.rotation + 270) % 360;
    }

    /// Get bounding box size in blocks
    pub fn bounds(&self) -> IVec3 {
        let mut min = IVec3::MAX;
        let mut max = IVec3::MIN;
        for cell in &self.cells {
            let rotated = self.rotate_cell(*cell);
            min = min.min(rotated);
            max = max.max(rotated);
        }
        max - min + IVec3::ONE
    }
}

/// Library of saved block templates
#[derive(Debug, Clone, Default)]
pub struct BlockLibrary {
    /// Templates indexed by ID
    templates: HashMap<u32, BuildingBlock>,
    /// Next available ID
    next_id: u32,
    /// Categories for organization
    categories: HashMap<String, Vec<u32>>,
}

impl BlockLibrary {
    pub fn new() -> Self {
        let mut lib = Self::default();

        // Add default castle-building blocks
        lib.add_default_blocks();

        lib
    }

    /// Add default castle-building blocks
    fn add_default_blocks(&mut self) {
        // Basic walls
        self.add_template(BuildingBlock::wall(10, 10, CornerType::Stone), "Walls");
        self.add_template(BuildingBlock::wall(5, 10, CornerType::Stone), "Walls");
        self.add_template(BuildingBlock::wall(10, 5, CornerType::Stone), "Walls");

        // Wooden scaffolding
        self.add_template(BuildingBlock::wall(10, 5, CornerType::Wood), "Scaffolding");

        // Floors
        self.add_template(BuildingBlock::floor(10, 10, CornerType::Stone), "Floors");
        self.add_template(BuildingBlock::floor(5, 5, CornerType::Wood), "Floors");

        // Stairs and ramps
        self.add_template(BuildingBlock::stairs(5, CornerType::Stone), "Stairs");
        self.add_template(BuildingBlock::ramp(10, 5, CornerType::Stone), "Ramps");

        // Columns
        self.add_template(BuildingBlock::column(10, CornerType::Stone), "Columns");
        self.add_template(BuildingBlock::column(5, CornerType::Wood), "Columns");
    }

    /// Add a template to the library
    pub fn add_template(&mut self, mut block: BuildingBlock, category: &str) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        block.id = id;
        block.is_template = true;

        self.templates.insert(id, block);
        self.categories
            .entry(category.to_string())
            .or_default()
            .push(id);

        id
    }

    /// Get a template by ID
    pub fn get(&self, id: u32) -> Option<&BuildingBlock> {
        self.templates.get(&id)
    }

    /// Get all templates in a category
    pub fn get_category(&self, category: &str) -> Vec<&BuildingBlock> {
        self.categories
            .get(category)
            .map(|ids| ids.iter().filter_map(|id| self.templates.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get all category names
    pub fn categories(&self) -> Vec<&String> {
        self.categories.keys().collect()
    }

    /// Save a custom block as template
    pub fn save_custom(&mut self, cells: Vec<IVec3>, material: CornerType, name: String) -> u32 {
        let block = BuildingBlock {
            id: 0,
            name,
            shape: BlockShape::Custom,
            material,
            cells,
            anchor: IVec3::ZERO,
            rotation: 0,
            is_template: true,
        };
        self.add_template(block, "Custom")
    }

    /// Remove a custom template
    pub fn remove_template(&mut self, id: u32) -> bool {
        if let Some(block) = self.templates.remove(&id) {
            // Remove from categories
            for ids in self.categories.values_mut() {
                ids.retain(|&i| i != id);
            }
            // Only allow removing custom templates
            block.shape == BlockShape::Custom
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cube_block() {
        let block = BuildingBlock::cube(CornerType::Stone);
        assert_eq!(block.cells.len(), 1);
        assert_eq!(block.bounds(), IVec3::ONE);
    }

    #[test]
    fn test_wall_block() {
        let block = BuildingBlock::wall(3, 2, CornerType::Stone);
        assert_eq!(block.cells.len(), 6); // 3x2
        assert_eq!(block.bounds(), IVec3::new(3, 2, 1));
    }

    #[test]
    fn test_rotation() {
        let mut block = BuildingBlock::wall(3, 1, CornerType::Stone);
        assert_eq!(block.bounds(), IVec3::new(3, 1, 1));

        block.rotate_cw();
        assert_eq!(block.bounds(), IVec3::new(1, 1, 3));
    }

    #[test]
    fn test_library() {
        let lib = BlockLibrary::new();
        assert!(!lib.categories().is_empty());
        assert!(!lib.get_category("Walls").is_empty());
    }
}
