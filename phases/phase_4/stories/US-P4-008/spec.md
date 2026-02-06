# US-P4-008: Sculpting Bridge

## Description
Create `src/game/asset_editor/sculpt_bridge.rs` that bridges the asset editor's Stage 3 (Sculpt) to the existing `SculptingManager` from `engine/src/render/sculpting.rs`. The bridge wraps the engine's face extrusion, edge pulling, and vertex pulling tools for use on the editor's 3D mesh, and adds three new sculpting tools: a Smooth brush that relaxes vertex positions toward their neighborhood average, an AddSphere tool that stamps a sphere SDF via `smooth_union()` and re-meshes with Marching Cubes, and a SubtractSphere tool that carves via `smooth_subtraction()` and re-meshes. The SDF boolean operations come from `engine/src/render/sdf_operations.rs`. The asset editor is a **separate binary** (`cargo run --bin battle_editor`); `battle_arena.rs` is **never modified**.

## The Core Concept / Why This Matters
After the artist extrudes a 2D outline into 3D (Stage 2), the resulting mesh is a rough starting shape. Sculpting transforms that rough shape into a detailed, expressive asset. The existing `SculptingManager` already provides face extrusion (`ExtrusionOperation`), edge pulling (`EdgeSelection`), and vertex pulling (`VertexSelection`) for building blocks -- the bridge adapts these for the editor's `Mesh` type. The three new tools fill critical gaps: Smooth relaxes noisy marching-cubes geometry into clean surfaces, AddSphere lets artists blob on material (growing horns, adding bumps, building up organic forms), and SubtractSphere lets artists carve holes and concavities (eye sockets, hollows, crevices). Together, the six tools give artists full sculptural control without leaving the engine.

## Goal
Create a `SculptBridge` struct in `sculpt_bridge.rs` that provides six sculpting tools (`FaceExtrude`, `VertexPull`, `EdgePull`, `Smooth`, `AddSphere`, `SubtractSphere`) operating on the editor's `Mesh`. Integrate into `mod.rs` so that Stage 3 input is routed to the sculpt bridge, and the mesh is re-uploaded to the GPU after each sculpt operation.

## Files to Create/Modify
- **Create** `src/game/asset_editor/sculpt_bridge.rs` -- `SculptBridge` struct, `SculptTool` enum, smooth brush algorithm, SDF sphere stamp/carve with Marching Cubes re-meshing, delegation to existing `SculptingManager` for face/edge/vertex operations
- **Modify** `src/game/asset_editor/mod.rs` -- Add `pub mod sculpt_bridge;`, add `sculpt: SculptBridge` field to `AssetEditor`, route Stage 3 (Sculpt) input and rendering to `SculptBridge`

## Implementation Steps
1. Define `SculptTool` enum with six variants:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub enum SculptTool {
       FaceExtrude,     // Wraps existing ExtrusionOperation from sculpting.rs
       VertexPull,      // Wraps existing VertexSelection from sculpting.rs
       EdgePull,        // Wraps existing EdgeSelection from sculpting.rs
       Smooth,          // NEW: Average vertex positions with neighbors (Laplacian relaxation)
       AddSphere,       // NEW: smooth_union() with sphere SDF at cursor + MarchingCubes re-mesh
       SubtractSphere,  // NEW: smooth_subtraction() with sphere SDF at cursor + MarchingCubes re-mesh
   }
   ```

2. Define `SculptBridge` struct:
   ```rust
   pub struct SculptBridge {
       pub active_tool: SculptTool,
       pub brush_radius: f32,       // World-space radius for smooth/add/subtract (default 0.5)
       pub smooth_factor: f32,      // SDF smooth_union/subtraction k value (default 0.15)
       pub smooth_strength: f32,    // Smooth brush blending strength (0.0-1.0, default 0.5)
       pub mc_resolution: u32,      // Marching Cubes resolution for re-meshing (default 48)
       sculpting_manager: SculptingManager,
       mesh_sdf_cache: Option<Vec<f32>>,  // Cached SDF grid for the current mesh
       mesh_bounds: (Vec3, Vec3),         // Current mesh AABB (min, max)
       dirty: bool,                       // Mesh has changed, needs GPU re-upload
   }
   ```

3. Implement `SculptBridge::new()` constructor:
   - Initialize `SculptingManager` via `SculptingManager::new()` and call `set_enabled(true)`
   - Set default `active_tool: SculptTool::Smooth`
   - Set `brush_radius: 0.5`, `smooth_factor: 0.15`, `smooth_strength: 0.5`
   - Set `mc_resolution: 48`

4. Implement `set_tool(tool: SculptTool)`:
   - Update `active_tool`
   - Cancel any in-progress operations on the `SculptingManager` via `cancel()` if switching away from FaceExtrude/VertexPull/EdgePull

5. Implement `build_adjacency(mesh: &Mesh) -> Vec<Vec<usize>>`:
   - From the index buffer, build a per-vertex adjacency list
   - For each triangle (i0, i1, i2), each vertex is adjacent to the other two
   - Deduplicate neighbor lists
   - This is needed for the smooth brush to find neighbor positions

6. Implement `average_neighbor_positions(mesh: &Mesh, vertex_index: usize, adjacency: &[Vec<usize>]) -> Vec3`:
   - Look up neighbors from adjacency list
   - Compute arithmetic mean of all neighbor positions
   - Return the average position (or the vertex's own position if no neighbors)

7. Implement `smooth_brush(mesh: &mut Mesh, center: Vec3, radius: f32, strength: f32)`:
   ```rust
   pub fn smooth_brush(mesh: &mut Mesh, center: Vec3, radius: f32, strength: f32) {
       let adjacency = build_adjacency(mesh);
       // Pass 1: compute new positions without mutating
       let new_positions: Vec<Option<[f32; 3]>> = (0..mesh.vertices.len()).map(|i| {
           let pos = Vec3::from(mesh.vertices[i].position);
           let dist = pos.distance(center);
           if dist < radius {
               let weight = 1.0 - (dist / radius); // Linear falloff
               let neighbors = &adjacency[i];
               if neighbors.is_empty() {
                   return None;
               }
               let avg: Vec3 = neighbors.iter()
                   .map(|&ni| Vec3::from(mesh.vertices[ni].position))
                   .sum::<Vec3>() / neighbors.len() as f32;
               let smoothed = pos.lerp(avg, weight * strength);
               Some(smoothed.to_array())
           } else {
               None
           }
       }).collect();

       // Pass 2: apply
       for (i, new_pos) in new_positions.into_iter().enumerate() {
           if let Some(p) = new_pos {
               mesh.vertices[i].position = p;
           }
       }
   }
   ```
   - **Key detail:** Collect all new positions before writing any back. If you mutate during iteration, later vertices read already-smoothed neighbors, causing asymmetric drift.

8. Implement `mesh_to_sdf(mesh: &Mesh, resolution: u32) -> (Vec<f32>, Vec3, Vec3)`:
   - Compute AABB of the mesh with margin (10% expansion)
   - Create a 3D grid of SDF values at `resolution^3` points
   - For each grid point, compute approximate signed distance to the mesh surface:
     - Distance = minimum distance to any triangle in the mesh
     - Sign = determined by dot product of displacement with nearest triangle's normal (positive = outside, negative = inside)
   - Return (sdf_grid, bounds_min, bounds_max)
   - This is needed for AddSphere/SubtractSphere to convert the mesh into an SDF, apply the boolean operation, then re-extract via Marching Cubes

9. Implement `sdf_sphere(p: Vec3, center: Vec3, radius: f32) -> f32`:
   ```rust
   fn sdf_sphere(p: Vec3, center: Vec3, radius: f32) -> f32 {
       p.distance(center) - radius
   }
   ```

10. Implement `stamp_sphere(mesh: &mut Mesh, center: Vec3, radius: f32, smooth_k: f32, mc_resolution: u32, add: bool)`:
    - Clone the current mesh as `old_mesh` for color transfer later
    - Compute mesh AABB expanded to include the sphere bounds + 10% margin
    - Build a combined SDF closure:
      - Compute approximate signed distance to the old mesh surface
      - Compute sphere SDF: `p.distance(center) - radius`
      - If `add == true`: combine with `smooth_union(mesh_sdf, sphere_sdf, smooth_k)` from `engine/src/render/sdf_operations.rs`
      - If `add == false`: combine with `smooth_subtraction(mesh_sdf, sphere_sdf, smooth_k)` from `engine/src/render/sdf_operations.rs`
    - Run `MarchingCubes::new(mc_resolution).generate_mesh(combined_sdf, min, max, default_color)` from `engine/src/render/marching_cubes.rs`
    - Transfer vertex colors from old mesh to new mesh via nearest-vertex lookup
    - Replace `mesh.vertices` and `mesh.indices` with the new mesh data
    - Normals are already computed by `MarchingCubes::generate_mesh` from SDF gradient

11. Implement `handle_input(mesh: &mut Mesh, cursor_world_pos: Vec3, is_pressed: bool, is_dragging: bool)`:
    - Route to appropriate tool based on `active_tool`:
      - `Smooth` + dragging: call `smooth_brush()` at cursor position, then `recompute_normals()`
      - `AddSphere` + pressed: call `stamp_sphere(..., add: true)`
      - `SubtractSphere` + pressed: call `stamp_sphere(..., add: false)`
      - `FaceExtrude` / `VertexPull` / `EdgePull`: delegate to `sculpting_manager` methods (`try_select_face`, `start_drag`, `update_drag`, `end_drag`)
    - Set `dirty = true` after any mesh modification

12. Implement `recompute_normals(mesh: &mut Mesh)`:
    - Zero all vertex normals
    - For each triangle, compute face normal via cross product of two edges
    - Accumulate face normals at each vertex (area-weighted -- the cross product magnitude equals 2x triangle area)
    - Normalize all vertex normals via `normalize_or_zero()`
    - This is used after smooth brush operations (Marching Cubes handles its own normals for stamp operations)

13. Implement `transfer_vertex_colors(old_mesh: &Mesh, new_mesh: &mut Mesh)`:
    - For each vertex in `new_mesh`, find the nearest vertex in `old_mesh` by squared distance
    - Copy its `color` field
    - This preserves painted colors across re-meshing from AddSphere/SubtractSphere

14. Integrate into `AssetEditor` in `mod.rs`:
    - Add `pub mod sculpt_bridge;` to module declarations
    - Add `sculpt: SculptBridge` field to `AssetEditor`
    - In the `update()` method, when `stage == EditorStage::Sculpt`:
      - Route mouse/keyboard input to `sculpt.handle_input()`
      - Number keys 1-6 switch sculpt tool
      - Scroll wheel adjusts `brush_radius` (clamped 0.1..5.0)
      - Shift+scroll adjusts `smooth_factor` (clamped 0.01..1.0)
    - In the `render()` method, when `stage == EditorStage::Sculpt`:
      - If `sculpt.is_dirty()`, re-upload mesh to GPU buffers, call `sculpt.clear_dirty()`
      - Draw brush radius preview circle at cursor position on mesh surface
    - Show active tool name and parameters in the property panel (right side)

## Code Patterns
Import paths for existing engine types:
```rust
// Sculpting manager and types from engine
use battle_tok_engine::render::sculpting::{
    SculptingManager, SculptMode, SculptState,
    ExtrusionOperation, FaceSelection, EdgeSelection, VertexSelection,
    SelectionType, FaceDirection,
};

// SDF boolean operations from engine
use battle_tok_engine::render::sdf_operations::{smooth_union, smooth_subtraction};

// Marching Cubes from engine
use battle_tok_engine::render::marching_cubes::MarchingCubes;

// Shared game types
use crate::game::types::{Vertex, Mesh};
```

SculptBridge struct definition:
```rust
pub struct SculptBridge {
    pub active_tool: SculptTool,
    pub brush_radius: f32,
    pub smooth_factor: f32,      // SDF smooth_union k value
    pub smooth_strength: f32,    // Smooth brush blend weight
    pub mc_resolution: u32,
    sculpting_manager: SculptingManager,
    mesh_sdf_cache: Option<Vec<f32>>,
    mesh_bounds: (Vec3, Vec3),
    dirty: bool,
}

impl SculptBridge {
    pub fn new() -> Self {
        let mut sm = SculptingManager::new();
        sm.set_enabled(true);
        Self {
            active_tool: SculptTool::Smooth,
            brush_radius: 0.5,
            smooth_factor: 0.15,
            smooth_strength: 0.5,
            mc_resolution: 48,
            sculpting_manager: sm,
            mesh_sdf_cache: None,
            mesh_bounds: (Vec3::ZERO, Vec3::ZERO),
            dirty: false,
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }
}
```

SculptTool enum with UI labels:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SculptTool {
    FaceExtrude,
    VertexPull,
    EdgePull,
    Smooth,
    AddSphere,
    SubtractSphere,
}

impl SculptTool {
    /// Keyboard shortcut label for UI display
    pub fn label(&self) -> &'static str {
        match self {
            Self::FaceExtrude    => "1: Face Extrude",
            Self::VertexPull     => "2: Vertex Pull",
            Self::EdgePull       => "3: Edge Pull",
            Self::Smooth         => "4: Smooth",
            Self::AddSphere      => "5: Add Sphere",
            Self::SubtractSphere => "6: Subtract Sphere",
        }
    }
}
```

Adjacency building from index buffer:
```rust
fn build_adjacency(mesh: &Mesh) -> Vec<Vec<usize>> {
    let vertex_count = mesh.vertices.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); vertex_count];

    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 { continue; }
        let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);

        if !adj[a].contains(&b) { adj[a].push(b); }
        if !adj[a].contains(&c) { adj[a].push(c); }
        if !adj[b].contains(&a) { adj[b].push(a); }
        if !adj[b].contains(&c) { adj[b].push(c); }
        if !adj[c].contains(&a) { adj[c].push(a); }
        if !adj[c].contains(&b) { adj[c].push(b); }
    }

    adj
}
```

Smooth brush with two-pass approach (read then write):
```rust
fn smooth_brush(mesh: &mut Mesh, center: Vec3, radius: f32, strength: f32) {
    let adjacency = build_adjacency(mesh);

    // Pass 1: compute new positions without mutating
    let new_positions: Vec<Option<[f32; 3]>> = (0..mesh.vertices.len()).map(|i| {
        let pos = Vec3::from(mesh.vertices[i].position);
        let dist = pos.distance(center);
        if dist < radius {
            let weight = 1.0 - (dist / radius);
            let neighbors = &adjacency[i];
            if neighbors.is_empty() {
                return None;
            }
            let avg: Vec3 = neighbors.iter()
                .map(|&ni| Vec3::from(mesh.vertices[ni].position))
                .sum::<Vec3>() / neighbors.len() as f32;
            let smoothed = pos.lerp(avg, weight * strength);
            Some(smoothed.to_array())
        } else {
            None
        }
    }).collect();

    // Pass 2: apply
    for (i, new_pos) in new_positions.into_iter().enumerate() {
        if let Some(p) = new_pos {
            mesh.vertices[i].position = p;
        }
    }
}
```

Sphere SDF stamp with re-meshing:
```rust
fn stamp_sphere(
    mesh: &mut Mesh,
    center: Vec3,
    radius: f32,
    smooth_k: f32,
    mc_resolution: u32,
    add: bool,
) {
    let old_mesh = Mesh {
        vertices: mesh.vertices.clone(),
        indices: mesh.indices.clone(),
    };

    // 1. Compute mesh AABB with margin, expanded for sphere
    let (mut min_b, mut max_b) = mesh_bounds(mesh);
    let sphere_min = center - Vec3::splat(radius);
    let sphere_max = center + Vec3::splat(radius);
    min_b = min_b.min(sphere_min);
    max_b = max_b.max(sphere_max);
    let margin = (max_b - min_b).max_element() * 0.1;
    min_b -= Vec3::splat(margin);
    max_b += Vec3::splat(margin);

    // 2. Build combined SDF closure
    let combined_sdf = |p: Vec3| -> f32 {
        let mesh_d = approximate_mesh_sdf(p, &old_mesh);
        let sphere_d = p.distance(center) - radius;
        if add {
            smooth_union(mesh_d, sphere_d, smooth_k)
        } else {
            smooth_subtraction(mesh_d, sphere_d, smooth_k)
        }
    };

    // 3. Run Marching Cubes
    let mc = MarchingCubes::new(mc_resolution);
    let default_color = [0.6, 0.6, 0.6, 1.0];
    let (new_block_verts, new_indices) = mc.generate_mesh(combined_sdf, min_b, max_b, default_color);

    // 4. Convert BlockVertex to Vertex and transfer colors from old mesh
    let new_verts: Vec<Vertex> = new_block_verts.iter().map(|bv| Vertex {
        position: bv.position,
        normal: bv.normal,
        color: nearest_vertex_color(&old_mesh, Vec3::from_array(bv.position)),
    }).collect();

    // 5. Replace mesh data
    mesh.vertices = new_verts;
    mesh.indices = new_indices;
}
```

Nearest vertex color transfer:
```rust
fn nearest_vertex_color(old_mesh: &Mesh, pos: Vec3) -> [f32; 4] {
    let mut best_dist_sq = f32::MAX;
    let mut best_color = [0.6, 0.6, 0.6, 1.0];
    for v in &old_mesh.vertices {
        let d = Vec3::from(v.position).distance_squared(pos);
        if d < best_dist_sq {
            best_dist_sq = d;
            best_color = v.color;
        }
    }
    best_color
}
```

Normal recomputation after smooth brush:
```rust
fn recompute_normals(mesh: &mut Mesh) {
    // Zero all normals
    for v in &mut mesh.vertices {
        v.normal = [0.0, 0.0, 0.0];
    }

    // Accumulate face normals (area-weighted via cross product magnitude)
    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 { continue; }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let p0 = Vec3::from(mesh.vertices[i0].position);
        let p1 = Vec3::from(mesh.vertices[i1].position);
        let p2 = Vec3::from(mesh.vertices[i2].position);

        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let face_normal = edge1.cross(edge2); // Length = 2x triangle area

        for &idx in &[i0, i1, i2] {
            mesh.vertices[idx].normal[0] += face_normal.x;
            mesh.vertices[idx].normal[1] += face_normal.y;
            mesh.vertices[idx].normal[2] += face_normal.z;
        }
    }

    // Normalize
    for v in &mut mesh.vertices {
        let n = Vec3::from(v.normal);
        v.normal = n.normalize_or_zero().to_array();
    }
}
```

Mesh bounds helper:
```rust
fn mesh_bounds(mesh: &Mesh) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for v in &mesh.vertices {
        let p = Vec3::from(v.position);
        min = min.min(p);
        max = max.max(p);
    }
    (min, max)
}
```

Integration in `mod.rs` update loop:
```rust
// In AssetEditor::update(), when stage == EditorStage::Sculpt:
match key {
    Key1 => self.sculpt.set_tool(SculptTool::FaceExtrude),
    Key2 => self.sculpt.set_tool(SculptTool::VertexPull),
    Key3 => self.sculpt.set_tool(SculptTool::EdgePull),
    Key4 => self.sculpt.set_tool(SculptTool::Smooth),
    Key5 => self.sculpt.set_tool(SculptTool::AddSphere),
    Key6 => self.sculpt.set_tool(SculptTool::SubtractSphere),
    _ => {}
}

// Scroll wheel adjusts brush radius (or smooth_factor with Shift)
if scroll_delta != 0.0 {
    if shift_held {
        self.sculpt.smooth_factor =
            (self.sculpt.smooth_factor + scroll_delta * 0.01).clamp(0.01, 1.0);
    } else {
        self.sculpt.brush_radius =
            (self.sculpt.brush_radius + scroll_delta * 0.05).clamp(0.1, 5.0);
    }
}

// Mouse input -> sculpt
self.sculpt.handle_input(
    &mut self.draft.mesh, cursor_world_pos, mouse_pressed, mouse_dragging
);

// Re-upload mesh to GPU if modified
if self.sculpt.is_dirty() {
    upload_mesh_to_gpu(
        &self.draft.mesh, device, queue,
        &mut self.preview_vb, &mut self.preview_ib,
    );
    self.sculpt.clear_dirty();
}
```

Existing engine function signatures referenced by the bridge:
```rust
// engine/src/render/sdf_operations.rs
pub fn smooth_union(d1: f32, d2: f32, k: f32) -> f32;
pub fn smooth_subtraction(d1: f32, d2: f32, k: f32) -> f32;

// engine/src/render/marching_cubes.rs
impl MarchingCubes {
    pub fn new(resolution: u32) -> Self;
    pub fn generate_mesh<F>(&self, sdf: F, min: Vec3, max: Vec3, color: [f32; 4])
        -> (Vec<BlockVertex>, Vec<u32>)
    where F: Fn(Vec3) -> f32;
}

// engine/src/render/sculpting.rs
impl SculptingManager {
    pub fn new() -> Self;
    pub fn set_enabled(&mut self, enabled: bool);
    pub fn set_mode(&mut self, mode: SculptMode);
    pub fn try_select_face(&mut self, ray_origin: Vec3, ray_dir: Vec3,
        manager: &BuildingBlockManager) -> bool;
    pub fn start_drag(&mut self, start_pos: Vec3,
        manager: &BuildingBlockManager) -> bool;
    pub fn update_drag(&mut self, current_pos: Vec3,
        manager: &mut BuildingBlockManager) -> Vec<u32>;
    pub fn end_drag(&mut self) -> Vec<u32>;
    pub fn cancel(&mut self);
}
```

## Acceptance Criteria
- [ ] `sculpt_bridge.rs` exists with `SculptBridge` struct and `SculptTool` enum (6 variants)
- [ ] `SculptBridge` wraps the existing `SculptingManager` from `engine/src/render/sculpting.rs`
- [ ] `FaceExtrude` delegates to `ExtrusionOperation` for face-based extrusion
- [ ] `VertexPull` delegates to `VertexSelection` for vertex dragging
- [ ] `EdgePull` delegates to `EdgeSelection` for edge dragging
- [ ] Smooth brush relaxes vertex positions toward neighbor average with linear distance falloff
- [ ] Smooth brush uses a two-pass approach (read-then-write) to avoid asymmetric drift
- [ ] `AddSphere` stamps a sphere via `smooth_union()` from `engine/src/render/sdf_operations.rs` and re-meshes with `MarchingCubes` from `engine/src/render/marching_cubes.rs`
- [ ] `SubtractSphere` carves via `smooth_subtraction()` from `engine/src/render/sdf_operations.rs` and re-meshes with `MarchingCubes`
- [ ] Vertex colors are transferred from old mesh to new mesh after SDF re-meshing (nearest-vertex lookup)
- [ ] Normals are recomputed after smooth brush operations (area-weighted face normal accumulation)
- [ ] `brush_radius` adjustable via scroll wheel (clamped 0.1..5.0)
- [ ] `smooth_factor` adjustable via Shift+scroll (clamped 0.01..1.0)
- [ ] Tool switching via keys 1-6 in Stage 3
- [ ] `mod.rs` adds `pub mod sculpt_bridge;` and routes Stage 3 input/render to `SculptBridge`
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo check --bin battle_arena` passes with 0 errors

## Verification Commands
- `cmd`: `test -f /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `sculpt_bridge.rs file exists`
- `cmd`: `grep -c 'SculptBridge\|SculptTool' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `SculptBridge and SculptTool types are defined`
- `cmd`: `grep -c 'FaceExtrude\|VertexPull\|EdgePull\|Smooth\|AddSphere\|SubtractSphere' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `All six sculpt tool variants are defined`
- `cmd`: `grep -c 'smooth_union\|smooth_subtraction' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `SDF boolean operations are used for sphere stamp/carve`
- `cmd`: `grep -c 'smooth_brush\|build_adjacency\|average_neighbor' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `Smooth brush algorithm with adjacency is implemented`
- `cmd`: `grep -c 'MarchingCubes\|generate_mesh' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `Marching Cubes re-meshing is used for sphere stamp/carve`
- `cmd`: `grep -c 'recompute_normals\|transfer_vertex_colors\|nearest_vertex_color' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `Normal recomputation and color transfer are implemented`
- `cmd`: `grep -c 'pub mod sculpt_bridge' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `sculpt_bridge module registered in mod.rs`
- `cmd`: `grep -c 'SculptingManager' /home/hungaromakker/battle_tok/src/game/asset_editor/sculpt_bridge.rs`
  `expect_gt`: 0
  `description`: `Wraps existing SculptingManager from engine`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with sculpt bridge module`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles unchanged`

## Success Looks Like
The artist finishes extruding a tree trunk shape in Stage 2 and presses `3` to enter Stage 3 (Sculpt). The toolbar shows six tool icons with keyboard shortcuts. They start with the Smooth brush (key `4`) and drag across the mesh surface -- the noisy marching-cubes facets melt into smooth, organic curves. They scroll up to increase the brush radius and make broader sweeps across the canopy. Next they switch to AddSphere (key `5`) and click on the top of the trunk -- a smooth blob of material appears, blended seamlessly into the existing geometry via `smooth_union`. They click several more times to build up a bulbous canopy shape, each sphere stamp adding material with smooth transitions controlled by `smooth_factor`. They switch to SubtractSphere (key `6`) and click on the trunk -- a smooth concavity appears, carved cleanly from the mesh. They use this to create a hollow knot in the tree trunk. The vertex colors from their earlier painting (if any) survive the re-meshing operations, transferred by nearest-vertex lookup. Finally they switch to FaceExtrude (key `1`), select a face on the canopy, and drag outward to create a branch stub. All operations feel responsive and produce artifact-free geometry. The orbit camera lets them inspect from every angle.

## Dependencies
- Depends on: US-P4-006 (needs the pump/inflate extrusion to have generated a 3D mesh to sculpt)

## Complexity
- Complexity: normal
- Min iterations: 1
