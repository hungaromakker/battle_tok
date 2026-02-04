//! FPS-style Mouse State Tracker
//!
//! Handles captured mouse input with delta accumulation for FPS-style camera control.
//! Unlike the standard `MouseState`, this module tracks raw mouse deltas that accumulate
//! between frames and can be consumed atomically.

/// FPS-style mouse state tracker with delta accumulation.
///
/// This struct is designed for FPS games where the mouse cursor is captured/hidden
/// and mouse movement is used directly for camera rotation. Key features:
///
/// - **Delta accumulation**: Raw mouse deltas accumulate until consumed
/// - **Cursor capture tracking**: Knows whether the cursor is currently captured
/// - **Atomic consumption**: `consume_delta()` returns accumulated delta and resets it
///
/// # Example
///
/// ```rust,ignore
/// use magic_engine::input::FpsMouseState;
///
/// let mut mouse = FpsMouseState::new();
///
/// // In event loop: accumulate raw mouse motion
/// mouse.accumulate_delta(10.0, -5.0);
/// mouse.accumulate_delta(3.0, 2.0);
///
/// // In update loop: consume accumulated delta
/// let (dx, dy) = mouse.consume_delta();
/// // dx = 13.0, dy = -3.0
/// camera.rotate(dx * sensitivity, dy * sensitivity);
/// ```
#[derive(Debug, Clone, Default)]
pub struct FpsMouseState {
    /// Accumulated horizontal delta since last consume.
    delta_x: f32,
    /// Accumulated vertical delta since last consume.
    delta_y: f32,
    /// Whether the cursor is currently captured (hidden and confined).
    cursor_captured: bool,
}

impl FpsMouseState {
    /// Create a new FPS mouse state with zero deltas and cursor not captured.
    pub fn new() -> Self {
        Self::default()
    }

    /// Accumulate raw mouse motion delta.
    ///
    /// Call this from your event loop whenever raw mouse motion is received.
    /// Deltas accumulate until `consume_delta()` is called.
    ///
    /// # Arguments
    ///
    /// * `dx` - Horizontal delta in device units (pixels on most systems)
    /// * `dy` - Vertical delta in device units
    #[inline]
    pub fn accumulate_delta(&mut self, dx: f32, dy: f32) {
        self.delta_x += dx;
        self.delta_y += dy;
    }

    /// Consume the accumulated delta, returning it and resetting to zero.
    ///
    /// Call this once per frame in your update loop to get all accumulated
    /// mouse motion since the last frame.
    ///
    /// # Returns
    ///
    /// A tuple `(delta_x, delta_y)` representing the total accumulated motion.
    #[inline]
    pub fn consume_delta(&mut self) -> (f32, f32) {
        let delta = (self.delta_x, self.delta_y);
        self.delta_x = 0.0;
        self.delta_y = 0.0;
        delta
    }

    /// Set whether the cursor is captured.
    ///
    /// When captured, the cursor is typically hidden and confined to the window,
    /// and raw mouse motion is used for camera control.
    ///
    /// # Arguments
    ///
    /// * `captured` - `true` to mark cursor as captured, `false` otherwise
    #[inline]
    pub fn set_captured(&mut self, captured: bool) {
        self.cursor_captured = captured;
        // Clear accumulated deltas when capture state changes to prevent jumps
        if !captured {
            self.delta_x = 0.0;
            self.delta_y = 0.0;
        }
    }

    /// Check if the cursor is currently captured.
    #[inline]
    pub fn is_captured(&self) -> bool {
        self.cursor_captured
    }

    /// Get the current accumulated delta without consuming it.
    ///
    /// Useful for debugging or preview purposes.
    #[inline]
    pub fn peek_delta(&self) -> (f32, f32) {
        (self.delta_x, self.delta_y)
    }

    /// Reset all state to defaults.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_state() {
        let state = FpsMouseState::new();
        assert_eq!(state.peek_delta(), (0.0, 0.0));
        assert!(!state.is_captured());
    }

    #[test]
    fn test_accumulate_delta() {
        let mut state = FpsMouseState::new();
        state.accumulate_delta(10.0, 5.0);
        assert_eq!(state.peek_delta(), (10.0, 5.0));

        state.accumulate_delta(3.0, -2.0);
        assert_eq!(state.peek_delta(), (13.0, 3.0));
    }

    #[test]
    fn test_consume_delta() {
        let mut state = FpsMouseState::new();
        state.accumulate_delta(10.0, 5.0);
        state.accumulate_delta(3.0, -2.0);

        let delta = state.consume_delta();
        assert_eq!(delta, (13.0, 3.0));

        // After consume, deltas should be zero
        assert_eq!(state.peek_delta(), (0.0, 0.0));
        assert_eq!(state.consume_delta(), (0.0, 0.0));
    }

    #[test]
    fn test_set_captured() {
        let mut state = FpsMouseState::new();
        assert!(!state.is_captured());

        state.set_captured(true);
        assert!(state.is_captured());

        // Accumulate some delta while captured
        state.accumulate_delta(10.0, 5.0);
        assert_eq!(state.peek_delta(), (10.0, 5.0));

        // Releasing capture should clear deltas to prevent jump
        state.set_captured(false);
        assert!(!state.is_captured());
        assert_eq!(state.peek_delta(), (0.0, 0.0));
    }

    #[test]
    fn test_reset() {
        let mut state = FpsMouseState::new();
        state.accumulate_delta(10.0, 5.0);
        state.set_captured(true);

        state.reset();
        assert_eq!(state.peek_delta(), (0.0, 0.0));
        assert!(!state.is_captured());
    }
}
