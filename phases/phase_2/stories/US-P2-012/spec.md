# US-P2-012: Update Terrain Colors to Apocalyptic Palette

## Description
Update the terrain shader and its Rust parameters to use scorched volcanic colors instead of bright green grass - this grounds the apocalyptic atmosphere.

## The Core Concept / Why This Matters
The current terrain likely uses standard grass/dirt/rock/snow colors which feel peaceful. For the apocalyptic battle arena, we need:

1. **Scorched grass** - Dark olive, not bright green
2. **Ashen dirt** - Brown-gray, like volcanic ash
3. **Volcanic rock** - Dark gray with purple tint
4. **Ash peaks** - Gray/tan instead of white snow

This simple color change dramatically affects the mood - the ground should look like the world has ended.

## Goal
Update `shaders/terrain_enhanced.wgsl` colors and the Rust code that sets TerrainParams to use apocalyptic palette.

## Files to Create/Modify
- `shaders/terrain_enhanced.wgsl` — May need to adjust height bands
- Rust code that initializes TerrainParams — Change color values

## Implementation Steps
1. Find where TerrainParams is initialized in Rust (likely in battle_arena.rs or a terrain module)

2. Update the color values:
   ```rust
   TerrainParams {
       grass: Vec3::new(0.15, 0.18, 0.10),    // Scorched olive (was ~0.3, 0.5, 0.2)
       dirt: Vec3::new(0.28, 0.20, 0.14),     // Ashen brown (was ~0.4, 0.3, 0.2)
       rock: Vec3::new(0.25, 0.22, 0.24),     // Dark volcanic with purple tint
       snow: Vec3::new(0.50, 0.48, 0.45),     // Ash/dust (was white ~0.9, 0.9, 0.9)
       // Keep height bands similar but can adjust if needed
   }
   ```

3. If colors are hardcoded in the shader, update there instead:
   ```wgsl
   // Replace bright colors with apocalyptic ones
   let grass = vec3<f32>(0.15, 0.18, 0.10);
   let dirt = vec3<f32>(0.28, 0.20, 0.14);
   let rock = vec3<f32>(0.25, 0.22, 0.24);
   let snow = vec3<f32>(0.50, 0.48, 0.45);
   ```

4. Consider adding more noise variation for scorched/burnt patches:
   ```wgsl
   // Random burnt patches
   let burn = noise(world_pos.xz * 0.5);
   if burn > 0.7 {
       col = col * 0.6;  // Darker burnt areas
   }
   ```

5. Run `cargo check`

## Code Patterns
The terrain shader already has TerrainParams with color fields:
```wgsl
struct TerrainParams {
    grass: vec3<f32>,
    _pad0: f32,
    dirt: vec3<f32>,
    _pad1: f32,
    rock: vec3<f32>,
    _pad2: f32,
    snow: vec3<f32>,
    _pad3: f32,
    // ... height bands ...
}
```

If using a Rust uniform buffer:
```rust
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct TerrainUniforms {
    grass_r: f32, grass_g: f32, grass_b: f32, _pad0: f32,
    dirt_r: f32, dirt_g: f32, dirt_b: f32, _pad1: f32,
    // ...
}

// When updating:
let uniforms = TerrainUniforms {
    grass_r: 0.15, grass_g: 0.18, grass_b: 0.10, _pad0: 0.0,
    dirt_r: 0.28, dirt_g: 0.20, dirt_b: 0.14, _pad1: 0.0,
    rock_r: 0.25, rock_g: 0.22, rock_b: 0.24, _pad2: 0.0,
    snow_r: 0.50, snow_g: 0.48, snow_b: 0.45, _pad3: 0.0,
    // ...
};
```

## Acceptance Criteria
- [ ] Terrain appears dark and scorched (no bright green)
- [ ] Grass areas are dark olive/brown
- [ ] Peaks are ash-colored, not white snow
- [ ] Rock dominates steep cliff areas
- [ ] Overall terrain matches apocalyptic mood
- [ ] `cargo check` passes

## Success Looks Like
When viewing the terrain:
- NO bright green grass anywhere - it's all scorched dark olive
- The ground looks like the aftermath of volcanic activity
- Peaks are gray/tan (ash/dust) not white
- Rocky cliffs are dark volcanic gray with slight purple tint
- The terrain supports the apocalyptic atmosphere instead of fighting it

## Dependencies
- Depends on: None (can be modified independently)
