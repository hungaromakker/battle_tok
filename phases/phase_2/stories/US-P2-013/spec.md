# US-P2-013: Create Material System Coordinator

## Description
Create a unified material system that manages all render pipelines and provides a clean interface for switching materials per-object - this organizes the growing number of materials.

## The Core Concept / Why This Matters
We now have many materials:
- Terrain
- Castle Stone
- Wood Plank
- Chain Metal
- Flag
- Lava
- (potentially more)

Without organization, battle_arena.rs becomes messy with lots of individual pipeline management. A MaterialSystem:

1. **Centralizes creation** - All pipelines created in one place
2. **Shared uniforms** - Scene uniforms (view_proj, camera, time) shared across materials
3. **Easy switching** - `render_with_material(MaterialType::CastleStone, mesh)` instead of manual bind group switching
4. **Maintainability** - Adding new materials is straightforward

## Goal
Create `engine/src/render/material_system.rs` that manages all material pipelines with a unified interface.

## Files to Create/Modify
- `engine/src/render/material_system.rs` (NEW) — Material system coordinator
- `engine/src/render/mod.rs` — Add export

## Implementation Steps
1. Define MaterialType enum:
   ```rust
   #[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
   pub enum MaterialType {
       Terrain,
       CastleStone,
       WoodPlank,
       ChainMetal,
       Flag,
       Lava,
       EmberParticle,
   }
   ```

2. Create SharedSceneUniforms:
   ```rust
   #[repr(C)]
   #[derive(Copy, Clone, Pod, Zeroable)]
   pub struct SceneUniforms {
       view_proj: [[f32; 4]; 4],
       camera_pos: [f32; 3],
       time: f32,
       sun_dir: [f32; 3],
       fog_density: f32,
       fog_color: [f32; 3],
       ambient: f32,
   }
   ```

3. Create MaterialSystem struct:
   ```rust
   pub struct MaterialSystem {
       pipelines: HashMap<MaterialType, wgpu::RenderPipeline>,
       material_bind_groups: HashMap<MaterialType, wgpu::BindGroup>,
       scene_uniform_buffer: wgpu::Buffer,
       scene_bind_group: wgpu::BindGroup,
       scene_bind_group_layout: wgpu::BindGroupLayout,
   }
   ```

4. Implement new() that creates all pipelines
5. Implement update_scene_uniforms()
6. Implement render_with_material()

7. Add export in mod.rs

8. Run `cargo check`

## Code Patterns
Scene bind group layout (shared by all materials):
```rust
let scene_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    label: Some("Scene Bind Group Layout"),
    entries: &[wgpu::BindGroupLayoutEntry {
        binding: 0,
        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }],
});
```

Render method:
```rust
impl MaterialSystem {
    pub fn render_with_material<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        material: MaterialType,
        vertex_buffer: &'a wgpu::Buffer,
        index_buffer: &'a wgpu::Buffer,
        index_count: u32,
    ) {
        if let Some(pipeline) = self.pipelines.get(&material) {
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &self.scene_bind_group, &[]);

            if let Some(mat_bind_group) = self.material_bind_groups.get(&material) {
                pass.set_bind_group(1, mat_bind_group, &[]);
            }

            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..index_count, 0, 0..1);
        }
    }
}
```

## Acceptance Criteria
- [ ] MaterialSystem can create pipelines for all material types
- [ ] Scene uniforms shared correctly across all materials
- [ ] render_with_material() switches pipeline and bind groups correctly
- [ ] Adding a new material type is straightforward (add enum variant + pipeline)
- [ ] `cargo check` passes

## Success Looks Like
After this story:
- battle_arena.rs can use `material_system.render_with_material(MaterialType::CastleStone, ...)`
- Scene uniforms (camera, time, etc.) automatically passed to all materials
- The code is cleaner and more maintainable
- Adding a new material is just: add enum variant, add pipeline creation

## Dependencies
- Depends on: US-P2-003 (castle stone), US-P2-006 (bridge materials), US-P2-007 (flag)
- These materials should exist before we organize them
