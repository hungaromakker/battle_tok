# US-P4-005: Background Image Loading for Tracing

## Description
Add the ability to load a reference image (PNG or JPG) as a semi-transparent background on the 2D canvas, allowing artists to trace over concept art or photographs. The image is rendered as a textured quad behind the outline drawing layer, with adjustable opacity, scale, and position. This requires a new wgsl shader for textured quad rendering since the existing canvas rendering uses untextured colored quads.

## The Core Concept / Why This Matters
Artists rarely draw from scratch — they work from references. Being able to load a concept sketch, photo of a real object, or previous iteration as a background layer lets the artist trace accurate outlines quickly. Without this, the artist must constantly switch between a reference viewer and the editor, eyeballing proportions. The opacity control is key: too opaque and it obscures the grid/outlines, too transparent and the reference is useless. The default of 0.3 strikes the right balance.

## Goal
Create `src/game/asset_editor/image_trace.rs` for image loading and management, create `shaders/canvas_2d.wgsl` for textured quad rendering with opacity, and integrate into the 2D canvas stage with keyboard/mouse controls.

## Files to Create/Modify
- **Create** `src/game/asset_editor/image_trace.rs` — `ImageTrace` struct: load PNG/JPG via `image` crate, create wgpu texture + bind group, manage opacity/scale/position
- **Create** `shaders/canvas_2d.wgsl` — vertex/fragment shader for textured quad rendering with uniform opacity
- **Modify** `src/game/asset_editor/canvas_2d.rs` — Integrate `ImageTrace` into canvas rendering (draw image behind grid and outlines)
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod image_trace;`
- **Modify** `src/bin/battle_editor.rs` — Handle Ctrl+I file dialog, Ctrl+scroll for image scale, Ctrl+middle-mouse for image position

## Implementation Steps
1. Create `ImageTrace` struct:
   ```rust
   pub struct ImageTrace {
       pub texture: Option<wgpu::Texture>,
       pub texture_view: Option<wgpu::TextureView>,
       pub bind_group: Option<wgpu::BindGroup>,
       pub opacity: f32,        // 0.0 to 1.0, default 0.3
       pub scale: f32,          // default 1.0 (1 pixel = 1 canvas unit)
       pub position: [f32; 2],  // offset in canvas coordinates, default [0, 0]
       pub visible: bool,       // toggle visibility
       image_width: u32,
       image_height: u32,
   }
   ```
2. Implement `ImageTrace::load(path: &str, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self>`:
   - Use `image::open(path)` to load PNG/JPG (the `image` crate is already in `Cargo.toml`)
   - Convert to RGBA8
   - Create `wgpu::Texture` with `TextureDescriptor` (RGBA8Unorm, usage TEXTURE_BINDING | COPY_DST)
   - Write pixel data via `queue.write_texture()`
   - Create `TextureView` and `Sampler` (linear filtering, clamp-to-edge)
   - Create `BindGroup` with texture view + sampler + opacity uniform
3. Create `shaders/canvas_2d.wgsl`:
   ```wgsl
   struct VertexInput {
       @location(0) position: vec3<f32>,
       @location(1) uv: vec2<f32>,
   };
   struct VertexOutput {
       @builtin(position) clip_position: vec4<f32>,
       @location(0) uv: vec2<f32>,
   };
   struct Uniforms {
       view_projection: mat4x4<f32>,
       opacity: f32,
   };
   @group(0) @binding(0) var<uniform> uniforms: Uniforms;
   @group(0) @binding(1) var t_texture: texture_2d<f32>;
   @group(0) @binding(2) var t_sampler: sampler;
   
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
4. Create render pipeline for the textured quad:
   - Vertex layout: position (vec3) + uv (vec2)
   - Bind group layout: uniform buffer + texture + sampler
   - Alpha blending enabled
   - Pipeline created once, reused each frame
5. Implement `ImageTrace::render()`:
   - Generate a fullscreen quad in canvas coordinates: centered at `position`, sized by `image_width * scale` x `image_height * scale`
   - UV coordinates: (0,0) to (1,1)
   - Update opacity uniform buffer
   - Draw with the textured pipeline
6. Implement controls:
   - `Ctrl+I`: trigger file open dialog (use `rfd` crate or simple stdin path for now — if `rfd` is not in Cargo.toml, accept path via a text input field in the UI)
   - Opacity slider: use existing `UISlider` from `src/game/ui/slider.rs` (range 0.0 to 1.0, step 0.05)
   - `Ctrl+Scroll`: adjust image scale (multiplicative, clamp 0.1 to 10.0)
   - `Ctrl+Middle-mouse drag`: adjust image position offset
   - `Ctrl+H`: toggle image visibility
7. Integrate into `Canvas2D`:
   - `Canvas2D` gets an `image_trace: Option<ImageTrace>` field
   - In render: if image_trace is Some and visible, render it FIRST (behind grid and outlines)
   - Alpha blending ensures outlines are visible over the semi-transparent image
8. Handle aspect ratio: the image quad preserves the original aspect ratio of the loaded image

## Code Patterns
Loading a texture from the `image` crate:
```rust
use image::GenericImageView;

pub fn load(path: &str, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self, String> {
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
    
    queue.write_texture(
        wgpu::ImageCopyTexture { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &rgba,
        wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );
    // ... create view, sampler, bind group
    Ok(Self { /* ... */ })
}
```

UISlider integration (from `src/game/ui/slider.rs`):
```rust
let opacity_slider = UISlider::new("Opacity", 0.0, 1.0, self.image_trace.opacity);
// Render slider, handle drag to update opacity
```

## Acceptance Criteria
- [ ] `image_trace.rs` exists with `ImageTrace` struct
- [ ] Can load PNG and JPG images via the `image` crate
- [ ] Image renders as a textured quad behind canvas outlines
- [ ] `shaders/canvas_2d.wgsl` implements textured quad rendering with opacity uniform
- [ ] Opacity adjustable (0.0 to 1.0, default 0.3) via `UISlider`
- [ ] Ctrl+Scroll adjusts image scale (0.1x to 10x)
- [ ] Ctrl+Middle-mouse drag adjusts image position
- [ ] Image preserves original aspect ratio
- [ ] Image visibility toggleable (Ctrl+H)
- [ ] Outlines remain visible on top of the background image
- [ ] `cargo check --bin battle_editor` compiles with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor compiles with image trace support`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles`
- `cmd`: `test -f src/game/asset_editor/image_trace.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `image_trace.rs file exists`
- `cmd`: `test -f shaders/canvas_2d.wgsl && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `canvas_2d shader file exists`
- `cmd`: `grep -c 'pub struct ImageTrace' src/game/asset_editor/image_trace.rs`
  `expect_gt`: 0
  `description`: `ImageTrace struct defined`
- `cmd`: `grep -c 'opacity' src/game/asset_editor/image_trace.rs`
  `expect_gt`: 1
  `description`: `Opacity handling implemented`
- `cmd`: `grep -c 'textureSample\|t_texture' shaders/canvas_2d.wgsl`
  `expect_gt`: 0
  `description`: `Shader samples texture with opacity`
- `cmd`: `grep -c 'pub mod image_trace' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `image_trace module registered`

## Success Looks Like
In Stage 1 (Draw2D), pressing Ctrl+I loads a reference image that appears as a semi-transparent background behind the grid. The image is at 30% opacity by default — grid lines and drawn outlines are clearly visible on top. Ctrl+scroll resizes the image smoothly. Ctrl+middle-mouse repositions it. The opacity slider in the UI allows fine adjustment. Ctrl+H toggles the image on/off. The artist can trace over the reference to create accurate outlines.

## Dependencies
- Depends on: US-P4-003

## Complexity
- Complexity: normal
- Min iterations: 1
