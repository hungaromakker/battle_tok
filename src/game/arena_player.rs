//! Arena Player Module
//!
//! First-person player controller with physics-based movement for the Battle Arena.
//! Island-aware ground collision: player can fall off island edges into the void.

use glam::Vec3;

use super::terrain::{
    get_bridge_height, is_inside_hexagon, terrain_height_at_island, BridgeConfig,
};

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

/// Describes a hexagonal island for ground collision.
#[derive(Clone, Copy, Debug)]
pub struct IslandDef {
    /// World-space center (XZ)
    pub center: Vec3,
    /// Hexagonal radius (circumradius)
    pub radius: f32,
    /// Base Y of the terrain surface (usually 0.0 for ground-level islands)
    pub surface_y: f32,
}

/// Bridge endpoints for ground collision.
#[derive(Clone, Debug)]
pub struct BridgeDef {
    pub start: Vec3,
    pub end: Vec3,
    pub config: BridgeConfig,
}

/// Arena ground context passed each frame so the player knows about islands + bridge.
pub struct ArenaGround {
    pub islands: Vec<IslandDef>,
    pub bridge: Option<BridgeDef>,
    /// Y-level at which the player dies (lava)
    pub kill_y: f32,
    /// Respawn position
    pub respawn_pos: Vec3,
}

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

    /// Update player physics with island-aware ground collision.
    ///
    /// The player only has ground beneath them when standing on an island
    /// (inside its hexagonal boundary) or on the bridge. Walking off the
    /// edge causes a free-fall. Hitting `kill_y` respawns the player.
    pub fn update(
        &mut self,
        movement: &MovementKeys,
        camera_yaw: f32,
        delta_time: f32,
        ground: &ArenaGround,
    ) {
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

        // ====================================================
        // Island-aware ground collision
        // ====================================================
        // Check each island: if player is inside its hexagonal boundary,
        // sample terrain height and use it as ground.
        let mut ground_height: Option<f32> = None;

        for island in &ground.islands {
            let dx = self.position.x - island.center.x;
            let dz = self.position.z - island.center.z;

            if is_inside_hexagon(dx, dz, island.radius) {
                let h = terrain_height_at_island(
                    self.position.x,
                    self.position.z,
                    island.surface_y,
                    island.center.x,
                    island.center.z,
                    island.radius,
                );
                ground_height = Some(match ground_height {
                    Some(prev) => prev.max(h),
                    None => h,
                });
            }
        }

        // Also check bridge
        if let Some(ref bridge) = ground.bridge {
            if let Some(bridge_y) = get_bridge_height(
                self.position.x,
                self.position.z,
                bridge.start,
                bridge.end,
                &bridge.config,
            ) {
                ground_height = Some(match ground_height {
                    Some(prev) => prev.max(bridge_y),
                    None => bridge_y,
                });
            }
        }

        // Ground collision: only if there IS ground beneath us
        match ground_height {
            Some(gh) => {
                if self.position.y <= gh {
                    self.position.y = gh;
                    self.vertical_velocity = 0.0;

                    if !self.is_grounded {
                        self.is_grounded = true;
                        self.coyote_time_remaining = COYOTE_TIME;
                    }
                } else if self.is_grounded {
                    self.is_grounded = false;
                    self.coyote_time_remaining = COYOTE_TIME;
                }
            }
            None => {
                // No ground — free-falling in the void
                if self.is_grounded {
                    self.is_grounded = false;
                    self.coyote_time_remaining = COYOTE_TIME;
                }
            }
        }

        // ====================================================
        // Lava kill plane — respawn if below kill_y
        // ====================================================
        if self.position.y < ground.kill_y {
            self.position = ground.respawn_pos;
            self.velocity = Vec3::ZERO;
            self.vertical_velocity = 0.0;
            self.is_grounded = false;
        }
    }
}
