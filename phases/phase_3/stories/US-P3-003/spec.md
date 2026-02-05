# US-P3-003: Create CollisionSystem

## Description
Extract player-block collision, player-hex collision, and projectile-wall collision logic from `battle_arena.rs` into a standalone `CollisionSystem`. This is pure game logic with no GPU dependencies, making it testable and reusable.

## The Core Concept / Why This Matters
Collision detection is currently inlined in the `update()` method of `battle_arena.rs`. The player-block collision (~lines 2025-2100) checks the player capsule against building blocks. The player-hex collision (~lines 2100-2160) checks against hex prism walls. Projectile-wall collision is embedded in the projectile update loop (~lines 1194-1236). Extracting these into a standalone system means: (1) collision can be unit tested without GPU, (2) collision logic is centralized rather than scattered, (3) new collision types can be added without modifying the monolith.

## Goal
Create `src/game/systems/collision_system.rs` with stateless collision functions that take references to game objects and return collision results.

## Files to Create/Modify
- Create `src/game/systems/mod.rs` — Systems module root
- Create `src/game/systems/collision_system.rs` — CollisionSystem with static methods
- Modify `src/game/mod.rs` — Add `pub mod systems;` and re-exports

## Implementation Steps
1. Read the collision code in `battle_arena.rs`:
   - Player-block collision uses `check_capsule_aabb_collision()` from `src/game/physics/collision.rs`
   - Player-hex collision uses `check_capsule_hex_collision()` from the same module
   - Projectile-wall collision checks projectile ray against hex prism grid

2. Create `src/game/systems/collision_system.rs`:
   ```rust
   use glam::Vec3;
   use crate::game::physics::{CollisionResult, check_capsule_aabb_collision, check_capsule_hex_collision};
   use battle_tok_engine::render::{HexPrismGrid, BuildingBlockManager};
   use crate::game::arena_player::Player;
   use battle_tok_engine::physics::ballistics::Projectile;

   pub struct CollisionSystem;

   impl CollisionSystem {
       /// Check player capsule against all building blocks
       /// Returns true if any collision was resolved
       pub fn check_player_blocks(
           player: &mut Player,
           blocks: &BuildingBlockManager,
           delta: f32,
       ) -> bool { ... }

       /// Check player capsule against hex prism walls
       pub fn check_player_hexes(
           player: &mut Player,
           hex_grid: &HexPrismGrid,
       ) -> bool { ... }

       /// Check projectile against hex prism walls
       /// Returns hit position and prism coordinate if hit
       pub fn check_projectile_walls(
           projectile: &Projectile,
           prev_pos: Vec3,
           hex_grid: &HexPrismGrid,
       ) -> Option<(Vec3, (i32, i32, i32))> { ... }
   }
   ```

3. The implementations should call the existing `check_capsule_aabb_collision()` and `check_capsule_hex_collision()` from `src/game/physics/collision.rs` — don't rewrite collision math, just wrap the existing functions into a system interface.

4. Create `src/game/systems/mod.rs` with `pub mod collision_system;` and re-exports.

5. Add `pub mod systems;` to `src/game/mod.rs`.

6. Run `cargo check`.

## Code Patterns
The existing physics module has the collision primitives:
```rust
// From src/game/physics/collision.rs
pub fn check_capsule_aabb_collision(
    capsule_pos: Vec3, capsule_radius: f32, capsule_height: f32,
    aabb: &AABB,
) -> Option<CollisionResult> { ... }
```

The system wraps these into higher-level operations (check ALL blocks, check ALL hex walls).

## Acceptance Criteria
- [ ] `CollisionSystem::check_player_blocks()` compiles and accepts correct types
- [ ] `CollisionSystem::check_player_hexes()` compiles and accepts correct types
- [ ] `CollisionSystem::check_projectile_walls()` compiles and returns `Option<(Vec3, (i32, i32, i32))>`
- [ ] No `wgpu` imports — pure game logic only
- [ ] Re-exported from `src/game/systems/mod.rs`
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
The three collision functions compile and have the correct signatures. They delegate to the existing physics module functions. When Story 11 wires them into `battle_arena.rs`, the inline collision code (~135 lines) can be replaced with 3 function calls.

## Dependencies
- Depends on: None
