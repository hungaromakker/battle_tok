//! Player physics constants and configuration.
//!
//! This module defines the physics parameters for player movement,
//! including walking, sprinting, jumping, and turning behavior.

/// Physics constants for player character movement.
///
/// All values are configurable via struct fields, allowing for
/// different player types or power-ups that modify movement.
///
/// # Example
///
/// ```ignore
/// use magic_engine::game::player::physics::PlayerPhysics;
///
/// // Use default physics
/// let physics = PlayerPhysics::default();
///
/// // Custom physics for a faster character
/// let fast_physics = PlayerPhysics {
///     walk_speed: 7.0,
///     sprint_speed: 14.0,
///     ..PlayerPhysics::default()
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayerPhysics {
    /// Walking speed in units per second.
    pub walk_speed: f32,

    /// Sprinting speed in units per second.
    pub sprint_speed: f32,

    /// Acceleration rate in units per second squared.
    /// How quickly the player reaches target speed.
    pub acceleration: f32,

    /// Deceleration rate in units per second squared.
    /// How quickly the player slows down when stopping.
    pub deceleration: f32,

    /// Maximum turn rate in degrees per second.
    /// Prevents instant spinning for more natural movement.
    pub max_turn_rate: f32,

    /// Gravity acceleration in units per second squared.
    /// Applied when the player is airborne.
    pub gravity: f32,

    /// Initial upward velocity when jumping in units per second.
    pub jump_velocity: f32,
}

impl Default for PlayerPhysics {
    fn default() -> Self {
        Self {
            walk_speed: 5.0,        // 5.0 units/sec
            sprint_speed: 10.0,     // 10.0 units/sec
            acceleration: 20.0,     // 20.0 units/sec²
            deceleration: 15.0,     // 15.0 units/sec²
            max_turn_rate: 180.0,   // 180°/sec
            gravity: 20.0,          // 20.0 units/sec²
            jump_velocity: 8.0,     // 8.0 units/sec
        }
    }
}

impl PlayerPhysics {
    /// Creates a new PlayerPhysics with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates PlayerPhysics with custom values.
    pub fn with_values(
        walk_speed: f32,
        sprint_speed: f32,
        acceleration: f32,
        deceleration: f32,
        max_turn_rate: f32,
        gravity: f32,
        jump_velocity: f32,
    ) -> Self {
        Self {
            walk_speed,
            sprint_speed,
            acceleration,
            deceleration,
            max_turn_rate,
            gravity,
            jump_velocity,
        }
    }

    /// Returns the current movement speed based on whether sprinting.
    pub fn get_speed(&self, is_sprinting: bool) -> f32 {
        if is_sprinting {
            self.sprint_speed
        } else {
            self.walk_speed
        }
    }

    /// Calculates the clamped turn amount for a frame.
    ///
    /// # Arguments
    /// * `desired_turn` - The desired turn in degrees
    /// * `delta_time` - Time elapsed since last frame in seconds
    ///
    /// # Returns
    /// The clamped turn amount that respects max_turn_rate.
    pub fn clamp_turn(&self, desired_turn: f32, delta_time: f32) -> f32 {
        let max_turn = self.max_turn_rate * delta_time;
        desired_turn.clamp(-max_turn, max_turn)
    }

    /// Calculates new velocity after applying acceleration/deceleration.
    ///
    /// # Arguments
    /// * `current_velocity` - Current velocity magnitude
    /// * `target_velocity` - Desired velocity magnitude
    /// * `delta_time` - Time elapsed since last frame in seconds
    ///
    /// # Returns
    /// The new velocity after acceleration/deceleration is applied.
    pub fn apply_acceleration(&self, current_velocity: f32, target_velocity: f32, delta_time: f32) -> f32 {
        if target_velocity > current_velocity {
            // Accelerating
            let velocity_change = self.acceleration * delta_time;
            (current_velocity + velocity_change).min(target_velocity)
        } else {
            // Decelerating
            let velocity_change = self.deceleration * delta_time;
            (current_velocity - velocity_change).max(target_velocity)
        }
    }

    /// Calculates new vertical velocity after applying gravity.
    ///
    /// # Arguments
    /// * `current_vertical_velocity` - Current vertical velocity (positive = up)
    /// * `delta_time` - Time elapsed since last frame in seconds
    ///
    /// # Returns
    /// The new vertical velocity after gravity is applied.
    pub fn apply_gravity(&self, current_vertical_velocity: f32, delta_time: f32) -> f32 {
        current_vertical_velocity - self.gravity * delta_time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let physics = PlayerPhysics::default();
        assert_eq!(physics.walk_speed, 5.0);
        assert_eq!(physics.sprint_speed, 10.0);
        assert_eq!(physics.acceleration, 20.0);
        assert_eq!(physics.deceleration, 15.0);
        assert_eq!(physics.max_turn_rate, 180.0);
        assert_eq!(physics.gravity, 20.0);
        assert_eq!(physics.jump_velocity, 8.0);
    }

    #[test]
    fn test_get_speed() {
        let physics = PlayerPhysics::default();
        assert_eq!(physics.get_speed(false), 5.0);
        assert_eq!(physics.get_speed(true), 10.0);
    }

    #[test]
    fn test_clamp_turn() {
        let physics = PlayerPhysics::default();

        // At 180°/sec max turn rate, 0.5 sec allows 90° turn
        let delta_time = 0.5;
        let max_allowed = 90.0;

        // Turn within limits
        assert_eq!(physics.clamp_turn(45.0, delta_time), 45.0);

        // Turn exceeds positive limit
        assert_eq!(physics.clamp_turn(120.0, delta_time), max_allowed);

        // Turn exceeds negative limit
        assert_eq!(physics.clamp_turn(-120.0, delta_time), -max_allowed);
    }

    #[test]
    fn test_apply_acceleration() {
        let physics = PlayerPhysics::default();
        let delta_time = 0.1;

        // Accelerating from 0 to 5
        let new_vel = physics.apply_acceleration(0.0, 5.0, delta_time);
        assert_eq!(new_vel, 2.0); // 20.0 * 0.1 = 2.0

        // Decelerating from 5 to 0
        let new_vel = physics.apply_acceleration(5.0, 0.0, delta_time);
        assert_eq!(new_vel, 3.5); // 5.0 - (15.0 * 0.1) = 3.5
    }

    #[test]
    fn test_apply_gravity() {
        let physics = PlayerPhysics::default();
        let delta_time = 0.1;

        // Starting with upward velocity
        let new_vel = physics.apply_gravity(8.0, delta_time);
        assert_eq!(new_vel, 6.0); // 8.0 - (20.0 * 0.1) = 6.0
    }
}
