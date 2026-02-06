# US-P4-014: World Placement System

## Description
Create `src/game/asset_editor/placement.rs` with a `PlacementSystem` for placing saved assets into the game world. Features include: a ghost preview that follows the cursor on terrain, click-to-place with variety seed derived from world position, scatter brush (Ctrl+Click) using Poisson disk sampling for natural mass placement, rotation (R key) and scale ([ / ] keys) controls, ground conforming via terrain raycast, and conversion to `CreatureInstance` (from `engine/src/render/instancing.rs`) for GPU instanced rendering. Each `PlacedAsset` stores minimal data -- position, asset_id, variety_seed, rotation, scale -- because the Variety System (US-P4-013) regenerates all visual variation deterministically from the seed. The editor is a separate binary (`cargo run --bin battle_editor`). `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
The placement system is where asset creation meets world building. It bridges the gap between having individual assets (from the editor pipeline) and having a populated game world. Without placement tools, artists would need to manually specify coordinates in data files. The ghost preview shows exactly where the asset will land before committing. The scatter brush enables rapid environment dressing -- painting trees across a hillside or rocks along a riverbed with a single drag.

**Poisson disk sampling** is the key algorithm for scatter brush. Unlike pure random placement (which creates visible clusters and gaps), Poisson disk sampling guarantees a minimum distance between all points while still looking natural. This produces the "blue noise" distribution seen in real forests and rock fields.

**Ground conforming** via raycast means assets sit naturally on terrain regardless of slope. The system stores minimal data per placement (position + seed + rotation + scale) because the Variety System regenerates all visual variation deterministically.

## Goal
Create `src/game/asset_editor/placement.rs` with `PlacementSystem` and `PlacedAsset` providing ghost preview, single placement, scatter brush with Poisson disk sampling, rotation/scale controls, ground-conforming placement, and conversion to `CreatureInstance` for GPU rendering.

## Files to Create/Modify
- **Create** `src/game/asset_editor/placement.rs` -- `PlacementSystem`, `PlacedAsset`, ghost preview, single/scatter placement, Poisson disk sampling, ground conforming, instance generation
- **Modify** `src/game/asset_editor/mod.rs` -- Add `pub mod placement;`, add `placement: PlacementSystem` field, wire input routing for R, [, ], click, Ctrl+click
- **Modify** `src/bin/battle_editor.rs` -- Forward placement input when library asset is selected

## Implementation Steps

1. Define placement data structures:
   ```rust
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct PlacedAsset {
       pub asset_id: String,
       pub position: Vec3,
       pub variety_seed: u32,
       pub manual_rotation: f32,
       pub manual_scale: f32,
   }

   pub struct PlacementSystem {
       pub selected_asset: Option<String>,
       pub ghost_position: Vec3,
       pub ghost_rotation: f32,
       pub ghost_scale: f32,
       pub placed_instances: Vec<PlacedAsset>,
       pub scatter_mode: bool,
       pub scatter_radius: f32,
       pub scatter_density: f32,
       pub scatter_min_spacing: f32,
   }
   ```

2. Implement ghost preview update:
   ```rust
   impl PlacementSystem {
       pub fn new() -> Self {
           Self {
               selected_asset: None, ghost_position: Vec3::ZERO,
               ghost_rotation: 0.0, ghost_scale: 1.0,
               placed_instances: Vec::new(), scatter_mode: false,
               scatter_radius: 5.0, scatter_density: 0.3, scatter_min_spacing: 2.0,
           }
       }
       pub fn update_ghost(&mut self, cursor_world_pos: Vec3) { self.ghost_position = cursor_world_pos; }
       pub fn rotate_ghost(&mut self, delta: f32) {
           self.ghost_rotation = (self.ghost_rotation + delta).rem_euclid(std::f32::consts::TAU);
       }
       pub fn scale_ghost(&mut self, delta: f32) {
           self.ghost_scale = (self.ghost_scale + delta).clamp(0.1, 5.0);
       }
   }
   ```

3. Implement single-click placement:
   ```rust
   pub fn place(&mut self) -> Option<PlacedAsset> {
       let asset_id = self.selected_asset.as_ref()?.clone();
       let seed = seed_from_position(self.ghost_position.x, self.ghost_position.y, self.ghost_position.z);
       let placed = PlacedAsset {
           asset_id, position: self.ghost_position, variety_seed: seed,
           manual_rotation: self.ghost_rotation, manual_scale: self.ghost_scale,
       };
       self.placed_instances.push(placed.clone());
       Some(placed)
   }
   ```

4. Implement Poisson disk sampling (Bridson's algorithm):
   ```rust
   pub fn poisson_disk_sample(
       center: [f32; 2], radius: f32, min_dist: f32, max_attempts: u32, seed: u32,
   ) -> Vec<[f32; 2]> {
       let mut rng = SimpleRng::new(seed);
       let cell_size = min_dist / std::f32::consts::SQRT_2;
       let grid_side = (2.0 * radius / cell_size).ceil() as usize + 1;
       let mut grid: Vec<Option<usize>> = vec![None; grid_side * grid_side];
       let mut points: Vec<[f32; 2]> = Vec::new();
       let mut active: Vec<usize> = Vec::new();
       // Initialize with center, then iteratively add points in annulus [r, 2r]
       // Check spatial grid for minimum distance violations
       // Remove from active list after max_attempts failures
       points
   }
   ```

5. Implement scatter placement with ground raycast:
   ```rust
   pub fn scatter(&mut self, ground_raycast: &dyn Fn(f32, f32) -> Option<f32>) -> Vec<PlacedAsset> {
       let asset_id = match &self.selected_asset { Some(id) => id.clone(), None => return Vec::new() };
       let center_seed = seed_from_position(self.ghost_position.x, 0.0, self.ghost_position.z);
       let sample_points = poisson_disk_sample(
           [self.ghost_position.x, self.ghost_position.z],
           self.scatter_radius, self.scatter_min_spacing, 30, center_seed,
       );
       let mut newly_placed = Vec::new();
       for pt in &sample_points {
           let ground_y = ground_raycast(pt[0], pt[1]).unwrap_or(0.0);
           let position = Vec3::new(pt[0], ground_y, pt[1]);
           let seed = seed_from_position(position.x, position.y, position.z);
           let placed = PlacedAsset {
               asset_id: asset_id.clone(), position, variety_seed: seed,
               manual_rotation: self.ghost_rotation, manual_scale: self.ghost_scale,
           };
           self.placed_instances.push(placed.clone());
           newly_placed.push(placed);
       }
       newly_placed
   }
   ```

6. Implement keyboard controls: R = rotate 15 degrees, [ = scale -0.1, ] = scale +0.1, Click = place, Ctrl+Click = scatter.

7. Implement conversion to `CreatureInstance` for GPU instanced rendering:
   ```rust
   pub fn generate_instances(&self, variety_params: &VarietyParams) -> Vec<CreatureInstance> {
       self.placed_instances.iter().map(|pa| {
           let variety = generate_variety(variety_params, pa.variety_seed);
           let total_scale = pa.manual_scale * variety.scale.x;
           let total_rotation_y = pa.manual_rotation + variety.rotation_y;
           let rotation = glam::Quat::from_rotation_y(total_rotation_y);
           CreatureInstance::new(pa.position.into(), rotation.into(), total_scale, 0)
       }).collect()
   }
   ```

8. Implement save/load for `assets/world/placements.json`.

## Code Patterns
Poisson disk sampling (Bridson's fast algorithm, O(n)):
```rust
// 1. Initialize with seed point
// 2. For each active point, try k candidates in annulus [r, 2r]
// 3. Accept if no existing point within distance r (spatial grid check)
// 4. Deactivate point after k failed attempts
```

`CreatureInstance` layout (48 bytes): position [f32;3], _pad0 u32, rotation [f32;4], scale f32, baked_sdf_id u32, animation_state u32, tint_color u32.

## Acceptance Criteria
- [ ] `placement.rs` exists with `PlacementSystem` and `PlacedAsset` structs
- [ ] Ghost preview follows cursor position on terrain surface
- [ ] Click places asset with variety seed derived from position
- [ ] R key rotates ghost by 15-degree increments
- [ ] [ and ] keys scale ghost (clamped 0.1 to 5.0)
- [ ] Scatter brush (Ctrl+Click) uses Poisson disk sampling
- [ ] Poisson disk sampling respects minimum spacing
- [ ] Ground conforming uses raycast for terrain height
- [ ] `generate_instances()` produces valid `CreatureInstance` data
- [ ] Placements saved to / loaded from `assets/world/placements.json`
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
