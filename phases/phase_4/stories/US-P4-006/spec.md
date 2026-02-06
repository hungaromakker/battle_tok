# US-P4-006: Pump/Inflate Extrusion (SDF-based)

## Description
Implement the primary extrusion method for turning 2D outlines into 3D meshes. The pump/inflate algorithm takes a closed 2D outline, constructs a signed distance field (SDF) based on the distance from each 3D point to the outline boundary, applies a thickness profile to shape the inflation, and converts the resulting SDF to a triangle mesh via the existing `MarchingCubes` implementation. This is the Stage 2 (Extrude) core functionality.

## The Core Concept / Why This Matters
This is the magic step that turns a flat 2D drawing into a 3D object. The "pump" metaphor is apt: imagine the 2D outline is a flat balloon, and you inflate it. Points near the center of the outline puff up the most, points near the edges stay thin. The SDF approach is powerful because it naturally handles concave outlines, holes, and complex shapes — unlike naive vertex extrusion which would produce self-intersecting geometry. The three profile types (elliptical, flat, pointed) give the artist control over the cross-section shape: elliptical for organic forms (fruits, rocks), flat for mechanical parts (panels, bricks), pointed for crystalline shapes (gems, spikes).

## Goal
Create `src/game/asset_editor/extrude.rs` with the pump/inflate extrusion algorithm. Take 2D outlines from the canvas, generate a 3D SDF, convert to mesh via `MarchingCubes`, and display the result using the orbit camera. Provide UI controls for inflation amount, thickness, and profile type.

## Files to Create/Modify
- **Create** `src/game/asset_editor/extrude.rs` — `ExtrudeMode` enum, `ProfileType` enum, `pump_inflate_sdf()` function, `outline_distance_2d()` helper, `extrude_outlines()` orchestrator, mesh buffer management
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod extrude;`, integrate extrusion into `AssetEditor` for Stage 2, add mesh vertex/index buffers
- **Modify** `src/bin/battle_editor.rs` — Wire Stage 2 rendering to use extruded mesh with orbit camera, forward UI events for extrusion parameters

## Implementation Steps
1. Define types:
   ```rust
   pub enum ProfileType {
       Elliptical,  // sqrt profile: height = sqrt(1 - (d/max_d)^2) * thickness
       Flat,        // constant: height = thickness while d < max_d
       Pointed,     // linear: height = (1 - d/max_d) * thickness
   }
   
   pub struct ExtrudeParams {
       pub inflation: f32,     // 0.0 to 1.0, controls how far inside points puff up
       pub thickness: f32,     // 0.1 to 5.0, max height of the inflation
       pub profile: ProfileType,
       pub resolution: u32,    // MarchingCubes grid resolution (32 default, 16-64 range)
   }
   ```
2. Implement `outline_distance_2d(point: [f32; 2], outline: &Outline2D) -> f32`:
   - Compute minimum distance from the point to any line segment in the outline
   - Returns positive if outside, negative if inside (use winding number or ray casting for inside/outside test)
3. Implement `point_in_outline(point: [f32; 2], outline: &Outline2D) -> bool`:
   - Ray casting algorithm: count intersections of a horizontal ray from point to the right
   - Odd count = inside, even count = outside
4. Implement `pump_inflate_sdf(point: [f32; 3], outlines: &[Outline2D], params: &ExtrudeParams) -> f32`:
   - Project the 3D point to 2D: `p2d = [point[0], point[1]]`
   - Compute `d = outline_distance_2d(p2d, outline)` — signed distance to boundary
   - If outside all outlines: return positive distance (outside surface)
   - If inside: compute height limit from profile:
     - Elliptical: `h_max = thickness * sqrt(1.0 - (d / (d_max * inflation)).powi(2).min(1.0))`
     - Flat: `h_max = thickness` (constant while inside)
     - Pointed: `h_max = thickness * (1.0 - d.abs() / (d_max * inflation)).max(0.0)`
   - `d_max` = maximum interior distance (approximate from outline bounding box or precompute)
   - SDF value = `point[2].abs() - h_max` — positive outside, negative inside the inflated shape
   - Combine the Z distance and the boundary distance: `sdf = max(boundary_sdf, z_sdf)` or use smooth_max for softer edges
5. Implement `extrude_outlines(outlines: &[Outline2D], params: &ExtrudeParams) -> (Vec<BlockVertex>, Vec<u32>)`:
   - Compute bounding box of all outlines, add padding equal to `thickness`
   - Set up `MarchingCubes::new(params.resolution)`
   - Define SDF closure: `|x, y, z| pump_inflate_sdf([x, y, z], outlines, params)`
   - Call `marching_cubes.generate_mesh(sdf_fn, min, max, default_color)`
   - Return the vertex and index arrays
6. Implement mesh upload to GPU:
   - Convert `BlockVertex` to `Vertex { position, normal, color }` (from `src/game/types.rs`)
   - Create/update wgpu vertex buffer and index buffer
   - Store in `AssetEditor` for rendering
7. Implement UI controls for Stage 2:
   - Inflation slider (0.0 to 1.0, default 0.5) using `UISlider`
   - Thickness slider (0.1 to 5.0, default 1.0) using `UISlider`
   - Profile dropdown (cycle with P key): Elliptical → Flat → Pointed
   - Resolution slider (16 to 64, step 8, default 32)
   - "Re-extrude" button or auto-regenerate on parameter change (with debounce — 200ms delay)
8. Integrate into `AssetEditor` Stage 2:
   - On entering Stage 2: auto-extrude with default params
   - Render the mesh with basic directional lighting (dot product of normal with light direction)
   - Use orbit camera for viewing
   - Show parameters UI alongside the 3D preview
9. Handle edge cases:
   - Empty outlines: show warning text, no mesh
   - Open outlines: auto-close by connecting first and last points
   - Multiple outlines: union via min(sdf1, sdf2) or use smooth_union from `engine/src/render/sdf_operations.rs`

## Code Patterns
SDF-based inflation:
```rust
pub fn pump_inflate_sdf(
    point: [f32; 3],
    outlines: &[Outline2D],
    params: &ExtrudeParams,
) -> f32 {
    let p2d = [point[0], point[1]];
    
    // Find minimum signed distance to any outline boundary
    let mut min_boundary = f32::MAX;
    for outline in outlines {
        let d = signed_distance_2d(p2d, outline);
        min_boundary = min_boundary.min(d);
    }
    
    if min_boundary > 0.0 {
        // Outside all outlines
        return min_boundary;
    }
    
    // Inside: compute inflation height based on profile
    let normalized_depth = (-min_boundary / (params.inflation * max_interior_distance)).min(1.0);
    let h_max = match params.profile {
        ProfileType::Elliptical => params.thickness * (1.0 - normalized_depth.powi(2)).max(0.0).sqrt(),
        ProfileType::Flat => params.thickness,
        ProfileType::Pointed => params.thickness * (1.0 - normalized_depth).max(0.0),
    };
    
    // Z-axis distance from center plane
    let z_dist = point[2].abs() - h_max;
    
    // Combine boundary and height: max gives hard intersection
    z_dist.max(min_boundary * 0.5)
}
```

Using existing MarchingCubes:
```rust
use crate::engine::render::marching_cubes::MarchingCubes;

let mc = MarchingCubes::new(params.resolution);
let (vertices, indices) = mc.generate_mesh(
    |x, y, z| pump_inflate_sdf([x, y, z], &outlines, &params),
    [min_x, min_y, -params.thickness],
    [max_x, max_y, params.thickness],
    [0.7, 0.7, 0.7, 1.0], // default gray color
);
```

## Acceptance Criteria
- [ ] `extrude.rs` exists with `ExtrudeMode`, `ProfileType`, `ExtrudeParams` types
- [ ] `pump_inflate_sdf()` computes correct SDF for 2D outline inflation into 3D
- [ ] `outline_distance_2d()` computes signed distance to outline boundary
- [ ] `point_in_outline()` correctly classifies inside/outside points
- [ ] `extrude_outlines()` uses `MarchingCubes` to convert SDF to mesh
- [ ] Three profile types produce visually distinct results: Elliptical (rounded), Flat (uniform), Pointed (peaked)
- [ ] Inflation slider (0.0-1.0) controls how far interior inflation extends
- [ ] Thickness slider (0.1-5.0) controls maximum inflation height
- [ ] Profile type cycles with P key
- [ ] Mesh renders in Stage 2 with orbit camera and basic lighting
- [ ] Multiple outlines combined via SDF union
- [ ] Open outlines auto-closed
- [ ] `cargo check --bin battle_editor` compiles with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor compiles with extrude module`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles`
- `cmd`: `test -f src/game/asset_editor/extrude.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `extrude.rs file exists`
- `cmd`: `grep -c 'pub struct ExtrudeParams' src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `ExtrudeParams struct defined`
- `cmd`: `grep -c 'ProfileType' src/game/asset_editor/extrude.rs`
  `expect_gt`: 2
  `description`: `ProfileType enum used in multiple places`
- `cmd`: `grep -c 'pump_inflate_sdf\|extrude_outlines' src/game/asset_editor/extrude.rs`
  `expect_gt`: 1
  `description`: `Core extrusion functions implemented`
- `cmd`: `grep -c 'MarchingCubes\|generate_mesh' src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `MarchingCubes integration present`
- `cmd`: `grep -c 'pub mod extrude' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `extrude module registered`

## Success Looks Like
After drawing a closed outline in Stage 1 and switching to Stage 2, the outline inflates into a 3D mesh like a balloon. The default elliptical profile creates a smooth, rounded shape — like a pillow or pebble. Switching to flat profile makes it look like a cookie cutter extrusion. Pointed profile creates a peaked ridge. Adjusting inflation makes the shape puffier or thinner. The orbit camera lets the artist rotate around the mesh to inspect it from all angles. Multiple outlines create a unified mesh. The resolution slider trades quality for speed.

## Dependencies
- Depends on: US-P4-003, US-P4-002

## Complexity
- Complexity: complex
- Min iterations: 2
