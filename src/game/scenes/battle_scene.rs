//! BattleScene — high-level composition of all game systems.
//!
//! Owns the player, terrain, hex grid, trees, and every extracted system
//! (collision, projectile, destruction, meteor, cannon, building, economy).
//! Its [`update`](BattleScene::update) method is the single entry point for
//! the entire per-frame game logic. **No wgpu imports** — this module is
//! GPU-agnostic.

use std::collections::HashSet;

use glam::Vec3;

use crate::game::arena_player::{ArenaGround, BridgeDef, IslandDef, MovementKeys, Player};
use crate::game::config::{ArenaConfig, VisualConfig};
use crate::game::destruction::{get_material_color, spawn_debris, spawn_meteor_impact};
use crate::game::input::MovementState;
use crate::game::state::GameState;
use crate::game::systems::building_system::DestroyedBlock;
use crate::game::systems::{
    BuildingSystem, CannonSystem, CollisionSystem, DestructionSystem, MeteorSystem, ProjectileKind,
    ProjectileSystem,
};
use crate::game::trees::{PlacedTree, generate_trees_on_terrain};
use crate::game::types::{Mesh, Vertex, generate_box, generate_oriented_box, generate_sphere};
use crate::physics::ballistics::{BallisticsConfig, ProjectileState};
use crate::render::hex_prism::{DEFAULT_HEX_HEIGHT, DEFAULT_HEX_RADIUS, HexPrismGrid};

/// Combat weapon mode selected by the player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponMode {
    Cannonball,
    RocketLauncher,
}

/// Single-frame explosion event emitted for rendering feedback.
#[derive(Debug, Clone, Copy)]
pub struct ExplosionEvent {
    pub position: Vec3,
    pub ember_count: usize,
}

#[derive(Debug, Clone)]
struct IntegrityRecheckJob {
    time_left: f32,
    block_ids: Vec<u32>,
}

/// Complete game scene composing all systems, terrain, and player state.
///
/// Created once from [`ArenaConfig`] + [`VisualConfig`]. Call
/// [`update`](BattleScene::update) each frame with the delta time and
/// current input state; all game logic executes in the correct order.
/// Read system fields directly for rendering data.
pub struct BattleScene {
    // -- Config --
    pub config: ArenaConfig,
    pub visuals: VisualConfig,

    // -- Player --
    pub player: Player,
    pub first_person_mode: bool,
    pub camera_yaw: f32,

    // -- Terrain --
    pub hex_grid: HexPrismGrid,
    pub trees_attacker: Vec<PlacedTree>,
    pub trees_defender: Vec<PlacedTree>,

    // -- Systems --
    pub collision: CollisionSystem,
    pub projectiles: ProjectileSystem,
    pub destruction: DestructionSystem,
    pub meteors: MeteorSystem,
    pub cannon: CannonSystem,
    pub building: BuildingSystem,

    // -- Economy + population --
    pub game_state: GameState,

    // -- Combat state --
    pub weapon_mode: WeaponMode,
    explosion_events: Vec<ExplosionEvent>,
    integrity_recheck_jobs: Vec<IntegrityRecheckJob>,

    // -- Ground context for player collision --
    pub arena_ground: ArenaGround,

    // -- Bridge endpoints (stored for ground collision) --
    pub bridge_start: Vec3,
    pub bridge_end: Vec3,

    // -- Flags --
    pub terrain_needs_rebuild: bool,
}

impl BattleScene {
    /// Create a complete scene from configuration.
    ///
    /// Initialises the hex grid, generates trees on both islands, and wires
    /// up every game system with config-derived parameters.
    pub fn new(config: ArenaConfig, visuals: VisualConfig) -> Self {
        // Generate trees for both islands
        let trees_attacker = generate_trees_on_terrain(
            config.island_attacker.position,
            config.island_attacker.radius,
            0.3,
            0.0,
        );
        let trees_defender = generate_trees_on_terrain(
            config.island_defender.position,
            config.island_defender.radius,
            0.3,
            100.0,
        );

        // Player starts on the attacker island (at ground level + small offset)
        let start_pos = config.island_attacker.position
            + Vec3::new(0.0, config.island_attacker.surface_height + 1.0, 0.0);
        let mut player = Player::default();
        player.position = start_pos;

        // Meteor system centred on the arena midpoint
        let arena_center =
            (config.island_attacker.position + config.island_defender.position) * 0.5;
        let meteors = MeteorSystem::new(arena_center, config.meteor_spawn_radius);

        // Build arena ground context for player collision
        let arena_ground = ArenaGround {
            islands: vec![
                IslandDef {
                    center: config.island_attacker.position,
                    radius: config.island_attacker.radius,
                    surface_y: config.island_attacker.surface_height,
                },
                IslandDef {
                    center: config.island_defender.position,
                    radius: config.island_defender.radius,
                    surface_y: config.island_defender.surface_height,
                },
            ],
            bridge: None, // Set after bridge mesh is generated in battle_arena.rs
            kill_y: config.lava_y - 2.0, // Die slightly below lava surface
            respawn_pos: start_pos,
        };

        Self {
            // Config
            config: config.clone(),
            visuals,

            // Player
            player,
            first_person_mode: true,
            camera_yaw: 0.0,

            // Terrain
            hex_grid: HexPrismGrid::new(),
            trees_attacker,
            trees_defender,

            // Systems
            collision: CollisionSystem,
            projectiles: ProjectileSystem::new(BallisticsConfig::default()),
            destruction: DestructionSystem::new(),
            meteors,
            cannon: CannonSystem::new(),
            building: BuildingSystem::new(config.physics_check_interval),

            // Economy
            game_state: GameState::new(),

            // Combat
            weapon_mode: WeaponMode::Cannonball,
            explosion_events: Vec::new(),
            integrity_recheck_jobs: Vec::new(),

            // Ground context
            arena_ground,
            bridge_start: Vec3::ZERO,
            bridge_end: Vec3::ZERO,

            // Flags
            terrain_needs_rebuild: true,
        }
    }

    /// Main per-frame update — executes all game logic in the correct order.
    ///
    /// # Order of operations
    /// 1. Player movement
    /// 2. Cannon follow (if grabbed) + aim from camera
    /// 3. Projectile physics
    /// 4. Projectile collisions / explosions → destruction
    /// 5. Destruction physics (falling prisms, debris)
    /// 6. Meteor spawning & impacts
    /// 7. Player-block collision
    /// 8. Player-hex collision (via render grid iteration)
    /// 9. Economy / day-cycle tick
    pub fn update(&mut self, delta: f32, movement: &MovementState, camera_forward: Vec3) {
        self.explosion_events.clear();

        // 1. Player movement (island-aware ground collision)
        let keys = MovementKeys {
            forward: movement.forward,
            backward: movement.backward,
            left: movement.left,
            right: movement.right,
            up: movement.up,
            down: movement.down,
            sprint: movement.sprint,
        };
        self.player
            .update(&keys, self.camera_yaw, delta, &self.arena_ground);

        // 2. Cannon: aim where camera looks + follow player if grabbed
        self.cannon.aim_at_camera(camera_forward);
        self.cannon
            .update_grabbed(self.player.position, self.camera_yaw);

        // 3. Update projectiles (physics integration)
        let updates = self.projectiles.update(delta);

        // 4. Projectile collisions/explosions → destruction
        let mut remove_indices: Vec<usize> = Vec::new();
        for upd in &updates {
            match upd.state {
                ProjectileState::Flying => {
                    let ray = upd.new_pos - upd.prev_pos;
                    let ray_length = ray.length();
                    if ray_length < 1e-6 {
                        continue;
                    }
                    let ray_dir = ray / ray_length;
                    let wall_hit = self.hex_grid.ray_cast(upd.prev_pos, ray_dir, ray_length);
                    let block_hit = CollisionSystem::check_projectile_blocks(
                        upd.prev_pos,
                        upd.new_pos,
                        Self::projectile_hit_radius(upd.kind),
                        self.building.blocks(),
                    );

                    let wall_dist = wall_hit
                        .as_ref()
                        .map(|hit| hit.position.distance(upd.prev_pos))
                        .unwrap_or(f32::MAX);
                    let block_dist = block_hit
                        .as_ref()
                        .map(|(p, _)| p.distance(upd.prev_pos))
                        .unwrap_or(f32::MAX);

                    if let Some((hit_pos, block_id)) = block_hit
                        && block_dist <= wall_dist
                    {
                        remove_indices.push(upd.index);
                        match upd.kind {
                            ProjectileKind::Cannonball => {
                                self.handle_cannonball_block_impact(hit_pos, block_id);
                            }
                            ProjectileKind::Rocket => {
                                self.trigger_rocket_explosion(hit_pos, None, Some(block_id));
                            }
                        }
                        continue;
                    }

                    if let Some(hit) = wall_hit {
                        remove_indices.push(upd.index);
                        match upd.kind {
                            ProjectileKind::Cannonball => {
                                self.destruction
                                    .destroy_prism(hit.prism_coord, &mut self.hex_grid);
                                self.terrain_needs_rebuild = true;
                                let impacted = self.apply_explosion_damage_to_blocks(
                                    hit.position,
                                    1.4,
                                    42.0,
                                    8.0,
                                );
                                self.schedule_integrity_recheck(impacted, 5.0);
                                self.explosion_events.push(ExplosionEvent {
                                    position: hit.position,
                                    ember_count: 18,
                                });
                            }
                            ProjectileKind::Rocket => {
                                self.trigger_rocket_explosion(
                                    hit.position,
                                    Some(hit.prism_coord),
                                    None,
                                );
                            }
                        }
                    }
                }
                ProjectileState::Hit { position, .. } => {
                    remove_indices.push(upd.index);
                    if upd.kind == ProjectileKind::Rocket {
                        self.trigger_rocket_explosion(position, None, None);
                    } else {
                        let impacted =
                            self.apply_explosion_damage_to_blocks(position, 1.2, 28.0, 4.0);
                        self.schedule_integrity_recheck(impacted, 5.0);
                        self.explosion_events.push(ExplosionEvent {
                            position,
                            ember_count: 10,
                        });
                    }
                }
                ProjectileState::Expired => {
                    remove_indices.push(upd.index);
                }
            }
        }

        // Remove projectiles after all collision checks.
        remove_indices.sort_unstable();
        remove_indices.dedup();
        for idx in remove_indices.into_iter().rev() {
            self.projectiles.remove(idx);
        }

        // Delayed structural re-checks (explosion-triggered only)
        self.process_integrity_rechecks(delta);

        // 5. Destruction physics (falling prisms + debris)
        self.destruction.update(delta, &mut self.hex_grid);

        // 6. Meteors — spawn and process impacts
        let impacts = self.meteors.update(delta);
        for impact in impacts {
            self.destruction.add_debris(impact.debris);
        }

        // 7. Player-block collision
        CollisionSystem::check_player_blocks(&mut self.player, self.building.blocks(), delta);

        // 8. Player-hex collision
        self.check_player_hex_collision();

        // 9. Economy / day cycle
        self.game_state.update(delta);
    }

    /// Set the bridge endpoints for ground collision after mesh generation.
    pub fn set_bridge(&mut self, start: Vec3, end: Vec3) {
        use crate::game::terrain::BridgeConfig as TerrainBridgeConfig;
        self.bridge_start = start;
        self.bridge_end = end;
        self.arena_ground.bridge = Some(BridgeDef {
            start,
            end,
            config: TerrainBridgeConfig::default(),
        });
    }

    /// Toggle cannon grab state. Returns true if state changed.
    pub fn toggle_cannon_grab(&mut self) -> bool {
        self.cannon.toggle_grab(self.player.position)
    }

    /// Fire the cannon, spawning a projectile from the barrel.
    ///
    /// Returns `true` if the projectile was added.
    pub fn fire_cannon(&mut self) -> bool {
        let (muzzle_pos, direction, speed) = self.cannon.fire_params();
        match self.weapon_mode {
            WeaponMode::Cannonball => self.projectiles.fire_with_kind(
                muzzle_pos,
                direction,
                speed,
                ProjectileKind::Cannonball,
            ),
            WeaponMode::RocketLauncher => self.projectiles.fire_with_kind(
                muzzle_pos,
                direction,
                speed * 0.85,
                ProjectileKind::Rocket,
            ),
        }
    }

    /// Toggle cannonball/rocket mode and return the new mode.
    pub fn toggle_weapon_mode(&mut self) -> WeaponMode {
        self.weapon_mode = match self.weapon_mode {
            WeaponMode::Cannonball => WeaponMode::RocketLauncher,
            WeaponMode::RocketLauncher => WeaponMode::Cannonball,
        };
        self.weapon_mode
    }

    /// Current selected weapon mode.
    pub fn weapon_mode(&self) -> WeaponMode {
        self.weapon_mode
    }

    /// Drain one-frame explosion events for renderer-side VFX spawning.
    pub fn drain_explosion_events(&mut self) -> Vec<ExplosionEvent> {
        std::mem::take(&mut self.explosion_events)
    }

    /// Clear all active projectiles.
    pub fn clear_projectiles(&mut self) {
        self.projectiles.clear();
    }

    /// Generate a combined mesh for all dynamic objects (projectiles,
    /// falling prisms, debris, meteors).
    pub fn generate_dynamic_mesh(&self) -> Vec<Vertex> {
        let mut mesh = Mesh::new();

        // Projectile spheres
        for (proj, kind) in self.projectiles.iter_with_kind() {
            match kind {
                ProjectileKind::Cannonball => {
                    let fireball = Self::generate_cannonball_fire_mesh(proj.position, proj.radius);
                    mesh.merge(&fireball);
                }
                ProjectileKind::Rocket => {
                    let rocket =
                        Self::generate_rocket_projectile_mesh(proj.position, proj.velocity);
                    mesh.merge(&rocket);
                }
            }
        }

        // Falling prisms (rendered as small boxes)
        for prism in self.destruction.falling_prisms() {
            let color = get_material_color(prism.material);
            let half = Vec3::splat(DEFAULT_HEX_HEIGHT * 0.5);
            let bx = generate_box(prism.position, half, color);
            mesh.merge(&bx);
        }

        // Debris particles (tiny cubes)
        for debris in self.destruction.debris() {
            let half = Vec3::splat(debris.size * 0.5);
            let bx = generate_box(debris.position, half, debris.color);
            mesh.merge(&bx);
        }

        // Meteors (glowing spheres)
        let meteor_color = [1.0, 0.4, 0.1, 1.0];
        for meteor in self.meteors.iter() {
            let sphere = generate_sphere(meteor.position, meteor.size, meteor_color, 6);
            mesh.merge(&sphere);
        }

        mesh.vertices
    }

    fn generate_cannonball_fire_mesh(position: Vec3, radius: f32) -> Mesh {
        let mut mesh = Mesh::new();

        let outer = generate_sphere(position, radius * 1.35, [2.4, 0.85, 0.22, 1.0], 10);
        mesh.merge(&outer);

        let mid = generate_sphere(position, radius * 1.05, [3.2, 1.15, 0.28, 1.0], 10);
        mesh.merge(&mid);

        let core = generate_sphere(position, radius * 0.65, [4.0, 1.7, 0.55, 1.0], 8);
        mesh.merge(&core);

        mesh
    }

    fn generate_rocket_projectile_mesh(position: Vec3, velocity: Vec3) -> Mesh {
        let mut mesh = Mesh::new();

        let forward = velocity.normalize_or_zero();
        let forward = if forward.length_squared() > 1e-6 {
            forward
        } else {
            Vec3::new(0.0, 0.0, -1.0)
        };
        let ref_up = if forward.dot(Vec3::Y).abs() > 0.92 {
            Vec3::X
        } else {
            Vec3::Y
        };
        let right = forward.cross(ref_up).normalize_or_zero();
        let up = right.cross(forward).normalize_or_zero();
        let up = if up.length_squared() > 1e-6 {
            up
        } else {
            Vec3::Y
        };

        let body_len = 0.90;
        let body_size = Vec3::new(0.18, 0.18, body_len);
        let body = generate_oriented_box(position, body_size, forward, up, [0.92, 0.92, 0.95, 1.0]);
        mesh.merge(&body);

        let nose_pos = position + forward * (body_len * 0.56);
        let nose = generate_sphere(nose_pos, 0.12, [0.88, 0.2, 0.2, 1.0], 8);
        mesh.merge(&nose);

        let bell_pos = position - forward * (body_len * 0.56);
        let bell = generate_oriented_box(
            bell_pos,
            Vec3::new(0.13, 0.13, 0.12),
            forward,
            up,
            [0.24, 0.24, 0.28, 1.0],
        );
        mesh.merge(&bell);

        let window_pos = position + forward * 0.12 + up * 0.03;
        let window = generate_sphere(window_pos, 0.07, [0.2, 0.55, 0.9, 1.0], 6);
        mesh.merge(&window);

        let fin_base = position - forward * 0.35;
        let fin_offsets = [right, -right, up, -up];
        for axis in fin_offsets {
            let fin_center = fin_base + axis * 0.12;
            let fin_forward = (axis * 0.25 - forward * 0.15).normalize_or_zero();
            let fin_forward = if fin_forward.length_squared() > 1e-6 {
                fin_forward
            } else {
                forward
            };
            let fin = generate_oriented_box(
                fin_center,
                Vec3::new(0.035, 0.18, 0.25),
                fin_forward,
                up,
                [0.88, 0.2, 0.2, 1.0],
            );
            mesh.merge(&fin);
        }

        mesh
    }

    fn projectile_hit_radius(kind: ProjectileKind) -> f32 {
        match kind {
            ProjectileKind::Cannonball => 0.36,
            ProjectileKind::Rocket => 0.24,
        }
    }

    fn handle_cannonball_block_impact(&mut self, impact_position: Vec3, block_id: u32) {
        let to_block = self
            .building
            .block_manager
            .get_block(block_id)
            .map(|b| b.position - impact_position)
            .unwrap_or(Vec3::ZERO)
            .normalize_or_zero();
        let impulse_dir = if to_block.length_squared() > 1e-6 {
            to_block
        } else {
            Vec3::Y
        };

        let outcome =
            self.building
                .apply_block_damage(block_id, 68.0, impulse_dir * 10.0 + Vec3::Y * 0.9);
        if outcome.crack_stage_advanced {
            self.explosion_events.push(ExplosionEvent {
                position: impact_position,
                ember_count: 8 + outcome.crack_stage as usize * 2,
            });
        }
        if let Some(destroyed) = outcome.destroyed {
            self.handle_destroyed_blocks(&[destroyed]);
        }

        let impacted = self.apply_explosion_damage_to_blocks(impact_position, 1.9, 32.0, 4.5);
        self.schedule_integrity_recheck(impacted, 5.0);
    }

    fn apply_explosion_damage_to_blocks(
        &mut self,
        impact_position: Vec3,
        radius: f32,
        base_damage: f32,
        base_impulse: f32,
    ) -> Vec<u32> {
        let candidates: Vec<(u32, Vec3)> = self
            .building
            .blocks()
            .blocks()
            .iter()
            .map(|b| (b.id, b.position))
            .collect();

        let mut impacted = Vec::new();
        let mut destroyed = Vec::new();

        for (block_id, block_pos) in candidates {
            let dist = block_pos.distance(impact_position);
            if dist > radius {
                continue;
            }

            impacted.push(block_id);
            let falloff = (1.0 - dist / radius).clamp(0.0, 1.0);
            let damage = (base_damage * falloff * falloff).max(0.8);

            let dir = (block_pos - impact_position).normalize_or_zero();
            let dir = if dir.length_squared() > 1e-6 {
                dir
            } else {
                Vec3::Y
            };
            let impulse =
                dir * (base_impulse * falloff) + Vec3::Y * (base_impulse * 0.08 * falloff);

            let outcome = self.building.apply_block_damage(block_id, damage, impulse);
            if outcome.crack_stage_advanced && outcome.destroyed.is_none() {
                self.explosion_events.push(ExplosionEvent {
                    position: block_pos,
                    ember_count: 6 + outcome.crack_stage as usize * 3,
                });
            }
            if let Some(block) = outcome.destroyed {
                destroyed.push(block);
            }
        }

        if !destroyed.is_empty() {
            self.handle_destroyed_blocks(&destroyed);
        }

        impacted.sort_unstable();
        impacted.dedup();
        impacted
    }

    fn schedule_integrity_recheck(&mut self, mut block_ids: Vec<u32>, delay_seconds: f32) {
        if block_ids.is_empty() {
            return;
        }
        block_ids.sort_unstable();
        block_ids.dedup();
        self.integrity_recheck_jobs.push(IntegrityRecheckJob {
            time_left: delay_seconds.max(0.1),
            block_ids,
        });
    }

    fn process_integrity_rechecks(&mut self, delta: f32) {
        if self.integrity_recheck_jobs.is_empty() {
            return;
        }

        let mut due_jobs: Vec<IntegrityRecheckJob> = Vec::new();
        let mut idx = 0usize;
        while idx < self.integrity_recheck_jobs.len() {
            self.integrity_recheck_jobs[idx].time_left -= delta;
            if self.integrity_recheck_jobs[idx].time_left <= 0.0 {
                due_jobs.push(self.integrity_recheck_jobs.swap_remove(idx));
            } else {
                idx += 1;
            }
        }

        for job in due_jobs {
            let destroyed = self.building.recheck_integrity_for_blocks(&job.block_ids);
            if destroyed.is_empty() {
                continue;
            }

            self.handle_destroyed_blocks(&destroyed);
            let followup = self.collect_neighbor_blocks(&destroyed, 1.8);
            self.schedule_integrity_recheck(followup, 5.0);
        }
    }

    fn collect_neighbor_blocks(&self, destroyed: &[DestroyedBlock], radius: f32) -> Vec<u32> {
        if destroyed.is_empty() {
            return Vec::new();
        }
        let centers: Vec<Vec3> = destroyed.iter().map(|b| b.position).collect();
        self.building
            .blocks()
            .blocks()
            .iter()
            .filter(|block| {
                centers
                    .iter()
                    .any(|c| block.position.distance(*c) <= radius)
            })
            .map(|block| block.id)
            .collect()
    }

    fn handle_destroyed_blocks(&mut self, destroyed: &[DestroyedBlock]) {
        for block in destroyed {
            self.destruction
                .add_debris(spawn_debris(block.position, block.material, 20));
            self.explosion_events.push(ExplosionEvent {
                position: block.position,
                ember_count: 22,
            });
        }
    }

    fn trigger_rocket_explosion(
        &mut self,
        impact_position: Vec3,
        direct_hit: Option<(i32, i32, i32)>,
        direct_block: Option<u32>,
    ) {
        const ROCKET_BLAST_RADIUS: f32 = DEFAULT_HEX_RADIUS * 7.0;
        const ROCKET_DEBRIS_COUNT: usize = 28;
        const PLAYER_BLAST_RADIUS: f32 = 5.0;
        const PLAYER_BLAST_HORIZONTAL_FORCE: f32 = 14.0;
        const PLAYER_BLAST_UPWARD_FORCE: f32 = 8.0;

        let mut targets = HashSet::new();
        if let Some(coord) = direct_hit {
            targets.insert(coord);
        }

        for (&coord, prism) in self.hex_grid.iter() {
            if prism.center.distance(impact_position) <= ROCKET_BLAST_RADIUS {
                targets.insert(coord);
            }
        }

        let mut destroyed = 0usize;
        for coord in targets {
            if self.hex_grid.contains(coord.0, coord.1, coord.2) {
                self.destruction.destroy_prism(coord, &mut self.hex_grid);
                destroyed += 1;
            }
        }

        if let Some(block_id) = direct_block {
            let to_block = self
                .building
                .block_manager
                .get_block(block_id)
                .map(|b| b.position - impact_position)
                .unwrap_or(Vec3::ZERO)
                .normalize_or_zero();
            let impulse_dir = if to_block.length_squared() > 1e-6 {
                to_block
            } else {
                Vec3::Y
            };
            let direct = self.building.apply_block_damage(
                block_id,
                130.0,
                impulse_dir * 14.0 + Vec3::Y * 1.3,
            );
            if let Some(block) = direct.destroyed {
                self.handle_destroyed_blocks(&[block]);
            }
        }

        let impacted = self.apply_explosion_damage_to_blocks(impact_position, 4.8, 130.0, 16.0);
        self.schedule_integrity_recheck(impacted, 5.0);

        self.destruction
            .add_debris(spawn_debris(impact_position, 2, ROCKET_DEBRIS_COUNT));
        self.destruction
            .add_debris(spawn_meteor_impact(impact_position, 16));
        self.push_player_from_explosion(
            impact_position,
            PLAYER_BLAST_RADIUS,
            PLAYER_BLAST_HORIZONTAL_FORCE,
            PLAYER_BLAST_UPWARD_FORCE,
        );

        let ember_count = (42 + destroyed * 6).min(140);
        self.explosion_events.push(ExplosionEvent {
            position: impact_position,
            ember_count,
        });

        if destroyed > 0 {
            self.terrain_needs_rebuild = true;
        }
    }

    fn push_player_from_explosion(
        &mut self,
        impact_position: Vec3,
        radius: f32,
        horizontal_force: f32,
        upward_force: f32,
    ) {
        let to_player = self.player.position - impact_position;
        let distance = to_player.length();
        if distance > radius {
            return;
        }

        let falloff = (1.0 - distance / radius).clamp(0.0, 1.0);
        let horizontal_dir = Vec3::new(to_player.x, 0.0, to_player.z).normalize_or_zero();
        let horizontal_dir = if horizontal_dir.length_squared() > 1e-6 {
            horizontal_dir
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        };

        self.player.velocity += horizontal_dir * (horizontal_force * falloff);
        self.player.vertical_velocity = self.player.vertical_velocity.max(upward_force * falloff);
        self.player.is_grounded = false;
    }

    /// Check player capsule against hex prisms in the render grid.
    fn check_player_hex_collision(&mut self) {
        use crate::game::arena_player::PLAYER_EYE_HEIGHT;
        use crate::game::physics::collision::check_capsule_hex_collision;

        const PLAYER_RADIUS: f32 = 0.3;
        let player_top = self.player.position.y + PLAYER_EYE_HEIGHT + 0.2;
        let player_vel = Vec3::new(
            self.player.velocity.x,
            self.player.vertical_velocity,
            self.player.velocity.z,
        );

        for (_, prism) in self.hex_grid.iter() {
            let hex_bottom = prism.center.y;
            let hex_top = prism.center.y + prism.height;
            let hex_collision_radius = prism.radius * 0.866;

            let result = check_capsule_hex_collision(
                self.player.position,
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
                self.player.position += result.push;
                self.player.velocity += Vec3::new(
                    result.velocity_adjustment.x,
                    0.0,
                    result.velocity_adjustment.z,
                );
                self.player.vertical_velocity += result.velocity_adjustment.y;

                if let (true, Some(ground_y)) = (result.grounded, result.ground_y) {
                    self.player.position.y = ground_y;
                    self.player.vertical_velocity = 0.0;
                    self.player.is_grounded = true;
                }
            }
        }
    }
}
