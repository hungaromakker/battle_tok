//! Keyboard Input Module
//!
//! Contains keyboard state tracking for movement keys and other input.
//! Decoupled from winit to use generic key codes.

/// Generic key codes for movement input, independent of windowing system.
///
/// These map to standard keyboard keys but are not tied to winit::keyboard::KeyCode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    // Movement keys
    W,
    A,
    S,
    D,
    Q,
    E,
    Space,
    ShiftLeft,
    ShiftRight,

    // Arrow keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Function keys
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // Number keys
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,

    // Numpad
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadEnter,
    NumpadDecimal,

    // Control keys
    Escape,
    Enter,
    Tab,
    Backspace,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    ControlLeft,
    ControlRight,

    // Letter keys (for various bindings)
    B,
    C,
    F,
    G,
    H,
    J, // Fog start distance control
    K, // Fog density control
    L,
    M,
    N,
    P,
    R,
    T,
    V,
    X,

    // Punctuation and brackets
    BracketLeft,
    BracketRight,
    Comma,
    Period,
    Minus,
    Equal,

    /// Catch-all for unhandled keys
    Unknown,
}

/// Tracks the current state of movement keys.
///
/// This struct maintains which movement keys are currently pressed,
/// allowing smooth continuous movement when keys are held down.
#[derive(Debug, Clone, Copy, Default)]
pub struct MovementKeys {
    /// W key - move forward
    pub forward: bool,
    /// S key - move backward
    pub backward: bool,
    /// A key - move left (strafe)
    pub left: bool,
    /// D key - move right (strafe)
    pub right: bool,
    /// Space - jump (when grounded) or move up (in fly mode)
    pub up: bool,
    /// Q, E - move down (fly down)
    pub down: bool,
    /// Shift - sprint
    pub sprint: bool,
    /// Ctrl - crouch (reduces player height, can go very low for small creatures)
    pub crouch: bool,
}

impl MovementKeys {
    /// Create a new movement keys state with all keys released.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update movement state based on key press/release.
    ///
    /// Returns `true` if the key was a movement key and was handled,
    /// `false` otherwise.
    pub fn handle_key(&mut self, key: KeyCode, pressed: bool) -> bool {
        match key {
            KeyCode::W => {
                self.forward = pressed;
                true
            }
            KeyCode::S => {
                self.backward = pressed;
                true
            }
            KeyCode::A => {
                self.left = pressed;
                true
            }
            KeyCode::D => {
                self.right = pressed;
                true
            }
            KeyCode::Q | KeyCode::E => {
                // Q and E for vertical movement (fly mode)
                self.down = pressed;
                true
            }
            KeyCode::Space => {
                // Space for jump / fly up
                self.up = pressed;
                true
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                // Shift for sprinting
                self.sprint = pressed;
                true
            }
            KeyCode::ControlLeft | KeyCode::ControlRight => {
                // Ctrl for crouching (can go very low for small creatures)
                self.crouch = pressed;
                true
            }
            _ => false,
        }
    }

    /// Check if any movement key is currently pressed.
    pub fn any_pressed(&self) -> bool {
        self.forward || self.backward || self.left || self.right || self.up || self.down
    }

    /// Check if sprint key is currently pressed.
    pub fn is_sprinting(&self) -> bool {
        self.sprint
    }

    /// Check if crouch key is currently pressed.
    pub fn is_crouching(&self) -> bool {
        self.crouch
    }

    /// Reset all movement keys to released state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Get the forward/backward movement direction (-1, 0, or 1).
    pub fn forward_axis(&self) -> i32 {
        (self.forward as i32) - (self.backward as i32)
    }

    /// Get the left/right movement direction (-1, 0, or 1).
    pub fn right_axis(&self) -> i32 {
        (self.right as i32) - (self.left as i32)
    }

    /// Get the up/down movement direction (-1, 0, or 1).
    pub fn up_axis(&self) -> i32 {
        (self.up as i32) - (self.down as i32)
    }
}

/// Complete keyboard state tracking.
///
/// Tracks movement keys and modifier state for comprehensive keyboard input handling.
#[derive(Debug, Clone, Default)]
pub struct KeyboardState {
    /// Movement key states
    pub movement: MovementKeys,
    /// Modifier keys (Shift, Ctrl, Alt, etc.)
    pub modifiers: ModifierState,
}

/// State of keyboard modifier keys.
#[derive(Debug, Clone, Copy, Default)]
pub struct ModifierState {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl ModifierState {
    /// Create a new empty modifier state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if no modifiers are pressed.
    pub fn is_empty(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && !self.meta
    }
}

impl KeyboardState {
    /// Create a new keyboard state with all keys released.
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle a key press or release event.
    ///
    /// Returns `true` if the key was handled as a movement key.
    pub fn handle_key(&mut self, key: KeyCode, pressed: bool) -> bool {
        self.movement.handle_key(key, pressed)
    }

    /// Update modifier state.
    pub fn set_modifiers(&mut self, modifiers: ModifierState) {
        self.modifiers = modifiers;
    }

    /// Reset all keyboard state.
    pub fn reset(&mut self) {
        self.movement.reset();
        self.modifiers = ModifierState::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movement_keys_default() {
        let keys = MovementKeys::new();
        assert!(!keys.any_pressed());
        assert_eq!(keys.forward_axis(), 0);
        assert_eq!(keys.right_axis(), 0);
        assert_eq!(keys.up_axis(), 0);
    }

    #[test]
    fn test_movement_keys_forward() {
        let mut keys = MovementKeys::new();
        assert!(keys.handle_key(KeyCode::W, true));
        assert!(keys.forward);
        assert!(keys.any_pressed());
        assert_eq!(keys.forward_axis(), 1);
    }

    #[test]
    fn test_movement_axes() {
        let mut keys = MovementKeys::new();
        keys.handle_key(KeyCode::W, true);
        keys.handle_key(KeyCode::S, true);
        // Both pressed cancels out
        assert_eq!(keys.forward_axis(), 0);

        keys.handle_key(KeyCode::D, true);
        assert_eq!(keys.right_axis(), 1);

        keys.handle_key(KeyCode::Space, true);
        assert_eq!(keys.up_axis(), 1);
    }

    #[test]
    fn test_sprint_key() {
        let mut keys = MovementKeys::new();
        assert!(!keys.is_sprinting());

        keys.handle_key(KeyCode::ShiftLeft, true);
        assert!(keys.is_sprinting());

        keys.handle_key(KeyCode::ShiftLeft, false);
        assert!(!keys.is_sprinting());
    }

    #[test]
    fn test_non_movement_key() {
        let mut keys = MovementKeys::new();
        assert!(!keys.handle_key(KeyCode::Escape, true));
        assert!(!keys.any_pressed());
    }
}
