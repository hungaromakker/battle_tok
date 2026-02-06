# US-P4-014: World Placement System

## Description
Create `src/game/asset_editor/placement.rs` with a `PlacementSystem` struct that enables placing saved assets into the game world. A ghost preview follows the cursor at terrain height, snapping to the ground with optional normal alignment. Single-click places one asset with a variety seed derived from position. A scatter brush (Ctrl+Click) places multiple assets in a circular area using Poisson disk sampling to avoid overlap. The editor provides rotate (R) and scale ([ / ]) controls for the preview. Each placed asset is stored as a `PlacedAsset` with minimal data — the variety system regenerates the visual variation from the seed. The asset editor is a **separate binary** (`battle_editor`); `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
Asset creation is only half the pipeline — placement is where assets become a living world. The placement system bridges the gap between the asset editor and the game environment. Ghost preview eliminates guesswork by showing exactly where and how an asset will appear before committing. The scatter brush enables rapid environment decoration — placing dozens of grass tufts or rocks in seconds with natural-looking distribution (Poisson disk sampling prevents the "grid pattern" look of uniform placement). Ground conforming ensures assets sit naturally on uneven terrain instead of floating or clipping. The variety seed ties into US-P4-013 so every placed instance looks unique without storing per-instance mesh data.

## Goal
Create `src/game/asset_editor/placement.rs` with ghost preview, single-click placement, scatter brush with Poisson disk sampling, ground conforming, and rotate/scale controls, all integrated with the variety system for per-instance variation.

## Files to Create/Modify
- **Create** `src/game/asset_editor/placement.rs` — `PlacementSystem`, `PlacedAsset`, `ScatterBrush`, ghost preview, placement logic
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod placement;`, add `placement: PlacementSystem` field to `AssetEditor`, wire placement input/rendering

## Implementation Steps
1. Define `PlacedAsset` struct — the minimal per-instance data:
   ```rust
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct PlacedAsset {
       pub asset_id: String,         // references library entry
       pub position: [f32; 3],       // world position
       pub variety_seed: u32,        // derived from position via seed_from_position()
       pub manual_rotation: f32,     // additional Y rotation (radians) from R key
       pub manual_scale: f32,        // additional scale multiplier from [ / ] keys
       pub ground_normal: [f32; 3],  // terrain normal at placement point
   }
   ```
2. Define `ScatterBrush` struct:
   ```rust
   pub struct ScatterBrush {
       pub radius: f32,           // brush circle radius (default 5.0)
       pub density: f32,          // assets per unit area (default 0.5)
       pub min_spacing: f32,      // minimum distance between placements (default 1.0)
   }
   ```
3. Define `PlacementSystem` struct:
   - `active: bool` — placement mode enabled
   - `selected_asset_id: Option<String>` — which asset to place (from library)
   - `ghost_position: [f32; 3]` — current cursor world position on terrain
   - `ghost_rotation: f32` — preview rotation (accumulated from R key)
   - `ghost_scale: f32` — preview scale (adjusted by [ / ] keys)
   - `placed_assets: Vec<PlacedAsset>` — all placed instances
   - `scatter: ScatterBrush` — scatter brush params
4. Implement ghost preview update:
   - Each frame, raycast from cursor through camera into the scene
   - Find intersection with terrain (Y-plane or actual terrain mesh)
   - Set `ghost_position` to intersection point
   - Render the selected asset mesh at ghost position with semi-transparent overlay
5. Implement single-click placement:
   - On left-click (without Ctrl), create `PlacedAsset`:
     - `position` = `ghost_position`
     - `variety_seed` = `seed_from_position(position.x, position.z)` (from variety.rs)
     - `manual_rotation` = current `ghost_rotation`
     - `manual_scale` = current `ghost_scale`
     - `ground_normal` = terrain normal at position (default `[0, 1, 0]` for flat)
   - Push to `placed_assets`
6. Implement rotate and scale controls:
   - R key: increment `ghost_rotation` by 15 degrees (PI/12 radians)
   - `]` key: multiply `ghost_scale` by 1.1
   - `[` key: multiply `ghost_scale` by 0.9 (clamped to [0.1, 5.0])
7. Implement Poisson disk sampling for scatter brush:
   ```rust
   pub fn poisson_disk_sample(center: [f32; 2], radius: f32, min_spacing: f32, density: f32, seed: u32) -> Vec<[f32; 2]> {
       let mut rng = SimpleRng::new(seed);
       let max_points = (std::f32::consts::PI * radius * radius * density) as usize;
       let mut points: Vec<[f32; 2]> = Vec::new();
       let max_attempts = max_points * 30;
       for _ in 0..max_attempts {
           if points.len() >= max_points { break; }
           let angle = rng.range(0.0, std::f32::consts::TAU);
           let r = rng.range(0.0, radius);
           let candidate = [center[0] + r * angle.cos(), center[1] + r * angle.sin()];
           let too_close = points.iter().any(|p| {
               let dx = p[0] - candidate[0];
               let dy = p[1] - candidate[1];
               (dx * dx + dy * dy).sqrt() < min_spacing
           });
           if !too_close { points.push(candidate); }
       }
       points
   }
   ```
8. Implement scatter placement (Ctrl+Click):
   - Generate Poisson disk points within scatter brush radius around cursor
   - For each point, create a `PlacedAsset` with position-derived variety seed
   - Add all to `placed_assets`
9. Implement ground conforming:
   - For each placement point, raycast downward to find terrain surface
   - Align asset Y-axis to terrain normal (tilt to match slope)
   - Store `ground_normal` in `PlacedAsset` for rendering
10. Wire into `mod.rs`: when an asset is selected from the library panel, activate placement mode. Render ghost preview during update. Handle click, Ctrl+click, R, [, ] inputs.

## Code Patterns
```rust
pub struct PlacementSystem {
    pub active: bool,
    pub selected_asset_id: Option<String>,
    pub ghost_position: [f32; 3],
    pub ghost_rotation: f32,
    pub ghost_scale: f32,
    pub placed_assets: Vec<PlacedAsset>,
    pub scatter: ScatterBrush,
}

impl PlacementSystem {
    pub fn place_single(&mut self, terrain_normal: [f32; 3]) {
        if let Some(ref asset_id) = self.selected_asset_id {
            let seed = crate::game::asset_editor::variety::seed_from_position(
                self.ghost_position[0], self.ghost_position[2]
            );
            self.placed_assets.push(PlacedAsset {
                asset_id: asset_id.clone(),
                position: self.ghost_position,
                variety_seed: seed,
                manual_rotation: self.ghost_rotation,
                manual_scale: self.ghost_scale,
                ground_normal: terrain_normal,
            });
        }
    }

    pub fn scatter_place(&mut self, terrain_query: &impl Fn([f32; 2]) -> ([f32; 3], [f32; 3])) {
        let center = [self.ghost_position[0], self.ghost_position[2]];
        let seed = crate::game::asset_editor::variety::seed_from_position(center[0], center[1]);
        let points = poisson_disk_sample(center, self.scatter.radius, self.scatter.min_spacing, self.scatter.density, seed);
        for pt in points {
            let (pos, normal) = terrain_query(pt);
            let variety_seed = crate::game::asset_editor::variety::seed_from_position(pt[0], pt[1]);
            if let Some(ref asset_id) = self.selected_asset_id {
                self.placed_assets.push(PlacedAsset {
                    asset_id: asset_id.clone(),
                    position: pos,
                    variety_seed,
                    manual_rotation: self.ghost_rotation,
                    manual_scale: self.ghost_scale,
                    ground_normal: normal,
                });
            }
        }
    }
}
```

## Acceptance Criteria
- [ ] `placement.rs` exists with `PlacementSystem`, `PlacedAsset`, `ScatterBrush` types
- [ ] Ghost preview renders selected asset at cursor position on terrain
- [ ] Left-click places a single asset with variety seed derived from world position
- [ ] R key rotates ghost preview by 15-degree increments
- [ ] `[` and `]` keys scale ghost preview (clamped to reasonable range)
- [ ] Ctrl+Click activates scatter brush, placing multiple assets with Poisson disk sampling
- [ ] Poisson disk sampling maintains minimum spacing between placed assets
- [ ] Ground conforming aligns assets to terrain normal
- [ ] `PlacedAsset` stores minimal data: asset_id, position, variety_seed, manual rotation/scale, ground_normal
- [ ] Variety seed is deterministic: same position always produces same seed
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo check --bin battle_arena` passes with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with placement module`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles unchanged`
- `cmd`: `test -f src/game/asset_editor/placement.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `placement.rs file exists`
- `cmd`: `grep -c 'PlacementSystem\|PlacedAsset\|ScatterBrush' src/game/asset_editor/placement.rs`
  `expect_gt`: 0
  `description`: `Core placement types are defined`
- `cmd`: `grep -c 'poisson_disk_sample\|place_single\|scatter_place\|ghost_position' src/game/asset_editor/placement.rs`
  `expect_gt`: 0
  `description`: `Placement functions are implemented`
- `cmd`: `grep -c 'seed_from_position\|variety_seed' src/game/asset_editor/placement.rs`
  `expect_gt`: 0
  `description`: `Variety system integration exists`
- `cmd`: `grep -c 'pub mod placement' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `placement module registered in mod.rs`

## Success Looks Like
The artist selects an oak tree from the library panel. A semi-transparent ghost tree follows their cursor, hovering at terrain height. They click and a tree appears with a unique variation (slightly different size and color tint). They press R twice to rotate the next tree 30 degrees, then click again. They hold Ctrl and click on a grassy hillside — a dozen grass tufts appear in a natural-looking cluster with no overlaps. Each tuft is slightly different due to its position-based variety seed. On a slope, the assets tilt to match the terrain angle. The scatter brush makes decorating large areas fast, while single-click gives precise control.

## Dependencies
- Depends on: US-P4-013 (variety system for per-instance variation), US-P4-012 (library panel for asset selection)

## Complexity
- Complexity: complex
- Min iterations: 2
