# US-P4-016: Full Integration + Preview Shader + Pipeline Wiring

## Description
This is the **final integration story** for the Battle Tok asset editor. It creates `shaders/asset_preview.wgsl` for lit 3D preview rendering with an orbit camera, wires all five editor stages together in `mod.rs` so the full pipeline flows seamlessly (Draw2D -> Extrude -> Sculpt -> Color -> Save), and adds GPU buffer management for canvas lines (dynamic, rebuilt per frame) and 3D preview mesh (rebuilt only on mesh change). The preview shader implements simple directional + ambient lighting with vertex input of position, normal, and color.

**CRITICAL: This story integrates into `battle_editor.rs` (the separate binary, `cargo run --bin battle_editor`), NOT into `battle_arena.rs`. The game binary is NEVER modified. The existing status.json criteria "Editor input fully routed in battle_arena.rs" is INCORRECT -- it should read "Editor input fully routed in battle_editor.rs".**

## The Core Concept / Why This Matters
Individual features working in isolation are not a product. This story is the glue that makes the asset editor feel like a cohesive creative tool. Without integration, each stage is a disconnected experiment. With it, an artist can go from a blank canvas to a saved, variety-enabled game asset in a single session without leaving the editor.

The preview shader is what transforms raw vertex data into a visually appealing 3D view. Simple diffuse + ambient lighting provides enough visual information to judge surface shape, while the orbit camera lets artists inspect their work from any angle. Without this, the 3D stages (Extrude, Sculpt, Color) would be unusable -- artists need to see their mesh to work on it.

GPU buffer management is critical for performance. The 2D canvas generates new line geometry every frame (pen strokes change constantly), so its buffer must be dynamic. But the 3D mesh only changes when an extrude, sculpt, or paint operation is applied -- rebuilding it every frame would waste GPU bandwidth. The `mesh_buffer_dirty` flag ensures mesh buffers are only rebuilt when necessary. This distinction between dynamic and lazy-rebuild buffers is a fundamental GPU programming pattern.

Stage transitions must preserve the full editing state. If switching from Sculpt back to Draw2D erased the sculpted mesh, or switching from Color back to Extrude reset vertex colors, the editor would be useless. Every piece of state -- outlines, mesh geometry, vertex colors, camera position -- persists across all transitions.

## Goal
Create `shaders/asset_preview.wgsl` with directional + ambient lighting and orbit camera, wire all editor stages into a complete pipeline in `mod.rs`, implement GPU buffer management for canvas and mesh rendering, and ensure both `cargo build --bin battle_editor` and `cargo build --bin battle_arena` succeed.

## Files to Create/Modify
- **Create** `shaders/asset_preview.wgsl` -- vertex/fragment shader with CameraUniform, directional lighting, ambient term, vertex color pass-through
- **Modify** `src/game/asset_editor/mod.rs` -- Full pipeline wiring: stage transition logic preserving state, centralized `update()`/`render()` dispatch to active stage, GPU buffer creation and lazy-rebuild management, `mesh_buffer_dirty` flag
- **Modify** `src/bin/battle_editor.rs` -- Orbit camera (right-drag to rotate, scroll to zoom), wgpu render pipeline creation for `asset_preview.wgsl`, bind group setup for camera uniform, render pass with depth buffer, canvas/mesh/UI layered rendering

## Implementation Steps

1. Create `shaders/asset_preview.wgsl` with the preview shader:
   ```wgsl
   // Camera uniform buffer
   struct CameraUniform {
       view_proj: mat4x4<f32>,
       camera_pos: vec3<f32>,
       _padding: f32,
   }

   @group(0) @binding(0)
   var<uniform> camera: CameraUniform;

   // Vertex input: position, normal, color
   struct VertexInput {
       @location(0) position: vec3<f32>,
       @location(1) normal: vec3<f32>,
       @location(2) color: vec4<f32>,
   }

   struct VertexOutput {
       @builtin(position) clip_position: vec4<f32>,
       @location(0) world_normal: vec3<f32>,
       @location(1) color: vec4<f32>,
       @location(2) world_pos: vec3<f32>,
   }

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
       let n = normalize(in.world_normal);
       let diffuse = max(dot(n, light_dir), 0.0);
       let lit = ambient + diffuse * 0.7;
       return vec4<f32>(in.color.rgb * lit, in.color.a);
   }
   ```

2. Implement the `OrbitCamera` struct in `battle_editor.rs`:
   ```rust
   pub struct OrbitCamera {
       pub yaw: f32,       // Horizontal angle (radians)
       pub pitch: f32,     // Vertical angle (radians), clamped to avoid gimbal lock
       pub distance: f32,  // Distance from target (zoom)
       pub target: [f32; 3], // Camera look-at target (center of mesh)
   }

   impl OrbitCamera {
       pub fn new() -> Self {
           Self {
               yaw: 0.0,
               pitch: 0.4,      // Slightly above horizon
               distance: 5.0,   // Default zoom
               target: [0.0, 0.0, 0.0],
           }
       }

       /// Compute eye position from spherical coordinates around target.
       pub fn eye_position(&self) -> [f32; 3] {
           [
               self.target[0] + self.pitch.cos() * self.yaw.sin() * self.distance,
               self.target[1] + self.pitch.sin() * self.distance,
               self.target[2] + self.pitch.cos() * self.yaw.cos() * self.distance,
           ]
       }

       /// Compute the view-projection matrix for the camera uniform.
       pub fn view_proj(&self, aspect_ratio: f32) -> [[f32; 4]; 4] {
           let eye = self.eye_position();
           let view = look_at(eye, self.target, [0.0, 1.0, 0.0]);
           let proj = perspective(45.0_f32.to_radians(), aspect_ratio, 0.1, 100.0);
           mat4_mul(proj, view)
       }

       /// Handle mouse drag for orbit rotation.
       pub fn handle_drag(&mut self, dx: f32, dy: f32) {
           self.yaw += dx * 0.01;
           self.pitch = (self.pitch - dy * 0.01).clamp(-1.4, 1.4); // ~80 degrees
       }

       /// Handle scroll wheel for zoom.
       pub fn handle_scroll(&mut self, delta: f32) {
           self.distance = (self.distance - delta * 0.5).clamp(0.5, 50.0);
       }
   }
   ```

3. Wire the complete stage pipeline in `mod.rs` update dispatch:
   ```rust
   impl AssetEditor {
       pub fn update(&mut self, input: &InputState) {
           // Handle stage switching (keys 1-5)
           if let Some(stage) = EditorStage::from_key(input.key_pressed) {
               self.set_stage(stage);
           }

           // UI panels get first priority for input
           if let Some(action) = self.ui.handle_click(input.mouse_x, input.mouse_y,
                                                       self.stage, input.screen_w, input.screen_h) {
               self.apply_ui_action(action);
               return; // UI consumed the input
           }

           // Dispatch to active stage
           match self.stage {
               EditorStage::Draw2D => {
                   self.canvas.update(input);
                   // Canvas lines change every frame
               },
               EditorStage::Extrude => {
                   if self.extrude.update(input, &self.canvas.outlines) {
                       self.mesh_buffer_dirty = true;
                   }
               },
               EditorStage::Sculpt => {
                   if self.sculpt.update(input, &mut self.mesh) {
                       self.mesh_buffer_dirty = true;
                   }
               },
               EditorStage::Color => {
                   if self.paint.update(input, &mut self.mesh, &self.color_picker) {
                       self.mesh_buffer_dirty = true;
                   }
               },
               EditorStage::Save => {
                   self.asset_file.update(input, &self.mesh, &mut self.library);
               },
           }
       }
   }
   ```

4. Wire the complete stage pipeline in `mod.rs` render dispatch:
   ```rust
   impl AssetEditor {
       pub fn render(&self) -> EditorRenderData {
           let mut render = EditorRenderData::default();

           match self.stage {
               EditorStage::Draw2D => {
                   // 2D canvas: generate line quads from outlines
                   let (canvas_v, canvas_i) = self.canvas.render();
                   render.canvas_vertices = canvas_v;
                   render.canvas_indices = canvas_i;
                   render.use_canvas_pipeline = true;
               },
               EditorStage::Extrude | EditorStage::Sculpt |
               EditorStage::Color | EditorStage::Save => {
                   // 3D preview: mesh is already in vertex/index buffers
                   render.use_preview_pipeline = true;
                   render.mesh_buffer_dirty = self.mesh_buffer_dirty;
                   if self.mesh_buffer_dirty {
                       render.mesh_vertices = self.mesh.vertices.clone();
                       render.mesh_indices = self.mesh.indices.clone();
                   }
               },
           }

           // UI overlay renders on top of everything, in all stages
           let (palette_v, palette_i) = self.ui.generate_tool_palette(
               self.stage, self.active_tool, render.screen_h);
           let (props_v, props_i) = self.ui.generate_property_panel(
               self.stage, &self.property_params, render.screen_w, render.screen_h);
           render.ui_vertices.extend(palette_v);
           render.ui_indices.extend(palette_i);
           render.ui_vertices.extend(props_v);
           render.ui_indices.extend(props_i);

           // Color picker only in Stage 4
           if self.stage == EditorStage::Color {
               let (picker_v, picker_i) = self.ui.generate_color_picker(
                   &self.color_picker, render.screen_w - 200.0, 160.0);
               render.ui_vertices.extend(picker_v);
               render.ui_indices.extend(picker_i);
           }

           render
       }
   }
   ```

5. Implement stage transition logic that preserves state:
   ```rust
   impl AssetEditor {
       pub fn set_stage(&mut self, new_stage: EditorStage) {
           let old_stage = self.stage;
           self.stage = new_stage;

           // Transition-specific actions
           match (old_stage, new_stage) {
               // Draw2D -> Extrude: generate initial mesh from outlines
               (EditorStage::Draw2D, EditorStage::Extrude) => {
                   if self.mesh.vertices.is_empty() && !self.canvas.outlines.is_empty() {
                       self.mesh = self.extrude.generate_mesh(&self.canvas.outlines);
                       self.mesh_buffer_dirty = true;
                   }
               },
               _ => {} // All other transitions: just switch, preserve state
           }

           // Auto-enable color picker in Color stage
           self.ui.color_picker_visible = matches!(new_stage, EditorStage::Color);

           // Reset tool selection to first tool of new stage
           self.active_tool = 0;

           // Mark status bar update needed
           self.status_message = format!("Stage {}: {}", new_stage.number(), new_stage.name());
       }
   }
   ```

6. Implement GPU buffer management in `battle_editor.rs`:
   ```rust
   struct EditorBuffers {
       // Canvas (Stage 1): rebuilt every frame
       canvas_vertex_buf: wgpu::Buffer,
       canvas_index_buf: wgpu::Buffer,
       canvas_vertex_count: u32,

       // Mesh (Stages 2-5): only rebuilt when dirty
       mesh_vertex_buf: wgpu::Buffer,
       mesh_index_buf: wgpu::Buffer,
       mesh_index_count: u32,

       // UI overlay: rebuilt every frame
       ui_vertex_buf: wgpu::Buffer,
       ui_index_buf: wgpu::Buffer,
       ui_index_count: u32,

       // Camera uniform
       camera_uniform_buf: wgpu::Buffer,
       camera_bind_group: wgpu::BindGroup,

       // Depth buffer for 3D rendering
       depth_texture: wgpu::TextureView,
   }

   impl EditorBuffers {
       fn update_canvas(&mut self, queue: &wgpu::Queue, verts: &[Vertex], idxs: &[u32]) {
           // Canvas is rebuilt every frame (dynamic)
           queue.write_buffer(&self.canvas_vertex_buf, 0, bytemuck::cast_slice(verts));
           queue.write_buffer(&self.canvas_index_buf, 0, bytemuck::cast_slice(idxs));
           self.canvas_vertex_count = idxs.len() as u32;
       }

       fn update_mesh_if_dirty(
           &mut self, device: &wgpu::Device, queue: &wgpu::Queue,
           verts: &[Vertex], idxs: &[u32], dirty: bool,
       ) {
           if !dirty { return; }
           // Recreate buffers if size changed, otherwise write_buffer
           self.mesh_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
               label: Some("Mesh Vertices"),
               contents: bytemuck::cast_slice(verts),
               usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
           });
           self.mesh_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
               label: Some("Mesh Indices"),
               contents: bytemuck::cast_slice(idxs),
               usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
           });
           self.mesh_index_count = idxs.len() as u32;
       }

       fn update_camera(&self, queue: &wgpu::Queue, camera: &OrbitCamera, aspect: f32) {
           let vp = camera.view_proj(aspect);
           let eye = camera.eye_position();
           // Pack into uniform struct: view_proj (64 bytes) + camera_pos (12 bytes) + padding (4 bytes)
           let mut data = [0u8; 80];
           // ... write view_proj and camera_pos ...
           queue.write_buffer(&self.camera_uniform_buf, 0, &data);
       }
   }
   ```

7. Create the wgpu render pipeline for the preview shader:
   ```rust
   fn create_preview_pipeline(
       device: &wgpu::Device,
       surface_format: wgpu::TextureFormat,
   ) -> (wgpu::RenderPipeline, wgpu::BindGroupLayout) {
       let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
           label: Some("Asset Preview Shader"),
           source: wgpu::ShaderSource::Wgsl(
               include_str!("../../../shaders/asset_preview.wgsl").into(),
           ),
       });

       let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
           label: Some("Camera Bind Group Layout"),
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

       // Vertex buffer layout: position (3xf32) + normal (3xf32) + color (4xf32) = 40 bytes
       let vertex_layout = wgpu::VertexBufferLayout {
           array_stride: 40,
           step_mode: wgpu::VertexStepMode::Vertex,
           attributes: &[
               wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
               wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
               wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 24, shader_location: 2 },
           ],
       };

       let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
           label: Some("Asset Preview Pipeline"),
           // ... layout, vertex, fragment stages, depth stencil, multisample, etc.
       });

       (pipeline, bind_group_layout)
   }
   ```

8. Implement the complete render pass in `battle_editor.rs`:
   ```rust
   fn render_frame(
       gpu: &EditorGpu,
       buffers: &EditorBuffers,
       pipelines: &EditorPipelines,
       editor: &AssetEditor,
   ) {
       let output = gpu.surface.get_current_texture().unwrap();
       let view = output.texture.create_view(&Default::default());
       let mut encoder = gpu.device.create_command_encoder(&Default::default());

       // Clear to dark editor background
       let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
           color_attachments: &[Some(wgpu::RenderPassColorAttachment {
               view: &view,
               ops: wgpu::Operations {
                   load: wgpu::LoadOp::Clear(wgpu::Color {
                       r: 0.12, g: 0.12, b: 0.14, a: 1.0,
                   }),
                   store: wgpu::StoreOp::Store,
               },
               resolve_target: None,
           })],
           depth_stencil_attachment: Some(/* depth buffer */),
           ..Default::default()
       });

       match editor.stage {
           EditorStage::Draw2D => {
               // Canvas pipeline: 2D line quads, no depth test
               render_pass.set_pipeline(&pipelines.canvas);
               render_pass.set_vertex_buffer(0, buffers.canvas_vertex_buf.slice(..));
               render_pass.set_index_buffer(buffers.canvas_index_buf.slice(..), wgpu::IndexFormat::Uint32);
               render_pass.draw_indexed(0..buffers.canvas_vertex_count, 0, 0..1);
           },
           _ => {
               // Preview pipeline: 3D mesh with lighting, depth test enabled
               render_pass.set_pipeline(&pipelines.preview);
               render_pass.set_bind_group(0, &buffers.camera_bind_group, &[]);
               render_pass.set_vertex_buffer(0, buffers.mesh_vertex_buf.slice(..));
               render_pass.set_index_buffer(buffers.mesh_index_buf.slice(..), wgpu::IndexFormat::Uint32);
               render_pass.draw_indexed(0..buffers.mesh_index_count, 0, 0..1);
           },
       }

       // UI overlay pass: no depth test, renders on top
       render_pass.set_pipeline(&pipelines.ui);
       render_pass.set_vertex_buffer(0, buffers.ui_vertex_buf.slice(..));
       render_pass.set_index_buffer(buffers.ui_index_buf.slice(..), wgpu::IndexFormat::Uint32);
       render_pass.draw_indexed(0..buffers.ui_index_count, 0, 0..1);

       drop(render_pass);
       gpu.queue.submit(std::iter::once(encoder.finish()));
       output.present();
   }
   ```

## Code Patterns
The preview shader follows a standard lit vertex color pattern:
```wgsl
// Simple N dot L lighting with ambient floor
let ambient = 0.3;
let diffuse = max(dot(normalize(normal), light_dir), 0.0);
let lit = ambient + diffuse * 0.7;
return vec4<f32>(color.rgb * lit, color.a);
```

GPU buffer management uses the dirty-flag pattern for mesh buffers:
```rust
// Only rebuild mesh buffers when geometry actually changes
if self.mesh_buffer_dirty {
    buffers.update_mesh(&device, &queue, &mesh.vertices, &mesh.indices);
    self.mesh_buffer_dirty = false;
}
// Canvas and UI are always rebuilt (dynamic content)
buffers.update_canvas(&queue, &canvas_verts, &canvas_idxs);
buffers.update_ui(&queue, &ui_verts, &ui_idxs);
```

Orbit camera computes eye position from spherical coordinates:
```rust
let eye_x = target.x + pitch.cos() * yaw.sin() * distance;
let eye_y = target.y + pitch.sin() * distance;
let eye_z = target.z + pitch.cos() * yaw.cos() * distance;
```

## Acceptance Criteria
- [ ] `shaders/asset_preview.wgsl` exists with `vs_main` and `fs_main` entry points
- [ ] Fragment shader implements directional lighting: `ambient (0.3) + diffuse * 0.7` with light direction `normalize(vec3(0.3, 1.0, 0.5))`
- [ ] Vertex input accepts `position: vec3<f32>`, `normal: vec3<f32>`, `color: vec4<f32>`
- [ ] `CameraUniform` struct in shader has `view_proj: mat4x4<f32>` and `camera_pos: vec3<f32>`
- [ ] Orbit camera in `battle_editor.rs` supports right-drag to rotate and scroll to zoom
- [ ] `mod.rs` dispatches `update()` to the correct stage based on `self.stage`
- [ ] `mod.rs` dispatches `render()` to the correct pipeline (canvas for Stage 1, preview for Stages 2-5)
- [ ] Stage transitions (keys 1-5) preserve all editor state (outlines, mesh geometry, vertex colors)
- [ ] Draw2D-to-Extrude transition generates initial mesh from outlines if mesh is empty
- [ ] Canvas line buffer is rebuilt every frame (dynamic vertex buffer)
- [ ] Mesh vertex/index buffers are only rebuilt when `mesh_buffer_dirty` flag is set (on extrude, sculpt, paint operations)
- [ ] `mesh_buffer_dirty` is set to `true` after: extrude parameter change, sculpt operation, paint operation, mesh load
- [ ] Depth buffer is created for 3D rendering (no Z-fighting artifacts)
- [ ] UI panels (tool palette, property panel, color picker) render as overlay on top of all stages without depth test
- [ ] Full pipeline flows end-to-end: draw outlines -> extrude to 3D -> sculpt -> paint colors -> save `.btasset`
- [ ] Editor input is fully routed in `battle_editor.rs` (NOT `battle_arena.rs`)
- [ ] Normal game rendering is skipped when editor is active (editor has its own render loop)
- [ ] `battle_arena.rs` is **NOT modified** -- zero changes to the game binary
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo build --bin battle_editor` succeeds
- [ ] `cargo build --bin battle_arena` succeeds (untouched)

## Verification Commands
- `cmd`: `test -f /home/hungaromakker/battle_tok/shaders/asset_preview.wgsl && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `Preview shader file exists`
- `cmd`: `grep -c 'vs_main\|fs_main\|view_proj\|light_dir\|CameraUniform' /home/hungaromakker/battle_tok/shaders/asset_preview.wgsl`
  `expect_gt`: 0
  `description`: `Shader contains entry points, camera uniform, and lighting`
- `cmd`: `grep -c 'OrbitCamera\|yaw\|pitch\|distance\|view_proj' /home/hungaromakker/battle_tok/src/bin/battle_editor.rs`
  `expect_gt`: 0
  `description`: `Orbit camera is implemented in battle_editor`
- `cmd`: `grep -c 'mesh_buffer_dirty\|update_mesh\|update_canvas\|EditorRenderData' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `GPU buffer management is implemented in mod.rs`
- `cmd`: `grep -c 'render_canvas\|render_preview\|set_stage\|EditorStage' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `Stage dispatch is wired in mod.rs`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor compiles with full integration`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles with zero changes`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo build --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary builds successfully`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo build --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena binary builds successfully`
- `cmd`: `git diff --name-only -- src/bin/battle_arena.rs | wc -l`
  `expect_contains`: `0`
  `description`: `battle_arena.rs has zero modifications`

## Success Looks Like
The artist launches `cargo run --bin battle_editor` and sees the editor window with a dark background. They are in Stage 1 (Draw2D) with the tool palette on the left and property panel on the right. They draw a tree silhouette on the 2D canvas using freehand and line tools. They press 2 to switch to Extrude -- the 2D outline transforms into a 3D mesh visible with the lit preview shader. They orbit the camera with right-drag, zoom with scroll wheel. The mesh has soft directional lighting with an ambient floor. They press 3 for Sculpt and push/pull vertices to refine the shape. They press 4 for Color -- the HSV picker appears, and they paint vertex colors directly on the mesh. They press 5 for Save, enter a name and category, and save to `.btasset`. They go back to Stage 1 -- their outlines are still there. They go forward to Stage 4 -- their painted colors are preserved. The entire pipeline flows seamlessly. Meanwhile, `cargo build --bin battle_arena` succeeds without any changes to the game binary.

## Dependencies
- Depends on: US-P4-001 (editor skeleton), US-P4-002 (undo/redo), US-P4-003 (canvas drawing), US-P4-006 (pump extrusion), US-P4-008 (sculpt smooth/add/subtract), US-P4-009 (face/vertex/edge selection), US-P4-010 (vertex color painting), US-P4-011 (save/load .btasset), US-P4-015 (UI panels)

## Complexity
- Complexity: complex
- Min iterations: 2
