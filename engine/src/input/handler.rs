//! Input Handler
//!
//! Centralized input handling for the game engine.
//! Maps physical input events to game actions.

use std::collections::HashMap;
use winit::event::MouseButton;
use winit::keyboard::KeyCode;

/// Input actions that can be triggered by the player
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameAction {
    // Movement
    MoveForward,
    MoveBackward,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    Sprint,
    Jump,

    // Camera/Aiming
    AimUp,
    AimDown,
    AimLeft,
    AimRight,
    ResetCamera,

    // Combat
    Fire,
    Reload,

    // Building
    ToggleBuildMode,
    PlaceBlock,
    RemoveBlock,
    RotateBlock,
    SelectShape1,
    SelectShape2,
    SelectShape3,
    SelectShape4,
    SelectShape5,
    SelectShape6,
    SelectShape7,
    NextShape,
    PrevShape,

    // Editing
    Undo,
    Redo,
    Copy,
    Paste,

    // UI
    ToggleTerrainEditor,
    ToggleFullscreen,
    ToggleFirstPerson,
    ClearProjectiles,

    // Terrain presets
    TerrainPreset1,
    TerrainPreset2,
    TerrainPreset3,
    TerrainPreset4,

    // System
    Escape,
    Confirm,
}

/// State of a key (pressed or released)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyState {
    pub pressed: bool,
    pub just_pressed: bool,
    pub just_released: bool,
}

impl Default for KeyState {
    fn default() -> Self {
        Self {
            pressed: false,
            just_pressed: false,
            just_released: false,
        }
    }
}

/// Current input state
pub struct InputState {
    /// Key states mapped by KeyCode
    keys: HashMap<KeyCode, KeyState>,
    /// Action states mapped by GameAction
    actions: HashMap<GameAction, KeyState>,
    /// Key-to-action bindings
    bindings: HashMap<KeyCode, Vec<GameAction>>,
    /// Whether Ctrl is held (for shortcuts)
    ctrl_held: bool,
    /// Whether Shift is held
    shift_held: bool,
    /// Whether Alt is held
    alt_held: bool,
    /// Mouse position in screen coordinates
    mouse_position: Option<(f32, f32)>,
    /// Mouse button states
    left_mouse: KeyState,
    right_mouse: KeyState,
    middle_mouse: KeyState,
    /// Mouse movement delta since last frame
    mouse_delta: (f32, f32),
}

impl InputState {
    pub fn new() -> Self {
        let mut state = Self {
            keys: HashMap::new(),
            actions: HashMap::new(),
            bindings: HashMap::new(),
            ctrl_held: false,
            shift_held: false,
            alt_held: false,
            mouse_position: None,
            left_mouse: KeyState::default(),
            right_mouse: KeyState::default(),
            middle_mouse: KeyState::default(),
            mouse_delta: (0.0, 0.0),
        };
        state.setup_default_bindings();
        state
    }

    /// Setup default key bindings
    fn setup_default_bindings(&mut self) {
        // Movement (WASD + Space/Shift)
        self.bind(KeyCode::KeyW, GameAction::MoveForward);
        self.bind(KeyCode::KeyS, GameAction::MoveBackward);
        self.bind(KeyCode::KeyA, GameAction::MoveLeft);
        self.bind(KeyCode::KeyD, GameAction::MoveRight);
        self.bind(KeyCode::Space, GameAction::Jump);
        self.bind(KeyCode::Space, GameAction::MoveUp);
        self.bind(KeyCode::ShiftLeft, GameAction::Sprint);
        self.bind(KeyCode::ShiftLeft, GameAction::MoveDown);
        self.bind(KeyCode::ShiftRight, GameAction::Sprint);
        self.bind(KeyCode::ShiftRight, GameAction::MoveDown);

        // Camera/Aiming (Arrow keys)
        self.bind(KeyCode::ArrowUp, GameAction::AimUp);
        self.bind(KeyCode::ArrowDown, GameAction::AimDown);
        self.bind(KeyCode::ArrowLeft, GameAction::AimLeft);
        self.bind(KeyCode::ArrowRight, GameAction::AimRight);
        self.bind(KeyCode::KeyR, GameAction::ResetCamera);

        // Building
        self.bind(KeyCode::KeyB, GameAction::ToggleBuildMode);
        self.bind(KeyCode::Tab, GameAction::NextShape);
        self.bind(KeyCode::Digit1, GameAction::SelectShape1);
        self.bind(KeyCode::Digit2, GameAction::SelectShape2);
        self.bind(KeyCode::Digit3, GameAction::SelectShape3);
        self.bind(KeyCode::Digit4, GameAction::SelectShape4);
        self.bind(KeyCode::Digit5, GameAction::SelectShape5);
        self.bind(KeyCode::Digit6, GameAction::SelectShape6);
        self.bind(KeyCode::Digit7, GameAction::SelectShape7);

        // Editing (Ctrl+Z, Ctrl+Y, Ctrl+C, Ctrl+V)
        // Note: These are handled specially due to modifier keys
        self.bind(KeyCode::KeyZ, GameAction::Undo);
        self.bind(KeyCode::KeyY, GameAction::Redo);
        self.bind(KeyCode::KeyC, GameAction::Copy);
        self.bind(KeyCode::KeyV, GameAction::Paste);
        self.bind(KeyCode::KeyR, GameAction::RotateBlock);

        // UI
        self.bind(KeyCode::KeyT, GameAction::ToggleTerrainEditor);
        self.bind(KeyCode::F11, GameAction::ToggleFullscreen);
        self.bind(KeyCode::KeyV, GameAction::ToggleFirstPerson);
        self.bind(KeyCode::KeyC, GameAction::ClearProjectiles);

        // Terrain presets
        self.bind(KeyCode::F1, GameAction::TerrainPreset1);
        self.bind(KeyCode::F2, GameAction::TerrainPreset2);
        self.bind(KeyCode::F3, GameAction::TerrainPreset3);
        self.bind(KeyCode::F4, GameAction::TerrainPreset4);

        // System
        self.bind(KeyCode::Escape, GameAction::Escape);
        self.bind(KeyCode::Enter, GameAction::Confirm);
    }

    /// Bind a key to an action
    pub fn bind(&mut self, key: KeyCode, action: GameAction) {
        self.bindings.entry(key).or_default().push(action);
    }

    /// Unbind a key from an action
    pub fn unbind(&mut self, key: KeyCode, action: GameAction) {
        if let Some(actions) = self.bindings.get_mut(&key) {
            actions.retain(|a| *a != action);
        }
    }

    /// Handle a key event
    pub fn handle_key(&mut self, key: KeyCode, pressed: bool) {
        // Update key state
        let state = self.keys.entry(key).or_default();
        state.just_pressed = pressed && !state.pressed;
        state.just_released = !pressed && state.pressed;
        state.pressed = pressed;

        // Update modifier keys
        match key {
            KeyCode::ControlLeft | KeyCode::ControlRight => self.ctrl_held = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.shift_held = pressed,
            KeyCode::AltLeft | KeyCode::AltRight => self.alt_held = pressed,
            _ => {}
        }

        // Update action states from bindings
        if let Some(actions) = self.bindings.get(&key).cloned() {
            for action in actions {
                let action_state = self.actions.entry(action).or_default();
                action_state.just_pressed = pressed && !action_state.pressed;
                action_state.just_released = !pressed && action_state.pressed;
                action_state.pressed = pressed;
            }
        }
    }

    /// Handle mouse button event
    pub fn handle_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        let state = match button {
            MouseButton::Left => &mut self.left_mouse,
            MouseButton::Right => &mut self.right_mouse,
            MouseButton::Middle => &mut self.middle_mouse,
            _ => return,
        };
        state.just_pressed = pressed && !state.pressed;
        state.just_released = !pressed && state.pressed;
        state.pressed = pressed;
    }

    /// Handle mouse movement
    pub fn handle_mouse_move(&mut self, x: f32, y: f32) {
        self.mouse_position = Some((x, y));
    }

    /// Handle raw mouse movement (for camera control)
    pub fn handle_mouse_delta(&mut self, dx: f32, dy: f32) {
        self.mouse_delta.0 += dx;
        self.mouse_delta.1 += dy;
    }

    /// Clear per-frame state (call at end of frame)
    pub fn end_frame(&mut self) {
        // Clear just_pressed/just_released flags
        for state in self.keys.values_mut() {
            state.just_pressed = false;
            state.just_released = false;
        }
        for state in self.actions.values_mut() {
            state.just_pressed = false;
            state.just_released = false;
        }
        self.left_mouse.just_pressed = false;
        self.left_mouse.just_released = false;
        self.right_mouse.just_pressed = false;
        self.right_mouse.just_released = false;
        self.middle_mouse.just_pressed = false;
        self.middle_mouse.just_released = false;
        self.mouse_delta = (0.0, 0.0);
    }

    // Query methods

    /// Check if a key is currently pressed
    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.keys.get(&key).is_some_and(|s| s.pressed)
    }

    /// Check if a key was just pressed this frame
    pub fn key_just_pressed(&self, key: KeyCode) -> bool {
        self.keys.get(&key).is_some_and(|s| s.just_pressed)
    }

    /// Check if a key was just released this frame
    pub fn key_just_released(&self, key: KeyCode) -> bool {
        self.keys.get(&key).is_some_and(|s| s.just_released)
    }

    /// Check if an action is currently active
    pub fn action_pressed(&self, action: GameAction) -> bool {
        self.actions.get(&action).is_some_and(|s| s.pressed)
    }

    /// Check if an action was just triggered this frame
    pub fn action_just_pressed(&self, action: GameAction) -> bool {
        self.actions.get(&action).is_some_and(|s| s.just_pressed)
    }

    /// Check if an action was just released this frame
    pub fn action_just_released(&self, action: GameAction) -> bool {
        self.actions.get(&action).is_some_and(|s| s.just_released)
    }

    /// Check if Ctrl is held (for shortcuts)
    pub fn ctrl_held(&self) -> bool {
        self.ctrl_held
    }

    /// Check if Shift is held
    pub fn shift_held(&self) -> bool {
        self.shift_held
    }

    /// Check if Alt is held
    pub fn alt_held(&self) -> bool {
        self.alt_held
    }

    /// Get mouse position in screen coordinates
    pub fn mouse_position(&self) -> Option<(f32, f32)> {
        self.mouse_position
    }

    /// Check if left mouse button is pressed
    pub fn left_mouse_pressed(&self) -> bool {
        self.left_mouse.pressed
    }

    /// Check if left mouse button was just clicked
    pub fn left_mouse_just_pressed(&self) -> bool {
        self.left_mouse.just_pressed
    }

    /// Check if right mouse button is pressed
    pub fn right_mouse_pressed(&self) -> bool {
        self.right_mouse.pressed
    }

    /// Check if right mouse button was just clicked
    pub fn right_mouse_just_pressed(&self) -> bool {
        self.right_mouse.just_pressed
    }

    /// Get mouse movement delta since last frame
    pub fn mouse_delta(&self) -> (f32, f32) {
        self.mouse_delta
    }

    // High-level movement queries (convenience methods)

    /// Get movement vector from WASD keys
    pub fn movement_vector(&self) -> (f32, f32, f32) {
        let mut x = 0.0;
        let mut y = 0.0;
        let mut z = 0.0;

        if self.action_pressed(GameAction::MoveForward) {
            z += 1.0;
        }
        if self.action_pressed(GameAction::MoveBackward) {
            z -= 1.0;
        }
        if self.action_pressed(GameAction::MoveLeft) {
            x -= 1.0;
        }
        if self.action_pressed(GameAction::MoveRight) {
            x += 1.0;
        }
        if self.action_pressed(GameAction::MoveUp) {
            y += 1.0;
        }
        if self.action_pressed(GameAction::MoveDown) {
            y -= 1.0;
        }

        (x, y, z)
    }

    /// Get aim direction from arrow keys
    pub fn aim_vector(&self) -> (f32, f32) {
        let mut x = 0.0;
        let mut y = 0.0;

        if self.action_pressed(GameAction::AimUp) {
            y += 1.0;
        }
        if self.action_pressed(GameAction::AimDown) {
            y -= 1.0;
        }
        if self.action_pressed(GameAction::AimLeft) {
            x -= 1.0;
        }
        if self.action_pressed(GameAction::AimRight) {
            x += 1.0;
        }

        (x, y)
    }

    /// Check if sprinting
    pub fn is_sprinting(&self) -> bool {
        self.action_pressed(GameAction::Sprint)
    }

    /// Check if jumping
    pub fn is_jumping(&self) -> bool {
        self.action_just_pressed(GameAction::Jump)
    }
}

impl Default for InputState {
    fn default() -> Self {
        Self::new()
    }
}
