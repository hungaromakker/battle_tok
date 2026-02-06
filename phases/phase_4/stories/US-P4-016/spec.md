# US-P4-016: Integration + Preview Shader

## Description
Create `shaders/asset_preview.wgsl` with a simple lit shader using orbit camera uniforms (view_proj, camera_pos, light_dir, ambient). Wire all editor stages together in `battle_editor.rs` render loop: Stage 1 renders the 2D canvas, Stages 2-5 render 3D mesh preview with the asset preview shader. Manage GPU buffers: dynamic vertex buffer for canvas lines, vertex+index buffers for 3D preview mesh (rebuilt on change). Stage transitions preserve state. The asset editor is a SEPARATE BINARY (`cargo run --bin battle_editor`), NOT an F9 toggle. It shares the engine library but has its own winit window and event loop. `battle_arena.rs` is NEVER modified.

## The Core Concept / Why This Matters
This is the capstone story that connects all individual modules into a working whole. Each previous story created an isolated component -- canvas, extrude, sculpt, paint, undo, save, library, variety, placement, UI panels. This story wires them together: the editor state machine in `battle_editor.rs` routes input and rendering to the correct stage, GPU buffers are created and updated for canvas lines and 3D mesh preview, the preview shader renders the 3D asset with proper lighting, and stage transitions preserve accumulated state (outlines carry from Stage 1 to Stage 2, mesh carries through Stages 2-5). Without this integration, the individual parts do not form a usable tool. Critically, this all runs in the `battle_editor` binary, which is independent of the game's `battle_arena` binary.

## Goal
Create `shaders/asset_preview.wgsl`, complete the `AssetEditor` state machine in `mod.rs`, manage GPU buffers for canvas and 3D preview, and fully integrate everything into the `battle_editor.rs` render and input loops as a standalone editor binary.

## Files to Create/Modify
- Create `shaders/asset_preview.wgsl` -- Lit 3D preview shader with orbit camera
- Modify `src/game/asset_editor/mod.rs` -- Complete state machine, GPU buffer management, render dispatch
- Modify `src/bin/battle_editor.rs` -- Full editor integration: input routing, render passes, GPU resources, winit event loop

## Implementation Steps
1. Create `shaders/asset_preview.wgsl`:
   ```wgsl
   struct Uniforms {
       view_proj: mat4x4<f32>,
       camera_pos: vec3<f32>,
       _pad0: f32,
       light_dir: vec3<f32>,
       ambient: f32,
   }

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

   @group(0) @binding(0) var<uniform> u: Uniforms;

   @vertex fn vs_main(in: VertexInput) -> VertexOutput {
       var out: VertexOutput;
       out.clip_position = u.view_proj * vec4(in.position, 1.0);
       out.world_normal = in.normal;
       out.color = in.color;
       out.world_pos = in.position;
       return out;
   }

   @fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
       let n = normalize(in.world_normal);
       let l = normalize(-u.light_dir);
       let diffuse = max(dot(n, l), 0.0);

       // Simple hemisphere ambient (slightly brighter on top)
       let up_factor = dot(n, vec3(0.0, 1.0, 0.0)) * 0.5 + 0.5;
       let ambient_color = mix(vec3(0.08, 0.08, 0.12), vec3(0.15, 0.18, 0.25), up_factor);

       let lit = in.color.rgb * (ambient_color * u.ambient + vec3(diffuse) * 0.8);
       return vec4(lit, in.color.a);
   }
   ```

2. Define orbit camera for preview:
   ```rust
   pub struct OrbitCamera {
       pub target: [f32; 3],    // look-at point (center of mesh)
       pub distance: f32,       // distance from target
       pub yaw: f32,            // horizontal angle (radians)
       pub pitch: f32,          // vertical angle (radians), clamped
       pub fov_y: f32,          // field of view (radians)
   }

   impl OrbitCamera {
       pub fn view_proj(&self, aspect: f32) -> glam::Mat4 {
           let eye = glam::Vec3::new(
               self.target[0] + self.distance * self.yaw.cos() * self.pitch.cos(),
               self.target[1] + self.distance * self.pitch.sin(),
               self.target[2] + self.distance * self.yaw.sin() * self.pitch.cos(),
           );
           let target = glam::Vec3::from(self.target);
           let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);
           let proj = glam::Mat4::perspective_rh(self.fov_y, aspect, 0.1, 100.0);
           proj * view
       }

       pub fn rotate(&mut self, dx: f32, dy: f32) {
           self.yaw += dx * 0.01;
           self.pitch = (self.pitch + dy * 0.01).clamp(-1.4, 1.4);
       }

       pub fn zoom(&mut self, delta: f32) {
           self.distance = (self.distance - delta * 0.5).clamp(1.0, 50.0);
       }
   }
   ```

3. Define GPU resource management in `mod.rs`:
   ```rust
   pub struct EditorGPU {
       pub canvas_vb: Option<wgpu::Buffer>,
       pub canvas_ib: Option<wgpu::Buffer>,
       pub canvas_count: u32,
       pub preview_vb: Option<wgpu::Buffer>,
       pub preview_ib: Option<wgpu::Buffer>,
       pub preview_index_count: u32,
       pub preview_pipeline: Option<wgpu::RenderPipeline>,
       pub uniform_buffer: Option<wgpu::Buffer>,
       pub uniform_bind_group: Option<wgpu::BindGroup>,
       pub initialized: bool,
   }
   ```

4. Implement `initialize_gpu()`:
   ```rust
   impl EditorGPU {
       pub fn initialize(
           &mut self,
           device: &wgpu::Device,
           format: wgpu::TextureFormat,
       ) {
           // Load and compile asset_preview.wgsl
           let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
               label: Some("Asset Preview Shader"),
               source: wgpu::ShaderSource::Wgsl(
                   include_str!("../../../shaders/asset_preview.wgsl").into()
               ),
           });

           // Create uniform buffer (view_proj + camera_pos + light_dir + ambient)
           let uniform_size = std::mem::size_of::<PreviewUniforms>() as u64;
           self.uniform_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
               label: Some("Preview Uniforms"),
               size: uniform_size,
               usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
               mapped_at_creation: false,
           }));

           // Create bind group layout and bind group
           // Create render pipeline with vertex layout: pos(3f) + norm(3f) + color(4f)
           // Store pipeline and bind group

           self.initialized = true;
       }
   }
   ```

5. Implement buffer update functions:
   ```rust
   impl EditorGPU {
       pub fn update_canvas_buffers(
           &mut self,
           device: &wgpu::Device,
           vertices: &[Vertex],
           indices: &[u32],
       ) {
           self.canvas_vb = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
               label: Some("Canvas VB"),
               contents: bytemuck::cast_slice(vertices),
               usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
           }));
           self.canvas_ib = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
               label: Some("Canvas IB"),
               contents: bytemuck::cast_slice(indices),
               usage: wgpu::BufferUsages::INDEX,
           }));
           self.canvas_count = indices.len() as u32;
       }

       pub fn update_preview_buffers(
           &mut self,
           device: &wgpu::Device,
           vertices: &[Vertex],
           indices: &[u32],
       ) {
           self.preview_vb = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
               label: Some("Preview VB"),
               contents: bytemuck::cast_slice(vertices),
               usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
           }));
           self.preview_ib = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
               label: Some("Preview IB"),
               contents: bytemuck::cast_slice(indices),
               usage: wgpu::BufferUsages::INDEX,
           }));
           self.preview_index_count = indices.len() as u32;
       }
   }
   ```

6. Implement render dispatch in `AssetEditor`:
   ```rust
   impl AssetEditor {
       pub fn render(
           &mut self,
           encoder: &mut wgpu::CommandEncoder,
           view: &wgpu::TextureView,
           depth_view: &wgpu::TextureView,
           device: &wgpu::Device,
           queue: &wgpu::Queue,
           screen_size: [f32; 2],
       ) {
           match self.stage {
               EditorStage::Draw2D => {
                   // Render 2D canvas: grid + outlines
                   let (verts, idxs) = self.canvas.generate_mesh();
                   self.gpu.update_canvas_buffers(device, &verts, &idxs);
                   self.render_canvas(encoder, view);
               }
               _ => {
                   // Stages 2-5: render 3D mesh preview
                   let (verts, idxs) = self.draft.to_render_data();
                   self.gpu.update_preview_buffers(device, &verts, &idxs);

                   // Update orbit camera uniforms
                   let uniforms = PreviewUniforms {
                       view_proj: self.camera.view_proj(screen_size[0] / screen_size[1]),
                       camera_pos: self.camera.eye_position(),
                       light_dir: [0.3, -0.8, 0.5],
                       ambient: 0.4,
                   };
                   queue.write_buffer(self.gpu.uniform_buffer.as_ref().unwrap(), 0,
                       bytemuck::bytes_of(&uniforms));

                   self.render_preview(encoder, view, depth_view);
               }
           }

           // Render UI overlay (tool palette, property panel, color picker)
           let (ui_verts, ui_idxs) = self.ui.generate_all(self.stage, screen_size);
           // Render UI quads on top
       }
   }
   ```

7. Implement stage transitions preserving state:
   ```rust
   impl AssetEditor {
       pub fn switch_stage(&mut self, new_stage: EditorStage) {
           let old_stage = self.stage;
           self.stage = new_stage;

           match (old_stage, new_stage) {
               (EditorStage::Draw2D, EditorStage::Extrude) => {
                   // Auto-generate initial mesh from canvas outlines
                   self.draft = self.extrude.generate_mesh(&self.canvas.outlines);
               }
               (EditorStage::Extrude, EditorStage::Sculpt) => {
                   // Mesh is already in self.draft, ready for sculpting
                   self.draft.recompute_normals();
               }
               (EditorStage::Sculpt, EditorStage::Color) => {
                   // Mesh ready for painting, ensure normals are current
                   self.draft.recompute_normals();
               }
               (EditorStage::Color, EditorStage::Save) => {
                   // Mesh is complete, ready for save dialog
               }
               _ => {
                   // Backward transitions or same-stage: no special handling
               }
           }
       }
   }
   ```

8. Wire everything into `battle_editor.rs`:
   ```rust
   // battle_editor.rs -- standalone editor binary
   fn main() {
       // Create winit window
       let event_loop = EventLoop::new();
       let window = WindowBuilder::new()
           .with_title("Battle Tok Asset Editor")
           .with_inner_size(PhysicalSize::new(1280, 800))
           .build(&event_loop)
           .unwrap();

       // Initialize wgpu (surface, device, queue, format)
       // Initialize AssetEditor
       let mut editor = AssetEditor::new();

       event_loop.run(move |event, _, control_flow| {
           *control_flow = ControlFlow::Poll;

           match event {
               Event::WindowEvent { event, .. } => match event {
                   WindowEvent::KeyboardInput { input, .. } => {
                       // Route to editor keyboard handling
                       // 1-5: switch stages
                       // Ctrl+Z/Y: undo/redo
                       // Tool shortcuts per stage
                       // F10: library toggle
                   }
                   WindowEvent::CursorMoved { position, .. } => {
                       // Route to active stage mouse handling
                       // Right-click drag: orbit camera (Stages 2-5)
                   }
                   WindowEvent::MouseInput { button, state, .. } => {
                       // Route to active stage click handling
                       // Check UI panels first (tool palette, property panel)
                   }
                   WindowEvent::MouseWheel { delta, .. } => {
                       // Orbit camera zoom
                   }
                   WindowEvent::CloseRequested => {
                       *control_flow = ControlFlow::Exit;
                   }
                   _ => {}
               }
               Event::MainEventsCleared => {
                   window.request_redraw();
               }
               Event::RedrawRequested(_) => {
                   // Get surface texture
                   // Create command encoder
                   // Call editor.render(...)
                   // Submit and present
               }
               _ => {}
           }
       });
   }
   ```

## Acceptance Criteria
- [ ] `shaders/asset_preview.wgsl` exists with vertex and fragment shader functions
- [ ] Shader has Uniforms struct with view_proj, camera_pos, light_dir, ambient
- [ ] Vertex input accepts position (vec3), normal (vec3), color (vec4)
- [ ] Fragment shader computes diffuse lighting with ambient term and vertex color
- [ ] `battle_editor.rs` runs as standalone binary (`cargo run --bin battle_editor`)
- [ ] Stage 1 (Draw2D) renders 2D canvas with grid and outlines
- [ ] Stages 2-5 render 3D mesh preview with asset_preview shader and orbit camera
- [ ] GPU buffers are created lazily and updated when content changes
- [ ] Stage transitions preserve state (outlines carry to extrude, mesh carries through sculpt/color/save)
- [ ] Orbit camera supports right-click drag to rotate and scroll wheel to zoom
- [ ] All keyboard input is routed correctly (stage switching, tool shortcuts, undo/redo)
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check` passes with 0 errors
- [ ] `cargo build --bin battle_editor` succeeds

## Verification Commands
- `test -f /home/hungaromakker/battle_tok/shaders/asset_preview.wgsl && echo EXISTS` -- expected: EXISTS
- `grep -c 'view_proj\|vs_main\|fs_main\|Uniforms' /home/hungaromakker/battle_tok/shaders/asset_preview.wgsl` -- expected: > 0
- `grep -c 'render_canvas\|render_preview\|initialize\|OrbitCamera\|EditorGPU' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs` -- expected: > 0
- `grep -c 'battle_editor\|Asset Editor\|EventLoop\|RedrawRequested' /home/hungaromakker/battle_tok/src/bin/battle_editor.rs` -- expected: > 0
- `grep -v 'asset_editor' /home/hungaromakker/battle_tok/src/bin/battle_arena.rs | wc -l` -- expected: same as total lines (no asset_editor references added)
- `cd /home/hungaromakker/battle_tok && cargo check 2>&1; echo EXIT:$?` -- expected: EXIT:0
- `cd /home/hungaromakker/battle_tok && cargo build --bin battle_editor 2>&1; echo EXIT:$?` -- expected: EXIT:0

## Success Looks Like
The artist runs `cargo run --bin battle_editor` and a dedicated editor window opens. In Stage 1, they see a grid canvas and draw an outline of a tree. They press 2 to switch to Extrude -- the outline inflates into a 3D shape, rendered with proper lighting in the preview. They right-click-drag to orbit around the mesh, scroll to zoom. They press 3 for Sculpt and smooth out rough edges -- the preview updates in real-time. They press 4 for Color and paint green leaves and brown bark. They press 5 for Save, name it "Oak Tree", and save. The entire flow works end-to-end in the standalone editor, completely independent of the game binary. `battle_arena.rs` is untouched.

## Dependencies
- Depends on: US-P4-002, US-P4-003, US-P4-006, US-P4-008, US-P4-009, US-P4-015 (needs all stages and UI)

## Complexity
- Complexity: complex
- Min iterations: 2
