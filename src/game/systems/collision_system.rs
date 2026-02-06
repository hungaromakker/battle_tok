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
/// Landing assist window below a block top (meters).
const LANDING_WINDOW_BELOW_TOP: f32 = 0.35;
/// Landing assist window above a block top (meters).
const LANDING_WINDOW_ABOVE_TOP: f32 = 0.55;
/// Maximum vertical step height the player can auto-climb (meters).
const MAX_STEP_HEIGHT: f32 = 1.05;

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
                // Step assist for stairs / low ledges: when side-colliding with a low top,
                // lift player up instead of hard stopping.
                let mostly_horizontal_hit = (result.push.x.abs() + result.push.z.abs()) > 1e-4
                    && result.push.y.abs() < 0.05;
                if mostly_horizontal_hit && player.vertical_velocity <= 0.2 {
                    let step_delta = aabb.max.y - player.position.y;
                    if step_delta > 0.05 && step_delta <= MAX_STEP_HEIGHT {
                        let stepped_x = player.position.x + result.push.x;
                        let stepped_z = player.position.z + result.push.z;
                        let on_top_xz = stepped_x >= aabb.min.x - PLAYER_RADIUS
                            && stepped_x <= aabb.max.x + PLAYER_RADIUS
                            && stepped_z >= aabb.min.z - PLAYER_RADIUS
                            && stepped_z <= aabb.max.z + PLAYER_RADIUS;
                        if on_top_xz {
                            player.position.x = stepped_x;
                            player.position.z = stepped_z;
                            player.position.y = aabb.max.y;
                            player.vertical_velocity = 0.0;
                            player.is_grounded = true;
                            any_collision = true;
                            continue;
                        }
                    }
                }

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

            // Landing assist: makes non-cube/top landings reliable with AABB tops.
            let on_top_xz = player.position.x >= aabb.min.x - PLAYER_RADIUS
                && player.position.x <= aabb.max.x + PLAYER_RADIUS
                && player.position.z >= aabb.min.z - PLAYER_RADIUS
                && player.position.z <= aabb.max.z + PLAYER_RADIUS;
            let near_top = player.position.y >= aabb.max.y - LANDING_WINDOW_BELOW_TOP
                && player.position.y <= aabb.max.y + LANDING_WINDOW_ABOVE_TOP;
            if on_top_xz && near_top && player.vertical_velocity <= 0.0 {
                player.position.y = aabb.max.y;
                player.vertical_velocity = 0.0;
                player.is_grounded = true;
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

    /// Cast a projectile segment against building blocks and return nearest hit.
    pub fn check_projectile_blocks(
        prev_pos: Vec3,
        new_pos: Vec3,
        radius: f32,
        blocks: &BuildingBlockManager,
    ) -> Option<(Vec3, u32)> {
        let ray = new_pos - prev_pos;
        let ray_length = ray.length();
        if ray_length < 1e-6 {
            return None;
        }
        let ray_dir = ray / ray_length;

        let mut nearest_t = f32::MAX;
        let mut nearest: Option<(Vec3, u32)> = None;

        for block in blocks.blocks() {
            let aabb = block.aabb();
            // Expand by projectile radius to emulate swept-sphere collision.
            let expanded = AABB::new(
                aabb.min - Vec3::splat(radius),
                aabb.max + Vec3::splat(radius),
            );

            if let Some(t_hit) = ray_aabb_hit_t(prev_pos, ray_dir, ray_length, &expanded)
                && t_hit < nearest_t
            {
                nearest_t = t_hit;
                nearest = Some((prev_pos + ray_dir * t_hit, block.id));
            }
        }

        nearest
    }
}

fn ray_aabb_hit_t(origin: Vec3, dir: Vec3, max_t: f32, aabb: &AABB) -> Option<f32> {
    let inv_dir = Vec3::new(
        if dir.x.abs() > 1e-7 {
            1.0 / dir.x
        } else {
            f32::INFINITY
        },
        if dir.y.abs() > 1e-7 {
            1.0 / dir.y
        } else {
            f32::INFINITY
        },
        if dir.z.abs() > 1e-7 {
            1.0 / dir.z
        } else {
            f32::INFINITY
        },
    );

    let mut t1 = (aabb.min.x - origin.x) * inv_dir.x;
    let mut t2 = (aabb.max.x - origin.x) * inv_dir.x;
    let mut tmin = t1.min(t2);
    let mut tmax = t1.max(t2);

    t1 = (aabb.min.y - origin.y) * inv_dir.y;
    t2 = (aabb.max.y - origin.y) * inv_dir.y;
    tmin = tmin.max(t1.min(t2));
    tmax = tmax.min(t1.max(t2));

    t1 = (aabb.min.z - origin.z) * inv_dir.z;
    t2 = (aabb.max.z - origin.z) * inv_dir.z;
    tmin = tmin.max(t1.min(t2));
    tmax = tmax.min(t1.max(t2));

    if tmax < 0.0 || tmin > tmax {
        return None;
    }

    let t_hit = if tmin >= 0.0 { tmin } else { tmax };
    if t_hit <= max_t { Some(t_hit) } else { None }
}
