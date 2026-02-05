# US-P2-002: Enhance Lava Shader with HDR Emission

## Description
Update the lava shader to output brighter HDR values so the molten cracks properly "pop" after tonemapping - this makes the lava look like actual glowing magma instead of flat orange.

## The Core Concept / Why This Matters
The lava shader already exists and works, but the emissive values are too conservative. In HDR rendering, emissive surfaces need to output values ABOVE 1.0 (sometimes 2.0-4.0) so that after tonemapping, they still appear bright and bloom correctly. Currently the lava might look flat and washed out - we need it to GLOW.

The reference image shows lava rivers that emit intense orange-yellow light, illuminating the surrounding environment. This requires:
1. Higher HDR core color values (3.0+ intensity)
2. Pulsing animation to make it feel alive
3. Reduced fog influence (emissive surfaces cut through fog)

## Goal
Make the lava cracks glow intensely with pulsing animation and HDR values that bloom after tonemapping.

## Files to Create/Modify
- `shaders/lava.wgsl` — Increase emissive values, add pulse, reduce fog

## Implementation Steps
1. Open `shaders/lava.wgsl` and find the `core_color` definition
2. Change core_color from current value to `vec3<f32>(3.0, 0.8, 0.1)` (bright HDR orange)
3. Find where `emissive_strength` is used and multiply by 2.5
4. Add pulsing: `let pulse = 0.9 + 0.1 * sin(lava.time * 2.0);` and apply to emissive
5. Find the fog application section and reduce fog_amount multiplier to 0.1 (emissive cuts through)
6. Run `cargo check` to ensure shader compiles (wgpu validates WGSL)

## Code Patterns
Current lava shader structure (from `shaders/lava.wgsl`):
```wgsl
// COLOR MIXING section - change these values:
let core = vec3<f32>(1.4, 0.3, 0.05);  // <- Change to (3.0, 0.8, 0.1)
let crust = vec3<f32>(0.08, 0.02, 0.01);

// EMISSIVE OUTPUT section - add pulse:
let pulse = 0.9 + 0.1 * sin(lava.time * 2.0);  // <- Add this
let emissive = lava.emissive_strength * 2.5 * (heat + edge_glow) * pulse;
color = color * emissive;

// FOG section - reduce influence:
let fog_amount = (1.0 - exp(-distance * uniforms.fog_density)) * 0.1;  // <- Change 0.15 to 0.1
```

## Acceptance Criteria
- [ ] Lava cracks appear bright orange/yellow (noticeably brighter than before)
- [ ] Cracks pulse/animate smoothly over time
- [ ] Fog doesn't completely wash out lava at distance
- [ ] Shader compiles without WGSL errors
- [ ] `cargo check` passes

## Success Looks Like
When you run the game and look at the lava:
- The cracks should be INTENSELY bright - almost uncomfortable to look at directly
- You should see a subtle pulsing animation (cracks brighten/dim rhythmically)
- Even at distance, the lava glow should cut through the fog
- The lava should look like actual molten rock, not flat orange paint

## Dependencies
- Depends on: None (shader can be modified independently)
