# US-P4-014: World Placement System

## Description
Create `src/game/asset_editor/placement.rs` with a `PlacementSystem` for placing saved assets into the game world. Features include: a ghost preview that follows the cursor on terrain, click-to-place with variety seed derived from world position, scatter brush (Ctrl+Click) using Poisson disk sampling for natural mass placement, rotation (R key) and scale ([ / ] keys) controls, ground conforming via terrain raycast, and conversion to `CreatureInstance` (from `engine/src/render/instancing.rs`) for GPU instanced rendering. Each `PlacedAsset` stores minimal data -- position, asset_id, variety_seed, rotation, scale -- because the Variety System (US-P4-013) regenerates all visual variation deterministically from the seed. The editor is a separate binary (`cargo run --bin battle_editor`). `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
The placement system is where asset creation meets world building. It bridges the gap between having individual assets (from the editor pipeline) and having a populated game world. Without placement tools, artists would need to manually specify coordinates in data files. The ghost preview shows exactly where the asset will land before committing. The scatter brush enables rapid environment dressing -- painting trees across a hillside or rocks along a riverbed with a single drag.

**Poisson disk sampling** is the key algorithm for scatter brush. Unlike pure random placement (which creates visible clusters and gaps), Poisson disk sampling guarantees a minimum distance between all points while still looking natural. This produces the "blue noise" distribution seen in real forests and rock fields -- not too regular (grid), not too chaotic (random), but naturally spaced.

**Ground conforming** via raycast means assets sit naturally on terrain regardless of slope. Combined with the Variety System's tilt parameter, placed assets can subtly lean to match terrain normal, making them look rooted rather than floating.

The system stores minimal data per placement (position + seed + rotation + scale = ~28 bytes) because the Variety System regenerates all visual variation deterministically. This means a forest of 10,000 trees costs only ~280KB of placement data.

## Goal
Create `src/game/asset_editor/placement.rs` with `PlacementSystem` and `PlacedAsset` providing ghost preview, single placement, scatter brush with Poisson disk sampling, rotation/scale controls, ground-conforming placement, and conversion to `CreatureInstance` for GPU rendering.

## Files to Create/Modify
- **Create** `src/game/asset_editor/placement.rs` -- `PlacementSystem`, `PlacedAsset`, ghost preview, single/scatter placement, Poisson disk sampling, ground conforming, instance generation
- **Modify** `src/game/asset_editor/mod.rs` -- Add `pub mod placement;`, add `placement: PlacementSystem` field to `AssetEditor`, wire input routing for R, [, ], click, and Ctrl+click
- **Modify** `src/bin/battle_editor.rs` -- Forward placement keyboard/mouse input when library asset is selected, render placed asset instances

## Implementation Steps

1. Define placement data structures:
   ```rust
   use crate::game::asset_editor::variety::{seed_from_position, generate_variety, VarietyParams};
   use serde::{Deserialize, Serialize};
   use glam::Vec3;

   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct PlacedAsset {
       pub asset_id: String,        // References library entry id
       pub position: Vec3,          // World position
       pub variety_seed: u32,       // For deterministic variation
       pub manual_rotation: f32,    // Manual Y rotation (radians)
       pub manual_scale: f32,       // Manual uniform scale multiplier
   }

   pub struct PlacementSystem {
       pub selected_asset: Option<String>,  // Asset ID from library
       pub ghost_position: Vec3,            // Preview position (follows cursor)
       pub ghost_rotation: f32,             // R key to rotate
       pub ghost_scale: f32,                // [ / ] keys to scale
       pub placed_instances: Vec<PlacedAsset>,
       pub scatter_mode: bool,              // Ctrl+Click activates
       pub scatter_radius: f32,             // Scroll to adjust (default 5.0)
       pub scatter_density: f32,            // Instances per square meter (default 0.3)
       pub scatter_min_spacing: f32,        // Poisson disk min distance (default 2.0)
   }
   ```

2. Implement ghost preview update that tracks cursor position on terrain:
   ```rust
   impl PlacementSystem {
       pub fn new() -> Self {
           Self {
               selected_asset: None,
               ghost_position: Vec3::ZERO,
               ghost_rotation: 0.0,
               ghost_scale: 1.0,
               placed_instances: Vec::new(),
               scatter_mode: false,
               scatter_radius: 5.0,
               scatter_density: 0.3,
               scatter_min_spacing: 2.0,
           }
       }

       /// Update ghost position to follow cursor on terrain surface.
       pub fn update_ghost(&mut self, cursor_world_pos: Vec3) {
           self.ghost_position = cursor_world_pos;
       }

       /// Rotate ghost preview by delta radians. R key = +15 degrees.
       pub fn rotate_ghost(&mut self, delta: f32) {
           self.ghost_rotation += delta;
           self.ghost_rotation = self.ghost_rotation.rem_euclid(std::f32::consts::TAU);
       }

       /// Scale ghost preview. [ key = -0.1, ] key = +0.1. Clamped to [0.1, 5.0].
       pub fn scale_ghost(&mut self, delta: f32) {
           self.ghost_scale = (self.ghost_scale + delta).clamp(0.1, 5.0);
       }
   }
   ```

3. Implement single-click placement with variety seed from position:
   ```rust
   impl PlacementSystem {
       /// Place a single asset at the current ghost position.
       pub fn place(&mut self) -> Option<PlacedAsset> {
           let asset_id = self.selected_asset.as_ref()?.clone();
           let seed = seed_from_position(
               self.ghost_position.x,
               self.ghost_position.y,
               self.ghost_position.z,
           );
           let placed = PlacedAsset {
               asset_id,
               position: self.ghost_position,
               variety_seed: seed,
               manual_rotation: self.ghost_rotation,
               manual_scale: self.ghost_scale,
           };
           self.placed_instances.push(placed.clone());
           Some(placed)
       }
   }
   ```

4. Implement Poisson disk sampling for scatter brush (the core distribution algorithm):
   ```rust
   use crate::game::asset_editor::variety::SimpleRng;

   /// Generate naturally-distributed points within a circle using Poisson disk sampling.
   /// Guarantees minimum spacing between all points (blue noise distribution).
   pub fn poisson_disk_sample(
       center: [f32; 2],     // XZ center of brush
       radius: f32,          // Brush radius
       min_dist: f32,        // Minimum spacing between points
       max_attempts: u32,    // Attempts per active point (30 is typical)
       seed: u32,
   ) -> Vec<[f32; 2]> {
       let mut rng = SimpleRng::new(seed);
       let mut points: Vec<[f32; 2]> = Vec::new();
       let mut active: Vec<usize> = Vec::new();

       // Spatial hash grid for fast neighbor lookup
       let cell_size = min_dist / std::f32::consts::SQRT_2;
       let grid_side = (2.0 * radius / cell_size).ceil() as usize + 1;
       let mut grid: Vec<Option<usize>> = vec![None; grid_side * grid_side];

       // Start with center point
       points.push([center[0], center[1]]);
       active.push(0);
       let gi = grid_index([center[0], center[1]], center, radius, cell_size, grid_side);
       if gi < grid.len() { grid[gi] = Some(0); }

       while !active.is_empty() {
           let active_idx = (rng.next_u32() as usize) % active.len();
           let point_idx = active[active_idx];
           let point = points[point_idx];
           let mut found = false;

           for _ in 0..max_attempts {
               // Generate random candidate in annulus [min_dist, 2*min_dist]
               let angle = rng.range(0.0, std::f32::consts::TAU);
               let dist = rng.range(min_dist, 2.0 * min_dist);
               let candidate = [
                   point[0] + angle.cos() * dist,
                   point[1] + angle.sin() * dist,
               ];

               // Check within brush radius
               let dx = candidate[0] - center[0];
               let dy = candidate[1] - center[1];
               if dx * dx + dy * dy > radius * radius { continue; }

               // Check grid neighbors for minimum distance violation
               if !has_nearby_point(&candidate, &grid, &points, cell_size, min_dist, center, radius, grid_side) {
                   let idx = points.len();
                   points.push(candidate);
                   active.push(idx);
                   let gi = grid_index(candidate, center, radius, cell_size, grid_side);
                   if gi < grid.len() { grid[gi] = Some(idx); }
                   found = true;
               }
           }

           if !found {
               active.swap_remove(active_idx);
           }
       }

       points
   }

   fn grid_index(
       point: [f32; 2], center: [f32; 2], radius: f32,
       cell_size: f32, grid_side: usize,
   ) -> usize {
       let gx = ((point[0] - center[0] + radius) / cell_size) as usize;
       let gy = ((point[1] - center[1] + radius) / cell_size) as usize;
       gy.min(grid_side - 1) * grid_side + gx.min(grid_side - 1)
   }

   fn has_nearby_point(
       candidate: &[f32; 2], grid: &[Option<usize>], points: &[[f32; 2]],
       cell_size: f32, min_dist: f32, center: [f32; 2], radius: f32, grid_side: usize,
   ) -> bool {
       let gx = ((candidate[0] - center[0] + radius) / cell_size) as i32;
       let gy = ((candidate[1] - center[1] + radius) / cell_size) as i32;

       // Check 5x5 neighborhood in grid
       for dy in -2..=2 {
           for dx in -2..=2 {
               let nx = gx + dx;
               let ny = gy + dy;
               if nx < 0 || ny < 0 || nx >= grid_side as i32 || ny >= grid_side as i32 { continue; }
               let idx = ny as usize * grid_side + nx as usize;
               if let Some(pi) = grid[idx] {
                   let p = points[pi];
                   let ddx = candidate[0] - p[0];
                   let ddy = candidate[1] - p[1];
                   if ddx * ddx + ddy * ddy < min_dist * min_dist {
                       return true;
                   }
               }
           }
       }
       false
   }
   ```

5. Implement scatter placement using Poisson disk + ground raycast:
   ```rust
   impl PlacementSystem {
       /// Scatter multiple assets in a circle around ghost position.
       /// Uses Poisson disk sampling for natural distribution.
       /// `ground_raycast` returns terrain height at given XZ coordinates.
       pub fn scatter(
           &mut self,
           ground_raycast: &dyn Fn(f32, f32) -> Option<f32>,
       ) -> Vec<PlacedAsset> {
           let asset_id = match &self.selected_asset {
               Some(id) => id.clone(),
               None => return Vec::new(),
           };

           let center_seed = seed_from_position(
               self.ghost_position.x, 0.0, self.ghost_position.z,
           );

           let sample_points = poisson_disk_sample(
               [self.ghost_position.x, self.ghost_position.z],
               self.scatter_radius,
               self.scatter_min_spacing,
               30,  // max attempts per point
               center_seed,
           );

           let mut newly_placed = Vec::new();
           for pt in &sample_points {
               // Raycast to find ground height at this XZ position
               let ground_y = ground_raycast(pt[0], pt[1]).unwrap_or(0.0);
               let position = Vec3::new(pt[0], ground_y, pt[1]);
               let seed = seed_from_position(position.x, position.y, position.z);

               let placed = PlacedAsset {
                   asset_id: asset_id.clone(),
                   position,
                   variety_seed: seed,
                   manual_rotation: self.ghost_rotation,
                   manual_scale: self.ghost_scale,
               };
               self.placed_instances.push(placed.clone());
               newly_placed.push(placed);
           }
           newly_placed
       }
   }
   ```

6. Implement keyboard controls routing in `battle_editor.rs`:
   ```rust
   // R key: rotate ghost by 15 degrees
   if key == VirtualKeyCode::R && !modifiers.ctrl() {
       editor.placement.rotate_ghost(15.0_f32.to_radians());
   }

   // [ key: scale down by 0.1
   if key == VirtualKeyCode::LBracket {
       editor.placement.scale_ghost(-0.1);
   }

   // ] key: scale up by 0.1
   if key == VirtualKeyCode::RBracket {
       editor.placement.scale_ghost(0.1);
   }

   // Left click: place single asset
   if mouse_button == MouseButton::Left && !modifiers.ctrl() {
       editor.placement.place();
   }

   // Ctrl + Left click: scatter brush
   if mouse_button == MouseButton::Left && modifiers.ctrl() {
       editor.placement.scatter(&terrain_raycast);
   }
   ```

7. Implement conversion to `CreatureInstance` for GPU instanced rendering:
   ```rust
   use engine::render::instancing::CreatureInstance;

   impl PlacementSystem {
       /// Convert all placed assets to CreatureInstance data for GPU rendering.
       /// Combines manual transform with variety-generated transform.
       pub fn generate_instances(
           &self,
           variety_params: &VarietyParams,
       ) -> Vec<CreatureInstance> {
           self.placed_instances.iter().map(|pa| {
               let variety = generate_variety(variety_params, pa.variety_seed);

               // Combine manual and variety transforms
               let total_scale = pa.manual_scale * variety.scale.x; // uniform for instancing
               let total_rotation_y = pa.manual_rotation + variety.rotation_y;

               // Build rotation quaternion from Y rotation
               let rotation = glam::Quat::from_rotation_y(total_rotation_y);

               CreatureInstance::new(
                   pa.position.into(),
                   rotation.into(),
                   total_scale,
                   0, // baked_sdf_id
               )
           }).collect()
       }
   }
   ```

8. Implement save/load for placements persistence:
   ```rust
   const PLACEMENTS_PATH: &str = "assets/world/placements.json";

   impl PlacementSystem {
       pub fn save_placements(&self) -> Result<(), std::io::Error> {
           let json = serde_json::to_string_pretty(&self.placed_instances)?;
           std::fs::create_dir_all("assets/world")?;
           std::fs::write(PLACEMENTS_PATH, json)?;
           Ok(())
       }

       pub fn load_placements(&mut self) -> Result<(), std::io::Error> {
           if let Ok(data) = std::fs::read_to_string(PLACEMENTS_PATH) {
               self.placed_instances = serde_json::from_str(&data)?;
           }
           Ok(())
       }
   }
   ```

## Code Patterns
The Poisson disk sampling algorithm follows Bridson's fast algorithm (O(n) complexity):
```rust
// 1. Initialize with seed point
// 2. For each active point, try k random candidates in annulus [r, 2r]
// 3. Accept candidate if no existing point within distance r (checked via spatial grid)
// 4. If no valid candidate found after k attempts, deactivate point
// 5. Repeat until no active points remain
```

Instanced rendering uses the existing `CreatureInstance` struct (48 bytes per instance):
```rust
// CreatureInstance layout:
// position: [f32; 3]     (12 bytes)
// _pad0: u32              (4 bytes)
// rotation: [f32; 4]      (16 bytes) -- quaternion
// scale: f32              (4 bytes)
// baked_sdf_id: u32       (4 bytes)
// animation_state: u32    (4 bytes)
// tint_color: u32         (4 bytes) -- packed RGBA
```

## Acceptance Criteria
- [ ] `placement.rs` exists with `PlacementSystem` and `PlacedAsset` structs
- [ ] Ghost preview follows cursor position on terrain surface
- [ ] Click places asset at ghost position with variety seed derived from position
- [ ] R key rotates ghost by 15-degree increments
- [ ] [ and ] keys scale ghost up and down (clamped 0.1 to 5.0)
- [ ] Scatter brush (Ctrl+Click) uses Poisson disk sampling for natural distribution
- [ ] Poisson disk sampling respects minimum spacing between all placed objects
- [ ] Ground conforming uses raycast to place assets at terrain height
- [ ] `generate_instances()` produces valid `CreatureInstance` data for GPU rendering
- [ ] Placements are saved to / loaded from `assets/world/placements.json`
- [ ] `PlacedAsset` derives `Serialize` and `Deserialize`
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check` passes with 0 errors

## Verification Commands
- `cmd`: `test -f /home/hungaromakker/battle_tok/src/game/asset_editor/placement.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `placement.rs module exists`
- `cmd`: `grep -c 'PlacementSystem\|PlacedAsset\|poisson_disk_sample' /home/hungaromakker/battle_tok/src/game/asset_editor/placement.rs`
  `expect_gt`: 0
  `description`: `Placement types and Poisson sampling defined`
- `cmd`: `grep -c 'scatter\|ghost_position\|rotate_ghost\|scale_ghost\|generate_instances' /home/hungaromakker/battle_tok/src/game/asset_editor/placement.rs`
  `expect_gt`: 0
  `description`: `Placement functions implemented`
- `cmd`: `grep -c 'pub mod placement' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `placement module registered in mod.rs`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `Project compiles`

## Success Looks Like
The artist selects an oak tree from the library. A translucent ghost tree follows their cursor over the terrain, always sitting on the ground surface. They press R a few times to rotate it, ] to make it bigger. They click and a tree appears exactly where the ghost was, with a unique variety variation determined by the position. They hold Ctrl and click on a hillside -- a natural-looking cluster of trees fills a circle. Each tree is slightly different (variety system) and all sit properly on the terrain surface with no overlapping (Poisson disk). They save and the placements persist across editor restarts. The world starts to feel like a real environment.

## Dependencies
- Depends on: US-P4-012 (needs library to select assets for placement), US-P4-013 (needs variety for per-instance variation and seed_from_position)

## Complexity
- Complexity: complex
- Min iterations: 2
