# Asset Editor Tool — Product Requirements Document

## 1. Overview

The **Asset Editor** is an in-engine tool for creating game assets through a 2D-to-3D pipeline. Artists and developers draw 2D outlines, extrude/inflate them into 3D shapes, sculpt details, apply colors and materials, then save them as reusable world assets.

This tool is a **separate binary** (`cargo run --bin battle_editor`) that shares the same engine library as the game. It uses the same SDF pipeline, rendering, and mesh systems. Assets created here are saved as `.btasset` files and directly usable in the game world without any export/import step.

### Goals

- Create any game asset visually: trees, grass, rocks, structures, props, decorations
- Draw 2D silhouette -> pump/extrude to 3D -> sculpt -> color -> save
- Built-in variety system: each saved asset generates visual variations automatically
- Asset library with categories, browsing, and drag-to-place
- Zero external tools required — everything happens in-engine

### Non-Goals (for V1)

- Animation / rigging (static assets only)
- Texture painting (vertex colors only for V1)
- Multi-user collaboration
- Import from external 3D formats (FBX, OBJ, etc.)

---

## 2. Pipeline Stages

The asset creation flow moves through five stages. Each stage has a dedicated UI mode. The user can go back to any previous stage to iterate.

```
┌──────────┐    ┌──────────────┐    ┌───────────┐    ┌──────────┐    ┌──────────┐
│ 2D Draw  │───>│ Extrude/Pump │───>│  Sculpt   │───>│  Color   │───>│   Save   │
│ (outline)│    │  (to 3D)     │    │ (detail)  │    │ (paint)  │    │ (library)│
└──────────┘    └──────────────┘    └───────────┘    └──────────┘    └──────────┘
```

### Stage 1: 2D Drawing Canvas

**Purpose**: Capture the asset's silhouette / profile shape.

**Entry**: Launch with `cargo run --bin battle_editor`. The editor opens its own window with an orthographic front view and grid background.

**Drawing tools**:

| Tool | Key | Description |
|------|-----|-------------|
| Freehand | `D` | Click-drag to draw smooth curves. Mouse movements are recorded as a polyline, then simplified (Ramer-Douglas-Peucker) to reduce point count. |
| Line | `L` | Click two points to draw a straight line segment. |
| Arc | `A` | Click three points (start, midpoint, end) to draw a smooth arc. |
| Eraser | `E` | Click-drag to remove nearby line segments. |
| Mirror | `M` | Toggle X-axis symmetry. Drawing on one side automatically mirrors to the other — essential for symmetric assets like trees and grass. |

**Canvas**:
- Grid with snapping (toggle with `G`)
- Zoom with scroll wheel (orthographic scale)
- Pan with middle-mouse drag
- Canvas size: conceptually infinite, but the grid shows a 20x20 unit area by default
- Background: light gray with darker grid lines at 1-unit intervals

**Outline rules**:
- The drawing must form a closed polygon (the tool auto-closes when the endpoint is near the start)
- Multiple separate closed outlines are allowed (e.g., a tree trunk + separate canopy outline)
- Each outline is stored as a `Vec<Vec2>` of control points
- Self-intersections are allowed (resolved during extrusion)

**Output**: One or more named 2D outline polygons, stored as part of the in-progress asset.

**Data structure**:
```rust
struct Outline2D {
    name: String,           // e.g. "trunk", "canopy"
    points: Vec<Vec2>,      // Closed polygon vertices (screen-space, then normalized)
    is_closed: bool,
    mirror_x: bool,         // Was this drawn with X symmetry?
}

struct AssetDraft {
    name: String,
    category: AssetCategory,
    outlines: Vec<Outline2D>,
    // Populated in later stages:
    mesh: Option<Mesh>,
    variety: VarietyParams,
}
```

### Stage 2: Extrude / Pump to 3D

**Purpose**: Convert the 2D outline into a 3D volume.

**Methods**:

#### A. Pump / Inflate
The primary method. Takes a 2D outline and generates a 3D SDF where the distance field is derived from the outline boundary.

**How it works**:
1. For each point in 3D space, compute the minimum distance to the 2D outline (projected onto the XY plane)
2. The Z-depth is determined by a "thickness profile" — how deep the shape is at each point
3. Default: elliptical cross-section (thickest at center, zero at outline edge)
4. The user controls the inflation amount with a slider (0 = flat, 1 = fully rounded)

**UI controls**:
- `Inflation` slider (0.0 – 1.0): How much the shape puffs out
- `Thickness` slider (0.1 – 5.0): Maximum depth
- `Profile` dropdown: Elliptical, Flat (constant depth), Pointed (like a leaf)

**SDF formulation**:
```
sdf_pumped(p) = distance_to_outline_2d(p.xy) - thickness * profile(distance_to_center)
```

This produces a smooth, organic shape from any outline — ideal for trees, leaves, rocks.

#### B. Extrude (Linear)
Simple extrusion along the Z-axis for architectural/structural assets.

**How it works**:
1. The 2D outline defines the cross-section
2. Extrude it a fixed depth along Z
3. Optionally taper (scale outline smaller at the back)

**UI controls**:
- `Depth` slider
- `Taper` slider (1.0 = no taper, 0.0 = taper to point)

#### C. Lathe (Revolution)
Rotate the outline around the Y-axis to create radially symmetric shapes (vases, columns, tree trunks).

**How it works**:
1. The outline's X coordinate becomes the radius
2. The outline's Y coordinate stays as height
3. Sweep 360 degrees around Y

**UI controls**:
- `Segments` slider (6 – 64): Angular resolution
- `Sweep` slider (0 – 360): Partial rotation for open shapes

#### Integration with existing systems

All three methods ultimately produce an SDF representation. This connects to the existing engine pipeline:

- **SDF evaluation**: `engine/src/render/sdf_operations.rs` — smooth_union, subtraction for combining parts
- **Marching Cubes**: `engine/src/render/marching_cubes.rs` — converts SDF to triangle mesh at configurable resolution (default 48³)
- **SDF baking**: `engine/src/render/sdf_baker.rs` — bakes complex SDFs to 64³ voxel bricks for GPU caching

### Stage 3: Sculpting

**Purpose**: Refine the 3D shape with interactive tools.

**Tools** (extending the existing `SculptingManager` from `engine/src/render/sculpting.rs`):

| Tool | Description | Existing Code |
|------|-------------|---------------|
| **Face Extrude** | Select a face, drag to extrude new geometry | `ExtrusionOperation` in `sculpting.rs` |
| **Pull Vertex** | Grab a vertex and drag to deform | `VertexSelection` (planned in sculpting.rs) |
| **Pull Edge** | Grab an edge and drag to deform | `EdgeSelection` (planned in sculpting.rs) |
| **Smooth** | Brush that smooths/relaxes nearby vertices | New — averages vertex positions with neighbors |
| **Pinch** | Brush that pulls vertices toward the center of the brush | New — moves vertices toward brush center |
| **Flatten** | Brush that flattens vertices to a common plane | New — projects vertices onto averaged plane |
| **Add Sphere** | Stamp a sphere SDF at cursor position (smooth-union with existing shape) | Uses `smooth_union()` from `sdf_operations.rs` |
| **Subtract Sphere** | Carve a sphere out of the shape | Uses `smooth_subtraction()` from `sdf_operations.rs` |

**Workflow**:
1. The shape from Stage 2 is displayed in perspective view
2. Camera orbits around the shape (same as the lava_water_3d.html orbit camera)
3. User selects a tool and clicks/drags on the shape surface
4. After each edit, the SDF is re-evaluated and the mesh updated via Marching Cubes
5. For performance, edits use a local bounding box — only re-mesh the affected region

**Undo/Redo**:
- Each sculpt operation is stored as a command in an undo stack
- `Ctrl+Z` / `Ctrl+Y` to undo/redo
- Stack limit: 50 operations

### Stage 4: Coloring / Painting

**Purpose**: Apply colors and material properties to the asset surface.

**Approach**: Per-vertex coloring (matching the existing `Vertex.color: [f32; 4]` format).

**Tools**:

| Tool | Description |
|------|-------------|
| **Color Brush** | Paint vertex colors with a round brush. Configurable radius and opacity. |
| **Fill Region** | Click a face/region to flood-fill with selected color. |
| **Gradient** | Apply a vertical/horizontal/radial gradient between two colors. |
| **Material Brush** | Set roughness and metallic values per-vertex (uses `GpuEntity` material properties). |

**Color Palette**:
- Preset palettes per category:
  - **Trees**: bark brown (#5C3A1E), leaf green (#3D7A2B), autumn orange (#C46210), dry yellow (#B5A642)
  - **Grass**: fresh green (#4CAF50), dry (#8B7D3C), wildflower purple (#7B1FA2), frost blue (#B3E5FC)
  - **Rock**: granite gray (#6E6E6E), sandstone (#D4A574), slate (#4A5568), mossy (#556B2F)
  - **Wood**: oak (#8B6914), pine (#654321), birch (#F5F0E1), mahogany (#4E1609)
- Custom color picker (HSV wheel + brightness slider)
- Eyedropper tool (`I`) to sample existing colors

**Data**: Colors are stored per-vertex in the mesh. When the asset is saved, the vertex colors are part of the mesh data.

### Stage 5: Save / Asset Library

**Purpose**: Save the finished asset and manage the asset collection.

**Save dialog**:
- Name (required): e.g., "Oak Tree", "Tall Grass", "Mossy Boulder"
- Category (required): Tree, Grass, Rock, Structure, Prop, Decoration
- Tags (optional): searchable keywords
- Variety parameters (see Section 3)

**File format**: Custom binary + JSON metadata (see Section 5 for full spec).

**Asset Library panel** (toggleable with `F10`):
- Grid of thumbnail previews organized by category
- Search bar with tag filtering
- Click to select, then click in world to place
- Drag-and-drop from library panel to world viewport
- Right-click for: Edit, Duplicate, Delete, Export

**File location**: `assets/world/<category>/<name>.btasset`

---

## 3. Variety System

Every saved asset can generate visual variations automatically. This prevents repetitive-looking worlds where every tree is identical.

### Variety Parameters

Each asset defines these randomization ranges:

```rust
struct VarietyParams {
    // Scale variation
    scale_min: f32,          // e.g., 0.7 (70% of original size)
    scale_max: f32,          // e.g., 1.3 (130%)
    scale_y_bias: f32,       // Extra vertical stretch variation (0.0 = uniform, 0.3 = +30% height var)

    // Rotation
    random_y_rotation: bool, // Rotate randomly around Y axis (almost always true)
    tilt_max_degrees: f32,   // Max random tilt off vertical (e.g., 5.0 for slight lean)

    // Color variation
    hue_shift_range: f32,    // +/- degrees of hue shift (e.g., 15.0)
    saturation_range: f32,   // +/- saturation adjustment (e.g., 0.1)
    brightness_range: f32,   // +/- brightness adjustment (e.g., 0.1)

    // Shape variation (vertex displacement)
    noise_displacement: f32, // Amount of Perlin noise displacement on vertices (0.0 = none, 0.1 = subtle)
    noise_frequency: f32,    // Frequency of displacement noise (higher = more detailed bumps)
}
```

### How Variety Works at Placement Time

When an asset is placed in the world:

1. A **seed** is derived from the world position (deterministic — same position always gives same variation)
2. The seed generates random values for each variety parameter
3. The base mesh is **instanced** with a per-instance transform (scale, rotation, tilt) and color shift
4. Optionally, the mesh vertices are displaced by seeded Perlin noise for shape variation

This means:
- 1 asset definition can look like 100 different objects in the world
- No extra mesh data stored — variation is computed from seed + params
- Rebuilding the world from the same seed produces identical results

### Presets

Common variety presets:

| Preset | Scale | Y-Rotation | Tilt | Hue Shift | Noise |
|--------|-------|------------|------|-----------|-------|
| Tree (natural) | 0.6–1.4 | Yes | 5 deg | +/-20 deg | 0.05 |
| Grass (field) | 0.5–1.2 | Yes | 15 deg | +/-10 deg | 0.02 |
| Rock (scattered) | 0.4–1.8 | Yes | 30 deg | +/-5 deg | 0.08 |
| Structure (placed) | 1.0 | No | 0 deg | 0 deg | 0.0 |

---

## 4. Technical Architecture

### System Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     Asset Editor Tool                        │
│                                                              │
│  ┌──────────┐  ┌───────────┐  ┌─────────┐  ┌────────────┐  │
│  │ 2D Canvas│  │ Extrude/  │  │ Sculpt  │  │ Color/     │  │
│  │ (input)  │──│ Pump (SDF)│──│ (modify)│──│ Paint      │  │
│  └──────────┘  └───────────┘  └─────────┘  └────────────┘  │
│        │              │             │              │         │
└────────┼──────────────┼─────────────┼──────────────┼────────┘
         │              │             │              │
         v              v             v              v
┌─────────────────────────────────────────────────────────────┐
│                    Engine Systems                            │
│                                                              │
│  ┌──────────────────┐  ┌──────────────────────────────────┐ │
│  │ SDF Pipeline     │  │ Rendering                        │ │
│  │                  │  │                                  │ │
│  │ sdf_operations   │  │ Main mesh pipeline (Vertex)      │ │
│  │ marching_cubes   │  │ SDF raymarcher (GpuEntity)       │ │
│  │ sdf_baker        │  │ Instancing (InstanceBuffer)      │ │
│  │ sculpting        │  │ Material system                  │ │
│  └──────────────────┘  └──────────────────────────────────┘ │
│                                                              │
│  ┌──────────────────┐  ┌──────────────────────────────────┐ │
│  │ UI System        │  │ Storage                          │ │
│  │                  │  │                                  │ │
│  │ UISlider         │  │ Asset files (.btasset)           │ │
│  │ draw_text()      │  │ Asset library index              │ │
│  │ add_quad()       │  │ Thumbnail cache                  │ │
│  │ TerrainEditorUI  │  │ Undo/redo stack                  │ │
│  └──────────────────┘  └──────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

### Existing Code Integration Points

| System | File | What It Provides |
|--------|------|-----------------|
| SDF evaluation | `engine/src/render/sdf_operations.rs` | `smooth_union()`, `subtraction()`, `intersection()` for combining shapes |
| Sculpting | `engine/src/render/sculpting.rs` | `SculptingManager`, `ExtrusionOperation`, face/edge/vertex selection |
| Mesh conversion | `engine/src/render/marching_cubes.rs` | `MarchingCubes::generate_mesh()` — SDF to triangles at configurable resolution |
| SDF baking | `engine/src/render/sdf_baker.rs` | `BrickCache` — GPU-cached 64³ SDF volumes for performance |
| Entity storage | `engine/src/render/entities.rs` | `GpuEntity` with material properties, noise displacement, LOD |
| Building blocks | `src/game/building/blocks.rs` | `BlockShape` primitives (Cube, Wall, Floor, Ramp, Stairs, Arch, Column) |
| Instancing | `engine/src/render/instancing.rs` | `InstanceBuffer` for rendering many copies of the same mesh efficiently |
| UI primitives | `src/game/ui/slider.rs` | `UISlider` — reusable slider component |
| UI text | `src/game/ui/text.rs` | `draw_text()`, `get_char_bitmap()` — bitmap font rendering |
| UI panels | `src/game/ui/terrain_editor.rs` | `TerrainEditorUI` — reference implementation for a multi-slider editor panel |
| Mesh types | `src/game/types.rs` | `Vertex` (pos + normal + color), `Mesh` (verts + indices) |

### New Code Required

| Module | Purpose |
|--------|---------|
| `src/game/asset_editor/mod.rs` | Main editor state machine, mode switching |
| `src/game/asset_editor/canvas_2d.rs` | 2D drawing tools, outline storage, symmetry |
| `src/game/asset_editor/extrude.rs` | Pump/extrude/lathe algorithms |
| `src/game/asset_editor/paint.rs` | Color brush, fill, gradient tools |
| `src/game/asset_editor/library.rs` | Asset save/load, library browsing, thumbnails |
| `src/game/asset_editor/variety.rs` | Variety parameter system, seed-based variation |
| `src/game/asset_editor/ui.rs` | Editor-specific UI panels (tool palette, property panel, color picker) |

### Separate Binary Architecture

The asset editor runs as a **separate binary** (`src/bin/battle_editor.rs`), not as a mode inside `battle_arena.rs`. This avoids bloating the game binary and keeps concerns separated.

**`battle_editor.rs` responsibilities:**
- Own winit window + event loop (window title: "Battle Tök — Asset Editor")
- Own wgpu device/surface initialization
- Route keyboard/mouse input to `AssetEditor` state machine
- Render editor stages (2D canvas, 3D preview, UI panels)
- No game logic (no economy, population, combat, terrain)

**Shared code** (via engine library + `src/game/asset_editor/` module):
- `AssetEditor` struct holds all editor state
- All 5 pipeline stages as sub-modules
- SDF operations, Marching Cubes, sculpting, instancing from engine
- UI primitives (`UISlider`, `draw_text`, `add_quad`) from game module

When the editor is running:
- Camera is always in orbit mode (around the asset being edited)
- The tool palette UI is shown
- The asset preview renders in the center of the viewport
- Keyboard shortcuts switch between pipeline stages (1-5)

---

## 5. Asset File Format

### `.btasset` Binary Format

```
┌────────────────────────────────────────┐
│ Header (32 bytes)                       │
│   magic: [u8; 4]  = "BTAS"            │
│   version: u32     = 1                  │
│   vertex_count: u32                     │
│   index_count: u32                      │
│   metadata_offset: u32                  │
│   metadata_size: u32                    │
│   variety_offset: u32                   │
│   variety_size: u32                     │
├────────────────────────────────────────┤
│ Vertex Data (vertex_count * 40 bytes)   │
│   position: [f32; 3]  (12 bytes)       │
│   normal: [f32; 3]    (12 bytes)       │
│   color: [f32; 4]     (16 bytes)       │
├────────────────────────────────────────┤
│ Index Data (index_count * 4 bytes)      │
│   indices: [u32; index_count]           │
├────────────────────────────────────────┤
│ Metadata (JSON, UTF-8)                  │
│   name, category, tags, bounds,         │
│   creation_date, thumbnail_hash         │
├────────────────────────────────────────┤
│ Variety Params (bincode, fixed size)    │
│   VarietyParams struct (see Section 3) │
└────────────────────────────────────────┘
```

### Metadata JSON Example

```json
{
  "name": "Oak Tree",
  "category": "tree",
  "tags": ["deciduous", "forest", "large"],
  "bounds": {
    "min": [-2.5, 0.0, -2.5],
    "max": [2.5, 8.0, 2.5]
  },
  "vertex_count": 1247,
  "triangle_count": 2086,
  "created": "2026-02-06T12:00:00Z",
  "modified": "2026-02-06T14:30:00Z"
}
```

### Asset Library Index

A JSON file at `assets/world/library.json` indexes all assets for fast browsing:

```json
{
  "assets": [
    {
      "id": "oak_tree_01",
      "path": "assets/world/tree/oak_tree.btasset",
      "name": "Oak Tree",
      "category": "tree",
      "tags": ["deciduous", "forest", "large"],
      "bounds_size": [5.0, 8.0, 5.0],
      "vertex_count": 1247
    }
  ]
}
```

---

## 6. Placement System

### Manual Placement
- Select asset from library panel
- Click in the world to place an instance
- Ghost preview shows the asset at cursor position before placing
- Hold `Shift` while placing to keep the asset selected for rapid placement
- `R` to rotate preview, `[` / `]` to scale preview

### Scatter Brush
- Select asset + switch to scatter mode (`Ctrl+Click`)
- A circular brush appears on the terrain
- Click-drag to scatter instances within the brush radius
- Controls:
  - Brush radius: scroll wheel
  - Density: slider (instances per square meter)
  - Random seed: auto-incremented or manual

### Ground Conforming
- Assets snap to terrain height at their placement position
- The Y-axis is aligned to the terrain normal (slight tilt to match slope)
- Trees/grass root at Y=0 of the asset (bottom of trunk/stem)
- Rocks can be partially buried (configurable sink depth)

### Instanced Rendering
- All instances of the same asset share one vertex/index buffer
- Per-instance data (transform + variety seed) stored in instance buffer
- Uses the existing `InstanceBuffer` from `engine/src/render/instancing.rs`
- Supports up to 1024 instances per asset type per chunk

### LOD (Level of Detail)
- Distance-based LOD:
  - Near (< 30m): Full mesh
  - Medium (30–80m): Simplified mesh (50% triangles)
  - Far (80–200m): Billboard (2D sprite facing camera)
  - Very far (> 200m): Not rendered (behind steam wall anyway)
- LOD meshes generated automatically from the full mesh using edge collapse
- Billboard textures rendered from the asset at save time (4 angles)

### Visibility Culling
- Assets behind the steam wall are never rendered
- Frustum culling per-instance (standard)
- Chunk-based spatial partitioning for efficient queries

---

## 7. Keyboard Shortcuts Summary

| Key | Action |
|-----|--------|
| `Esc` | Close editor / go back |
| `F10` | Toggle Asset Library panel |
| `1-5` | Switch pipeline stage (Draw / Extrude / Sculpt / Color / Save) |
| `D` | Freehand draw tool |
| `L` | Line tool |
| `A` | Arc tool |
| `E` | Eraser tool |
| `M` | Toggle mirror symmetry |
| `G` | Toggle grid snapping |
| `I` | Eyedropper (sample color) |
| `B` | Color brush |
| `F` | Fill region |
| `Ctrl+Z` | Undo |
| `Ctrl+Y` | Redo |
| `Ctrl+S` | Save asset |
| `Delete` | Delete selected outline/element |
| `Tab` | Cycle through sub-tools |
| `Scroll` | Brush size / zoom |

---

## 8. Implementation Phases

### Phase 1: Foundation (1-2 days)
- `battle_editor` binary with own window + orbit camera
- 2D canvas with freehand drawing and line tools
- Basic outline storage and display
- Simple pump/inflate to 3D using SDF
- Marching Cubes mesh preview

### Phase 2: Sculpting (1-2 days)
- Face extrusion (already exists in `SculptingManager`)
- Sphere add/subtract SDF operations
- Smooth brush
- Undo/redo stack

### Phase 3: Coloring (1 day)
- Color brush with radius/opacity
- Fill tool
- Preset color palettes
- HSV color picker UI

### Phase 4: Save/Load + Library (1-2 days)
- .btasset file format implementation
- Save dialog UI
- Library panel with grid view
- Load and place assets in world

### Phase 5: Variety + Placement (1-2 days)
- Variety parameter system
- Seed-based randomization at placement
- Scatter brush for mass placement
- Ground conforming

### Phase 6: Polish (1 day)
- LOD generation
- Billboard rendering for far assets
- Thumbnail generation
- Mirror symmetry for 2D drawing
- Lathe revolution tool

**Total estimated effort: 7-10 days**
