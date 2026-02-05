//! Input Configuration
//!
//! Defines all key bindings as a data structure, enabling future remapping
//! and centralizing input documentation. This replaces hardcoded key matches
//! in battle_arena.rs with a configurable struct.

use winit::keyboard::KeyCode;

/// Category a key binding belongs to, returned by `InputConfig::classify_key`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputCategory {
    Movement,
    Aiming,
    Building,
    Camera,
    UI,
    Combat,
    Editing,
}

/// Movement key bindings (WASD + jump/sprint).
#[derive(Clone, Debug)]
pub struct MovementBindings {
    pub forward: KeyCode,
    pub backward: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
    pub jump: KeyCode,
    pub sprint_left: KeyCode,
    pub sprint_right: KeyCode,
}

/// Cannon/camera aiming key bindings (arrow keys).
#[derive(Clone, Debug)]
pub struct AimingBindings {
    pub up: KeyCode,
    pub down: KeyCode,
    pub left: KeyCode,
    pub right: KeyCode,
}

/// Building mode key bindings.
#[derive(Clone, Debug)]
pub struct BuildingBindings {
    pub toggle_mode: KeyCode,
    pub cycle_shape: KeyCode,
    pub shape_keys: [KeyCode; 7],
    pub material_keys: [KeyCode; 8],
}

/// Camera control key bindings.
#[derive(Clone, Debug)]
pub struct CameraBindings {
    pub toggle_mode: KeyCode,
    pub reset: KeyCode,
}

/// UI key bindings.
#[derive(Clone, Debug)]
pub struct UIBindings {
    pub terrain_editor: KeyCode,
    pub terrain_preset_flat: KeyCode,
    pub terrain_preset_hills: KeyCode,
    pub terrain_preset_rocky: KeyCode,
    pub terrain_preset_mountains: KeyCode,
    pub fullscreen: KeyCode,
}

/// Editing key bindings (undo/redo/copy/paste/rotate).
#[derive(Clone, Debug)]
pub struct EditingBindings {
    pub undo: KeyCode,
    pub redo: KeyCode,
    pub copy: KeyCode,
    pub paste: KeyCode,
    pub rotate: KeyCode,
    pub ctrl_left: KeyCode,
    pub ctrl_right: KeyCode,
}

/// Combat key bindings.
#[derive(Clone, Debug)]
pub struct CombatBindings {
    pub fire: KeyCode,
}

/// Centralized input configuration containing all key bindings.
///
/// `InputConfig::default()` returns the current hardcoded bindings.
/// Story 11 will use this struct to drive the input handler instead
/// of inline `match` arms.
#[derive(Clone, Debug)]
pub struct InputConfig {
    pub movement: MovementBindings,
    pub aiming: AimingBindings,
    pub building: BuildingBindings,
    pub camera: CameraBindings,
    pub ui: UIBindings,
    pub editing: EditingBindings,
    pub combat: CombatBindings,
    pub clear_projectiles: KeyCode,
    pub exit: KeyCode,
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            movement: MovementBindings {
                forward: KeyCode::KeyW,
                backward: KeyCode::KeyS,
                left: KeyCode::KeyA,
                right: KeyCode::KeyD,
                jump: KeyCode::Space,
                sprint_left: KeyCode::ShiftLeft,
                sprint_right: KeyCode::ShiftRight,
            },
            aiming: AimingBindings {
                up: KeyCode::ArrowUp,
                down: KeyCode::ArrowDown,
                left: KeyCode::ArrowLeft,
                right: KeyCode::ArrowRight,
            },
            building: BuildingBindings {
                toggle_mode: KeyCode::KeyB,
                cycle_shape: KeyCode::Tab,
                shape_keys: [
                    KeyCode::Digit1,
                    KeyCode::Digit2,
                    KeyCode::Digit3,
                    KeyCode::Digit4,
                    KeyCode::Digit5,
                    KeyCode::Digit6,
                    KeyCode::Digit7,
                ],
                material_keys: [
                    KeyCode::Digit1,
                    KeyCode::Digit2,
                    KeyCode::Digit3,
                    KeyCode::Digit4,
                    KeyCode::Digit5,
                    KeyCode::Digit6,
                    KeyCode::Digit7,
                    KeyCode::Digit8,
                ],
            },
            camera: CameraBindings {
                toggle_mode: KeyCode::KeyV,
                reset: KeyCode::KeyR,
            },
            ui: UIBindings {
                terrain_editor: KeyCode::KeyT,
                terrain_preset_flat: KeyCode::F1,
                terrain_preset_hills: KeyCode::F2,
                terrain_preset_rocky: KeyCode::F3,
                terrain_preset_mountains: KeyCode::F4,
                fullscreen: KeyCode::F11,
            },
            editing: EditingBindings {
                undo: KeyCode::KeyZ,
                redo: KeyCode::KeyY,
                copy: KeyCode::KeyC,
                paste: KeyCode::KeyV,
                rotate: KeyCode::KeyR,
                ctrl_left: KeyCode::ControlLeft,
                ctrl_right: KeyCode::ControlRight,
            },
            combat: CombatBindings {
                fire: KeyCode::Space,
            },
            clear_projectiles: KeyCode::KeyC,
            exit: KeyCode::Escape,
        }
    }
}

impl InputConfig {
    /// Classify which category a key belongs to.
    ///
    /// Returns `None` if the key is not bound to any action.
    /// When a key is shared across categories (e.g., Space for jump and fire,
    /// or digit keys for shapes and materials), the first matching category
    /// is returned in this priority order: Movement, Combat, Building,
    /// Aiming, Editing, Camera, UI.
    pub fn classify_key(&self, key: KeyCode) -> Option<InputCategory> {
        // Movement
        if key == self.movement.forward
            || key == self.movement.backward
            || key == self.movement.left
            || key == self.movement.right
            || key == self.movement.jump
            || key == self.movement.sprint_left
            || key == self.movement.sprint_right
        {
            return Some(InputCategory::Movement);
        }

        // Combat
        if key == self.combat.fire || key == self.clear_projectiles {
            return Some(InputCategory::Combat);
        }

        // Building
        if key == self.building.toggle_mode
            || key == self.building.cycle_shape
            || self.building.shape_keys.contains(&key)
            || self.building.material_keys.contains(&key)
        {
            return Some(InputCategory::Building);
        }

        // Aiming
        if key == self.aiming.up
            || key == self.aiming.down
            || key == self.aiming.left
            || key == self.aiming.right
        {
            return Some(InputCategory::Aiming);
        }

        // Editing
        if key == self.editing.undo
            || key == self.editing.redo
            || key == self.editing.copy
            || key == self.editing.paste
            || key == self.editing.rotate
            || key == self.editing.ctrl_left
            || key == self.editing.ctrl_right
        {
            return Some(InputCategory::Editing);
        }

        // Camera
        if key == self.camera.toggle_mode || key == self.camera.reset {
            return Some(InputCategory::Camera);
        }

        // UI
        if key == self.ui.terrain_editor
            || key == self.ui.terrain_preset_flat
            || key == self.ui.terrain_preset_hills
            || key == self.ui.terrain_preset_rocky
            || key == self.ui.terrain_preset_mountains
            || key == self.ui.fullscreen
            || key == self.exit
        {
            return Some(InputCategory::UI);
        }

        None
    }
}
