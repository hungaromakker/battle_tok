//! Camera Controller Module
//!
//! Provides player-centric camera control with third-person and first-person modes.
//! Features Unity-style orbit camera for third-person with spring damping and camera
//! collision. Supports smooth 0.3s mode transitions.
//! This is window-system agnostic - it only manages camera state and transformations.
//!
//! ## Spherical Planet Physics
//!
//! For spherical worlds, gravity points toward the planet center (radial gravity).
//! The planet center is at `(0, -planet_radius, 0)`, so the surface is at Y=0.
//!
//! Forces acting on the player:
//! - **Gravity**: F_g = m * g, directed toward planet center
//! - **Normal force**: F_n = -F_g when on surface (ground pushback)
//! - **Friction**: F_f = μ * |F_n|, opposes motion along surface tangent
//!
//! For simplicity, we use 1 kg/dm² mass density, so a 1.8m tall player ≈ 70kg.

use glam::Vec3;

/// Camera mode - determines camera position relative to player
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub enum CameraMode {
    /// Default: Unity-style camera behind/above player (orbit camera)
    #[default]
    ThirdPerson,
    /// First-person: camera at player head, hands and body visible
    FirstPerson,
}

/// Transition state for smooth camera mode switching
#[derive(Clone, Debug)]
pub struct CameraTransition {
    /// Whether a transition is currently active
    pub active: bool,
    /// Progress of the transition (0.0 to 1.0)
    pub progress: f32,
    /// Duration of the transition in seconds
    pub duration: f32,
    /// Starting camera position for interpolation
    pub from_position: Vec3,
    /// Target camera position for interpolation
    pub to_position: Vec3,
    /// Starting distance (for third-person)
    pub from_distance: f32,
    /// Target distance
    pub to_distance: f32,
}

impl Default for CameraTransition {
    fn default() -> Self {
        Self {
            active: false,
            progress: 0.0,
            duration: 0.3, // 0.3 second transition as per spec
            from_position: Vec3::ZERO,
            to_position: Vec3::ZERO,
            from_distance: 0.0,
            to_distance: 0.0,
        }
    }
}

/// Spring damping configuration for smooth camera follow
#[derive(Clone, Copy, Debug)]
pub struct SpringConfig {
    /// Spring stiffness (higher = faster response)
    pub stiffness: f32,
    /// Damping coefficient (higher = less oscillation)
    pub damping: f32,
}

impl Default for SpringConfig {
    fn default() -> Self {
        Self {
            stiffness: 10.0,
            damping: 5.0,
        }
    }
}

/// Camera collision configuration
#[derive(Clone, Copy, Debug)]
pub struct CameraCollisionConfig {
    /// Whether camera collision is enabled
    pub enabled: bool,
    /// Minimum distance from obstacles
    pub min_distance: f32,
    /// Collision sphere radius for camera
    pub radius: f32,
}

impl Default for CameraCollisionConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            min_distance: 0.2, // 20cm minimum distance from obstacles
            radius: 0.3,       // 30cm collision sphere
        }
    }
}

/// Camera controller state
///
/// Manages camera position, orientation, and movement in a window-system agnostic way.
/// Supports third-person (Unity-style orbit) and first-person camera modes with smooth transitions.
/// Features spring damping for smooth camera follow and camera collision to avoid wall clipping.
/// Input handling should be done externally and passed to this controller via delta values.
#[derive(Clone, Debug)]
pub struct CameraController {
    /// Actual camera position in world space
    pub position: Vec3,
    /// Horizontal angle (radians) - unrestricted
    pub yaw: f32,
    /// Vertical angle (radians) - limited to pitch_limits
    pub pitch: f32,
    /// Distance from player (for third-person mode)
    pub distance: f32,
    /// Player position (camera follows this point)
    pub player_position: Vec3,
    /// Player yaw rotation (radians) - follows camera yaw when moving
    pub player_yaw: f32,
    /// Current camera mode (ThirdPerson or FirstPerson)
    pub mode: CameraMode,
    /// Target mode (for transitions)
    target_mode: CameraMode,
    /// Pitch limits (min, max) in radians: -89° to +89°
    pub pitch_limits: (f32, f32),
    /// Transition state for smooth mode switching
    pub transition: CameraTransition,
    /// Movement speed in meters per second (default: 5.0 m/s walking)
    pub move_speed: f32,
    /// Sprint speed multiplier (default: 2.0x)
    pub sprint_multiplier: f32,
    /// Mouse look sensitivity
    pub look_sensitivity: f32,
    /// Pan sensitivity for middle-mouse panning
    pub pan_sensitivity: f32,
    /// Third-person camera offset: (0, height_above_player, distance_behind)
    /// Default: 3m behind, 2m above (Unity-style)
    pub third_person_offset: Vec3,
    /// Third-person look target height above player center
    /// Default: 0.5m (looks at chest/upper body)
    pub third_person_look_height: f32,
    /// First-person eye height offset above feet
    /// Default: 1.6m (realistic eye level)
    pub first_person_head_height: f32,
    /// Spring damping configuration
    pub spring_config: SpringConfig,
    /// Camera collision configuration
    pub collision_config: CameraCollisionConfig,
    /// Actual camera distance after collision check
    actual_distance: f32,
    /// Vertical velocity for gravity (m/s, positive = up)
    pub vertical_velocity: f32,
    /// Whether the player is on the ground
    pub is_grounded: bool,
    /// Gravity acceleration (m/s², default: 9.81)
    pub gravity: f32,
    /// Jump velocity (m/s, default: 5.0)
    pub jump_velocity: f32,
    /// Minimum height above ground (player capsule height)
    pub min_height: f32,
    /// Standing height (normal eye level, default: 1.8m)
    pub standing_height: f32,
    /// Crouching height (eye level when crouched, default: 0.9m - half standing)
    pub crouch_height: f32,
    /// Minimum crouch height (for small creatures or prone, default: 0.3m)
    pub min_crouch_height: f32,
    /// Whether currently crouching
    pub is_crouching: bool,
    /// Current player height (smoothly transitions between standing/crouch)
    current_height: f32,
    // === Spherical Planet Physics ===
    /// Planet radius for spherical gravity (0 = flat world, use standard Y-down gravity)
    pub planet_radius: f32,
    /// Planet center position (for spherical worlds, typically (0, -planet_radius, 0))
    planet_center: Vec3,
    /// Friction coefficient (μ) for surface movement (default: 0.6 for grass/dirt)
    pub friction_coefficient: f32,
    /// Player mass in kg (default: 70kg for average human)
    pub player_mass: f32,
    /// Current velocity vector (for momentum-based movement)
    pub velocity: Vec3,
    /// Surface normal at player position (up direction on spherical world)
    surface_normal: Vec3,
}

/// Pitch limit constant: -89 degrees in radians
const PITCH_LIMIT_MIN: f32 = -89.0 * std::f32::consts::PI / 180.0;
/// Pitch limit constant: +89 degrees in radians
const PITCH_LIMIT_MAX: f32 = 89.0 * std::f32::consts::PI / 180.0;

impl Default for CameraController {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 2.0, 3.0),
            yaw: 0.0,
            pitch: -0.2,
            distance: 3.0, // 3m behind player
            player_position: Vec3::ZERO,
            player_yaw: 0.0, // Player facing direction
            mode: CameraMode::ThirdPerson,
            target_mode: CameraMode::ThirdPerson,
            pitch_limits: (PITCH_LIMIT_MIN, PITCH_LIMIT_MAX),
            transition: CameraTransition::default(),
            move_speed: 5.0,        // 5 m/s walking speed (realistic human walking)
            sprint_multiplier: 2.0, // 10 m/s sprinting
            look_sensitivity: 8.0,  // Snappy video game feel (was 2.0, too sluggish)
            pan_sensitivity: 10.0,
            // Unity-style: 3m behind, 2m above player
            third_person_offset: Vec3::new(0.0, 2.0, 3.0),
            // Look at 0.5m above player center (chest area)
            third_person_look_height: 0.5,
            // Eye level: 1.6m above feet
            first_person_head_height: 1.6,
            // Spring damping: stiffness=10, damping=5
            spring_config: SpringConfig::default(),
            collision_config: CameraCollisionConfig::default(),
            actual_distance: 3.0,
            // Physics
            vertical_velocity: 0.0,
            is_grounded: true,
            gravity: 9.81,      // Earth gravity (m/s²)
            jump_velocity: 5.0, // ~1.3m jump height
            min_height: 1.8,    // Player standing height (eyes at 1.6m + 0.2m margin)
            // Crouch system
            standing_height: 1.8,   // Normal standing eye level
            crouch_height: 0.9,     // Crouched eye level (half height)
            min_crouch_height: 0.3, // Prone/small creature level
            is_crouching: false,
            current_height: 1.8, // Start at standing height
            // Spherical planet physics (default: flat world)
            planet_radius: 0.0, // 0 = flat world
            planet_center: Vec3::ZERO,
            friction_coefficient: 0.6, // Grass/dirt friction
            player_mass: 70.0,         // 70kg average human
            velocity: Vec3::ZERO,
            surface_normal: Vec3::Y, // Default: flat world, up is Y
        }
    }
}

impl CameraController {
    /// Create a new camera controller with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a camera controller with a custom initial position
    pub fn with_position(position: Vec3) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    /// Get the camera's current position in world space
    #[inline]
    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    /// Set the camera's position directly
    ///
    /// Use this for teleporting the camera or when world-wrapping.
    #[inline]
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    /// Get the point the camera is looking at
    ///
    /// Get the point the camera is looking at.
    ///
    /// For FPS-style camera control, both modes use the forward direction based on
    /// pitch and yaw angles. This ensures the camera looks where the player aims,
    /// not at a fixed point.
    pub fn get_target(&self) -> Vec3 {
        // FPS-style: target is always based on camera forward direction
        // This works for both first-person and third-person modes
        self.position + self.get_forward() * 10.0
    }

    /// Get the look target height above player (for third-person)
    pub fn get_look_target_height(&self) -> f32 {
        self.third_person_look_height
    }

    /// Set the look target height above player (for third-person)
    pub fn set_look_target_height(&mut self, height: f32) {
        self.third_person_look_height = height;
    }

    /// Get the player position that the camera follows
    pub fn get_player_position(&self) -> Vec3 {
        self.player_position
    }

    /// Set the player position (camera follows this)
    pub fn set_player_position(&mut self, position: Vec3) {
        self.player_position = position;
    }

    /// Get the player's yaw rotation (facing direction)
    pub fn get_player_yaw(&self) -> f32 {
        self.player_yaw
    }

    /// Set the player's yaw rotation directly
    pub fn set_player_yaw(&mut self, yaw: f32) {
        self.player_yaw = yaw;
    }

    /// Update player rotation to follow camera yaw when moving
    ///
    /// In third-person mode, the player character rotates to face the
    /// direction of movement (which follows camera yaw).
    ///
    /// # Arguments
    /// * `is_moving` - Whether the player is currently moving (WASD input)
    /// * `delta_time` - Time since last frame for smooth rotation
    pub fn update_player_rotation(&mut self, is_moving: bool, delta_time: f32) {
        if is_moving && self.mode == CameraMode::ThirdPerson {
            // Smoothly rotate player to match camera yaw
            let target_yaw = self.yaw;
            let turn_speed = 10.0; // radians per second

            // Calculate shortest angle difference
            let mut diff = target_yaw - self.player_yaw;
            while diff > std::f32::consts::PI {
                diff -= std::f32::consts::TAU;
            }
            while diff < -std::f32::consts::PI {
                diff += std::f32::consts::TAU;
            }

            // Apply smooth rotation
            let max_turn = turn_speed * delta_time;
            let turn = diff.clamp(-max_turn, max_turn);
            self.player_yaw += turn;
        }
    }

    /// Get the camera's forward direction vector
    ///
    /// This is the direction the camera is looking, derived from yaw and pitch.
    #[inline]
    pub fn get_forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
        .normalize()
    }

    /// Get the camera's right direction vector
    #[inline]
    pub fn get_right(&self) -> Vec3 {
        let forward = self.get_forward();
        // forward.cross(Y) gives right in this coordinate system
        // When forward = (0, 0, -1) and Y = (0, 1, 0):
        // (0,0,-1) × (0,1,0) = (0*0 - (-1)*1, (-1)*0 - 0*0, 0*1 - 0*0) = (1, 0, 0) = right
        forward.cross(Vec3::Y).normalize()
    }

    /// Get the camera's up direction vector
    #[inline]
    pub fn get_up(&self) -> Vec3 {
        let forward = self.get_forward();
        let right = self.get_right();
        right.cross(forward).normalize()
    }

    /// Handle mouse look input (typically from right-mouse drag)
    ///
    /// # Arguments
    /// * `delta_x` - Mouse movement in X (normalized 0-1 screen coordinates)
    /// * `delta_y` - Mouse movement in Y (normalized 0-1 screen coordinates)
    /// * `invert_y` - Whether to invert Y axis (flight sim style)
    pub fn handle_mouse_look(&mut self, delta_x: f32, delta_y: f32, invert_y: bool) {
        let y_sign = if invert_y { -1.0 } else { 1.0 };

        // FPS camera: mouse movement controls where you look
        //
        // get_forward() = (sin(yaw) * cos(pitch), sin(pitch), -cos(yaw) * cos(pitch))
        // At yaw=0: forward = (0, 0, -1) = looking toward -Z
        // At yaw=+π/2: forward = (1, 0, 0) = looking toward +X (right)
        // At yaw=-π/2: forward = (-1, 0, 0) = looking toward -X (left)
        //
        // So to look RIGHT (toward +X), yaw must INCREASE (become more positive)
        // Mouse right = delta_x > 0 => yaw should INCREASE => yaw += delta_x
        //
        // For pitch:
        // At pitch=0: looking horizontally
        // At pitch=+π/2: looking up (y component = 1)
        // At pitch=-π/2: looking down (y component = -1)
        //
        // Screen Y increases downward, so mouse up = delta_y < 0
        // To look UP when mouse moves UP: pitch should INCREASE when delta_y < 0
        // So: pitch -= delta_y (when delta_y < 0, pitch increases)
        self.yaw += delta_x * self.look_sensitivity;
        self.pitch -= delta_y * self.look_sensitivity * y_sign;

        // Clamp pitch to prevent camera flip
        self.pitch = self.pitch.clamp(self.pitch_limits.0, self.pitch_limits.1);
    }

    /// Handle middle-mouse panning
    ///
    /// # Arguments
    /// * `delta_x` - Mouse movement in X (normalized 0-1 screen coordinates)
    /// * `delta_y` - Mouse movement in Y (normalized 0-1 screen coordinates)
    pub fn handle_pan(&mut self, delta_x: f32, delta_y: f32) {
        let right = self.get_right();
        let up = Vec3::Y;

        // Pan in screen space
        self.position -= right * delta_x * self.pan_sensitivity;
        self.position += up * delta_y * self.pan_sensitivity;
    }

    /// Update camera position based on movement input (frame-rate independent)
    ///
    /// # Arguments
    /// * `forward` - Move forward (positive) or backward (negative)
    /// * `right` - Move right (positive) or left (negative)
    /// * `up` - Move up (positive) or down (negative) - only works in fly mode
    /// * `delta_time` - Time since last frame in seconds
    /// * `is_sprinting` - Whether sprint key is held
    pub fn update_movement_with_physics(
        &mut self,
        forward: f32,
        right: f32,
        up: f32,
        delta_time: f32,
        is_sprinting: bool,
    ) {
        // Clamp delta_time to prevent huge jumps (max 100ms)
        let dt = delta_time.clamp(0.001, 0.1);

        let forward_dir = self.get_forward();
        let right_dir = self.get_right();

        // Calculate speed based on sprinting
        let speed = if is_sprinting {
            self.move_speed * self.sprint_multiplier
        } else {
            self.move_speed
        };

        // Horizontal movement (XZ plane) - frame independent
        let forward_xz = Vec3::new(forward_dir.x, 0.0, forward_dir.z).normalize_or_zero();
        let right_xz = Vec3::new(right_dir.x, 0.0, right_dir.z).normalize_or_zero();

        self.position += forward_xz * forward * speed * dt;
        self.position += right_xz * right * speed * dt;

        // Vertical movement - only in fly mode (Space/E/Q/Shift)
        // In grounded mode, up input triggers jump
        if up > 0.0 && self.is_grounded {
            // Jump
            self.vertical_velocity = self.jump_velocity;
            self.is_grounded = false;
        } else if !self.is_grounded || up < 0.0 {
            // Flying down or airborne - allow vertical movement
            self.position.y += up * speed * dt;
        }
    }

    /// Update camera position based on movement input (legacy - no delta_time)
    ///
    /// DEPRECATED: Use update_movement_with_physics() for frame-independent movement.
    ///
    /// # Arguments
    /// * `forward` - Move forward (positive) or backward (negative)
    /// * `right` - Move right (positive) or left (negative)
    /// * `up` - Move up (positive) or down (negative)
    pub fn update_movement(&mut self, forward: f32, right: f32, up: f32) {
        // Legacy behavior - assumes 60 FPS for backwards compatibility
        self.update_movement_with_physics(forward, right, up, 1.0 / 60.0, false);
    }

    /// Apply gravity and ground collision (for flat worlds)
    ///
    /// Call this every frame after movement to apply gravity and keep player on ground.
    /// For spherical worlds, use `apply_spherical_physics` instead.
    ///
    /// # Arguments
    /// * `delta_time` - Time since last frame in seconds
    /// * `ground_height` - The height of the ground at the player's current XZ position
    pub fn apply_gravity(&mut self, delta_time: f32, ground_height: f32) {
        let dt = delta_time.clamp(0.001, 0.1);

        // Apply gravity to vertical velocity
        if !self.is_grounded {
            self.vertical_velocity -= self.gravity * dt;
        }

        // Apply vertical velocity to position
        self.position.y += self.vertical_velocity * dt;

        // Ground collision - min_height is the player's eye level above feet
        let min_y = ground_height + self.min_height;
        if self.position.y <= min_y {
            self.position.y = min_y;
            self.vertical_velocity = 0.0;
            self.is_grounded = true;
        }
    }

    // =========================================================================
    // SPHERICAL PLANET PHYSICS
    // =========================================================================

    /// Configure the controller for a spherical planet.
    ///
    /// This sets up radial gravity pointing toward the planet center.
    /// The planet center is placed at (0, -planet_radius, 0) so the surface
    /// is approximately at Y=0 in the center of the world.
    ///
    /// Also teleports the player to be on the surface at the current XZ position.
    ///
    /// # Arguments
    /// * `planet_radius` - Radius of the planet in meters (e.g., 3183.0 for 10km world)
    pub fn set_spherical_world(&mut self, planet_radius: f32) {
        self.planet_radius = planet_radius;
        self.planet_center = Vec3::new(0.0, -planet_radius, 0.0);

        // Teleport player to be ON the surface at their current XZ position
        // Direction from center to current position
        let to_player = self.position - self.planet_center;
        let current_distance = to_player.length();

        if current_distance > 0.01 {
            // Normalize direction and place player at surface + standing height
            let direction = to_player / current_distance;
            let target_distance = planet_radius + self.min_height;
            self.position = self.planet_center + direction * target_distance;
            self.surface_normal = direction;
        } else {
            // Player is at center (shouldn't happen) - place at north pole
            self.position = Vec3::new(0.0, self.min_height, 0.0);
            self.surface_normal = Vec3::Y;
        }

        // Reset velocity to prevent falling through
        self.velocity = Vec3::ZERO;
        self.vertical_velocity = 0.0;
        self.is_grounded = true;

        // Update player position
        self.player_position = self.position - self.surface_normal * self.first_person_head_height;
    }

    /// Check if we're on a spherical world
    pub fn is_spherical_world(&self) -> bool {
        self.planet_radius > 0.0
    }

    /// Get the planet center position
    pub fn get_planet_center(&self) -> Vec3 {
        self.planet_center
    }

    /// Get the current surface normal (up direction at player position)
    ///
    /// On a flat world, this is always Vec3::Y (0, 1, 0).
    /// On a spherical world, this points away from the planet center.
    pub fn get_surface_normal(&self) -> Vec3 {
        self.surface_normal
    }

    /// Update the surface normal based on player position.
    ///
    /// For spherical worlds, the "up" direction changes based on where
    /// you are on the sphere.
    pub fn update_surface_normal(&mut self) {
        if self.planet_radius <= 0.0 {
            // Flat world - up is always Y
            self.surface_normal = Vec3::Y;
            return;
        }

        // Direction from planet center to player = surface normal (up)
        let to_player = self.position - self.planet_center;
        let distance = to_player.length();

        if distance > 0.01 {
            self.surface_normal = to_player / distance;
        } else {
            // Player is at planet center (shouldn't happen)
            self.surface_normal = Vec3::Y;
        }
    }

    /// Get the gravity direction at the player's current position.
    ///
    /// For flat worlds: always (0, -1, 0)
    /// For spherical worlds: points toward planet center (radial gravity)
    pub fn get_gravity_direction(&self) -> Vec3 {
        if self.planet_radius <= 0.0 {
            // Flat world - gravity is straight down
            Vec3::NEG_Y
        } else {
            // Spherical world - gravity points to planet center
            -self.surface_normal
        }
    }

    /// Calculate the height above the planet surface.
    ///
    /// For flat worlds: returns position.y
    /// For spherical worlds: returns distance from planet center minus radius
    pub fn get_height_above_surface(&self) -> f32 {
        if self.planet_radius <= 0.0 {
            // Flat world
            self.position.y
        } else {
            // Spherical world - distance from center minus radius
            let distance = (self.position - self.planet_center).length();
            distance - self.planet_radius
        }
    }

    /// Apply spherical gravity, normal force, and friction.
    ///
    /// This replaces `apply_gravity` for spherical worlds. It handles:
    /// - Radial gravity toward planet center
    /// - Normal force (ground pushback) when on surface
    /// - Surface friction that slows movement
    ///
    /// # Physics Model
    /// - Gravity force: F_g = m * g, direction = toward planet center
    /// - Normal force: F_n = -F_g (perpendicular to surface) when grounded
    /// - Friction force: F_f = μ * |F_n|, opposes velocity along surface tangent
    ///
    /// # Arguments
    /// * `delta_time` - Time since last frame in seconds
    /// * `terrain_offset` - Additional height offset from terrain (caves, hills)
    pub fn apply_spherical_physics(&mut self, delta_time: f32, terrain_offset: f32) {
        let dt = delta_time.clamp(0.001, 0.1);

        // Update surface normal based on current position
        self.update_surface_normal();

        // Get gravity direction and force
        let gravity_dir = self.get_gravity_direction();
        let gravity_force = gravity_dir * self.gravity; // acceleration (m/s²)

        // Calculate height above surface
        let surface_height = self.planet_radius + terrain_offset;
        let distance_from_center = (self.position - self.planet_center).length();
        let height_above_surface = distance_from_center - surface_height;

        // Minimum height is player eye level above surface
        let min_height_above_surface = self.min_height;

        // Tolerance zone to prevent bouncing - if within 0.05m of target, treat as grounded
        let ground_tolerance = 0.05;

        if height_above_surface <= min_height_above_surface + ground_tolerance {
            // ON THE GROUND - apply normal force and friction
            self.is_grounded = true;

            // Only snap if actually below the target height (not in tolerance zone above)
            if height_above_surface < min_height_above_surface {
                let target_distance = surface_height + min_height_above_surface;
                let correction = target_distance - distance_from_center;
                self.position += self.surface_normal * correction;
            }

            // Cancel velocity component toward the ground (normal force)
            let velocity_toward_ground = self.velocity.dot(-self.surface_normal);
            if velocity_toward_ground > 0.0 {
                // Remove the component moving into the ground
                self.velocity += self.surface_normal * velocity_toward_ground;
            }

            // Apply friction to tangential velocity (along surface)
            // Friction force = μ * Normal force magnitude
            // Normal force = m * g (player weight)
            let normal_force_magnitude = self.player_mass * self.gravity;
            let friction_force = self.friction_coefficient * normal_force_magnitude;
            let friction_deceleration = friction_force / self.player_mass; // a = F/m

            // Get tangential velocity (velocity along the surface)
            let tangential_velocity =
                self.velocity - self.surface_normal * self.velocity.dot(self.surface_normal);
            let tangential_speed = tangential_velocity.length();

            if tangential_speed > 0.01 {
                // Apply friction (opposite to velocity direction)
                let friction_direction = -tangential_velocity.normalize();
                let friction_delta = friction_deceleration * dt;

                if friction_delta >= tangential_speed {
                    // Friction stops the player completely
                    self.velocity = Vec3::ZERO;
                } else {
                    // Reduce speed by friction
                    self.velocity += friction_direction * friction_delta;
                }
            }

            // Reset vertical velocity (we're on ground)
            self.vertical_velocity = 0.0;
        } else {
            // IN THE AIR - apply gravity
            self.is_grounded = false;

            // Apply gravity to velocity
            self.velocity += gravity_force * dt;

            // Also track vertical velocity for jump logic
            self.vertical_velocity = self.velocity.dot(-self.surface_normal);
        }

        // Apply velocity to position
        self.position += self.velocity * dt;

        // Update player position to match camera position (for third-person)
        self.player_position = self.position - self.surface_normal * self.first_person_head_height;
    }

    /// Apply movement on a spherical surface.
    ///
    /// Converts WASD input into movement along the planet surface tangent.
    /// The "forward" direction is relative to the camera yaw, projected onto
    /// the surface tangent plane.
    ///
    /// # Arguments
    /// * `forward` - Forward/backward input (-1 to 1)
    /// * `right` - Left/right input (-1 to 1)
    /// * `delta_time` - Time since last frame
    /// * `is_sprinting` - Whether sprint key is held
    pub fn apply_spherical_movement(
        &mut self,
        forward: f32,
        right: f32,
        delta_time: f32,
        is_sprinting: bool,
    ) {
        let dt = delta_time.clamp(0.001, 0.1);

        // Get the surface tangent plane basis vectors
        let up = self.surface_normal;

        // Forward direction: project camera forward onto the tangent plane
        // Camera forward is based on yaw (ignoring pitch for ground movement)
        let camera_forward_world = Vec3::new(self.yaw.sin(), 0.0, -self.yaw.cos());

        // Project onto tangent plane and normalize
        let forward_tangent =
            (camera_forward_world - up * camera_forward_world.dot(up)).normalize_or_zero();

        // Right direction is perpendicular to forward and up
        // forward.cross(up) gives the right vector in right-handed coordinate system
        // When forward = (0,0,-1) and up = (0,1,0):
        // (0,0,-1) × (0,1,0) = (0*0-(-1)*1, (-1)*0-0*0, 0*1-0*0) = (1, 0, 0) = right ✓
        let right_tangent = forward_tangent.cross(up).normalize_or_zero();

        // Calculate target velocity based on input
        let speed = if is_sprinting {
            self.move_speed * self.sprint_multiplier
        } else {
            self.move_speed
        };

        // Movement direction on the surface
        // Note: We negate both tangent vectors because:
        // - forward_tangent points away from where camera looks, so negate for W to move toward target
        // - right_tangent points left, so negate for D to move right
        let move_direction = -forward_tangent * forward - right_tangent * right;
        let move_direction = move_direction.normalize_or_zero();

        // Apply movement as velocity change (acceleration model)
        // This allows momentum and smooth stopping via friction
        let acceleration = 20.0; // m/s² - how fast we reach target speed

        if move_direction.length() > 0.01 {
            // Accelerate toward target velocity
            let target_velocity = move_direction * speed;
            let current_tangent_velocity = self.velocity - up * self.velocity.dot(up);
            let velocity_diff = target_velocity - current_tangent_velocity;

            // Apply acceleration (limited by delta time)
            let accel_this_frame = acceleration * dt;
            if velocity_diff.length() <= accel_this_frame {
                // Reached target
                self.velocity = target_velocity + up * self.velocity.dot(up);
            } else {
                // Accelerate toward target
                self.velocity += velocity_diff.normalize() * accel_this_frame;
            }
        }

        // Note: deceleration is handled by friction in apply_spherical_physics
    }

    /// Handle jumping on a spherical planet.
    ///
    /// Jump direction is along the surface normal (away from planet center).
    pub fn spherical_jump(&mut self) {
        if self.is_grounded {
            // Jump velocity is along surface normal (up from the surface)
            self.velocity += self.surface_normal * self.jump_velocity;
            self.vertical_velocity = self.jump_velocity;
            self.is_grounded = false;
        }
    }

    /// Get the player's current velocity
    pub fn get_velocity(&self) -> Vec3 {
        self.velocity
    }

    /// Get the player's current speed (magnitude of velocity)
    pub fn get_speed(&self) -> f32 {
        self.velocity.length()
    }

    /// Get tangential speed (speed along surface, ignoring vertical component)
    pub fn get_tangential_speed(&self) -> f32 {
        let tangential =
            self.velocity - self.surface_normal * self.velocity.dot(self.surface_normal);
        tangential.length()
    }

    /// Apply movement from held keys (frame-rate independent)
    ///
    /// # Arguments
    /// * `forward` - W key held
    /// * `backward` - S key held
    /// * `left` - A key held
    /// * `right` - D key held
    /// * `up` - Space/E key held (jump when grounded)
    /// * `down` - Shift/Q key held
    /// * `delta_time` - Time since last frame in seconds
    /// * `is_sprinting` - Whether sprint key is held
    pub fn apply_key_movement_with_physics(
        &mut self,
        forward: bool,
        backward: bool,
        left: bool,
        right: bool,
        up: bool,
        down: bool,
        delta_time: f32,
        is_sprinting: bool,
    ) {
        let forward_input = if forward { 1.0 } else { 0.0 } - if backward { 1.0 } else { 0.0 };
        let right_input = if right { 1.0 } else { 0.0 } - if left { 1.0 } else { 0.0 };
        let up_input = if up { 1.0 } else { 0.0 } - if down { 1.0 } else { 0.0 };

        self.update_movement_with_physics(
            forward_input,
            right_input,
            up_input,
            delta_time,
            is_sprinting,
        );
    }

    /// Apply movement from held keys (legacy - no delta_time)
    ///
    /// DEPRECATED: Use apply_key_movement_with_physics() for frame-independent movement.
    ///
    /// # Arguments
    /// * `forward` - W key held
    /// * `backward` - S key held
    /// * `left` - A key held
    /// * `right` - D key held
    /// * `up` - Space/E key held
    /// * `down` - Shift/Q key held
    pub fn apply_key_movement(
        &mut self,
        forward: bool,
        backward: bool,
        left: bool,
        right: bool,
        up: bool,
        down: bool,
    ) {
        let forward_input = if forward { 1.0 } else { 0.0 } - if backward { 1.0 } else { 0.0 };
        let right_input = if right { 1.0 } else { 0.0 } - if left { 1.0 } else { 0.0 };
        let up_input = if up { 1.0 } else { 0.0 } - if down { 1.0 } else { 0.0 };

        self.update_movement(forward_input, right_input, up_input);
    }

    /// Check if player is on the ground
    pub fn is_on_ground(&self) -> bool {
        self.is_grounded
    }

    /// Force the player to be grounded (e.g., after teleport)
    pub fn set_grounded(&mut self, grounded: bool) {
        self.is_grounded = grounded;
        if grounded {
            self.vertical_velocity = 0.0;
        }
    }

    /// Set crouching state
    ///
    /// When crouching, the player's eye level is lowered. Hold Ctrl to crouch.
    /// Small creatures can crouch even lower (down to min_crouch_height).
    pub fn set_crouching(&mut self, crouching: bool) {
        self.is_crouching = crouching;
    }

    /// Check if player is currently crouching
    pub fn is_crouched(&self) -> bool {
        self.is_crouching
    }

    /// Get current player height (accounting for crouch)
    pub fn get_current_height(&self) -> f32 {
        self.current_height
    }

    /// Set the crouch height (how low the player crouches)
    ///
    /// Can be set very low (0.3m) for small creatures or prone position.
    pub fn set_crouch_height(&mut self, height: f32) {
        self.crouch_height = height.max(self.min_crouch_height);
    }

    /// Update crouch height smoothly (call every frame)
    ///
    /// Smoothly transitions between standing and crouched height.
    pub fn update_crouch(&mut self, delta_time: f32) {
        let target_height = if self.is_crouching {
            self.crouch_height
        } else {
            self.standing_height
        };

        // Smooth transition (faster going down, slower standing up)
        let speed = if self.is_crouching { 12.0 } else { 8.0 };
        let diff = target_height - self.current_height;

        if diff.abs() < 0.01 {
            self.current_height = target_height;
        } else {
            self.current_height += diff * speed * delta_time;
        }

        // Update min_height to match current crouch state
        self.min_height = self.current_height;
    }

    /// Move camera forward/backward (typically from scroll wheel)
    pub fn zoom(&mut self, amount: f32) {
        let forward = self.get_forward();
        self.position += forward * amount;
    }

    /// Rotate camera by delta angles
    ///
    /// # Arguments
    /// * `delta_yaw` - Change in yaw (radians)
    /// * `delta_pitch` - Change in pitch (radians)
    pub fn rotate(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.yaw += delta_yaw;
        self.pitch = (self.pitch + delta_pitch).clamp(self.pitch_limits.0, self.pitch_limits.1);
    }

    /// Reset camera to default position and orientation
    pub fn reset(&mut self) {
        self.position = Vec3::new(0.0, 2.0, 3.0);
        self.yaw = 0.0;
        self.pitch = -0.2;
        self.distance = 3.0;
        self.actual_distance = 3.0;
        self.velocity = Vec3::ZERO;
    }

    /// Get the spring damping configuration
    pub fn get_spring_config(&self) -> SpringConfig {
        self.spring_config
    }

    /// Set the spring damping configuration
    pub fn set_spring_config(&mut self, config: SpringConfig) {
        self.spring_config = config;
    }

    /// Get the camera collision configuration
    pub fn get_collision_config(&self) -> CameraCollisionConfig {
        self.collision_config
    }

    /// Set the camera collision configuration
    pub fn set_collision_config(&mut self, config: CameraCollisionConfig) {
        self.collision_config = config;
    }

    /// Enable or disable camera collision
    pub fn set_collision_enabled(&mut self, enabled: bool) {
        self.collision_config.enabled = enabled;
    }

    /// Point camera at a specific world position
    pub fn look_at(&mut self, target: Vec3) {
        let to_target = target - self.position;
        if to_target.length() > 0.1 {
            self.yaw = to_target.x.atan2(-to_target.z);
            self.pitch = (to_target.y / to_target.length())
                .asin()
                .clamp(self.pitch_limits.0, self.pitch_limits.1);
        }
    }

    /// Set camera mode with smooth transition
    ///
    /// This initiates a 0.3 second transition between camera modes.
    pub fn set_mode(&mut self, mode: CameraMode) {
        if mode == self.mode && !self.transition.active {
            return;
        }

        // Start transition
        self.target_mode = mode;
        self.transition.active = true;
        self.transition.progress = 0.0;
        self.transition.from_position = self.position;
        self.transition.from_distance = self.distance;

        // Calculate target position based on mode
        match mode {
            CameraMode::ThirdPerson => {
                self.transition.to_distance = self.third_person_offset.z;
                self.transition.to_position = self.calculate_third_person_position();
            }
            CameraMode::FirstPerson => {
                self.transition.to_distance = 0.0;
                self.transition.to_position = self.calculate_first_person_position();
            }
        }
    }

    /// Toggle between ThirdPerson and FirstPerson modes (V key)
    ///
    /// Initiates a smooth 0.3s transition to the opposite mode.
    pub fn toggle_mode(&mut self) {
        let new_mode = match self.target_mode {
            CameraMode::ThirdPerson => CameraMode::FirstPerson,
            CameraMode::FirstPerson => CameraMode::ThirdPerson,
        };
        self.set_mode(new_mode);
    }

    /// Calculate the ideal third-person camera position (behind and above player)
    ///
    /// Uses the configured offset: 3m behind, 2m above by default (Unity-style).
    fn calculate_third_person_position(&self) -> Vec3 {
        self.calculate_third_person_position_with_distance(self.third_person_offset.z)
    }

    /// Calculate third-person position with a specific distance (for collision adjustment)
    fn calculate_third_person_position_with_distance(&self, distance: f32) -> Vec3 {
        // Unity-style orbit: camera positioned behind and above player
        let offset_height = self.third_person_offset.y;

        // Calculate position behind player based on yaw
        // Note: camera is BEHIND the player, so we add to get farther from player
        let horizontal_offset =
            Vec3::new(self.yaw.sin() * distance, 0.0, -self.yaw.cos() * distance);

        self.player_position + Vec3::new(0.0, offset_height, 0.0) + horizontal_offset
    }

    /// Calculate the first-person camera position (at player eye level)
    fn calculate_first_person_position(&self) -> Vec3 {
        self.player_position + Vec3::new(0.0, self.first_person_head_height, 0.0)
    }

    /// Apply spring damping to smoothly follow target position
    ///
    /// Uses a critically damped spring for smooth, responsive camera movement
    /// without overshoot.
    fn apply_spring_damping(&mut self, target: Vec3, delta_time: f32) {
        // Spring physics: F = -stiffness * (x - target) - damping * velocity
        // Using critically damped spring for smooth camera follow
        let stiffness = self.spring_config.stiffness;
        let damping = self.spring_config.damping;

        // Calculate spring force
        let displacement = self.position - target;
        let spring_force = -stiffness * displacement - damping * self.velocity;

        // Update velocity and position
        self.velocity += spring_force * delta_time;
        self.position += self.velocity * delta_time;
    }

    /// Perform camera collision raycast
    ///
    /// Casts a ray from the look target (player) to the ideal camera position.
    /// Returns the safe camera distance (reduced if collision detected).
    ///
    /// # Arguments
    /// * `collision_check` - Optional closure that takes (origin, direction, max_dist)
    ///   and returns the distance to first hit, or None if no collision
    pub fn check_camera_collision<F>(&mut self, collision_check: F) -> f32
    where
        F: FnOnce(Vec3, Vec3, f32) -> Option<f32>,
    {
        if !self.collision_config.enabled || self.mode != CameraMode::ThirdPerson {
            return self.third_person_offset.z;
        }

        // Calculate ray from player look target to ideal camera position
        let look_target = self.player_position + Vec3::new(0.0, self.third_person_look_height, 0.0);
        let ideal_pos = self.calculate_third_person_position();
        let direction = (ideal_pos - look_target).normalize_or_zero();
        let max_distance = self.third_person_offset.z + self.collision_config.radius;

        // Perform collision check
        if let Some(hit_distance) = collision_check(look_target, direction, max_distance) {
            // Collision detected - move camera closer
            let safe_distance = (hit_distance - self.collision_config.min_distance)
                .max(self.collision_config.radius);
            self.actual_distance = safe_distance.min(self.third_person_offset.z);
        } else {
            // No collision - use full distance
            self.actual_distance = self.third_person_offset.z;
        }

        self.actual_distance
    }

    /// Get the actual camera distance after collision adjustment
    pub fn get_actual_distance(&self) -> f32 {
        self.actual_distance
    }

    /// Update the camera state (call every frame with delta time)
    ///
    /// Handles smooth transitions between modes, spring damping for smooth follow,
    /// and updates camera position based on the current mode.
    ///
    /// # Arguments
    /// * `delta_time` - Time since last frame in seconds
    pub fn update(&mut self, delta_time: f32) {
        // Handle transition
        if self.transition.active {
            self.transition.progress += delta_time / self.transition.duration;

            if self.transition.progress >= 1.0 {
                // Transition complete
                self.transition.progress = 1.0;
                self.transition.active = false;
                self.mode = self.target_mode;
                self.position = self.transition.to_position;
                self.distance = self.transition.to_distance;
                self.velocity = Vec3::ZERO; // Reset velocity after transition
            } else {
                // Smooth interpolation using ease-in-out
                let t = self.ease_in_out(self.transition.progress);
                self.position = self
                    .transition
                    .from_position
                    .lerp(self.transition.to_position, t);
                self.distance = self.transition.from_distance
                    + (self.transition.to_distance - self.transition.from_distance) * t;
            }
        } else {
            // Update position based on current mode with spring damping
            match self.mode {
                CameraMode::ThirdPerson => {
                    // Use actual_distance (which may be reduced due to collision)
                    let target =
                        self.calculate_third_person_position_with_distance(self.actual_distance);
                    self.apply_spring_damping(target, delta_time);
                }
                CameraMode::FirstPerson => {
                    // First-person follows player directly (no spring for snappy feel)
                    self.position = self.calculate_first_person_position();
                    self.velocity = Vec3::ZERO;
                }
            }
        }
    }

    /// Update camera with collision check
    ///
    /// Convenience method that performs collision check before normal update.
    ///
    /// # Arguments
    /// * `delta_time` - Time since last frame in seconds
    /// * `collision_check` - Closure that performs collision raycast
    pub fn update_with_collision<F>(&mut self, delta_time: f32, collision_check: F)
    where
        F: FnOnce(Vec3, Vec3, f32) -> Option<f32>,
    {
        self.check_camera_collision(collision_check);
        self.update(delta_time);
    }

    /// Smooth ease-in-out interpolation function
    fn ease_in_out(&self, t: f32) -> f32 {
        if t < 0.5 {
            2.0 * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
        }
    }

    /// Check if currently transitioning between modes
    pub fn is_transitioning(&self) -> bool {
        self.transition.active
    }

    /// Get the current camera mode (during transition, returns the source mode)
    pub fn get_mode(&self) -> CameraMode {
        self.mode
    }

    /// Get the target camera mode (the mode being transitioned to)
    pub fn get_target_mode(&self) -> CameraMode {
        self.target_mode
    }

    /// Check if camera is in third-person mode
    pub fn is_third_person(&self) -> bool {
        self.mode == CameraMode::ThirdPerson && !self.transition.active
    }

    /// Check if camera is in first-person mode
    pub fn is_first_person(&self) -> bool {
        self.mode == CameraMode::FirstPerson && !self.transition.active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_camera() {
        let camera = CameraController::new();
        assert_eq!(camera.mode, CameraMode::ThirdPerson);
        assert!(camera.position.y > 0.0);
    }

    #[test]
    fn test_forward_vector() {
        let mut camera = CameraController::new();
        camera.yaw = 0.0;
        camera.pitch = 0.0;

        let forward = camera.get_forward();
        // When yaw=0 and pitch=0, should look towards -Z
        assert!(forward.z < 0.0);
        assert!(forward.y.abs() < 0.01);
    }

    #[test]
    fn test_pitch_clamping() {
        let mut camera = CameraController::new();
        camera.rotate(0.0, 10.0); // Try to rotate way past limit
        assert!(camera.pitch <= camera.pitch_limits.1);
        // Verify pitch is within -89° to +89° range
        assert!(camera.pitch <= PITCH_LIMIT_MAX);
        assert!(camera.pitch >= PITCH_LIMIT_MIN);
    }

    #[test]
    fn test_pitch_limits_are_89_degrees() {
        let camera = CameraController::new();
        // Check that pitch limits are approximately ±89 degrees
        let expected_limit = 89.0 * std::f32::consts::PI / 180.0;
        assert!((camera.pitch_limits.0 - (-expected_limit)).abs() < 0.001);
        assert!((camera.pitch_limits.1 - expected_limit).abs() < 0.001);
    }

    #[test]
    fn test_mode_toggle() {
        let mut camera = CameraController::new();
        assert_eq!(camera.get_mode(), CameraMode::ThirdPerson);
        assert_eq!(camera.get_target_mode(), CameraMode::ThirdPerson);

        // Toggle to first-person
        camera.toggle_mode();
        assert!(camera.is_transitioning());
        assert_eq!(camera.get_target_mode(), CameraMode::FirstPerson);

        // Complete transition
        camera.update(1.0); // More than 0.3s
        assert!(!camera.is_transitioning());
        assert_eq!(camera.get_mode(), CameraMode::FirstPerson);

        // Toggle back to third-person
        camera.toggle_mode();
        assert!(camera.is_transitioning());
        assert_eq!(camera.get_target_mode(), CameraMode::ThirdPerson);
    }

    #[test]
    fn test_transition_duration() {
        let mut camera = CameraController::new();
        camera.toggle_mode(); // Start transition

        // After 0.15s (half of 0.3s), should still be transitioning
        camera.update(0.15);
        assert!(camera.is_transitioning());

        // After another 0.2s (total 0.35s), transition should be complete
        camera.update(0.2);
        assert!(!camera.is_transitioning());
    }

    #[test]
    fn test_third_person_position() {
        let mut camera = CameraController::new();
        camera.set_player_position(Vec3::new(10.0, 0.0, 10.0));
        camera.update(0.0);

        // Camera should be behind and above player
        assert!(camera.position.y > camera.player_position.y);
        let horizontal_dist = ((camera.position.x - camera.player_position.x).powi(2)
            + (camera.position.z - camera.player_position.z).powi(2))
        .sqrt();
        assert!(horizontal_dist > 0.0); // Camera should be behind player
    }

    #[test]
    fn test_first_person_position() {
        let mut camera = CameraController::new();
        camera.set_player_position(Vec3::new(5.0, 0.0, 5.0));
        camera.set_mode(CameraMode::FirstPerson);
        camera.update(1.0); // Complete transition

        // Camera should be at player head height
        assert!((camera.position.x - 5.0).abs() < 0.001);
        assert!((camera.position.z - 5.0).abs() < 0.001);
        assert!(camera.position.y > camera.player_position.y);
        assert!(
            (camera.position.y - (camera.player_position.y + camera.first_person_head_height))
                .abs()
                < 0.001
        );
    }

    #[test]
    fn test_third_person_offset_defaults() {
        let camera = CameraController::new();
        // Third-person: 3m behind, 2m above
        assert_eq!(camera.third_person_offset.z, 3.0);
        assert_eq!(camera.third_person_offset.y, 2.0);
        // Look target: 0.5m above player
        assert_eq!(camera.third_person_look_height, 0.5);
    }

    #[test]
    fn test_first_person_eye_level() {
        let camera = CameraController::new();
        // First-person: 1.6m eye level
        assert_eq!(camera.first_person_head_height, 1.6);
    }

    #[test]
    fn test_spring_config_defaults() {
        let camera = CameraController::new();
        // Spring damping: stiffness=10, damping=5
        assert_eq!(camera.spring_config.stiffness, 10.0);
        assert_eq!(camera.spring_config.damping, 5.0);
    }

    #[test]
    fn test_collision_config_defaults() {
        let camera = CameraController::new();
        assert!(camera.collision_config.enabled);
        assert!(camera.collision_config.min_distance > 0.0);
        assert!(camera.collision_config.radius > 0.0);
    }

    #[test]
    fn test_camera_collision_no_hit() {
        let mut camera = CameraController::new();
        camera.set_player_position(Vec3::new(0.0, 0.0, 0.0));

        // No collision - should return full distance
        let distance = camera.check_camera_collision(|_, _, _| None);
        assert_eq!(distance, camera.third_person_offset.z);
    }

    #[test]
    fn test_camera_collision_with_hit() {
        let mut camera = CameraController::new();
        camera.set_player_position(Vec3::new(0.0, 0.0, 0.0));

        // Collision at 1.5m - should reduce distance
        let distance = camera.check_camera_collision(|_, _, _| Some(1.5));
        assert!(distance < camera.third_person_offset.z);
        assert!(distance > 0.0);
    }

    #[test]
    fn test_player_rotation_follows_camera_when_moving() {
        let mut camera = CameraController::new();
        camera.yaw = 1.0; // Camera facing a specific direction
        camera.player_yaw = 0.0; // Player facing different direction

        // Update with movement - player should rotate towards camera yaw
        camera.update_player_rotation(true, 0.1);
        assert!(camera.player_yaw != 0.0); // Player yaw should have changed
        assert!(camera.player_yaw > 0.0); // Should be moving towards camera yaw
    }

    #[test]
    fn test_player_rotation_stays_when_not_moving() {
        let mut camera = CameraController::new();
        camera.yaw = 1.0;
        camera.player_yaw = 0.0;

        // Update without movement - player should NOT rotate
        camera.update_player_rotation(false, 0.1);
        assert_eq!(camera.player_yaw, 0.0);
    }

    #[test]
    fn test_third_person_look_target() {
        // FPS-style camera: get_target() returns position + forward * 10.0
        // Setting player_position doesn't directly affect get_target()
        // because it's based on camera's yaw/pitch, not orbital around player
        let camera = CameraController::new();

        let target = camera.get_target();
        let forward = camera.get_forward();

        // Target should be 10 units along forward direction from camera position
        let expected_target = camera.position + forward * 10.0;
        assert!((target.x - expected_target.x).abs() < 0.001);
        assert!((target.y - expected_target.y).abs() < 0.001);
        assert!((target.z - expected_target.z).abs() < 0.001);
    }

    #[test]
    fn test_spring_damping_moves_towards_target() {
        let mut camera = CameraController::new();
        camera.set_player_position(Vec3::new(100.0, 0.0, 100.0));
        let initial_pos = camera.position;

        // Update multiple times to let spring catch up
        for _ in 0..100 {
            camera.update(0.016); // ~60fps
        }

        // Camera should have moved significantly towards player
        let dist_before = (initial_pos - camera.player_position).length();
        let dist_after = (camera.position - camera.player_position).length();
        assert!(dist_after < dist_before);
    }
}
