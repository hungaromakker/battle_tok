//! Arena Cannon Module
//!
//! Cannon state for aiming and firing in the Battle Arena.
//! Supports camera-based aiming and grab-to-move interaction.

use crate::physics::ballistics::Projectile;
use glam::Vec3;

use super::terrain::terrain_height_at;
use super::types::{Mesh, generate_box, generate_oriented_box};

/// Distance in front of the player the cannon is placed when grabbed
pub const CANNON_GRAB_OFFSET: f32 = 3.0;
/// How close the player must be to grab the cannon
pub const CANNON_GRAB_RANGE: f32 = 6.0;
/// Height offset above terrain when placed
pub const CANNON_TERRAIN_OFFSET: f32 = 0.5;

/// Cannon state for aiming and firing.
///
/// The cannon aims based on an externally-provided look direction (from the
/// camera). It can be grabbed by the player and repositioned by walking.
pub struct ArenaCannon {
    pub position: Vec3,
    /// Current look direction (set from camera each frame)
    pub look_direction: Vec3,
    /// Whether the cannon is currently grabbed/carried by the player
    pub grabbed: bool,
    pub barrel_length: f32,
    pub muzzle_velocity: f32,
    pub projectile_mass: f32,
}

impl Default for ArenaCannon {
    fn default() -> Self {
        // Position cannon on the attacker platform
        let cannon_x = 0.0;
        let cannon_z = 25.0;
        let cannon_y = terrain_height_at(cannon_x, cannon_z, 0.0) + CANNON_TERRAIN_OFFSET;
        Self {
            position: Vec3::new(cannon_x, cannon_y, cannon_z),
            look_direction: Vec3::new(0.0, 0.0, -1.0),
            grabbed: false,
            barrel_length: 4.0,
            muzzle_velocity: 50.0, // m/s
            projectile_mass: 5.0,  // kg
        }
    }
}

impl ArenaCannon {
    /// Get the direction the barrel is pointing (camera look direction,
    /// clamped so the barrel doesn't aim below -10 degrees).
    pub fn get_barrel_direction(&self) -> Vec3 {
        let dir = self.look_direction.normalize_or_zero();
        if dir == Vec3::ZERO {
            return Vec3::new(0.0, 0.0, -1.0);
        }
        // Clamp vertical aim so cannon doesn't shoot straight down
        let min_pitch = -10.0_f32.to_radians();
        let horizontal = (dir.x * dir.x + dir.z * dir.z).sqrt();
        let current_pitch = dir.y.atan2(horizontal);
        if current_pitch < min_pitch {
            let cos_p = min_pitch.cos();
            let sin_p = min_pitch.sin();
            let h_norm = Vec3::new(dir.x, 0.0, dir.z).normalize_or_zero();
            Vec3::new(h_norm.x * cos_p, sin_p, h_norm.z * cos_p).normalize()
        } else {
            dir
        }
    }

    /// Get the position of the barrel tip (muzzle)
    pub fn get_muzzle_position(&self) -> Vec3 {
        self.position + Vec3::Y * 0.5 + self.get_barrel_direction() * self.barrel_length
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

    /// Set look direction from camera forward vector
    pub fn set_look_direction(&mut self, direction: Vec3) {
        self.look_direction = direction;
    }

    /// Update cannon position when grabbed: follow the player
    pub fn follow_player(&mut self, player_pos: Vec3, camera_yaw: f32) {
        // Place cannon a bit in front of the player (on the ground)
        let forward = Vec3::new(camera_yaw.sin(), 0.0, -camera_yaw.cos());
        let target = player_pos + forward * CANNON_GRAB_OFFSET;
        let ground_y = terrain_height_at(target.x, target.z, 0.0) + CANNON_TERRAIN_OFFSET;
        // Keep cannon on the ground but allow player-height variation
        self.position = Vec3::new(target.x, ground_y.max(player_pos.y - 1.0), target.z);
    }

    /// Try to grab the cannon (checks distance)
    pub fn try_grab(&mut self, player_pos: Vec3) -> bool {
        let dist = (self.position - player_pos).length();
        if dist <= CANNON_GRAB_RANGE {
            self.grabbed = true;
            true
        } else {
            false
        }
    }

    /// Release the cannon at current position
    pub fn release(&mut self) {
        self.grabbed = false;
    }

    /// Check if player is close enough to fire the cannon
    pub fn in_fire_range(&self, player_pos: Vec3) -> bool {
        let dist = (self.position - player_pos).length();
        dist <= CANNON_GRAB_RANGE
    }
}

/// Generate cannon mesh (simplified box + cylinder representation)
pub fn generate_cannon_mesh(cannon: &ArenaCannon) -> Mesh {
    let mut mesh = Mesh::new();
    let pos = cannon.position;
    let dir = cannon.get_barrel_direction();

    // Base/body color — brighter when grabbed
    let body_color = if cannon.grabbed {
        [0.5, 0.4, 0.2, 1.0] // Golden highlight when grabbed
    } else {
        [0.3, 0.3, 0.3, 1.0] // Gray metal
    };

    // Cannon body (box)
    let body_size = Vec3::new(1.0, 0.5, 1.5);
    let body_mesh = generate_box(pos, body_size, body_color);
    mesh.merge(&body_mesh);

    // Wheels (small boxes on sides)
    let wheel_color = [0.2, 0.15, 0.1, 1.0];
    let wheel_size = Vec3::new(0.15, 0.4, 0.4);
    let wheel_l = generate_box(pos + Vec3::new(-0.7, -0.1, 0.0), wheel_size, wheel_color);
    let wheel_r = generate_box(pos + Vec3::new(0.7, -0.1, 0.0), wheel_size, wheel_color);
    mesh.merge(&wheel_l);
    mesh.merge(&wheel_r);

    // Barrel (elongated box)
    let barrel_center = pos + Vec3::Y * 0.5 + dir * (cannon.barrel_length / 2.0);
    let barrel_up = Vec3::Y;
    let barrel_size = Vec3::new(0.3, 0.3, cannon.barrel_length);

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
