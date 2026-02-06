# US-P4-005: Background Image Loading for Tracing

## Description
Add the ability to load a reference image (PNG or JPG) as a semi-transparent background layer on the 2D drawing canvas (Stage 1 — Draw2D) of the Asset Editor. The image is rendered as a textured quad behind the grid and outlines at configurable opacity, scale, and position, allowing artists to trace over concept art, photographs, or previous iterations. This requires a new WGSL shader (`shaders/canvas_2d.wgsl`) for textured quad rendering with alpha blending, a new Rust module (`src/game/asset_editor/image_trace.rs`) for image loading and GPU texture management, and a `Cargo.toml` change to enable the `jpeg` feature on the `image` crate.

## The Core Concept / Why This Matters
Artists rarely draw from scratch — they work from references. A game artist creating a tree asset will load a photo of a real tree or a concept sketch as a background, then trace the silhouette on top of it. Without this feature, the artist must constantly switch between a reference viewer and the editor window, eyeballing proportions and losing context. The semi-transparent background solves this: the reference image sits behind the grid at 30% opacity by default, visible enough to trace but transparent enough that grid lines and drawn outlines remain clearly readable. The opacity, scale, and position controls let the artist fit any reference image to the canvas. This is a standard feature in every 2D drawing tool (Photoshop, Krita, Inkscape) and its absence would make the asset editor feel incomplete.

## Goal
Create `src/game/asset_editor/image_trace.rs` for loading PNG/JPG images into GPU textures, create `shaders/canvas_2d.wgsl` for rendering textured quads with adjustable opacity, integrate the background image into the `Canvas2D` rendering pipeline (drawn first, behind grid and outlines), and wire up keyboard/mouse controls for loading, opacity, scale, and position adjustment.

## Files to Create/Modify
- **Create** `src/game/asset_editor/image_trace.rs` — `ImageTrace` struct: loads PNG/JPG via the `image` crate, creates wgpu texture + texture view + sampler + bind group, manages opacity/scale/position state, provides `render()` method to draw textured quad
- **Create** `shaders/canvas_2d.wgsl` — Vertex/fragment shader for rendering a textured quad with a uniform opacity multiplier; uses its own bind group layout (uniform buffer + texture_2d + sampler) separate from the main game uniforms
- **Modify** `Cargo.toml` — Add `"jpeg"` to the `image` crate features: `features = ["png", "jpeg"]`
- **Modify** `src/game/asset_editor/canvas_2d.rs` — Add `image_trace: Option<ImageTrace>` field to `Canvas2D`, call `image_trace.render()` before grid and outline rendering, forward Ctrl-modified input events to image trace controls
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod image_trace;`
- **Modify** `src/bin/battle_editor.rs` — Handle Ctrl+I to trigger image load (accept path via stdin or command-line argument since `rfd` is not available), Ctrl+Scroll for image scale, Ctrl+MiddleMouse for image position, Ctrl+H for visibility toggle

## Implementation Steps

1. **Update `Cargo.toml` to enable JPEG support:**
   The `image` crate currently only has the `"png"` feature. JPG loading requires adding `"jpeg"`:
   ```toml
   image = { version = "0.25", default-features = false, features = ["png", "jpeg"] }
   ```
   This is the only dependency change. No new crates are added.

2. **Create the `ImageTrace` struct** in `src/game/asset_editor/image_trace.rs`:
   ```rust
   pub struct ImageTrace {
       // GPU resources
       pub texture: wgpu::Texture,
       pub texture_view: wgpu::TextureView,
       pub sampler: wgpu::Sampler,
       pub bind_group: wgpu::BindGroup,
       pub uniform_buffer: wgpu::Buffer,
       pub vertex_buffer: wgpu::Buffer,
       pub index_buffer: wgpu::Buffer,
       pub pipeline: wgpu::RenderPipeline,
       // State
       pub opacity: f32,        // 0.0 to 1.0, default 0.3
       pub scale: f32,          // default 1.0
       pub position: [f32; 2],  // canvas-space offset, default [0.0, 0.0]
       pub visible: bool,       // toggle with Ctrl+H
       // Metadata
       image_width: u32,
       image_height: u32,
   }
   ```

3. **Implement `ImageTrace::load()`** to create the GPU texture from a file:
   ```rust
   use image::GenericImageView;

   impl ImageTrace {
       pub fn load(
           path: &str,
           device: &wgpu::Device,
           queue: &wgpu::Queue,
           surface_format: wgpu::TextureFormat,
       ) -> Result<Self, String> {
           let img = image::open(path).map_err(|e| format!("Failed to load image: {e}"))?;
           let rgba = img.to_rgba8();
           let (width, height) = img.dimensions();

           // Create GPU texture
           let texture = device.create_texture(&wgpu::TextureDescriptor {
               label: Some("trace_background"),
               size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
               mip_level_count: 1,
               sample_count: 1,
               dimension: wgpu::TextureDimension::D2,
               format: wgpu::TextureFormat::Rgba8UnormSrgb,
               usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
               view_formats: &[],
           });

           // Upload pixel data
           queue.write_texture(
               wgpu::TexelCopyTextureInfo {
                   texture: &texture,
                   mip_level: 0,
                   origin: wgpu::Origin3d::ZERO,
                   aspect: wgpu::TextureAspect::All,
               },
               &rgba,
               wgpu::TexelCopyBufferLayout {
                   offset: 0,
                   bytes_per_row: Some(4 * width),
                   rows_per_image: Some(height),
               },
               wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
           );

           let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

           let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
               label: Some("trace_sampler"),
               address_mode_u: wgpu::AddressMode::ClampToEdge,
               address_mode_v: wgpu::AddressMode::ClampToEdge,
               mag_filter: wgpu::FilterMode::Linear,
               min_filter: wgpu::FilterMode::Linear,
               ..Default::default()
           });

           // ... create uniform buffer, bind group layout, bind group, pipeline, vertex/index buffers
           // (see steps 4-6 below)

           Ok(Self {
               texture, texture_view, sampler, bind_group, uniform_buffer,
               vertex_buffer, index_buffer, pipeline,
               opacity: 0.3,
               scale: 1.0,
               position: [0.0, 0.0],
               visible: true,
               image_width: width,
               image_height: height,
           })
       }
   }
   ```

4. **Create the uniform buffer** for the canvas_2d shader:
   ```rust
   #[repr(C)]
   #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
   struct Canvas2DUniforms {
       view_projection: [[f32; 4]; 4],  // 64 bytes — orthographic VP matrix
       opacity: f32,                     // 4 bytes
       _pad0: f32,                       // 4 bytes
       _pad1: f32,                       // 4 bytes
       _pad2: f32,                       // 4 bytes — total 80 bytes (16-byte aligned)
   }
   ```
   Create with `BufferUsages::UNIFORM | BufferUsages::COPY_DST`. Update each frame via `queue.write_buffer()` with current opacity and the canvas orthographic view-projection matrix.

5. **Create the bind group layout and bind group:**
   ```rust
   let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
       label: Some("canvas_2d_bind_group_layout"),
       entries: &[
           // @binding(0): uniform buffer (vertex + fragment)
           wgpu::BindGroupLayoutEntry {
               binding: 0,
               visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
               ty: wgpu::BindingType::Buffer {
                   ty: wgpu::BufferBindingType::Uniform,
                   has_dynamic_offset: false,
                   min_binding_size: None,
               },
               count: None,
           },
           // @binding(1): texture
           wgpu::BindGroupLayoutEntry {
               binding: 1,
               visibility: wgpu::ShaderStages::FRAGMENT,
               ty: wgpu::BindingType::Texture {
                   sample_type: wgpu::TextureSampleType::Float { filterable: true },
                   view_dimension: wgpu::TextureViewDimension::D2,
                   multisampled: false,
               },
               count: None,
           },
           // @binding(2): sampler
           wgpu::BindGroupLayoutEntry {
               binding: 2,
               visibility: wgpu::ShaderStages::FRAGMENT,
               ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
               count: None,
           },
       ],
   });

   let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
       label: Some("canvas_2d_bind_group"),
       layout: &bind_group_layout,
       entries: &[
           wgpu::BindGroupEntry { binding: 0, resource: uniform_buffer.as_entire_binding() },
           wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&texture_view) },
           wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&sampler) },
       ],
   });
   ```

6. **Create `shaders/canvas_2d.wgsl`:**
   This shader renders a textured quad with opacity. It uses its own vertex format with position (vec3) and UV (vec2), separate from the game's `Vertex` type which has position + normal + color but no UVs.
   ```wgsl
   // Canvas 2D Background Image Shader
   // Renders a textured quad with adjustable opacity for image tracing.
   // Alpha blending must be enabled on the pipeline.

   struct Uniforms {
       view_projection: mat4x4<f32>,
       opacity: f32,
       _pad0: f32,
       _pad1: f32,
       _pad2: f32,
   };

   @group(0) @binding(0) var<uniform> uniforms: Uniforms;
   @group(0) @binding(1) var t_texture: texture_2d<f32>;
   @group(0) @binding(2) var t_sampler: sampler;

   struct VertexInput {
       @location(0) position: vec3<f32>,
       @location(1) uv: vec2<f32>,
   };

   struct VertexOutput {
       @builtin(position) clip_position: vec4<f32>,
       @location(0) uv: vec2<f32>,
   };

   @vertex
   fn vs_main(in: VertexInput) -> VertexOutput {
       var out: VertexOutput;
       out.clip_position = uniforms.view_projection * vec4<f32>(in.position, 1.0);
       out.uv = in.uv;
       return out;
   }

   @fragment
   fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
       let color = textureSample(t_texture, t_sampler, in.uv);
       return vec4<f32>(color.rgb, color.a * uniforms.opacity);
   }
   ```

7. **Create the render pipeline** for the textured quad:
   ```rust
   // Vertex layout: position (vec3<f32>) + uv (vec2<f32>) = 20 bytes per vertex
   #[repr(C)]
   #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
   struct Canvas2DVertex {
       position: [f32; 3],
       uv: [f32; 2],
   }

   let vertex_buffers = [wgpu::VertexBufferLayout {
       array_stride: std::mem::size_of::<Canvas2DVertex>() as u64, // 20 bytes
       step_mode: wgpu::VertexStepMode::Vertex,
       attributes: &[
           wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
           wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 12, shader_location: 1 },
       ],
   }];
   ```
   Pipeline must enable alpha blending:
   ```rust
   fragment: Some(wgpu::FragmentState {
       // ...
       targets: &[Some(wgpu::ColorTargetState {
           format: surface_format,
           blend: Some(wgpu::BlendState {
               color: wgpu::BlendComponent {
                   src_factor: wgpu::BlendFactor::SrcAlpha,
                   dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                   operation: wgpu::BlendOperation::Add,
               },
               alpha: wgpu::BlendComponent {
                   src_factor: wgpu::BlendFactor::One,
                   dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                   operation: wgpu::BlendOperation::Add,
               },
           }),
           write_mask: wgpu::ColorWrites::ALL,
       })],
   }),
   ```
   No depth testing (2D canvas layer). Cull mode: None (quad is always visible).

8. **Implement `ImageTrace::render()`:**
   Generate a quad in canvas coordinates centered at `self.position`, sized to `(image_width * scale, image_height * scale)` in canvas units (where 1 unit = 1 grid cell). The aspect ratio of the image is always preserved.
   ```rust
   pub fn render(
       &self,
       encoder: &mut wgpu::CommandEncoder,
       view: &wgpu::TextureView,
       queue: &wgpu::Queue,
       canvas_vp: glam::Mat4,
   ) {
       if !self.visible { return; }

       // Update uniform buffer with current opacity + view-projection
       let uniforms = Canvas2DUniforms {
           view_projection: canvas_vp.to_cols_array_2d(),
           opacity: self.opacity,
           _pad0: 0.0, _pad1: 0.0, _pad2: 0.0,
       };
       queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

       // Compute quad corners in canvas coordinates
       let half_w = self.image_width as f32 * self.scale * 0.5;
       let half_h = self.image_height as f32 * self.scale * 0.5;
       let cx = self.position[0];
       let cy = self.position[1];

       // Scale factor: image pixels to canvas units (configurable, default 0.01)
       let px_to_canvas = 0.01 * self.scale;
       let hw = self.image_width as f32 * px_to_canvas * 0.5;
       let hh = self.image_height as f32 * px_to_canvas * 0.5;

       let vertices = [
           Canvas2DVertex { position: [cx - hw, cy + hh, 0.0], uv: [0.0, 0.0] }, // top-left
           Canvas2DVertex { position: [cx + hw, cy + hh, 0.0], uv: [1.0, 0.0] }, // top-right
           Canvas2DVertex { position: [cx + hw, cy - hh, 0.0], uv: [1.0, 1.0] }, // bottom-right
           Canvas2DVertex { position: [cx - hw, cy - hh, 0.0], uv: [0.0, 1.0] }, // bottom-left
       ];
       let indices: [u32; 6] = [0, 1, 2, 0, 2, 3];

       queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
       queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));

       let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
           label: Some("canvas_2d_background"),
           color_attachments: &[Some(wgpu::RenderPassColorAttachment {
               view,
               resolve_target: None,
               ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
           })],
           depth_stencil_attachment: None,
           ..Default::default()
       });
       pass.set_pipeline(&self.pipeline);
       pass.set_bind_group(0, &self.bind_group, &[]);
       pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
       pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
       pass.draw_indexed(0..6, 0, 0..1);
   }
   ```

9. **Implement input controls:**
   - **Ctrl+I** — Load image: since the `rfd` file dialog crate is not in `Cargo.toml` and we do not add new dependencies, accept the image path via a simple text input approach. When Ctrl+I is pressed, print a prompt to stdout and read a line from stdin on a background thread. Alternatively, accept a `--trace-image <path>` command-line argument on startup.
     ```rust
     // In battle_editor.rs, on Ctrl+I:
     println!("[Editor] Enter image path for tracing:");
     // Read from stdin in a non-blocking way or use a simple hardcoded path input field
     ```
   - **Opacity slider** — Use `UISlider` from `src/game/ui/slider.rs` positioned in the right property panel. Range 0.0 to 1.0, step 0.05. Maps `UISlider.value` (always 0.0-1.0) directly to `ImageTrace.opacity`:
     ```rust
     let opacity_slider = UISlider::new("Opacity", panel_x, panel_y, image_trace.opacity, [0.4, 0.7, 1.0, 1.0]);
     if opacity_slider.contains(mouse_x, mouse_y) && mouse_pressed {
         image_trace.opacity = opacity_slider.value_from_x(mouse_x);
     }
     ```
   - **Ctrl+Scroll** — Adjust image scale: multiplicative factor, clamped to 0.1..10.0:
     ```rust
     if ctrl_held && scroll_delta != 0.0 {
         image_trace.scale *= if scroll_delta > 0.0 { 1.1 } else { 1.0 / 1.1 };
         image_trace.scale = image_trace.scale.clamp(0.1, 10.0);
     }
     ```
   - **Ctrl+MiddleMouse drag** — Adjust image position offset in canvas coordinates:
     ```rust
     if ctrl_held && middle_mouse_dragging {
         image_trace.position[0] += mouse_delta_x / canvas_zoom;
         image_trace.position[1] += mouse_delta_y / canvas_zoom;
     }
     ```
   - **Ctrl+H** — Toggle visibility:
     ```rust
     if ctrl_held && key_just_pressed(KeyCode::KeyH) {
         image_trace.visible = !image_trace.visible;
     }
     ```

10. **Integrate into `Canvas2D`:**
    Add an `image_trace: Option<ImageTrace>` field to the `Canvas2D` struct. In the `Canvas2D::render()` method, if the image trace is `Some` and `visible`, render it FIRST before the grid and outlines. This ensures the render order is:
    1. Background image (semi-transparent textured quad)
    2. Grid (thin colored quads via `add_quad()`)
    3. Outlines (thin white quads via `add_quad()`)

    Alpha blending on the background image pipeline means the grid and outlines (drawn later) appear on top.

11. **Handle aspect ratio preservation:**
    The quad dimensions are derived from the original image dimensions, so the aspect ratio is always preserved:
    ```rust
    let aspect = image_width as f32 / image_height as f32;
    // Quad width and height in canvas units always maintain this ratio
    ```
    The scale factor applies uniformly to both width and height.

## Code Patterns

Loading a texture from the `image` crate (follows the established pattern in `engine/src/render/cubemap_skybox.rs`):
```rust
use image::GenericImageView;

pub fn load_image_texture(
    path: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> Result<(wgpu::Texture, wgpu::TextureView, u32, u32), String> {
    let img = image::open(path).map_err(|e| format!("Failed to load image: {e}"))?;
    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("trace_image"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Uses wgpu 27 API: TexelCopyTextureInfo and TexelCopyBufferLayout
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &rgba,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * width),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    Ok((texture, view, width, height))
}
```

Uniform buffer pattern (16-byte aligned, matches existing shader conventions):
```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Canvas2DUniforms {
    view_projection: [[f32; 4]; 4],  // 64 bytes
    opacity: f32,                     // 4 bytes
    _pad0: f32,                       // 4 bytes
    _pad1: f32,                       // 4 bytes
    _pad2: f32,                       // 4 bytes — row total 16, grand total 80
}

// Compile-time size assertion (follows project pattern from cubemap_skybox.rs)
const _: () = assert!(std::mem::size_of::<Canvas2DUniforms>() == 80);
```

UISlider integration (from `src/game/ui/slider.rs`):
```rust
// In the right-side property panel when in Draw2D stage and image is loaded:
let opacity_slider = UISlider::new(
    "Opacity",
    panel_x,
    panel_y + 30.0,
    image_trace.opacity,
    [0.4, 0.7, 1.0, 1.0], // Blue slider color
);
// Render slider background and handle using add_quad()
// On click/drag within slider bounds:
if opacity_slider.contains(mouse_x, mouse_y) && mouse_held {
    image_trace.opacity = opacity_slider.value_from_x(mouse_x);
}
```

## Acceptance Criteria
- [ ] `src/game/asset_editor/image_trace.rs` exists with `ImageTrace` struct and `load()` method
- [ ] `shaders/canvas_2d.wgsl` exists with vertex and fragment shaders implementing textured quad rendering with opacity uniform
- [ ] `Cargo.toml` `image` crate features include both `"png"` and `"jpeg"`
- [ ] `pub mod image_trace;` declared in `src/game/asset_editor/mod.rs`
- [ ] PNG images load correctly and display as a textured quad on the canvas
- [ ] JPG images load correctly and display as a textured quad on the canvas
- [ ] Image renders BEHIND the grid and outlines (render order: image first, grid second, outlines third)
- [ ] Default opacity is 0.3 (grid lines and outlines clearly visible on top)
- [ ] Opacity adjustable from 0.0 to 1.0 via `UISlider` in the property panel
- [ ] Ctrl+Scroll adjusts image scale (clamped 0.1x to 10.0x)
- [ ] Ctrl+MiddleMouse drag adjusts image position offset in canvas coordinates
- [ ] Image preserves original aspect ratio at all scale levels
- [ ] Ctrl+H toggles image visibility on/off
- [ ] Ctrl+I initiates image loading (via stdin path input or command-line argument)
- [ ] Outlines drawn on top of the image are clearly visible at default opacity
- [ ] Alpha blending is enabled on the textured quad pipeline
- [ ] `cargo check --bin battle_editor` compiles with 0 errors
- [ ] `cargo check --bin battle_arena` compiles with 0 errors (battle_arena.rs is NOT modified)
- [ ] `battle_arena.rs` is NOT modified

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor compiles with image trace support`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles (not modified)`
- `cmd`: `test -f src/game/asset_editor/image_trace.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `image_trace.rs file exists`
- `cmd`: `test -f shaders/canvas_2d.wgsl && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `canvas_2d.wgsl shader file exists`
- `cmd`: `grep -c 'pub struct ImageTrace' src/game/asset_editor/image_trace.rs`
  `expect_gt`: 0
  `description`: `ImageTrace struct is defined`
- `cmd`: `grep -c 'pub fn load' src/game/asset_editor/image_trace.rs`
  `expect_gt`: 0
  `description`: `load function is implemented`
- `cmd`: `grep -c 'opacity' src/game/asset_editor/image_trace.rs`
  `expect_gt`: 2
  `description`: `Opacity field and handling implemented`
- `cmd`: `grep -c 'textureSample' shaders/canvas_2d.wgsl`
  `expect_gt`: 0
  `description`: `Shader samples texture`
- `cmd`: `grep -c 'opacity' shaders/canvas_2d.wgsl`
  `expect_gt`: 0
  `description`: `Shader uses opacity uniform`
- `cmd`: `grep -c 'pub mod image_trace' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `image_trace module registered in mod.rs`
- `cmd`: `grep 'jpeg' Cargo.toml`
  `expect_contains`: `jpeg`
  `description`: `JPEG feature enabled in Cargo.toml`
- `cmd`: `grep -c 'image_trace' src/game/asset_editor/canvas_2d.rs`
  `expect_gt`: 0
  `description`: `ImageTrace integrated into Canvas2D`
- `cmd`: `! grep -q 'image_trace\|ImageTrace\|background_image\|BackgroundImage' src/bin/battle_arena.rs && echo UNTOUCHED`
  `expect_contains`: `UNTOUCHED`
  `description`: `battle_arena.rs was not modified`

## Success Looks Like
In Stage 1 (Draw2D), pressing Ctrl+I prompts for an image path. After entering a path to a PNG or JPG file, the image appears as a semi-transparent background behind the grid. The default opacity of 30% makes the reference image visible but muted — the white grid lines and drawn outlines stand out clearly on top. Ctrl+Scroll resizes the image smoothly while preserving its aspect ratio; a 1920x1080 photo and a 512x512 icon both display with correct proportions. Ctrl+MiddleMouse drag repositions the image to align the relevant part with the drawing area. The opacity slider in the right-side property panel allows fine-tuning: slide to 0.0 and the image disappears, slide to 1.0 and it becomes fully opaque. Ctrl+H quickly toggles the image off when the artist wants to see their outlines without the reference, and back on when they need it again. The artist draws freehand or line outlines on top of the reference image, tracing the silhouette of the object they want to create.

## Dependencies
- Depends on: US-P4-003

## Complexity
- Complexity: normal
- Min iterations: 1
