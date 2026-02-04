//! Mouse Input Module
//!
//! Contains mouse state tracking for position, buttons, and scroll wheel.
//! Decoupled from winit to use generic types.

/// Mouse button identifiers, independent of windowing system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
    /// Additional mouse buttons (button 4, 5, etc.)
    Other(u16),
}

/// State of all mouse buttons.
#[derive(Debug, Clone, Copy, Default)]
pub struct ButtonState {
    pub left: bool,
    pub middle: bool,
    pub right: bool,
}

impl ButtonState {
    /// Create a new button state with all buttons released.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update button state for a specific button.
    pub fn set(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::Left => self.left = pressed,
            MouseButton::Middle => self.middle = pressed,
            MouseButton::Right => self.right = pressed,
            MouseButton::Other(_) => {} // Ignore extra buttons for now
        }
    }

    /// Check if any button is pressed.
    pub fn any_pressed(&self) -> bool {
        self.left || self.middle || self.right
    }

    /// Check if a specific button is pressed.
    pub fn is_pressed(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => self.left,
            MouseButton::Middle => self.middle,
            MouseButton::Right => self.right,
            MouseButton::Other(_) => false,
        }
    }

    /// Reset all buttons to released state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// 2D position, used for mouse coordinates.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    /// Create a new position.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Create a zero position.
    pub fn zero() -> Self {
        Self::default()
    }

    /// Convert to tuple.
    pub fn to_tuple(&self) -> (f32, f32) {
        (self.x, self.y)
    }

    /// Calculate distance to another position.
    pub fn distance(&self, other: &Position) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }
}

impl From<(f32, f32)> for Position {
    fn from(tuple: (f32, f32)) -> Self {
        Self { x: tuple.0, y: tuple.1 }
    }
}

impl From<Position> for (f32, f32) {
    fn from(pos: Position) -> (f32, f32) {
        (pos.x, pos.y)
    }
}

/// Scroll wheel delta, can be line-based or pixel-based.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScrollDelta {
    /// Horizontal scroll (positive = right)
    pub x: f32,
    /// Vertical scroll (positive = up/forward)
    pub y: f32,
}

impl ScrollDelta {
    /// Create a new scroll delta.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Create from line delta (common for mouse wheels).
    pub fn from_lines(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Create from pixel delta (common for trackpads).
    /// Normalizes by dividing by 100 to get approximate line equivalents.
    pub fn from_pixels(x: f64, y: f64) -> Self {
        Self {
            x: (x / 100.0) as f32,
            y: (y / 100.0) as f32,
        }
    }

    /// Check if there's any scroll movement.
    pub fn is_zero(&self) -> bool {
        self.x == 0.0 && self.y == 0.0
    }
}

/// Complete mouse state tracking.
///
/// Tracks position, buttons, scroll wheel, and provides delta calculations.
#[derive(Debug, Clone, Default)]
pub struct MouseState {
    /// Current mouse position in normalized UV coordinates (0.0 to 1.0).
    /// Origin is bottom-left, Y increases upward.
    pub position: Option<Position>,

    /// Current mouse position in raw pixel coordinates.
    pub position_pixels: Option<Position>,

    /// Previous position for delta calculations.
    pub last_position: Option<Position>,

    /// Current button states.
    pub buttons: ButtonState,

    /// Most recent scroll wheel delta.
    pub scroll: ScrollDelta,

    /// Whether the mouse is inside the window.
    pub in_window: bool,
}

impl MouseState {
    /// Create a new mouse state with no position and all buttons released.
    pub fn new() -> Self {
        Self::default()
    }

    /// Update mouse position from raw pixel coordinates.
    ///
    /// # Arguments
    /// * `x` - X position in pixels
    /// * `y` - Y position in pixels (origin at top)
    /// * `window_width` - Window width in pixels
    /// * `window_height` - Window height in pixels
    pub fn set_position(&mut self, x: f64, y: f64, window_width: u32, window_height: u32) {
        // Store previous position for delta calculations
        self.last_position = self.position;

        // Store raw pixel position
        self.position_pixels = Some(Position::new(x as f32, y as f32));

        // Calculate normalized UV coordinates (bottom-left origin, Y up)
        let norm_x = x as f32 / window_width as f32;
        let norm_y = 1.0 - (y as f32 / window_height as f32); // Flip Y
        self.position = Some(Position::new(norm_x, norm_y));
    }

    /// Get the normalized position as a tuple, if available.
    pub fn normalized_position(&self) -> Option<(f32, f32)> {
        self.position.map(|p| p.to_tuple())
    }

    /// Calculate the position delta since last update.
    pub fn delta(&self) -> Option<Position> {
        match (self.position, self.last_position) {
            (Some(current), Some(last)) => Some(Position::new(
                current.x - last.x,
                current.y - last.y,
            )),
            _ => None,
        }
    }

    /// Handle a mouse button press/release event.
    pub fn set_button(&mut self, button: MouseButton, pressed: bool) {
        self.buttons.set(button, pressed);

        // Clear last position when releasing any look/pan button
        // to prevent jumps when re-pressing
        if !pressed {
            match button {
                MouseButton::Middle | MouseButton::Right => {
                    self.last_position = None;
                }
                _ => {}
            }
        }
    }

    /// Handle a scroll wheel event.
    pub fn set_scroll(&mut self, delta: ScrollDelta) {
        self.scroll = delta;
    }

    /// Clear the scroll delta (call after processing).
    pub fn clear_scroll(&mut self) {
        self.scroll = ScrollDelta::default();
    }

    /// Handle mouse entering the window.
    pub fn enter_window(&mut self) {
        self.in_window = true;
    }

    /// Handle mouse leaving the window.
    pub fn leave_window(&mut self) {
        self.in_window = false;
        // Clear positions when leaving
        self.position = None;
        self.position_pixels = None;
        self.last_position = None;
    }

    /// Reset all mouse state.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Check if the mouse is being used for camera look (right button held).
    pub fn is_looking(&self) -> bool {
        self.buttons.right
    }

    /// Check if the mouse is being used for camera pan (middle button held).
    pub fn is_panning(&self) -> bool {
        self.buttons.middle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_button_state_default() {
        let buttons = ButtonState::new();
        assert!(!buttons.any_pressed());
        assert!(!buttons.is_pressed(MouseButton::Left));
    }

    #[test]
    fn test_button_state_set() {
        let mut buttons = ButtonState::new();
        buttons.set(MouseButton::Left, true);
        assert!(buttons.left);
        assert!(buttons.any_pressed());
        assert!(buttons.is_pressed(MouseButton::Left));
        assert!(!buttons.is_pressed(MouseButton::Right));
    }

    #[test]
    fn test_position_distance() {
        let p1 = Position::new(0.0, 0.0);
        let p2 = Position::new(3.0, 4.0);
        assert_eq!(p1.distance(&p2), 5.0);
    }

    #[test]
    fn test_mouse_state_position() {
        let mut mouse = MouseState::new();
        mouse.set_position(100.0, 50.0, 200, 100);

        // Check normalized position (Y is flipped)
        let pos = mouse.position.unwrap();
        assert_eq!(pos.x, 0.5);
        assert_eq!(pos.y, 0.5); // 50/100 = 0.5, flipped = 0.5

        // Check pixel position
        let px = mouse.position_pixels.unwrap();
        assert_eq!(px.x, 100.0);
        assert_eq!(px.y, 50.0);
    }

    #[test]
    fn test_mouse_state_delta() {
        let mut mouse = MouseState::new();
        mouse.set_position(100.0, 50.0, 200, 100);
        assert!(mouse.delta().is_none()); // No previous position

        mouse.set_position(120.0, 60.0, 200, 100);
        let delta = mouse.delta().unwrap();
        // Use approximate comparison for floating point
        assert!((delta.x - 0.1).abs() < 0.001); // (120-100)/200 = 0.1
        assert!((delta.y + 0.1).abs() < 0.001); // Y inverted: (60-50)/100 = 0.1, but direction is negative
    }

    #[test]
    fn test_scroll_delta() {
        let scroll = ScrollDelta::from_lines(0.0, 2.0);
        assert!(!scroll.is_zero());
        assert_eq!(scroll.y, 2.0);

        let scroll_px = ScrollDelta::from_pixels(0.0, 200.0);
        assert_eq!(scroll_px.y, 2.0);
    }
}
