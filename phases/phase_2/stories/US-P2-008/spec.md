# US-P2-008: Create Ember/Ash Particle System

## Description
Create a GPU-instanced billboard particle system for floating embers and ash rising from lava - this adds atmospheric detail that makes the scene feel alive.

## The Core Concept / Why This Matters
The reference art shows glowing embers drifting up from the lava rivers. These particles:
1. **Add life** - Static lava looks dead; floating embers show heat and movement
2. **Depth cue** - Particles at different distances help perceive space
3. **Atmosphere** - Ash/embers reinforce the apocalyptic setting

The system uses GPU instancing for performance:
- One quad mesh (two triangles)
- Instance buffer with per-particle data (position, color, size, lifetime)
- Billboard rotation (particles always face camera)
- Additive blending (embers glow without occluding)

## Goal
Create `shaders/ember_particle.wgsl` and `engine/src/render/particles.rs` for a performant particle system.

## Files to Create/Modify
- `shaders/ember_particle.wgsl` (NEW) — Billboard particle shader with soft circle
- `engine/src/render/particles.rs` (NEW) — Particle system with spawning/updating
- `engine/src/render/mod.rs` — Add export

## Implementation Steps
1. Create `shaders/ember_particle.wgsl`:
   - Instance data: position, size, color, lifetime
   - Vertex shader: billboard the quad to face camera
   - Fragment shader: soft circle with additive blend

2. Create Particle struct in Rust:
   ```rust
   #[repr(C)]
   #[derive(Copy, Clone, Pod, Zeroable)]
   pub struct Particle {
       pub position: [f32; 3],
       pub lifetime: f32,      // 0.0 to 1.0 (fade out as it approaches 0)
       pub velocity: [f32; 3],
       pub size: f32,
       pub color: [f32; 4],
   }
   ```

3. Create ParticleSystem struct:
   - particles: Vec<Particle>
   - instance_buffer: wgpu::Buffer
   - pipeline with additive blend state
   - spawn_ember(pos) method
   - update(dt) method - move particles, kill dead ones

4. Implement spawning near lava positions

5. Add export in mod.rs

6. Run `cargo check`

## Code Patterns
Billboard vertex shader:
```wgsl
struct ParticleInstance {
    @location(5) position: vec3<f32>,
    @location(6) size: f32,
    @location(7) color: vec4<f32>,
    @location(8) lifetime: f32,
}

@vertex
fn vs_particle(
    @location(0) local_pos: vec2<f32>,  // Quad vertex: -0.5 to 0.5
    instance: ParticleInstance,
) -> VertexOutput {
    // Billboard: get right and up vectors from view matrix
    let right = vec3<f32>(uniforms.view[0][0], uniforms.view[1][0], uniforms.view[2][0]);
    let up = vec3<f32>(uniforms.view[0][1], uniforms.view[1][1], uniforms.view[2][1]);

    // Offset from particle center
    let world_pos = instance.position
        + right * local_pos.x * instance.size
        + up * local_pos.y * instance.size;

    // ...
}
```

Fragment with soft circle:
```wgsl
@fragment
fn fs_particle(in: VertexOutput) -> @location(0) vec4<f32> {
    let d = length(in.uv - vec2<f32>(0.5));
    let alpha = smoothstep(0.5, 0.2, d) * in.lifetime;

    // Emissive orange (HDR for bloom)
    let color = vec3<f32>(2.0, 0.6, 0.1);
    return vec4<f32>(color, alpha);
}
```

Additive blend state in Rust:
```rust
blend: Some(wgpu::BlendState {
    color: wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::SrcAlpha,
        dst_factor: wgpu::BlendFactor::One,  // Additive!
        operation: wgpu::BlendOperation::Add,
    },
    alpha: wgpu::BlendComponent::OVER,
}),
```

## Acceptance Criteria
- [ ] Embers spawn near lava (at specified positions)
- [ ] Particles float upward with slight random drift
- [ ] Particles fade out over lifetime (2-4 seconds)
- [ ] Additive blending creates glow effect
- [ ] Performance handles 500+ particles at 60fps
- [ ] `cargo check` passes

## Success Looks Like
When running the game:
- You see glowing orange dots rising from the lava
- Particles drift upward with slight horizontal wobble
- They gradually fade and disappear after a few seconds
- New particles continuously spawn, creating a stream of embers
- The effect adds significant atmosphere to the scene

## Dependencies
- Depends on: None (can be created independently)
