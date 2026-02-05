# US-P3-004: Create ProjectileSystem

## Description
Extract projectile spawning, physics updates, and state management from `battle_arena.rs` into a self-contained `ProjectileSystem`. Currently projectile code is split between `fire_projectile()`, the main update loop, and mesh generation — consolidating it into one system makes the ballistics feature modular and testable.

## The Core Concept / Why This Matters
The game features a cannon that fires ballistic projectiles. The code for this is currently scattered across `battle_arena.rs`: firing at `fire_projectile()` (~line 1470), physics updates in the update loop (~lines 1194-1236), and mesh generation for rendering. The `ProjectileSystem` will own the `Vec<Projectile>` and `BallisticsConfig`, encapsulating the entire lifecycle. This is important because projectile management is purely mathematical (ballistic arcs, gravity) and should have zero GPU coupling.

## Goal
Create `src/game/systems/projectile_system.rs` that owns projectile state and provides fire/update/clear/iterate operations.

## Files to Create/Modify
- Create `src/game/systems/projectile_system.rs` — ProjectileSystem struct
- Modify `src/game/systems/mod.rs` — Add module and re-exports

## Implementation Steps
1. Read the projectile code in `battle_arena.rs`:
   - `fire_projectile()` method creates a `Projectile` and pushes to `self.projectiles`
   - Update loop iterates projectiles, applies gravity, checks state (Active/Hit/OutOfBounds)
   - Uses `BallisticsConfig` for physics parameters
   - Calls `collision_system` logic for wall hits

2. Create the struct:
   ```rust
   use glam::Vec3;
   use battle_tok_engine::physics::ballistics::{BallisticsConfig, Projectile, ProjectileState};

   pub struct ProjectileSystem {
       projectiles: Vec<Projectile>,
       config: BallisticsConfig,
   }

   impl ProjectileSystem {
       pub fn new(config: BallisticsConfig) -> Self;

       /// Spawn a new projectile
       pub fn fire(&mut self, position: Vec3, direction: Vec3, speed: f32);

       /// Update all projectiles, remove expired ones
       /// Returns list of active projectile states for collision checking
       pub fn update(&mut self, delta: f32) -> Vec<ProjectileUpdate>;

       /// Clear all projectiles
       pub fn clear(&mut self);

       /// Number of active projectiles
       pub fn active_count(&self) -> usize;

       /// Iterate for rendering
       pub fn iter(&self) -> impl Iterator<Item = &Projectile>;

       /// Remove projectile by index (after collision)
       pub fn remove(&mut self, index: usize);
   }

   pub struct ProjectileUpdate {
       pub index: usize,
       pub prev_pos: Vec3,
       pub new_pos: Vec3,
       pub state: ProjectileState,
   }
   ```

3. The `update()` method should:
   - Save previous positions before updating
   - Call `projectile.update(delta)` for each
   - Return update data for collision checking by the caller
   - Remove `OutOfBounds` projectiles automatically

4. Update systems mod.rs.

5. Run `cargo check`.

## Code Patterns
Follow the existing `BallisticsConfig` and `Projectile` from `engine/src/physics/ballistics.rs`:
```rust
// From engine - these are the types we wrap
pub struct Projectile {
    pub position: Vec3,
    pub velocity: Vec3,
    pub state: ProjectileState,
    // ...
}
impl Projectile {
    pub fn update(&mut self, delta: f32) { /* applies gravity */ }
}
```

## Acceptance Criteria
- [ ] `ProjectileSystem` owns `Vec<Projectile>` and `BallisticsConfig`
- [ ] `fire()` creates projectile with correct initial state
- [ ] `update()` applies physics and returns position changes
- [ ] No `wgpu` imports — pure game logic
- [ ] Re-exported from systems module
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
You can create a `ProjectileSystem`, fire projectiles, update them, and iterate over them for rendering — all without any GPU code. The ~40-line projectile update block in `battle_arena.rs` can later be replaced with a single `self.projectiles.update(delta)` call.

## Dependencies
- Depends on: None
