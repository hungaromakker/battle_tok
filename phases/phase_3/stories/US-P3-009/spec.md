# US-P3-009: Create BattleScene

## Description
Create the high-level `BattleScene` that composes all extracted systems (collision, projectile, destruction, meteor, cannon, building) with player state, terrain, and config into a cohesive scene. This is the integration point — it replaces the scattered game state fields in `BattleArenaApp` with a single unified scene object.

## The Core Concept / Why This Matters
Stories 1-8 created individual systems and configs. This story wires them together into `BattleScene` — the single struct that represents the entire game state (minus GPU resources). The scene owns the player, terrain, hex grid, trees, and all game systems. Its `update()` method is the main game loop logic: move player → check collisions → update projectiles → process destruction → update meteors → tick building physics → update economy. This is the key architectural piece that enables `battle_arena.rs` to become a thin wrapper.

## Goal
Create `src/game/scenes/battle_scene.rs` that composes all systems into a unified scene with a single `update()` entry point.

## Files to Create/Modify
- Create `src/game/scenes/mod.rs` — Scenes module root
- Create `src/game/scenes/battle_scene.rs` — BattleScene struct
- Modify `src/game/mod.rs` — Add `pub mod scenes;` and re-exports

## Implementation Steps
1. Create the struct that holds all game state:
   ```rust
   use glam::Vec3;
   use battle_tok_engine::render::HexPrismGrid;
   use crate::game::config::{ArenaConfig, VisualConfig};
   use crate::game::systems::{
       CollisionSystem, ProjectileSystem, DestructionSystem,
       MeteorSystem, CannonSystem, BuildingSystem,
   };
   use crate::game::arena_player::Player;
   use crate::game::trees::PlacedTree;
   use crate::game::state::GameState;
   use crate::game::input::{MovementState, AimingState};

   pub struct BattleScene {
       // Config
       pub config: ArenaConfig,
       pub visuals: VisualConfig,

       // Player
       pub player: Player,
       pub first_person_mode: bool,

       // Terrain
       pub hex_grid: HexPrismGrid,
       pub trees_attacker: Vec<PlacedTree>,
       pub trees_defender: Vec<PlacedTree>,

       // Systems
       pub collision: CollisionSystem,
       pub projectiles: ProjectileSystem,
       pub destruction: DestructionSystem,
       pub meteors: MeteorSystem,
       pub cannon: CannonSystem,
       pub building: BuildingSystem,

       // Economy + population
       pub game_state: GameState,

       // Flags
       pub terrain_needs_rebuild: bool,
   }
   ```

2. Implement construction:
   ```rust
   impl BattleScene {
       pub fn new(config: ArenaConfig, visuals: VisualConfig) -> Self {
           // Create hex grid from config
           // Generate trees for both islands
           // Initialize all systems with config values
           // Create player at starting position
       }
   }
   ```

3. Implement the main update loop:
   ```rust
   pub fn update(&mut self, delta: f32, movement: &MovementState, aiming: &AimingState) {
       // 1. Update player movement
       // 2. Cannon aiming
       // 3. Update projectiles
       // 4. Check projectile-wall collisions → trigger destruction
       // 5. Update destruction (falling prisms, debris)
       // 6. Update meteors → spawn impact debris
       // 7. Player-block collision
       // 8. Player-hex collision
       // 9. Building physics tick
       // 10. Game state update (economy, day cycle)
   }
   ```

4. Add mesh generation helpers (for rendering):
   ```rust
   pub fn generate_dynamic_mesh(&self) -> Vec<Vertex> {
       // Combine projectile spheres + falling prisms + debris + meteors
   }
   ```

5. Create `src/game/scenes/mod.rs` with re-exports.

6. Add `pub mod scenes;` to `src/game/mod.rs`.

7. Run `cargo check`.

## Code Patterns
Follow the existing `GameState` pattern from `src/game/state.rs`:
```rust
pub struct GameState {
    pub resources: Resources,
    pub day_cycle: DayCycle,
    pub population: Population,
    // ...
}
impl GameState {
    pub fn new() -> Self { ... }
    pub fn update(&mut self, delta: f32) { ... }
}
```
BattleScene follows the same pattern but at a higher level.

## Acceptance Criteria
- [ ] `BattleScene::new()` creates complete scene from configs
- [ ] `BattleScene::update()` processes all game logic in correct order
- [ ] All systems accessible via public fields for rendering
- [ ] `generate_dynamic_mesh()` returns combined mesh data
- [ ] No `wgpu` imports — scene is GPU-agnostic
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
`BattleScene` is the single struct that holds the entire game world. The main `battle_arena.rs` can call `scene.update(delta, &movement, &aiming)` once per frame and all game logic executes. It can then read `scene.projectiles.iter()`, `scene.destruction.debris()`, etc. to generate meshes for rendering.

## Dependencies
- Depends on: US-P3-001, US-P3-002, US-P3-003, US-P3-004, US-P3-005, US-P3-006, US-P3-007, US-P3-008
