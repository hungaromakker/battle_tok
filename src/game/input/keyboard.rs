//! Keyboard Mapping
//!
//! Maps keyboard input to game actions.

use super::actions::{AimingKey, InputAction, InputContext, MovementKey};
use winit::keyboard::KeyCode;

/// Map a keyboard key event to an InputAction
///
/// # Arguments
/// * `key` - The key code that was pressed/released
/// * `pressed` - Whether the key was pressed (true) or released (false)
/// * `context` - Current input context (what modes are active)
///
/// # Returns
/// Optional InputAction if the key maps to an action
pub fn map_key_to_action(
    key: KeyCode,
    pressed: bool,
    context: &InputContext,
) -> Option<InputAction> {
    match key {
        // Movement
        KeyCode::KeyW => Some(InputAction::Movement(MovementKey::Forward, pressed)),
        KeyCode::KeyS => Some(InputAction::Movement(MovementKey::Backward, pressed)),
        KeyCode::KeyA => Some(InputAction::Movement(MovementKey::Left, pressed)),
        KeyCode::KeyD => Some(InputAction::Movement(MovementKey::Right, pressed)),

        KeyCode::Space => {
            if pressed {
                if context.first_person_mode {
                    Some(InputAction::Jump)
                } else if !context.builder_mode_enabled {
                    Some(InputAction::Fire)
                } else {
                    Some(InputAction::Movement(MovementKey::Up, pressed))
                }
            } else {
                Some(InputAction::Movement(MovementKey::Up, pressed))
            }
        }

        KeyCode::ShiftLeft | KeyCode::ShiftRight => Some(InputAction::Sprint(pressed)),

        // Ctrl key
        KeyCode::ControlLeft | KeyCode::ControlRight => Some(InputAction::CtrlHeld(pressed)),

        // Builder mode toggle
        KeyCode::KeyB if pressed => Some(InputAction::ToggleBuilderMode),

        // Tab - cycle shapes
        KeyCode::Tab if pressed && context.build_toolbar_visible => Some(InputAction::NextShape),

        // Shape selection (1-7) when toolbar visible
        KeyCode::Digit1 if pressed && context.build_toolbar_visible => {
            Some(InputAction::SelectShape(0))
        }
        KeyCode::Digit2 if pressed && context.build_toolbar_visible => {
            Some(InputAction::SelectShape(1))
        }
        KeyCode::Digit3 if pressed && context.build_toolbar_visible => {
            Some(InputAction::SelectShape(2))
        }
        KeyCode::Digit4 if pressed && context.build_toolbar_visible => {
            Some(InputAction::SelectShape(3))
        }
        KeyCode::Digit5 if pressed && context.build_toolbar_visible => {
            Some(InputAction::SelectShape(4))
        }
        KeyCode::Digit6 if pressed && context.build_toolbar_visible => {
            Some(InputAction::SelectShape(5))
        }
        KeyCode::Digit7 if pressed && context.build_toolbar_visible => {
            Some(InputAction::SelectShape(6))
        }

        // Arrow keys for shape navigation in build mode
        KeyCode::ArrowUp if pressed && context.build_toolbar_visible => {
            Some(InputAction::PreviousShape)
        }
        KeyCode::ArrowDown if pressed && context.build_toolbar_visible => {
            Some(InputAction::NextShape)
        }

        // F11 - Fullscreen
        KeyCode::F11 if pressed => Some(InputAction::ToggleFullscreen),

        // Terrain editor toggle
        KeyCode::KeyT if pressed => Some(InputAction::ToggleTerrainEditor),

        // First-person mode toggle
        KeyCode::KeyV if pressed && !context.ctrl_held => Some(InputAction::ToggleFirstPersonMode),

        // Terrain presets (F1-F4)
        KeyCode::F1 if pressed => Some(InputAction::TerrainPreset(1)),
        KeyCode::F2 if pressed => Some(InputAction::TerrainPreset(2)),
        KeyCode::F3 if pressed => Some(InputAction::TerrainPreset(3)),
        KeyCode::F4 if pressed => Some(InputAction::TerrainPreset(4)),

        // Material selection (1-8) in builder mode
        KeyCode::Digit1
            if pressed
                && context.builder_mode_enabled
                && !context.terrain_ui_visible
                && !context.build_toolbar_visible =>
        {
            Some(InputAction::SelectMaterial(0))
        }
        KeyCode::Digit2
            if pressed
                && context.builder_mode_enabled
                && !context.terrain_ui_visible
                && !context.build_toolbar_visible =>
        {
            Some(InputAction::SelectMaterial(1))
        }
        KeyCode::Digit3
            if pressed
                && context.builder_mode_enabled
                && !context.terrain_ui_visible
                && !context.build_toolbar_visible =>
        {
            Some(InputAction::SelectMaterial(2))
        }
        KeyCode::Digit4
            if pressed
                && context.builder_mode_enabled
                && !context.terrain_ui_visible
                && !context.build_toolbar_visible =>
        {
            Some(InputAction::SelectMaterial(3))
        }
        KeyCode::Digit5
            if pressed
                && context.builder_mode_enabled
                && !context.terrain_ui_visible
                && !context.build_toolbar_visible =>
        {
            Some(InputAction::SelectMaterial(4))
        }
        KeyCode::Digit6
            if pressed
                && context.builder_mode_enabled
                && !context.terrain_ui_visible
                && !context.build_toolbar_visible =>
        {
            Some(InputAction::SelectMaterial(5))
        }
        KeyCode::Digit7
            if pressed
                && context.builder_mode_enabled
                && !context.terrain_ui_visible
                && !context.build_toolbar_visible =>
        {
            Some(InputAction::SelectMaterial(6))
        }
        KeyCode::Digit8
            if pressed
                && context.builder_mode_enabled
                && !context.terrain_ui_visible
                && !context.build_toolbar_visible =>
        {
            Some(InputAction::SelectMaterial(7))
        }

        // Undo/Redo
        KeyCode::KeyZ if pressed && context.ctrl_held && context.builder_mode_enabled => {
            Some(InputAction::Undo)
        }
        KeyCode::KeyY if pressed && context.ctrl_held && context.builder_mode_enabled => {
            Some(InputAction::Redo)
        }

        // Copy/Paste
        KeyCode::KeyC if pressed && context.ctrl_held && context.builder_mode_enabled => {
            Some(InputAction::Copy)
        }
        KeyCode::KeyV if pressed && context.ctrl_held && context.builder_mode_enabled => {
            Some(InputAction::Paste)
        }

        // Rotate selection
        KeyCode::KeyR if pressed && context.builder_mode_enabled => {
            Some(InputAction::RotateSelection)
        }

        // Camera/cannon controls
        KeyCode::KeyR if pressed && !context.builder_mode_enabled => Some(InputAction::ResetCamera),
        KeyCode::KeyC if pressed && !context.ctrl_held => Some(InputAction::ClearProjectiles),

        // Aiming keys (when not in build mode with toolbar)
        KeyCode::ArrowUp if !context.build_toolbar_visible => {
            Some(InputAction::Aiming(AimingKey::Up, pressed))
        }
        KeyCode::ArrowDown if !context.build_toolbar_visible => {
            Some(InputAction::Aiming(AimingKey::Down, pressed))
        }
        KeyCode::ArrowLeft => Some(InputAction::Aiming(AimingKey::Left, pressed)),
        KeyCode::ArrowRight => Some(InputAction::Aiming(AimingKey::Right, pressed)),

        _ => None,
    }
}
