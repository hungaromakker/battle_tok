# US-P4-007: Linear Extrude + Lathe Revolution

## Description
Add two additional extrusion methods to the existing `extrude.rs`: linear extrusion with optional taper, and lathe revolution around the Y axis. Linear extrude pushes the 2D outline along the Z axis to create prismatic shapes (walls, pillars, beams). Lathe revolves the outline around the Y axis to create rotationally symmetric shapes (vases, columns, barrels). Both methods complement the pump/inflate extrusion (US-P4-006) and share the same Stage 2 UI.

## The Core Concept / Why This Matters
Pump/inflate is great for organic, pillow-like shapes, but many game assets need geometric precision. Linear extrude creates architectural elements — walls, fences, flat decorations — by simply pushing the outline into depth. The taper parameter adds visual interest: a tapered extrusion narrows toward the back, creating pyramid-like or wedge shapes. Lathe revolution is essential for any rotationally symmetric asset — barrels, columns, bottles, tree trunks, lamp posts. Together with pump/inflate, these three methods cover the vast majority of asset shapes needed in Battle Tök.

## Goal
Add `sdf_linear_extrude()` and `lathe_mesh()` functions to `src/game/asset_editor/extrude.rs`. Integrate both into the Stage 2 UI with an extrusion mode selector and per-mode parameter controls.

## Files to Create/Modify
- **Modify** `src/game/asset_editor/extrude.rs` — Add `Linear` and `Lathe` to `ExtrudeMode` enum, implement `sdf_linear_extrude()`, implement `lathe_mesh()`, add `LinearParams` and `LatheParams` structs, update `extrude_outlines()` to dispatch by mode
- **Modify** `src/game/asset_editor/mod.rs` — Update Stage 2 UI to include extrusion mode selector (Tab key cycles: Inflate → Linear → Lathe)
- **Modify** `src/bin/battle_editor.rs` — Forward Tab key and mode-specific parameter inputs

## Implementation Steps
1. Define mode enum and parameter structs:
   ```rust
   pub enum ExtrudeMode {
       Inflate,   // existing pump/inflate (US-P4-006)
       Linear,    // new: push along Z axis
       Lathe,     // new: revolve around Y axis
   }
   
   pub struct LinearParams {
       pub depth: f32,       // extrusion depth along Z (0.1 to 10.0, default 1.0)
       pub taper: f32,       // back face scale factor (0.0 to 1.0, default 1.0 = no taper)
       pub resolution: u32,  // MarchingCubes resolution
   }
   
   pub struct LatheParams {
       pub segments: u32,    // number of radial divisions (8 to 64, default 24)
       pub sweep: f32,       // rotation angle in degrees (1.0 to 360.0, default 360.0)
       pub axis_offset: f32, // distance from Y axis to outline (0.0 = outline touches axis)
   }
   ```
2. Implement `sdf_linear_extrude()`:
   - Takes a 3D point, outlines, and `LinearParams`
   - Project to 2D: `p2d = [point[0], point[1]]`
   - Compute signed distance to outline boundary in 2D
   - Compute Z bounds: front face at `z=0`, back face at `z=depth`
   - Apply taper: at z position, scale the outline by `lerp(1.0, taper, z/depth)`
   - Effectively: `scaled_p2d = p2d / scale_at_z` before computing 2D distance
   - SDF = `max(boundary_2d_distance, z_distance)` where `z_distance = max(-point[2], point[2] - depth)`
   - Use `MarchingCubes` to convert to mesh (same pattern as pump/inflate)
3. Implement `lathe_mesh()`:
   - This is direct mesh generation, NOT SDF-based (more efficient for rotational symmetry)
   - Take outline points as a 2D profile in the XY plane
   - For each segment `i` in `0..segments`:
     - Compute angle: `theta = (i as f32 / segments as f32) * sweep_radians`
     - For each outline point `(x, y)`:
       - 3D position: `[(x + axis_offset) * cos(theta), y, (x + axis_offset) * sin(theta)]`
     - Compute normal by cross product of tangent vectors (along outline and around revolution)
   - Generate triangle indices connecting adjacent rings:
     - For each ring pair (i, i+1) and outline point pair (j, j+1):
       - Two triangles forming a quad
   - If `sweep < 360.0`: do NOT connect last ring to first ring (partial revolution)
   - If `sweep == 360.0`: connect last ring back to first ring for seamless loop
4. Compute normals for lathe mesh:
   ```rust
   // For point at ring i, outline point j:
   // tangent_ring = position[i+1][j] - position[i-1][j]  (around revolution)
   // tangent_outline = position[i][j+1] - position[i][j-1]  (along profile)
   // normal = normalize(cross(tangent_ring, tangent_outline))
   ```
5. Update `extrude_outlines()` to dispatch by mode:
   ```rust
   pub fn extrude_outlines(
       outlines: &[Outline2D],
       mode: &ExtrudeMode,
       inflate_params: &ExtrudeParams,
       linear_params: &LinearParams,
       lathe_params: &LatheParams,
   ) -> (Vec<Vertex>, Vec<u32>) {
       match mode {
           ExtrudeMode::Inflate => /* existing pump_inflate code */,
           ExtrudeMode::Linear => /* sdf_linear_extrude via MarchingCubes */,
           ExtrudeMode::Lathe => /* lathe_mesh direct generation */,
       }
   }
   ```
6. Update Stage 2 UI:
   - Tab key cycles extrusion mode: Inflate → Linear → Lathe → Inflate
   - Show mode name at top of parameter panel
   - Show mode-specific sliders:
     - Inflate: inflation, thickness, profile (existing)
     - Linear: depth, taper sliders
     - Lathe: segments, sweep, axis_offset sliders
   - Auto-regenerate mesh on parameter change (with 200ms debounce)
7. Handle lathe-specific outline interpretation:
   - For lathe, the outline is treated as a profile curve in XY
   - The X coordinate becomes the radius (distance from Y axis)
   - The Y coordinate becomes the height
   - If outline has negative X values, warn the user (or take absolute value)
8. Handle sweep parameter for partial revolution:
   - When sweep < 360: generate cap faces at the start and end of the revolution
   - Cap faces: triangulate the outline profile as a flat polygon

## Code Patterns
Linear extrude SDF with taper:
```rust
pub fn sdf_linear_extrude(
    point: [f32; 3],
    outlines: &[Outline2D],
    params: &LinearParams,
) -> f32 {
    // Z bounds
    let z_dist = (-point[2]).max(point[2] - params.depth);
    
    // Taper: scale factor at this Z
    let t = (point[2] / params.depth).clamp(0.0, 1.0);
    let scale = 1.0 + (params.taper - 1.0) * t; // lerp from 1.0 to taper
    
    // Scale the 2D query point inversely
    let p2d = [point[0] / scale, point[1] / scale];
    
    let mut min_d = f32::MAX;
    for outline in outlines {
        let d = signed_distance_2d(p2d, outline);
        min_d = min_d.min(d);
    }
    
    // Union of Z bounds and 2D boundary
    min_d.max(z_dist)
}
```

Lathe revolution mesh generation:
```rust
pub fn lathe_mesh(
    outline: &Outline2D,
    params: &LatheParams,
) -> (Vec<Vertex>, Vec<u32>) {
    let sweep_rad = params.sweep.to_radians();
    let n_rings = if params.sweep >= 360.0 { params.segments } else { params.segments + 1 };
    let n_profile = outline.points.len();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    for i in 0..n_rings {
        let theta = (i as f32 / params.segments as f32) * sweep_rad;
        let cos_t = theta.cos();
        let sin_t = theta.sin();
        
        for &[px, py] in &outline.points {
            let r = px + params.axis_offset;
            let pos = [r * cos_t, py, r * sin_t];
            // Normal computation deferred to second pass
            vertices.push(Vertex { position: pos, normal: [0.0; 3], color: [0.7, 0.7, 0.7, 1.0] });
        }
    }
    
    // Generate indices: quads between adjacent rings
    for i in 0..params.segments {
        let ring_a = (i % n_rings) as u32;
        let ring_b = ((i + 1) % n_rings) as u32;
        for j in 0..(n_profile as u32 - 1) {
            let a = ring_a * n_profile as u32 + j;
            let b = ring_a * n_profile as u32 + j + 1;
            let c = ring_b * n_profile as u32 + j + 1;
            let d = ring_b * n_profile as u32 + j;
            indices.extend_from_slice(&[a, b, c, a, c, d]);
        }
    }
    
    // Compute normals via cross products (second pass)
    compute_smooth_normals(&mut vertices, &indices);
    
    (vertices, indices)
}
```

## Acceptance Criteria
- [ ] `ExtrudeMode` enum has `Inflate`, `Linear`, and `Lathe` variants
- [ ] `sdf_linear_extrude()` pushes outline along Z with SDF, converted via MarchingCubes
- [ ] Linear taper parameter scales the back face (1.0 = no taper, 0.0 = point)
- [ ] `lathe_mesh()` revolves outline around Y axis with direct mesh generation
- [ ] Lathe segments parameter controls smoothness (8 to 64)
- [ ] Lathe sweep parameter allows partial revolution (< 360 degrees)
- [ ] Tab key cycles between Inflate, Linear, and Lathe modes in Stage 2
- [ ] Each mode shows its own parameter sliders
- [ ] Mesh auto-regenerates on parameter change (debounced)
- [ ] All three modes produce valid, renderable meshes
- [ ] Existing pump/inflate (US-P4-006) still works correctly
- [ ] `cargo check --bin battle_editor` compiles with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor compiles with linear and lathe extrusion`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles`
- `cmd`: `grep -c 'Linear\|Lathe' src/game/asset_editor/extrude.rs`
  `expect_gt`: 3
  `description`: `Linear and Lathe modes referenced multiple times`
- `cmd`: `grep -c 'sdf_linear_extrude' src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `Linear extrude SDF function exists`
- `cmd`: `grep -c 'lathe_mesh' src/game/asset_editor/extrude.rs`
  `expect_gt`: 0
  `description`: `Lathe mesh generation function exists`
- `cmd`: `grep -c 'taper\|sweep' src/game/asset_editor/extrude.rs`
  `expect_gt`: 2
  `description`: `Taper and sweep parameters implemented`
- `cmd`: `grep -c 'ExtrudeMode' src/game/asset_editor/extrude.rs`
  `expect_gt`: 2
  `description`: `ExtrudeMode enum used in dispatch logic`

## Success Looks Like
In Stage 2, pressing Tab cycles between three extrusion modes. **Linear** mode takes a drawn outline and pushes it straight back into depth — like a cookie cutter pressed into clay. Adjusting the taper slider narrows the back face, creating a wedge or pyramid effect. **Lathe** mode revolves the outline around the Y axis — a drawn vase profile becomes a full 3D vase. Reducing sweep to 180 degrees creates a half-vase. All three modes (including the existing inflate) produce clean meshes viewable with the orbit camera. Parameter changes immediately regenerate the mesh.

## Dependencies
- Depends on: US-P4-006

## Complexity
- Complexity: normal
- Min iterations: 1
