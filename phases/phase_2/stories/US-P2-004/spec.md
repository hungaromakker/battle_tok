# US-P2-004: Create Point Light System for Torches

## Description
Create a point light manager system that handles up to 16 dynamic lights with position, color, radius, and flickering - this enables realistic torch lighting on castle walls.

## The Core Concept / Why This Matters
Medieval castles have torches that cast warm, flickering light on nearby walls. This requires a point light system that:
1. Stores light positions, colors, radii in a GPU buffer
2. Updates each frame to apply flickering (sin-based intensity variation)
3. Exposes a bind group that material shaders can sample

Without this system, castle walls would only have directional sun lighting - torches are essential for the atmosphere. The 16-light limit ensures good performance while allowing enough torches for two castles.

## Goal
Create `engine/src/render/point_lights.rs` with a PointLightManager that can add/remove torches and provides a GPU buffer for shaders to sample.

## Files to Create/Modify
- `engine/src/render/point_lights.rs` (NEW) — Point light manager with buffer and bind group
- `engine/src/render/mod.rs` — Add `pub mod point_lights;` export

## Implementation Steps
1. Create `engine/src/render/point_lights.rs` with:
   - `PointLight` struct: position [f32;3], radius f32, color [f32;3], intensity f32
   - `PointLightManager` struct: lights Vec, buffer, bind_group_layout, bind_group
   - `new(device)` constructor creating buffer for 16 lights
   - `add_torch(pos, color, radius)` method
   - `remove_torch(index)` method
   - `update(queue, time)` method that applies flicker and writes to buffer
   - `bind_group()` getter for shaders

2. Add `pub mod point_lights;` to `engine/src/render/mod.rs`

3. Run `cargo check`

## Code Patterns
GPU buffer layout for point lights:
```rust
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PointLight {
    pub position: [f32; 3],
    pub radius: f32,         // 16 bytes
    pub color: [f32; 3],
    pub intensity: f32,      // 32 bytes total
}

// Buffer for up to 16 lights = 512 bytes
const MAX_POINT_LIGHTS: usize = 16;
```

Flicker implementation:
```rust
pub fn update(&mut self, queue: &wgpu::Queue, time: f32) {
    for (i, light) in self.lights.iter_mut().enumerate() {
        // Each torch has unique flicker phase
        let phase = i as f32 * 1.7;
        let flicker = 0.85 + 0.15 * (time * 12.0 + phase).sin();
        light.intensity = light.base_intensity * flicker;
    }
    queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&self.lights));
}
```

Bind group layout:
```rust
let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label: Some("Point Lights Bind Group Layout"),
    entries: &[
        wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        },
        // Binding 1: light_count uniform
    ],
});
```

## Acceptance Criteria
- [ ] Can add/remove torches dynamically via API
- [ ] Torches flicker with sin(time * frequency + phase) pattern
- [ ] Buffer updates correctly each frame without panics
- [ ] Bind group is accessible for material shaders
- [ ] `cargo check` passes

## Success Looks Like
After this story:
- You can create a PointLightManager and add torch lights at specific positions
- Calling `update()` each frame causes lights to flicker realistically
- The bind_group can be passed to shaders that need to sample point lights
- The system handles up to 16 lights efficiently

## Dependencies
- Depends on: None (can be created independently)
