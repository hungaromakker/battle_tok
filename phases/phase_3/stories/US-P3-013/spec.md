# US-P3-013: Extract Render Passes to Methods

## Description
Break the monolithic `render()` method in `battle_arena.rs` into discrete render pass methods. Currently `render()` is ~500 lines with 4-5 render passes inline. Each pass should be its own method with a clear purpose.

## The Core Concept / Why This Matters
The render method currently does: (1) sky pass, (2) mesh pass (terrain + walls + blocks + trees), (3) dynamic pass (projectiles, debris), (4) SDF cannon pass, (5) UI pass, (6) fog post-process. Each of these has its own render pass descriptor, pipeline binding, and draw calls. Extracting them into methods means: each pass is independently readable, you can easily disable/enable passes, and the main render() becomes a clear pipeline.

## Goal
Break `render()` into 4-6 helper methods, making the render pipeline explicit and each pass self-contained.

## Files to Create/Modify
- Modify `src/bin/battle_arena.rs` — Split render() into methods

## Implementation Steps
1. Identify the current render passes in `render()`:
   - Pass 0: Apocalyptic sky (background, no depth)
   - Pass 1: Mesh rendering (terrain, walls, blocks, trees — with depth)
   - Pass 2: Dynamic mesh (projectiles, falling prisms, debris, meteors)
   - Pass 3: SDF cannon (separate pipeline)
   - Pass 4: Fog post-process
   - Pass 5: UI overlay (no depth)

2. Create helper methods:
   ```rust
   impl BattleArenaApp {
       fn render(&mut self) {
           let gpu = self.gpu.as_ref().unwrap();
           let output = gpu.surface.get_current_texture().unwrap();
           let view = output.texture.create_view(&Default::default());
           let mut encoder = gpu.device.create_command_encoder(&Default::default());

           self.render_sky(&mut encoder, &view);
           self.render_meshes(&mut encoder, &view, &gpu.depth_texture);
           self.render_dynamic(&mut encoder, &view, &gpu.depth_texture);
           self.render_sdf_cannon(&mut encoder, &view, &gpu.depth_texture);
           self.render_fog_post(&mut encoder, &view);
           self.render_ui(&mut encoder, &view);

           gpu.queue.submit([encoder.finish()]);
           output.present();
       }

       fn render_sky(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
           // Apocalyptic sky render
       }

       fn render_meshes(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, depth: &wgpu::TextureView) {
           // Static terrain + hex walls + blocks + trees
       }

       fn render_dynamic(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, depth: &wgpu::TextureView) {
           // Projectiles, falling prisms, debris, meteors
       }

       fn render_sdf_cannon(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, depth: &wgpu::TextureView) {
           // SDF cannon with separate pipeline
       }

       fn render_fog_post(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
           // Fog post-processing pass
       }

       fn render_ui(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
           // 2D UI overlay (toolbar, top bar, overlays)
       }
   }
   ```

3. Move the appropriate render pass code into each method, keeping the exact same wgpu operations.

4. Verify visual output is identical.

5. Run `cargo build --bin battle_arena`.

## Acceptance Criteria
- [ ] `render()` is ~20 lines calling helper methods
- [ ] Each helper method handles one render pass
- [ ] Same visual output as before (no rendering changes)
- [ ] Each method has a doc comment explaining the pass
- [ ] Game compiles and renders correctly
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
The render pipeline is readable at a glance: sky → meshes → dynamic → cannon → fog → ui. Each pass is its own method that can be understood independently. The main `render()` is a clean sequence of calls. ~500 lines of render code are organized into ~100-line methods.

## Dependencies
- Depends on: US-P3-012 (GpuResources)
