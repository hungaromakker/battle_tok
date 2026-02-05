//! Arena Cannon Module
//!
//! Cannon state for aiming and firing in the Battle Arena.

use crate::physics::ballistics::Projectile;
use glam::Vec3;

use super::terrain::terrain_height_at;
use super::types::{Mesh, generate_box, generate_oriented_box};

/// Smoothing factor for cannon movement (higher = faster response)
pub const CANNON_SMOOTHING: f32 = 0.15;
/// Cannon rotation speed in radians per second
pub const CANNON_ROTATION_SPEED: f32 = 1.0; // ~57 degrees per second

/// Cannon state for aiming and firing (US-017: Cannon aiming controls)
pub struct ArenaCannon {
    pub position: Vec3,
    pub barrel_elevation: f32, // Current elevation in radians (-10 to 45 degrees)
    pub barrel_azimuth: f32,   // Current azimuth in radians (-45 to 45 degrees)
    pub target_elevation: f32, // Target elevation for smooth interpolation
    pub target_azimuth: f32,   // Target azimuth for smooth interpolation
    pub barrel_length: f32,
    pub muzzle_velocity: f32,
    pub projectile_mass: f32,
}

impl Default for ArenaCannon {
    fn default() -> Self {
        let default_elevation = 30.0_f32.to_radians(); // 30 degrees up
        // Position cannon on the attacker platform - sample terrain height at that location
        let cannon_x = 0.0;
        let cannon_z = 25.0;
        let cannon_y = terrain_height_at(cannon_x, cannon_z, 0.0) + 0.5; // Slightly above terrain
        Self {
            position: Vec3::new(cannon_x, cannon_y, cannon_z),
            barrel_elevation: default_elevation,
            barrel_azimuth: 0.0,
            target_elevation: default_elevation,
            target_azimuth: 0.0,
            barrel_length: 4.0,
            muzzle_velocity: 50.0, // m/s
            projectile_mass: 5.0,  // kg
        }
    }
}

impl ArenaCannon {
    /// Get the direction the barrel is pointing
    pub fn get_barrel_direction(&self) -> Vec3 {
        // Start with forward direction (-Z in our coordinate system)
        let base_dir = Vec3::new(0.0, 0.0, -1.0);

        // Apply elevation (rotation around X)
        let cos_elev = self.barrel_elevation.cos();
        let sin_elev = self.barrel_elevation.sin();
        let elevated = Vec3::new(
            base_dir.x,
            base_dir.y * cos_elev - base_dir.z * sin_elev,
            base_dir.y * sin_elev + base_dir.z * cos_elev,
        );

        // Apply azimuth (rotation around Y)
        let cos_az = self.barrel_azimuth.cos();
        let sin_az = self.barrel_azimuth.sin();
        Vec3::new(
            elevated.x * cos_az + elevated.z * sin_az,
            elevated.y,
            -elevated.x * sin_az + elevated.z * cos_az,
        )
        .normalize()
    }

    /// Get the position of the barrel tip (muzzle)
    pub fn get_muzzle_position(&self) -> Vec3 {
        self.position + self.get_barrel_direction() * self.barrel_length
    }

    /// Spawn a projectile from the cannon
    pub fn fire(&self) -> Projectile {
        let muzzle_pos = self.get_muzzle_position();
        let direction = self.get_barrel_direction();
        Projectile::spawn(
            muzzle_pos,
            direction,
            self.muzzle_velocity,
            self.projectile_mass,
        )
    }

    /// Adjust target elevation (smooth movement toward this target)
    pub fn adjust_elevation(&mut self, delta: f32) {
        self.target_elevation += delta;
        let min_elev = -10.0_f32.to_radians();
        let max_elev = 45.0_f32.to_radians();
        self.target_elevation = self.target_elevation.clamp(min_elev, max_elev);
    }

    /// Adjust target azimuth (smooth movement toward this target)
    pub fn adjust_azimuth(&mut self, delta: f32) {
        self.target_azimuth += delta;
        let max_az = 45.0_f32.to_radians();
        self.target_azimuth = self.target_azimuth.clamp(-max_az, max_az);
    }

    /// Update cannon for smooth movement interpolation (call each frame)
    pub fn update(&mut self, delta_time: f32) {
        // Exponential smoothing toward target angles
        let smoothing = 1.0 - (1.0 - CANNON_SMOOTHING).powf(delta_time * 60.0);
        self.barrel_elevation += (self.target_elevation - self.barrel_elevation) * smoothing;
        self.barrel_azimuth += (self.target_azimuth - self.barrel_azimuth) * smoothing;
    }

    /// Check if cannon is currently moving toward target
    pub fn is_aiming(&self) -> bool {
        let elev_diff = (self.target_elevation - self.barrel_elevation).abs();
        let az_diff = (self.target_azimuth - self.barrel_azimuth).abs();
        elev_diff > 0.001 || az_diff > 0.001
    }
}

/// Generate cannon mesh (simplified box + cylinder representation)
pub fn generate_cannon_mesh(cannon: &ArenaCannon) -> Mesh {
    let mut mesh = Mesh::new();
    let pos = cannon.position;
    let dir = cannon.get_barrel_direction();
    let color = [0.3, 0.3, 0.3, 1.0]; // Gray metal

    // Cannon body (box)
    let body_size = Vec3::new(1.0, 0.5, 1.5);
    let body_mesh = generate_box(pos, body_size, color);
    mesh.merge(&body_mesh);

    // Barrel (elongated box for simplicity)
    let barrel_center = pos + dir * (cannon.barrel_length / 2.0);
    let barrel_up = Vec3::Y;
    let _barrel_right = dir.cross(barrel_up).normalize();
    let barrel_size = Vec3::new(0.3, 0.3, cannon.barrel_length);

    // Generate rotated barrel
    let barrel_mesh = generate_oriented_box(
        barrel_center,
        barrel_size,
        dir,
        barrel_up,
        [0.2, 0.2, 0.2, 1.0],
    );
    mesh.merge(&barrel_mesh);

    mesh
}
