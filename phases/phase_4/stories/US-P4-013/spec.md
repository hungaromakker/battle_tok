# US-P4-013: Variety System

## Description
Create `src/game/asset_editor/variety.rs` with a seed-based procedural variation system for placed assets. `VarietyParams` defines ranges for scale, rotation, tilt, hue shift, saturation, brightness, and noise displacement. A `SimpleRng` using xorshift32 ensures deterministic output -- the same seed always produces the same variation. Seeds are derived from world position via FNV-1a hashing so an asset placed at the same location always looks identical. The module provides `VarietyInstance` (the computed variation result), `apply_color_variety()` for shifting vertex colors, `variety_to_transform()` for producing a Mat4, and preset profiles for Tree, Grass, Rock, and Structure categories. This is a **pure math/data module** with no rendering, UI, or GPU dependencies. The editor is a separate binary (`cargo run --bin battle_editor`). `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
Natural environments look terrible when every tree, rock, and grass clump is identical. Variety is what makes a forest feel like a forest instead of a grid of clones. But manually placing hundreds of unique assets is impractical. The Variety System solves this by taking a single base asset and generating controlled random variations: slightly taller, slightly tilted, shifted hue, displaced vertices. The key insight is **determinism** -- given the same seed (derived from world position), the same variation is always produced. This means variety is computed at placement time, never stored per-instance, keeping memory usage constant regardless of how many copies exist in the world.

Preset profiles encode artistic knowledge about how different asset types should vary: trees sway more than rocks, grass varies in scale more than structures, rocks have more noise displacement but less color shift. These presets give artists a sensible starting point they can then fine-tune per asset.

## Goal
Create `src/game/asset_editor/variety.rs` with `VarietyParams`, `VarietyInstance`, `SimpleRng` (xorshift32), `generate_variety()`, `seed_from_position()`, `apply_color_variety()`, `variety_to_transform()`, and preset profiles for Tree, Grass, Rock, and Structure categories.

## Files to Create/Modify
- **Create** `src/game/asset_editor/variety.rs` -- All variety types, xorshift32 RNG, generation functions, color shifting, transform matrix, presets
- **Modify** `src/game/asset_editor/mod.rs` -- Add `pub mod variety;`

## Implementation Steps

1. Define `VarietyParams` with all 10 variation fields (must derive Serialize/Deserialize for .btasset format):
   ```rust
   #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
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

   impl Default for VarietyParams {
       fn default() -> Self {
           Self {
               scale_min: 0.8, scale_max: 1.2, scale_y_bias: 0.0,
               random_y_rotation: true, tilt_max_degrees: 5.0,
               hue_shift_range: 15.0, saturation_range: 0.1, brightness_range: 0.1,
               noise_displacement: 0.0, noise_frequency: 1.0,
           }
       }
   }
   ```

2. Implement `SimpleRng` using xorshift32:
   ```rust
   pub struct SimpleRng { state: u32 }

   impl SimpleRng {
       pub fn new(seed: u32) -> Self { Self { state: seed.max(1) } }

       pub fn next_u32(&mut self) -> u32 {
           let mut x = self.state;
           x ^= x << 13;
           x ^= x >> 17;
           x ^= x << 5;
           self.state = x;
           x
       }

       pub fn next_f32(&mut self) -> f32 { self.next_u32() as f32 / u32::MAX as f32 }

       pub fn range(&mut self, min: f32, max: f32) -> f32 {
           min + self.next_f32() * (max - min)
       }
   }
   ```

3. Implement `seed_from_position()` using FNV-1a hashing:
   ```rust
   pub fn seed_from_position(x: f32, y: f32, z: f32) -> u32 {
       let ix = (x * 1000.0) as i32;
       let iy = (y * 1000.0) as i32;
       let iz = (z * 1000.0) as i32;
       let mut h: u32 = 2166136261;
       h ^= ix as u32; h = h.wrapping_mul(16777619);
       h ^= iy as u32; h = h.wrapping_mul(16777619);
       h ^= iz as u32; h = h.wrapping_mul(16777619);
       if h == 0 { 1 } else { h }
   }
   ```

4. Define `VarietyInstance`:
   ```rust
   pub struct VarietyInstance {
       pub scale: Vec3,
       pub rotation_y: f32,
       pub tilt_angle: f32,
       pub tilt_axis: f32,
       pub hue_shift: f32,
       pub saturation_shift: f32,
       pub brightness_shift: f32,
       pub noise_seed: u32,
   }
   ```

5. Implement `generate_variety()`:
   ```rust
   pub fn generate_variety(params: &VarietyParams, seed: u32) -> VarietyInstance {
       let mut rng = SimpleRng::new(seed);
       let scale_base = rng.range(params.scale_min, params.scale_max);
       let scale_y = scale_base * (1.0 + rng.range(-params.scale_y_bias, params.scale_y_bias));
       let rotation_y = if params.random_y_rotation {
           rng.range(0.0, std::f32::consts::TAU)
       } else { 0.0 };
       VarietyInstance {
           scale: Vec3::new(scale_base, scale_y, scale_base),
           rotation_y,
           tilt_angle: rng.range(0.0, params.tilt_max_degrees.to_radians()),
           tilt_axis: rng.range(0.0, std::f32::consts::TAU),
           hue_shift: rng.range(-params.hue_shift_range, params.hue_shift_range),
           saturation_shift: rng.range(-params.saturation_range, params.saturation_range),
           brightness_shift: rng.range(-params.brightness_range, params.brightness_range),
           noise_seed: rng.next_u32(),
       }
   }
   ```

6. Implement `apply_color_variety()` with HSV manipulation:
   ```rust
   pub fn apply_color_variety(color: [f32; 4], instance: &VarietyInstance) -> [f32; 4] {
       let (h, s, v) = rgb_to_hsv(color[0], color[1], color[2]);
       let new_h = (h + instance.hue_shift).rem_euclid(360.0);
       let new_s = (s + instance.saturation_shift).clamp(0.0, 1.0);
       let new_v = (v + instance.brightness_shift).clamp(0.0, 1.0);
       let (r, g, b) = hsv_to_rgb(new_h, new_s, new_v);
       [r, g, b, color[3]]
   }
   ```

7. Implement `variety_to_transform()`:
   ```rust
   pub fn variety_to_transform(instance: &VarietyInstance, position: Vec3) -> Mat4 {
       let y_rot = Quat::from_rotation_y(instance.rotation_y);
       let tilt_dir = Vec3::new(instance.tilt_axis.cos(), 0.0, instance.tilt_axis.sin());
       let tilt_rot = Quat::from_axis_angle(tilt_dir, instance.tilt_angle);
       let rotation = tilt_rot * y_rot;
       Mat4::from_scale_rotation_translation(instance.scale, rotation, position)
   }
   ```

8. Create preset profiles:
   ```rust
   impl VarietyParams {
       pub fn tree_preset() -> Self {
           Self { scale_min: 0.7, scale_max: 1.4, scale_y_bias: 0.3,
               random_y_rotation: true, tilt_max_degrees: 8.0,
               hue_shift_range: 20.0, saturation_range: 0.15, brightness_range: 0.2,
               noise_displacement: 0.08, noise_frequency: 1.5 }
       }
       pub fn grass_preset() -> Self {
           Self { scale_min: 0.5, scale_max: 1.5, scale_y_bias: 0.5,
               random_y_rotation: true, tilt_max_degrees: 15.0,
               hue_shift_range: 25.0, saturation_range: 0.2, brightness_range: 0.25,
               noise_displacement: 0.03, noise_frequency: 3.0 }
       }
       pub fn rock_preset() -> Self {
           Self { scale_min: 0.6, scale_max: 1.6, scale_y_bias: 0.1,
               random_y_rotation: true, tilt_max_degrees: 20.0,
               hue_shift_range: 8.0, saturation_range: 0.05, brightness_range: 0.15,
               noise_displacement: 0.12, noise_frequency: 1.0 }
       }
       pub fn structure_preset() -> Self {
           Self { scale_min: 0.95, scale_max: 1.05, scale_y_bias: 0.0,
               random_y_rotation: false, tilt_max_degrees: 0.0,
               hue_shift_range: 5.0, saturation_range: 0.05, brightness_range: 0.1,
               noise_displacement: 0.0, noise_frequency: 0.0 }
       }
   }
   ```

9. Add `pub mod variety;` to `src/game/asset_editor/mod.rs`.

## Code Patterns
The xorshift32 pattern is a standard minimal PRNG:
```rust
let seed = seed_from_position(world_x, world_y, world_z);
let variety = generate_variety(&params, seed);
let transform = variety_to_transform(&variety, position);
let shifted_color = apply_color_variety(vertex_color, &variety);
```

Color shifting via HSV preserves the character of the color while shifting in perceptually meaningful ways.

## Acceptance Criteria
- [ ] `variety.rs` exists with `VarietyParams`, `VarietyInstance`, `SimpleRng` types
- [ ] `VarietyParams` has all 10 fields: scale_min, scale_max, scale_y_bias, random_y_rotation, tilt_max_degrees, hue_shift_range, saturation_range, brightness_range, noise_displacement, noise_frequency
- [ ] `VarietyParams` derives `Serialize` and `Deserialize`
- [ ] `SimpleRng` uses xorshift32 and is deterministic (same seed produces same output)
- [ ] `seed_from_position()` maps world coordinates to deterministic seeds
- [ ] Same seed always produces the same `VarietyInstance`
- [ ] `generate_variety()` respects all parameter ranges
- [ ] `apply_color_variety()` shifts hue, saturation, and brightness correctly
- [ ] `variety_to_transform()` produces a valid `Mat4` combining scale, rotation, and tilt
- [ ] Preset profiles exist for Tree, Grass, Rock, and Structure
- [ ] `Default` is implemented for `VarietyParams`
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check` passes with 0 errors

## Verification Commands
- `cmd`: `test -f /home/hungaromakker/battle_tok/src/game/asset_editor/variety.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `variety.rs module exists`
- `cmd`: `grep -c 'VarietyParams\|VarietyInstance\|SimpleRng' /home/hungaromakker/battle_tok/src/game/asset_editor/variety.rs`
  `expect_gt`: 0
  `description`: `Core variety types defined`
- `cmd`: `grep -c 'generate_variety\|seed_from_position\|apply_color_variety\|variety_to_transform' /home/hungaromakker/battle_tok/src/game/asset_editor/variety.rs`
  `expect_gt`: 0
  `description`: `Core variety functions implemented`
- `cmd`: `grep -c 'tree_preset\|grass_preset\|rock_preset\|structure_preset' /home/hungaromakker/battle_tok/src/game/asset_editor/variety.rs`
  `expect_gt`: 0
  `description`: `Preset profiles exist`
- `cmd`: `grep -c 'pub mod variety' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `variety module registered in mod.rs`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `Project compiles`

## Success Looks Like
An artist places 20 copies of the same oak tree asset in the world. Each tree looks slightly different -- some are taller, some wider, some have more yellow-green leaves, some lean slightly to the left. But the variations feel natural, not like random noise. If the artist deletes a tree and places a new one at the exact same position, the same variation appears again (determinism). Switching to the Rock preset and placing rocks produces subtler variation -- rocks barely change shape but vary in shade and tilt significantly. Structures barely vary at all -- buildings stay upright and the right size.

## Dependencies
- Depends on: US-P4-001 (needs editor skeleton for module registration)

## Complexity
- Complexity: normal
- Min iterations: 1
