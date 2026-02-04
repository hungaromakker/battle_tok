//! Input Module
//!
//! Provides platform-agnostic input handling for keyboard and mouse.
//! This module is decoupled from any specific windowing system (like winit)
//! to allow for flexible integration.
//!
//! # Example
//!
//! ```rust,ignore
//! use magic_engine::input::{KeyboardState, MouseState, KeyCode, MouseButton};
//!
//! let mut keyboard = KeyboardState::new();
//! let mut mouse = MouseState::new();
//!
//! // Handle keyboard input
//! keyboard.handle_key(KeyCode::W, true); // W pressed
//! if keyboard.movement.forward {
//!     // Move forward
//! }
//!
//! // Handle mouse input
//! mouse.set_position(100.0, 50.0, 800, 600);
//! mouse.set_button(MouseButton::Left, true);
//! if let Some((x, y)) = mouse.normalized_position() {
//!     // Use normalized position for raycasting
//! }
//! ```

pub mod bindings;
pub mod cursor_manager;
pub mod keyboard;
pub mod mouse;
pub mod mouse_state;

// Re-export commonly used types at module level
pub use bindings::{InputAction, KeyBindings};
pub use cursor_manager::{CursorAction, CursorManager};
pub use keyboard::{KeyCode, KeyboardState, ModifierState, MovementKeys};
pub use mouse::{ButtonState, MouseButton, MouseState, Position, ScrollDelta};
pub use mouse_state::FpsMouseState;

/// Combined input state for both keyboard and mouse.
///
/// This provides a convenient way to track all input state in a single struct.
#[derive(Debug, Clone, Default)]
pub struct InputState {
    pub keyboard: KeyboardState,
    pub mouse: MouseState,
}

impl InputState {
    /// Create a new input state with all inputs in their default state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all input state to defaults.
    pub fn reset(&mut self) {
        self.keyboard.reset();
        self.mouse.reset();
    }

    /// Check if any movement input is active (keyboard movement or mouse look/pan).
    pub fn is_moving(&self) -> bool {
        self.keyboard.movement.any_pressed()
            || self.mouse.is_looking()
            || self.mouse.is_panning()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state_default() {
        let input = InputState::new();
        assert!(!input.is_moving());
    }

    #[test]
    fn test_input_state_keyboard_movement() {
        let mut input = InputState::new();
        input.keyboard.handle_key(KeyCode::W, true);
        assert!(input.is_moving());
    }

    #[test]
    fn test_input_state_mouse_look() {
        let mut input = InputState::new();
        input.mouse.set_button(MouseButton::Right, true);
        assert!(input.is_moving());
    }
}
