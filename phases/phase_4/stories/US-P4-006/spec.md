# US-P4-006: Pump/Inflate Extrusion (SDF-based)

## Description
Create `src/game/asset_editor/extrude.rs` with the pump/inflate extrusion algorithm that converts 2D outlines into 3D meshes using signed distance fields (SDFs). The algorithm projects each 3D sample point onto the XY plane, computes the signed distance to the 2D outline boundary, then modulates an allowable Z-depth using a thickness profile (Elliptical, Flat, or Pointed). The resulting 3D scalar field is fed into the engine's existing `MarchingCubes` (from `engine/src/render/marching_cubes.rs`) to extract an isosurface as a triangle mesh. Three UI sliders (Inflation, Thickness, Profile dropdown) control the shape in real-time, and the 3D result is rendered in the editor viewport using the orbit camera from US-P4-002. The asset editor is a **separate binary** (`cargo run --bin battle_editor`) that shares the engine library. `battle_arena.rs` is NEVER modified.

## The Core Concept / Why This Matters
Pump/inflate extrusion is the signature technique for turning flat 2D drawings into organic 3D shapes. Unlike linear extrusion (which produces flat-sided prisms), inflation produces rounded, natural forms -- like inflating a balloon that conforms to the outline shape. This makes it ideal for trees, rocks, characters, shields, and organic props. The algorithm works by treating the 2D outline as a boundary on the XY plane, then "inflating" it outward along Z, with the profile curve controlling how the cross-section tapers from the center to the edges. A circle inflates into a smooth sphere-like shape; a star inflates into a puffy star pillow. This is what makes the asset editor capable of producing game-quality assets from simple 2D sketches -- the core value proposition of the entire editor pipeline.

## Goal
Create the extrusion module that takes `Outline2D` data from the 2D canvas (US-P4-003), generates a 3D SDF scalar field using the pump/inflate formulation, extracts a triangle mesh via the engine's `MarchingCubes`, and renders it in the editor viewport with orbit camera navigation. UI sliders allow real-time adjustment of inflation, thickness, and profile type.

## Files to Create/Modify
- **Create** `src/game/asset_editor/extrude.rs` -- `ExtrudeMethod` enum, `PumpProfile` enum, `ExtrudeParams` struct, `Extruder` struct, SDF computation (`sdf_pumped`), polygon helper functions (`min_distance_to_polygon`, `point_in_polygon`), Marching Cubes mesh generation, GPU buffer upload, mesh rendering
- **Modify** `src/game/asset_editor/mod.rs` -- Add `pub mod extrude;`, add `extruder: Extruder` field to `AssetEditor`, integrate into Stage 2 rendering and input, wire up slider controls

## Implementation Steps

1. **Define extrusion type enums and params struct:**
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum ExtrudeMethod {
       Pump,       // Inflate outline into organic 3D shape (this story)
       Linear,     // Extrude along Z axis (US-P4-007)
       Lathe,      // Revolve around Y axis (US-P4-007)
   }

   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum PumpProfile {
       Elliptical, // Default -- semicircular cross-section, thickest at center, zero at edge
       Flat,       // Constant depth (uniform thickness)
       Pointed,    // Like a leaf -- thin edges, sharp center ridge
   }

   pub struct ExtrudeParams {
       pub method: ExtrudeMethod,
       pub inflation: f32,         // 0.0-1.0, how much shape puffs out (default 0.5)
       pub thickness: f32,         // 0.1-5.0, maximum depth in world units (default 1.0)
       pub profile: PumpProfile,   // default Elliptical
       pub depth: f32,             // Linear extrude depth (US-P4-007)
       pub taper: f32,             // Linear extrude taper (US-P4-007)
       pub segments: u32,          // Lathe angular segments 6-64 (US-P4-007)
       pub sweep_degrees: f32,     // Lathe sweep 0-360 (US-P4-007)
       pub mc_resolution: u32,     // Marching Cubes grid resolution per axis (default 48)
   }
   ```

2. **Define `Extruder` struct:**
   ```rust
   pub struct Extruder {
       pub params: ExtrudeParams,
       pub mesh_vertices: Vec<BlockVertex>,
       pub mesh_indices: Vec<u32>,
       pub dirty: bool,            // needs recompute when params change
       pub gpu_vertex_buffer: Option<wgpu::Buffer>,
       pub gpu_index_buffer: Option<wgpu::Buffer>,
   }
   ```
   - `dirty` flag indicates the SDF parameters changed and mesh must be regenerated
   - GPU buffers hold the uploaded mesh for rendering
   - `BlockVertex` comes from `engine/src/render/building_blocks.rs` (position + normal + color, 40 bytes)

3. **Implement `min_distance_to_polygon(point: Vec2, polygon: &[Vec2]) -> f32`:**
   - For each consecutive edge `(polygon[i], polygon[(i+1) % n])`, compute the minimum distance from `point` to that line segment
   - Use the standard point-to-segment projection formula: project `point` onto the segment direction, clamp parameter `t` to `[0,1]`, compute distance to the closest point on the segment
   - Return the minimum distance across all edges
   ```rust
   fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
       let ab = b - a;
       let ap = p - a;
       let t = (ap.dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
       let closest = a + ab * t;
       (p - closest).length()
   }

   fn min_distance_to_polygon(point: Vec2, polygon: &[Vec2]) -> f32 {
       let n = polygon.len();
       (0..n)
           .map(|i| point_segment_distance(point, polygon[i], polygon[(i + 1) % n]))
           .fold(f32::MAX, f32::min)
   }
   ```

4. **Implement `point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool` using ray casting:**
   - Cast a horizontal ray from `point` in the +X direction
   - Count the number of polygon edges that the ray crosses
   - Odd count = inside, even count = outside
   ```rust
   fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
       let n = polygon.len();
       let mut inside = false;
       let mut j = n - 1;
       for i in 0..n {
           let pi = polygon[i];
           let pj = polygon[j];
           if ((pi.y > point.y) != (pj.y > point.y))
               && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
           {
               inside = !inside;
           }
           j = i;
       }
       inside
   }
   ```

5. **Compute `max_inradius` for the outline:**
   - The maximum inscribed radius is the largest distance from any interior point to the nearest boundary edge
   - This normalizes the distance used for profile computation, ensuring profiles work consistently regardless of outline size
   - Approximation: compute the centroid of the polygon, then find its distance to the nearest edge. For better accuracy, sample a small grid of interior points and take the maximum `min_distance_to_polygon` among those that pass `point_in_polygon`
   - Cache this value per outline to avoid recomputation on each SDF sample

6. **Implement `sdf_pumped(p: Vec3, outline: &Outline2D, params: &ExtrudeParams, max_inradius: f32) -> f32`:**
   This is the core SDF formulation from the phase spec:
   ```rust
   fn sdf_pumped(p: Vec3, outline: &Outline2D, params: &ExtrudeParams, max_inradius: f32) -> f32 {
       // 1. Project point onto XY plane
       let p2d = Vec2::new(p.x, p.y);

       // 2. Compute minimum unsigned distance to outline boundary
       let dist_to_boundary = min_distance_to_polygon(p2d, &outline.points);

       // 3. Determine inside/outside and sign the distance
       let inside = point_in_polygon(p2d, &outline.points);
       let signed_dist_2d = if inside { -dist_to_boundary } else { dist_to_boundary };

       // 4. Normalize distance for profile evaluation
       //    0.0 = at boundary, 1.0 = at deepest interior (center)
       let normalized_dist = (dist_to_boundary / max_inradius).clamp(0.0, 1.0);

       // 5. Evaluate thickness profile -- controls how Z-depth varies from edge to center
       let profile_depth = match params.profile {
           PumpProfile::Elliptical => (1.0 - normalized_dist * normalized_dist).sqrt(),
           PumpProfile::Flat => 1.0,
           PumpProfile::Pointed => 1.0 - normalized_dist,
       };

       // 6. Maximum Z extent at this XY position
       let max_z = params.thickness * params.inflation * profile_depth;

       // 7. Signed distance along Z (symmetric about z=0 plane)
       let z_dist = p.z.abs() - max_z;

       // 8. Combine: inside the 2D outline, only Z matters;
       //    outside, combine 2D boundary distance with Z overshoot
       if inside {
           z_dist.max(0.0)
       } else {
           (signed_dist_2d.powi(2) + z_dist.max(0.0).powi(2)).sqrt()
       }
   }
   ```

7. **Integrate with engine's `MarchingCubes`:**
   - Use the existing `MarchingCubes` from `engine/src/render/marching_cubes.rs` -- do NOT reimplement Marching Cubes
   - The engine's `generate_mesh()` signature: `fn generate_mesh<F>(&self, sdf: F, min: Vec3, max: Vec3, color: [f32; 4]) -> (Vec<BlockVertex>, Vec<u32>)` where `F: Fn(Vec3) -> f32`
   - It returns `(Vec<BlockVertex>, Vec<u32>)` -- vertices with position, normal, and color already computed (normals are derived from SDF gradient via central differences)
   - Compute the bounding box from the outline's AABB extended by the max Z extent plus a margin
   ```rust
   use battle_tok_engine::render::marching_cubes::MarchingCubes;

   fn generate_pump_mesh(
       outline: &Outline2D,
       params: &ExtrudeParams,
   ) -> (Vec<BlockVertex>, Vec<u32>) {
       let max_inradius = compute_max_inradius(outline);
       let mc = MarchingCubes::new(params.mc_resolution); // default 48

       // Compute bounding box from outline AABB + max thickness along Z
       let (bb_min, bb_max) = outline_bounding_box(&outline.points);
       let margin = 0.5;
       let z_extent = params.thickness * params.inflation + margin;
       let bounds_min = Vec3::new(bb_min.x - margin, bb_min.y - margin, -z_extent);
       let bounds_max = Vec3::new(bb_max.x + margin, bb_max.y + margin, z_extent);

       let sdf = |p: Vec3| -> f32 { sdf_pumped(p, outline, params, max_inradius) };
       let color = [0.6, 0.6, 0.6, 1.0]; // Default grey, painted later in Stage 4
       mc.generate_mesh(sdf, bounds_min, bounds_max, color)
   }
   ```

8. **Implement `Extruder::generate_preview(outline: &Outline2D) -> bool`:**
   - Validate outline: needs >= 3 points and `closed == true`
   - Call `generate_pump_mesh()` to produce the mesh
   - Store results in `mesh_vertices` and `mesh_indices`
   - Set `dirty = false`
   - Return `true` if mesh has > 0 triangles
   - On failure or degenerate outline, clear the mesh and return `false`

9. **Implement `Extruder::upload_to_gpu(device: &wgpu::Device)`:**
   - Create a `wgpu::Buffer` with `BufferUsages::VERTEX` from the `BlockVertex` array (40 bytes per vertex: 3xf32 position + 3xf32 normal + 4xf32 color)
   - Create a `wgpu::Buffer` with `BufferUsages::INDEX` from the `u32` index array
   - Store in `gpu_vertex_buffer` and `gpu_index_buffer`

10. **Implement `Extruder::render_mesh(pass: &mut wgpu::RenderPass, view_proj: &[[f32; 4]; 4])`:**
    - If GPU buffers exist and have data: set the render pipeline, bind vertex/index buffers, set the view-projection uniform, draw indexed triangles
    - The `BlockVertex` layout matches the existing building blocks pipeline, so the engine's render pipeline can be reused

11. **Integrate into `AssetEditor` in `mod.rs`:**
    - Add `pub mod extrude;` to `src/game/asset_editor/mod.rs`
    - Add `extruder: Extruder` field to the `AssetEditor` struct
    - When the editor transitions to Stage 2 (Extrude), call `extruder.generate_preview()` with the current outlines from `Canvas2D`
    - Render the 3D mesh using the orbit camera from US-P4-002
    - Wire up 3 UI controls using `UISlider` from `src/game/ui/slider.rs`:
      - **Inflation slider**: range 0.0 - 1.0, default 0.5, step 0.01
      - **Thickness slider**: range 0.1 - 5.0, default 1.0, step 0.1
      - **Profile dropdown**: cycle with P key between Elliptical / Flat / Pointed (or use a dropdown widget)
    - On any parameter change, set `extruder.dirty = true` and trigger mesh regeneration
    - Handle edge cases: empty outlines show warning text, open outlines auto-close

## Code Patterns

SDF point-to-segment distance (2D polygon helper):
```rust
fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let t = (ap.dot(ab) / ab.dot(ab)).clamp(0.0, 1.0);
    let closest = a + ab * t;
    (p - closest).length()
}
```

Ray-casting point-in-polygon test:
```rust
fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let n = polygon.len();
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let pi = polygon[i];
        let pj = polygon[j];
        if ((pi.y > point.y) != (pj.y > point.y))
            && (point.x < (pj.x - pi.x) * (point.y - pi.y) / (pj.y - pi.y) + pi.x)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
}
```

Core SDF formulation (from phase spec):
```rust
fn sdf_pumped(p: Vec3, outline: &Outline2D, params: &ExtrudeParams, max_inradius: f32) -> f32 {
    let p2d = Vec2::new(p.x, p.y);
    let dist_to_boundary = min_distance_to_polygon(p2d, &outline.points);
    let inside = point_in_polygon(p2d, &outline.points);
    let signed_dist_2d = if inside { -dist_to_boundary } else { dist_to_boundary };
    let normalized_dist = (dist_to_boundary / max_inradius).clamp(0.0, 1.0);
    let profile_depth = match params.profile {
        PumpProfile::Elliptical => (1.0 - normalized_dist * normalized_dist).sqrt(),
        PumpProfile::Flat => 1.0,
        PumpProfile::Pointed => 1.0 - normalized_dist,
    };
    let max_z = params.thickness * params.inflation * profile_depth;
    let z_dist = p.z.abs() - max_z;
    if inside {
        z_dist.max(0.0)
    } else {
        (signed_dist_2d.powi(2) + z_dist.max(0.0).powi(2)).sqrt()
    }
}
```

Using the engine's `MarchingCubes` (existing API -- do NOT reimplement):
```rust
use battle_tok_engine::render::marching_cubes::MarchingCubes;
use battle_tok_engine::render::building_blocks::BlockVertex;

let mc = MarchingCubes::new(params.mc_resolution); // default 48
let sdf = |p: Vec3| -> f32 { sdf_pumped(p, outline, params, max_inradius) };
let (vertices, indices): (Vec<BlockVertex>, Vec<u32>) =
    mc.generate_mesh(sdf, bounds_min, bounds_max, [0.6, 0.6, 0.6, 1.0]);
```

Note: `MarchingCubes::generate_mesh` already computes per-vertex normals from SDF gradients via central differences, so no separate normal computation step is needed.

## Acceptance Criteria
- [ ] `extrude.rs` exists at `src/game/asset_editor/extrude.rs`
- [ ] `ExtrudeMethod` enum defined with `Pump`, `Linear`, `Lathe` variants
- [ ] `PumpProfile` enum defined with `Elliptical`, `Flat`, `Pointed` variants
- [ ] `ExtrudeParams` struct defined with all fields (inflation, thickness, profile, mc_resolution, etc.)
- [ ] `Extruder` struct defined with mesh data fields, dirty flag, and GPU buffer handles
- [ ] `min_distance_to_polygon()` correctly computes unsigned distance from a point to a polygon boundary
- [ ] `point_in_polygon()` correctly determines inside/outside using ray casting
- [ ] `sdf_pumped()` produces a valid 3D signed distance field: negative inside the inflated volume, positive outside
- [ ] Elliptical profile produces smooth, rounded shapes (semicircular cross-section)
- [ ] Flat profile produces uniform-thickness slabs with the outline shape
- [ ] Pointed profile produces ridge-topped shapes (linear taper from edge to center)
- [ ] Marching Cubes integration uses the engine's existing `MarchingCubes` from `engine/src/render/marching_cubes.rs` (not a reimplementation)
- [ ] Inflation slider (0.0-1.0) controls how much the shape puffs out, updating mesh on change
- [ ] Thickness slider (0.1-5.0) controls maximum Z-depth, updating mesh on change
- [ ] Profile dropdown/cycle switches between Elliptical, Flat, and Pointed with visible difference
- [ ] Mesh renders with correct normals and basic lighting in the editor viewport
- [ ] `pub mod extrude;` added to `src/game/asset_editor/mod.rs`
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo check --bin battle_arena` still passes (no regressions)

## Verification Commands
- `cmd`: `test -f /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `extrude.rs file exists`
- `cmd`: `grep -c 'ExtrudeMethod\|PumpProfile\|ExtrudeParams\|Extruder' /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `Core types defined (enums, params, struct)`
- `cmd`: `grep -c 'sdf_pumped\|min_distance_to_polygon\|point_in_polygon' /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `SDF function and polygon helpers implemented`
- `cmd`: `grep -c 'MarchingCubes' /home/hungaromakker/battle_tok/src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `MarchingCubes integration present (using engine crate)`
- `cmd`: `grep -c 'pub mod extrude' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `extrude module declared in mod.rs`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor compiles with extrude module`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles (no regressions)`

## Success Looks Like
After drawing a closed 2D outline in Stage 1 (Draw2D) and pressing 2 to enter Stage 2 (Extrude), the outline inflates into a 3D mesh visible from the orbit camera. The default Elliptical profile produces a smooth, rounded balloon-like shape. Moving the Inflation slider toward 1.0 makes the shape maximally bulbous; moving it toward 0.0 flattens it back down. Adjusting Thickness increases or decreases the Z-depth proportionally. Pressing P to cycle to Flat profile turns the shape into a uniform-thickness slab with the outline boundary. Cycling to Pointed profile creates a ridge along the shape's interior. A simple circle outline inflates into a smooth hemisphere pair (mirrored above and below Z=0). A star outline inflates into a puffy star pillow. The mesh has smooth normals and renders with basic lighting. Higher mc_resolution (e.g., 64) produces a finer mesh at the cost of more computation. Interaction feels responsive -- a 48^3 grid evaluates in well under a second on typical hardware.

## Dependencies
- Depends on: US-P4-003 (provides `Outline2D` data from the 2D canvas)
- Uses: `MarchingCubes` from `engine/src/render/marching_cubes.rs` (existing engine code)
- Uses: `BlockVertex` from `engine/src/render/building_blocks.rs` (existing engine vertex type)
- Uses: `UISlider` from `src/game/ui/slider.rs` (existing UI component)
- Orbit camera from US-P4-002 (for 3D viewport navigation)

## Complexity
- Complexity: normal
- Min iterations: 1
