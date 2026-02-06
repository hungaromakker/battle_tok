# US-P4-013: Variety System

## Description
Create `src/game/asset_editor/variety.rs` with a deterministic variety generation system that produces unique visual variations of the same base asset. `VarietyParams` defines ranges for scale, rotation, tilt, color shifts, and noise displacement. A `SimpleRng` using xorshift32 ensures that the same seed always produces the same variation. Seeds are derived from world position via hashing, so an asset placed at the same location always looks identical. This is a **pure math/data module** with no rendering or UI dependencies.

## The Core Concept / Why This Matters
A forest of identical trees looks artificial. Variety systems solve this by generating per-instance variations from a single base asset. The key insight is **deterministic randomness**: each world position maps to a unique seed, which produces a unique variation. This means the world looks consistent across sessions, artists can predict results, and no per-instance variety data needs to be stored. Different asset categories need different profiles: trees vary a lot (organic), structures barely vary (man-made).

## Goal
Create `src/game/asset_editor/variety.rs` with `VarietyParams`, `VarietyInstance`, and `SimpleRng` (xorshift32) providing deterministic, seed-based asset variation generation with category-specific presets.

## Files to Create/Modify
- **Create** `src/game/asset_editor/variety.rs` - `VarietyParams`, `VarietyInstance`, `SimpleRng`, generation functions, category presets
- **Modify** `src/game/asset_editor/mod.rs` - Add `pub mod variety;`

## Implementation Steps
1. Define `VarietyParams` struct with `Serialize`/`Deserialize` for .btasset storage:
   - `scale_min: f32` - minimum uniform scale (e.g., 0.8)
   - `scale_max: f32` - maximum uniform scale (e.g., 1.2)
   - `scale_y_bias: f32` - extra Y-axis stretch range (e.g., 0.1)
   - `random_y_rotation: bool` - full 360 degree random Y rotation
   - `tilt_max_degrees: f32` - max tilt from vertical (e.g., 5.0)
   - `hue_shift_range: f32` - max hue shift in degrees (e.g., 15.0)
   - `saturation_range: f32` - max saturation deviation (e.g., 0.1)
   - `brightness_range: f32` - max brightness deviation (e.g., 0.1)
   - `noise_displacement: f32` - vertex noise amplitude (e.g., 0.02)
   - `noise_frequency: f32` - vertex noise frequency (e.g., 1.0)
2. Define `VarietyInstance` struct (the output of generation):
   - `scale: [f32; 3]` - non-uniform scale (x, y, z)
   - `y_rotation: f32` - radians
   - `tilt: [f32; 2]` - tilt angles (x, z) in radians
   - `hue_shift: f32` - color hue offset
   - `saturation_shift: f32` - color saturation offset
   - `brightness_shift: f32` - color brightness offset
3. Implement `SimpleRng` using xorshift32:
   - `new(seed: u32)` - initialize with seed (ensure non-zero)
   - `next_u32()` - xorshift32 step: `state ^= state << 13; state ^= state >> 17; state ^= state << 5`
   - `next_f32()` - normalize u32 to 0.0..1.0
   - `range(min, max)` - map to arbitrary float range
4. Implement `seed_from_position(world_x: f32, world_z: f32) -> u32`:
   - Convert floats to integer bits
   - Combine with hash mixing: `(x_bits.wrapping_mul(73856093)) ^ (z_bits.wrapping_mul(19349663))`
   - Ensure non-zero output (xorshift32 requires seed != 0)
5. Implement `generate_variety(params, seed) -> VarietyInstance`:
   - Create `SimpleRng::new(seed)`
   - Sample uniform scale from [scale_min, scale_max]
   - Add Y bias: scale_y += rng.range(-scale_y_bias, scale_y_bias)
   - If random_y_rotation, sample y_rotation from [0, 2*PI]
   - Sample tilt angles within tilt_max_degrees (converted to radians)
   - Sample color shifts within respective ranges
6. Implement `apply_color_variety(color: [f32; 4], instance: &VarietyInstance) -> [f32; 4]`:
   - Convert RGB to HSV
   - Shift hue, saturation, brightness by instance values
   - Clamp to valid ranges
   - Convert back to RGBA
7. Create category presets as associated functions:
   - `tree_preset()` - large scale range, full rotation, moderate tilt, large color variation
   - `grass_preset()` - small scale range, full rotation, more tilt, subtle color variation
   - `rock_preset()` - moderate scale, no rotation, minimal tilt, subtle color variation
   - `structure_preset()` - minimal scale, no rotation, no tilt, no color variation

## Code Patterns
```rust
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VarietyParams {
    pub scale_min: f32,
    pub scale_max: f32,
    pub scale_y_bias: f32,
    pub random_y_rotation: bool,
    pub tilt_max_degrees: f32,
    pub hue_shift_range: f32,
    pub saturation_range: f32,
    pub brightness_range: f32,
    pub noise_displacement: f32,
    pub noise_frequency: f32,
}

pub struct SimpleRng { state: u32 }
impl SimpleRng {
    pub fn new(seed: u32) -> Self { Self { state: seed.max(1) } }
    pub fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }
    pub fn next_f32(&mut self) -> f32 { (self.next_u32() as f32) / (u32::MAX as f32) }
    pub fn range(&mut self, min: f32, max: f32) -> f32 { min + self.next_f32() * (max - min) }
}

pub fn seed_from_position(world_x: f32, world_z: f32) -> u32 {
    let hash = world_x.to_bits().wrapping_mul(73856093) ^ world_z.to_bits().wrapping_mul(19349663);
    hash.max(1)
}

pub fn generate_variety(params: &VarietyParams, seed: u32) -> VarietyInstance {
    let mut rng = SimpleRng::new(seed);
    let base_scale = rng.range(params.scale_min, params.scale_max);
    let y_bias = rng.range(-params.scale_y_bias, params.scale_y_bias);
    VarietyInstance {
        scale: [base_scale, base_scale + y_bias, base_scale],
        y_rotation: if params.random_y_rotation { rng.range(0.0, std::f32::consts::TAU) } else { 0.0 },
        tilt: [
            rng.range(-params.tilt_max_degrees, params.tilt_max_degrees).to_radians(),
            rng.range(-params.tilt_max_degrees, params.tilt_max_degrees).to_radians(),
        ],
        hue_shift: rng.range(-params.hue_shift_range, params.hue_shift_range),
        saturation_shift: rng.range(-params.saturation_range, params.saturation_range),
        brightness_shift: rng.range(-params.brightness_range, params.brightness_range),
    }
}
```

## Acceptance Criteria
- [ ] `variety.rs` exists with `VarietyParams`, `VarietyInstance`, `SimpleRng` types
- [ ] `VarietyParams` has all 10 fields: scale_min, scale_max, scale_y_bias, random_y_rotation, tilt_max_degrees, hue_shift_range, saturation_range, brightness_range, noise_displacement, noise_frequency
- [ ] `SimpleRng` (xorshift32) produces deterministic output for a given seed
- [ ] `seed_from_position()` maps world coordinates to deterministic seeds
- [ ] Same seed always produces the same `VarietyInstance`
- [ ] `generate_variety()` respects all parameter ranges
- [ ] `apply_color_variety()` shifts hue, saturation, and brightness within ranges
- [ ] Preset profiles exist for Tree, Grass, Rock, and Structure
- [ ] `VarietyParams` derives `Serialize` and `Deserialize` (for .btasset format)
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo check --bin battle_arena` passes with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with variety module`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles unchanged`
- `cmd`: `test -f src/game/asset_editor/variety.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `variety.rs file exists`
- `cmd`: `grep -c 'VarietyParams\|VarietyInstance\|SimpleRng\|generate_variety' src/game/asset_editor/variety.rs`
  `expect_gt`: 0
  `description`: `Core variety types and functions exist`
- `cmd`: `grep -c 'tree_preset\|grass_preset\|rock_preset\|structure_preset' src/game/asset_editor/variety.rs`
  `expect_gt`: 0
  `description`: `Category presets are defined`
- `cmd`: `grep -c 'seed_from_position\|apply_color_variety' src/game/asset_editor/variety.rs`
  `expect_gt`: 0
  `description`: `Position seeding and color variety functions exist`
- `cmd`: `grep -c 'pub mod variety' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `variety module registered in mod.rs`

## Success Looks Like
An artist places 20 copies of the same oak tree asset in the world. Each tree looks slightly different -- some are taller, some wider, some have more yellow-green leaves, some lean slightly to the left. But the variations feel natural, not like random noise. If the artist deletes a tree and places a new one at the exact same position, the same variation appears again (determinism). Switching to the Rock preset and placing rocks produces subtler variation -- rocks change shade but barely change shape. Structures barely vary at all -- buildings stay upright and properly sized.

## Dependencies
- Depends on: None (pure data/algorithm module)

## Complexity
- Complexity: normal
- Min iterations: 1
