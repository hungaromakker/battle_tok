//! Player Movement Controller
//!
//! Provides a physics-based movement controller for player characters.
//! Movement direction is relative to camera facing direction.
//!
//! # Physics Model
//!
//! - Walk speed: 5.0 m/s
//! - Sprint speed: 10.0 m/s
//! - Acceleration: 50.0 m/s^2
//! - Deceleration: 30.0 m/s^2
//!
//! # Gravity Modes
//!
//! - **Flat mode (default)**: Standard gravity pointing in -Y direction
//! - **Spherical mode**: Radial gravity toward planet center, movement tangent to surface
//!
//! # Usage
//!
//! ```rust,ignore
//! use battle_tok_engine::player::PlayerMovementController;
//! use battle_tok_engine::input::MovementKeys;
//! use glam::Vec3;
//!
//! let mut controller = PlayerMovementController::new();
//!
//! // Optional: Enable spherical gravity mode
//! controller.set_spherical_mode(Vec3::ZERO, 1000.0); // Planet at origin, radius 1000m
//!
//! // Each frame:
//! let velocity = controller.update(delta_time, &movement_input, camera_yaw);
//! player_position += velocity * delta_time;
//! ```

use glam::Vec3;

use crate::input::MovementKeys;

/// Walk speed in meters per second
pub const WALK_SPEED: f32 = 5.0;

/// Sprint speed in meters per second
pub const SPRINT_SPEED: f32 = 10.0;

/// Acceleration in meters per second squared
pub const ACCELERATION: f32 = 50.0;

/// Deceleration in meters per second squared
pub const DECELERATION: f32 = 30.0;

/// Jump velocity in meters per second
pub const JUMP_VELOCITY: f32 = 8.0;

/// Gravity acceleration in meters per second squared
pub const GRAVITY: f32 = 20.0;

/// Coyote time duration in seconds
/// Allows jumping shortly after leaving ground
pub const COYOTE_TIME: f32 = 0.1;

/// Configuration for spherical gravity mode.
///
/// In spherical mode:
/// - "Down" points toward the planet center
/// - "Up" points away from the planet center (surface normal)
/// - Movement is tangent to the sphere surface
/// - Jump direction follows the surface normal
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphericalGravityConfig {
    /// Center of the planet in world coordinates
    pub planet_center: Vec3,
    /// Radius of the planet surface in meters
    pub planet_radius: f32,
}

impl SphericalGravityConfig {
    /// Create a new spherical gravity configuration.
    ///
    /// # Arguments
    /// * `planet_center` - World position of the planet center
    /// * `planet_radius` - Radius of the planet surface in meters
    pub fn new(planet_center: Vec3, planet_radius: f32) -> Self {
        Self {
            planet_center,
            planet_radius,
        }
    }

    /// Calculate the up direction (surface normal) at a given position.
    ///
    /// Returns the normalized vector from planet center to the position.
    /// If the position is at the planet center, returns Vec3::Y as fallback.
    pub fn get_up_at(&self, position: Vec3) -> Vec3 {
        let to_position = position - self.planet_center;
        if to_position.length_squared() < 0.0001 {
            Vec3::Y // Fallback if at center
        } else {
            to_position.normalize()
        }
    }

    /// Calculate the down direction (toward planet center) at a given position.
    ///
    /// Returns the normalized vector from the position to planet center.
    pub fn get_down_at(&self, position: Vec3) -> Vec3 {
        -self.get_up_at(position)
    }

    /// Calculate the surface height (distance from planet center) at a position.
    ///
    /// Returns how far above/below the surface the position is.
    /// Positive = above surface, negative = below surface.
    pub fn get_height_above_surface(&self, position: Vec3) -> f32 {
        let distance_from_center = (position - self.planet_center).length();
        distance_from_center - self.planet_radius
    }

    /// Calculate the surface position directly below/above the given position.
    ///
    /// Projects the position onto the sphere surface along the radial direction.
    pub fn get_surface_position(&self, position: Vec3) -> Vec3 {
        let up = self.get_up_at(position);
        self.planet_center + up * self.planet_radius
    }
}

/// Gravity mode for the movement controller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GravityMode {
    /// Standard flat-world gravity pointing in -Y direction.
    Flat,
    /// Spherical planet gravity pointing toward planet center.
    Spherical(SphericalGravityConfig),
}

impl Default for GravityMode {
    fn default() -> Self {
        Self::Flat
    }
}

/// Player movement controller with camera-relative movement and smooth acceleration.
///
/// WASD movement is relative to the camera's facing direction:
/// - W/S: Move forward/backward in camera's look direction (XZ plane)
/// - A/D: Strafe left/right relative to camera
///
/// Movement uses acceleration and deceleration for smooth starts and stops.
/// Includes jump and gravity support with coyote time for responsive jumping.
#[derive(Debug, Clone)]
pub struct PlayerMovementController {
    /// Current velocity in world space (meters per second)
    velocity: Vec3,

    /// Walk speed in m/s (default: 5.0)
    walk_speed: f32,

    /// Sprint speed in m/s (default: 10.0)
    sprint_speed: f32,

    /// Acceleration rate in m/s^2 (default: 50.0)
    acceleration: f32,

    /// Deceleration rate in m/s^2 (default: 30.0)
    deceleration: f32,

    /// Current vertical velocity in m/s (positive = upward)
    vertical_velocity: f32,

    /// Whether the player is currently on the ground
    is_grounded: bool,

    /// Time remaining for coyote time (allows jumping shortly after leaving ground)
    coyote_time_remaining: f32,

    /// Jump velocity in m/s (default: 8.0)
    jump_velocity: f32,

    /// Gravity acceleration in m/s^2 (default: 20.0)
    gravity: f32,

    /// Gravity mode (flat or spherical)
    gravity_mode: GravityMode,

    /// Player position (needed for spherical gravity calculations)
    player_position: Vec3,
}

impl Default for PlayerMovementController {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            walk_speed: WALK_SPEED,
            sprint_speed: SPRINT_SPEED,
            acceleration: ACCELERATION,
            deceleration: DECELERATION,
            vertical_velocity: 0.0,
            is_grounded: true,
            coyote_time_remaining: 0.0,
            jump_velocity: JUMP_VELOCITY,
            gravity: GRAVITY,
            gravity_mode: GravityMode::Flat,
            player_position: Vec3::ZERO,
        }
    }
}

impl PlayerMovementController {
    /// Create a new movement controller with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a movement controller with custom speed settings.
    ///
    /// # Arguments
    /// * `walk_speed` - Walking speed in m/s
    /// * `sprint_speed` - Sprinting speed in m/s
    pub fn with_speeds(walk_speed: f32, sprint_speed: f32) -> Self {
        Self {
            walk_speed,
            sprint_speed,
            ..Default::default()
        }
    }

    /// Create a movement controller with custom physics settings.
    ///
    /// # Arguments
    /// * `walk_speed` - Walking speed in m/s
    /// * `sprint_speed` - Sprinting speed in m/s
    /// * `acceleration` - Acceleration rate in m/s^2
    /// * `deceleration` - Deceleration rate in m/s^2
    pub fn with_physics(
        walk_speed: f32,
        sprint_speed: f32,
        acceleration: f32,
        deceleration: f32,
    ) -> Self {
        Self {
            velocity: Vec3::ZERO,
            walk_speed,
            sprint_speed,
            acceleration,
            deceleration,
            vertical_velocity: 0.0,
            is_grounded: true,
            coyote_time_remaining: 0.0,
            jump_velocity: JUMP_VELOCITY,
            gravity: GRAVITY,
            gravity_mode: GravityMode::Flat,
            player_position: Vec3::ZERO,
        }
    }

    /// Create a movement controller with full custom settings including jump/gravity.
    ///
    /// # Arguments
    /// * `walk_speed` - Walking speed in m/s
    /// * `sprint_speed` - Sprinting speed in m/s
    /// * `acceleration` - Acceleration rate in m/s^2
    /// * `deceleration` - Deceleration rate in m/s^2
    /// * `jump_velocity` - Jump velocity in m/s
    /// * `gravity` - Gravity acceleration in m/s^2
    pub fn with_full_physics(
        walk_speed: f32,
        sprint_speed: f32,
        acceleration: f32,
        deceleration: f32,
        jump_velocity: f32,
        gravity: f32,
    ) -> Self {
        Self {
            velocity: Vec3::ZERO,
            walk_speed,
            sprint_speed,
            acceleration,
            deceleration,
            vertical_velocity: 0.0,
            is_grounded: true,
            coyote_time_remaining: 0.0,
            jump_velocity,
            gravity,
            gravity_mode: GravityMode::Flat,
            player_position: Vec3::ZERO,
        }
    }

    /// Get the current velocity in world space.
    pub fn get_velocity(&self) -> Vec3 {
        self.velocity
    }

    /// Get the current speed (magnitude of velocity).
    pub fn get_speed(&self) -> f32 {
        self.velocity.length()
    }

    /// Get the horizontal speed (XZ plane only).
    pub fn get_horizontal_speed(&self) -> f32 {
        Vec3::new(self.velocity.x, 0.0, self.velocity.z).length()
    }

    /// Get the walk speed setting.
    pub fn get_walk_speed(&self) -> f32 {
        self.walk_speed
    }

    /// Set the walk speed.
    pub fn set_walk_speed(&mut self, speed: f32) {
        self.walk_speed = speed;
    }

    /// Get the sprint speed setting.
    pub fn get_sprint_speed(&self) -> f32 {
        self.sprint_speed
    }

    /// Set the sprint speed.
    pub fn set_sprint_speed(&mut self, speed: f32) {
        self.sprint_speed = speed;
    }

    /// Get the acceleration rate.
    pub fn get_acceleration(&self) -> f32 {
        self.acceleration
    }

    /// Set the acceleration rate.
    pub fn set_acceleration(&mut self, acceleration: f32) {
        self.acceleration = acceleration;
    }

    /// Get the deceleration rate.
    pub fn get_deceleration(&self) -> f32 {
        self.deceleration
    }

    /// Set the deceleration rate.
    pub fn set_deceleration(&mut self, deceleration: f32) {
        self.deceleration = deceleration;
    }

    /// Get the current vertical velocity.
    pub fn get_vertical_velocity(&self) -> f32 {
        self.vertical_velocity
    }

    /// Set the vertical velocity directly.
    pub fn set_vertical_velocity(&mut self, velocity: f32) {
        self.vertical_velocity = velocity;
    }

    /// Check if the player is currently grounded.
    pub fn is_grounded(&self) -> bool {
        self.is_grounded
    }

    /// Set the grounded state directly.
    pub fn set_grounded(&mut self, grounded: bool) {
        self.is_grounded = grounded;
        if grounded {
            self.coyote_time_remaining = COYOTE_TIME;
        }
    }

    /// Get the remaining coyote time.
    pub fn get_coyote_time_remaining(&self) -> f32 {
        self.coyote_time_remaining
    }

    /// Check if the player can currently jump (grounded or within coyote time).
    pub fn can_jump(&self) -> bool {
        self.is_grounded || self.coyote_time_remaining > 0.0
    }

    /// Get the jump velocity setting.
    pub fn get_jump_velocity(&self) -> f32 {
        self.jump_velocity
    }

    /// Set the jump velocity.
    pub fn set_jump_velocity(&mut self, velocity: f32) {
        self.jump_velocity = velocity;
    }

    /// Get the gravity setting.
    pub fn get_gravity(&self) -> f32 {
        self.gravity
    }

    /// Set the gravity.
    pub fn set_gravity(&mut self, gravity: f32) {
        self.gravity = gravity;
    }

    /// Enable spherical gravity mode.
    ///
    /// In spherical mode:
    /// - "Down" points toward the planet center
    /// - "Up" points away from the planet center (surface normal)
    /// - Movement stays tangent to the sphere surface
    /// - Jump direction follows the surface normal
    ///
    /// # Arguments
    /// * `planet_center` - World position of the planet center
    /// * `planet_radius` - Radius of the planet surface in meters
    ///
    /// # Example
    /// ```rust,ignore
    /// // Planet centered at origin with 1000m radius
    /// controller.set_spherical_mode(Vec3::ZERO, 1000.0);
    /// ```
    pub fn set_spherical_mode(&mut self, planet_center: Vec3, planet_radius: f32) {
        self.gravity_mode =
            GravityMode::Spherical(SphericalGravityConfig::new(planet_center, planet_radius));
    }

    /// Disable spherical gravity mode and return to flat mode.
    ///
    /// In flat mode, gravity points in the -Y direction.
    pub fn set_flat_mode(&mut self) {
        self.gravity_mode = GravityMode::Flat;
    }

    /// Get the current gravity mode.
    pub fn get_gravity_mode(&self) -> &GravityMode {
        &self.gravity_mode
    }

    /// Check if the controller is in spherical gravity mode.
    pub fn is_spherical_mode(&self) -> bool {
        matches!(self.gravity_mode, GravityMode::Spherical(_))
    }

    /// Get the spherical gravity configuration, if in spherical mode.
    pub fn get_spherical_config(&self) -> Option<&SphericalGravityConfig> {
        match &self.gravity_mode {
            GravityMode::Spherical(config) => Some(config),
            GravityMode::Flat => None,
        }
    }

    /// Set the player position (required for spherical gravity calculations).
    ///
    /// This should be called each frame with the current player position.
    pub fn set_player_position(&mut self, position: Vec3) {
        self.player_position = position;
    }

    /// Get the current player position.
    pub fn get_player_position(&self) -> Vec3 {
        self.player_position
    }

    /// Get the "up" direction at the current player position.
    ///
    /// - Flat mode: Always Vec3::Y (world up)
    /// - Spherical mode: Direction from planet center to player (surface normal)
    pub fn get_up_direction(&self) -> Vec3 {
        match &self.gravity_mode {
            GravityMode::Flat => Vec3::Y,
            GravityMode::Spherical(config) => config.get_up_at(self.player_position),
        }
    }

    /// Get the "down" direction at the current player position.
    ///
    /// - Flat mode: Always -Vec3::Y (world down)
    /// - Spherical mode: Direction from player to planet center
    pub fn get_down_direction(&self) -> Vec3 {
        match &self.gravity_mode {
            GravityMode::Flat => -Vec3::Y,
            GravityMode::Spherical(config) => config.get_down_at(self.player_position),
        }
    }

    /// Get the jump direction at the current player position.
    ///
    /// Jump direction is always "up" (surface normal in spherical mode).
    pub fn get_jump_direction(&self) -> Vec3 {
        self.get_up_direction()
    }

    /// Reset velocity to zero (including vertical velocity).
    pub fn reset(&mut self) {
        self.velocity = Vec3::ZERO;
        self.vertical_velocity = 0.0;
        self.is_grounded = true;
        self.coyote_time_remaining = 0.0;
    }

    /// Set velocity directly (e.g., for teleportation or knockback).
    pub fn set_velocity(&mut self, velocity: Vec3) {
        self.velocity = velocity;
    }

    /// Attempt to jump. Returns true if jump was initiated, false if not possible.
    ///
    /// Jump is allowed when grounded or within coyote time (0.1s after leaving ground).
    /// Sets vertical_velocity to jump_velocity and clears grounded state.
    ///
    /// # Returns
    /// `true` if jump was initiated, `false` if player cannot jump (already airborne)
    ///
    /// # Example
    /// ```rust,ignore
    /// if input.jump_pressed {
    ///     controller.apply_jump();
    /// }
    /// ```
    pub fn apply_jump(&mut self) -> bool {
        if self.can_jump() {
            self.vertical_velocity = self.jump_velocity;
            self.is_grounded = false;
            self.coyote_time_remaining = 0.0;
            true
        } else {
            false
        }
    }

    /// Apply gravity and update vertical position.
    ///
    /// Updates vertical_velocity based on gravity, then calculates new Y position.
    /// Handles landing detection when player reaches or goes below ground_height.
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds
    /// * `ground_height` - The Y coordinate of the ground at player's XZ position
    ///
    /// # Returns
    /// The change in Y position (delta Y) to apply to the player's position
    ///
    /// # Example
    /// ```rust,ignore
    /// let ground_y = terrain.get_height_at(player.x, player.z);
    /// let delta_y = controller.apply_gravity(delta_time, ground_y);
    /// player.position.y += delta_y;
    /// ```
    pub fn apply_gravity(&mut self, dt: f32, _ground_height: f32) -> f32 {
        // Clamp delta time to prevent physics explosions
        let dt = dt.clamp(0.0001, 0.1);

        // Store previous vertical velocity for midpoint integration
        let prev_vertical_velocity = self.vertical_velocity;

        // Apply gravity to velocity
        self.vertical_velocity -= self.gravity * dt;

        // Calculate position change using average velocity (midpoint method)
        // This provides better accuracy than Euler integration
        let avg_velocity = (prev_vertical_velocity + self.vertical_velocity) * 0.5;
        let delta_y = avg_velocity * dt;

        // Update coyote time when not grounded
        if !self.is_grounded {
            self.coyote_time_remaining = (self.coyote_time_remaining - dt).max(0.0);
        }

        delta_y
    }

    /// Update grounded state based on current position and ground height.
    ///
    /// Call this after applying delta_y to the player position.
    ///
    /// # Arguments
    /// * `player_y` - Current Y position of the player
    /// * `ground_height` - The Y coordinate of the ground at player's XZ position
    ///
    /// # Returns
    /// Corrected Y position (clamped to ground if needed)
    pub fn update_grounded_state(&mut self, player_y: f32, ground_height: f32) -> f32 {
        if player_y <= ground_height {
            // Landed on ground
            self.is_grounded = true;
            self.vertical_velocity = 0.0;
            self.coyote_time_remaining = COYOTE_TIME;
            ground_height
        } else {
            // Airborne
            if self.is_grounded {
                // Just left ground - start coyote time
                self.is_grounded = false;
                self.coyote_time_remaining = COYOTE_TIME;
            }
            player_y
        }
    }

    /// Apply gravity in spherical mode and return position delta.
    ///
    /// In spherical mode, gravity pulls toward the planet center.
    /// The vertical velocity is applied along the radial direction.
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds
    ///
    /// # Returns
    /// The position delta vector to apply to the player position
    ///
    /// # Example
    /// ```rust,ignore
    /// controller.set_player_position(player_position);
    /// let delta = controller.apply_gravity_spherical(delta_time);
    /// player_position += delta;
    /// ```
    pub fn apply_gravity_spherical(&mut self, dt: f32) -> Vec3 {
        // Clamp delta time to prevent physics explosions
        let dt = dt.clamp(0.0001, 0.1);

        // Store previous vertical velocity for midpoint integration
        let prev_vertical_velocity = self.vertical_velocity;

        // Apply gravity to velocity (in radial direction)
        self.vertical_velocity -= self.gravity * dt;

        // Calculate position change using average velocity (midpoint method)
        let avg_velocity = (prev_vertical_velocity + self.vertical_velocity) * 0.5;
        let delta_magnitude = avg_velocity * dt;

        // Update coyote time when not grounded
        if !self.is_grounded {
            self.coyote_time_remaining = (self.coyote_time_remaining - dt).max(0.0);
        }

        // Return delta along the up direction (radial outward)
        self.get_up_direction() * delta_magnitude
    }

    /// Update grounded state for spherical mode based on height above surface.
    ///
    /// Call this after applying the gravity delta to the player position.
    ///
    /// # Arguments
    /// * `new_position` - The new player position after applying gravity
    ///
    /// # Returns
    /// Corrected position (clamped to surface if below)
    pub fn update_grounded_state_spherical(&mut self, new_position: Vec3) -> Vec3 {
        match &self.gravity_mode {
            GravityMode::Flat => {
                // Fallback to flat mode behavior - treat Y as height
                let corrected_y = self.update_grounded_state(new_position.y, 0.0);
                Vec3::new(new_position.x, corrected_y, new_position.z)
            }
            GravityMode::Spherical(config) => {
                let height_above_surface = config.get_height_above_surface(new_position);

                if height_above_surface <= 0.0 {
                    // Landed on surface
                    self.is_grounded = true;
                    self.vertical_velocity = 0.0;
                    self.coyote_time_remaining = COYOTE_TIME;
                    // Return position clamped to surface
                    config.get_surface_position(new_position)
                } else {
                    // Airborne
                    if self.is_grounded {
                        // Just left ground - start coyote time
                        self.is_grounded = false;
                        self.coyote_time_remaining = COYOTE_TIME;
                    }
                    new_position
                }
            }
        }
    }

    /// Get the tangent forward direction for movement on a sphere.
    ///
    /// Projects the camera forward direction onto the tangent plane of the sphere.
    /// In flat mode, this is the same as regular camera forward (XZ plane).
    ///
    /// # Arguments
    /// * `camera_yaw` - Camera yaw angle in radians
    ///
    /// # Returns
    /// Normalized forward direction tangent to the sphere surface
    fn get_tangent_forward(&self, camera_yaw: f32) -> Vec3 {
        match &self.gravity_mode {
            GravityMode::Flat => {
                // Standard flat-world forward
                Self::get_camera_forward(camera_yaw)
            }
            GravityMode::Spherical(_) => {
                // Get the raw camera forward in world space
                let world_forward = Vec3::new(camera_yaw.sin(), 0.0, -camera_yaw.cos());

                // Project onto the tangent plane by removing the component along "up"
                let up = self.get_up_direction();
                let tangent_forward = world_forward - up * world_forward.dot(up);

                // Normalize, with fallback if tangent is too small
                if tangent_forward.length_squared() > 0.0001 {
                    tangent_forward.normalize()
                } else {
                    // Edge case: looking directly up/down, use any perpendicular direction
                    Vec3::X.cross(up).normalize()
                }
            }
        }
    }

    /// Get the tangent right direction for movement on a sphere.
    ///
    /// Perpendicular to tangent forward, both tangent to the sphere surface.
    ///
    /// # Arguments
    /// * `camera_yaw` - Camera yaw angle in radians
    ///
    /// # Returns
    /// Normalized right direction tangent to the sphere surface
    fn get_tangent_right(&self, camera_yaw: f32) -> Vec3 {
        match &self.gravity_mode {
            GravityMode::Flat => {
                // Standard flat-world right
                Self::get_camera_right(camera_yaw)
            }
            GravityMode::Spherical(_) => {
                // Right = Up × Forward (cross product for perpendicular on tangent plane)
                let up = self.get_up_direction();
                let forward = self.get_tangent_forward(camera_yaw);
                up.cross(forward).normalize()
            }
        }
    }

    /// Calculate the forward direction vector from camera yaw (XZ plane).
    ///
    /// Returns a normalized vector pointing in the camera's forward direction,
    /// projected onto the horizontal plane (Y=0).
    fn get_camera_forward(camera_yaw: f32) -> Vec3 {
        // Camera yaw: angle in radians where 0 = looking toward -Z
        // sin(yaw) gives X component, -cos(yaw) gives Z component
        Vec3::new(camera_yaw.sin(), 0.0, -camera_yaw.cos()).normalize()
    }

    /// Calculate the right direction vector from camera yaw (XZ plane).
    ///
    /// Returns a normalized vector pointing to the camera's right,
    /// perpendicular to forward on the horizontal plane.
    fn get_camera_right(camera_yaw: f32) -> Vec3 {
        // Right is perpendicular to forward on the XZ plane
        // Using cross product: forward × Y = right
        // For forward = (sin(yaw), 0, -cos(yaw)):
        // right = forward.cross(Y) = (-forward.z, 0, forward.x) = (cos(yaw), 0, sin(yaw))
        let forward = Self::get_camera_forward(camera_yaw);
        Vec3::new(-forward.z, 0.0, forward.x).normalize()
    }

    /// Update movement based on input and return the current velocity.
    ///
    /// Movement direction is calculated relative to the camera's facing direction:
    /// - Forward/backward input moves along the camera's forward axis (XZ plane)
    /// - Left/right input strafes perpendicular to the camera's forward axis
    ///
    /// Uses acceleration when input is provided, deceleration when no input.
    ///
    /// # Arguments
    /// * `dt` - Delta time in seconds
    /// * `input` - Movement input state (WASD keys)
    /// * `camera_yaw` - Camera yaw angle in radians
    ///
    /// # Returns
    /// Current velocity in world space (meters per second)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let velocity = controller.update(delta_time, &movement_input, camera.yaw);
    /// player_position += velocity * delta_time;
    /// ```
    pub fn update(&mut self, dt: f32, input: &MovementKeys, camera_yaw: f32) -> Vec3 {
        // Clamp delta time to prevent physics explosions
        let dt = dt.clamp(0.0001, 0.1);

        // Calculate input direction in world space
        // Use tangent directions for spherical mode, camera directions for flat mode
        let forward = self.get_tangent_forward(camera_yaw);
        let right = self.get_tangent_right(camera_yaw);

        // Get input axes (-1, 0, or 1)
        let forward_input = input.forward_axis() as f32;
        let right_input = input.right_axis() as f32;

        // Calculate desired movement direction
        // Movement direction = camera forward * input + camera right * strafe
        // In spherical mode, this is tangent to the sphere surface
        let input_dir = forward * forward_input + right * right_input;
        let input_dir = input_dir.normalize_or_zero();

        // Determine target speed based on sprint state
        let target_speed = if input.is_sprinting() {
            self.sprint_speed
        } else {
            self.walk_speed
        };

        // Calculate target velocity
        let target_velocity = input_dir * target_speed;

        // Check if we have input or need to decelerate
        let has_input = input_dir.length_squared() > 0.001;

        if has_input {
            // Accelerate toward target velocity
            let velocity_diff = target_velocity - self.velocity;
            let accel_this_frame = self.acceleration * dt;

            if velocity_diff.length() <= accel_this_frame {
                // Reached target velocity
                self.velocity = target_velocity;
            } else {
                // Accelerate toward target
                self.velocity += velocity_diff.normalize() * accel_this_frame;
            }
        } else {
            // Decelerate to stop
            let current_speed = self.velocity.length();

            if current_speed > 0.001 {
                let decel_this_frame = self.deceleration * dt;

                if current_speed <= decel_this_frame {
                    // Stopped completely
                    self.velocity = Vec3::ZERO;
                } else {
                    // Reduce speed while maintaining direction
                    let direction = self.velocity.normalize();
                    self.velocity = direction * (current_speed - decel_this_frame);
                }
            } else {
                self.velocity = Vec3::ZERO;
            }
        }

        self.velocity
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_input(
        forward: bool,
        backward: bool,
        left: bool,
        right: bool,
        sprint: bool,
    ) -> MovementKeys {
        let mut input = MovementKeys::default();
        input.forward = forward;
        input.backward = backward;
        input.left = left;
        input.right = right;
        input.sprint = sprint;
        input
    }

    #[test]
    fn test_default_controller() {
        let controller = PlayerMovementController::new();
        assert_eq!(controller.get_velocity(), Vec3::ZERO);
        assert_eq!(controller.get_walk_speed(), WALK_SPEED);
        assert_eq!(controller.get_sprint_speed(), SPRINT_SPEED);
        assert_eq!(controller.get_acceleration(), ACCELERATION);
        assert_eq!(controller.get_deceleration(), DECELERATION);
    }

    #[test]
    fn test_custom_speeds() {
        let controller = PlayerMovementController::with_speeds(3.0, 8.0);
        assert_eq!(controller.get_walk_speed(), 3.0);
        assert_eq!(controller.get_sprint_speed(), 8.0);
    }

    #[test]
    fn test_custom_physics() {
        let controller = PlayerMovementController::with_physics(4.0, 8.0, 40.0, 25.0);
        assert_eq!(controller.get_walk_speed(), 4.0);
        assert_eq!(controller.get_sprint_speed(), 8.0);
        assert_eq!(controller.get_acceleration(), 40.0);
        assert_eq!(controller.get_deceleration(), 25.0);
    }

    #[test]
    fn test_no_input_no_movement() {
        let mut controller = PlayerMovementController::new();
        let input = MovementKeys::default();

        let velocity = controller.update(0.016, &input, 0.0);
        assert_eq!(velocity, Vec3::ZERO);
    }

    #[test]
    fn test_forward_movement_accelerates() {
        let mut controller = PlayerMovementController::new();
        let input = create_input(true, false, false, false, false);

        // First frame - should have some velocity but not full speed
        let velocity1 = controller.update(0.016, &input, 0.0);
        assert!(velocity1.length() > 0.0);
        assert!(velocity1.length() < WALK_SPEED);

        // After many frames, should reach walk speed
        for _ in 0..100 {
            controller.update(0.016, &input, 0.0);
        }
        let final_speed = controller.get_speed();
        assert!((final_speed - WALK_SPEED).abs() < 0.1);
    }

    #[test]
    fn test_sprint_speed() {
        let mut controller = PlayerMovementController::new();
        let input = create_input(true, false, false, false, true); // Sprint enabled

        // Run for enough time to reach sprint speed
        for _ in 0..100 {
            controller.update(0.016, &input, 0.0);
        }

        let speed = controller.get_speed();
        assert!((speed - SPRINT_SPEED).abs() < 0.1);
    }

    #[test]
    fn test_deceleration() {
        let mut controller = PlayerMovementController::new();
        let forward_input = create_input(true, false, false, false, false);
        let no_input = MovementKeys::default();

        // Build up velocity
        for _ in 0..100 {
            controller.update(0.016, &forward_input, 0.0);
        }
        let moving_speed = controller.get_speed();
        assert!(moving_speed > 4.0);

        // Now decelerate
        for _ in 0..50 {
            controller.update(0.016, &no_input, 0.0);
        }
        let slowing_speed = controller.get_speed();
        assert!(slowing_speed < moving_speed);

        // Eventually should stop
        for _ in 0..100 {
            controller.update(0.016, &no_input, 0.0);
        }
        assert!(controller.get_speed() < 0.1);
    }

    #[test]
    fn test_camera_relative_forward() {
        let mut controller = PlayerMovementController::new();
        let input = create_input(true, false, false, false, false);

        // Camera facing -Z (yaw = 0)
        for _ in 0..100 {
            controller.update(0.016, &input, 0.0);
        }
        let velocity = controller.get_velocity();
        assert!(velocity.z < 0.0); // Moving toward -Z
        assert!(velocity.x.abs() < 0.1); // Not moving in X

        // Reset and face +X (yaw = PI/2)
        controller.reset();
        for _ in 0..100 {
            controller.update(0.016, &input, std::f32::consts::FRAC_PI_2);
        }
        let velocity = controller.get_velocity();
        assert!(velocity.x > 0.0); // Moving toward +X
        assert!(velocity.z.abs() < 0.1); // Not moving in Z
    }

    #[test]
    fn test_strafe_right() {
        let mut controller = PlayerMovementController::new();
        let input = create_input(false, false, false, true, false); // D key

        // Camera facing -Z (yaw = 0), strafe should go +X
        for _ in 0..100 {
            controller.update(0.016, &input, 0.0);
        }
        let velocity = controller.get_velocity();
        assert!(velocity.x > 0.0); // Moving toward +X (right)
        assert!(velocity.z.abs() < 0.1);
    }

    #[test]
    fn test_strafe_left() {
        let mut controller = PlayerMovementController::new();
        let input = create_input(false, false, true, false, false); // A key

        // Camera facing -Z (yaw = 0), strafe left should go -X
        for _ in 0..100 {
            controller.update(0.016, &input, 0.0);
        }
        let velocity = controller.get_velocity();
        assert!(velocity.x < 0.0); // Moving toward -X (left)
        assert!(velocity.z.abs() < 0.1);
    }

    #[test]
    fn test_diagonal_movement() {
        let mut controller = PlayerMovementController::new();
        let input = create_input(true, false, false, true, false); // W + D

        for _ in 0..100 {
            controller.update(0.016, &input, 0.0);
        }
        let velocity = controller.get_velocity();

        // Should move diagonally but at walk speed
        assert!(velocity.x > 0.0);
        assert!(velocity.z < 0.0);
        assert!((controller.get_speed() - WALK_SPEED).abs() < 0.1);
    }

    #[test]
    fn test_backward_movement() {
        let mut controller = PlayerMovementController::new();
        let input = create_input(false, true, false, false, false); // S key

        // Camera facing -Z (yaw = 0), backward should go +Z
        for _ in 0..100 {
            controller.update(0.016, &input, 0.0);
        }
        let velocity = controller.get_velocity();
        assert!(velocity.z > 0.0); // Moving toward +Z (backward)
    }

    #[test]
    fn test_opposite_inputs_cancel() {
        let mut controller = PlayerMovementController::new();
        let input = create_input(true, true, false, false, false); // W + S

        let velocity = controller.update(0.016, &input, 0.0);
        assert!(velocity.length() < 0.01);
    }

    #[test]
    fn test_reset() {
        let mut controller = PlayerMovementController::new();
        let input = create_input(true, false, false, false, false);

        // Build up velocity
        for _ in 0..50 {
            controller.update(0.016, &input, 0.0);
        }
        assert!(controller.get_speed() > 0.0);

        // Reset
        controller.reset();
        assert_eq!(controller.get_velocity(), Vec3::ZERO);
    }

    #[test]
    fn test_set_velocity() {
        let mut controller = PlayerMovementController::new();
        controller.set_velocity(Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(controller.get_velocity(), Vec3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_horizontal_speed() {
        let mut controller = PlayerMovementController::new();
        controller.set_velocity(Vec3::new(3.0, 4.0, 4.0));

        // Horizontal speed = sqrt(3^2 + 4^2) = 5
        assert!((controller.get_horizontal_speed() - 5.0).abs() < 0.001);
    }

    // Jump and Gravity Tests

    #[test]
    fn test_default_jump_gravity_values() {
        let controller = PlayerMovementController::new();
        assert_eq!(controller.get_jump_velocity(), JUMP_VELOCITY);
        assert_eq!(controller.get_gravity(), GRAVITY);
        assert_eq!(controller.get_vertical_velocity(), 0.0);
        assert!(controller.is_grounded());
        assert_eq!(controller.get_coyote_time_remaining(), 0.0);
    }

    #[test]
    fn test_jump_when_grounded() {
        let mut controller = PlayerMovementController::new();
        assert!(controller.is_grounded());

        let jumped = controller.apply_jump();
        assert!(jumped);
        assert_eq!(controller.get_vertical_velocity(), JUMP_VELOCITY);
        assert!(!controller.is_grounded());
        assert_eq!(controller.get_coyote_time_remaining(), 0.0);
    }

    #[test]
    fn test_cannot_double_jump() {
        let mut controller = PlayerMovementController::new();

        // First jump succeeds
        assert!(controller.apply_jump());

        // Second jump should fail (not grounded, no coyote time)
        let jumped_again = controller.apply_jump();
        assert!(!jumped_again);
    }

    #[test]
    fn test_gravity_decreases_velocity() {
        let mut controller = PlayerMovementController::new();
        controller.apply_jump();

        let initial_velocity = controller.get_vertical_velocity();
        let dt = 0.1;
        controller.apply_gravity(dt, 0.0);

        // Velocity should decrease by gravity * dt
        let expected = initial_velocity - GRAVITY * dt;
        assert!((controller.get_vertical_velocity() - expected).abs() < 0.01);
    }

    #[test]
    fn test_gravity_returns_delta_y() {
        let mut controller = PlayerMovementController::new();
        controller.set_vertical_velocity(10.0);
        controller.set_grounded(false);

        let dt = 0.1;
        let delta_y = controller.apply_gravity(dt, 0.0);

        // Using midpoint integration: avg_velocity = (10.0 + (10.0 - 20.0*0.1)) / 2 = (10.0 + 8.0) / 2 = 9.0
        // delta_y = 9.0 * 0.1 = 0.9
        let expected_delta = 0.9;
        assert!((delta_y - expected_delta).abs() < 0.01);
    }

    #[test]
    fn test_update_grounded_state_landing() {
        let mut controller = PlayerMovementController::new();
        controller.set_grounded(false);
        controller.set_vertical_velocity(-5.0); // Falling

        // Player Y is at or below ground height
        let corrected_y = controller.update_grounded_state(0.0, 0.0);

        assert!(controller.is_grounded());
        assert_eq!(controller.get_vertical_velocity(), 0.0);
        assert_eq!(corrected_y, 0.0);
    }

    #[test]
    fn test_update_grounded_state_airborne() {
        let mut controller = PlayerMovementController::new();
        controller.set_grounded(true);

        // Player Y is above ground - leaves ground
        let corrected_y = controller.update_grounded_state(5.0, 0.0);

        assert!(!controller.is_grounded());
        assert_eq!(corrected_y, 5.0);
        // Coyote time should be started
        assert!((controller.get_coyote_time_remaining() - COYOTE_TIME).abs() < 0.001);
    }

    #[test]
    fn test_coyote_time_allows_jump() {
        let mut controller = PlayerMovementController::new();

        // Simulate leaving ground
        controller.set_grounded(true);
        controller.update_grounded_state(1.0, 0.0); // Now airborne with coyote time

        assert!(!controller.is_grounded());
        assert!(controller.can_jump()); // Coyote time allows jump

        // Jump should work
        assert!(controller.apply_jump());
    }

    #[test]
    fn test_coyote_time_expires() {
        let mut controller = PlayerMovementController::new();

        // Simulate leaving ground
        controller.set_grounded(true);
        controller.update_grounded_state(1.0, 0.0);

        // Apply gravity for longer than coyote time
        for _ in 0..10 {
            controller.apply_gravity(0.02, 0.0); // 0.2s total > 0.1s coyote time
        }

        assert_eq!(controller.get_coyote_time_remaining(), 0.0);
        assert!(!controller.can_jump());
        assert!(!controller.apply_jump());
    }

    #[test]
    fn test_full_jump_arc() {
        let mut controller = PlayerMovementController::new();

        // Start on ground at y=0
        let mut player_y = 0.0;

        // Jump
        controller.apply_jump();
        assert_eq!(controller.get_vertical_velocity(), JUMP_VELOCITY); // 8.0

        let dt = 0.016; // ~60fps
        let mut max_height = 0.0;
        let mut frames = 0;

        // Simulate until landing
        while frames < 1000 {
            let delta_y = controller.apply_gravity(dt, 0.0);
            player_y += delta_y;
            player_y = controller.update_grounded_state(player_y, 0.0);

            if player_y > max_height {
                max_height = player_y;
            }

            if controller.is_grounded() && frames > 10 {
                break;
            }
            frames += 1;
        }

        // With v0=8.0, g=20.0: max height = v0^2 / (2g) = 64 / 40 = 1.6m
        assert!(
            (max_height - 1.6).abs() < 0.1,
            "Max height was {} expected ~1.6",
            max_height
        );

        // Should land back on ground
        assert!(controller.is_grounded());
        assert!(player_y <= 0.01, "Player Y was {} expected ~0.0", player_y);
    }

    #[test]
    fn test_reset_clears_vertical_state() {
        let mut controller = PlayerMovementController::new();

        // Jump and apply some gravity
        controller.apply_jump();
        controller.apply_gravity(0.1, 0.0);

        controller.reset();

        assert_eq!(controller.get_vertical_velocity(), 0.0);
        assert!(controller.is_grounded());
        assert_eq!(controller.get_coyote_time_remaining(), 0.0);
    }

    #[test]
    fn test_custom_jump_gravity() {
        let controller = PlayerMovementController::with_full_physics(
            5.0, 10.0, 50.0, 30.0, 10.0, // custom jump velocity
            15.0, // custom gravity
        );

        assert_eq!(controller.get_jump_velocity(), 10.0);
        assert_eq!(controller.get_gravity(), 15.0);
    }

    // Spherical Gravity Mode Tests

    #[test]
    fn test_set_spherical_mode() {
        let mut controller = PlayerMovementController::new();

        // Initially in flat mode
        assert!(!controller.is_spherical_mode());
        assert!(controller.get_spherical_config().is_none());

        // Enable spherical mode
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);

        assert!(controller.is_spherical_mode());
        let config = controller.get_spherical_config().unwrap();
        assert_eq!(config.planet_center, Vec3::ZERO);
        assert_eq!(config.planet_radius, 1000.0);
    }

    #[test]
    fn test_set_flat_mode() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);
        assert!(controller.is_spherical_mode());

        controller.set_flat_mode();
        assert!(!controller.is_spherical_mode());
    }

    #[test]
    fn test_spherical_up_direction() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);

        // Player above the planet at +Y
        controller.set_player_position(Vec3::new(0.0, 1100.0, 0.0));
        let up = controller.get_up_direction();
        assert!((up - Vec3::Y).length() < 0.001, "Up should point +Y");

        // Player to the +X side of planet
        controller.set_player_position(Vec3::new(1100.0, 0.0, 0.0));
        let up = controller.get_up_direction();
        assert!((up - Vec3::X).length() < 0.001, "Up should point +X");

        // Player at -Z side of planet
        controller.set_player_position(Vec3::new(0.0, 0.0, -1100.0));
        let up = controller.get_up_direction();
        assert!((up - (-Vec3::Z)).length() < 0.001, "Up should point -Z");
    }

    #[test]
    fn test_spherical_down_direction() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);

        // Player above the planet at +Y
        controller.set_player_position(Vec3::new(0.0, 1100.0, 0.0));
        let down = controller.get_down_direction();
        assert!((down - (-Vec3::Y)).length() < 0.001, "Down should point -Y");
    }

    #[test]
    fn test_spherical_jump_direction() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);

        // Player at +X side of planet
        controller.set_player_position(Vec3::new(1100.0, 0.0, 0.0));
        let jump_dir = controller.get_jump_direction();
        assert!(
            (jump_dir - Vec3::X).length() < 0.001,
            "Jump should point +X (surface normal)"
        );
    }

    #[test]
    fn test_flat_mode_up_direction() {
        let controller = PlayerMovementController::new();

        // In flat mode, up is always +Y regardless of position
        assert!((controller.get_up_direction() - Vec3::Y).length() < 0.001);
        assert!((controller.get_down_direction() - (-Vec3::Y)).length() < 0.001);
    }

    #[test]
    fn test_spherical_gravity_config_height() {
        let config = SphericalGravityConfig::new(Vec3::ZERO, 1000.0);

        // On surface
        let height = config.get_height_above_surface(Vec3::new(0.0, 1000.0, 0.0));
        assert!(height.abs() < 0.001, "Should be at surface");

        // Above surface
        let height = config.get_height_above_surface(Vec3::new(0.0, 1100.0, 0.0));
        assert!(
            (height - 100.0).abs() < 0.001,
            "Should be 100m above surface"
        );

        // Below surface (inside planet)
        let height = config.get_height_above_surface(Vec3::new(0.0, 900.0, 0.0));
        assert!(
            (height - (-100.0)).abs() < 0.001,
            "Should be 100m below surface"
        );
    }

    #[test]
    fn test_spherical_gravity_config_surface_position() {
        let config = SphericalGravityConfig::new(Vec3::ZERO, 1000.0);

        // From above, project to surface
        let surface = config.get_surface_position(Vec3::new(0.0, 1500.0, 0.0));
        assert!((surface - Vec3::new(0.0, 1000.0, 0.0)).length() < 0.001);

        // From diagonal position
        let pos = Vec3::new(1000.0, 1000.0, 0.0); // Distance = sqrt(2) * 1000
        let surface = config.get_surface_position(pos);
        let expected = pos.normalize() * 1000.0;
        assert!((surface - expected).length() < 0.001);
    }

    #[test]
    fn test_apply_gravity_spherical() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);

        // Position player above the planet's +Y pole
        controller.set_player_position(Vec3::new(0.0, 1100.0, 0.0));
        controller.set_vertical_velocity(10.0); // Moving upward
        controller.set_grounded(false);

        let dt = 0.1;
        let delta = controller.apply_gravity_spherical(dt);

        // Delta should be along the up direction (+Y)
        // With gravity pulling down, velocity decreases but still positive initially
        // avg_velocity = (10.0 + (10.0 - 20.0*0.1)) / 2 = 9.0
        // delta_magnitude = 9.0 * 0.1 = 0.9
        assert!(
            delta.y > 0.0,
            "Delta should still be positive (upward) initially"
        );
        assert!(delta.x.abs() < 0.001, "Delta should have no X component");
        assert!(delta.z.abs() < 0.001, "Delta should have no Z component");
    }

    #[test]
    fn test_update_grounded_state_spherical_landing() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);
        controller.set_grounded(false);
        controller.set_vertical_velocity(-5.0);

        // Player is at surface level
        let new_pos = Vec3::new(0.0, 1000.0, 0.0);
        let corrected = controller.update_grounded_state_spherical(new_pos);

        assert!(controller.is_grounded());
        assert_eq!(controller.get_vertical_velocity(), 0.0);
        // Should be exactly on surface
        let height = controller
            .get_spherical_config()
            .unwrap()
            .get_height_above_surface(corrected);
        assert!(height.abs() < 0.001);
    }

    #[test]
    fn test_update_grounded_state_spherical_airborne() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);
        controller.set_grounded(true);

        // Player is above surface
        let new_pos = Vec3::new(0.0, 1100.0, 0.0);
        let corrected = controller.update_grounded_state_spherical(new_pos);

        assert!(!controller.is_grounded());
        assert_eq!(corrected, new_pos); // Position unchanged when airborne
        assert!((controller.get_coyote_time_remaining() - COYOTE_TIME).abs() < 0.001);
    }

    #[test]
    fn test_spherical_movement_tangent_to_surface() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);

        // Player at +Y pole of planet
        controller.set_player_position(Vec3::new(0.0, 1100.0, 0.0));

        let forward = controller.get_tangent_forward(0.0);
        let right = controller.get_tangent_right(0.0);
        let up = controller.get_up_direction();

        // Forward and right should be perpendicular to up
        assert!(
            forward.dot(up).abs() < 0.001,
            "Forward should be tangent to surface"
        );
        assert!(
            right.dot(up).abs() < 0.001,
            "Right should be tangent to surface"
        );

        // Forward and right should be perpendicular to each other
        assert!(
            forward.dot(right).abs() < 0.001,
            "Forward and right should be perpendicular"
        );
    }

    #[test]
    fn test_spherical_movement_velocity() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);

        // Player at +X side of planet (up = +X)
        controller.set_player_position(Vec3::new(1100.0, 0.0, 0.0));

        let input = create_input(true, false, false, false, false);

        // Update movement - velocity should be tangent to sphere
        for _ in 0..50 {
            controller.update(0.016, &input, 0.0);
        }

        let velocity = controller.get_velocity();
        let up = controller.get_up_direction();

        // Velocity should be perpendicular to up (tangent to surface)
        let dot = velocity.normalize().dot(up);
        assert!(
            dot.abs() < 0.1,
            "Velocity should be tangent to surface, got dot={}",
            dot
        );
    }

    #[test]
    fn test_full_spherical_jump_arc() {
        let mut controller = PlayerMovementController::new();
        controller.set_spherical_mode(Vec3::ZERO, 1000.0);

        // Start on surface at +Y pole
        let mut position = Vec3::new(0.0, 1000.0, 0.0);
        controller.set_player_position(position);

        // Jump
        controller.apply_jump();
        assert_eq!(controller.get_vertical_velocity(), JUMP_VELOCITY);

        let dt = 0.016;
        let mut max_height = 0.0;
        let mut frames = 0;

        while frames < 1000 {
            controller.set_player_position(position);
            let delta = controller.apply_gravity_spherical(dt);
            position += delta;
            position = controller.update_grounded_state_spherical(position);

            let height = controller
                .get_spherical_config()
                .unwrap()
                .get_height_above_surface(position);
            if height > max_height {
                max_height = height;
            }

            if controller.is_grounded() && frames > 10 {
                break;
            }
            frames += 1;
        }

        // Max height should be approximately v0^2 / (2g) = 64 / 40 = 1.6m
        assert!(
            (max_height - 1.6).abs() < 0.2,
            "Max height was {} expected ~1.6",
            max_height
        );

        // Should land back on surface
        assert!(controller.is_grounded());
        let final_height = controller
            .get_spherical_config()
            .unwrap()
            .get_height_above_surface(position);
        assert!(
            final_height.abs() < 0.01,
            "Should be on surface, height={}",
            final_height
        );
    }
}
