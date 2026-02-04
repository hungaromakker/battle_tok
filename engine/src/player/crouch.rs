//! Player Crouch System
//!
//! Provides stance management for player characters with smooth height transitions.
//!
//! # Stances
//!
//! - Standing: 1.8m height (normal movement speed)
//! - Crouching: 0.9m height (0.5x movement speed)
//! - Prone: 0.4m height (0.25x movement speed)
//!
//! # Height Transitions
//!
//! All height transitions take 0.15 seconds for smooth visual feedback.
//! Standing up is blocked if there is an obstacle above the player.
//!
//! # Usage
//!
//! ```rust,ignore
//! use battle_tok_engine::player::{CrouchController, Stance};
//!
//! let mut crouch = CrouchController::new();
//!
//! // Each frame:
//! let height = crouch.update(delta_time, crouch_input, &obstacle_check);
//! player_camera_height = height;
//! let speed_multiplier = crouch.speed_multiplier();
//! ```

/// Standing height in meters
pub const STANDING_HEIGHT: f32 = 1.8;

/// Crouching height in meters
pub const CROUCH_HEIGHT: f32 = 0.9;

/// Prone height in meters
pub const PRONE_HEIGHT: f32 = 0.4;

/// Height transition duration in seconds
pub const TRANSITION_DURATION: f32 = 0.15;

/// Speed multiplier when crouching
pub const CROUCH_SPEED_MULTIPLIER: f32 = 0.5;

/// Speed multiplier when prone
pub const PRONE_SPEED_MULTIPLIER: f32 = 0.25;

/// Player stance states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stance {
    /// Standing upright at full height (1.8m)
    Standing,
    /// Crouched at half height (0.9m)
    Crouching,
    /// Prone at minimal height (0.4m)
    Prone,
}

impl Stance {
    /// Get the target height for this stance in meters.
    pub fn height(&self) -> f32 {
        match self {
            Stance::Standing => STANDING_HEIGHT,
            Stance::Crouching => CROUCH_HEIGHT,
            Stance::Prone => PRONE_HEIGHT,
        }
    }

    /// Get the speed multiplier for this stance.
    pub fn speed_multiplier(&self) -> f32 {
        match self {
            Stance::Standing => 1.0,
            Stance::Crouching => CROUCH_SPEED_MULTIPLIER,
            Stance::Prone => PRONE_SPEED_MULTIPLIER,
        }
    }
}

impl Default for Stance {
    fn default() -> Self {
        Stance::Standing
    }
}

/// Manages player crouching state with smooth height transitions.
///
/// The controller handles:
/// - Toggle-based stance changes (Ctrl toggles crouch)
/// - Smooth height interpolation over 0.15 seconds
/// - Obstacle detection to prevent standing up in tight spaces
/// - Speed multipliers based on current stance
#[derive(Debug, Clone)]
pub struct CrouchController {
    /// Current stance state
    stance: Stance,

    /// Current actual height (may differ from target during transitions)
    current_height: f32,

    /// Target height based on stance
    target_height: f32,

    /// Transition progress (0.0 to 1.0)
    transition_progress: f32,

    /// Height at start of transition
    transition_start_height: f32,

    /// Whether crouch input was pressed last frame (for toggle detection)
    was_crouch_pressed: bool,
}

impl Default for CrouchController {
    fn default() -> Self {
        Self {
            stance: Stance::Standing,
            current_height: STANDING_HEIGHT,
            target_height: STANDING_HEIGHT,
            transition_progress: 1.0,
            transition_start_height: STANDING_HEIGHT,
            was_crouch_pressed: false,
        }
    }
}

impl CrouchController {
    /// Create a new crouch controller in standing stance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a crouch controller with a specific starting stance.
    pub fn with_stance(stance: Stance) -> Self {
        let height = stance.height();
        Self {
            stance,
            current_height: height,
            target_height: height,
            transition_progress: 1.0,
            transition_start_height: height,
            was_crouch_pressed: false,
        }
    }

    /// Get the current stance.
    pub fn stance(&self) -> Stance {
        self.stance
    }

    /// Get the current height in meters.
    ///
    /// This may be between stance heights during transitions.
    pub fn current_height(&self) -> f32 {
        self.current_height
    }

    /// Get the target height based on current stance.
    pub fn target_height(&self) -> f32 {
        self.target_height
    }

    /// Check if currently transitioning between heights.
    pub fn is_transitioning(&self) -> bool {
        self.transition_progress < 1.0
    }

    /// Get the speed multiplier based on current stance.
    ///
    /// Returns:
    /// - 1.0 when standing
    /// - 0.5 when crouching
    /// - 0.25 when prone
    pub fn speed_multiplier(&self) -> f32 {
        self.stance.speed_multiplier()
    }

    /// Check if the player can stand up given the current clearance.
    ///
    /// # Arguments
    /// * `clearance_above` - Available vertical space above the player in meters
    ///
    /// # Returns
    /// `true` if there is enough room to stand (clearance >= standing height)
    pub fn can_stand(&self, clearance_above: f32) -> bool {
        clearance_above >= STANDING_HEIGHT
    }

    /// Check if the player can crouch (transition from prone to crouch) given clearance.
    ///
    /// # Arguments
    /// * `clearance_above` - Available vertical space above the player in meters
    ///
    /// # Returns
    /// `true` if there is enough room to crouch (clearance >= crouch height)
    pub fn can_crouch(&self, clearance_above: f32) -> bool {
        clearance_above >= CROUCH_HEIGHT
    }

    /// Set the stance directly, starting a transition.
    ///
    /// This does not check for obstacles - use `try_set_stance` for safe transitions.
    pub fn set_stance(&mut self, stance: Stance) {
        if self.stance != stance {
            self.stance = stance;
            self.target_height = stance.height();
            self.transition_start_height = self.current_height;
            self.transition_progress = 0.0;
        }
    }

    /// Try to set the stance, checking for obstacles.
    ///
    /// # Arguments
    /// * `stance` - The target stance
    /// * `clearance_above` - Available vertical space above the player
    ///
    /// # Returns
    /// `true` if the stance change was accepted, `false` if blocked by obstacle
    pub fn try_set_stance(&mut self, stance: Stance, clearance_above: f32) -> bool {
        // Going down is always allowed
        if stance.height() <= self.current_height {
            self.set_stance(stance);
            return true;
        }

        // Going up requires clearance check
        if clearance_above >= stance.height() {
            self.set_stance(stance);
            true
        } else {
            false
        }
    }

    /// Update the crouch controller.
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds
    /// * `crouch_pressed` - Whether the crouch key (Ctrl) is currently pressed
    /// * `clearance_above` - Closure that returns the clearance above the player
    ///
    /// # Returns
    /// The current player height in meters
    ///
    /// # Toggle Behavior
    ///
    /// Crouch is a toggle: pressing Ctrl once crouches, pressing again stands (if possible).
    /// This prevents needing to hold the key, which is more ergonomic.
    pub fn update<F>(&mut self, dt: f32, crouch_pressed: bool, clearance_above: F) -> f32
    where
        F: Fn() -> f32,
    {
        // Clamp delta time
        let dt = dt.clamp(0.0001, 0.1);

        // Detect crouch toggle (on key down edge)
        if crouch_pressed && !self.was_crouch_pressed {
            self.handle_crouch_toggle(clearance_above());
        }
        self.was_crouch_pressed = crouch_pressed;

        // Update height transition
        if self.transition_progress < 1.0 {
            self.transition_progress += dt / TRANSITION_DURATION;
            self.transition_progress = self.transition_progress.min(1.0);

            // Smooth interpolation using ease-in-out
            let t = ease_in_out(self.transition_progress);
            self.current_height = lerp(self.transition_start_height, self.target_height, t);
        }

        self.current_height
    }

    /// Handle crouch toggle input.
    ///
    /// Cycles through: Standing -> Crouching -> Standing
    /// (Prone is accessed through extended crouch or separate prone key)
    fn handle_crouch_toggle(&mut self, clearance_above: f32) {
        match self.stance {
            Stance::Standing => {
                // Toggle to crouch (always allowed - going down)
                self.set_stance(Stance::Crouching);
            }
            Stance::Crouching => {
                // Try to stand up (may be blocked by obstacle)
                if self.can_stand(clearance_above) {
                    self.set_stance(Stance::Standing);
                }
                // If blocked, stay crouched (no feedback needed - player will notice)
            }
            Stance::Prone => {
                // Try to crouch first (may be blocked)
                if self.can_crouch(clearance_above) {
                    self.set_stance(Stance::Crouching);
                }
                // If blocked, stay prone
            }
        }
    }

    /// Force transition to prone stance.
    ///
    /// Prone can be accessed via a separate key or double-tap crouch.
    pub fn go_prone(&mut self) {
        self.set_stance(Stance::Prone);
    }

    /// Try to stand up from any stance.
    ///
    /// # Arguments
    /// * `clearance_above` - Available vertical space above the player
    ///
    /// # Returns
    /// `true` if standing was successful, `false` if blocked by obstacle
    pub fn try_stand(&mut self, clearance_above: f32) -> bool {
        self.try_set_stance(Stance::Standing, clearance_above)
    }

    /// Reset to standing stance immediately (no transition).
    ///
    /// Use this for teleportation or respawning.
    pub fn reset(&mut self) {
        self.stance = Stance::Standing;
        self.current_height = STANDING_HEIGHT;
        self.target_height = STANDING_HEIGHT;
        self.transition_progress = 1.0;
        self.transition_start_height = STANDING_HEIGHT;
        self.was_crouch_pressed = false;
    }
}

/// Linear interpolation between two values.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Smooth ease-in-out interpolation (cubic).
fn ease_in_out(t: f32) -> f32 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.001;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_stance_heights() {
        assert!(approx_eq(Stance::Standing.height(), 1.8));
        assert!(approx_eq(Stance::Crouching.height(), 0.9));
        assert!(approx_eq(Stance::Prone.height(), 0.4));
    }

    #[test]
    fn test_stance_speed_multipliers() {
        assert!(approx_eq(Stance::Standing.speed_multiplier(), 1.0));
        assert!(approx_eq(Stance::Crouching.speed_multiplier(), 0.5));
        assert!(approx_eq(Stance::Prone.speed_multiplier(), 0.25));
    }

    #[test]
    fn test_default_controller() {
        let controller = CrouchController::new();
        assert_eq!(controller.stance(), Stance::Standing);
        assert!(approx_eq(controller.current_height(), STANDING_HEIGHT));
        assert!(approx_eq(controller.speed_multiplier(), 1.0));
        assert!(!controller.is_transitioning());
    }

    #[test]
    fn test_with_stance() {
        let controller = CrouchController::with_stance(Stance::Crouching);
        assert_eq!(controller.stance(), Stance::Crouching);
        assert!(approx_eq(controller.current_height(), CROUCH_HEIGHT));
    }

    #[test]
    fn test_toggle_to_crouch() {
        let mut controller = CrouchController::new();

        // Press crouch key
        controller.update(0.016, true, || 10.0);
        assert_eq!(controller.stance(), Stance::Crouching);
        assert!(controller.is_transitioning());
    }

    #[test]
    fn test_toggle_back_to_standing() {
        let mut controller = CrouchController::new();

        // Crouch
        controller.update(0.016, true, || 10.0);
        // Release key
        controller.update(0.016, false, || 10.0);
        // Complete transition
        for _ in 0..20 {
            controller.update(0.016, false, || 10.0);
        }

        // Press again to stand
        controller.update(0.016, true, || 10.0);
        assert_eq!(controller.stance(), Stance::Standing);
    }

    #[test]
    fn test_cannot_stand_with_obstacle() {
        let mut controller = CrouchController::with_stance(Stance::Crouching);

        // Complete any pending transition
        for _ in 0..20 {
            controller.update(0.016, false, || 1.0);
        }

        // Try to stand with low clearance (1.0m < 1.8m standing height)
        controller.update(0.016, true, || 1.0);

        // Should stay crouched
        assert_eq!(controller.stance(), Stance::Crouching);
    }

    #[test]
    fn test_can_stand_with_clearance() {
        let mut controller = CrouchController::with_stance(Stance::Crouching);

        // Complete any pending transition
        for _ in 0..20 {
            controller.update(0.016, false, || 10.0);
        }

        // Try to stand with enough clearance
        controller.update(0.016, true, || 10.0);

        // Should be standing
        assert_eq!(controller.stance(), Stance::Standing);
    }

    #[test]
    fn test_smooth_transition() {
        let mut controller = CrouchController::new();

        // Start crouch
        controller.update(0.001, true, || 10.0);
        let height_during = controller.current_height();

        // Height should be between standing and crouching during transition
        assert!(height_during < STANDING_HEIGHT);
        assert!(height_during > CROUCH_HEIGHT);

        // After full transition time
        for _ in 0..20 {
            controller.update(0.016, false, || 10.0);
        }

        // Should be at crouch height
        assert!(approx_eq(controller.current_height(), CROUCH_HEIGHT));
    }

    #[test]
    fn test_go_prone() {
        let mut controller = CrouchController::new();
        controller.go_prone();

        // Complete transition
        for _ in 0..20 {
            controller.update(0.016, false, || 10.0);
        }

        assert_eq!(controller.stance(), Stance::Prone);
        assert!(approx_eq(controller.current_height(), PRONE_HEIGHT));
        assert!(approx_eq(controller.speed_multiplier(), 0.25));
    }

    #[test]
    fn test_try_stand_from_prone() {
        let mut controller = CrouchController::with_stance(Stance::Prone);

        // Complete initial state
        for _ in 0..20 {
            controller.update(0.016, false, || 10.0);
        }

        // Try to stand with enough clearance
        assert!(controller.try_stand(10.0));
        assert_eq!(controller.stance(), Stance::Standing);
    }

    #[test]
    fn test_try_stand_blocked() {
        let mut controller = CrouchController::with_stance(Stance::Prone);

        // Try to stand with low clearance
        assert!(!controller.try_stand(1.5));
        assert_eq!(controller.stance(), Stance::Prone);
    }

    #[test]
    fn test_reset() {
        let mut controller = CrouchController::with_stance(Stance::Prone);
        controller.reset();

        assert_eq!(controller.stance(), Stance::Standing);
        assert!(approx_eq(controller.current_height(), STANDING_HEIGHT));
        assert!(!controller.is_transitioning());
    }

    #[test]
    fn test_holding_crouch_key_no_rapid_toggle() {
        let mut controller = CrouchController::new();

        // First press - should crouch
        controller.update(0.016, true, || 10.0);
        assert_eq!(controller.stance(), Stance::Crouching);

        // Holding key for many frames should not toggle back
        for _ in 0..100 {
            controller.update(0.016, true, || 10.0);
        }

        // Should still be crouching
        assert_eq!(controller.stance(), Stance::Crouching);
    }

    #[test]
    fn test_can_stand_check() {
        let controller = CrouchController::new();

        assert!(controller.can_stand(2.0));
        assert!(controller.can_stand(1.8));
        assert!(!controller.can_stand(1.7));
        assert!(!controller.can_stand(0.5));
    }

    #[test]
    fn test_can_crouch_check() {
        let controller = CrouchController::new();

        assert!(controller.can_crouch(1.0));
        assert!(controller.can_crouch(0.9));
        assert!(!controller.can_crouch(0.8));
    }

    #[test]
    fn test_transition_duration() {
        let mut controller = CrouchController::new();

        // Start transition to crouch
        controller.update(0.001, true, || 10.0);
        assert!(controller.is_transitioning());

        // Transition should complete within TRANSITION_DURATION total time
        // First update was 0.001s, so we need slightly less than TRANSITION_DURATION more
        // Using multiple small updates to respect the 0.1s dt clamp
        let mut elapsed = 0.001;
        while elapsed < TRANSITION_DURATION && controller.is_transitioning() {
            controller.update(0.05, false, || 10.0);
            elapsed += 0.05;
        }

        // Should complete within expected time (+/- one frame)
        assert!(!controller.is_transitioning());
        assert!(elapsed <= TRANSITION_DURATION + 0.05);
    }
}
