# US-P4-008: Sculpting Bridge

## Description
Create a bridge module that wraps the existing `SculptingManager` (from `engine/src/render/sculpting.rs`) and exposes a set of sculpting tools for refining extruded meshes in Stage 3 (Sculpt). This includes face extrude, vertex pull, edge pull, smooth brush, and SDF-based add/subtract sphere operations. The add/subtract sphere tools use the existing `smooth_union` and `smooth_subtraction` functions from `engine/src/render/sdf_operations.rs` combined with `MarchingCubes` for re-meshing.

## The Core Concept / Why This Matters
Extrusion (Stage 2) produces a good starting shape, but it is rarely the final form. Sculpting is where the artist refines the mesh — pulling out a branch on a tree, smoothing a jagged edge on a rock, adding a knob to a structure, or carving a hollow into a decoration. The existing engine has a `SculptingManager` that handles low-level mesh manipulation, but it needs a higher-level interface tailored to the editor workflow. The smooth brush is essential for cleaning up marching cubes artifacts (staircase edges). The add/subtract sphere tools use SDF operations to seamlessly blend new geometry into the existing mesh, which is far superior to manually pulling vertices.

## Goal
Create `src/game/asset_editor/sculpt_bridge.rs` that wraps `SculptingManager` and provides a tool-based sculpting interface. Implement six sculpt tools with appropriate UI controls. Integrate into the editor Stage 3 with orbit camera and tool selection.

## Files to Create/Modify
- **Create** `src/game/asset_editor/sculpt_bridge.rs` — `SculptBridge` struct wrapping `SculptingManager`, `SculptTool` enum, tool implementations (FaceExtrude, VertexPull, EdgePull, Smooth, AddSphere, SubtractSphere), ray-mesh intersection for cursor targeting
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod sculpt_bridge;`, integrate into `AssetEditor` for Stage 3, wire up tool selection and mouse input
- **Modify** `src/bin/battle_editor.rs` — Forward sculpt-specific inputs (tool keys, mouse click/drag, brush size controls) when in Stage 3

## Implementation Steps
1. Define types:
   ```rust
   pub enum SculptTool {
       FaceExtrude,     // push/pull selected face along its normal
       VertexPull,      // drag individual vertices
       EdgePull,        // drag edge loops
       Smooth,          // average vertex positions with neighbors
       AddSphere,       // smooth_union with sphere SDF at cursor
       SubtractSphere,  // smooth_subtraction with sphere SDF at cursor
   }
   
   pub struct SculptBridge {
       manager: SculptingManager,
       pub tool: SculptTool,
       pub brush_radius: f32,    // 0.1 to 3.0, default 0.5
       pub brush_strength: f32,  // 0.0 to 1.0, default 0.5
       pub smooth_k: f32,        // smoothness factor for SDF ops, 0.1 to 1.0, default 0.3
       // Internal state
       dragging: bool,
       hit_point: Option<[f32; 3]>,
       hit_normal: Option<[f32; 3]>,
       hit_face_idx: Option<u32>,
       hit_vertex_idx: Option<u32>,
   }
   ```
2. Implement ray-mesh intersection for cursor targeting:
   ```rust
   fn raycast_mesh(
       ray_origin: [f32; 3],
       ray_dir: [f32; 3],
       vertices: &[Vertex],
       indices: &[u32],
   ) -> Option<(f32, [f32; 3], [f32; 3], u32)>  // (t, hit_point, hit_normal, face_index)
   ```
   - Test ray against each triangle (Moller-Trumbore algorithm)
   - Return closest hit
   - Compute screen-to-world ray from mouse position + orbit camera inverse view-projection
3. Implement `SculptTool::FaceExtrude`:
   - On click: raycast to find hit face, record face index and normal
   - On drag: move all vertices of the hit face along its normal by drag distance
   - Scale movement by `brush_strength`
   - Delegate vertex movement to `SculptingManager`
4. Implement `SculptTool::VertexPull`:
   - On click: raycast to find nearest vertex to hit point
   - On drag: move vertex along camera-relative direction (screen-space drag → world-space offset)
   - Scale by `brush_strength`
5. Implement `SculptTool::EdgePull`:
   - On click: find the edge nearest to the hit point
   - On drag: move both vertices of the edge along the averaged normal
   - Scale by `brush_strength`
6. Implement `SculptTool::Smooth`:
   - Smooth brush: on click/drag, for each vertex within `brush_radius` of the hit point:
     - Find all neighbor vertices (connected by edges in the index buffer)
     - New position = lerp(current, average_of_neighbors, brush_strength)
   - Apply iteratively while mouse is held (1 iteration per frame)
   - This is a Laplacian smooth weighted by distance from cursor center (falloff)
   ```rust
   fn smooth_vertices(
       vertices: &mut [Vertex],
       indices: &[u32],
       center: [f32; 3],
       radius: f32,
       strength: f32,
   ) {
       let adjacency = build_adjacency(indices, vertices.len());
       let affected: Vec<usize> = vertices.iter().enumerate()
           .filter(|(_, v)| distance(v.position, center) < radius)
           .map(|(i, _)| i)
           .collect();
       
       let new_positions: Vec<[f32; 3]> = affected.iter().map(|&i| {
           let neighbors = &adjacency[i];
           if neighbors.is_empty() { return vertices[i].position; }
           let avg = neighbors.iter()
               .map(|&n| vertices[n].position)
               .fold([0.0; 3], |a, b| add3(a, b));
           let avg = scale3(avg, 1.0 / neighbors.len() as f32);
           let falloff = 1.0 - distance(vertices[i].position, center) / radius;
           lerp3(vertices[i].position, avg, strength * falloff)
       }).collect();
       
       for (idx, &vi) in affected.iter().enumerate() {
           vertices[vi].position = new_positions[idx];
       }
   }
   ```
7. Implement `SculptTool::AddSphere`:
   - On click: place a sphere SDF at the hit point with `brush_radius`
   - Convert current mesh back to SDF (approximate: use mesh distance field, or store original SDF)
   - Combine: `smooth_union(mesh_sdf, sphere_sdf, smooth_k)` using the existing function from `engine/src/render/sdf_operations.rs`
   - Re-mesh via `MarchingCubes::new(resolution).generate_mesh(...)`
   - Replace current mesh with new mesh
   - Sphere SDF: `fn sphere_sdf(p: [f32; 3], center: [f32; 3], radius: f32) -> f32 { distance(p, center) - radius }`
8. Implement `SculptTool::SubtractSphere`:
   - Same as AddSphere but use `smooth_subtraction(mesh_sdf, sphere_sdf, smooth_k)`
   - This carves a smooth hole/dent at the cursor position
   - Re-mesh via MarchingCubes
9. For AddSphere/SubtractSphere, maintain an SDF representation alongside the mesh:
   - Store a list of SDF operations: `Vec<SdfOp>` where `SdfOp` is `Union(center, radius)` or `Subtract(center, radius)`
   - On each add/subtract, append to the list and regenerate mesh from the full SDF chain
   - This avoids the lossy mesh→SDF→mesh round-trip
   ```rust
   enum SdfOp {
       Base(Box<dyn Fn(f32, f32, f32) -> f32>),
       Union { center: [f32; 3], radius: f32 },
       Subtract { center: [f32; 3], radius: f32 },
   }
   ```
10. Tool selection hotkeys:
    - 1 (on numpad or with Shift): FaceExtrude
    - 2: VertexPull
    - 3: EdgePull
    - 4: Smooth
    - 5: AddSphere
    - 6: SubtractSphere
    - Note: these are Stage 3 specific — number keys 1-5 for stage switching only work outside of sculpt mode
    - Use F1-F6 or Shift+1-6 to avoid conflict with stage switching
11. Brush controls:
    - `[` / `]`: decrease/increase `brush_radius` (step 0.1, clamp 0.1 to 3.0)
    - `-` / `=`: decrease/increase `brush_strength` (step 0.05, clamp 0.05 to 1.0)
    - Shift+`[` / Shift+`]`: decrease/increase `smooth_k` for SDF operations
12. Visual feedback:
    - Render brush cursor as a wireframe sphere at the hit point with `brush_radius`
    - Highlight affected vertices/faces in a different color during hover
    - Show tool name and parameter values in the UI panel
13. Build vertex adjacency map for smooth tool:
    ```rust
    fn build_adjacency(indices: &[u32], vertex_count: usize) -> Vec<Vec<usize>> {
        let mut adj = vec![vec![]; vertex_count];
        for tri in indices.chunks(3) {
            let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
            adj[a].push(b); adj[a].push(c);
            adj[b].push(a); adj[b].push(c);
            adj[c].push(a); adj[c].push(b);
        }
        for list in &mut adj { list.sort_unstable(); list.dedup(); }
        adj
    }
    ```

## Code Patterns
Using existing engine SDF operations:
```rust
use crate::engine::render::sdf_operations::{smooth_union, smooth_subtraction};
use crate::engine::render::marching_cubes::MarchingCubes;
use crate::engine::render::sculpting::SculptingManager;

// AddSphere: combine existing SDF with a new sphere
fn add_sphere_op(
    base_sdf: &dyn Fn(f32, f32, f32) -> f32,
    center: [f32; 3],
    radius: f32,
    k: f32,
) -> impl Fn(f32, f32, f32) -> f32 + '_ {
    move |x, y, z| {
        let d1 = base_sdf(x, y, z);
        let d2 = ((x - center[0]).powi(2) + (y - center[1]).powi(2) + (z - center[2]).powi(2)).sqrt() - radius;
        smooth_union(d1, d2, k)
    }
}
```

Moller-Trumbore ray-triangle intersection:
```rust
fn ray_triangle_intersect(
    origin: [f32; 3], dir: [f32; 3],
    v0: [f32; 3], v1: [f32; 3], v2: [f32; 3],
) -> Option<(f32, f32, f32)> {  // (t, u, v)
    let edge1 = sub3(v1, v0);
    let edge2 = sub3(v2, v0);
    let h = cross3(dir, edge2);
    let a = dot3(edge1, h);
    if a.abs() < 1e-8 { return None; }
    let f = 1.0 / a;
    let s = sub3(origin, v0);
    let u = f * dot3(s, h);
    if !(0.0..=1.0).contains(&u) { return None; }
    let q = cross3(s, edge1);
    let v = f * dot3(dir, q);
    if v < 0.0 || u + v > 1.0 { return None; }
    let t = f * dot3(edge2, q);
    if t > 1e-8 { Some((t, u, v)) } else { None }
}
```

## Acceptance Criteria
- [ ] `sculpt_bridge.rs` exists with `SculptBridge` struct wrapping `SculptingManager`
- [ ] `SculptTool` enum has all six variants: FaceExtrude, VertexPull, EdgePull, Smooth, AddSphere, SubtractSphere
- [ ] Ray-mesh intersection (Moller-Trumbore) correctly finds clicked face/vertex
- [ ] FaceExtrude: push/pull face along its normal on drag
- [ ] VertexPull: move individual vertex on drag
- [ ] EdgePull: move edge vertices along averaged normal on drag
- [ ] Smooth brush: averages vertex positions with neighbors within radius, with distance falloff
- [ ] AddSphere: `smooth_union` with sphere SDF at cursor, re-mesh via MarchingCubes
- [ ] SubtractSphere: `smooth_subtraction` with sphere SDF at cursor, re-mesh via MarchingCubes
- [ ] SDF operation history maintained for lossless add/subtract chains
- [ ] Tool selection via F1-F6 or Shift+1-6
- [ ] Brush radius adjustable with `[` / `]`
- [ ] Brush strength adjustable with `-` / `=`
- [ ] Visual brush cursor (wireframe sphere) at hit point
- [ ] `cargo check --bin battle_editor` compiles with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor compiles with sculpt bridge`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles`
- `cmd`: `test -f src/game/asset_editor/sculpt_bridge.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `sculpt_bridge.rs file exists`
- `cmd`: `grep -c 'pub struct SculptBridge' src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `SculptBridge struct defined`
- `cmd`: `grep -c 'SculptTool' src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 3
  `description`: `SculptTool enum defined and used`
- `cmd`: `grep -c 'FaceExtrude\|VertexPull\|EdgePull\|Smooth\|AddSphere\|SubtractSphere' src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 5
  `description`: `All six sculpt tool variants present`
- `cmd`: `grep -c 'smooth_union\|smooth_subtraction' src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 1
  `description`: `SDF smooth operations used for add/subtract sphere`
- `cmd`: `grep -c 'SculptingManager' src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `Wraps existing SculptingManager`
- `cmd`: `grep -c 'MarchingCubes\|generate_mesh' src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `MarchingCubes used for re-meshing after SDF operations`
- `cmd`: `grep -c 'pub mod sculpt_bridge' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `sculpt_bridge module registered`

## Success Looks Like
In Stage 3 (Sculpt), the extruded mesh from Stage 2 is displayed with the orbit camera. Pressing F1 activates FaceExtrude — clicking a face and dragging pushes it outward or inward. F4 activates Smooth — painting over jagged marching cubes edges softens them into clean curves. F5 activates AddSphere — clicking on the mesh surface blends a smooth sphere into the geometry, like adding a blob of clay. F6 activates SubtractSphere — clicking carves a smooth dent or hole. The brush cursor (wireframe sphere) follows the mouse on the mesh surface. Bracket keys resize the brush. The mesh updates in real-time during sculpting.

## Dependencies
- Depends on: US-P4-006

## Complexity
- Complexity: complex
- Min iterations: 2
