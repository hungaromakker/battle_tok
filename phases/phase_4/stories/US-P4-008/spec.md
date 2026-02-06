# US-P4-008: Sculpting Bridge

## Description
Create `src/game/asset_editor/sculpt_bridge.rs` wrapping the existing `SculptingManager` from `engine/src/render/sculpting.rs` for Stage 3 of the asset editor. Add sphere add/subtract tools using `smooth_union()` and `smooth_subtraction()` from `sdf_operations.rs`. The sculpt bridge provides face extrusion, vertex/edge pulling, smooth brush, and SDF-based sphere add/subtract operations. The asset editor is a separate binary (`cargo run --bin battle_editor`) that shares the engine library but has its own winit window and event loop. `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
The sculpting stage is where flat extruded shapes become organic 3D objects. The existing `SculptingManager` in the engine provides face extrusion, vertex pulling, and edge pulling -- these are direct mesh manipulation tools. But artists also need additive/subtractive sculpting: stamping spheres to add bumps (knots on a tree trunk, rivets on armor) and carving spheres to create hollows (eye sockets, bowl interiors). These operations use SDF smooth_union and smooth_subtraction followed by marching cubes re-meshing, bridging the gap between direct mesh editing and SDF-based sculpting. The smooth brush is essential for cleaning up harsh edges left by other operations, averaging vertex positions with their neighbors to create organic-looking surfaces.

## Goal
Create `src/game/asset_editor/sculpt_bridge.rs` with `SculptBridge` struct providing face extrusion (via SculptingManager), AddSphere/SubtractSphere (via SDF operations + MarchingCubes), and Smooth brush (vertex averaging), integrated into Stage 3 of the `battle_editor` binary.

## Files to Create/Modify
- Create `src/game/asset_editor/sculpt_bridge.rs` -- SculptBridge, SculptTool enum, smooth brush, SDF sphere operations
- Modify `src/game/asset_editor/mod.rs` -- Add `pub mod sculpt_bridge;`, route Stage 3 input to SculptBridge

## Implementation Steps
1. Define the sculpt tool enum:
   ```rust
   #[derive(Clone, Copy, Debug, PartialEq)]
   pub enum SculptTool {
       FaceExtrude,
       VertexPull,
       EdgePull,
       Smooth,
       AddSphere,
       SubtractSphere,
   }
   ```

2. Define the SculptBridge struct:
   ```rust
   pub struct SculptBridge {
       pub active_tool: SculptTool,
       pub brush_radius: f32,    // default 0.5, range 0.1-5.0
       pub smooth_factor: f32,   // default 0.5, range 0.0-1.0
   }
   ```

3. Implement smooth brush algorithm:
   ```rust
   /// Smooth brush: find vertices within radius of cursor hit point,
   /// average each with its neighbor positions weighted by distance falloff.
   pub fn smooth_brush(
       &self,
       mesh: &mut Mesh,
       hit_point: Vec3,
   ) {
       let radius_sq = self.brush_radius * self.brush_radius;
       // Build adjacency: for each vertex, find connected neighbors via shared edges
       let adjacency = build_vertex_adjacency(&mesh.indices, mesh.vertices.len());

       // Collect smoothed positions first (avoid mutating while iterating)
       let mut new_positions: Vec<Option<[f32; 3]>> = vec![None; mesh.vertices.len()];

       for (i, vertex) in mesh.vertices.iter().enumerate() {
           let pos = Vec3::from(vertex.position);
           let dist_sq = (pos - hit_point).length_squared();
           if dist_sq > radius_sq { continue; }

           // Distance falloff: 1.0 at center, 0.0 at edge
           let falloff = 1.0 - (dist_sq / radius_sq).sqrt();
           let weight = falloff * self.smooth_factor;

           // Average with neighbor positions
           let neighbors = &adjacency[i];
           if neighbors.is_empty() { continue; }
           let avg: Vec3 = neighbors.iter()
               .map(|&ni| Vec3::from(mesh.vertices[ni].position))
               .sum::<Vec3>() / neighbors.len() as f32;

           let smoothed = pos.lerp(avg, weight);
           new_positions[i] = Some(smoothed.into());
       }

       // Apply smoothed positions
       for (i, new_pos) in new_positions.iter().enumerate() {
           if let Some(p) = new_pos {
               mesh.vertices[i].position = *p;
           }
       }
   }
   ```

4. Implement AddSphere operation:
   ```rust
   /// Add a sphere at cursor position using SDF smooth_union + MarchingCubes remesh.
   /// Converts current mesh to SDF, unions with sphere SDF, re-meshes.
   pub fn add_sphere(
       &self,
       mesh: &Mesh,
       center: Vec3,
       radius: f32,
       smoothness: f32,
   ) -> Mesh {
       // Convert current mesh to SDF representation
       let mesh_sdf = mesh_to_sdf(mesh);
       // Create sphere SDF at cursor position
       let sphere_sdf = sdf_sphere(center, radius);
       // Combine using smooth union from sdf_operations.rs
       let combined = smooth_union(mesh_sdf, sphere_sdf, smoothness);
       // Re-mesh using MarchingCubes
       marching_cubes(&combined, grid_resolution)
   }
   ```

5. Implement SubtractSphere operation:
   ```rust
   /// Subtract a sphere at cursor position using SDF smooth_subtraction + MarchingCubes remesh.
   pub fn subtract_sphere(
       &self,
       mesh: &Mesh,
       center: Vec3,
       radius: f32,
       smoothness: f32,
   ) -> Mesh {
       let mesh_sdf = mesh_to_sdf(mesh);
       let sphere_sdf = sdf_sphere(center, radius);
       let carved = smooth_subtraction(sphere_sdf, mesh_sdf, smoothness);
       marching_cubes(&carved, grid_resolution)
   }
   ```

6. Delegate FaceExtrude, VertexPull, EdgePull to existing SculptingManager:
   ```rust
   pub fn delegate_to_sculpting_manager(
       &self,
       sculpting_manager: &mut SculptingManager,
       tool: SculptTool,
       // ... input parameters
   ) {
       match tool {
           SculptTool::FaceExtrude => sculpting_manager.extrude_face(/*...*/),
           SculptTool::VertexPull => sculpting_manager.pull_vertex(/*...*/),
           SculptTool::EdgePull => sculpting_manager.pull_edge(/*...*/),
           _ => {} // Smooth, AddSphere, SubtractSphere handled locally
       }
   }
   ```

7. Build vertex adjacency helper:
   ```rust
   fn build_vertex_adjacency(indices: &[u32], vertex_count: usize) -> Vec<Vec<usize>> {
       let mut adj = vec![Vec::new(); vertex_count];
       for tri in indices.chunks(3) {
           let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
           // Add bidirectional edges
           if !adj[a].contains(&b) { adj[a].push(b); adj[b].push(a); }
           if !adj[b].contains(&c) { adj[b].push(c); adj[c].push(b); }
           if !adj[a].contains(&c) { adj[a].push(c); adj[c].push(a); }
       }
       adj
   }
   ```

8. Wire into `mod.rs`: add `pub mod sculpt_bridge;`, add `sculpt: SculptBridge` field to AssetEditor, route Stage 3 input events and render calls to SculptBridge.

## Acceptance Criteria
- [ ] `sculpt_bridge.rs` exists with `SculptBridge` struct and `SculptTool` enum
- [ ] SculptTool enum has FaceExtrude, VertexPull, EdgePull, Smooth, AddSphere, SubtractSphere variants
- [ ] Face extrusion from SculptingManager is accessible through the bridge
- [ ] AddSphere uses SDF smooth_union at cursor position + MarchingCubes remesh
- [ ] SubtractSphere uses SDF smooth_subtraction + MarchingCubes remesh
- [ ] Smooth brush finds vertices within radius and averages with neighbor positions using distance falloff
- [ ] Vertex adjacency is built from triangle indices
- [ ] SculptBridge is wired into mod.rs for Stage 3 input routing
- [ ] `cargo check` passes with 0 errors

## Verification Commands
- `test -f /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs && echo EXISTS` -- expected: EXISTS
- `grep -c 'SculptBridge\|SculptTool' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs` -- expected: > 0
- `grep -c 'AddSphere\|SubtractSphere\|Smooth\|FaceExtrude' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs` -- expected: > 0
- `grep -c 'smooth_union\|smooth_subtraction\|marching_cubes\|build_vertex_adjacency' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs` -- expected: > 0
- `grep -c 'pub mod sculpt_bridge' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs` -- expected: > 0
- `cd /home/hungaromakker/battle_tok && cargo check 2>&1; echo EXIT:$?` -- expected: EXIT:0

## Success Looks Like
The artist enters Stage 3 (Sculpt) with an extruded mesh. They select FaceExtrude and click a face -- it pushes outward, creating a protrusion. They switch to AddSphere, set radius to 0.3, and click on the trunk of a tree mesh -- a smooth bump appears where they clicked, blending seamlessly into the trunk surface. They use SubtractSphere to carve a small hollow. The smooth brush lets them soften any harsh transitions. Every operation updates the mesh in real-time, and the sculpted geometry looks organic and natural.

## Dependencies
- Depends on: US-P4-006 (needs a 3D mesh from the extrusion stage to sculpt)

## Complexity
- Complexity: normal
- Min iterations: 1
