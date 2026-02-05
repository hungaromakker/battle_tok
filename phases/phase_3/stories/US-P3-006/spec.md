# US-P3-006: Create MeteorSystem

## Description
Extract meteor spawning, falling physics, and impact effects from `battle_arena.rs` into a `MeteorSystem`. Meteors are atmospheric fireballs that add visual drama to the battlefield — they spawn periodically above the arena, fall with gravity, and create debris impacts on the ground.

## The Core Concept / Why This Matters
The meteor system is a relatively self-contained feature: `MeteorSpawner` creates meteors on a timer, they fall with gravity, and on impact they spawn debris. Currently this lives in `battle_arena.rs` fields (`meteors: Vec<Meteor>`, `meteor_spawner: MeteorSpawner`) and the update loop. Extracting it makes the meteor feature toggleable and configurable without touching the main game loop.

## Goal
Create `src/game/systems/meteor_system.rs` that wraps `MeteorSpawner` + `Vec<Meteor>` and provides a simple update/iterate interface.

## Files to Create/Modify
- Create `src/game/systems/meteor_system.rs` — MeteorSystem struct
- Modify `src/game/systems/mod.rs` — Add module and re-exports

## Implementation Steps
1. Read the meteor code in `battle_arena.rs`:
   - `MeteorSpawner` initialized with center and radius in `new()`
   - In `update()`: `meteor_spawner.update(delta)` → spawns new meteors
   - Each meteor has position, velocity, trail
   - Impact: when meteor hits ground, spawn debris via `spawn_meteor_impact()`
   - Existing types in `src/game/destruction.rs`: `Meteor`, `MeteorSpawner`, `spawn_meteor_impact`

2. Create the struct:
   ```rust
   use glam::Vec3;
   use crate::game::destruction::{Meteor, MeteorSpawner, spawn_meteor_impact, DebrisParticle};

   pub struct MeteorImpact {
       pub position: Vec3,
       pub debris: Vec<DebrisParticle>,
   }

   pub struct MeteorSystem {
       meteors: Vec<Meteor>,
       spawner: MeteorSpawner,
   }

   impl MeteorSystem {
       pub fn new(center: Vec3, radius: f32, spawn_interval: f32) -> Self;

       /// Update spawner and all meteors
       /// Returns impacts for debris spawning
       pub fn update(&mut self, delta: f32, ground_y: f32) -> Vec<MeteorImpact>;

       /// Iterate meteors for rendering
       pub fn iter(&self) -> impl Iterator<Item = &Meteor>;

       /// Active meteor count
       pub fn count(&self) -> usize;
   }
   ```

3. The `update()` method should:
   - Call `spawner.update(delta)` to potentially spawn new meteors
   - Move new spawned meteors into `self.meteors`
   - Update each meteor position (apply gravity)
   - Check if meteor hit ground (position.y < ground_y)
   - Return `MeteorImpact` for each ground hit
   - Remove impacted meteors

4. Run `cargo check`.

## Code Patterns
Existing types in `src/game/destruction.rs`:
```rust
pub struct Meteor {
    pub position: Vec3,
    pub velocity: Vec3,
    pub size: f32,
    pub trail: Vec<Vec3>,
}

pub struct MeteorSpawner {
    pub center: Vec3,
    pub radius: f32,
    pub spawn_timer: f32,
    pub spawn_interval: f32,
}
```

## Acceptance Criteria
- [ ] `MeteorSystem` owns both `Vec<Meteor>` and `MeteorSpawner`
- [ ] `update()` handles spawning, physics, and impact detection
- [ ] `MeteorImpact` returned for caller to handle debris
- [ ] No `wgpu` imports
- [ ] Re-exported from systems module
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
Meteor management is a one-liner in the game loop: `let impacts = self.meteors.update(delta, ground_y)`. The caller handles debris from impacts. Clean separation between meteor physics and rendering.

## Dependencies
- Depends on: None
