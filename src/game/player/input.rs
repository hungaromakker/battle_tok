//! Player Input System
//!
//! Provides high-level player input handling for movement, sprint, camera toggle,
//! and mouse-based camera rotation. This module builds on top of the engine's
//! low-level input system to provide game-specific player controls.
//!
//! # Example
//!
//! ```rust,ignore
//! use game::player::input::{PlayerInput, KeyCode};
//!
//! let mut player_input = PlayerInput::new();
//!
//! // Process keyboard input
//! player_input.handle_key(KeyCode::W, true);  // Start moving forward
//! player_input.handle_key(KeyCode::ShiftLeft, true);  // Enable sprint
//!
//! // Process mouse movement
//! player_input.handle_mouse_delta(0.5, -0.3);
//!
//! // Use the input state
//! let movement = player_input.get_movement_direction();
//! let speed_multiplier = player_input.get_speed_multiplier();
//! let camera_delta = player_input.get_camera_delta();
//!
//! // Check for camera toggle
//! if player_input.camera_toggle_triggered() {
//!     // Toggle camera mode
//! }
//!
//! // Reset at end of frame
//! player_input.end_frame();
//! ```

/// Key codes supported by PlayerInput.
///
/// This is a subset of common key codes, containing only the keys
/// relevant to player input. Can be converted from the engine's KeyCode
/// or winit's KeyCode via the `From` trait implementations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// W key - forward movement
    W,
    /// A key - left strafe
    A,
    /// S key - backward movement
    S,
    /// D key - right strafe
    D,
    /// Left Shift - sprint modifier
    ShiftLeft,
    /// Right Shift - sprint modifier
    ShiftRight,
    /// V key - camera toggle
    V,
    /// Catch-all for unsupported keys
    Unknown,
}

/// Movement direction for the player.
///
/// Represents normalized movement intent based on WASD keys.
#[derive(Debug, Clone, Copy, Default)]
pub struct MovementDirection {
    /// Forward/backward movement (-1.0 to 1.0, positive = forward)
    pub forward: f32,
    /// Left/right movement (-1.0 to 1.0, positive = right)
    pub right: f32,
}

impl MovementDirection {
    /// Create a zero movement direction.
    pub fn zero() -> Self {
        Self::default()
    }

    /// Check if there is any movement input.
    pub fn is_moving(&self) -> bool {
        self.forward != 0.0 || self.right != 0.0
    }

    /// Get the movement as a normalized (x, z) vector where x is right and z is forward.
    ///
    /// Returns (0, 0) if no movement, otherwise normalizes the vector.
    pub fn normalized(&self) -> (f32, f32) {
        let len_sq = self.forward * self.forward + self.right * self.right;
        if len_sq == 0.0 {
            return (0.0, 0.0);
        }
        let len = len_sq.sqrt();
        (self.right / len, self.forward / len)
    }
}

/// Camera rotation delta from mouse input.
#[derive(Debug, Clone, Copy, Default)]
pub struct CameraDelta {
    /// Horizontal rotation (yaw) delta in normalized units
    pub yaw: f32,
    /// Vertical rotation (pitch) delta in normalized units
    pub pitch: f32,
}

impl CameraDelta {
    /// Create a zero camera delta.
    pub fn zero() -> Self {
        Self::default()
    }

    /// Check if there is any camera rotation input.
    pub fn has_rotation(&self) -> bool {
        self.yaw != 0.0 || self.pitch != 0.0
    }
}

/// Player input state tracking.
///
/// This struct tracks all player-relevant input state including:
/// - WASD movement keys
/// - Sprint modifier (Shift) - provides 2x speed multiplier
/// - Camera toggle (V key with debounce)
/// - Mouse delta for camera rotation
///
/// Input state is designed to be reset each frame after processing.
#[derive(Debug, Clone)]
pub struct PlayerInput {
    // Movement keys (WASD)
    key_forward: bool,
    key_backward: bool,
    key_left: bool,
    key_right: bool,

    // Sprint modifier
    key_sprint: bool,

    // Camera toggle state (V key)
    key_camera_toggle: bool,
    camera_toggle_was_pressed: bool,
    camera_toggle_triggered: bool,

    // Mouse delta for camera rotation
    mouse_delta_x: f32,
    mouse_delta_y: f32,

    // Sprint speed multiplier (default 2.0)
    sprint_multiplier: f32,
}

impl Default for PlayerInput {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerInput {
    /// Create a new player input state with all inputs released.
    pub fn new() -> Self {
        Self {
            key_forward: false,
            key_backward: false,
            key_left: false,
            key_right: false,
            key_sprint: false,
            key_camera_toggle: false,
            camera_toggle_was_pressed: false,
            camera_toggle_triggered: false,
            mouse_delta_x: 0.0,
            mouse_delta_y: 0.0,
            sprint_multiplier: 2.0,
        }
    }

    /// Create a player input state with a custom sprint multiplier.
    pub fn with_sprint_multiplier(sprint_multiplier: f32) -> Self {
        Self {
            sprint_multiplier,
            ..Self::new()
        }
    }

    /// Handle a key press or release event.
    ///
    /// Accepts a KeyCode and updates the appropriate input state.
    /// Returns `true` if the key was a player-relevant key and was handled.
    ///
    /// # Arguments
    /// * `key` - The key code
    /// * `pressed` - Whether the key is pressed (true) or released (false)
    pub fn handle_key(&mut self, key: KeyCode, pressed: bool) -> bool {
        match key {
            // Movement keys
            KeyCode::W => {
                self.key_forward = pressed;
                true
            }
            KeyCode::S => {
                self.key_backward = pressed;
                true
            }
            KeyCode::A => {
                self.key_left = pressed;
                true
            }
            KeyCode::D => {
                self.key_right = pressed;
                true
            }

            // Sprint modifier (both shift keys)
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.key_sprint = pressed;
                true
            }

            // Camera toggle (V key - debounced)
            KeyCode::V => {
                self.key_camera_toggle = pressed;
                // Trigger on press only (debounce)
                if pressed && !self.camera_toggle_was_pressed {
                    self.camera_toggle_triggered = true;
                }
                self.camera_toggle_was_pressed = pressed;
                true
            }

            KeyCode::Unknown => false,
        }
    }

    /// Handle mouse movement delta.
    ///
    /// The delta values should be in screen-space units (e.g., pixels or normalized coordinates).
    /// Positive delta_x rotates right (clockwise), positive delta_y looks up.
    ///
    /// # Arguments
    /// * `delta_x` - Horizontal mouse movement (right is positive)
    /// * `delta_y` - Vertical mouse movement (up is positive)
    pub fn handle_mouse_delta(&mut self, delta_x: f32, delta_y: f32) {
        self.mouse_delta_x += delta_x;
        self.mouse_delta_y += delta_y;
    }

    /// Get the current movement direction based on WASD keys.
    ///
    /// Returns a `MovementDirection` with forward and right values in range -1.0 to 1.0.
    pub fn get_movement_direction(&self) -> MovementDirection {
        MovementDirection {
            forward: (self.key_forward as i32 - self.key_backward as i32) as f32,
            right: (self.key_right as i32 - self.key_left as i32) as f32,
        }
    }

    /// Check if the player is currently sprinting.
    pub fn is_sprinting(&self) -> bool {
        self.key_sprint
    }

    /// Get the speed multiplier based on sprint state.
    ///
    /// Returns `sprint_multiplier` (default 2.0) if sprinting, otherwise 1.0.
    pub fn get_speed_multiplier(&self) -> f32 {
        if self.key_sprint {
            self.sprint_multiplier
        } else {
            1.0
        }
    }

    /// Check if the camera toggle was triggered this frame.
    ///
    /// This is debounced - it only returns true on the frame when V was first pressed,
    /// not while held or on release.
    pub fn camera_toggle_triggered(&self) -> bool {
        self.camera_toggle_triggered
    }

    /// Get the accumulated mouse delta for camera rotation.
    ///
    /// Returns a `CameraDelta` where positive yaw rotates right
    /// and positive pitch looks up.
    pub fn get_camera_delta(&self) -> CameraDelta {
        CameraDelta {
            yaw: self.mouse_delta_x,
            pitch: self.mouse_delta_y,
        }
    }

    /// Get raw mouse delta values.
    ///
    /// Returns (delta_x, delta_y).
    pub fn get_mouse_delta(&self) -> (f32, f32) {
        (self.mouse_delta_x, self.mouse_delta_y)
    }

    /// Reset per-frame input state.
    ///
    /// This should be called at the end of each frame after processing input.
    /// It resets:
    /// - Mouse delta accumulator
    /// - Camera toggle triggered flag
    ///
    /// Continuous key states (movement, sprint) are NOT reset since they persist
    /// until the key is released.
    pub fn end_frame(&mut self) {
        self.mouse_delta_x = 0.0;
        self.mouse_delta_y = 0.0;
        self.camera_toggle_triggered = false;
    }

    /// Fully reset all input state.
    ///
    /// Resets everything including held keys. Use this when losing focus or
    /// when all input should be cleared.
    pub fn reset(&mut self) {
        *self = Self {
            sprint_multiplier: self.sprint_multiplier,
            ..Self::new()
        };
    }

    /// Check if any movement key is currently pressed.
    pub fn is_any_movement_pressed(&self) -> bool {
        self.key_forward || self.key_backward || self.key_left || self.key_right
    }

    /// Set the sprint speed multiplier.
    pub fn set_sprint_multiplier(&mut self, multiplier: f32) {
        self.sprint_multiplier = multiplier;
    }

    /// Get the current sprint multiplier setting.
    pub fn get_sprint_multiplier(&self) -> f32 {
        self.sprint_multiplier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_input_default() {
        let input = PlayerInput::new();
        assert!(!input.is_any_movement_pressed());
        assert!(!input.is_sprinting());
        assert!(!input.camera_toggle_triggered());
        assert_eq!(input.get_speed_multiplier(), 1.0);
    }

    #[test]
    fn test_movement_direction() {
        let mut input = PlayerInput::new();

        // Test forward
        input.handle_key(KeyCode::W, true);
        let dir = input.get_movement_direction();
        assert_eq!(dir.forward, 1.0);
        assert_eq!(dir.right, 0.0);

        // Test backward
        input.handle_key(KeyCode::W, false);
        input.handle_key(KeyCode::S, true);
        let dir = input.get_movement_direction();
        assert_eq!(dir.forward, -1.0);

        // Test both forward and backward (cancels out)
        input.handle_key(KeyCode::W, true);
        let dir = input.get_movement_direction();
        assert_eq!(dir.forward, 0.0);
    }

    #[test]
    fn test_strafe_movement() {
        let mut input = PlayerInput::new();

        // Test right strafe
        input.handle_key(KeyCode::D, true);
        let dir = input.get_movement_direction();
        assert_eq!(dir.right, 1.0);
        assert_eq!(dir.forward, 0.0);

        // Test left strafe
        input.handle_key(KeyCode::D, false);
        input.handle_key(KeyCode::A, true);
        let dir = input.get_movement_direction();
        assert_eq!(dir.right, -1.0);
    }

    #[test]
    fn test_diagonal_movement() {
        let mut input = PlayerInput::new();
        input.handle_key(KeyCode::W, true);
        input.handle_key(KeyCode::D, true);

        let dir = input.get_movement_direction();
        assert_eq!(dir.forward, 1.0);
        assert_eq!(dir.right, 1.0);

        // Normalized should give roughly 0.707, 0.707
        let (norm_x, norm_z) = dir.normalized();
        assert!((norm_x - 0.7071).abs() < 0.01);
        assert!((norm_z - 0.7071).abs() < 0.01);
    }

    #[test]
    fn test_sprint_modifier() {
        let mut input = PlayerInput::new();
        assert_eq!(input.get_speed_multiplier(), 1.0);

        // Enable sprint
        input.handle_key(KeyCode::ShiftLeft, true);
        assert!(input.is_sprinting());
        assert_eq!(input.get_speed_multiplier(), 2.0);

        // Disable sprint
        input.handle_key(KeyCode::ShiftLeft, false);
        assert!(!input.is_sprinting());
        assert_eq!(input.get_speed_multiplier(), 1.0);
    }

    #[test]
    fn test_sprint_multiplier_custom() {
        let input = PlayerInput::with_sprint_multiplier(3.0);
        assert_eq!(input.get_sprint_multiplier(), 3.0);
    }

    #[test]
    fn test_camera_toggle_debounce() {
        let mut input = PlayerInput::new();

        // First press triggers
        input.handle_key(KeyCode::V, true);
        assert!(input.camera_toggle_triggered());

        // End frame clears trigger
        input.end_frame();
        assert!(!input.camera_toggle_triggered());

        // Holding doesn't re-trigger
        input.handle_key(KeyCode::V, true);
        assert!(!input.camera_toggle_triggered());

        // Release and re-press triggers again
        input.handle_key(KeyCode::V, false);
        input.handle_key(KeyCode::V, true);
        assert!(input.camera_toggle_triggered());
    }

    #[test]
    fn test_mouse_delta() {
        let mut input = PlayerInput::new();

        input.handle_mouse_delta(1.0, 0.5);
        let (dx, dy) = input.get_mouse_delta();
        assert_eq!(dx, 1.0);
        assert_eq!(dy, 0.5);

        // Accumulates
        input.handle_mouse_delta(0.5, 0.25);
        let (dx, dy) = input.get_mouse_delta();
        assert_eq!(dx, 1.5);
        assert_eq!(dy, 0.75);

        // End frame resets
        input.end_frame();
        let (dx, dy) = input.get_mouse_delta();
        assert_eq!(dx, 0.0);
        assert_eq!(dy, 0.0);
    }

    #[test]
    fn test_camera_delta() {
        let mut input = PlayerInput::new();
        input.handle_mouse_delta(2.0, -1.0);

        let delta = input.get_camera_delta();
        assert_eq!(delta.yaw, 2.0);
        assert_eq!(delta.pitch, -1.0);
        assert!(delta.has_rotation());

        input.end_frame();
        let delta = input.get_camera_delta();
        assert!(!delta.has_rotation());
    }

    #[test]
    fn test_end_frame_preserves_movement() {
        let mut input = PlayerInput::new();
        input.handle_key(KeyCode::W, true);
        input.handle_key(KeyCode::ShiftLeft, true);
        input.handle_mouse_delta(1.0, 0.0);

        input.end_frame();

        // Movement and sprint should still be active
        assert!(input.is_any_movement_pressed());
        assert!(input.is_sprinting());

        // Mouse delta should be reset
        let (dx, _) = input.get_mouse_delta();
        assert_eq!(dx, 0.0);
    }

    #[test]
    fn test_full_reset() {
        let mut input = PlayerInput::with_sprint_multiplier(3.0);
        input.handle_key(KeyCode::W, true);
        input.handle_key(KeyCode::ShiftLeft, true);
        input.handle_mouse_delta(1.0, 1.0);

        input.reset();

        assert!(!input.is_any_movement_pressed());
        assert!(!input.is_sprinting());
        let (dx, dy) = input.get_mouse_delta();
        assert_eq!(dx, 0.0);
        assert_eq!(dy, 0.0);
        // Sprint multiplier should be preserved
        assert_eq!(input.get_sprint_multiplier(), 3.0);
    }

    #[test]
    fn test_movement_direction_normalized() {
        let dir = MovementDirection {
            forward: 0.0,
            right: 0.0,
        };
        let (x, z) = dir.normalized();
        assert_eq!(x, 0.0);
        assert_eq!(z, 0.0);
    }

    #[test]
    fn test_unknown_key_not_handled() {
        let mut input = PlayerInput::new();
        let handled = input.handle_key(KeyCode::Unknown, true);
        assert!(!handled);
    }

    #[test]
    fn test_right_shift_sprint() {
        let mut input = PlayerInput::new();

        // Right shift should also trigger sprint
        input.handle_key(KeyCode::ShiftRight, true);
        assert!(input.is_sprinting());
        assert_eq!(input.get_speed_multiplier(), 2.0);

        input.handle_key(KeyCode::ShiftRight, false);
        assert!(!input.is_sprinting());
    }
}
