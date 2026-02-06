# US-P4-007: Linear Extrude + Lathe Revolution

## Description
Add linear extrusion and lathe revolution as two new extrusion methods in `src/game/asset_editor/extrude.rs`, complementing the pump/inflate extrusion from US-P4-006. Linear extrusion pushes a 2D outline along the Z axis with configurable depth and taper, using an SDF formulation evaluated via Marching Cubes. Lathe revolution spins a 2D outline profile around the Y axis to produce rotationally symmetric geometry, generating mesh directly (not via SDF/Marching Cubes) for efficiency and artifact-free results. Both methods integrate into the existing `ExtrudeMethod` enum and are controlled by UI sliders in Stage 2 (Extrude). The asset editor is a **separate binary** (`cargo run --bin battle_editor`); `battle_arena.rs` is **never modified**.

## The Core Concept / Why This Matters
Many common game assets have either extruded or rotationally symmetric geometry. Fence posts, walls, beams, pillars, and architectural elements are linear extrusions -- a 2D cross-section pushed along an axis. Columns, vases, tree trunks, barrels, bottles, and goblets are lathe revolutions -- a 2D profile spun around an axis. Without these two operations, artists must manually sculpt what should be procedurally generated geometry, wasting time and producing inconsistent results.

Linear extrusion with taper is particularly powerful because it enables a whole family of shapes from a single outline: taper=0.0 produces a uniform prism, intermediate taper values produce truncated pyramids and wedge shapes, and taper=1.0 tapers to a point. This covers everything from walls to roof peaks to chisel tips.

Lathe revolution is uniquely efficient: rotationally symmetric shapes can be fully defined by a single 2D profile curve (half the silhouette), and the mesh generation is deterministic with user-controlled resolution. Unlike SDF+Marching Cubes, direct mesh generation produces clean quad topology without marching cubes staircase artifacts, and the vertex/index count is predictable (segments x profile_points). Partial sweeps (less than 360 degrees) enable half-pipes, arches, and open shell shapes.

Together, linear extrude and lathe revolution dramatically expand the range of shapes the asset editor can produce from simple 2D outlines, covering the majority of hard-surface and architectural asset needs.

## Goal
Extend `src/game/asset_editor/extrude.rs` with `sdf_linear_extrude()` for tapered Z-axis extrusion (evaluated via Marching Cubes) and `lathe_mesh()` for direct mesh generation via Y-axis revolution. Add UI slider controls for depth, taper, segments, and sweep. Integrate both methods into the extrude module's mode-switching logic so the user can select Linear or Lathe from the `ExtrudeMethod` enum.

## Files to Create/Modify
- **Modify** `src/game/asset_editor/extrude.rs` -- Add `sdf_linear_extrude()` function, `lathe_mesh()` function, `recompute_lathe_normals()` helper, UI slider controls for depth/taper/segments/sweep, and integration into the extrude module's update/render flow
- **Modify** `src/game/asset_editor/mod.rs` -- Ensure `ExtrudeMethod::Linear` and `ExtrudeMethod::Lathe` are handled in Stage 2 rendering and input routing, update property panel to show mode-specific sliders

## Implementation Steps

### Step 1: Implement the `sdf_linear_extrude` function
This function defines the SDF for a 2D outline extruded along the Z axis with optional taper. The outline defines the cross-section at z=0. At z=depth, the outline is scaled by `(1.0 - taper)`. The SDF is the intersection of the scaled 2D outline distance and the Z-axis bounds.

```rust
/// Linear extrusion SDF: extrude a 2D outline along Z with optional taper.
///
/// - `p`: 3D sample point
/// - `outline`: the 2D cross-section (closed polygon from canvas)
/// - `depth`: extrusion distance along +Z (range: 0.1 to 10.0)
/// - `taper`: 0.0 = no taper (uniform prism), 1.0 = full taper (point at z=depth)
///
/// At z=0, the cross-section is the original outline at full scale.
/// At z=depth, the cross-section is scaled by (1.0 - taper).
/// The SDF combines the scaled 2D signed distance with Z-axis slab bounds.
pub fn sdf_linear_extrude(
    p: Vec3,
    outline: &Outline2D,
    depth: f32,
    taper: f32,
) -> f32 {
    // Clamp Z ratio to [0, 1] for taper interpolation
    let t = if depth > 0.0 { p.z.clamp(0.0, depth) / depth } else { 0.0 };

    // Scale factor at this Z height: lerps from 1.0 at z=0 to (1.0 - taper) at z=depth
    let scale = 1.0 - taper * t;

    // Guard against division by zero when taper = 1.0 and t = 1.0 (scale = 0)
    if scale < 1e-6 {
        return p.length(); // At the tip, everything is outside
    }

    // Scale XY coordinates inversely to test against the original outline
    // This is equivalent to shrinking the outline, but faster (avoids rebuilding outline)
    let p2d = Vec2::new(p.x / scale, p.y / scale);

    // Compute signed distance in 2D, then scale the result back
    let d_2d = outline.sdf_2d(p2d) * scale;

    // Z-axis bounding: signed distance to the [0, depth] slab
    let d_z = (p.z - depth * 0.5).abs() - depth * 0.5;

    // Intersection of 2D outline and Z slab (max = intersection in SDF algebra)
    d_2d.max(d_z)
}
```

**Key design decisions:**
- The taper convention is `0.0 = no taper` (uniform prism), `1.0 = full taper` (point). This is intuitive for the slider: drag right to make it pointier.
- The SDF scales the query point inversely rather than scaling the outline geometry, avoiding per-sample outline reconstruction.
- The `scale < 1e-6` guard handles the degenerate singularity where taper=1.0 at z=depth produces a zero-scale division.
- The Z bounds use signed distance to a slab centered at `depth/2` with half-thickness `depth/2`, which is the standard SDF for an axis-aligned slab.

### Step 2: Implement the `lathe_mesh` function (direct mesh generation)
Lathe generates mesh directly by sweeping a 2D profile around the Y axis. This avoids SDF/Marching Cubes entirely, producing clean quad-based topology with predictable vertex counts and no staircase artifacts.

```rust
/// Generate a mesh by revolving a 2D outline profile around the Y axis.
///
/// - `outline`: profile points where x = radius from Y axis, y = height along Y axis.
///   Points should be ordered along the profile (e.g., bottom to top).
/// - `segments`: number of radial divisions (clamped to 6..=64)
/// - `sweep`: degrees of revolution (0.0 to 360.0)
///
/// Returns a `Mesh` with position, normal, and default gray color per vertex.
/// For a full 360-degree sweep with matching first/last rings, the geometry
/// is seamlessly closed. For partial sweeps, open edges remain at the start
/// and end of the revolution.
pub fn lathe_mesh(
    outline: &Outline2D,
    segments: u32,
    sweep: f32,
) -> Mesh {
    let sweep_rad = sweep.to_radians();
    let profile = &outline.points;
    let seg_count = segments.clamp(6, 64);
    let ring_size = profile.len();

    // Pre-allocate: (seg_count + 1) rings of ring_size vertices each
    let total_verts = (seg_count as usize + 1) * ring_size;
    let total_indices = seg_count as usize * (ring_size - 1) * 6;
    let mut vertices: Vec<Vertex> = Vec::with_capacity(total_verts);
    let mut indices: Vec<u32> = Vec::with_capacity(total_indices);

    // --- Generate vertex rings ---
    for seg in 0..=seg_count {
        let angle = sweep_rad * (seg as f32 / seg_count as f32);
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        for (i, pt) in profile.iter().enumerate() {
            // Profile convention: x = radius from Y axis, y = height
            let x = pt.x * cos_a;
            let z = pt.x * sin_a;
            let y = pt.y;

            // Compute normal from profile tangent:
            // The profile tangent in 2D is (dr, dy) where dr = change in radius.
            // The outward-facing normal in profile space is (dy, -dr), normalized.
            // This is then rotated around Y by the current angle.
            let tangent_r = if i + 1 < ring_size {
                profile[i + 1].x - pt.x
            } else if i > 0 {
                pt.x - profile[i - 1].x
            } else {
                0.0
            };
            let tangent_y = if i + 1 < ring_size {
                profile[i + 1].y - pt.y
            } else if i > 0 {
                pt.y - profile[i - 1].y
            } else {
                1.0
            };

            // Profile normal in 2D: perpendicular to tangent = (tangent_y, -tangent_r)
            let pn_r = tangent_y;
            let pn_y = -tangent_r;
            let len = (pn_r * pn_r + pn_y * pn_y).sqrt();
            let (pnr, pny) = if len > 1e-6 {
                (pn_r / len, pn_y / len)
            } else {
                (1.0, 0.0) // Fallback: point outward radially
            };

            // Rotate the radial component of the normal around Y
            let normal = Vec3::new(pnr * cos_a, pny, pnr * sin_a).normalize();

            vertices.push(Vertex {
                position: [x, y, z],
                normal: [normal.x, normal.y, normal.z],
                color: [0.8, 0.8, 0.8, 1.0], // Default light gray
            });
        }
    }

    // --- Generate quad indices (two triangles per quad) ---
    let ring_size_u32 = ring_size as u32;
    for seg in 0..seg_count {
        for i in 0..ring_size_u32 - 1 {
            let curr = seg * ring_size_u32 + i;
            let next = (seg + 1) * ring_size_u32 + i;

            // Triangle 1: curr -> next -> curr+1
            indices.push(curr);
            indices.push(next);
            indices.push(curr + 1);

            // Triangle 2: curr+1 -> next -> next+1
            indices.push(curr + 1);
            indices.push(next);
            indices.push(next + 1);
        }
    }

    Mesh { vertices, indices }
}
```

**Key design decisions:**
- Direct mesh generation (not SDF) avoids marching cubes staircase artifacts and produces predictable topology.
- Normals are computed analytically from the profile tangent, rotated into 3D. This produces smooth shading immediately without a post-process step.
- The vertex layout uses the existing `Vertex` type from `src/game/types.rs` (40 bytes: position + normal + color) for full pipeline compatibility.
- Memory is pre-allocated with `Vec::with_capacity` for a single allocation with no reallocations during generation.
- For partial sweeps (< 360 degrees), the last ring is generated at the sweep angle, leaving open edges. Cap faces are handled in Step 3.

### Step 3: Add normal recomputation helper for sharp profiles
For profiles with sharp corners (e.g., a hexagonal column profile), the tangent-based normals from Step 2 may produce discontinuities. Provide a post-process function that averages face normals per vertex:

```rust
/// Recompute per-vertex normals by averaging adjacent face normals (area-weighted).
/// Use this as a fallback for lathe meshes with sharp profile corners.
fn recompute_lathe_normals(vertices: &mut [Vertex], indices: &[u32]) {
    // Zero all normals
    for v in vertices.iter_mut() {
        v.normal = [0.0, 0.0, 0.0];
    }

    // Accumulate area-weighted face normals at each vertex
    for tri in indices.chunks_exact(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let p0 = Vec3::from(vertices[i0].position);
        let p1 = Vec3::from(vertices[i1].position);
        let p2 = Vec3::from(vertices[i2].position);

        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let face_normal = edge1.cross(edge2); // Magnitude = 2x triangle area

        for &idx in &[i0, i1, i2] {
            vertices[idx].normal[0] += face_normal.x;
            vertices[idx].normal[1] += face_normal.y;
            vertices[idx].normal[2] += face_normal.z;
        }
    }

    // Normalize all vertex normals
    for v in vertices.iter_mut() {
        let n = Vec3::from(v.normal);
        let len = n.length();
        if len > 1e-6 {
            v.normal = (n / len).into();
        } else {
            v.normal = [0.0, 1.0, 0.0]; // Fallback: up
        }
    }
}
```

### Step 4: Add UI controls for linear extrude
Add two sliders for the linear extrude parameters, using the existing `UISlider` from `src/game/ui/slider.rs`. These appear in the right-side property panel when `ExtrudeMethod::Linear` is selected:

```rust
// In the extrude module's UI rendering for ExtrudeMethod::Linear:

// Depth slider: controls how far the outline is pushed along Z
let depth_slider = UISlider::new("Depth", 0.1, 10.0, params.depth);
// Default: 2.0
// Step: 0.1
// Tooltip: "Extrusion distance along Z axis"

// Taper slider: controls how much the far face shrinks
let taper_slider = UISlider::new("Taper", 0.0, 1.0, params.taper);
// Default: 0.0 (no taper = uniform prism)
// Step: 0.05
// Tooltip: "0 = uniform prism, 1 = taper to point"
```

**Slider behavior:**
- When the user adjusts depth or taper, set `extruder.dirty = true`
- On next frame, if dirty, re-evaluate the SDF and re-run Marching Cubes to regenerate the preview mesh
- The preview updates in real-time as sliders are dragged

### Step 5: Add UI controls for lathe revolution
Add two sliders for the lathe parameters. These appear in the right-side property panel when `ExtrudeMethod::Lathe` is selected:

```rust
// In the extrude module's UI rendering for ExtrudeMethod::Lathe:

// Segments slider: controls angular resolution (how smooth the revolution is)
let segments_slider = UISlider::new("Segments", 6.0, 64.0, params.segments as f32);
// Default: 16
// Step: 1.0 (integer steps)
// Tooltip: "Radial divisions (higher = smoother)"

// Sweep slider: controls how far around the Y axis to revolve
let sweep_slider = UISlider::new("Sweep", 0.0, 360.0, params.sweep_degrees);
// Default: 360.0 (full revolution)
// Step: 5.0
// Tooltip: "Degrees of rotation (360 = full circle)"
```

**Slider behavior:**
- Segment and sweep changes regenerate the lathe mesh directly via `lathe_mesh()` (no SDF involved)
- Since lathe mesh generation is O(segments * profile_points), regeneration is near-instant even at 64 segments with a complex profile
- The preview updates immediately on slider change

### Step 6: Integrate both methods into the extrude module update/render flow
Wire the new methods into the `ExtrudeMethod` enum dispatch logic. When the user switches modes, the property panel updates and the mesh is regenerated:

```rust
// In the Extruder's generate_preview or equivalent function:

match params.method {
    ExtrudeMethod::Pump => {
        // Existing: sdf_pumped() + MarchingCubes (from US-P4-006)
        let sdf = |p: Vec3| sdf_pumped(p, outline, params);
        let mc = MarchingCubes::new(params.mc_resolution);
        self.mesh = mc.generate_mesh(&sdf, bounds_min, bounds_max);
    }
    ExtrudeMethod::Linear => {
        // NEW: sdf_linear_extrude() + MarchingCubes
        let depth = params.depth;
        let taper = params.taper;
        let sdf = |p: Vec3| sdf_linear_extrude(p, outline, depth, taper);
        let mc = MarchingCubes::new(params.mc_resolution);
        // Compute bounds from outline bounding box + depth
        let ob = outline.bounding_box();
        let bounds_min = Vec3::new(ob.min.x - 0.5, ob.min.y - 0.5, -0.5);
        let bounds_max = Vec3::new(ob.max.x + 0.5, ob.max.y + 0.5, depth + 0.5);
        self.mesh = mc.generate_mesh(&sdf, bounds_min, bounds_max);
    }
    ExtrudeMethod::Lathe => {
        // NEW: direct mesh generation (no SDF, no MarchingCubes)
        self.mesh = lathe_mesh(outline, params.segments, params.sweep_degrees);
        // Optionally recompute normals for sharp-cornered profiles:
        // recompute_lathe_normals(&mut self.mesh.vertices, &self.mesh.indices);
    }
}

// Upload mesh to GPU and clear dirty flag
self.upload_to_gpu(device);
self.dirty = false;
```

**Mode switching UI:**
- The user selects between Pump, Linear, and Lathe via Tab key (cycles modes) or direct selection in the property panel
- The property panel dynamically shows the appropriate sliders for the selected method:
  - **Pump**: Inflation, Thickness, Profile (existing from US-P4-006)
  - **Linear**: Depth, Taper (this story)
  - **Lathe**: Segments, Sweep (this story)
- Switching modes triggers an immediate mesh regeneration with the new method

## Code Patterns

Existing types from the extrude module (established in US-P4-006):
```rust
// Already defined in extrude.rs by US-P4-006:
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrudeMethod {
    Pump,     // Inflate outline into organic 3D shape (US-P4-006)
    Linear,   // Extrude along Z axis with optional taper (this story)
    Lathe,    // Revolve around Y axis (this story)
}

pub struct ExtrudeParams {
    pub method: ExtrudeMethod,
    // Pump parameters (US-P4-006)
    pub inflation: f32,
    pub thickness: f32,
    pub profile: PumpProfile,
    // Linear extrude parameters (this story)
    pub depth: f32,             // 0.1-10.0, default 2.0
    pub taper: f32,             // 0.0-1.0, default 0.0
    // Lathe parameters (this story)
    pub segments: u32,          // 6-64, default 16
    pub sweep_degrees: f32,     // 0-360, default 360.0
    // Shared
    pub mc_resolution: u32,     // Marching Cubes resolution (default 48)
}
```

Vertex and Mesh types from `src/game/types.rs`:
```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],  // 12 bytes
    pub normal: [f32; 3],    // 12 bytes
    pub color: [f32; 4],     // 16 bytes
}
// Total: 40 bytes per vertex

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}
```

Marching Cubes integration (same pattern as US-P4-006 pump/inflate):
```rust
let mc = MarchingCubes::new(params.mc_resolution); // default 48
let sdf = |p: Vec3| sdf_linear_extrude(p, outline, depth, taper);
let mesh = mc.generate_mesh(&sdf, bounds_min, bounds_max);
```

UISlider from `src/game/ui/slider.rs`:
```rust
let slider = UISlider::new("Label", min, max, current_value);
// Render slider, handle mouse drag to update value
```

## Acceptance Criteria
- [ ] `sdf_linear_extrude()` function exists in `extrude.rs` with `outline`, `depth`, and `taper` parameters
- [ ] Linear extrude produces clean box-like shapes from rectangular outlines (taper=0.0)
- [ ] Taper slider smoothly scales the far face from full size (taper=0.0) to a point (taper=1.0)
- [ ] Intermediate taper values (e.g., 0.5) produce truncated pyramid shapes
- [ ] `lathe_mesh()` function generates mesh directly (no SDF, no Marching Cubes) from outline profile
- [ ] Lathe creates rotationally symmetric shapes (columns, vases, tree trunks) from a 2D profile
- [ ] Lathe normals are smooth and correct (no dark faces or inverted normals)
- [ ] Segments parameter controls radial resolution (clamped to 6-64)
- [ ] Sweep parameter allows partial rotation (0-360 degrees) with clean open edges
- [ ] UI shows Depth and Taper sliders when Linear method is selected
- [ ] UI shows Segments and Sweep sliders when Lathe method is selected
- [ ] Mode switching between Pump, Linear, and Lathe updates the property panel and regenerates the mesh
- [ ] Existing pump/inflate mode (US-P4-006) still works correctly after changes
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo check --bin battle_arena` passes with 0 errors (unchanged)

## Verification Commands
- `cmd`: `grep -c 'sdf_linear_extrude\|lathe_mesh' /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `Both sdf_linear_extrude and lathe_mesh functions exist`
- `cmd`: `grep -c 'taper' /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `Taper parameter is implemented in linear extrude`
- `cmd`: `grep -c 'sweep' /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `Sweep parameter is implemented in lathe`
- `cmd`: `grep -c 'segments' /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `Segments parameter controls lathe radial resolution`
- `cmd`: `grep -c 'depth' /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `Depth parameter controls extrusion distance`
- `cmd`: `grep -c 'recompute_lathe_normals\|face_normal\|cross' /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `Normal computation for lathe mesh is implemented`
- `cmd`: `grep -c 'pub mod extrude' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `Extrude module is registered in mod.rs`
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with linear extrude and lathe`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles unchanged`

## Success Looks Like
The artist opens the asset editor and switches to Stage 2 (Extrude). They draw a rectangular outline in Stage 1 and return to Stage 2. They select "Linear" from the extrude method selector (Tab key or panel). A clean box shape appears in the 3D preview. They adjust the depth slider from 2.0 to 5.0 and the box stretches along Z. They increase the taper slider -- the far face smoothly shrinks, creating a truncated pyramid. At taper=1.0, the shape tapers to a sharp point like a pyramid. At taper=0.5, they get a truncated wedge.

They then switch to "Lathe" mode. They draw a new profile in Stage 1 -- a vase silhouette with a narrow neck and wide body. The profile's X coordinates define radius from the Y axis, Y coordinates define height. A full 360-degree vase mesh appears instantly in the preview, smooth and symmetric with correct normals and lighting. They reduce the sweep to 270 degrees and see a three-quarter revolution with clean open edges. They increase segments from 16 to 32 and watch the mesh become visibly smoother. They reduce to 6 segments and the shape becomes faceted like a hexagonal column. They switch back to Pump mode and confirm that the original inflate behavior still works as before. All three modes feel responsive and produce artifact-free geometry ready for sculpting and painting in later stages.

## Dependencies
- Depends on: US-P4-006 (needs `extrude.rs` foundation with `Extruder` struct, `ExtrudeMethod` enum, `ExtrudeParams` struct, `PumpProfile` enum, Marching Cubes integration, `sdf_pumped()` function, 2D SDF computation from outline, and GPU buffer upload)

## Complexity
- Complexity: normal
- Min iterations: 1
