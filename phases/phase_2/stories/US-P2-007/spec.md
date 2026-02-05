# US-P2-007: Create Team Flag Shader with Wind Animation

## Description
Create a flag shader with vertex-based wind animation and team-colored stripe pattern - this brings the castle flags to life with realistic cloth movement.

## The Core Concept / Why This Matters
Flags are important visual indicators in the game:
1. **Team identification** - Red vs Blue flags show territory ownership
2. **Atmosphere** - Waving flags make the world feel alive
3. **Win condition** - Capturing the flag is core gameplay

The flag needs:
- **Vertex animation** - sin() wave displacement based on UV.x position
- **Team color** - Primary color set per-flag
- **Stripe pattern** - Horizontal band for visual interest

## Goal
Create `shaders/flag.wgsl` with wind animation vertex shader and team-colored fragment shader.

## Files to Create/Modify
- `shaders/flag.wgsl` (NEW) — Flag shader with vertex animation
- `engine/src/render/flag_material.rs` (NEW) — Rust module for flag rendering
- `engine/src/render/mod.rs` — Add export

## Implementation Steps
1. Create `shaders/flag.wgsl` with FlagParams:
   ```wgsl
   struct FlagParams {
       time: f32,
       team_color_r: f32,
       team_color_g: f32,
       team_color_b: f32,
       stripe_color_r: f32,
       stripe_color_g: f32,
       stripe_color_b: f32,
       wind_strength: f32,  // 0.1..0.3
   }
   ```

2. Vertex shader with wave displacement:
   ```wgsl
   // Wave increases toward flag edge (uv.x), decreases toward pole (uv.y=0)
   let wave = sin(uv.x * 10.0 + params.time * 3.5) * (1.0 - uv.y) * params.wind_strength;
   let displaced_pos = pos + vec3<f32>(0.0, wave, 0.0);
   ```

3. Fragment shader with stripe:
   ```wgsl
   // Horizontal stripe band
   let stripe = smoothstep(0.45, 0.48, uv.y) - smoothstep(0.52, 0.55, uv.y);
   let color = mix(team_color, stripe_color, stripe * 0.85);
   ```

4. Create `engine/src/render/flag_material.rs`:
   - FlagMaterial struct with pipeline, uniform buffer
   - Method to set team color (red or blue)
   - Follow existing material patterns

5. Add export in mod.rs

6. Run `cargo check`

## Code Patterns
Flag vertex shader:
```wgsl
struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_flag(
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
) -> VSOut {
    // Wave displacement - more at edge (high uv.x), less at pole (low uv.y)
    let wave_amount = uv.x * (1.0 - uv.y * 0.5);
    let wave = sin(uv.x * 10.0 + params.time * 3.5) * wave_amount * params.wind_strength;

    let displaced = pos + vec3<f32>(wave * 0.3, wave, wave * 0.1);

    var out: VSOut;
    out.position = uniforms.view_proj * vec4<f32>(displaced, 1.0);
    out.uv = uv;
    return out;
}
```

Team colors (typical):
- Red team: `(0.8, 0.1, 0.1)` with darker stripe `(0.4, 0.05, 0.05)`
- Blue team: `(0.1, 0.4, 0.9)` with darker stripe `(0.05, 0.2, 0.45)`

## Acceptance Criteria
- [ ] Flag mesh deforms with wind wave animation
- [ ] Wave is stronger at flag edge, weaker near pole
- [ ] Team color displays correctly (red or blue)
- [ ] Horizontal stripe band visible
- [ ] Shader compiles without WGSL errors
- [ ] `cargo check` passes

## Success Looks Like
When rendering flags:
- The flag waves in the wind - fabric ripples from pole to edge
- The wave animation is smooth and continuous
- Team color is clearly visible (red castle has red flag)
- A darker stripe band runs horizontally across the flag

## Dependencies
- Depends on: None (can be created independently)
