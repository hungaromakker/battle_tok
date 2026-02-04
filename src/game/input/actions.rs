//! Input Actions
//!
//! Defines all possible input actions for decoupled input handling.

use glam::Vec3;

/// Movement state for WASD/arrow keys
#[derive(Debug, Clone, Copy, Default)]
pub struct MovementState {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
}

impl MovementState {
    /// Get movement direction as normalized vector
    pub fn get_direction(&self) -> Vec3 {
        let mut dir = Vec3::ZERO;
        
        if self.forward { dir.z -= 1.0; }
        if self.backward { dir.z += 1.0; }
        if self.left { dir.x -= 1.0; }
        if self.right { dir.x += 1.0; }
        if self.up { dir.y += 1.0; }
        if self.down { dir.y -= 1.0; }
        
        if dir != Vec3::ZERO {
            dir = dir.normalize();
        }
        
        dir
    }
    
    /// Get speed multiplier based on sprint state
    pub fn speed_multiplier(&self) -> f32 {
        if self.sprint { 2.0 } else { 1.0 }
    }
}

/// Aiming state for cannon controls
#[derive(Debug, Clone, Copy, Default)]
pub struct AimingState {
    pub aim_up: bool,
    pub aim_down: bool,
    pub aim_left: bool,
    pub aim_right: bool,
}

impl AimingState {
    /// Check if any aiming input is active
    pub fn is_aiming(&self) -> bool {
        self.aim_up || self.aim_down || self.aim_left || self.aim_right
    }
    
    /// Get elevation delta (-1.0 to 1.0)
    pub fn get_elevation_delta(&self) -> f32 {
        let mut delta = 0.0;
        if self.aim_up { delta += 1.0; }
        if self.aim_down { delta -= 1.0; }
        delta
    }
    
    /// Get azimuth delta (-1.0 to 1.0)
    pub fn get_azimuth_delta(&self) -> f32 {
        let mut delta = 0.0;
        if self.aim_left { delta -= 1.0; }
        if self.aim_right { delta += 1.0; }
        delta
    }
}

/// High-level input action enum
#[derive(Debug, Clone)]
pub enum InputAction {
    /// Movement key pressed/released
    Movement(MovementKey, bool),
    /// Sprint state changed
    Sprint(bool),
    /// Jump requested
    Jump,
    /// Fire projectile
    Fire,
    /// Toggle builder mode
    ToggleBuilderMode,
    /// Toggle terrain editor UI
    ToggleTerrainEditor,
    /// Toggle first-person mode
    ToggleFirstPersonMode,
    /// Toggle fullscreen
    ToggleFullscreen,
    /// Select shape in build toolbar (0-6)
    SelectShape(usize),
    /// Next shape in toolbar
    NextShape,
    /// Previous shape in toolbar
    PreviousShape,
    /// Select material (0-7)
    SelectMaterial(u8),
    /// Apply terrain preset (1-4)
    TerrainPreset(u8),
    /// Aiming control
    Aiming(AimingKey, bool),
    /// Ctrl key state
    CtrlHeld(bool),
    /// Undo action
    Undo,
    /// Redo action
    Redo,
    /// Copy action
    Copy,
    /// Paste action
    Paste,
    /// Rotate selection
    RotateSelection,
    /// Clear projectiles
    ClearProjectiles,
    /// Reset camera
    ResetCamera,
}

/// Movement keys
#[derive(Debug, Clone, Copy)]
pub enum MovementKey {
    Forward,
    Backward,
    Left,
    Right,
    Up,
    Down,
}

/// Aiming keys
#[derive(Debug, Clone, Copy)]
pub enum AimingKey {
    Up,
    Down,
    Left,
    Right,
}

/// Context for input handling (what modes are active)
#[derive(Debug, Clone, Copy, Default)]
pub struct InputContext {
    pub builder_mode_enabled: bool,
    pub build_toolbar_visible: bool,
    pub terrain_ui_visible: bool,
    pub first_person_mode: bool,
    pub ctrl_held: bool,
}
