# US-P4-016: Full Integration + Preview Shader + Pipeline Wiring

## Description
This is the **final integration story** for the Battle Tok asset editor. It wires all five stages (Draw2D, Extrude, Sculpt, Color, Save) into a seamless pipeline in `mod.rs`, creates a lit preview shader (`shaders/asset_preview.wgsl`) with an orbit camera for 3D visualization, and establishes proper GPU buffer management for both the 2D canvas overlay and 3D mesh preview. Stage transitions preserve all state. The editor runs as `battle_editor` — a **separate binary** from `battle_arena`. This story touches `battle_editor` code only; `battle_arena.rs` is **never modified**.

## The Core Concept / Why This Matters
Individual features working in isolation are not a product. This story is the glue that makes the asset editor feel like a cohesive creative tool. The preview shader transforms raw vertex data into a visually appealing lit 3D view with orbit camera controls, giving artists real-time feedback on how their asset will look in the game. GPU buffer management ensures smooth performance — canvas lines are rebuilt every frame (dynamic) while the 3D mesh buffer is only rebuilt when the mesh changes (on extrude, sculpt, or paint operations). Stage transitions must preserve the full editing state so artists can freely move between stages without losing work. After this story, an artist can go from a blank canvas to a saved, variety-enabled game asset in a single session.

## Goal
Wire all editor stages into a complete pipeline in `mod.rs`, create `shaders/asset_preview.wgsl` with a simple lit shader and orbit camera, implement GPU buffer management for canvas and mesh rendering, and ensure `cargo build --bin battle_editor` and `cargo build --bin battle_arena` both succeed.

## Files to Create/Modify
- **Create** `shaders/asset_preview.wgsl` — vertex/fragment shader with directional lighting and orbit camera uniforms
- **Modify** `src/game/asset_editor/mod.rs` — Full pipeline wiring: stage transition logic, centralized update/render dispatch, GPU buffer creation/updates, orbit camera integration
- **Modify** `src/bin/battle_editor.rs` — Orbit camera mouse input (drag to rotate, scroll to zoom), shader pipeline creation, bind group setup, render pass integration

## Implementation Steps
1. Create `shaders/asset_preview.wgsl`:
   ```wgsl
   struct CameraUniform {
       view_proj: mat4x4<f32>,
       camera_pos: vec3<f32>,
   };
   struct VertexInput {
       @location(0) position: vec3<f32>,
       @location(1) normal: vec3<f32>,
       @location(2) color: vec4<f32>,
   };
   struct VertexOutput {
       @builtin(position) clip_position: vec4<f32>,
       @location(0) world_normal: vec3<f32>,
       @location(1) color: vec4<f32>,
       @location(2) world_pos: vec3<f32>,
   };
   ```
   - Vertex shader: transform position by `view_proj`, pass through normal and color
   - Fragment shader: simple directional light (hardcoded sun direction `normalize(vec3(0.3, 1.0, 0.5))`) + ambient (0.3)
   - Compute `diffuse = max(dot(normal, light_dir), 0.0)`, final color = `vertex_color * (ambient + diffuse)`

2. Implement orbit camera in `battle_editor.rs`:
   ```rust
   pub struct OrbitCamera {
       pub yaw: f32,          // horizontal angle (radians)
       pub pitch: f32,        // vertical angle (clamped -89..89 degrees)
       pub distance: f32,     // distance from target
       pub target: [f32; 3],  // look-at point (typically mesh center)
   }
   ```
   - Right-mouse drag: adjust yaw and pitch
   - Scroll wheel: adjust distance (zoom in/out)
   - Compute `view_proj` matrix: `projection * look_at(eye, target, up)`
   - `eye` position derived from spherical coordinates: `target + [cos(pitch)*sin(yaw)*dist, sin(pitch)*dist, cos(pitch)*cos(yaw)*dist]`

3. Wire stage pipeline in `mod.rs`:
   - `update()` dispatches to the active stage's update logic:
     - Stage 1 → `canvas.update()` (2D drawing)
     - Stage 2 → `extrude.update()` (parameter adjustment, mesh generation)
     - Stage 3 → `sculpt.update()` (mesh deformation)
     - Stage 4 → `paint.update()` (vertex color painting)
     - Stage 5 → `asset_file` save/load UI
   - `render()` dispatches to the active stage's render logic:
     - Stage 1 → render 2D canvas lines (flat orthographic view)
     - Stages 2-4 → render 3D mesh preview with `asset_preview.wgsl` shader + orbit camera
     - Stage 5 → render 3D mesh preview + save UI overlay
   - UI panels render as overlay on top of all stages (tool palette, property panel, color picker)

4. Implement stage transition logic:
   ```rust
   pub fn set_stage(&mut self, new_stage: EditorStage) {
       match (&self.stage, &new_stage) {
           (EditorStage::Draw2D, EditorStage::Extrude) => {
               // Generate initial mesh from outlines
               self.rebuild_mesh_from_outlines();
               self.mesh_buffer_dirty = true;
           }
           (_, EditorStage::Color) => {
               // Ensure mesh is up-to-date before painting
               self.mesh_buffer_dirty = true;
           }
           _ => {}
       }
       self.stage = new_stage;
       self.property_panel.rebuild_for_stage(&self.stage);
       self.tool_palette.selected_tool = 0;
   }
   ```
   - State is NEVER cleared on transition — all outlines, mesh data, vertex colors persist
   - Moving backward (e.g., Stage 3 back to Stage 2) preserves sculpt modifications

5. Implement GPU buffer management:
   - **Canvas line buffer** (Stage 1): `wgpu::Buffer` with `COPY_DST` usage, rebuilt every frame from current outlines
     ```rust
     pub fn update_canvas_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
         let line_vertices = self.canvas.generate_line_vertices();
         if line_vertices.is_empty() { return; }
         // Recreate buffer if size changed, otherwise queue write
         queue.write_buffer(&self.canvas_vertex_buffer, 0, bytemuck::cast_slice(&line_vertices));
         self.canvas_vertex_count = line_vertices.len() as u32;
     }
     ```
   - **Mesh vertex/index buffers** (Stages 2-5): only rebuilt when `mesh_buffer_dirty` flag is set
     ```rust
     pub fn update_mesh_buffers(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
         if !self.mesh_buffer_dirty { return; }
         // Recreate buffers with current mesh vertices and indices
         self.mesh_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
             label: Some("Editor Mesh Vertices"),
             contents: bytemuck::cast_slice(&self.draft.mesh_vertices),
             usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
         });
         self.mesh_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
             label: Some("Editor Mesh Indices"),
             contents: bytemuck::cast_slice(&self.draft.mesh_indices),
             usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
         });
         self.mesh_index_count = self.draft.mesh_indices.len() as u32;
         self.mesh_buffer_dirty = false;
     }
     ```
   - Set `mesh_buffer_dirty = true` after: extrude parameter change, sculpt operation, paint operation, mesh load

6. Create wgpu render pipeline for `asset_preview.wgsl`:
   - Vertex buffer layout matching `Vertex { position: [f32;3], normal: [f32;3], color: [f32;4] }`
   - Depth buffer (depth24plus format) for proper 3D rendering
   - Camera uniform bind group with `view_proj` and `camera_pos`
   - Backface culling enabled, polygon fill mode

7. Implement the complete render pass in `battle_editor.rs`:
   ```rust
   // In the RedrawRequested handler:
   let output = surface.get_current_texture()?;
   let view = output.texture.create_view(&Default::default());
   let mut encoder = device.create_command_encoder(&Default::default());

   // Update buffers
   editor.update_canvas_buffer(&device, &queue);
   editor.update_mesh_buffers(&device, &queue);
   editor.update_camera_uniform(&queue);

   {
       let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
           color_attachments: &[Some(wgpu::RenderPassColorAttachment {
               view: &view,
               resolve_target: None,
               ops: wgpu::Operations {
                   load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.12, g: 0.12, b: 0.14, a: 1.0 }),
                   store: wgpu::StoreOp::Store,
               },
           })],
           depth_stencil_attachment: Some(/* depth buffer */),
           ..Default::default()
       });

       match editor.stage {
           EditorStage::Draw2D => {
               // Render canvas lines with 2D pipeline
               render_pass.set_pipeline(&canvas_pipeline);
               render_pass.set_vertex_buffer(0, editor.canvas_vertex_buffer.slice(..));
               render_pass.draw(0..editor.canvas_vertex_count, 0..1);
           }
           _ => {
               // Render 3D mesh with preview shader
               render_pass.set_pipeline(&preview_pipeline);
               render_pass.set_bind_group(0, &camera_bind_group, &[]);
               render_pass.set_vertex_buffer(0, editor.mesh_vertex_buffer.slice(..));
               render_pass.set_index_buffer(editor.mesh_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
               render_pass.draw_indexed(0..editor.mesh_index_count, 0, 0..1);
           }
       }
   }

   // Render UI overlay (tool palette, property panel, color picker)
   editor.render_ui_overlay(&device, &queue, &mut encoder, &view);

   queue.submit(std::iter::once(encoder.finish()));
   output.present();
   ```

8. Verify both binaries compile:
   - `cargo build --bin battle_editor` must succeed
   - `cargo build --bin battle_arena` must succeed (it is untouched)

## Code Patterns
```wgsl
// shaders/asset_preview.wgsl
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(in.position, 1.0);
    out.world_normal = in.normal;
    out.color = in.color;
    out.world_pos = in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let light_dir = normalize(vec3<f32>(0.3, 1.0, 0.5));
    let ambient = 0.3;
    let diffuse = max(dot(normalize(in.world_normal), light_dir), 0.0);
    let lit = ambient + diffuse * 0.7;
    return vec4<f32>(in.color.rgb * lit, in.color.a);
}
```

```rust
pub struct OrbitCamera {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub target: [f32; 3],
}

impl OrbitCamera {
    pub fn view_proj(&self, aspect_ratio: f32) -> [[f32; 4]; 4] {
        let eye = [
            self.target[0] + self.pitch.cos() * self.yaw.sin() * self.distance,
            self.target[1] + self.pitch.sin() * self.distance,
            self.target[2] + self.pitch.cos() * self.yaw.cos() * self.distance,
        ];
        let view = look_at(eye, self.target, [0.0, 1.0, 0.0]);
        let proj = perspective(45.0_f32.to_radians(), aspect_ratio, 0.1, 100.0);
        mat4_mul(proj, view)
    }
}
```

## Acceptance Criteria
- [ ] `shaders/asset_preview.wgsl` exists with vertex and fragment shaders
- [ ] Fragment shader implements directional lighting with ambient (0.3) and diffuse components
- [ ] Orbit camera supports right-drag to rotate and scroll to zoom
- [ ] `mod.rs` dispatches update/render to the correct stage
- [ ] Stage transitions (keys 1-5) preserve all editor state (outlines, mesh, vertex colors)
- [ ] Canvas line buffer is rebuilt every frame (dynamic)
- [ ] Mesh vertex/index buffers are only rebuilt when `mesh_buffer_dirty` is set (on extrude, sculpt, paint)
- [ ] 3D mesh renders with proper depth testing (no Z-fighting artifacts)
- [ ] UI panels (tool palette, property panel, color picker) render as overlay on top of all stages
- [ ] Full pipeline works end-to-end: draw outlines → extrude to 3D → sculpt → paint colors → save .btasset
- [ ] `battle_arena.rs` is **NOT modified** — zero changes to the game binary
- [ ] `cargo build --bin battle_editor` succeeds with 0 errors
- [ ] `cargo build --bin battle_arena` succeeds with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with full integration`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles with zero changes`
- `cmd`: `cargo build --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary builds successfully`
- `cmd`: `cargo build --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena binary builds successfully`
- `cmd`: `test -f shaders/asset_preview.wgsl && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `Preview shader file exists`
- `cmd`: `grep -c 'vs_main\|fs_main\|view_proj\|light_dir' shaders/asset_preview.wgsl`
  `expect_gt`: 0
  `description`: `Shader contains vertex/fragment entry points and lighting`
- `cmd`: `grep -c 'EditorStage::Draw2D\|EditorStage::Extrude\|EditorStage::Sculpt\|EditorStage::Color\|EditorStage::Save' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `All five editor stages are wired in mod.rs`
- `cmd`: `grep -c 'mesh_buffer_dirty\|update_mesh_buffers\|update_canvas_buffer' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `GPU buffer management is implemented`
- `cmd`: `grep -c 'OrbitCamera\|yaw\|pitch\|distance' src/bin/battle_editor.rs`
  `expect_gt`: 0
  `description`: `Orbit camera is implemented in battle_editor`
- `cmd`: `git diff --name-only -- src/bin/battle_arena.rs | wc -l`
  `expect_contains`: `0`
  `description`: `battle_arena.rs has zero modifications`

## Success Looks Like
The artist launches `cargo run --bin battle_editor` and sees the editor window with "Battle Tok - Asset Editor" in the title. They are in Stage 1 (Draw2D) with the tool palette on the left and property panel on the right. They draw a tree silhouette on the 2D canvas. They press 2 to switch to Extrude — the 2D outline transforms into a 3D mesh visible with the lit preview shader. They orbit the camera with right-drag, zoom with scroll wheel. The mesh has soft directional lighting. They press 3 for Sculpt and pull/push vertices to refine the shape. They press 4 for Color — the HSV picker appears, and they paint vertex colors directly on the mesh. They press 5 for Save, enter a name and category, and save to `.btasset`. They go back to Stage 1 — their outlines are still there. They go forward to Stage 4 — their painted colors are preserved. The entire pipeline flows seamlessly. Meanwhile, `cargo build --bin battle_arena` succeeds without any changes to the game binary.

## Dependencies
- Depends on: ALL previous stories (US-P4-001 through US-P4-015)

## Complexity
- Complexity: complex
- Min iterations: 2
