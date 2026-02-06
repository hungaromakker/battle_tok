# Phase 4: Asset Editor — In-Engine Drawing & 3D Pipeline

## Problem Statement

Battle Tök currently has no way to create custom game assets (trees, rocks, grass, structures, props) inside the engine. All geometry is either procedural (terrain, hex prisms) or hardcoded (cannon models, building blocks). To build a visually rich world with varied environments, we need an in-engine tool that lets users:

1. **Draw 2D outlines** on a canvas (freehand, line, arc tools)
2. **Trace over reference images** loaded as canvas backgrounds
3. **Extrude/inflate those outlines into 3D** via the existing SDF pipeline
4. **Sculpt details** using the existing `SculptingManager`
5. **Paint vertex colors** onto the surface
6. **Save as reusable assets** with a variety system for natural variation
7. **Place assets in the world** with instanced rendering

The tool must use **zero external dependencies** — it builds entirely on the existing wgpu rendering, SDF operations, Marching Cubes, and UI primitives already in the engine, and than baking it into meshes so perfomant for real word use

---

## Solution Overview

### Architecture

The Asset Editor is a **separate binary** (`cargo run --bin battle_editor`) that shares the engine library but runs independently from the game. This keeps `battle_arena.rs` clean and focused on gameplay while the editor has its own window, event loop, and UI layout optimized for asset creation.

The editor pipeline has 5 stages:

```
[2D Canvas] → [Extrude/Pump] → [Sculpt] → [Color] → [Save/Library]
     1              2              3          4            5
```

Each stage is a separate sub-module with its own state. The user presses number keys (1-5) to switch between stages, and can go back to any previous stage to iterate.

**Binary**: `src/bin/battle_editor.rs` — owns the winit event loop, wgpu initialization, and routes input/render to the `AssetEditor` state machine. Window title: "Battle Tök — Asset Editor".

### Integration Points (Existing Code)

| System | File | What We Use |
|--------|------|-------------|
| SDF smooth_union/subtraction | `engine/src/render/sdf_operations.rs` | Combining 3D shapes from outlines |
| Marching Cubes mesh generation | `engine/src/render/marching_cubes.rs` | `MarchingCubes::new(resolution)` + `generate_mesh()` to convert SDF → triangles |
| Sculpting tools | `engine/src/render/sculpting.rs` | `SculptingManager`, `ExtrusionOperation`, face/edge/vertex selection |
| Instance buffer | `engine/src/render/instancing.rs` | `CreatureInstance` struct for rendering many copies of same asset |
| Vertex type | `src/game/types.rs` | `Vertex { position: [f32;3], normal: [f32;3], color: [f32;4] }` |
| Mesh type | `src/game/types.rs` | `Mesh { vertices: Vec<Vertex>, indices: Vec<u32> }` with `merge()` |
| UI slider | `src/game/ui/slider.rs` | `UISlider` for parameter controls |
| UI text | `src/game/ui/text.rs` | `draw_text()`, `add_quad()`, `get_char_bitmap()` for labels |
| Camera | `src/game/types.rs` | `Camera` struct — we create an orbit variant for the editor |
| Building blocks | `engine/src/render/building_blocks.rs` | `BlockVertex` type (pos + normal + color, 40 bytes) for GPU buffers |

### New Code Modules

```
src/game/asset_editor/
├── mod.rs              # AssetEditor state machine, mode switching, main update/render
├── canvas_2d.rs        # 2D drawing tools: freehand, line, arc, eraser, mirror
├── image_trace.rs      # Load PNG/JPG as canvas background for tracing
├── extrude.rs          # Pump/inflate, linear extrude, lathe revolution
├── sculpt_bridge.rs    # Bridge between editor and existing SculptingManager
├── paint.rs            # Vertex color brush, fill, gradient tools
├── asset_file.rs       # .btasset binary format: save/load
├── library.rs          # Asset library panel: grid view, search, thumbnails
├── variety.rs          # Seed-based variation: scale, rotation, hue, noise displacement
├── placement.rs        # World placement: ghost preview, scatter brush, ground conform
├── orbit_camera.rs     # Orbit camera for 3D preview (separate from FPS camera)
├── ui_panels.rs        # Tool palette, property panel, color picker UI
└── undo.rs             # Undo/redo command stack (50 operations max)
```

---

## Data Structures

### Core Types

```rust
// src/game/asset_editor/mod.rs

/// The five pipeline stages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorStage {
    Draw2D,      // Stage 1: 2D outline drawing
    Extrude,     // Stage 2: Convert to 3D
    Sculpt,      // Stage 3: Refine shape
    Color,       // Stage 4: Paint vertex colors
    Save,        // Stage 5: Save to library
}

/// Asset categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AssetCategory {
    Tree,
    Grass,
    Rock,
    Structure,
    Prop,
    Decoration,
}

/// Main editor state
pub struct AssetEditor {
    pub active: bool,
    pub stage: EditorStage,
    pub draft: AssetDraft,
    pub orbit_camera: OrbitCamera,
    pub undo_stack: UndoStack,
    pub library: AssetLibrary,
    pub ui: EditorUI,
    // GPU resources created once on init
    pub canvas_vb: Option<wgpu::Buffer>,
    pub canvas_ib: Option<wgpu::Buffer>,
    pub preview_vb: Option<wgpu::Buffer>,
    pub preview_ib: Option<wgpu::Buffer>,
    pub background_texture: Option<wgpu::Texture>,
    pub background_bind_group: Option<wgpu::BindGroup>,
}
```

### 2D Canvas Types

```rust
// src/game/asset_editor/canvas_2d.rs

/// A single 2D outline (closed polygon)
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Outline2D {
    pub name: String,               // e.g. "trunk", "canopy"
    pub points: Vec<glam::Vec2>,    // Closed polygon vertices (normalized coords)
    pub is_closed: bool,
    pub mirror_x: bool,             // Drawn with X-axis symmetry?
}

/// Drawing tool selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTool {
    Freehand,   // D key - click-drag smooth curves
    Line,       // L key - click two endpoints
    Arc,        // A key - click three points (start, mid, end)
    Eraser,     // E key - click-drag to remove nearby segments
}

/// Canvas state
pub struct Canvas2D {
    pub outlines: Vec<Outline2D>,
    pub current_outline: Option<Outline2D>,  // Being drawn right now
    pub tool: DrawTool,
    pub mirror_x: bool,                      // M key toggle
    pub grid_snap: bool,                     // G key toggle
    pub grid_size: f32,                      // Default 1.0 unit
    pub zoom: f32,                           // Orthographic scale
    pub pan: glam::Vec2,                     // Canvas pan offset
    pub background_image: Option<BackgroundImage>,  // For tracing
    pub rdp_epsilon: f32,                    // Ramer-Douglas-Peucker simplification threshold
}

/// Background image for tracing
pub struct BackgroundImage {
    pub width: u32,
    pub height: u32,
    pub opacity: f32,           // 0.0-1.0, default 0.3
    pub scale: f32,             // Fit to canvas
    pub offset: glam::Vec2,     // Position on canvas
    pub texture_id: u32,        // GPU texture handle reference
}
```

### Extrusion Types

```rust
// src/game/asset_editor/extrude.rs

/// Extrusion method selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrudeMethod {
    Pump,       // Inflate outline into organic 3D shape
    Linear,     // Extrude along Z axis
    Lathe,      // Revolve around Y axis
}

/// Cross-section profile for Pump method
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpProfile {
    Elliptical, // Default - thickest at center, zero at edge
    Flat,       // Constant depth
    Pointed,    // Like a leaf - thin edges, sharp center ridge
}

/// Extrusion parameters
pub struct ExtrudeParams {
    pub method: ExtrudeMethod,
    // Pump parameters
    pub inflation: f32,         // 0.0-1.0, how much shape puffs out
    pub thickness: f32,         // 0.1-5.0, maximum depth in units
    pub profile: PumpProfile,
    // Linear extrude parameters
    pub depth: f32,             // Extrusion depth
    pub taper: f32,             // 1.0 = no taper, 0.0 = taper to point
    // Lathe parameters
    pub segments: u32,          // 6-64, angular resolution
    pub sweep_degrees: f32,     // 0-360, partial rotation
    // Shared
    pub mc_resolution: u32,     // Marching Cubes resolution (default 48)
}
```

### Paint Types

```rust
// src/game/asset_editor/paint.rs

/// Painting tool selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintTool {
    Brush,      // B key - paint per-vertex color with round brush
    Fill,       // F key - flood-fill connected region
    Gradient,   // Click-drag for gradient between two colors
    Eyedropper, // I key - sample existing color
}

/// Color palette for painting
pub struct ColorPalette {
    pub primary: [f32; 4],      // Currently selected color (RGBA)
    pub secondary: [f32; 4],    // Alt-click color
    pub presets: Vec<([f32; 4], &'static str)>,  // (color, name)
    pub recent: Vec<[f32; 4]>,  // Recently used colors (max 8)
    pub hsv: [f32; 3],          // HSV representation of primary
}

/// Brush parameters
pub struct BrushParams {
    pub radius: f32,        // World-space radius
    pub opacity: f32,       // 0.0-1.0
    pub hardness: f32,      // 0.0 = soft falloff, 1.0 = hard edge
}
```

### Variety System Types

```rust
// src/game/asset_editor/variety.rs

/// Per-asset variety parameters - controls randomization at placement time
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct VarietyParams {
    // Scale variation
    pub scale_min: f32,              // e.g. 0.7
    pub scale_max: f32,              // e.g. 1.3
    pub scale_y_bias: f32,           // Extra vertical stretch (0.0 = uniform)

    // Rotation
    pub random_y_rotation: bool,     // Almost always true
    pub tilt_max_degrees: f32,       // Max random lean off vertical

    // Color variation
    pub hue_shift_range: f32,        // +/- degrees of hue shift
    pub saturation_range: f32,       // +/- saturation adjustment
    pub brightness_range: f32,       // +/- brightness adjustment

    // Shape variation
    pub noise_displacement: f32,     // Perlin noise vertex displacement amount
    pub noise_frequency: f32,        // Displacement noise frequency
}

impl Default for VarietyParams {
    fn default() -> Self {
        Self {
            scale_min: 0.8,
            scale_max: 1.2,
            scale_y_bias: 0.0,
            random_y_rotation: true,
            tilt_max_degrees: 5.0,
            hue_shift_range: 15.0,
            saturation_range: 0.1,
            brightness_range: 0.1,
            noise_displacement: 0.0,
            noise_frequency: 1.0,
        }
    }
}
```

### Asset File Format

```rust
// src/game/asset_editor/asset_file.rs

/// .btasset binary file layout:
///
/// ┌────────────────────────────────────────┐
/// │ Header (32 bytes)                       │
/// │   magic: [u8; 4]  = b"BTAS"           │
/// │   version: u32     = 1                  │
/// │   vertex_count: u32                     │
/// │   index_count: u32                      │
/// │   metadata_offset: u32                  │
/// │   metadata_size: u32                    │
/// │   variety_offset: u32                   │
/// │   variety_size: u32                     │
/// ├────────────────────────────────────────┤
/// │ Vertex Data (vertex_count * 40 bytes)   │
/// │   position: [f32; 3]  (12 bytes)       │
/// │   normal: [f32; 3]    (12 bytes)       │
/// │   color: [f32; 4]     (16 bytes)       │
/// ├────────────────────────────────────────┤
/// │ Index Data (index_count * 4 bytes)      │
/// │   indices: [u32; index_count]           │
/// ├────────────────────────────────────────┤
/// │ Metadata (JSON, UTF-8)                  │
/// │   name, category, tags, bounds          │
/// ├────────────────────────────────────────┤
/// │ Variety Params (JSON, UTF-8)            │
/// │   VarietyParams struct serialized       │
/// └────────────────────────────────────────┘

pub const BTASSET_MAGIC: [u8; 4] = *b"BTAS";
pub const BTASSET_VERSION: u32 = 1;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BtassetHeader {
    pub magic: [u8; 4],
    pub version: u32,
    pub vertex_count: u32,
    pub index_count: u32,
    pub metadata_offset: u32,
    pub metadata_size: u32,
    pub variety_offset: u32,
    pub variety_size: u32,
}
```

---

## What Changes vs What Stays

### New Files (to create)

| File | Purpose | Lines (est.) |
|------|---------|-------------|
| `src/bin/battle_editor.rs` | Standalone editor binary: winit event loop, wgpu init, input routing | ~400 |
| `src/game/asset_editor/mod.rs` | Editor state machine, stage switching | ~300 |
| `src/game/asset_editor/canvas_2d.rs` | 2D drawing: freehand, line, arc, eraser, mirror, grid snap | ~500 |
| `src/game/asset_editor/image_trace.rs` | Load PNG as canvas background, opacity/scale controls | ~200 |
| `src/game/asset_editor/extrude.rs` | Pump (SDF inflate), linear extrude, lathe revolution | ~400 |
| `src/game/asset_editor/sculpt_bridge.rs` | Connect editor to existing SculptingManager | ~150 |
| `src/game/asset_editor/paint.rs` | Vertex color brush, fill, gradient, eyedropper | ~350 |
| `src/game/asset_editor/asset_file.rs` | .btasset binary save/load | ~250 |
| `src/game/asset_editor/library.rs` | Library panel: grid view, search, thumbnails | ~300 |
| `src/game/asset_editor/variety.rs` | Seed-based variation at placement time | ~200 |
| `src/game/asset_editor/placement.rs` | Ghost preview, scatter brush, ground conform | ~250 |
| `src/game/asset_editor/orbit_camera.rs` | Orbit camera for asset preview | ~120 |
| `src/game/asset_editor/ui_panels.rs` | Tool palette, property sliders, color picker | ~400 |
| `src/game/asset_editor/undo.rs` | Command stack (50 max), Ctrl+Z/Y | ~150 |
| `shaders/canvas_2d.wgsl` | 2D line rendering + background image | ~80 |
| `shaders/asset_preview.wgsl` | 3D preview with orbit lighting | ~60 |

### Changed Files

| File | What Changes |
|------|-------------|
| `Cargo.toml` | Add `[[bin]] name = "battle_editor" path = "src/bin/battle_editor.rs"` |
| `src/game/mod.rs` | Add `pub mod asset_editor;` |
| `src/game/types.rs` | No changes — `Vertex` and `Mesh` already have everything we need |

**NOTE:** `battle_arena.rs` is **NOT modified**. The editor is a completely separate binary.

### Unchanged Files

| File | Why Unchanged |
|------|--------------|
| `engine/src/render/sdf_operations.rs` | We call its functions but don't modify it |
| `engine/src/render/marching_cubes.rs` | We call `generate_mesh()` but don't modify it |
| `engine/src/render/sculpting.rs` | We wrap it via `sculpt_bridge.rs` but don't modify it |
| `engine/src/render/instancing.rs` | We use `CreatureInstance` for placed assets but don't modify it |
| `src/game/ui/slider.rs` | We reuse `UISlider` for editor panels |
| `src/game/ui/text.rs` | We reuse `draw_text()` and `add_quad()` |
| All terrain/building/economy/population code | Unrelated to asset editor |

---

## Stories

### Story 1: Create battle_editor Binary + AssetEditor Module Skeleton

**What:** Create `src/bin/battle_editor.rs` as a standalone binary with its own winit event loop and wgpu initialization. Create `src/game/asset_editor/mod.rs` with the `AssetEditor` struct, `EditorStage` enum, and `AssetCategory` enum. Add `[[bin]]` entry to `Cargo.toml`. The editor opens its own window titled "Battle Tök — Asset Editor" with a dark background and stage indicator.

**Files:**
- Create `src/bin/battle_editor.rs` — standalone binary (winit + wgpu init, event loop, input routing)
- Create `src/game/asset_editor/mod.rs` — editor state machine
- Modify `src/game/mod.rs` — add `pub mod asset_editor;`
- Modify `Cargo.toml` — add `[[bin]] name = "battle_editor" path = "src/bin/battle_editor.rs"`

**Acceptance:**
- `cargo run --bin battle_editor` opens a window with "Battle Tök — Asset Editor" title
- Stage switching with keys 1-5 changes `editor.stage` and is visible in window
- `battle_arena.rs` is NOT modified
- `cargo check` passes for both binaries

---

### Story 2: Orbit Camera for Asset Preview

**What:** Create `src/game/asset_editor/orbit_camera.rs` with an `OrbitCamera` struct that orbits around a center point. Middle-mouse drag to orbit, scroll to zoom, right-mouse to pan. This is used for all 3D preview stages (2-5).

**Files:**
- Create `src/game/asset_editor/orbit_camera.rs`
- Modify `src/game/asset_editor/mod.rs` — add `orbit_camera` field, delegate mouse events

**Details:**
```rust
pub struct OrbitCamera {
    pub center: Vec3,       // Point camera orbits around (default: origin)
    pub distance: f32,      // Distance from center (default: 5.0)
    pub yaw: f32,           // Horizontal angle (radians)
    pub pitch: f32,         // Vertical angle (radians), clamped -89..89 degrees
    pub fov: f32,           // Field of view (default: 60 degrees)
    pub near: f32,          // Near plane (0.01)
    pub far: f32,           // Far plane (100.0)
}

impl OrbitCamera {
    pub fn view_matrix(&self) -> Mat4 { ... }
    pub fn projection_matrix(&self, aspect: f32) -> Mat4 { ... }
    pub fn handle_mouse_drag(&mut self, dx: f32, dy: f32) { ... }  // Middle mouse
    pub fn handle_scroll(&mut self, delta: f32) { ... }             // Zoom
    pub fn handle_pan(&mut self, dx: f32, dy: f32) { ... }          // Right mouse
}
```

**Acceptance:**
- Camera orbits smoothly around center point
- Zoom works with scroll wheel (clamped to 0.5..50.0)
- `cargo check` passes

---

### Story 3: 2D Canvas Drawing — Freehand + Line Tools

**What:** Create `src/game/asset_editor/canvas_2d.rs` with `Canvas2D` struct, `Outline2D` struct, and drawing tools (freehand + line). The canvas uses orthographic projection. Mouse events create outline points. Freehand uses Ramer-Douglas-Peucker simplification after mouse-up.

**Files:**
- Create `src/game/asset_editor/canvas_2d.rs`
- Modify `src/game/asset_editor/mod.rs` — add canvas field, route input in Stage 1

**Details:**

**Ramer-Douglas-Peucker algorithm** (for simplifying freehand polylines):
```rust
fn rdp_simplify(points: &[Vec2], epsilon: f32) -> Vec<Vec2> {
    if points.len() < 3 { return points.to_vec(); }
    // Find point with maximum distance from line (first, last)
    let mut max_dist = 0.0f32;
    let mut max_idx = 0;
    let line_start = points[0];
    let line_end = points[points.len() - 1];
    for (i, p) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let dist = point_to_line_distance(*p, line_start, line_end);
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }
    if max_dist > epsilon {
        let mut left = rdp_simplify(&points[..=max_idx], epsilon);
        let right = rdp_simplify(&points[max_idx..], epsilon);
        left.pop(); // Remove duplicate point at junction
        left.extend(right);
        left
    } else {
        vec![points[0], points[points.len() - 1]]
    }
}
```

**Canvas coordinate system:**
- Origin at canvas center
- 1 unit = 1 grid cell
- Default view: 20x20 units visible
- Mouse position → canvas position via inverse orthographic projection

**Rendering:** Generate line segments as thin quads (2px wide) using `add_quad()` from `src/game/ui/text.rs`. Render with the existing UI pipeline (no depth test).

**Acceptance:**
- Freehand drawing works: click-drag creates smooth polyline, simplified on mouse-up
- Line tool works: click start, click end
- Grid visible with 1-unit spacing
- Zoom (scroll) and pan (middle-mouse) work
- `cargo check` passes

---

### Story 4: 2D Canvas — Arc Tool, Eraser, Mirror Symmetry

**What:** Add arc tool (3-point click: start, mid, end → circular arc approximated as polyline segments), eraser tool (remove points near cursor), and X-axis mirror toggle.

**Files:**
- Modify `src/game/asset_editor/canvas_2d.rs`

**Details:**

**Arc from 3 points** (circumscribed circle):
```rust
fn arc_from_3_points(p0: Vec2, p1: Vec2, p2: Vec2, num_segments: u32) -> Vec<Vec2> {
    // Find circumcenter of triangle (p0, p1, p2)
    // Generate arc points from p0 to p2 passing through p1
    // Return num_segments evenly spaced points on the arc
}
```

**Mirror symmetry:**
- When `mirror_x = true`, every point drawn at `(x, y)` also creates a point at `(-x, y)`
- Mirror applies to all tools (freehand, line, arc)
- Visual: a dashed vertical line at x=0 indicates the mirror axis

**Eraser:**
- Circle cursor (radius = eraser_size, configurable)
- On mouse drag, remove any outline points within `eraser_size` distance from cursor
- If eraser breaks an outline into disconnected segments, split into separate outlines

**Acceptance:**
- Arc tool creates smooth arc through 3 clicked points
- Mirror toggle (M key) mirrors drawing across X axis
- Eraser removes nearby points/segments
- `cargo check` passes

---

### Story 5: Background Image Loading for Tracing

**What:** Create `src/game/asset_editor/image_trace.rs` that loads a PNG/JPG file as a background texture on the 2D canvas. The image is displayed behind the drawing grid at configurable opacity, scale, and position. Users draw outlines on top of the reference image.

**Files:**
- Create `src/game/asset_editor/image_trace.rs`
- Modify `src/game/asset_editor/canvas_2d.rs` — render background image before grid
- Create `shaders/canvas_2d.wgsl` — textured quad shader for background image

**Details:**

**Image loading** uses the existing `image` crate (already in Cargo.toml):
```rust
use image::GenericImageView;

pub fn load_background_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    path: &str,
) -> Result<BackgroundImage, String> {
    let img = image::open(path).map_err(|e| e.to_string())?;
    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Background Image"),
        size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        &rgba,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(4 * width), rows_per_image: Some(height) },
        wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
    );

    // Create sampler, bind group, etc.
    Ok(BackgroundImage { width, height, opacity: 0.3, scale: 1.0, offset: Vec2::ZERO, texture_id: 0 })
}
```

**Canvas shader** (`shaders/canvas_2d.wgsl`):
```wgsl
// Renders a textured quad with adjustable opacity
@group(0) @binding(0) var bg_texture: texture_2d<f32>;
@group(0) @binding(1) var bg_sampler: sampler;

struct Uniforms {
    opacity: f32,
    // ... transform
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(bg_texture, bg_sampler, in.uv);
    return vec4(color.rgb, color.a * uniforms.opacity);
}
```

**Controls:**
- File picker: press `Ctrl+I` to open file dialog (or type path in text input)
- Opacity slider: 0.0-1.0 (default 0.3)
- Scale: scroll while holding Ctrl
- Position: drag while holding Ctrl + middle-mouse

**Acceptance:**
- PNG/JPG loads and displays as canvas background
- Opacity slider works
- Image can be scaled and repositioned
- Drawing tools work on top of the image
- `cargo check` passes

---

### Story 6: Pump/Inflate Extrusion (SDF-based)

**What:** Create `src/game/asset_editor/extrude.rs` with the pump/inflate algorithm. Takes a 2D outline and generates a 3D SDF by computing distance to the outline boundary, then modulating depth by a thickness profile. Uses `MarchingCubes` from `engine/src/render/marching_cubes.rs` to convert SDF to triangle mesh.

**Files:**
- Create `src/game/asset_editor/extrude.rs`
- Modify `src/game/asset_editor/mod.rs` — route stage 2 to extrusion, show sliders

**Details:**

**SDF formulation for Pump:**
```rust
fn sdf_pumped(p: Vec3, outline: &Outline2D, params: &ExtrudeParams) -> f32 {
    // 1. Project point onto XY plane
    let p2d = Vec2::new(p.x, p.y);

    // 2. Compute minimum distance to outline boundary (2D)
    let dist_to_boundary = min_distance_to_polygon(p2d, &outline.points);

    // 3. Determine if point is inside outline (2D)
    let inside = point_in_polygon(p2d, &outline.points);
    let signed_dist_2d = if inside { -dist_to_boundary } else { dist_to_boundary };

    // 4. Compute thickness profile at this distance from boundary
    let normalized_dist = (dist_to_boundary / max_inradius).clamp(0.0, 1.0);
    let profile_depth = match params.profile {
        PumpProfile::Elliptical => (1.0 - normalized_dist * normalized_dist).sqrt(),
        PumpProfile::Flat => 1.0,
        PumpProfile::Pointed => 1.0 - normalized_dist,
    };

    // 5. The SDF: negative inside the inflated volume
    let max_z = params.thickness * params.inflation * profile_depth;
    let z_dist = p.z.abs() - max_z;

    // 6. Combine 2D boundary distance with Z thickness
    if inside {
        z_dist.max(0.0)  // Inside boundary: only Z matters
    } else {
        (signed_dist_2d * signed_dist_2d + z_dist.max(0.0).powi(2)).sqrt()
    }
}
```

**UI controls** (3 sliders using `UISlider`):
- `Inflation`: 0.0-1.0 (default 0.5) — how much shape puffs out
- `Thickness`: 0.1-5.0 (default 1.0) — maximum depth in units
- `Profile`: dropdown (Elliptical / Flat / Pointed)

**Marching Cubes integration:**
```rust
let mc = MarchingCubes::new(params.mc_resolution); // default 48
let sdf = |p: Vec3| -> f32 { sdf_pumped(p, outline, params) };
let mesh = mc.generate_mesh(&sdf, bounds_min, bounds_max);
```

**Acceptance:**
- Pump creates smooth 3D shape from any closed outline
- All 3 profile types produce visually different results
- Sliders update preview in real-time (re-mesh on change)
- `cargo check` passes

---

### Story 7: Linear Extrude + Lathe Revolution

**What:** Add linear extrusion (push outline along Z with optional taper) and lathe revolution (spin outline profile around Y axis) to `extrude.rs`.

**Files:**
- Modify `src/game/asset_editor/extrude.rs` — add `sdf_linear_extrude()` and `lathe_mesh()`

**Details:**

**Linear extrude SDF:**
```rust
fn sdf_linear_extrude(p: Vec3, outline: &Outline2D, depth: f32, taper: f32) -> f32 {
    // Taper: scale outline smaller at the back (z = depth)
    let z_ratio = (p.z / depth).clamp(0.0, 1.0);
    let scale = 1.0 + (taper - 1.0) * z_ratio;  // taper=1.0: no taper, taper=0.0: point

    let scaled_p2d = Vec2::new(p.x / scale, p.y / scale);
    let dist_2d = signed_distance_to_polygon(scaled_p2d, &outline.points);

    let z_dist = if p.z < 0.0 { -p.z } else if p.z > depth { p.z - depth } else { 0.0 };
    dist_2d.max(z_dist)
}
```

**Lathe (revolution):** Generate mesh directly (not via SDF/MC — more efficient for rotationally symmetric shapes):
```rust
fn lathe_mesh(outline: &Outline2D, segments: u32, sweep: f32) -> Mesh {
    // outline.points define a 2D profile where:
    //   x = radius from Y axis
    //   y = height
    // Sweep the profile around Y axis
    let angle_step = sweep.to_radians() / segments as f32;
    for seg in 0..=segments {
        let angle = seg as f32 * angle_step;
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        for point in &outline.points {
            let vertex = Vertex {
                position: [point.x * cos_a, point.y, point.x * sin_a],
                normal: computed_normal,
                color: [0.8, 0.8, 0.8, 1.0], // Default gray
            };
            // Connect quads between this ring and the previous
        }
    }
}
```

**Acceptance:**
- Linear extrude produces clean box-like shapes from outlines
- Taper slider smoothly scales the back face
- Lathe produces rotationally symmetric shapes (columns, vases, trunks)
- Sweep parameter allows partial rotation (e.g., 180° for half-shapes)
- `cargo check` passes

---

### Story 8: Sculpting Bridge

**What:** Create `src/game/asset_editor/sculpt_bridge.rs` that wraps the existing `SculptingManager` from `engine/src/render/sculpting.rs` for use in Stage 3. Also add sphere add/subtract tools using `smooth_union()` and `smooth_subtraction()` from `sdf_operations.rs`.

**Files:**
- Create `src/game/asset_editor/sculpt_bridge.rs`
- Modify `src/game/asset_editor/mod.rs` — route stage 3 input to sculpt bridge

**Details:**

The bridge struct holds a reference to the existing SculptingManager and adds editor-specific tools:

```rust
pub struct SculptBridge {
    pub active_tool: SculptTool,
    pub brush_radius: f32,      // For smooth/add/subtract sphere
    pub smooth_factor: f32,     // SDF smooth_union k value
}

pub enum SculptTool {
    FaceExtrude,    // Existing: ExtrusionOperation
    VertexPull,     // Existing: VertexSelection
    EdgePull,       // Existing: EdgeSelection
    Smooth,         // NEW: Average vertex positions with neighbors
    AddSphere,      // NEW: smooth_union with sphere SDF at cursor
    SubtractSphere, // NEW: smooth_subtraction with sphere SDF at cursor
}
```

**Smooth brush** algorithm:
```rust
fn smooth_brush(mesh: &mut Mesh, center: Vec3, radius: f32, strength: f32) {
    for vertex in &mut mesh.vertices {
        let pos = Vec3::from(vertex.position);
        let dist = pos.distance(center);
        if dist < radius {
            let weight = 1.0 - (dist / radius);  // Falloff
            // Average with neighbors (find adjacent vertices via shared triangles)
            let avg = average_neighbor_positions(mesh, vertex_index);
            vertex.position = Vec3::lerp(pos, avg, weight * strength).into();
        }
    }
}
```

**Acceptance:**
- Face extrusion from existing SculptingManager works in editor context
- AddSphere stamps a sphere and re-meshes via SDF smooth_union + MarchingCubes
- SubtractSphere carves and re-meshes
- Smooth brush relaxes vertex positions
- `cargo check` passes

---

### Story 9: Vertex Color Painting

**What:** Create `src/game/asset_editor/paint.rs` with color brush, fill, gradient, and eyedropper tools. Colors are stored per-vertex in `Vertex.color`.

**Files:**
- Create `src/game/asset_editor/paint.rs`
- Modify `src/game/asset_editor/mod.rs` — route stage 4 input to paint tools

**Details:**

**Color brush:**
```rust
fn paint_brush(mesh: &mut Mesh, hit_point: Vec3, brush: &BrushParams, color: [f32; 4]) {
    for vertex in &mut mesh.vertices {
        let pos = Vec3::from(vertex.position);
        let dist = pos.distance(hit_point);
        if dist < brush.radius {
            // Falloff based on hardness
            let t = dist / brush.radius;
            let alpha = if brush.hardness >= 1.0 {
                brush.opacity
            } else {
                brush.opacity * (1.0 - t.powf(1.0 / (1.0 - brush.hardness)))
            };
            // Blend vertex color toward brush color
            for i in 0..4 {
                vertex.color[i] = vertex.color[i] * (1.0 - alpha) + color[i] * alpha;
            }
        }
    }
}
```

**Flood fill:**
```rust
fn flood_fill(mesh: &mut Mesh, start_triangle: u32, color: [f32; 4], tolerance: f32) {
    // Build adjacency graph from triangle indices
    // BFS from start triangle, spread to neighbors if their color is within tolerance
    // Set all reached vertices to the fill color
}
```

**Gradient:**
- Click start point, drag to end point
- All vertices between start and end get interpolated color
- Direction = start→end vector, position along this axis determines blend

**Color palette presets** (from PRD):
- Trees: bark brown `[0.36, 0.23, 0.12, 1.0]`, leaf green `[0.24, 0.48, 0.17, 1.0]`
- Rock: granite `[0.43, 0.43, 0.43, 1.0]`, sandstone `[0.83, 0.65, 0.45, 1.0]`
- Wood: oak `[0.55, 0.41, 0.08, 1.0]`, pine `[0.40, 0.26, 0.13, 1.0]`

**Acceptance:**
- Color brush paints vertex colors with radius falloff
- Fill tool floods connected area
- Gradient applies linear color blend between two points
- Eyedropper samples color from clicked vertex
- `cargo check` passes

---

### Story 10: Undo/Redo System

**What:** Create `src/game/asset_editor/undo.rs` with a command stack supporting `Ctrl+Z` (undo) and `Ctrl+Y` (redo). Each operation stores the minimal state needed to reverse it.

**Files:**
- Create `src/game/asset_editor/undo.rs`
- Modify `src/game/asset_editor/mod.rs` — integrate undo into all stages

**Details:**

```rust
pub enum UndoCommand {
    // Canvas operations
    AddOutline { index: usize, outline: Outline2D },
    RemoveOutline { index: usize, outline: Outline2D },
    ModifyOutline { index: usize, before: Outline2D, after: Outline2D },

    // Extrusion operations
    ChangeExtrudeParams { before: ExtrudeParams, after: ExtrudeParams },

    // Sculpt operations
    MeshSnapshot { before: Mesh, after: Mesh },  // Full mesh before/after for sculpt ops

    // Paint operations
    VertexColors { before: Vec<[f32; 4]>, after: Vec<[f32; 4]> },
}

pub struct UndoStack {
    commands: Vec<UndoCommand>,
    cursor: usize,        // Points to next undo position
    max_size: usize,      // Default 50
}

impl UndoStack {
    pub fn push(&mut self, cmd: UndoCommand) {
        // Truncate any redo history beyond cursor
        self.commands.truncate(self.cursor);
        self.commands.push(cmd);
        self.cursor += 1;
        // Evict oldest if over max_size
        if self.commands.len() > self.max_size {
            self.commands.remove(0);
            self.cursor -= 1;
        }
    }

    pub fn undo(&mut self) -> Option<&UndoCommand> { ... }
    pub fn redo(&mut self) -> Option<&UndoCommand> { ... }
}
```

**Acceptance:**
- Ctrl+Z undoes last operation in any stage
- Ctrl+Y redoes
- Stack holds 50 operations max, oldest evicted
- Undo restores previous state correctly (outlines, mesh, colors)
- `cargo check` passes

---

### Story 11: Asset File Save/Load (.btasset)

**What:** Create `src/game/asset_editor/asset_file.rs` implementing the binary `.btasset` format for saving and loading assets. Also create the save dialog UI in stage 5.

**Files:**
- Create `src/game/asset_editor/asset_file.rs`
- Modify `src/game/asset_editor/mod.rs` — stage 5 renders save dialog

**Details:**

**Save:**
```rust
pub fn save_btasset(
    path: &str,
    mesh: &Mesh,
    metadata: &AssetMetadata,
    variety: &VarietyParams,
) -> Result<(), String> {
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;

    // 1. Compute sizes
    let vertex_data_size = mesh.vertices.len() * std::mem::size_of::<Vertex>();
    let index_data_size = mesh.indices.len() * std::mem::size_of::<u32>();
    let metadata_json = serde_json::to_string(metadata).unwrap();
    let variety_json = serde_json::to_string(variety).unwrap();

    let metadata_offset = 32 + vertex_data_size as u32 + index_data_size as u32;
    let variety_offset = metadata_offset + metadata_json.len() as u32;

    // 2. Write header
    let header = BtassetHeader {
        magic: BTASSET_MAGIC,
        version: BTASSET_VERSION,
        vertex_count: mesh.vertices.len() as u32,
        index_count: mesh.indices.len() as u32,
        metadata_offset,
        metadata_size: metadata_json.len() as u32,
        variety_offset,
        variety_size: variety_json.len() as u32,
    };
    file.write_all(bytemuck::bytes_of(&header))?;

    // 3. Write vertex data
    file.write_all(bytemuck::cast_slice(&mesh.vertices))?;

    // 4. Write index data
    file.write_all(bytemuck::cast_slice(&mesh.indices))?;

    // 5. Write metadata JSON
    file.write_all(metadata_json.as_bytes())?;

    // 6. Write variety JSON
    file.write_all(variety_json.as_bytes())?;

    Ok(())
}
```

**Load** follows the same layout in reverse.

**File location:** `assets/world/<category>/<name>.btasset`

**Acceptance:**
- Save produces valid .btasset binary file
- Load reconstructs identical Mesh + Metadata + VarietyParams
- Round-trip test: save → load → compare = identical
- `cargo check` passes

---

### Story 12: Asset Library Panel

**What:** Create `src/game/asset_editor/library.rs` with a browsable grid of saved assets. Toggle with F10. Grid shows asset names organized by category. Click to select, then place in world.

**Files:**
- Create `src/game/asset_editor/library.rs`
- Modify `src/game/asset_editor/mod.rs` — F10 toggles library panel

**Details:**

```rust
pub struct AssetLibrary {
    pub entries: Vec<AssetEntry>,
    pub visible: bool,
    pub selected: Option<usize>,
    pub filter_category: Option<AssetCategory>,
    pub search_text: String,
    pub scroll_offset: f32,
}

pub struct AssetEntry {
    pub id: String,
    pub name: String,
    pub category: AssetCategory,
    pub tags: Vec<String>,
    pub path: String,
    pub vertex_count: u32,
    pub bounds_size: Vec3,
}
```

**Library index:** Reads/writes `assets/world/library.json`.

**UI rendering:** Use `add_quad()` for panel background, `draw_text()` for labels. Grid layout: 4 columns, each cell 120x120 pixels. Category tabs across the top.

**Acceptance:**
- F10 toggles library panel overlay
- Panel shows all saved assets in grid
- Category filtering works
- Click selects an asset
- `cargo check` passes

---

### Story 13: Variety System

**What:** Create `src/game/asset_editor/variety.rs` implementing seed-based variation for placed assets. Each asset's `VarietyParams` controls scale, rotation, hue shift, and noise displacement. Seed is derived from world position for deterministic variation.

**Files:**
- Create `src/game/asset_editor/variety.rs`

**Details:**

```rust
/// Generate a variety transform from a seed
pub fn generate_variety(params: &VarietyParams, seed: u32) -> VarietyInstance {
    let mut rng = SimpleRng::new(seed);

    let scale_base = rng.range(params.scale_min, params.scale_max);
    let scale_y = scale_base * (1.0 + rng.range(-params.scale_y_bias, params.scale_y_bias));

    let y_rotation = if params.random_y_rotation {
        rng.range(0.0, std::f32::consts::TAU)
    } else { 0.0 };

    let tilt = rng.range(0.0, params.tilt_max_degrees.to_radians());
    let tilt_axis_angle = rng.range(0.0, std::f32::consts::TAU);

    let hue_shift = rng.range(-params.hue_shift_range, params.hue_shift_range);
    let sat_shift = rng.range(-params.saturation_range, params.saturation_range);
    let bright_shift = rng.range(-params.brightness_range, params.brightness_range);

    VarietyInstance {
        scale: Vec3::new(scale_base, scale_y, scale_base),
        rotation_y: y_rotation,
        tilt_angle: tilt,
        tilt_axis: tilt_axis_angle,
        hue_shift,
        saturation_shift: sat_shift,
        brightness_shift: bright_shift,
        noise_seed: rng.next_u32(),
    }
}

/// Simple deterministic RNG (xorshift32)
struct SimpleRng { state: u32 }
impl SimpleRng {
    fn new(seed: u32) -> Self { Self { state: seed.max(1) } }
    fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }
    fn range(&mut self, min: f32, max: f32) -> f32 {
        let t = self.next_u32() as f32 / u32::MAX as f32;
        min + t * (max - min)
    }
}
```

**Seed derivation:** `seed = hash(world_x as u32, world_z as u32)` — same position always gives same variation.

**Acceptance:**
- Same seed produces identical variation
- Different seeds produce visibly different instances
- All parameter ranges respected
- `cargo check` passes

---

### Story 14: World Placement System

**What:** Create `src/game/asset_editor/placement.rs` with ghost preview, click-to-place, scatter brush, and ground conforming. Uses `CreatureInstance` from `engine/src/render/instancing.rs` for rendering placed assets.

**Files:**
- Create `src/game/asset_editor/placement.rs`
- Modify `src/bin/battle_editor.rs` — render placed asset instances in editor preview

**Details:**

```rust
pub struct PlacementSystem {
    pub selected_asset: Option<String>,     // Asset ID from library
    pub ghost_position: Vec3,               // Preview position (follows cursor)
    pub ghost_rotation: f32,                // R key to rotate
    pub ghost_scale: f32,                   // [ / ] keys to scale
    pub placed_instances: Vec<PlacedAsset>,
    pub scatter_mode: bool,                 // Ctrl+Click for scatter
    pub scatter_radius: f32,               // Scroll to adjust
    pub scatter_density: f32,              // Instances per square meter
}

pub struct PlacedAsset {
    pub asset_id: String,
    pub position: Vec3,
    pub variety_seed: u32,
    pub manual_rotation: f32,
    pub manual_scale: f32,
}
```

**Ground conforming:**
- Ray-cast from cursor position downward to find terrain height
- Align asset Y axis to terrain normal (slight tilt to match slope)
- Configurable sink depth for partially-buried objects

**Scatter brush:**
- Circle brush on terrain surface
- Click-drag to populate with random instances
- Poisson disk sampling for natural distribution (not pure random)

**Acceptance:**
- Ghost preview follows cursor on terrain
- Click places asset at cursor position with variety applied
- R rotates preview, [ / ] scales
- Scatter brush places multiple instances within radius
- `cargo check` passes

---

### Story 15: Editor UI Panels (Tool Palette + Color Picker)

**What:** Create `src/game/asset_editor/ui_panels.rs` with the tool palette (left side), property panel (right side), and HSV color picker. All UI uses existing `add_quad()` + `draw_text()` primitives.

**Files:**
- Create `src/game/asset_editor/ui_panels.rs`
- Modify `src/game/asset_editor/mod.rs` — render UI panels when editor active

**Details:**

**Tool palette** (left side, vertical):
- One icon per tool for the current stage
- Highlight selected tool
- Keyboard shortcuts shown next to each tool

**Property panel** (right side, vertical):
- Stage 1: Grid size slider, symmetry toggle
- Stage 2: Inflation, Thickness, Profile sliders
- Stage 3: Brush radius, smooth factor sliders
- Stage 4: Color picker, brush radius, opacity, hardness sliders
- Stage 5: Category dropdown, name input, variety params

**HSV color picker:**
```
┌──────────────────┐
│  ┌────────┐  ┌─┐ │
│  │ SV box │  │H│ │
│  │        │  │u│ │
│  │        │  │e│ │
│  └────────┘  └─┘ │
│  [####] Opacity   │
│  Primary  Secondry│
│  Recent colors... │
└──────────────────┘
```

- SV box: 128x128 pixel quad with saturation on X, value on Y
- Hue bar: Vertical strip cycling through hues
- Click in SV box or hue bar to select color
- Recent colors: last 8 used colors as small swatches

**Acceptance:**
- Tool palette renders correctly for each stage
- Property panel shows appropriate sliders per stage
- HSV color picker allows intuitive color selection
- All controls respond to mouse clicks/drags
- `cargo check` passes

---

### Story 16: Integration + Preview Shader

**What:** Create `shaders/asset_preview.wgsl` for rendering the 3D asset preview in the editor. Wire all editor stages together in `mod.rs` so the full pipeline flows: draw → extrude → sculpt → color → save. Add GPU buffer management for canvas lines and 3D preview mesh.

**Files:**
- Create `shaders/asset_preview.wgsl`
- Modify `src/game/asset_editor/mod.rs` — complete render pipeline

**Details:**

**Preview shader** — simple lit shader with orbit camera:
```wgsl
struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    light_dir: vec3<f32>,
    ambient: f32,
}

@vertex fn vs_main(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec4<f32>) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4(position, 1.0);
    out.world_normal = normal;
    out.color = color;
    out.world_pos = position;
    return out;
}

@fragment fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.world_normal);
    let diffuse = max(dot(n, -uniforms.light_dir), 0.0);
    let lit = in.color.rgb * (uniforms.ambient + diffuse * 0.8);
    return vec4(lit, in.color.a);
}
```

**Buffer management:**
- Canvas lines: dynamic vertex buffer, rebuilt each frame from outline points
- 3D preview: vertex + index buffer, rebuilt only when mesh changes (extrude params change, sculpt operation, paint operation)
- Use `wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST` for dynamic updates

**Acceptance:**
- Full pipeline flows: draw outlines → extrude → sculpt → paint → save
- 3D preview renders with proper lighting
- Canvas renders 2D outlines with background image
- Transitions between stages preserve state
- `cargo check` passes
- `cargo build --bin battle_editor` succeeds
- `cargo build --bin battle_arena` still succeeds (no changes to it)

---

## Technical Considerations

### Performance
- **Marching Cubes resolution**: Default 48³ = 110,592 cells. At 60fps target, re-meshing takes ~5-15ms. Only re-mesh when parameters actually change (not every frame).
- **Canvas rendering**: Outlines are just a few hundred vertices of thin quads — negligible cost.
- **Sculpt operations**: Smooth brush iterates all vertices in radius. For meshes under 10K vertices (typical for game assets), this is fast enough for interactive use.
- **Background image**: Single textured quad — no performance concern.

### Memory
- **Undo stack**: MeshSnapshot stores full vertex array. For a 5K vertex mesh, each snapshot = 200KB. 50 snapshots = 10MB max. Acceptable.
- **Asset instances**: Each `CreatureInstance` = 48 bytes. 1000 placed instances = 48KB. Instanced rendering keeps GPU memory low.

### Existing Code Patterns to Follow
- **UI rendering**: Follow `terrain_editor.rs` pattern — generate vertex/index data, upload to GPU buffer, render in UI pass.
- **State management**: Follow `GameState` pattern from `state.rs` — editor is a field on `GameState`, updated each frame.
- **Shader format**: Match existing shader uniforms layout (16-byte aligned, `@group(0) @binding(0)`).
- **Vertex format**: Use `Vertex` (40 bytes: 3×f32 pos + 3×f32 normal + 4×f32 color) consistently.

### Dependencies
- **No new crate dependencies.** The `image` crate is already in `Cargo.toml` for skybox loading. `serde` + `serde_json` are already available via `glam`'s `serde` feature. `bytemuck` is already present.

---

## Non-Goals (This Phase)

- **Animation / rigging** — Static assets only (V1)
- **Texture painting** — Vertex colors only, no UV mapping
- **Multi-user collaboration** — Single-user editor
- **Import from external formats** (FBX, OBJ) — Draw everything in-engine
- **LOD generation** — Full mesh only for now (LOD is a polish item)
- **Billboard rendering** — No distance-based LOD sprites
- **Terrain-integrated assets** — Assets are separate objects, not blended into terrain mesh
