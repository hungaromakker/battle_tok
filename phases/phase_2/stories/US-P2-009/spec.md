# US-P2-009: Enhance Depth-Based Fog Post-Pass

## Description
Update the fog post-pass shader to use the depth buffer for distance-based fog with height variation - this creates atmospheric depth and makes distant objects fade into the stormy haze.

## The Core Concept / Why This Matters
Good fog does two things:
1. **Depth perception** - Objects fade with distance, helping the brain perceive 3D space
2. **Atmosphere** - Matches the stormy purple/brown environment

The fog is a POST-PROCESS, meaning it:
- Reads the color and depth textures from the main render pass
- Reconstructs world position from depth
- Applies fog based on distance and height
- Writes to the final output

Height-based fog makes ground level hazier than elevated areas, matching real atmospheric conditions and the concept art.

## Goal
Update `shaders/fog_post.wgsl` to read depth buffer and apply distance + height-based fog.

## Files to Create/Modify
- `shaders/fog_post.wgsl` — Enhance with depth buffer sampling
- `engine/src/render/fog_post.rs` (NEW or enhance) — Rust module for fog post-pass

## Implementation Steps
1. Update `shaders/fog_post.wgsl`:
   - Add depth texture and sampler bindings
   - Add inv_view_proj uniform for world position reconstruction
   - Reconstruct world position from depth
   - Calculate distance fog: `1.0 - exp(-dist * density)`
   - Add height fog: `exp(-world_pos.y * 0.05)`
   - Blend fog color with scene color

2. Create/update `engine/src/render/fog_post.rs`:
   - FogPostPass struct with pipeline, bind group
   - Requires: scene color texture, depth texture
   - Apply() method renders fullscreen quad

3. Add export in mod.rs

4. Run `cargo check`

## Code Patterns
World position reconstruction from depth:
```wgsl
fn reconstruct_world_pos(uv: vec2<f32>, depth: f32, inv_view_proj: mat4x4<f32>) -> vec3<f32> {
    // NDC coordinates (-1 to 1)
    let ndc = vec4<f32>(
        uv.x * 2.0 - 1.0,
        (1.0 - uv.y) * 2.0 - 1.0,  // Flip Y
        depth,
        1.0
    );

    // World position
    let world = inv_view_proj * ndc;
    return world.xyz / world.w;
}
```

Fog calculation:
```wgsl
@fragment
fn fs_fog(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene_color = textureSample(color_tex, color_samp, in.uv).rgb;
    let depth = textureSample(depth_tex, depth_samp, in.uv).r;

    // Skip sky (depth at far plane)
    if depth >= 0.9999 {
        return vec4<f32>(scene_color, 1.0);
    }

    let world_pos = reconstruct_world_pos(in.uv, depth, uniforms.inv_view_proj);
    let camera_pos = uniforms.camera_pos;

    // Distance fog (exponential)
    let dist = length(world_pos - camera_pos);
    var fog_factor = 1.0 - exp(-dist * uniforms.fog_density);

    // Height fog (thicker near ground)
    let height_fog = exp(-max(world_pos.y, 0.0) * 0.05);
    fog_factor = mix(fog_factor, 1.0, height_fog * 0.3);

    // Stormy purple-brown fog color
    let fog_color = vec3<f32>(0.4, 0.3, 0.5);

    let final_color = mix(scene_color, fog_color, clamp(fog_factor, 0.0, 1.0));
    return vec4<f32>(final_color, 1.0);
}
```

## Acceptance Criteria
- [ ] Distant objects fade into fog (not sharp cutoff)
- [ ] Ground level is foggier than elevated areas
- [ ] Fog color matches stormy atmosphere (purple-brown)
- [ ] Sky is NOT fogged (depth check skips far plane)
- [ ] Depth buffer reads correctly (no artifacts)
- [ ] `cargo check` passes

## Success Looks Like
When running the game:
- Looking at distant terrain, it gradually fades into purple-brown haze
- Objects near ground level are slightly hazier
- The sky remains clear (fog doesn't affect far plane)
- The fog adds significant atmosphere and depth perception
- There's no hard cutoff - it's a smooth exponential falloff

## Dependencies
- Depends on: None (can be created independently, but integrates with post-process chain)
