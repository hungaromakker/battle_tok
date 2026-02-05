//! BattleScene — high-level composition of all game systems.
//!
//! Owns the player, terrain, hex grid, trees, and every extracted system
//! (collision, projectile, destruction, meteor, cannon, building, economy).
//! Its [`update`](BattleScene::update) method is the single entry point for
//! the entire per-frame game logic. **No wgpu imports** — this module is
//! GPU-agnostic.

use glam::Vec3;

use crate::game::arena_player::{ArenaGround, BridgeDef, IslandDef, MovementKeys, Player};
use crate::game::config::{ArenaConfig, VisualConfig};
use crate::game::destruction::get_material_color;
use crate::game::input::{AimingState, MovementState};
use crate::game::state::GameState;
use crate::game::systems::{
    BuildingSystem, CannonSystem, CollisionSystem, DestructionSystem, MeteorSystem,
    ProjectileSystem,
};
use crate::game::trees::{PlacedTree, generate_trees_on_terrain};
use crate::game::types::{Mesh, Vertex, generate_box, generate_sphere};
use crate::physics::ballistics::BallisticsConfig;
use crate::render::hex_prism::{DEFAULT_HEX_HEIGHT, HexPrismGrid};

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
    /// 2. Cannon aiming
    /// 3. Projectile physics
    /// 4. Projectile-wall collision → destruction
    /// 5. Destruction physics (falling prisms, debris)
    /// 6. Meteor spawning & impacts
    /// 7. Player-block collision
    /// 8. Player-hex collision (via render grid iteration)
    /// 9. Building structural physics
    /// 10. Economy / day-cycle tick
    pub fn update(&mut self, delta: f32, movement: &MovementState, aiming: &AimingState) {
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
        self.player.update(&keys, self.camera_yaw, delta, &self.arena_ground);

        // 2. Cannon aiming
        self.cannon.aim(aiming, delta);

        // 3. Update projectiles (physics integration)
        let updates = self.projectiles.update(delta);

        // 4. Projectile-wall collisions → destruction
        //    Uses the render grid's ray_cast directly (same AABB logic
        //    as CollisionSystem::check_projectile_walls).
        let mut hits: Vec<(usize, (i32, i32, i32))> = Vec::new();
        for upd in &updates {
            let ray = upd.new_pos - upd.prev_pos;
            let ray_length = ray.length();
            if ray_length < 1e-6 {
                continue;
            }
            let ray_dir = ray / ray_length;
            if let Some(hit) = self.hex_grid.ray_cast(upd.prev_pos, ray_dir, ray_length) {
                hits.push((upd.index, hit.prism_coord));
            }
        }
        // Remove hit projectiles (reverse to keep indices valid)
        hits.sort_by(|a, b| b.0.cmp(&a.0));
        for (idx, coord) in &hits {
            self.projectiles.remove(*idx);
            self.destruction.destroy_prism(*coord, &mut self.hex_grid);
            self.terrain_needs_rebuild = true;
        }

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
        //    Iterates the render grid directly since CollisionSystem::check_player_hexes
        //    expects the physics grid type. Uses the same capsule-hex logic.
        self.check_player_hex_collision();

        // 9. Building structural physics
        let _removed = self.building.update_physics(delta);

        // 10. Economy / day cycle
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

    /// Fire the cannon, spawning a projectile from the barrel.
    ///
    /// Returns `true` if the projectile was added (i.e. under the active limit).
    pub fn fire_cannon(&mut self) -> bool {
        let (muzzle_pos, direction, speed) = self.cannon.fire_params();
        self.projectiles.fire(muzzle_pos, direction, speed)
    }

    /// Clear all active projectiles.
    pub fn clear_projectiles(&mut self) {
        self.projectiles.clear();
    }

    /// Generate a combined mesh for all dynamic objects (projectiles,
    /// falling prisms, debris, meteors).
    ///
    /// This is the data the renderer needs each frame to update the
    /// dynamic-object vertex buffer.
    pub fn generate_dynamic_mesh(&self) -> Vec<Vertex> {
        let mut mesh = Mesh::new();

        // Projectile spheres
        let projectile_color = [1.0, 0.8, 0.2, 1.0];
        for proj in self.projectiles.iter() {
            let sphere = generate_sphere(proj.position, proj.radius, projectile_color, 8);
            mesh.merge(&sphere);
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

    /// Check player capsule against hex prisms in the render grid.
    ///
    /// Uses the same capsule-hex collision primitive as `CollisionSystem`
    /// but iterates the render grid directly.
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
