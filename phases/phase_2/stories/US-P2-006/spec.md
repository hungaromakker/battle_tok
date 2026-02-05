# US-P2-006: Create Chain Bridge Material Shaders

## Description
Create two shaders for the chain bridge connecting hex islands: wood planks for the walkway and metallic chain for the supports - this makes the bridge look realistic rather than flat colored.

## The Core Concept / Why This Matters
The reference art shows a dramatic chain bridge spanning the lava between two castle islands. Currently we likely have a simple colored mesh. The bridge needs TWO materials:

1. **Wood planks** - Brown with subtle grain variation, matte lighting
2. **Chain metal** - Metallic gray with bright Fresnel rim highlights

The metallic chain is especially important - real metal has a distinctive look where edges catch light (the Fresnel effect). This makes chains look like actual iron/steel.

## Goal
Create `shaders/wood_plank.wgsl` and `shaders/chain_metal.wgsl` with corresponding Rust module.

## Files to Create/Modify
- `shaders/wood_plank.wgsl` (NEW) — Wood plank shader with grain
- `shaders/chain_metal.wgsl` (NEW) — Metallic shader with Fresnel rim
- `engine/src/render/bridge_materials.rs` (NEW) — Rust module for both materials
- `engine/src/render/mod.rs` — Add export

## Implementation Steps
1. Create `shaders/wood_plank.wgsl`:
   - Base brown color: `vec3<f32>(0.38, 0.26, 0.16)`
   - Add noise variation for wood grain using world_pos
   - Standard Lambert lighting
   - Include fog application

2. Create `shaders/chain_metal.wgsl`:
   - Base steel color: `vec3<f32>(0.55, 0.58, 0.62)`
   - Fresnel rim: `pow(1.0 - dot(normal, view_dir), 4.0)`
   - Add rim highlight: `color += vec3(1.0) * rim * shine`
   - Slightly specular look

3. Create `engine/src/render/bridge_materials.rs`:
   - WoodPlankMaterial and ChainMetalMaterial structs
   - Both follow same pattern as castle_material.rs
   - Include ChainParams with `shine` uniform

4. Add export in mod.rs

5. Run `cargo check`

## Code Patterns
Wood plank shader (simple):
```wgsl
@fragment
fn fs_wood(
    @location(0) world_pos: vec3<f32>,
    @location(1) world_nrm: vec3<f32>,
    @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {
    // Base brown wood color
    var col = vec3<f32>(0.38, 0.26, 0.16);

    // Add noise for grain variation
    let grain = noise(world_pos.xz * 5.0) * 0.15 - 0.075;
    col = col + vec3<f32>(grain, grain * 0.5, 0.0);

    // Lambert lighting
    let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
    col = col * (0.3 + ndl * 0.9);

    return vec4<f32>(col, 1.0);
}
```

Chain metal shader with Fresnel:
```wgsl
struct ChainParams {
    shine: f32,
}

@group(1) @binding(0) var<uniform> chain: ChainParams;

@fragment
fn fs_chain(
    @location(0) world_pos: vec3<f32>,
    @location(1) world_nrm: vec3<f32>,
    @location(2) view_pos: vec3<f32>,
    @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {
    let steel = vec3<f32>(0.55, 0.58, 0.62);
    let n = normalize(world_nrm);
    let v = normalize(view_pos - world_pos);
    let l = normalize(light_dir);

    // Fresnel rim (bright at grazing angles)
    let rim = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 4.0);

    // Lambert + rim
    let ndl = max(dot(n, l), 0.0);
    var col = steel * (0.2 + ndl * 1.1);
    col = col + vec3<f32>(1.0) * rim * chain.shine;

    return vec4<f32>(col, 1.0);
}
```

## Acceptance Criteria
- [ ] Wood planks show brown color with subtle grain variation
- [ ] Chains show metallic gray with bright rim/edge highlights
- [ ] Both shaders compile without WGSL errors
- [ ] Rust module creates pipelines without panics
- [ ] `cargo check` passes

## Success Looks Like
When applied to the bridge:
- Planks look like actual wood boards with visible grain pattern
- Chains have bright edges where light catches them (Fresnel)
- The materials clearly distinguish wood vs metal parts
- Bridge looks realistic, not like flat colored mesh

## Dependencies
- Depends on: None (can be created independently)
