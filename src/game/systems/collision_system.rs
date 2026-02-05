//! Collision system — centralized collision detection for player and projectile interactions.
//!
//! Wraps the low-level collision primitives from [`crate::game::physics`] into
//! higher-level operations that check against *all* blocks or *all* hex prisms
//! in one call.  Pure game logic with **no** GPU dependencies.

use glam::Vec3;

use crate::game::arena_player::{PLAYER_EYE_HEIGHT, Player};
use crate::game::physics::collision::{
    AABB, check_capsule_aabb_collision, check_capsule_hex_collision,
};
use crate::physics::collision::HexPrismGrid;
use crate::render::building_blocks::BuildingBlockManager;

/// Player capsule horizontal radius (meters).
const PLAYER_RADIUS: f32 = 0.3;
/// Player capsule top offset above feet (meters).
const PLAYER_TOP_OFFSET: f32 = PLAYER_EYE_HEIGHT + 0.2;

/// Stateless collision system that delegates to the existing physics primitives.
pub struct CollisionSystem;

impl CollisionSystem {
    /// Check a player capsule against every building block managed by `blocks`.
    ///
    /// For each collision detected the player's position and velocity are
    /// adjusted in-place.  Returns `true` if **any** collision was resolved.
    pub fn check_player_blocks(
        player: &mut Player,
        blocks: &BuildingBlockManager,
        _delta: f32,
    ) -> bool {
        let mut any_collision = false;

        for block in blocks.blocks() {
            let aabb = block.aabb();
            let player_top = player.position.y + PLAYER_TOP_OFFSET;
            let player_vel = Vec3::new(
                player.velocity.x,
                player.vertical_velocity,
                player.velocity.z,
            );

            let result = check_capsule_aabb_collision(
                player.position,
                player_top,
                PLAYER_RADIUS,
                player_vel,
                &AABB::new(aabb.min, aabb.max),
            );

            if result.has_collision() {
                player.position += result.push;
                player.velocity += Vec3::new(
                    result.velocity_adjustment.x,
                    0.0,
                    result.velocity_adjustment.z,
                );
                player.vertical_velocity += result.velocity_adjustment.y;

                if let (true, Some(ground_y)) = (result.grounded, result.ground_y) {
                    player.position.y = ground_y;
                    player.vertical_velocity = 0.0;
                    player.is_grounded = true;
                }

                any_collision = true;
            }
        }

        any_collision
    }

    /// Check a player capsule against every hex prism in `hex_grid`.
    ///
    /// Returns `true` if **any** collision was resolved.
    pub fn check_player_hexes(player: &mut Player, hex_grid: &HexPrismGrid) -> bool {
        let mut any_collision = false;
        let player_top = player.position.y + PLAYER_TOP_OFFSET;
        let player_vel = Vec3::new(
            player.velocity.x,
            player.vertical_velocity,
            player.velocity.z,
        );

        for (&(_q, _r, _level), prism) in hex_grid.iter() {
            let hex_bottom = prism.center.y;
            let hex_top = prism.center.y + prism.height;
            // Inscribed-circle radius ≈ circumradius × cos(30°)
            let hex_collision_radius = prism.radius * 0.866;

            let result = check_capsule_hex_collision(
                player.position,
                player_top,
                PLAYER_RADIUS,
                player_vel,
                prism.center.x,
                prism.center.z,
                hex_bottom,
                hex_top,
                hex_collision_radius,
            );

            if result.has_collision() {
                player.position += result.push;
                player.velocity += Vec3::new(
                    result.velocity_adjustment.x,
                    0.0,
                    result.velocity_adjustment.z,
                );
                player.vertical_velocity += result.velocity_adjustment.y;

                if let (true, Some(ground_y)) = (result.grounded, result.ground_y) {
                    player.position.y = ground_y;
                    player.vertical_velocity = 0.0;
                    player.is_grounded = true;
                }

                any_collision = true;
            }
        }

        any_collision
    }

    /// Cast a ray from `prev_pos` to `projectile.position` against `hex_grid`.
    ///
    /// Returns the hit position and the axial coordinate `(q, r, level)` of the
    /// struck prism, or `None` if no collision occurred.
    pub fn check_projectile_walls(
        projectile: &crate::physics::ballistics::Projectile,
        prev_pos: Vec3,
        hex_grid: &HexPrismGrid,
    ) -> Option<(Vec3, (i32, i32, i32))> {
        let ray = projectile.position - prev_pos;
        let ray_length = ray.length();
        if ray_length < 1e-6 {
            return None;
        }
        let ray_dir = ray / ray_length;

        hex_grid
            .ray_cast(prev_pos, ray_dir, ray_length)
            .map(|hit| (hit.position, hit.prism_coord))
    }
}
