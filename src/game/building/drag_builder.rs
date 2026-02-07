//! Drag Builder - Click-and-drag continuous building
//!
//! Hold left click to continuously place blocks as you drag.
//! The system automatically:
//! - Extrudes in the drag direction
//! - Snaps to grid
//! - Combines adjacent blocks for cleaner meshes
//! - Shows a preview of what will be built

use glam::{IVec3, Vec3};
use std::collections::HashSet;

use super::blocks::BuildingBlock;
use super::dual_grid::{BLOCK_SIZE, CornerType, DualGrid};

/// State of the drag builder
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragState {
    /// Not building
    Idle,
    /// Started drag, waiting for direction
    Started,
    /// Actively dragging in a direction
    Dragging,
    /// Just released - finalize build
    Released,
}

/// Direction of drag extrusion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragDirection {
    /// Building along X axis
    X,
    /// Building along Y axis (up/down)
    Y,
    /// Building along Z axis
    Z,
    /// Free-form (not locked to axis)
    Free,
}

/// Events emitted by the drag builder
#[derive(Debug, Clone)]
pub enum BuildEvent {
    /// Preview positions changed
    PreviewChanged(Vec<IVec3>),
    /// Blocks were placed
    BlocksPlaced(Vec<IVec3>, CornerType),
    /// Blocks were removed
    BlocksRemoved(Vec<IVec3>),
    /// Build canceled
    Canceled,
}

/// The drag builder system
#[derive(Debug, Clone)]
pub struct DragBuilder {
    /// Current state
    pub state: DragState,
    /// Starting grid position of drag
    start_pos: Option<IVec3>,
    /// Current grid position
    current_pos: Option<IVec3>,
    /// Last placed position (to avoid duplicates)
    last_placed: Option<IVec3>,
    /// Detected drag direction
    direction: DragDirection,
    /// All positions in current drag
    drag_positions: Vec<IVec3>,
    /// Preview positions (what will be built)
    preview_positions: HashSet<IVec3>,
    /// Current material
    pub material: CornerType,
    /// Is delete mode active?
    pub delete_mode: bool,
    /// Current block template (optional)
    pub block_template: Option<BuildingBlock>,
    /// Minimum drag distance to detect direction (in blocks)
    pub direction_threshold: i32,
    /// Lock to axis once direction detected
    pub axis_lock: bool,
}

impl Default for DragBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DragBuilder {
    pub fn new() -> Self {
        Self {
            state: DragState::Idle,
            start_pos: None,
            current_pos: None,
            last_placed: None,
            direction: DragDirection::Free,
            drag_positions: Vec::new(),
            preview_positions: HashSet::new(),
            material: CornerType::Stone,
            delete_mode: false,
            block_template: None,
            direction_threshold: 2,
            axis_lock: true,
        }
    }

    /// Start a drag at world position
    pub fn start_drag(&mut self, world_pos: Vec3) -> BuildEvent {
        let grid_pos = world_to_grid(world_pos);

        self.state = DragState::Started;
        self.start_pos = Some(grid_pos);
        self.current_pos = Some(grid_pos);
        self.last_placed = None;
        self.direction = DragDirection::Free;
        self.drag_positions.clear();
        self.preview_positions.clear();

        // Add initial position
        self.drag_positions.push(grid_pos);
        self.preview_positions.insert(grid_pos);

        BuildEvent::PreviewChanged(vec![grid_pos])
    }

    /// Update drag position (called while mouse is held)
    pub fn update_drag(&mut self, world_pos: Vec3) -> Option<BuildEvent> {
        if self.state == DragState::Idle {
            return None;
        }

        let grid_pos = world_to_grid(world_pos);
        let prev_pos = self.current_pos?;

        // Skip if same position
        if grid_pos == prev_pos {
            return None;
        }

        self.current_pos = Some(grid_pos);

        // Detect direction if not yet determined
        if self.state == DragState::Started {
            if let Some(start) = self.start_pos {
                let delta = grid_pos - start;
                let dist = delta.abs();

                if dist.max_element() >= self.direction_threshold {
                    self.direction = Self::detect_direction(delta);
                    self.state = DragState::Dragging;
                }
            }
        }

        // Get positions along the path
        let new_positions = self.get_path_positions(prev_pos, grid_pos);

        for pos in new_positions {
            if !self.preview_positions.contains(&pos) {
                self.drag_positions.push(pos);
                self.preview_positions.insert(pos);
            }
        }

        Some(BuildEvent::PreviewChanged(
            self.preview_positions.iter().copied().collect(),
        ))
    }

    /// End the drag - finalize building
    pub fn end_drag(&mut self, grid: &mut DualGrid) -> BuildEvent {
        if self.state == DragState::Idle {
            return BuildEvent::Canceled;
        }

        let positions: Vec<IVec3> = self.preview_positions.iter().copied().collect();

        if self.delete_mode {
            // Remove blocks
            for pos in &positions {
                grid.clear_cell(*pos);
            }
            self.reset();
            BuildEvent::BlocksRemoved(positions)
        } else {
            // Place blocks
            for pos in &positions {
                grid.set_solid(*pos, self.material);
            }
            let material = self.material;
            self.reset();
            BuildEvent::BlocksPlaced(positions, material)
        }
    }

    /// Cancel current drag
    pub fn cancel(&mut self) -> BuildEvent {
        self.reset();
        BuildEvent::Canceled
    }

    /// Reset builder state
    fn reset(&mut self) {
        self.state = DragState::Idle;
        self.start_pos = None;
        self.current_pos = None;
        self.last_placed = None;
        self.direction = DragDirection::Free;
        self.drag_positions.clear();
        self.preview_positions.clear();
    }

    /// Detect drag direction from delta
    fn detect_direction(delta: IVec3) -> DragDirection {
        let abs = delta.abs();
        if abs.x >= abs.y && abs.x >= abs.z {
            DragDirection::X
        } else if abs.y >= abs.x && abs.y >= abs.z {
            DragDirection::Y
        } else {
            DragDirection::Z
        }
    }

    /// Get all grid positions along a path (Bresenham-like)
    fn get_path_positions(&self, from: IVec3, to: IVec3) -> Vec<IVec3> {
        let mut positions = Vec::new();

        let delta = to - from;
        let steps = delta.abs().max_element();

        if steps == 0 {
            return vec![from];
        }

        // Apply axis lock if enabled
        let constrained_to = if self.axis_lock && self.state == DragState::Dragging {
            match self.direction {
                DragDirection::X => IVec3::new(to.x, from.y, from.z),
                DragDirection::Y => IVec3::new(from.x, to.y, from.z),
                DragDirection::Z => IVec3::new(from.x, from.y, to.z),
                DragDirection::Free => to,
            }
        } else {
            to
        };

        let step_delta = (constrained_to - from).as_vec3() / steps as f32;

        for i in 0..=steps {
            let pos = from.as_vec3() + step_delta * i as f32;
            let grid_pos = IVec3::new(
                pos.x.round() as i32,
                pos.y.round() as i32,
                pos.z.round() as i32,
            );
            if positions.last() != Some(&grid_pos) {
                positions.push(grid_pos);
            }
        }

        positions
    }

    /// Get current preview positions
    pub fn preview(&self) -> &HashSet<IVec3> {
        &self.preview_positions
    }

    /// Is currently building?
    pub fn is_active(&self) -> bool {
        self.state != DragState::Idle
    }

    /// Set material
    pub fn set_material(&mut self, material: CornerType) {
        self.material = material;
    }

    /// Toggle delete mode
    pub fn toggle_delete(&mut self) {
        self.delete_mode = !self.delete_mode;
    }

    /// Get current direction (for UI)
    pub fn get_direction(&self) -> DragDirection {
        self.direction
    }
}

/// Convert world position to grid coordinates
pub fn world_to_grid(world_pos: Vec3) -> IVec3 {
    IVec3::new(
        (world_pos.x / BLOCK_SIZE).floor() as i32,
        (world_pos.y / BLOCK_SIZE).floor() as i32,
        (world_pos.z / BLOCK_SIZE).floor() as i32,
    )
}

/// Convert grid coordinates to world position (cell center)
pub fn grid_to_world(grid_pos: IVec3) -> Vec3 {
    Vec3::new(
        grid_pos.x as f32 * BLOCK_SIZE + BLOCK_SIZE / 2.0,
        grid_pos.y as f32 * BLOCK_SIZE + BLOCK_SIZE / 2.0,
        grid_pos.z as f32 * BLOCK_SIZE + BLOCK_SIZE / 2.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_drag() {
        let mut builder = DragBuilder::new();

        let event = builder.start_drag(Vec3::new(0.5, 0.5, 0.5));

        assert_eq!(builder.state, DragState::Started);
        assert!(builder.preview_positions.contains(&IVec3::new(5, 5, 5)));

        match event {
            BuildEvent::PreviewChanged(positions) => {
                assert_eq!(positions.len(), 1);
            }
            _ => panic!("Expected PreviewChanged event"),
        }
    }

    #[test]
    fn test_drag_path() {
        let mut builder = DragBuilder::new();
        builder.axis_lock = false; // Disable for this test

        builder.start_drag(Vec3::ZERO);

        // Drag to (0.3, 0, 0) - 3 blocks
        let event = builder.update_drag(Vec3::new(0.3, 0.0, 0.0));

        assert!(event.is_some());
        assert!(builder.preview_positions.len() >= 3);
    }

    #[test]
    fn test_end_drag() {
        let mut builder = DragBuilder::new();
        let mut grid = DualGrid::new();

        builder.start_drag(Vec3::ZERO);
        builder.update_drag(Vec3::new(0.2, 0.0, 0.0));

        let event = builder.end_drag(&mut grid);

        match event {
            BuildEvent::BlocksPlaced(positions, material) => {
                assert!(!positions.is_empty());
                assert_eq!(material, CornerType::Stone);
            }
            _ => panic!("Expected BlocksPlaced event"),
        }

        // Check blocks were placed in grid
        assert!(grid.cell_count() > 0);
    }

    #[test]
    fn test_direction_detection() {
        // X direction
        let delta = IVec3::new(5, 1, 2);
        assert_eq!(DragBuilder::detect_direction(delta), DragDirection::X);

        // Y direction
        let delta = IVec3::new(1, 5, 2);
        assert_eq!(DragBuilder::detect_direction(delta), DragDirection::Y);

        // Z direction
        let delta = IVec3::new(1, 2, 5);
        assert_eq!(DragBuilder::detect_direction(delta), DragDirection::Z);
    }
}
