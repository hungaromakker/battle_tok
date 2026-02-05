# US-P2-003: Create Castle Stone Material Shader

## Description
Create a new WGSL shader for medieval castle stone with procedural brick pattern, mortar lines, grime darkening near the ground, and warm torch bounce lighting - this transforms plain building blocks into realistic castle walls.

## The Core Concept / Why This Matters
Currently building blocks use simple flat colors. Real castles have:
1. **Brick/block pattern** - visible stone blocks with mortar between them
2. **Grime accumulation** - lower parts of walls are darker from dirt/moss
3. **Torch lighting** - warm orange glow bouncing from below

This shader makes buildings look like actual medieval castles instead of Minecraft blocks. The procedural approach means we don't need textures - the pattern is computed from world position, so it tiles perfectly on any size structure.

## Goal
Create `shaders/castle_stone.wgsl` and `engine/src/render/castle_material.rs` that renders realistic medieval stone.

## Files to Create/Modify
- `shaders/castle_stone.wgsl` (NEW) — The WGSL shader with brick pattern and lighting
- `engine/src/render/castle_material.rs` (NEW) — Rust module to create pipeline and bind groups
- `engine/src/render/mod.rs` — Add `pub mod castle_material;` export

## Implementation Steps
1. Create `shaders/castle_stone.wgsl` with:
   - Standard Uniforms struct (view_proj, camera_pos, time, sun_dir, fog)
   - CastleParams struct (torch_color, torch_strength, time)
   - hash/noise functions for procedural pattern
   - Vertex shader passing world_pos, world_normal, view_dir
   - Fragment shader with brick pattern, grime, torch bounce, Lambert lighting

2. Create `engine/src/render/castle_material.rs` with:
   - CastleMaterial struct holding pipeline, bind_group_layout, uniform_buffer
   - `new()` constructor following StormySky pattern
   - `update()` to write uniforms to buffer
   - `render()` to bind pipeline and draw

3. Add export in `engine/src/render/mod.rs`

4. Run `cargo check`

## Code Patterns
Follow the existing `stormy_sky.rs` pattern for Rust module structure:
```rust
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CastleUniforms {
    view_proj: [[f32; 4]; 4],
    camera_pos_x: f32,
    camera_pos_y: f32,
    camera_pos_z: f32,
    time: f32,
    // ... etc, scalar fields for alignment
}

pub struct CastleMaterial {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}
```

WGSL brick pattern:
```wgsl
// Procedural brick using world position
let block_uv = world_pos.xz * 1.2 + world_pos.yy * 0.35;
let b = noise(block_uv);

// Mortar lines between bricks
let mortar = smoothstep(0.48, 0.52, abs(fract(block_uv.x) - 0.5))
           + smoothstep(0.48, 0.52, abs(fract(block_uv.y) - 0.5));

// Grime darkening near bottom
let grime = clamp(1.0 - world_pos.y * 0.12, 0.0, 1.0);

// Torch flicker
let torch_flicker = 0.85 + 0.15 * sin(time * 12.0 + world_pos.x * 0.3);
```

## Acceptance Criteria
- [ ] `shaders/castle_stone.wgsl` exists and compiles (no WGSL syntax errors)
- [ ] Stone shows visible brick/block pattern when rendered
- [ ] Bottom of walls appears darker (grime effect)
- [ ] Warm orange glow visible on stone surfaces (torch bounce)
- [ ] Rust module creates pipeline without panics
- [ ] `cargo check` passes

## Success Looks Like
When you apply this material to building blocks:
- You see a clear brick pattern - rectangular stones with darker mortar lines
- The base of walls is noticeably darker than the top
- There's a warm orange tint on lower surfaces (simulated torch light)
- It looks like an actual medieval castle wall, not a solid color block

## Dependencies
- Depends on: None (can be created independently)
