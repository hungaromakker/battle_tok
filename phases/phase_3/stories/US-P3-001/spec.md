# US-P3-001: Create Config Module with ArenaConfig

## Description
Extract all hardcoded arena layout constants from `battle_arena.rs` into a dedicated config module. This is the foundation for the entire refactoring — every other story depends on having a clean, centralized configuration system rather than magic numbers scattered across 3,880 lines.

## The Core Concept / Why This Matters
`battle_arena.rs` has dozens of hardcoded values (island positions, radii, lava Y coordinates, spawn intervals, etc.) embedded deep in initialization and update code. This makes it impossible to tweak gameplay parameters without editing rendering code, and duplicates values across multiple places. A config module creates a single source of truth for all arena parameters.

The game is a hex-based strategy with floating islands above a lava ocean. The config must capture: island layout (two hexagonal islands at specific positions), bridge connections, lava ocean dimensions, and gameplay timing (meteor spawns, physics checks, day length).

## Goal
Create `src/game/config/` module with `ArenaConfig` struct that replaces all hardcoded arena constants in `battle_arena.rs`.

## Files to Create/Modify
- Create `src/game/config/mod.rs` — Module root, re-exports ArenaConfig
- Create `src/game/config/arena_config.rs` — ArenaConfig + IslandConfig + BridgeConfig structs
- Modify `src/game/mod.rs` — Add `pub mod config;` and re-exports

## Implementation Steps
1. Read through `battle_arena.rs` and collect all hardcoded arena constants:
   - Island positions: `Vec3::new(-30.0, 10.0, 0.0)` and `Vec3::new(30.0, 10.0, 0.0)` (used in `initialize()` and `rebuild_terrain()`)
   - Island radius: `30.0` (used in `FloatingIslandConfig`)
   - Surface height: `5.0`, thickness: `25.0`
   - Lava ocean: size `200.0`, Y position `-15.0`
   - Meteor spawn center: `Vec3::new(0.0, 0.0, 0.0)`, radius: `60.0`
   - Spawn interval from `MeteorSpawner`
   - `PHYSICS_CHECK_INTERVAL` constant (already `5.0`)
   - Day length: `600.0` seconds

2. Create `src/game/config/arena_config.rs` with:
   ```rust
   use glam::Vec3;

   #[derive(Clone, Debug)]
   pub struct IslandConfig {
       pub position: Vec3,
       pub radius: f32,
       pub surface_height: f32,
       pub thickness: f32,
       pub taper_amount: f32,
   }

   #[derive(Clone, Debug)]
   pub struct BridgeConfig {
       pub width: f32,
       pub rail_height: f32,
   }

   #[derive(Clone, Debug)]
   pub struct ArenaConfig {
       pub island_attacker: IslandConfig,
       pub island_defender: IslandConfig,
       pub bridge: BridgeConfig,
       pub lava_size: f32,
       pub lava_y: f32,
       pub meteor_spawn_interval: f32,
       pub meteor_spawn_radius: f32,
       pub physics_check_interval: f32,
       pub day_length_seconds: f32,
   }

   impl Default for ArenaConfig { ... }
   ```

3. Create `src/game/config/mod.rs` that re-exports everything.

4. Add `pub mod config;` to `src/game/mod.rs` and add re-exports.

5. Run `cargo check` to verify compilation.

## Code Patterns
Follow the existing pattern from `src/game/terrain/params.rs` for config structs:
```rust
// From terrain/params.rs - this is how configs are structured in this codebase
#[derive(Clone)]
pub struct TerrainParams {
    pub amplitude: f32,
    pub frequency: f32,
    // ...
}

impl Default for TerrainParams {
    fn default() -> Self { /* sensible defaults */ }
}
```

## Acceptance Criteria
- [ ] `ArenaConfig::default()` returns values matching current hardcoded constants
- [ ] `IslandConfig` captures position, radius, surface_height, thickness, taper
- [ ] `BridgeConfig` captures width and rail_height
- [ ] All structs derive `Clone` and `Debug`
- [ ] `src/game/mod.rs` exports config types
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
After this story, you can write `ArenaConfig::default()` anywhere in the codebase and get the exact same values that are currently hardcoded in `battle_arena.rs`. The config module compiles and is accessible from the game module. No changes to `battle_arena.rs` yet — that comes in Story 11.

## Dependencies
- Depends on: None
