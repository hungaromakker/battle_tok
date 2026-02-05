//! Arena Player Module
//!
//! First-person player controller with physics-based movement for the Battle Arena.

use glam::Vec3;

use super::terrain::terrain_height_at;

/// Movement key state
#[derive(Default)]
pub struct MovementKeys {
    pub forward: bool,
    pub backward: bool,
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub sprint: bool,
}

/// Cannon aiming key state (for smooth continuous movement)
#[derive(Default)]
pub struct AimingKeys {
    pub aim_up: bool,
    pub aim_down: bool,
    pub aim_left: bool,
    pub aim_right: bool,
}

// Player physics constants
/// Eye height in meters
pub const PLAYER_EYE_HEIGHT: f32 = 1.7;
/// Walking speed in m/s
pub const PLAYER_WALK_SPEED: f32 = 5.0;
/// Sprinting speed in m/s
pub const PLAYER_SPRINT_SPEED: f32 = 10.0;
/// Gravity in m/s²
pub const PLAYER_GRAVITY: f32 = 20.0;
/// Jump velocity in m/s
pub const PLAYER_JUMP_VELOCITY: f32 = 8.0;
/// Acceleration in m/s²
pub const PLAYER_ACCELERATION: f32 = 50.0;
/// Deceleration when no input
pub const PLAYER_DECELERATION: f32 = 30.0;
/// Time after leaving ground where jump is still allowed
pub const COYOTE_TIME: f32 = 0.1;

/// First-person player with physics-based movement
pub struct Player {
    /// Position of player's feet in world space
    pub position: Vec3,
    /// Horizontal velocity (XZ plane)
    pub velocity: Vec3,
    /// Vertical velocity (for jumping/falling)
    pub vertical_velocity: f32,
    /// Whether player is currently on the ground
    pub is_grounded: bool,
    /// Coyote time remaining (for forgiving jump timing)
    pub coyote_time_remaining: f32,
    /// Whether jump was requested (consumed when jump happens)
    pub jump_requested: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 15.0, 30.0),
            velocity: Vec3::ZERO,
            vertical_velocity: 0.0,
            is_grounded: false,
            coyote_time_remaining: 0.0,
            jump_requested: false,
        }
    }
}

impl Player {
    /// Get the camera position (player's eye level)
    pub fn get_eye_position(&self) -> Vec3 {
        self.position + Vec3::new(0.0, PLAYER_EYE_HEIGHT, 0.0)
    }

    /// Request a jump (will be processed in update)
    pub fn request_jump(&mut self) {
        self.jump_requested = true;
    }

    /// Check if can currently jump
    pub fn can_jump(&self) -> bool {
        self.is_grounded || self.coyote_time_remaining > 0.0
    }

    /// Update player physics
    pub fn update(&mut self, movement: &MovementKeys, camera_yaw: f32, delta_time: f32) {
        let dt = delta_time.clamp(0.0001, 0.1);

        // Calculate forward/right directions from camera yaw (XZ plane only)
        let forward = Vec3::new(camera_yaw.sin(), 0.0, -camera_yaw.cos()).normalize();
        let right = Vec3::new(-forward.z, 0.0, forward.x).normalize();

        // Get input direction
        let forward_input =
            if movement.forward { 1.0 } else { 0.0 } - if movement.backward { 1.0 } else { 0.0 };
        let right_input =
            if movement.right { 1.0 } else { 0.0 } - if movement.left { 1.0 } else { 0.0 };

        let input_dir = (forward * forward_input + right * right_input).normalize_or_zero();

        let target_speed = if movement.sprint {
            PLAYER_SPRINT_SPEED
        } else {
            PLAYER_WALK_SPEED
        };

        let target_velocity = input_dir * target_speed;

        let has_input = input_dir.length_squared() > 0.001;
        if has_input {
            let velocity_diff = target_velocity - self.velocity;
            let accel_amount = PLAYER_ACCELERATION * dt;

            if velocity_diff.length() <= accel_amount {
                self.velocity = target_velocity;
            } else {
                self.velocity += velocity_diff.normalize() * accel_amount;
            }
        } else {
            let speed = self.velocity.length();
            if speed > 0.001 {
                let decel_amount = PLAYER_DECELERATION * dt;
                if speed <= decel_amount {
                    self.velocity = Vec3::ZERO;
                } else {
                    self.velocity -= self.velocity.normalize() * decel_amount;
                }
            } else {
                self.velocity = Vec3::ZERO;
            }
        }

        self.position += self.velocity * dt;

        // Handle jump request
        if self.jump_requested {
            if self.can_jump() {
                self.vertical_velocity = PLAYER_JUMP_VELOCITY;
                self.is_grounded = false;
                self.coyote_time_remaining = 0.0;
            }
            self.jump_requested = false;
        }

        // Apply gravity
        self.vertical_velocity -= PLAYER_GRAVITY * dt;
        self.position.y += self.vertical_velocity * dt;

        // Update coyote time
        if !self.is_grounded {
            self.coyote_time_remaining = (self.coyote_time_remaining - dt).max(0.0);
        }

        // Ground collision
        let ground_height = terrain_height_at(self.position.x, self.position.z, 0.0);

        if self.position.y <= ground_height {
            self.position.y = ground_height;
            self.vertical_velocity = 0.0;

            if !self.is_grounded {
                self.is_grounded = true;
                self.coyote_time_remaining = COYOTE_TIME;
            }
        } else {
            if self.is_grounded {
                self.is_grounded = false;
                self.coyote_time_remaining = COYOTE_TIME;
            }
        }
    }
}
