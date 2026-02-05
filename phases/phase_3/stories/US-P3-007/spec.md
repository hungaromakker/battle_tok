# US-P3-007: Create CannonSystem

## Description
Extract cannon aiming, rotation smoothing, and fire coordination from `battle_arena.rs` into a `CannonSystem`. The cannon is the player's main weapon — it rotates based on keyboard input and fires projectiles. Currently the aiming math is inline in the update loop.

## The Core Concept / Why This Matters
The cannon in the game has elevation and azimuth controlled by arrow keys, with smooth interpolation. The aiming code (~lines 1176-1192 in `battle_arena.rs`) reads `AimingKeys` and updates the `Cannon` struct. The fire logic creates a projectile from the cannon's barrel position. Extracting this into `CannonSystem` means the weapon mechanics are isolated and can be extended (e.g., multiple weapon types, cooldowns, upgrades) without modifying the main game loop.

## Goal
Create `src/game/systems/cannon_system.rs` that manages cannon state, aiming, and fire coordination.

## Files to Create/Modify
- Create `src/game/systems/cannon_system.rs` — CannonSystem struct
- Modify `src/game/systems/mod.rs` — Add module and re-exports

## Implementation Steps
1. Read cannon code in `battle_arena.rs`:
   - `Cannon` struct from `src/game/arena_cannon.rs` (aliased as `ArenaCannon`)
   - Aiming: reads `aiming.up/down/left/right`, applies rotation speed
   - Constants: `CANNON_ROTATION_SPEED`, `CANNON_SMOOTHING`
   - Fire: creates projectile at barrel tip, uses barrel direction for velocity
   - Mesh: `generate_cannon_mesh()` called when direction changes

2. Create the struct:
   ```rust
   use glam::Vec3;
   use crate::game::arena_cannon::{ArenaCannon, CANNON_ROTATION_SPEED, CANNON_SMOOTHING, generate_cannon_mesh};
   use crate::game::input::AimingState;

   pub struct CannonSystem {
       cannon: ArenaCannon,
       rotation_speed: f32,
       last_direction: Vec3,  // For mesh cache invalidation
   }

   impl CannonSystem {
       pub fn new() -> Self;

       /// Update cannon aim based on input
       pub fn aim(&mut self, aiming: &AimingState, delta: f32);

       /// Get fire parameters (position, direction, speed)
       pub fn fire_params(&self) -> (Vec3, Vec3, f32);

       /// Check if cannon mesh needs regeneration
       pub fn mesh_dirty(&self) -> bool;

       /// Mark mesh as clean after regeneration
       pub fn mark_mesh_clean(&mut self);

       /// Access cannon for rendering
       pub fn cannon(&self) -> &ArenaCannon;
   }
   ```

3. The `aim()` method should:
   - Read aiming state (up/down/left/right)
   - Apply rotation with `CANNON_ROTATION_SPEED * delta`
   - Clamp elevation angles
   - Track if direction changed (for mesh dirty flag)

4. Run `cargo check`.

## Code Patterns
From `src/game/arena_cannon.rs`:
```rust
pub struct ArenaCannon {
    pub position: Vec3,
    pub barrel_direction: Vec3,
    pub elevation: f32,
    pub azimuth: f32,
    // ...
}
```

## Acceptance Criteria
- [ ] `CannonSystem` wraps `ArenaCannon` with aim/fire interface
- [ ] `aim()` reads `AimingState` and updates angles
- [ ] `fire_params()` returns barrel tip position, direction, speed
- [ ] `mesh_dirty()` tracks when mesh needs regeneration
- [ ] No `wgpu` imports
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
Cannon aiming is a one-liner: `self.cannon_system.aim(&aiming_state, delta)`. Firing is: `let (pos, dir, speed) = self.cannon_system.fire_params()`. The aiming math is no longer inline in the update loop.

## Dependencies
- Depends on: None
