//! Cannon interaction and fire coordination system.
//!
//! Wraps [`ArenaCannon`] with a grab/move/fire interface. The cannon aims
//! where the camera looks (no arrow-key aiming). The player can grab the
//! cannon with G, walk to reposition it, and fire with Space/F.

use glam::Vec3;

use crate::game::arena_cannon::ArenaCannon;

/// Manages cannon state: grab, move, aim (camera-based), and fire.
pub struct CannonSystem {
    cannon: ArenaCannon,
    /// Cached direction for mesh-dirty detection.
    last_direction: Vec3,
    /// Cached position for mesh-dirty detection.
    last_position: Vec3,
    mesh_dirty: bool,
}

impl CannonSystem {
    /// Create a new cannon system with default cannon state.
    pub fn new() -> Self {
        let cannon = ArenaCannon::default();
        let dir = cannon.get_barrel_direction();
        let pos = cannon.position;
        Self {
            cannon,
            last_direction: dir,
            last_position: pos,
            mesh_dirty: true, // Dirty on first frame so mesh gets generated
        }
    }

    /// Update cannon aim from camera look direction.
    ///
    /// Call each frame with the camera's forward vector.
    pub fn aim_at_camera(&mut self, camera_forward: Vec3) {
        self.cannon.set_look_direction(camera_forward);

        // Check if direction changed enough to warrant mesh regeneration
        let new_dir = self.cannon.get_barrel_direction();
        if (new_dir - self.last_direction).length_squared() > 1e-6 {
            self.mesh_dirty = true;
            self.last_direction = new_dir;
        }
    }

    /// Update cannon position when grabbed — call each frame.
    pub fn update_grabbed(&mut self, player_pos: Vec3, camera_yaw: f32) {
        if self.cannon.grabbed {
            self.cannon.follow_player(player_pos, camera_yaw);

            // Check if position changed
            if (self.cannon.position - self.last_position).length_squared() > 1e-4 {
                self.mesh_dirty = true;
                self.last_position = self.cannon.position;
            }
        }
    }

    /// Try to grab the cannon (toggle grab/release).
    ///
    /// Returns `true` if the state changed.
    pub fn toggle_grab(&mut self, player_pos: Vec3) -> bool {
        if self.cannon.grabbed {
            self.cannon.release();
            self.mesh_dirty = true;
            true
        } else {
            let grabbed = self.cannon.try_grab(player_pos);
            if grabbed {
                self.mesh_dirty = true;
            }
            grabbed
        }
    }

    /// Whether the cannon is currently grabbed.
    pub fn is_grabbed(&self) -> bool {
        self.cannon.grabbed
    }

    /// Get fire parameters: (muzzle position, barrel direction, muzzle velocity).
    pub fn fire_params(&self) -> (Vec3, Vec3, f32) {
        (
            self.cannon.get_muzzle_position(),
            self.cannon.get_barrel_direction(),
            self.cannon.muzzle_velocity,
        )
    }

    /// Check if the player is close enough to fire.
    pub fn can_fire(&self, player_pos: Vec3) -> bool {
        self.cannon.grabbed || self.cannon.in_fire_range(player_pos)
    }

    /// Check if the cannon mesh needs regeneration.
    pub fn mesh_dirty(&self) -> bool {
        self.mesh_dirty
    }

    /// Mark the mesh as clean after regeneration.
    pub fn mark_mesh_clean(&mut self) {
        self.mesh_dirty = false;
    }

    /// Access the underlying cannon for rendering or other reads.
    pub fn cannon(&self) -> &ArenaCannon {
        &self.cannon
    }

    /// Mutable access to the underlying cannon (e.g. repositioning).
    pub fn cannon_mut(&mut self) -> &mut ArenaCannon {
        &mut self.cannon
    }
}
