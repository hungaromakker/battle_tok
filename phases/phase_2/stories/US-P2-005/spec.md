# US-P2-005: Integrate Point Lights into Castle Stone Shader

## Description
Update the castle stone shader to sample point lights from the storage buffer and add their contribution to the final color - this makes torch lights actually illuminate castle walls.

## The Core Concept / Why This Matters
Story US-P2-003 creates the castle stone shader with placeholder torch bounce. Story US-P2-004 creates the point light system. This story CONNECTS them - making the shader read actual point light positions from the GPU buffer and compute realistic lighting.

Each point light contributes based on:
1. **Distance attenuation** - light falls off with distance squared (inverse square law)
2. **N·L factor** - surfaces facing the light receive more light
3. **Color and intensity** - each torch has its own color and flickering intensity

## Goal
Update `castle_stone.wgsl` to loop over point lights from a storage buffer and accumulate their lighting contribution.

## Files to Create/Modify
- `shaders/castle_stone.wgsl` — Add point light sampling loop
- `engine/src/render/castle_material.rs` — Update bind group to include point lights buffer

## Implementation Steps
1. Add PointLight struct definition to `castle_stone.wgsl`:
   ```wgsl
   struct PointLight {
       position: vec3<f32>,
       radius: f32,
       color: vec3<f32>,
       intensity: f32,
   }
   ```

2. Add storage buffer binding for point lights:
   ```wgsl
   @group(1) @binding(1) var<storage, read> point_lights: array<PointLight>;
   @group(1) @binding(2) var<uniform> light_count: u32;
   ```

3. In fragment shader, add loop to accumulate point light contribution:
   ```wgsl
   for (var i = 0u; i < light_count; i++) {
       let light = point_lights[i];
       let light_vec = light.position - world_pos;
       let dist = length(light_vec);
       let attenuation = 1.0 / (1.0 + dist * dist / (light.radius * light.radius));
       let ndl = max(dot(normal, normalize(light_vec)), 0.0);
       color += light.color * light.intensity * attenuation * ndl;
   }
   ```

4. Update `castle_material.rs` to:
   - Accept PointLightManager reference in constructor
   - Add point lights buffer binding to bind group
   - Add light_count uniform binding

5. Run `cargo check`

## Code Patterns
Point light attenuation formula (physically based):
```wgsl
// Inverse square law with radius-based falloff
let dist_sq = dot(light_vec, light_vec);
let radius_sq = light.radius * light.radius;
let attenuation = radius_sq / (dist_sq + radius_sq);  // Smooth falloff
```

Alternative simpler attenuation:
```wgsl
// Linear falloff within radius
let dist = length(light_vec);
let attenuation = max(0.0, 1.0 - dist / light.radius);
```

Bind group layout update in Rust:
```rust
// Add to bind group layout entries:
wgpu::BindGroupLayoutEntry {
    binding: 1,
    visibility: wgpu::ShaderStages::FRAGMENT,
    ty: wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Storage { read_only: true },
        has_dynamic_offset: false,
        min_binding_size: None,
    },
    count: None,
},
wgpu::BindGroupLayoutEntry {
    binding: 2,
    visibility: wgpu::ShaderStages::FRAGMENT,
    ty: wgpu::BindingType::Buffer {
        ty: wgpu::BufferBindingType::Uniform,
        has_dynamic_offset: false,
        min_binding_size: None,
    },
    count: None,
},
```

## Acceptance Criteria
- [ ] Castle stone is lit by nearby point lights (visible warm glow on walls)
- [ ] Light attenuates with distance (walls further from torch are dimmer)
- [ ] Multiple torches combine correctly (overlapping lights add up)
- [ ] Shader compiles without WGSL errors
- [ ] `cargo check` passes

## Success Looks Like
When you place torches near castle walls:
- Walls facing the torch glow warm orange
- The glow fades with distance - walls far from torch are dark
- Placing multiple torches creates pools of overlapping light
- Torches flickering causes the wall lighting to dance

## Dependencies
- Depends on: US-P2-003 (castle stone shader exists)
- Depends on: US-P2-004 (point light system exists)
