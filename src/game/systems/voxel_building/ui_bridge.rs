use super::types::VoxelHit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildMode {
    Place,
    Remove,
    CornerBrush,
}

#[derive(Debug, Clone)]
pub struct VoxelHudState {
    pub visible: bool,
    pub mode: BuildMode,
    pub selected_slot: usize,
    pub corner_radius_vox: u8,
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
            BuildMode::CornerBrush => BuildMode::Place,
        };
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
        let next = (self.corner_radius_vox as i32 + delta).clamp(1, 16);
        self.corner_radius_vox = next as u8;
    }
}
