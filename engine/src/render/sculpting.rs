//! Sculpting System (Phase 4)
//!
//! Provides interactive sculpting tools for building blocks:
//! - Face extrusion: Select a face and drag to create new geometry
//! - Edge/vertex pulling: Drag edges or vertices to deform shapes
//!
//! # Extrusion Workflow
//!
//! 1. Select a face on a building block
//! 2. Drag outward to extrude
//! 3. Each drag step creates new connected geometry
//! 4. Release to auto-merge with SDF
//!
//! # Edge/Vertex Pulling
//!
//! 1. Select an edge or vertex
//! 2. Drag to deform the shape
//! 3. SDF deformation creates smooth transitions
//! 4. Bake to mesh when done editing

use glam::Vec3;

use super::building_blocks::{BuildingBlock, BuildingBlockManager, BuildingBlockShape};

// ============================================================================
// SELECTION TYPES
// ============================================================================

/// Type of element selected for sculpting
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SelectionType {
    /// No selection
    None,
    /// A face is selected
    Face(FaceSelection),
    /// An edge is selected
    Edge(EdgeSelection),
    /// A vertex is selected
    Vertex(VertexSelection),
}

impl Default for SelectionType {
    fn default() -> Self {
        Self::None
    }
}

/// Face direction for box-like shapes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceDirection {
    PositiveX,
    NegativeX,
    PositiveY,
    NegativeY,
    PositiveZ,
    NegativeZ,
}

impl FaceDirection {
    /// Get the normal vector for this face
    pub fn normal(&self) -> Vec3 {
        match self {
            Self::PositiveX => Vec3::X,
            Self::NegativeX => Vec3::NEG_X,
            Self::PositiveY => Vec3::Y,
            Self::NegativeY => Vec3::NEG_Y,
            Self::PositiveZ => Vec3::Z,
            Self::NegativeZ => Vec3::NEG_Z,
        }
    }

    /// Get all face directions
    pub fn all() -> [Self; 6] {
        [
            Self::PositiveX,
            Self::NegativeX,
            Self::PositiveY,
            Self::NegativeY,
            Self::PositiveZ,
            Self::NegativeZ,
        ]
    }

    /// Get the opposite face direction
    pub fn opposite(&self) -> Self {
        match self {
            Self::PositiveX => Self::NegativeX,
            Self::NegativeX => Self::PositiveX,
            Self::PositiveY => Self::NegativeY,
            Self::NegativeY => Self::PositiveY,
            Self::PositiveZ => Self::NegativeZ,
            Self::NegativeZ => Self::PositiveZ,
        }
    }
}

/// A selected face
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceSelection {
    /// Block ID
    pub block_id: u32,
    /// Face direction in block's local space
    pub direction: FaceDirection,
    /// Center of the face in world space
    pub center: Vec3,
    /// Normal of the face in world space
    pub normal: Vec3,
}

/// A selected edge
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeSelection {
    /// Block ID
    pub block_id: u32,
    /// Start vertex in world space
    pub start: Vec3,
    /// End vertex in world space
    pub end: Vec3,
    /// Edge index (0-11 for box)
    pub edge_index: u8,
}

/// A selected vertex
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexSelection {
    /// Block ID
    pub block_id: u32,
    /// Vertex position in world space
    pub position: Vec3,
    /// Vertex index (0-7 for box)
    pub vertex_index: u8,
}

// ============================================================================
// SCULPTING STATE
// ============================================================================

/// State of the sculpting tool
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SculptState {
    /// Idle - no sculpting in progress
    Idle,
    /// Selecting an element (face, edge, or vertex)
    Selecting,
    /// Dragging/extruding
    Dragging {
        /// Start position of the drag
        start_pos: Vec3,
        /// Current position of the drag
        current_pos: Vec3,
    },
}

impl Default for SculptState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Mode of sculpting operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SculptMode {
    /// Extrude faces
    Extrude,
    /// Pull edges
    PullEdge,
    /// Pull vertices
    PullVertex,
}

impl Default for SculptMode {
    fn default() -> Self {
        Self::Extrude
    }
}

// ============================================================================
// EXTRUSION STEP
// ============================================================================

/// A single step in a multi-step extrusion
#[derive(Debug, Clone)]
pub struct ExtrusionStep {
    /// Position where this step was created
    pub position: Vec3,
    /// Size of the extruded block
    pub size: Vec3,
    /// Created block ID (if any)
    pub block_id: Option<u32>,
}

/// An in-progress extrusion operation
#[derive(Debug, Clone)]
pub struct ExtrusionOperation {
    /// Source block ID
    pub source_block_id: u32,
    /// Face being extruded
    pub face: FaceDirection,
    /// Original face center
    pub face_center: Vec3,
    /// Face normal in world space
    pub face_normal: Vec3,
    /// Size of faces being extruded
    pub face_size: Vec3,
    /// Steps in this extrusion
    pub steps: Vec<ExtrusionStep>,
    /// Minimum step distance
    pub step_size: f32,
}

impl ExtrusionOperation {
    /// Create a new extrusion operation
    pub fn new(block: &BuildingBlock, face: FaceDirection, step_size: f32) -> Option<Self> {
        match block.shape {
            BuildingBlockShape::Cube { half_extents } => {
                // Calculate face center and normal in world space
                let local_normal = face.normal();
                let world_normal = block.rotation * local_normal;

                // Face center is at position + normal * corresponding half-extent
                let face_offset = match face {
                    FaceDirection::PositiveX | FaceDirection::NegativeX => half_extents.x,
                    FaceDirection::PositiveY | FaceDirection::NegativeY => half_extents.y,
                    FaceDirection::PositiveZ | FaceDirection::NegativeZ => half_extents.z,
                };
                let face_center = block.position + world_normal * face_offset;

                // Face size (perpendicular dimensions)
                let face_size = match face {
                    FaceDirection::PositiveX | FaceDirection::NegativeX => {
                        Vec3::new(half_extents.y * 2.0, half_extents.z * 2.0, 0.0)
                    }
                    FaceDirection::PositiveY | FaceDirection::NegativeY => {
                        Vec3::new(half_extents.x * 2.0, half_extents.z * 2.0, 0.0)
                    }
                    FaceDirection::PositiveZ | FaceDirection::NegativeZ => {
                        Vec3::new(half_extents.x * 2.0, half_extents.y * 2.0, 0.0)
                    }
                };

                Some(Self {
                    source_block_id: block.id,
                    face,
                    face_center,
                    face_normal: world_normal,
                    face_size,
                    steps: Vec::new(),
                    step_size,
                })
            }
            // Can extend to other shapes as needed
            _ => None,
        }
    }

    /// Update the extrusion based on drag distance
    ///
    /// Returns IDs of any newly created blocks
    pub fn update(
        &mut self,
        drag_distance: f32,
        manager: &mut BuildingBlockManager,
        material: u8,
    ) -> Vec<u32> {
        let mut new_blocks = Vec::new();

        // Calculate number of steps needed
        let num_steps = (drag_distance / self.step_size).floor() as usize;

        // Create new steps if needed
        while self.steps.len() < num_steps {
            let step_index = self.steps.len();
            let step_distance = (step_index + 1) as f32 * self.step_size;

            // Position of new block
            let position =
                self.face_center + self.face_normal * (step_distance - self.step_size * 0.5);

            // Size of the new block (thin slice)
            let half_extents = match self.face {
                FaceDirection::PositiveX | FaceDirection::NegativeX => Vec3::new(
                    self.step_size * 0.5,
                    self.face_size.x * 0.5,
                    self.face_size.y * 0.5,
                ),
                FaceDirection::PositiveY | FaceDirection::NegativeY => Vec3::new(
                    self.face_size.x * 0.5,
                    self.step_size * 0.5,
                    self.face_size.y * 0.5,
                ),
                FaceDirection::PositiveZ | FaceDirection::NegativeZ => Vec3::new(
                    self.face_size.x * 0.5,
                    self.face_size.y * 0.5,
                    self.step_size * 0.5,
                ),
            };

            // Create the block
            let block = BuildingBlock::new(
                BuildingBlockShape::Cube { half_extents },
                position,
                material,
            );
            let block_id = manager.add_block(block);

            self.steps.push(ExtrusionStep {
                position,
                size: half_extents * 2.0,
                block_id: Some(block_id),
            });

            new_blocks.push(block_id);
        }

        // Remove steps if drag distance decreased
        while self.steps.len() > num_steps && !self.steps.is_empty() {
            if let Some(step) = self.steps.pop() {
                if let Some(block_id) = step.block_id {
                    manager.remove_block(block_id);
                }
            }
        }

        new_blocks
    }

    /// Finalize the extrusion (returns all created block IDs)
    pub fn finalize(self) -> Vec<u32> {
        self.steps.iter().filter_map(|s| s.block_id).collect()
    }

    /// Cancel the extrusion and remove all created blocks
    pub fn cancel(self, manager: &mut BuildingBlockManager) {
        for step in self.steps {
            if let Some(block_id) = step.block_id {
                manager.remove_block(block_id);
            }
        }
    }
}

// ============================================================================
// SCULPTING MANAGER
// ============================================================================

/// Manager for sculpting operations
pub struct SculptingManager {
    /// Current sculpting mode
    mode: SculptMode,
    /// Current state
    state: SculptState,
    /// Current selection
    selection: SelectionType,
    /// Active extrusion operation
    active_extrusion: Option<ExtrusionOperation>,
    /// Step size for extrusion
    extrusion_step_size: f32,
    /// Default material for new geometry
    default_material: u8,
    /// Whether sculpting is enabled
    enabled: bool,
}

impl Default for SculptingManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SculptingManager {
    /// Create a new sculpting manager
    pub fn new() -> Self {
        Self {
            mode: SculptMode::Extrude,
            state: SculptState::Idle,
            selection: SelectionType::None,
            active_extrusion: None,
            extrusion_step_size: 0.5, // 0.5 meter steps
            default_material: 0,
            enabled: false,
        }
    }

    /// Enable/disable sculpting
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.cancel();
        }
    }

    /// Check if sculpting is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Set the sculpting mode
    pub fn set_mode(&mut self, mode: SculptMode) {
        if self.mode != mode {
            self.cancel();
            self.mode = mode;
        }
    }

    /// Get current mode
    pub fn mode(&self) -> SculptMode {
        self.mode
    }

    /// Get current state
    pub fn state(&self) -> SculptState {
        self.state
    }

    /// Get current selection
    pub fn selection(&self) -> &SelectionType {
        &self.selection
    }

    /// Set step size for extrusion
    pub fn set_step_size(&mut self, size: f32) {
        self.extrusion_step_size = size.max(0.1);
    }

    /// Set default material
    pub fn set_material(&mut self, material: u8) {
        self.default_material = material;
    }

    /// Try to select a face on a block
    ///
    /// Returns true if a face was selected
    pub fn try_select_face(
        &mut self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        manager: &BuildingBlockManager,
    ) -> bool {
        if !self.enabled || self.mode != SculptMode::Extrude {
            return false;
        }

        // Find the closest face intersection
        let mut best_hit: Option<(f32, FaceSelection)> = None;

        for block in manager.blocks() {
            if let Some((t, face_sel)) = self.ray_face_intersection(ray_origin, ray_dir, block) {
                if best_hit.is_none() || t < best_hit.as_ref().unwrap().0 {
                    best_hit = Some((t, face_sel));
                }
            }
        }

        if let Some((_, face_sel)) = best_hit {
            self.selection = SelectionType::Face(face_sel);
            self.state = SculptState::Selecting;
            true
        } else {
            false
        }
    }

    /// Ray-face intersection test for a block
    fn ray_face_intersection(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        block: &BuildingBlock,
    ) -> Option<(f32, FaceSelection)> {
        let aabb = block.aabb();

        // Simple AABB ray intersection
        let inv_dir = Vec3::new(
            if ray_dir.x.abs() > 1e-6 {
                1.0 / ray_dir.x
            } else {
                f32::MAX
            },
            if ray_dir.y.abs() > 1e-6 {
                1.0 / ray_dir.y
            } else {
                f32::MAX
            },
            if ray_dir.z.abs() > 1e-6 {
                1.0 / ray_dir.z
            } else {
                f32::MAX
            },
        );

        let t1 = (aabb.min - ray_origin) * inv_dir;
        let t2 = (aabb.max - ray_origin) * inv_dir;

        let t_min = t1.min(t2);
        let t_max = t1.max(t2);

        let t_enter = t_min.max_element();
        let t_exit = t_max.min_element();

        if t_enter > t_exit || t_exit < 0.0 {
            return None;
        }

        let t = if t_enter > 0.0 { t_enter } else { t_exit };
        let hit_point = ray_origin + ray_dir * t;

        // Determine which face was hit
        let center = aabb.center();
        let half_size = aabb.size() * 0.5;
        let local_hit = hit_point - center;

        // Find the dominant axis
        let abs_local = local_hit.abs();
        let face_dir = if abs_local.x > abs_local.y && abs_local.x > abs_local.z {
            if local_hit.x > 0.0 {
                FaceDirection::PositiveX
            } else {
                FaceDirection::NegativeX
            }
        } else if abs_local.y > abs_local.z {
            if local_hit.y > 0.0 {
                FaceDirection::PositiveY
            } else {
                FaceDirection::NegativeY
            }
        } else {
            if local_hit.z > 0.0 {
                FaceDirection::PositiveZ
            } else {
                FaceDirection::NegativeZ
            }
        };

        let face_normal = block.rotation * face_dir.normal();
        let face_offset = match face_dir {
            FaceDirection::PositiveX | FaceDirection::NegativeX => half_size.x,
            FaceDirection::PositiveY | FaceDirection::NegativeY => half_size.y,
            FaceDirection::PositiveZ | FaceDirection::NegativeZ => half_size.z,
        };
        let face_center = center + face_normal * face_offset;

        Some((
            t,
            FaceSelection {
                block_id: block.id,
                direction: face_dir,
                center: face_center,
                normal: face_normal,
            },
        ))
    }

    /// Start dragging (extrusion or pulling)
    pub fn start_drag(&mut self, start_pos: Vec3, manager: &BuildingBlockManager) -> bool {
        if !self.enabled {
            return false;
        }

        match &self.selection {
            SelectionType::Face(face_sel) => {
                // Start extrusion
                if let Some(block) = manager.get_block(face_sel.block_id) {
                    if let Some(extrusion) =
                        ExtrusionOperation::new(block, face_sel.direction, self.extrusion_step_size)
                    {
                        self.active_extrusion = Some(extrusion);
                        self.state = SculptState::Dragging {
                            start_pos,
                            current_pos: start_pos,
                        };
                        return true;
                    }
                }
            }
            SelectionType::Edge(_) | SelectionType::Vertex(_) => {
                // Edge/vertex pulling - would need more complex deformation
                self.state = SculptState::Dragging {
                    start_pos,
                    current_pos: start_pos,
                };
                return true;
            }
            SelectionType::None => {}
        }

        false
    }

    /// Update drag position
    ///
    /// Returns IDs of any newly created blocks
    pub fn update_drag(
        &mut self,
        current_pos: Vec3,
        manager: &mut BuildingBlockManager,
    ) -> Vec<u32> {
        if !self.enabled {
            return Vec::new();
        }

        if let SculptState::Dragging { start_pos, .. } = &mut self.state {
            let _prev_pos = *start_pos;
            self.state = SculptState::Dragging {
                start_pos: *start_pos,
                current_pos,
            };

            // Handle extrusion
            if let Some(ref mut extrusion) = self.active_extrusion {
                // Calculate drag distance along face normal
                let drag_vec = current_pos - extrusion.face_center;
                let drag_distance = drag_vec.dot(extrusion.face_normal).max(0.0);

                return extrusion.update(drag_distance, manager, self.default_material);
            }
        }

        Vec::new()
    }

    /// End dragging
    ///
    /// Returns IDs of all blocks created during the extrusion
    pub fn end_drag(&mut self) -> Vec<u32> {
        self.state = SculptState::Idle;

        if let Some(extrusion) = self.active_extrusion.take() {
            return extrusion.finalize();
        }

        Vec::new()
    }

    /// Cancel current operation
    pub fn cancel(&mut self) {
        self.state = SculptState::Idle;
        self.selection = SelectionType::None;
        self.active_extrusion = None;
    }

    /// Cancel with block removal (for active extrusion)
    pub fn cancel_with_cleanup(&mut self, manager: &mut BuildingBlockManager) {
        if let Some(extrusion) = self.active_extrusion.take() {
            extrusion.cancel(manager);
        }
        self.cancel();
    }

    /// Check if there's an active extrusion
    pub fn has_active_extrusion(&self) -> bool {
        self.active_extrusion.is_some()
    }

    /// Get the number of steps in active extrusion
    pub fn extrusion_step_count(&self) -> usize {
        self.active_extrusion.as_ref().map_or(0, |e| e.steps.len())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_face_direction_normal() {
        assert_eq!(FaceDirection::PositiveX.normal(), Vec3::X);
        assert_eq!(FaceDirection::NegativeX.normal(), Vec3::NEG_X);
        assert_eq!(FaceDirection::PositiveY.normal(), Vec3::Y);
    }

    #[test]
    fn test_face_direction_opposite() {
        assert_eq!(
            FaceDirection::PositiveX.opposite(),
            FaceDirection::NegativeX
        );
        assert_eq!(
            FaceDirection::NegativeY.opposite(),
            FaceDirection::PositiveY
        );
    }

    #[test]
    fn test_extrusion_operation() {
        let block = BuildingBlock::new(
            BuildingBlockShape::Cube {
                half_extents: Vec3::splat(1.0),
            },
            Vec3::ZERO,
            0,
        );

        let extrusion = ExtrusionOperation::new(&block, FaceDirection::PositiveY, 0.5);
        assert!(extrusion.is_some());

        let ext = extrusion.unwrap();
        assert_eq!(ext.face, FaceDirection::PositiveY);
        assert!((ext.face_center.y - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_extrusion_update() {
        let block = BuildingBlock::new(
            BuildingBlockShape::Cube {
                half_extents: Vec3::splat(1.0),
            },
            Vec3::ZERO,
            0,
        );

        let mut manager = BuildingBlockManager::new();
        manager.add_block(block.clone());

        let mut extrusion = ExtrusionOperation::new(&block, FaceDirection::PositiveY, 0.5).unwrap();

        // Drag 1.5 units - should create 3 steps
        let new_blocks = extrusion.update(1.5, &mut manager, 0);

        assert_eq!(extrusion.steps.len(), 3);
        assert_eq!(new_blocks.len(), 3);
    }

    #[test]
    fn test_sculpting_manager() {
        let mut sculpt = SculptingManager::new();

        assert!(!sculpt.is_enabled());
        assert_eq!(sculpt.mode(), SculptMode::Extrude);

        sculpt.set_enabled(true);
        assert!(sculpt.is_enabled());

        sculpt.set_mode(SculptMode::PullEdge);
        assert_eq!(sculpt.mode(), SculptMode::PullEdge);
    }
}
