//! Player Movement System
//!
//! Implements smooth player movement with acceleration/deceleration curves,
//! turn rate clamping, sprint support, and camera-relative movement.
//!
//! # Movement Characteristics
//!
//! - Velocity approaches target over ~0.2 seconds (acceleration)
//! - Velocity decreases over ~0.15 seconds when no input (deceleration)
//! - Turn rate is clamped to prevent instant 180° spins
//! - Sprint multiplies target speed by 2x
//! - Movement is relative to camera facing direction
//!
//! # Example
//!
//! ```rust,ignore
//! use magic_engine::game::player::movement::PlayerMovement;
//! use magic_engine::game::player::input::{PlayerInput, MovementDirection};
//! use glam::Vec3;
//!
//! let mut movement = PlayerMovement::new();
//!
//! // Get input from PlayerInput system
//! let direction = player_input.get_movement_direction();
//! let is_sprinting = player_input.is_sprinting();
//!
//! // Camera facing direction (from CameraController)
//! let camera_yaw = 0.5; // radians
//!
//! // Update movement each frame
//! let delta_time = 1.0 / 60.0;
//! movement.update(direction, camera_yaw, is_sprinting, delta_time);
//!
//! // Apply the velocity to player position
//! let velocity = movement.get_velocity();
//! player_position += velocity * delta_time;
//! ```

use glam::Vec3;

use super::input::MovementDirection;

/// Configuration for player movement physics.
///
/// Separate from PlayerPhysics to allow for independent tuning of
/// movement behavior without affecting other physics systems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MovementConfig {
    /// Base walking speed in meters per second.
    pub walk_speed: f32,

    /// Time in seconds to reach target velocity from rest (acceleration ramp-up).
    /// ~0.2 seconds gives a snappy but smooth feel.
    pub acceleration_time: f32,

    /// Time in seconds to stop from full speed when no input (deceleration ramp-down).
    /// ~0.15 seconds gives quick but not instant stop.
    pub deceleration_time: f32,

    /// Maximum turn rate in radians per second.
    /// Prevents instant 180° spins for more natural movement.
    pub max_turn_rate: f32,

    /// Sprint speed multiplier (applied to walk_speed).
    /// 2.0 means sprinting is twice as fast as walking.
    pub sprint_multiplier: f32,
}

impl Default for MovementConfig {
    fn default() -> Self {
        Self {
            walk_speed: 5.0,           // 5 m/s walking speed
            acceleration_time: 0.2,    // ~0.2s to reach target velocity
            deceleration_time: 0.15,   // ~0.15s to stop
            max_turn_rate: std::f32::consts::PI, // 180°/s max turn rate
            sprint_multiplier: 2.0,    // 2x speed when sprinting
        }
    }
}

impl MovementConfig {
    /// Calculate acceleration rate based on acceleration_time.
    ///
    /// Returns units per second squared to reach walk_speed in acceleration_time.
    #[inline]
    pub fn acceleration_rate(&self) -> f32 {
        if self.acceleration_time > 0.0 {
            self.walk_speed / self.acceleration_time
        } else {
            f32::MAX
        }
    }

    /// Calculate deceleration rate based on deceleration_time.
    ///
    /// Returns units per second squared to stop from walk_speed in deceleration_time.
    #[inline]
    pub fn deceleration_rate(&self) -> f32 {
        if self.deceleration_time > 0.0 {
            self.walk_speed / self.deceleration_time
        } else {
            f32::MAX
        }
    }

    /// Get the target speed based on sprinting state.
    #[inline]
    pub fn target_speed(&self, is_sprinting: bool) -> f32 {
        if is_sprinting {
            self.walk_speed * self.sprint_multiplier
        } else {
            self.walk_speed
        }
    }

    /// Calculate acceleration rate adjusted for sprint speed.
    ///
    /// When sprinting, we accelerate faster to maintain the ~0.2s ramp-up time.
    #[inline]
    pub fn effective_acceleration_rate(&self, is_sprinting: bool) -> f32 {
        if self.acceleration_time > 0.0 {
            self.target_speed(is_sprinting) / self.acceleration_time
        } else {
            f32::MAX
        }
    }

    /// Calculate deceleration rate adjusted for current speed.
    ///
    /// When coming from sprint speed, we decelerate faster to maintain ~0.15s stop time.
    #[inline]
    pub fn effective_deceleration_rate(&self, is_sprinting: bool) -> f32 {
        if self.deceleration_time > 0.0 {
            self.target_speed(is_sprinting) / self.deceleration_time
        } else {
            f32::MAX
        }
    }
}

/// Player movement state and physics.
///
/// Handles smooth acceleration/deceleration, turn rate limiting,
/// and camera-relative movement direction.
#[derive(Debug, Clone)]
pub struct PlayerMovement {
    /// Current velocity vector in world space (meters per second).
    velocity: Vec3,

    /// Current facing direction in radians (yaw angle).
    /// This is the direction the player character is facing,
    /// which may differ from the camera direction during turning.
    facing_yaw: f32,

    /// Movement configuration.
    config: MovementConfig,

    /// Whether the player was sprinting last frame.
    /// Used to calculate proper deceleration rate.
    was_sprinting: bool,
}

impl Default for PlayerMovement {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerMovement {
    /// Create a new PlayerMovement with default configuration.
    pub fn new() -> Self {
        Self {
            velocity: Vec3::ZERO,
            facing_yaw: 0.0,
            config: MovementConfig::default(),
            was_sprinting: false,
        }
    }

    /// Create a PlayerMovement with custom configuration.
    pub fn with_config(config: MovementConfig) -> Self {
        Self {
            velocity: Vec3::ZERO,
            facing_yaw: 0.0,
            config,
            was_sprinting: false,
        }
    }

    /// Get the current velocity vector.
    #[inline]
    pub fn get_velocity(&self) -> Vec3 {
        self.velocity
    }

    /// Get the current speed (magnitude of velocity).
    #[inline]
    pub fn get_speed(&self) -> f32 {
        self.velocity.length()
    }

    /// Get the current facing direction in radians.
    #[inline]
    pub fn get_facing_yaw(&self) -> f32 {
        self.facing_yaw
    }

    /// Get the movement configuration.
    #[inline]
    pub fn get_config(&self) -> &MovementConfig {
        &self.config
    }

    /// Set the movement configuration.
    pub fn set_config(&mut self, config: MovementConfig) {
        self.config = config;
    }

    /// Check if the player is currently moving.
    #[inline]
    pub fn is_moving(&self) -> bool {
        self.velocity.length_squared() > 0.0001
    }

    /// Reset velocity to zero (e.g., on landing, collision).
    pub fn stop(&mut self) {
        self.velocity = Vec3::ZERO;
    }

    /// Set the facing direction directly (in radians).
    pub fn set_facing_yaw(&mut self, yaw: f32) {
        self.facing_yaw = yaw;
    }

    /// Update player movement based on input and camera direction.
    ///
    /// This is the main update function that should be called each frame.
    ///
    /// # Arguments
    ///
    /// * `input_direction` - Movement direction from PlayerInput (forward/right, -1 to 1)
    /// * `camera_yaw` - Camera facing direction in radians (from CameraController)
    /// * `is_sprinting` - Whether the sprint key is held
    /// * `delta_time` - Time since last frame in seconds
    pub fn update(
        &mut self,
        input_direction: MovementDirection,
        camera_yaw: f32,
        is_sprinting: bool,
        delta_time: f32,
    ) {
        // Clamp delta_time to prevent huge jumps
        let dt = delta_time.min(0.1);

        if input_direction.is_moving() {
            // Calculate target velocity in world space (camera-relative)
            let target_velocity = self.calculate_target_velocity(
                input_direction,
                camera_yaw,
                is_sprinting,
            );

            // Calculate desired facing direction
            let desired_yaw = target_velocity.x.atan2(-target_velocity.z);

            // Apply turn rate clamping
            self.facing_yaw = self.clamp_turn(self.facing_yaw, desired_yaw, dt);

            // Accelerate towards target velocity
            self.velocity = self.accelerate_towards(
                self.velocity,
                target_velocity,
                is_sprinting,
                dt,
            );

            self.was_sprinting = is_sprinting;
        } else {
            // No input - decelerate to stop
            self.velocity = self.decelerate(self.velocity, dt);
            // Keep facing direction when stopped
        }
    }

    /// Calculate the target velocity in world space based on input and camera direction.
    ///
    /// Movement is relative to where the camera is facing:
    /// - Forward input moves in the camera's forward direction (XZ plane)
    /// - Right input moves to the camera's right
    fn calculate_target_velocity(
        &self,
        input_direction: MovementDirection,
        camera_yaw: f32,
        is_sprinting: bool,
    ) -> Vec3 {
        // Get normalized input direction
        let (input_right, input_forward) = input_direction.normalized();

        // Calculate camera-relative vectors (horizontal plane only)
        let cam_forward = Vec3::new(camera_yaw.sin(), 0.0, -camera_yaw.cos());
        let cam_right = Vec3::new(camera_yaw.cos(), 0.0, camera_yaw.sin());

        // Combine input with camera direction
        let world_direction = cam_forward * input_forward + cam_right * input_right;

        // Normalize and apply target speed
        let target_speed = self.config.target_speed(is_sprinting);

        if world_direction.length_squared() > 0.0001 {
            world_direction.normalize() * target_speed
        } else {
            Vec3::ZERO
        }
    }

    /// Clamp the turn from current_yaw towards desired_yaw by max_turn_rate.
    fn clamp_turn(&self, current_yaw: f32, desired_yaw: f32, delta_time: f32) -> f32 {
        // Calculate the shortest angle difference
        let mut angle_diff = desired_yaw - current_yaw;

        // Normalize to [-PI, PI]
        while angle_diff > std::f32::consts::PI {
            angle_diff -= 2.0 * std::f32::consts::PI;
        }
        while angle_diff < -std::f32::consts::PI {
            angle_diff += 2.0 * std::f32::consts::PI;
        }

        // Calculate maximum turn this frame
        let max_turn = self.config.max_turn_rate * delta_time;

        // Clamp the turn
        let clamped_turn = angle_diff.clamp(-max_turn, max_turn);

        // Apply turn
        current_yaw + clamped_turn
    }

    /// Accelerate velocity towards target over ~0.2 seconds.
    fn accelerate_towards(
        &self,
        current: Vec3,
        target: Vec3,
        is_sprinting: bool,
        delta_time: f32,
    ) -> Vec3 {
        let diff = target - current;
        let diff_length = diff.length();

        if diff_length < 0.0001 {
            return target;
        }

        // Calculate acceleration for this frame
        let accel_rate = self.config.effective_acceleration_rate(is_sprinting);
        let max_change = accel_rate * delta_time;

        if diff_length <= max_change {
            // Close enough, snap to target
            target
        } else {
            // Accelerate towards target
            current + diff.normalize() * max_change
        }
    }

    /// Decelerate velocity to zero over ~0.15 seconds.
    fn decelerate(&self, current: Vec3, delta_time: f32) -> Vec3 {
        let speed = current.length();

        if speed < 0.0001 {
            return Vec3::ZERO;
        }

        // Use deceleration rate based on whether we were sprinting
        let decel_rate = self.config.effective_deceleration_rate(self.was_sprinting);
        let speed_reduction = decel_rate * delta_time;

        if speed <= speed_reduction {
            // Close enough, snap to zero
            Vec3::ZERO
        } else {
            // Maintain direction while reducing speed
            current.normalize() * (speed - speed_reduction)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_movement_config_default() {
        let config = MovementConfig::default();
        assert_eq!(config.walk_speed, 5.0);
        assert_eq!(config.acceleration_time, 0.2);
        assert_eq!(config.deceleration_time, 0.15);
        assert_eq!(config.sprint_multiplier, 2.0);
    }

    #[test]
    fn test_acceleration_rate() {
        let config = MovementConfig::default();
        // 5.0 m/s / 0.2s = 25 m/s²
        assert_eq!(config.acceleration_rate(), 25.0);
    }

    #[test]
    fn test_deceleration_rate() {
        let config = MovementConfig::default();
        // 5.0 m/s / 0.15s = 33.33... m/s²
        assert!((config.deceleration_rate() - 33.333).abs() < 0.1);
    }

    #[test]
    fn test_target_speed() {
        let config = MovementConfig::default();
        assert_eq!(config.target_speed(false), 5.0);
        assert_eq!(config.target_speed(true), 10.0); // 2x sprint
    }

    #[test]
    fn test_player_movement_new() {
        let movement = PlayerMovement::new();
        assert_eq!(movement.get_velocity(), Vec3::ZERO);
        assert_eq!(movement.get_speed(), 0.0);
        assert!(!movement.is_moving());
    }

    #[test]
    fn test_forward_movement() {
        let mut movement = PlayerMovement::new();
        let direction = MovementDirection {
            forward: 1.0,
            right: 0.0,
        };

        // Update for several frames
        for _ in 0..30 {
            movement.update(direction, 0.0, false, 1.0 / 60.0);
        }

        // Should be moving at approximately walk speed in the -Z direction
        let vel = movement.get_velocity();
        assert!(vel.z < -4.0); // Close to -5.0
        assert!(vel.x.abs() < 0.01); // Not moving sideways
    }

    #[test]
    fn test_camera_relative_movement() {
        let mut movement = PlayerMovement::new();
        let direction = MovementDirection {
            forward: 1.0,
            right: 0.0,
        };

        // Camera facing 90° right (looking down +X axis)
        let camera_yaw = std::f32::consts::FRAC_PI_2;

        for _ in 0..30 {
            movement.update(direction, camera_yaw, false, 1.0 / 60.0);
        }

        // Forward relative to this camera should move in +X direction
        let vel = movement.get_velocity();
        assert!(vel.x > 4.0); // Moving in +X
        assert!(vel.z.abs() < 0.5); // Not much Z movement
    }

    #[test]
    fn test_sprint_doubles_speed() {
        let mut movement = PlayerMovement::new();
        let direction = MovementDirection {
            forward: 1.0,
            right: 0.0,
        };

        // Update for several frames with sprint
        for _ in 0..50 {
            movement.update(direction, 0.0, true, 1.0 / 60.0);
        }

        // Should be moving at approximately sprint speed
        let speed = movement.get_speed();
        assert!(speed > 9.0); // Close to 10.0 (2x walk speed)
    }

    #[test]
    fn test_acceleration_time() {
        let mut movement = PlayerMovement::new();
        let direction = MovementDirection {
            forward: 1.0,
            right: 0.0,
        };

        // After ~0.1s (half of acceleration time), should be at roughly half speed
        for _ in 0..6 {
            movement.update(direction, 0.0, false, 1.0 / 60.0);
        }

        let speed_at_half = movement.get_speed();

        // Continue to ~0.2s
        for _ in 0..6 {
            movement.update(direction, 0.0, false, 1.0 / 60.0);
        }

        let speed_at_full = movement.get_speed();

        // Speed should be increasing
        assert!(speed_at_full > speed_at_half);
        // Should be close to target speed at ~0.2s
        assert!(speed_at_full > 4.0);
    }

    #[test]
    fn test_deceleration_time() {
        let mut movement = PlayerMovement::new();
        let direction = MovementDirection {
            forward: 1.0,
            right: 0.0,
        };

        // First get up to speed
        for _ in 0..30 {
            movement.update(direction, 0.0, false, 1.0 / 60.0);
        }

        let initial_speed = movement.get_speed();
        assert!(initial_speed > 4.5);

        // Now release input and decelerate
        let no_input = MovementDirection::zero();

        // After ~0.075s (half of deceleration time), should be at roughly half speed
        for _ in 0..5 {
            movement.update(no_input, 0.0, false, 1.0 / 60.0);
        }

        let speed_at_half = movement.get_speed();
        assert!(speed_at_half < initial_speed);
        assert!(speed_at_half > 0.5);

        // After ~0.15s total, should be nearly stopped
        for _ in 0..10 {
            movement.update(no_input, 0.0, false, 1.0 / 60.0);
        }

        let final_speed = movement.get_speed();
        assert!(final_speed < 1.0);
    }

    #[test]
    fn test_turn_rate_clamping() {
        let mut movement = PlayerMovement::new();

        // Start facing forward (yaw = 0)
        movement.set_facing_yaw(0.0);

        // Try to move backward (should require 180° turn)
        let direction = MovementDirection {
            forward: -1.0,
            right: 0.0,
        };

        // Update for one frame at 60 FPS
        movement.update(direction, 0.0, false, 1.0 / 60.0);

        // With max_turn_rate of PI (180°/s), one frame allows PI/60 radians turn
        let max_turn_per_frame = std::f32::consts::PI / 60.0;
        let actual_turn = movement.get_facing_yaw().abs();

        // The turn should be clamped
        assert!(actual_turn <= max_turn_per_frame + 0.001);
        assert!(actual_turn < std::f32::consts::PI); // Did not instantly turn 180°
    }

    #[test]
    fn test_diagonal_movement_normalized() {
        let mut movement = PlayerMovement::new();
        let direction = MovementDirection {
            forward: 1.0,
            right: 1.0,
        };

        for _ in 0..30 {
            movement.update(direction, 0.0, false, 1.0 / 60.0);
        }

        // Diagonal movement should not be faster than straight movement
        let speed = movement.get_speed();
        assert!(speed <= 5.1); // Walk speed + small tolerance
    }

    #[test]
    fn test_stop_immediately() {
        let mut movement = PlayerMovement::new();
        let direction = MovementDirection {
            forward: 1.0,
            right: 0.0,
        };

        for _ in 0..30 {
            movement.update(direction, 0.0, false, 1.0 / 60.0);
        }

        assert!(movement.is_moving());

        movement.stop();

        assert!(!movement.is_moving());
        assert_eq!(movement.get_velocity(), Vec3::ZERO);
    }

    #[test]
    fn test_strafe_movement() {
        let mut movement = PlayerMovement::new();
        let direction = MovementDirection {
            forward: 0.0,
            right: 1.0,
        };

        for _ in 0..30 {
            movement.update(direction, 0.0, false, 1.0 / 60.0);
        }

        let vel = movement.get_velocity();
        // Strafing right with camera facing -Z should move in +X
        assert!(vel.x > 4.0);
        assert!(vel.z.abs() < 0.5);
    }
}
