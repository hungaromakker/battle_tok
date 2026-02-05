# US-P2-010: Integrate ACES Tonemapping Post-Pass

## Description
Ensure ACES tonemapping is integrated as the final post-process pass, converting HDR rendered values to LDR for display - this is what makes bright lava and embers "bloom" properly.

## The Core Concept / Why This Matters
The engine renders to an HDR texture where values can exceed 1.0 (the lava shader outputs 3.0+ for bright cracks). But monitors can only display 0.0-1.0. ACES tonemapping:

1. **Compresses HDR to LDR** - Maps infinite range to [0,1]
2. **Preserves contrast** - Bright areas stay bright relative to dark
3. **Cinematic look** - ACES is the film industry standard, used in movies
4. **Enables bloom** - Values > 1.0 appear extra bright after compression

Without tonemapping, HDR values just clamp to white (ugly). With ACES, they compress beautifully.

## Goal
Verify `shaders/tonemap_aces.wgsl` exists and is correctly integrated into the render pipeline as the final pass.

## Files to Create/Modify
- `shaders/tonemap_aces.wgsl` — Verify/create ACES tonemap shader
- `src/bin/battle_arena.rs` — Add tonemap pass after all rendering

## Implementation Steps
1. Check if `shaders/tonemap_aces.wgsl` exists:
   - If yes, verify it has ACES function
   - If no, create it

2. ACES implementation:
   ```wgsl
   fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
       let a = 2.51;
       let b = 0.03;
       let c = 2.43;
       let d = 0.59;
       let e = 0.14;
       return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3(0.0), vec3(1.0));
   }
   ```

3. Full tonemap pass:
   - Sample HDR color texture
   - Apply exposure (optional, default 1.0)
   - Apply ACES tonemap
   - Apply gamma correction (pow(color, 1/2.2))
   - Output to swapchain

4. Update battle_arena.rs render order:
   - Scene renders to HDR texture
   - Fog post-pass
   - Tonemap pass → swapchain

5. Run `cargo check`

## Code Patterns
Complete tonemap shader:
```wgsl
struct TonemapParams {
    exposure: f32,
}

@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_samp: sampler;
@group(0) @binding(2) var<uniform> params: TonemapParams;

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs_tonemap(in: VertexOutput) -> @location(0) vec4<f32> {
    let hdr = textureSample(hdr_tex, hdr_samp, in.uv).rgb;

    // Apply exposure
    let exposed = hdr * params.exposure;

    // ACES tonemap
    let tonemapped = aces_tonemap(exposed);

    // Gamma correction
    let gamma = pow(tonemapped, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(gamma, 1.0);
}
```

Pipeline without depth:
```rust
depth_stencil: None,  // Post-process doesn't need depth
```

## Acceptance Criteria
- [ ] HDR scene renders to intermediate texture (not swapchain)
- [ ] Tonemap pass reads HDR texture, outputs to swapchain
- [ ] Lava and embers appear bright but not clipped white
- [ ] Dark areas maintain detail (not crushed to black)
- [ ] No color banding artifacts
- [ ] `cargo check` passes

## Success Looks Like
When running the game:
- Bright lava cracks appear intensely bright orange, but you can still see detail
- Dark castle stone maintains visible texture even in shadows
- The overall image has a cinematic, filmic quality
- HDR values (>1.0) compress nicely instead of clipping to white
- Colors feel natural and balanced

## Dependencies
- Depends on: US-P2-009 (fog pass, so tonemap is truly final)
