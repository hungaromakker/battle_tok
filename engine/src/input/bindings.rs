//! Input Bindings Module
//!
//! Provides a flexible input binding system that maps physical keys to logical actions,
//! allowing for future key remapping support.

use std::collections::{HashMap, HashSet};

use super::KeyCode;

/// Logical input actions that can be bound to physical keys.
///
/// These actions represent high-level game inputs independent of their physical key mappings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputAction {
    /// Move forward (default: W)
    MoveForward,
    /// Move backward (default: S)
    MoveBack,
    /// Strafe left (default: A)
    MoveLeft,
    /// Strafe right (default: D)
    MoveRight,
    /// Sprint modifier (default: Shift)
    Sprint,
    /// Jump (default: Space)
    Jump,
    /// Crouch (default: Ctrl)
    Crouch,
    /// Toggle camera mode (default: V)
    CameraToggle,
    /// Interact with objects (default: E)
    Interact,
    /// Open menu / cancel (default: Escape)
    Escape,
}

/// Maps physical keys to logical actions, supporting customizable key bindings.
///
/// This struct allows the game to use logical actions in game code while
/// maintaining the ability to remap keys without changing game logic.
#[derive(Debug, Clone)]
pub struct KeyBindings {
    /// Map from physical key to logical action
    key_to_action: HashMap<KeyCode, InputAction>,
    /// Map from logical action to physical key (for reverse lookup and display)
    action_to_key: HashMap<InputAction, KeyCode>,
}

impl Default for KeyBindings {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyBindings {
    /// Create a new KeyBindings instance with default key mappings.
    ///
    /// Default bindings:
    /// - W = MoveForward
    /// - S = MoveBack
    /// - A = MoveLeft
    /// - D = MoveRight
    /// - Shift (Left) = Sprint
    /// - Space = Jump
    /// - Ctrl (Left) = Crouch
    /// - V = CameraToggle
    /// - E = Interact
    /// - Escape = Escape
    pub fn new() -> Self {
        let mut bindings = Self {
            key_to_action: HashMap::new(),
            action_to_key: HashMap::new(),
        };

        // Set up default bindings
        bindings.bind(KeyCode::W, InputAction::MoveForward);
        bindings.bind(KeyCode::S, InputAction::MoveBack);
        bindings.bind(KeyCode::A, InputAction::MoveLeft);
        bindings.bind(KeyCode::D, InputAction::MoveRight);
        bindings.bind(KeyCode::ShiftLeft, InputAction::Sprint);
        bindings.bind(KeyCode::Space, InputAction::Jump);
        bindings.bind(KeyCode::ControlLeft, InputAction::Crouch);
        bindings.bind(KeyCode::V, InputAction::CameraToggle);
        bindings.bind(KeyCode::E, InputAction::Interact);
        bindings.bind(KeyCode::Escape, InputAction::Escape);

        bindings
    }

    /// Bind a physical key to a logical action.
    ///
    /// If the key was previously bound to another action, that binding is removed.
    /// If the action was previously bound to another key, that binding is also removed.
    pub fn bind(&mut self, key: KeyCode, action: InputAction) {
        // Remove old binding for this key (if any)
        if let Some(old_action) = self.key_to_action.remove(&key) {
            self.action_to_key.remove(&old_action);
        }

        // Remove old binding for this action (if any)
        if let Some(old_key) = self.action_to_key.remove(&action) {
            self.key_to_action.remove(&old_key);
        }

        // Create new binding
        self.key_to_action.insert(key, action);
        self.action_to_key.insert(action, key);
    }

    /// Remove the binding for a specific key.
    pub fn unbind_key(&mut self, key: KeyCode) {
        if let Some(action) = self.key_to_action.remove(&key) {
            self.action_to_key.remove(&action);
        }
    }

    /// Remove the binding for a specific action.
    pub fn unbind_action(&mut self, action: InputAction) {
        if let Some(key) = self.action_to_key.remove(&action) {
            self.key_to_action.remove(&key);
        }
    }

    /// Get the action bound to a physical key, if any.
    pub fn get_action(&self, key: KeyCode) -> Option<InputAction> {
        self.key_to_action.get(&key).copied()
    }

    /// Get the key bound to a logical action, if any.
    pub fn get_key(&self, action: InputAction) -> Option<KeyCode> {
        self.action_to_key.get(&action).copied()
    }

    /// Check if a specific action is currently pressed, given a set of pressed keys.
    ///
    /// This method looks up which key is bound to the given action and checks
    /// if that key is in the pressed keys set.
    pub fn is_action_pressed(&self, action: InputAction, pressed_keys: &HashSet<KeyCode>) -> bool {
        if let Some(key) = self.action_to_key.get(&action) {
            pressed_keys.contains(key)
        } else {
            false
        }
    }

    /// Get all current bindings as key-action pairs.
    pub fn all_bindings(&self) -> impl Iterator<Item = (KeyCode, InputAction)> + '_ {
        self.key_to_action.iter().map(|(&k, &a)| (k, a))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_bindings() {
        let bindings = KeyBindings::new();

        assert_eq!(bindings.get_action(KeyCode::W), Some(InputAction::MoveForward));
        assert_eq!(bindings.get_action(KeyCode::S), Some(InputAction::MoveBack));
        assert_eq!(bindings.get_action(KeyCode::A), Some(InputAction::MoveLeft));
        assert_eq!(bindings.get_action(KeyCode::D), Some(InputAction::MoveRight));
        assert_eq!(bindings.get_action(KeyCode::ShiftLeft), Some(InputAction::Sprint));
        assert_eq!(bindings.get_action(KeyCode::Space), Some(InputAction::Jump));
        assert_eq!(bindings.get_action(KeyCode::ControlLeft), Some(InputAction::Crouch));
        assert_eq!(bindings.get_action(KeyCode::V), Some(InputAction::CameraToggle));
        assert_eq!(bindings.get_action(KeyCode::E), Some(InputAction::Interact));
        assert_eq!(bindings.get_action(KeyCode::Escape), Some(InputAction::Escape));
    }

    #[test]
    fn test_reverse_lookup() {
        let bindings = KeyBindings::new();

        assert_eq!(bindings.get_key(InputAction::MoveForward), Some(KeyCode::W));
        assert_eq!(bindings.get_key(InputAction::Sprint), Some(KeyCode::ShiftLeft));
        assert_eq!(bindings.get_key(InputAction::Jump), Some(KeyCode::Space));
    }

    #[test]
    fn test_rebind_key() {
        let mut bindings = KeyBindings::new();

        // Rebind forward to Up arrow
        bindings.bind(KeyCode::ArrowUp, InputAction::MoveForward);

        // W should no longer be bound
        assert_eq!(bindings.get_action(KeyCode::W), None);

        // Arrow up should now be forward
        assert_eq!(bindings.get_action(KeyCode::ArrowUp), Some(InputAction::MoveForward));
        assert_eq!(bindings.get_key(InputAction::MoveForward), Some(KeyCode::ArrowUp));
    }

    #[test]
    fn test_is_action_pressed() {
        let bindings = KeyBindings::new();

        let mut pressed = HashSet::new();
        pressed.insert(KeyCode::W);
        pressed.insert(KeyCode::ShiftLeft);

        assert!(bindings.is_action_pressed(InputAction::MoveForward, &pressed));
        assert!(bindings.is_action_pressed(InputAction::Sprint, &pressed));
        assert!(!bindings.is_action_pressed(InputAction::MoveBack, &pressed));
        assert!(!bindings.is_action_pressed(InputAction::Jump, &pressed));
    }

    #[test]
    fn test_unbind_key() {
        let mut bindings = KeyBindings::new();

        bindings.unbind_key(KeyCode::W);

        assert_eq!(bindings.get_action(KeyCode::W), None);
        assert_eq!(bindings.get_key(InputAction::MoveForward), None);
    }

    #[test]
    fn test_unbind_action() {
        let mut bindings = KeyBindings::new();

        bindings.unbind_action(InputAction::Sprint);

        assert_eq!(bindings.get_action(KeyCode::ShiftLeft), None);
        assert_eq!(bindings.get_key(InputAction::Sprint), None);
    }

    #[test]
    fn test_unbound_action_not_pressed() {
        let mut bindings = KeyBindings::new();
        bindings.unbind_action(InputAction::MoveForward);

        let mut pressed = HashSet::new();
        pressed.insert(KeyCode::W);

        // Even with W pressed, MoveForward is not triggered because it's unbound
        assert!(!bindings.is_action_pressed(InputAction::MoveForward, &pressed));
    }
}
