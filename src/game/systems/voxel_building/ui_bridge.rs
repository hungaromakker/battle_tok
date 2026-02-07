use super::types::VoxelCoord;
use super::types::VoxelHit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    Place,
    Remove,
    CornerBrush,
    BasePlateRect,
    BasePlateCircle,
    WallLine,
    WallRing,
    JointColumn,
}

#[derive(Debug, Clone)]
pub struct VoxelHudState {
    pub visible: bool,
    pub mode: BuildMode,
    pub selected_slot: usize,
    pub corner_radius_vox: u8,
    pub wall_height_vox: u8,
    pub wall_thickness_vox: u8,
    pub plate_thickness_vox: u8,
    pub joint_spacing_vox: u8,
    pub joint_radius_vox: u8,
    pub rib_spacing_vox: u8,
    pub ring_radius_vox: u8,
    pub tool_anchor_a: Option<VoxelCoord>,
    pub hotbar_materials: [u8; 10],
    pub target_hit: Option<VoxelHit>,
}

impl Default for VoxelHudState {
    fn default() -> Self {
        Self {
            visible: false,
            mode: BuildMode::Place,
            selected_slot: 0,
            corner_radius_vox: 2,
            wall_height_vox: 24,
            wall_thickness_vox: 4,
            plate_thickness_vox: 3,
            joint_spacing_vox: 4,
            joint_radius_vox: 2,
            rib_spacing_vox: 4,
            ring_radius_vox: 20,
            tool_anchor_a: None,
            hotbar_materials: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9],
            target_hit: None,
        }
    }
}

impl VoxelHudState {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn cycle_mode(&mut self) {
        self.mode = match self.mode {
            BuildMode::Place => BuildMode::Remove,
            BuildMode::Remove => BuildMode::CornerBrush,
            BuildMode::CornerBrush => BuildMode::BasePlateRect,
            BuildMode::BasePlateRect => BuildMode::BasePlateCircle,
            BuildMode::BasePlateCircle => BuildMode::WallLine,
            BuildMode::WallLine => BuildMode::WallRing,
            BuildMode::WallRing => BuildMode::JointColumn,
            BuildMode::JointColumn => BuildMode::Place,
        };
        self.tool_anchor_a = None;
    }

    pub fn select_slot(&mut self, slot: usize) {
        if slot < self.hotbar_materials.len() {
            self.selected_slot = slot;
        }
    }

    pub fn selected_material(&self) -> u8 {
        self.hotbar_materials[self.selected_slot]
    }

    pub fn adjust_radius(&mut self, delta: i32) {
        self.adjust_primary_param(delta);
    }

    pub fn adjust_primary_param(&mut self, delta: i32) {
        match self.mode {
            BuildMode::CornerBrush => {
                let next = (self.corner_radius_vox as i32 + delta).clamp(1, 16);
                self.corner_radius_vox = next as u8;
            }
            BuildMode::BasePlateCircle | BuildMode::WallRing => {
                let next = (self.ring_radius_vox as i32 + delta).clamp(1, 64);
                self.ring_radius_vox = next as u8;
            }
            BuildMode::WallLine => {
                let next = (self.wall_thickness_vox as i32 + delta).clamp(1, 16);
                self.wall_thickness_vox = next as u8;
            }
            BuildMode::BasePlateRect => {
                let next = (self.plate_thickness_vox as i32 + delta).clamp(1, 16);
                self.plate_thickness_vox = next as u8;
            }
            BuildMode::JointColumn => {
                let next = (self.joint_radius_vox as i32 + delta).clamp(1, 8);
                self.joint_radius_vox = next as u8;
            }
            BuildMode::Place | BuildMode::Remove => {}
        }
    }

    pub fn adjust_height_param(&mut self, delta: i32) {
        match self.mode {
            BuildMode::BasePlateRect | BuildMode::BasePlateCircle => {
                let next = (self.plate_thickness_vox as i32 + delta).clamp(1, 16);
                self.plate_thickness_vox = next as u8;
            }
            BuildMode::WallLine | BuildMode::WallRing | BuildMode::JointColumn => {
                let next = (self.wall_height_vox as i32 + delta).clamp(1, 64);
                self.wall_height_vox = next as u8;
            }
            BuildMode::Place | BuildMode::Remove | BuildMode::CornerBrush => {}
        }
    }
}
