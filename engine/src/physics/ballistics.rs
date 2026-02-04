//! Ballistics simulation for projectile trajectories
//!
//! Provides types for simulating cannon projectiles with gravity and air drag.
//! No external physics dependencies - implements our own ballistics math.
//!
//! # Example
//!
//! ```ignore
//! use battle_tok_engine::physics::ballistics::{Projectile, BallisticsConfig, ProjectileState};
//! use glam::Vec3;
//!
//! let config = BallisticsConfig::default();
//! let mut projectile = Projectile {
//!     position: Vec3::new(0.0, 10.0, 0.0),
//!     velocity: Vec3::new(50.0, 30.0, 0.0),
//!     mass: 5.0,
//!     drag_coefficient: 0.47,
//!     radius: 0.1,
//!     active: true,
//! };
//! ```

use glam::Vec3;

/// A projectile being simulated through the air.
///
/// Contains all physical properties needed for ballistic trajectory calculation.
#[derive(Debug, Clone, Copy)]
pub struct Projectile {
    /// Current position in world space (meters)
    pub position: Vec3,
    /// Current velocity vector (meters/second)
    pub velocity: Vec3,
    /// Mass of the projectile (kilograms)
    pub mass: f32,
    /// Drag coefficient (dimensionless, typically 0.1-1.0 for projectiles)
    pub drag_coefficient: f32,
    /// Radius of the projectile (meters) - used for drag area calculation
    pub radius: f32,
    /// Whether the projectile is still being simulated
    pub active: bool,
    /// Total distance traveled since spawn (meters)
    pub distance_traveled: f32,
}

impl Default for Projectile {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            mass: 1.0,
            drag_coefficient: 0.47, // Sphere drag coefficient
            radius: 0.05,           // 5cm radius (10cm diameter cannonball)
            active: true,
            distance_traveled: 0.0,
        }
    }
}

impl Projectile {
    /// Spawn a new projectile with the given initial conditions.
    ///
    /// # Arguments
    /// * `position` - Initial position in world space (meters)
    /// * `direction` - Normalized direction vector (will be normalized if not)
    /// * `speed` - Initial speed (meters/second)
    /// * `mass` - Mass of the projectile (kilograms)
    ///
    /// # Example
    /// ```ignore
    /// let projectile = Projectile::spawn(
    ///     Vec3::new(0.0, 2.0, 0.0),
    ///     Vec3::new(1.0, 0.5, 0.0),
    ///     50.0,
    ///     5.0,
    /// );
    /// ```
    pub fn spawn(position: Vec3, direction: Vec3, speed: f32, mass: f32) -> Self {
        let normalized_dir = direction.normalize_or_zero();
        Self {
            position,
            velocity: normalized_dir * speed,
            mass: mass.max(0.001), // Prevent division by zero
            drag_coefficient: 0.47,
            radius: 0.15, // 15cm radius cannonball
            active: true,
            distance_traveled: 0.0,
        }
    }

    /// Integrate the projectile's physics over a time step.
    ///
    /// Uses semi-implicit Euler integration with air drag:
    /// - acceleration = gravity + (drag_force / mass)
    /// - velocity += acceleration * dt
    /// - position += velocity * dt
    ///
    /// Air drag formula: F_drag = -0.5 * air_density * drag_coeff * area * |v|^2 * normalize(v)
    ///
    /// # Arguments
    /// * `config` - Ballistics configuration (gravity, air density, expired_distance)
    /// * `dt` - Time step in seconds
    ///
    /// # Returns
    /// The current state of the projectile after integration.
    pub fn integrate(&mut self, config: &BallisticsConfig, dt: f32) -> ProjectileState {
        if !self.active {
            return ProjectileState::Expired;
        }

        // Calculate air drag force
        let speed = self.velocity.length();
        let drag_force = if speed > 0.001 {
            // Cross-sectional area of sphere: π * r²
            let area = std::f32::consts::PI * self.radius * self.radius;
            // Drag force magnitude: 0.5 * ρ * Cd * A * v²
            let drag_magnitude =
                0.5 * config.air_density * self.drag_coefficient * area * speed * speed;
            // Direction opposite to velocity
            -self.velocity.normalize() * drag_magnitude
        } else {
            Vec3::ZERO
        };

        // Calculate acceleration: gravity + drag/mass
        let acceleration = config.gravity + drag_force / self.mass;

        // Semi-implicit Euler integration
        // Update velocity first (semi-implicit)
        self.velocity += acceleration * dt;

        // Calculate displacement and track distance traveled
        let displacement = self.velocity * dt;
        let displacement_length = displacement.length();

        // Update position with new velocity
        self.position += displacement;

        // Track total distance traveled
        self.distance_traveled += displacement_length;

        // Check if projectile has traveled too far (expired)
        if self.distance_traveled >= config.expired_distance {
            self.active = false;
            return ProjectileState::Expired;
        }

        // Check for ground collision (y < 0)
        if self.position.y < 0.0 {
            self.active = false;
            // Clamp to ground level
            self.position.y = 0.0;
            return ProjectileState::Hit {
                position: self.position,
                normal: Vec3::Y, // Ground normal points up
            };
        }

        ProjectileState::Flying
    }

    /// Check if the projectile has traveled beyond a maximum distance from origin.
    ///
    /// # Arguments
    /// * `max_distance` - Maximum allowed distance from origin (meters)
    ///
    /// # Returns
    /// True if the projectile should be expired due to distance.
    pub fn is_beyond_distance(&self, max_distance: f32) -> bool {
        self.position.length() > max_distance
    }
}

/// Configuration for the ballistics simulation environment.
///
/// Contains global physics parameters that affect all projectiles.
#[derive(Debug, Clone, Copy)]
pub struct BallisticsConfig {
    /// Gravity acceleration vector (m/s²).
    /// Earth default: Vec3::new(0.0, -9.81, 0.0)
    pub gravity: Vec3,
    /// Air density (kg/m³).
    /// Earth sea level default: 1.225
    pub air_density: f32,
    /// Distance after which projectile is considered expired (meters).
    /// Used to despawn projectiles that have traveled too far.
    pub expired_distance: f32,
}

impl Default for BallisticsConfig {
    fn default() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            air_density: 1.225,
            expired_distance: 5000.0,
        }
    }
}

impl BallisticsConfig {
    /// Create a config with no air drag (vacuum)
    pub fn vacuum() -> Self {
        Self {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            air_density: 0.0,
            expired_distance: 5000.0,
        }
    }

    /// Create a config with custom gravity and no air drag
    pub fn with_gravity(gravity: Vec3) -> Self {
        Self {
            gravity,
            air_density: 0.0,
            expired_distance: 5000.0,
        }
    }
}

/// The current state of a projectile in the simulation.
#[derive(Debug, Clone, Copy)]
pub enum ProjectileState {
    /// Projectile is still flying through the air
    Flying,
    /// Projectile has hit something
    Hit {
        /// Position where the hit occurred (meters)
        position: Vec3,
        /// Surface normal at the hit point (normalized)
        normal: Vec3,
    },
    /// Projectile has exceeded its lifetime or left the simulation bounds
    Expired,
}

impl Default for ProjectileState {
    fn default() -> Self {
        Self::Flying
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projectile_default() {
        let p = Projectile::default();
        assert!(p.active);
        assert_eq!(p.position, Vec3::ZERO);
        assert_eq!(p.mass, 1.0);
    }

    #[test]
    fn test_ballistics_config_default() {
        let config = BallisticsConfig::default();
        assert_eq!(config.gravity.y, -9.81);
        assert_eq!(config.air_density, 1.225);
    }

    #[test]
    fn test_projectile_state_default() {
        let state = ProjectileState::default();
        assert!(matches!(state, ProjectileState::Flying));
    }

    #[test]
    fn test_projectile_state_hit() {
        let state = ProjectileState::Hit {
            position: Vec3::new(10.0, 0.0, 5.0),
            normal: Vec3::Y,
        };
        if let ProjectileState::Hit { position, normal } = state {
            assert_eq!(position.x, 10.0);
            assert_eq!(normal, Vec3::Y);
        } else {
            panic!("Expected Hit state");
        }
    }

    #[test]
    fn test_projectile_spawn() {
        let p = Projectile::spawn(
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(1.0, 0.5, 0.0),
            50.0,
            5.0,
        );
        assert!(p.active);
        assert_eq!(p.position, Vec3::new(0.0, 10.0, 0.0));
        assert_eq!(p.mass, 5.0);
        // Velocity should be normalized direction * speed
        let expected_speed = p.velocity.length();
        assert!((expected_speed - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_projectile_integrate_falls() {
        // Projectile dropped from height should fall
        let config = BallisticsConfig::default();
        let mut p = Projectile::spawn(
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::ZERO, // No initial velocity
            0.0,
            5.0,
        );

        // Simulate for 1 second (100 steps of 0.01s)
        for _ in 0..100 {
            p.integrate(&config, 0.01);
        }

        // Should have fallen significantly
        assert!(p.position.y < 10.0);
        // With gravity of -9.81 m/s², after 1s from rest:
        // y = y0 - 0.5*g*t² = 10 - 0.5*9.81*1 ≈ 5.1m (without drag)
        // With drag it will be slightly higher
        assert!(p.position.y < 6.0);
    }

    #[test]
    fn test_projectile_hits_ground() {
        let config = BallisticsConfig::default();
        let mut p = Projectile::spawn(
            Vec3::new(0.0, 1.0, 0.0), // Start 1m above ground
            Vec3::ZERO,
            0.0,
            5.0,
        );

        // Simulate until it hits ground
        let mut hit_ground = false;
        for _ in 0..1000 {
            let state = p.integrate(&config, 0.01);
            if matches!(state, ProjectileState::Hit { .. }) {
                hit_ground = true;
                break;
            }
        }

        assert!(hit_ground);
        assert!(!p.active);
        assert!(p.position.y <= 0.0);
    }

    #[test]
    fn test_projectile_with_drag_vs_without() {
        // Projectile with drag should travel shorter horizontal distance
        let config_with_drag = BallisticsConfig {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            air_density: 1.225,
            expired_distance: 5000.0,
        };
        let config_no_drag = BallisticsConfig {
            gravity: Vec3::new(0.0, -9.81, 0.0),
            air_density: 0.0, // No air = no drag
            expired_distance: 5000.0,
        };

        let mut p_drag = Projectile::spawn(
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            50.0,
            5.0,
        );
        let mut p_no_drag = Projectile::spawn(
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            50.0,
            5.0,
        );

        // Simulate both until they hit ground
        for _ in 0..10000 {
            let state1 = p_drag.integrate(&config_with_drag, 0.001);
            let state2 = p_no_drag.integrate(&config_no_drag, 0.001);
            if !p_drag.active && !p_no_drag.active {
                break;
            }
            if matches!(state1, ProjectileState::Hit { .. }) && matches!(state2, ProjectileState::Hit { .. }) {
                break;
            }
        }

        // Projectile with drag should have traveled less horizontal distance
        assert!(p_drag.position.x < p_no_drag.position.x);
    }

    #[test]
    fn test_expired_distance() {
        // Test that projectiles expire after traveling expired_distance
        let mut config = BallisticsConfig::vacuum();
        config.expired_distance = 50.0; // Expire after 50m
        config.gravity = Vec3::ZERO; // No gravity for simpler test

        let mut p = Projectile::spawn(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            100.0, // 100 m/s
            1.0,
        );

        let dt = 1.0 / 60.0;
        let mut expired = false;

        // At 100 m/s, should expire after 0.5 seconds (30 frames)
        for i in 0..100 {
            let state = p.integrate(&config, dt);
            if matches!(state, ProjectileState::Expired) {
                expired = true;
                // Should expire around iteration 30 (0.5s * 60fps)
                assert!(
                    i >= 28 && i <= 32,
                    "Should expire around 30 iterations, expired at {}",
                    i
                );
                break;
            }
        }

        assert!(expired, "Projectile should have expired");
        assert!(!p.active, "Projectile should be inactive after expiring");
        assert!(
            p.distance_traveled >= config.expired_distance,
            "Distance traveled ({}) should be >= expired_distance ({})",
            p.distance_traveled,
            config.expired_distance
        );
    }

    #[test]
    fn test_distance_traveled_tracking() {
        // Test that distance_traveled is properly accumulated
        let mut config = BallisticsConfig::vacuum();
        config.gravity = Vec3::ZERO; // No gravity
        config.expired_distance = 10000.0; // High enough to not expire

        let mut p = Projectile::spawn(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            10.0, // 10 m/s
            1.0,
        );

        // Simulate for exactly 1 second (100 steps of 0.01s)
        for _ in 0..100 {
            p.integrate(&config, 0.01);
        }

        // Should have traveled approximately 10m (10 m/s * 1s)
        let expected_distance = 10.0;
        assert!(
            (p.distance_traveled - expected_distance).abs() < 0.1,
            "Expected ~{} m traveled, got {}",
            expected_distance,
            p.distance_traveled
        );
    }
}
