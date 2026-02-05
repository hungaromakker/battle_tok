# US-P3-002: Create VisualConfig

## Description
Extract all visual settings (fog, lighting, sky, torch parameters) from hardcoded values in `battle_arena.rs` into a `VisualConfig` struct. This separates visual tuning from game logic, making it easy to create visual presets or adjust the apocalyptic atmosphere without touching game code.

## The Core Concept / Why This Matters
The game has a specific apocalyptic visual style: purple-orange sky, lava underglow, heavy fog, warm sun color, and flickering torches. Currently these values are scattered across `battle_arena.rs` in the uniform setup code (~lines 1100-1200) and the sky initialization. Centralizing them means an artist or designer can tweak the entire visual feel by changing one struct, and we can later add visual presets (e.g., "calm day", "heavy storm", "night raid").

## Goal
Create `src/game/config/visual_config.rs` with a `VisualConfig` struct that captures all visual atmosphere settings used in the battle arena.

## Files to Create/Modify
- Create `src/game/config/visual_config.rs` — VisualConfig struct with fog, lighting, sky settings
- Modify `src/game/config/mod.rs` — Add `pub mod visual_config;` and re-exports

## Implementation Steps
1. Scan `battle_arena.rs` for visual constants in the uniform update code and initialization:
   - Fog density: `0.008`
   - Fog color: `(0.4, 0.25, 0.35)` — apocalyptic purple-brown
   - Sun direction: low horizon angle for rim lighting
   - Sun color: `(1.2, 0.6, 0.35)` — orange-red HDR
   - Ambient intensity: `0.15` — dark for contrast
   - Sky config: `ApocalypticSkyConfig` values (already a struct, reference it)
   - Torch intensity and flicker speed from `PointLightManager` setup
   - Lava glow color for uniform submission

2. Create the struct:
   ```rust
   use glam::Vec3;

   #[derive(Clone, Debug)]
   pub struct VisualConfig {
       // Fog
       pub fog_density: f32,
       pub fog_color: Vec3,

       // Directional light (sun)
       pub sun_direction: Vec3,
       pub sun_color: Vec3,
       pub ambient_intensity: f32,

       // Torches
       pub torch_intensity: f32,
       pub torch_flicker_speed: f32,
       pub torch_radius: f32,

       // Lava glow (affects fog and sky)
       pub lava_glow_color: Vec3,
       pub lava_glow_strength: f32,
   }

   impl Default for VisualConfig {
       fn default() -> Self { /* match current hardcoded values */ }
   }

   impl VisualConfig {
       pub fn battle_arena() -> Self { Self::default() }
   }
   ```

3. Update `src/game/config/mod.rs` to include and re-export.

4. Run `cargo check`.

## Code Patterns
Follow the `ApocalypticSkyConfig` pattern from `engine/src/render/apocalyptic_sky.rs`:
```rust
// This is how visual configs are already structured in the engine
pub struct ApocalypticSkyConfig {
    pub cloud_density: f32,
    pub cloud_coverage: f32,
    pub zenith_color: (f32, f32, f32),
    // ...
}
impl ApocalypticSkyConfig {
    pub fn battle_arena() -> Self { /* preset */ }
}
```

## Acceptance Criteria
- [ ] `VisualConfig::default()` matches current hardcoded visual values
- [ ] Fog, lighting, torch, and lava glow settings are all captured
- [ ] Struct derives `Clone` and `Debug`
- [ ] Re-exported from `src/game/config/mod.rs`
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
After this story, all visual atmosphere values have a single-source-of-truth struct. You can create `VisualConfig::default()` and get the exact apocalyptic look currently hardcoded. No changes to `battle_arena.rs` yet — integration comes in Story 11.

## Dependencies
- Depends on: None
