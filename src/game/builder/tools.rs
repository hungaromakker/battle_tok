//! Builder Tools
//!
//! Tool definitions and state for the building system.

use glam::Vec3;

/// Shape names for display
pub const SHAPE_NAMES: [&str; 7] = [
    "Cube", "Cylinder", "Sphere", "Dome", "Arch", "Wedge", "Bridge",
];

/// Grid size for block placement snapping
pub const BLOCK_GRID_SIZE: f32 = 1.0;

/// Snap distance - blocks within this distance snap together
pub const BLOCK_SNAP_DISTANCE: f32 = 0.3;

/// Physics support check interval in seconds
pub const PHYSICS_CHECK_INTERVAL: f32 = 5.0;

/// Selected face for bridge tool
#[derive(Clone, Copy, Debug)]
pub struct SelectedFace {
    /// Block ID
    pub block_id: u32,
    /// Face center position in world space
    pub position: Vec3,
    /// Face normal direction
    pub _normal: Vec3,
    /// Face size (width, height)
    pub size: (f32, f32),
}

/// Bridge tool state
#[derive(Default)]
pub struct BridgeTool {
    /// First selected face
    pub first_face: Option<SelectedFace>,
    /// Second selected face (when both are set, can create bridge)
    pub second_face: Option<SelectedFace>,
    /// Whether in face selection mode
    pub selecting: bool,
}

impl BridgeTool {
    pub fn clear(&mut self) {
        self.first_face = None;
        self.second_face = None;
    }

    pub fn select_face(&mut self, face: SelectedFace) {
        if self.first_face.is_none() {
            self.first_face = Some(face);
            println!(
                "[Bridge] First face selected at ({:.1}, {:.1}, {:.1})",
                face.position.x, face.position.y, face.position.z
            );
        } else if self.second_face.is_none() {
            self.second_face = Some(face);
            println!("[Bridge] Second face selected - ready to create bridge!");
        }
    }

    pub fn is_ready(&self) -> bool {
        self.first_face.is_some() && self.second_face.is_some()
    }
}
