# US-P3-005: Create DestructionSystem

## Description
Extract the prism destruction, cascade collapse, falling prism physics, and debris particle management from `battle_arena.rs` into a `DestructionSystem`. This is the most complex extraction because destruction involves multiple interacting subsystems: structural support checking, cascade failures, falling physics, and debris spawning.

## The Core Concept / Why This Matters
When a projectile hits a hex wall prism, the game needs to: (1) remove the prism, (2) check if neighboring prisms lost support, (3) cascade-remove unsupported prisms, (4) create falling animations for removed prisms, (5) spawn debris particles on impact. This is currently 4 methods spread across `battle_arena.rs` (~lines 1261-1470). Consolidating into `DestructionSystem` means all destruction state (falling prisms, debris, destroyed count) lives in one place.

## Goal
Create `src/game/systems/destruction_system.rs` that encapsulates the full destruction lifecycle: destroy → cascade → fall → debris.

## Files to Create/Modify
- Create `src/game/systems/destruction_system.rs` — DestructionSystem struct
- Modify `src/game/systems/mod.rs` — Add module and re-exports

## Implementation Steps
1. Read the destruction code in `battle_arena.rs`:
   - `destroy_prism_with_physics()` (~line 1261) — removes prism, creates FallingPrism
   - `check_support_cascade()` (~line 1280) — finds unsupported neighbors, chain reaction
   - `update_falling_prisms()` (~line 1380) — gravity, rotation, debris on ground hit
   - `update_debris_particles()` (~line 1440) — lifetime tick, remove expired

2. Note the existing helpers in `src/game/physics/support.rs`:
   - `has_support()` — checks if a prism has structural support
   - `find_unsupported_cascade()` — BFS to find all prisms that lost support
   - `check_falling_prism_collision()` — ground collision for falling prisms

3. Create the struct:
   ```rust
   use glam::Vec3;
   use battle_tok_engine::render::HexPrismGrid;
   use crate::game::destruction::{FallingPrism, DebrisParticle, spawn_debris, get_material_color};
   use crate::game::physics::{has_support, find_unsupported_cascade, check_falling_prism_collision};

   pub struct DestructionSystem {
       falling_prisms: Vec<FallingPrism>,
       debris: Vec<DebrisParticle>,
       total_destroyed: u32,
   }

   impl DestructionSystem {
       pub fn new() -> Self;

       /// Destroy a prism and trigger cascade check
       pub fn destroy_prism(
           &mut self,
           coord: (i32, i32, i32),
           hex_grid: &mut HexPrismGrid,
       );

       /// Update falling prisms and debris particles
       pub fn update(&mut self, delta: f32);

       /// Access falling prisms for rendering
       pub fn falling_prisms(&self) -> &[FallingPrism];

       /// Access debris for rendering
       pub fn debris(&self) -> &[DebrisParticle];

       /// Total prisms destroyed this session
       pub fn total_destroyed(&self) -> u32;
   }
   ```

4. The `destroy_prism()` method should:
   - Remove prism from hex_grid
   - Create FallingPrism with initial velocity
   - Call `find_unsupported_cascade()` to find chain reaction
   - Create FallingPrism for each cascaded prism
   - Increment destroyed counter

5. The `update()` method should:
   - Apply gravity to falling prisms
   - Check ground collision, spawn debris on impact
   - Tick debris lifetimes, remove expired

6. Run `cargo check`.

## Code Patterns
The existing destruction types in `src/game/destruction.rs`:
```rust
pub struct FallingPrism {
    pub center: Vec3,
    pub color: [f32; 3],
    pub velocity: Vec3,
    pub rotation: f32,
    pub size: f32,
}

pub struct DebrisParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub color: [f32; 3],
    pub lifetime: f32,
    pub max_lifetime: f32,
}
```

## Acceptance Criteria
- [ ] `DestructionSystem` owns falling_prisms and debris vectors
- [ ] `destroy_prism()` handles cascade logic correctly
- [ ] `update()` handles gravity, collision, and debris lifecycle
- [ ] Uses existing helpers from `physics/support.rs`
- [ ] No `wgpu` imports
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
The destruction lifecycle is fully encapsulated. You call `system.destroy_prism(coord, &mut grid)` and it handles the entire chain reaction. You call `system.update(delta)` and all falling/debris physics update. The ~210 lines of destruction code in `battle_arena.rs` can be replaced with these two calls.

## Dependencies
- Depends on: None
