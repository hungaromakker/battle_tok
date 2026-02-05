# US-P3-012: Extract GPU Resource Management to GpuResources Struct

## Description
Group all GPU-related fields (device, queue, surface, pipelines, buffers, textures) from `BattleArenaApp` into a `GpuResources` struct defined in `battle_arena.rs`. This is a local refactoring — the struct stays in the binary, not the library — but dramatically cleans up the field list and groups related operations.

## The Core Concept / Why This Matters
After Story 11, `BattleArenaApp` still has ~20 GPU fields (device, queue, surface, 3 pipelines, ~10 buffers, depth texture, etc.). Grouping these into `GpuResources` means: (1) the App struct becomes readable, (2) buffer operations are co-located, (3) resize logic is in one place, (4) it's clearer what's GPU state vs game state.

## Goal
Create a `GpuResources` struct within `battle_arena.rs` that owns all GPU fields and provides buffer update + resize methods.

## Files to Create/Modify
- Modify `src/bin/battle_arena.rs` — Add GpuResources struct, refactor fields

## Implementation Steps
1. Define `GpuResources` struct in `battle_arena.rs`:
   ```rust
   struct GpuResources {
       device: wgpu::Device,
       queue: wgpu::Queue,
       surface: wgpu::Surface<'static>,
       surface_config: wgpu::SurfaceConfiguration,

       // Pipelines
       main_pipeline: wgpu::RenderPipeline,
       sdf_cannon_pipeline: wgpu::RenderPipeline,
       ui_pipeline: wgpu::RenderPipeline,

       // Main uniform
       uniform_buffer: wgpu::Buffer,
       uniform_bind_group: wgpu::BindGroup,

       // Static mesh (terrain)
       static_vertex_buffer: wgpu::Buffer,
       static_index_buffer: wgpu::Buffer,
       static_index_count: u32,

       // Dynamic mesh (projectiles, debris)
       dynamic_vertex_buffer: wgpu::Buffer,
       dynamic_index_buffer: wgpu::Buffer,
       dynamic_index_count: u32,

       // Hex walls
       hex_wall_vertex_buffer: wgpu::Buffer,
       hex_wall_index_buffer: wgpu::Buffer,
       hex_wall_index_count: u32,

       // Building blocks
       block_vertex_buffer: wgpu::Buffer,
       block_index_buffer: wgpu::Buffer,
       block_index_count: u32,

       // Trees
       tree_vertex_buffer: wgpu::Buffer,
       tree_index_buffer: wgpu::Buffer,
       tree_index_count: u32,

       // SDF cannon
       sdf_cannon_uniform_buffer: wgpu::Buffer,
       sdf_cannon_data_buffer: wgpu::Buffer,
       sdf_cannon_bind_group: wgpu::BindGroup,

       // UI
       ui_uniform_buffer: wgpu::Buffer,
       ui_bind_group: wgpu::BindGroup,

       // Depth
       depth_texture: wgpu::TextureView,
       depth_texture_raw: wgpu::Texture,
   }
   ```

2. Add methods to `GpuResources`:
   ```rust
   impl GpuResources {
       fn resize(&mut self, new_size: PhysicalSize<u32>) { ... }
       fn update_static_buffers(&mut self, vertices: &[Vertex], indices: &[u32]) { ... }
       fn update_dynamic_buffers(&mut self, vertices: &[Vertex], indices: &[u32]) { ... }
       fn update_hex_wall_buffers(&mut self, vertices: &[Vertex], indices: &[u32]) { ... }
       fn update_block_buffers(&mut self, vertices: &[Vertex], indices: &[u32]) { ... }
       fn update_tree_buffers(&mut self, vertices: &[Vertex], indices: &[u32]) { ... }
   }
   ```

3. Update `BattleArenaApp`:
   ```rust
   struct BattleArenaApp {
       window: Option<Arc<Window>>,
       gpu: Option<GpuResources>,  // Replaces ~20 fields
       scene: Option<BattleScene>,
       // ... input and timing
   }
   ```

4. Update all code that references `self.device` → `self.gpu.as_ref().unwrap().device`, etc.
   Or better: use `let gpu = self.gpu.as_mut().unwrap();` at the start of methods.

5. Run `cargo build --bin battle_arena`.

## Acceptance Criteria
- [ ] All GPU fields moved into `GpuResources`
- [ ] `BattleArenaApp` has `gpu: Option<GpuResources>` instead of ~20 fields
- [ ] `resize()` method works on `GpuResources`
- [ ] Buffer update methods work correctly
- [ ] Game compiles and renders correctly
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
`BattleArenaApp` struct definition fits on one screen. GPU operations go through `self.gpu` methods. The field list is: window, gpu, scene, input, timing — clean and obvious.

## Dependencies
- Depends on: US-P3-011 (refactored battle_arena.rs)
