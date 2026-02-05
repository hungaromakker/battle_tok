//! Cannon aiming and fire coordination system.
//!
//! Wraps [`ArenaCannon`] with a high-level aim/fire interface and tracks
//! when the cannon mesh needs regeneration due to direction changes.

use glam::Vec3;

use crate::game::arena_cannon::{ArenaCannon, CANNON_ROTATION_SPEED};
use crate::game::input::AimingState;

/// Manages cannon state, aiming, and fire coordination.
///
/// Encapsulates the aiming math (elevation/azimuth from keyboard input),
/// smooth interpolation, and mesh-dirty tracking so the main game loop
/// only needs one-liner calls.
pub struct CannonSystem {
    cannon: ArenaCannon,
    rotation_speed: f32,
    /// Cached direction for mesh-dirty detection.
    last_direction: Vec3,
    mesh_dirty: bool,
}

impl CannonSystem {
    /// Create a new cannon system with default cannon state.
    pub fn new() -> Self {
        let cannon = ArenaCannon::default();
        let dir = cannon.get_barrel_direction();
        Self {
            cannon,
            rotation_speed: CANNON_ROTATION_SPEED,
            last_direction: dir,
            mesh_dirty: true, // Dirty on first frame so mesh gets generated
        }
    }

    /// Update cannon aim based on input.
    ///
    /// Reads the aiming state (up/down/left/right), applies rotation scaled
    /// by `delta`, and runs smooth interpolation toward the target angles.
    pub fn aim(&mut self, aiming: &AimingState, delta: f32) {
        let aim_delta = self.rotation_speed * delta;

        if aiming.aim_up {
            self.cannon.adjust_elevation(aim_delta);
        }
        if aiming.aim_down {
            self.cannon.adjust_elevation(-aim_delta);
        }
        if aiming.aim_left {
            self.cannon.adjust_azimuth(-aim_delta);
        }
        if aiming.aim_right {
            self.cannon.adjust_azimuth(aim_delta);
        }

        // Smooth interpolation toward target angles
        self.cannon.update(delta);

        // Check if direction changed enough to warrant mesh regeneration
        let new_dir = self.cannon.get_barrel_direction();
        if (new_dir - self.last_direction).length_squared() > 1e-6 {
            self.mesh_dirty = true;
            self.last_direction = new_dir;
        }
    }

    /// Get fire parameters: (muzzle position, barrel direction, muzzle velocity).
    pub fn fire_params(&self) -> (Vec3, Vec3, f32) {
        (
            self.cannon.get_muzzle_position(),
            self.cannon.get_barrel_direction(),
            self.cannon.muzzle_velocity,
        )
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

    /// Check if the cannon is currently interpolating toward its target.
    pub fn is_aiming(&self) -> bool {
        self.cannon.is_aiming()
    }
}
