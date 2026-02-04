//! Cursor Manager Module
//!
//! Manages cursor capture/release for FPS gameplay.
//! Handles cursor visibility, grab mode, and FPS camera mode state.
//!
//! # Usage
//!
//! ```rust,ignore
//! use magic_engine::input::CursorManager;
//!
//! let mut cursor = CursorManager::new();
//!
//! // On game start: enable FPS mode
//! cursor.enable_fps_mode();
//!
//! // ESC pressed: release cursor
//! cursor.disable_fps_mode();
//!
//! // Left-click when released: re-capture
//! if !cursor.is_fps_mode() && left_click_pressed {
//!     cursor.enable_fps_mode();
//! }
//!
//! // Apply state to window
//! cursor.apply_to_window(&window);
//! ```

/// Actions that the CursorManager recommends after handling events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorAction {
    /// No action needed
    None,
    /// Apply cursor state to window (call apply_to_window)
    ApplyState,
}

/// Manages cursor state for FPS gameplay.
///
/// Tracks whether FPS mode is active (cursor captured and hidden)
/// and provides methods to handle various events that affect cursor state.
#[derive(Debug, Clone)]
pub struct CursorManager {
    /// Whether FPS mouse mode is active (cursor captured, hidden)
    fps_mode: bool,
    /// Whether the window currently has focus
    has_focus: bool,
    /// Whether the cursor is currently inside the window
    cursor_in_window: bool,
    /// Tracks if state changed and needs to be applied to window
    state_dirty: bool,
}

impl Default for CursorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CursorManager {
    /// Create a new CursorManager with FPS mode enabled by default.
    ///
    /// Games typically start with cursor captured for immediate FPS gameplay.
    pub fn new() -> Self {
        Self {
            fps_mode: true,
            has_focus: true,
            cursor_in_window: true,
            state_dirty: true, // Need to apply initial state
        }
    }

    /// Create a new CursorManager with FPS mode disabled.
    ///
    /// Use this when starting in a menu or editor mode where the cursor
    /// should be visible.
    pub fn new_released() -> Self {
        Self {
            fps_mode: false,
            has_focus: true,
            cursor_in_window: true,
            state_dirty: true,
        }
    }

    /// Check if FPS mode is currently active.
    ///
    /// When FPS mode is active, the cursor is hidden and captured
    /// for camera control.
    pub fn is_fps_mode(&self) -> bool {
        self.fps_mode
    }

    /// Check if the window has focus.
    pub fn has_focus(&self) -> bool {
        self.has_focus
    }

    /// Check if the cursor is inside the window.
    pub fn is_cursor_in_window(&self) -> bool {
        self.cursor_in_window
    }

    /// Check if cursor state needs to be applied to the window.
    pub fn is_dirty(&self) -> bool {
        self.state_dirty
    }

    /// Clear the dirty flag after applying state.
    pub fn clear_dirty(&mut self) {
        self.state_dirty = false;
    }

    /// Enable FPS mode: capture cursor and hide it.
    ///
    /// Call `apply_to_window()` after this to actually update the window.
    pub fn enable_fps_mode(&mut self) {
        if !self.fps_mode {
            self.fps_mode = true;
            self.state_dirty = true;
        }
    }

    /// Disable FPS mode: release cursor and show it.
    ///
    /// Call `apply_to_window()` after this to actually update the window.
    pub fn disable_fps_mode(&mut self) {
        if self.fps_mode {
            self.fps_mode = false;
            self.state_dirty = true;
        }
    }

    /// Toggle FPS mode on/off.
    ///
    /// Call `apply_to_window()` after this to actually update the window.
    pub fn toggle_fps_mode(&mut self) {
        self.fps_mode = !self.fps_mode;
        self.state_dirty = true;
    }

    /// Handle ESC key press: release cursor, show it, pause FPS mode.
    ///
    /// Returns the action to take. If `ApplyState`, call `apply_to_window()`.
    pub fn handle_escape(&mut self) -> CursorAction {
        if self.fps_mode {
            self.disable_fps_mode();
            CursorAction::ApplyState
        } else {
            CursorAction::None
        }
    }

    /// Handle left-click when cursor is released: re-capture cursor.
    ///
    /// Only re-captures if FPS mode is currently disabled.
    /// Returns the action to take. If `ApplyState`, call `apply_to_window()`.
    pub fn handle_left_click(&mut self) -> CursorAction {
        if !self.fps_mode {
            self.enable_fps_mode();
            CursorAction::ApplyState
        } else {
            CursorAction::None
        }
    }

    /// Handle window focus gained event.
    ///
    /// Restores cursor state when window regains focus.
    /// Returns the action to take. If `ApplyState`, call `apply_to_window()`.
    pub fn handle_focus_gained(&mut self) -> CursorAction {
        self.has_focus = true;
        self.state_dirty = true;
        CursorAction::ApplyState
    }

    /// Handle window focus lost event.
    ///
    /// When focus is lost, we typically want to release the cursor
    /// so it can be used in other windows.
    pub fn handle_focus_lost(&mut self) {
        self.has_focus = false;
        // Note: We don't change fps_mode here - we remember the user's preference
        // and restore it when focus is regained.
    }

    /// Handle cursor entering the window.
    ///
    /// Restores cursor state when cursor re-enters.
    /// Returns the action to take. If `ApplyState`, call `apply_to_window()`.
    pub fn handle_cursor_enter(&mut self) -> CursorAction {
        self.cursor_in_window = true;
        self.state_dirty = true;
        CursorAction::ApplyState
    }

    /// Handle cursor leaving the window.
    pub fn handle_cursor_leave(&mut self) {
        self.cursor_in_window = false;
    }

    /// Get the desired cursor visibility based on current state.
    ///
    /// Returns `false` if FPS mode is active and window has focus,
    /// `true` otherwise (cursor should be visible).
    pub fn should_cursor_be_visible(&self) -> bool {
        // Cursor should be hidden only when:
        // - FPS mode is active
        // - Window has focus
        !(self.fps_mode && self.has_focus)
    }

    /// Get whether cursor should be grabbed/captured.
    ///
    /// Returns `true` if FPS mode is active and window has focus.
    pub fn should_cursor_be_grabbed(&self) -> bool {
        self.fps_mode && self.has_focus
    }

    /// Get a human-readable status message for the current cursor state.
    pub fn status_message(&self) -> &'static str {
        if self.fps_mode {
            "FPS mode enabled. ESC to release cursor."
        } else {
            "FPS mode disabled. Left-click to re-enable."
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_starts_in_fps_mode() {
        let cursor = CursorManager::new();
        assert!(cursor.is_fps_mode());
        assert!(cursor.has_focus());
        assert!(cursor.is_dirty());
    }

    #[test]
    fn test_new_released_starts_without_fps_mode() {
        let cursor = CursorManager::new_released();
        assert!(!cursor.is_fps_mode());
    }

    #[test]
    fn test_enable_disable_fps_mode() {
        let mut cursor = CursorManager::new_released();
        cursor.clear_dirty();

        cursor.enable_fps_mode();
        assert!(cursor.is_fps_mode());
        assert!(cursor.is_dirty());

        cursor.clear_dirty();
        cursor.disable_fps_mode();
        assert!(!cursor.is_fps_mode());
        assert!(cursor.is_dirty());
    }

    #[test]
    fn test_toggle_fps_mode() {
        let mut cursor = CursorManager::new();
        assert!(cursor.is_fps_mode());

        cursor.toggle_fps_mode();
        assert!(!cursor.is_fps_mode());

        cursor.toggle_fps_mode();
        assert!(cursor.is_fps_mode());
    }

    #[test]
    fn test_handle_escape_releases_cursor() {
        let mut cursor = CursorManager::new();

        let action = cursor.handle_escape();
        assert_eq!(action, CursorAction::ApplyState);
        assert!(!cursor.is_fps_mode());

        // Pressing ESC again when already released does nothing
        let action = cursor.handle_escape();
        assert_eq!(action, CursorAction::None);
    }

    #[test]
    fn test_handle_left_click_recaptures() {
        let mut cursor = CursorManager::new_released();

        let action = cursor.handle_left_click();
        assert_eq!(action, CursorAction::ApplyState);
        assert!(cursor.is_fps_mode());

        // Clicking again when already captured does nothing
        let action = cursor.handle_left_click();
        assert_eq!(action, CursorAction::None);
    }

    #[test]
    fn test_focus_handling() {
        let mut cursor = CursorManager::new();

        cursor.handle_focus_lost();
        assert!(!cursor.has_focus());
        // FPS mode preference is preserved
        assert!(cursor.is_fps_mode());

        let action = cursor.handle_focus_gained();
        assert_eq!(action, CursorAction::ApplyState);
        assert!(cursor.has_focus());
        // FPS mode is still enabled
        assert!(cursor.is_fps_mode());
    }

    #[test]
    fn test_cursor_visibility_state() {
        let mut cursor = CursorManager::new();

        // FPS mode + focus = hidden cursor
        assert!(!cursor.should_cursor_be_visible());
        assert!(cursor.should_cursor_be_grabbed());

        // No FPS mode = visible cursor
        cursor.disable_fps_mode();
        assert!(cursor.should_cursor_be_visible());
        assert!(!cursor.should_cursor_be_grabbed());

        // FPS mode but no focus = visible cursor
        cursor.enable_fps_mode();
        cursor.handle_focus_lost();
        assert!(cursor.should_cursor_be_visible());
        assert!(!cursor.should_cursor_be_grabbed());
    }

    #[test]
    fn test_cursor_enter_leave() {
        let mut cursor = CursorManager::new();

        cursor.handle_cursor_leave();
        assert!(!cursor.is_cursor_in_window());

        let action = cursor.handle_cursor_enter();
        assert_eq!(action, CursorAction::ApplyState);
        assert!(cursor.is_cursor_in_window());
    }

    #[test]
    fn test_status_message() {
        let mut cursor = CursorManager::new();
        assert!(cursor.status_message().contains("ESC"));

        cursor.disable_fps_mode();
        assert!(cursor.status_message().contains("Left-click"));
    }
}
