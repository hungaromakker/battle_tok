# US-P4-003: 2D Canvas Drawing — Freehand + Line Tools

## Description
Create the 2D drawing canvas system for Stage 1 (Draw2D) of the Asset Editor. This is where the user draws the 2D outline/silhouette that will later be extruded into a 3D mesh. The canvas provides freehand drawing (D key) with automatic point simplification via Ramer-Douglas-Peucker on mouse-up, and a line tool (L key) for straight edges. The canvas uses an orthographic projection with a grid, zoom, and pan.

## The Core Concept / Why This Matters
Every 3D asset in Battle Tök starts as a 2D outline. The Draw2D stage is where the user sketches the cross-section silhouette of the asset — think of it like drawing the side profile of a tree, rock, or structure. This outline is the input for the Extrude stage (US-P4-006/007), which inflates or revolves it into 3D geometry. The quality of the final asset depends on clean, well-simplified outlines. Ramer-Douglas-Peucker simplification is critical: freehand input generates hundreds of points, but we only need 20-50 to define the shape. Without simplification, extrusion would be painfully slow and produce noisy geometry.

## Goal
Create `src/game/asset_editor/canvas_2d.rs` with `Canvas2D`, `Outline2D`, and `DrawTool` types. Implement freehand and line drawing tools that produce clean `Outline2D` data. Render outlines as thin quads using the existing `add_quad()` function from `src/game/ui/text.rs`.

## Files to Create/Modify
- **Create** `src/game/asset_editor/canvas_2d.rs` — `Canvas2D` struct, `Outline2D` struct, `DrawTool` enum, freehand/line tools, RDP simplification, orthographic canvas with grid/zoom/pan
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod canvas_2d;`, integrate `Canvas2D` into `AssetEditor` for Stage 1, wire up input events
- **Modify** `src/bin/battle_editor.rs` — Forward keyboard (D, L, G keys) and mouse events to canvas when in Draw2D stage

## Implementation Steps
1. Define core types:
   ```rust
   pub enum DrawTool { Freehand, Line }
   
   pub struct Outline2D {
       pub points: Vec<[f32; 2]>,
       pub closed: bool,
   }
   
   pub struct Canvas2D {
       pub outlines: Vec<Outline2D>,
       pub active_outline: Option<Outline2D>,
       pub tool: DrawTool,
       pub zoom: f32,          // default 1.0 → 20x20 world units visible
       pub pan: [f32; 2],      // camera offset
       pub show_grid: bool,    // default true
       pub grid_size: f32,     // default 1.0
       // Internal state
       drawing: bool,
       line_start: Option<[f32; 2]>,
   }
   ```
2. Implement orthographic canvas coordinate system:
   - Origin at center of viewport
   - 1 unit = 1 grid cell
   - Default view: 20x20 units (±10 in each axis)
   - `screen_to_canvas(screen_x, screen_y, viewport_w, viewport_h) -> [f32; 2]` — converts screen pixels to canvas world coordinates accounting for zoom and pan
3. Implement freehand tool (D key activates):
   - On mouse-down: start new `Outline2D`, set `drawing = true`
   - On mouse-move while drawing: append point (in canvas coordinates)
   - On mouse-up: run Ramer-Douglas-Peucker simplification (epsilon = 0.05), set `drawing = false`, push to `outlines`
4. Implement Ramer-Douglas-Peucker simplification:
   ```rust
   fn rdp_simplify(points: &[[f32; 2]], epsilon: f32) -> Vec<[f32; 2]>
   ```
   - Find point with maximum perpendicular distance from line between first and last
   - If max distance > epsilon, recursively simplify both halves
   - Otherwise, return just the endpoints
5. Implement line tool (L key activates):
   - First click: record `line_start`
   - Second click: create `Outline2D` with 2 points (start, end), append to active outline or create new one
   - If an outline is active and its last point is near the click, extend that outline
6. Implement canvas navigation:
   - Scroll wheel: adjust `zoom` (multiplicative, clamp 0.1 to 10.0)
   - Middle-mouse drag: adjust `pan`
   - G key: toggle `show_grid`
7. Implement rendering (in `render()` method):
   - Grid: render as thin quads using `add_quad()`, color `[0.3, 0.3, 0.3, 0.5]`
   - Outlines: render each segment as a thin quad (width ~0.02 canvas units), color `[1.0, 1.0, 1.0, 1.0]`
   - Active drawing: render in-progress outline in a different color `[0.5, 0.8, 1.0, 1.0]`
   - Line tool preview: render dashed line from `line_start` to current mouse position
8. Implement `Canvas2D::new() -> Self` with defaults
9. Implement `Canvas2D::update(input: &InputState) -> ()` to process tool state
10. Wire into `AssetEditor`: when `stage == EditorStage::Draw2D`, delegate input/render to `Canvas2D`

## Code Patterns
Rendering outlines as thin quads using the existing `add_quad()` from `src/game/ui/text.rs`:
```rust
// For each segment in the outline
for i in 0..outline.points.len() - 1 {
    let a = outline.points[i];
    let b = outline.points[i + 1];
    let dir = normalize_2d(sub_2d(b, a));
    let perp = [-dir[1] * half_width, dir[0] * half_width];
    // add_quad with 4 corners: a±perp, b±perp
    add_quad(
        [a[0] - perp[0], a[1] - perp[1], 0.0],
        [a[0] + perp[0], a[1] + perp[1], 0.0],
        [b[0] + perp[0], b[1] + perp[1], 0.0],
        [b[0] - perp[0], b[1] - perp[1], 0.0],
        color,
        vertices, indices,
    );
}
```

RDP simplification pattern:
```rust
fn rdp_simplify(points: &[[f32; 2]], epsilon: f32) -> Vec<[f32; 2]> {
    if points.len() <= 2 { return points.to_vec(); }
    let (max_dist, max_idx) = points.iter().enumerate().skip(1).take(points.len() - 2)
        .map(|(i, p)| (perp_distance(*p, points[0], *points.last().unwrap()), i))
        .fold((0.0f32, 0), |(d, i), (nd, ni)| if nd > d { (nd, ni) } else { (d, i) });
    if max_dist > epsilon {
        let mut left = rdp_simplify(&points[..=max_idx], epsilon);
        let right = rdp_simplify(&points[max_idx..], epsilon);
        left.pop();
        left.extend(right);
        left
    } else {
        vec![points[0], *points.last().unwrap()]
    }
}
```

## Acceptance Criteria
- [ ] `canvas_2d.rs` exists with `Canvas2D`, `Outline2D`, and `DrawTool` types
- [ ] Freehand tool (D key): mouse-down starts drawing, mouse-move adds points, mouse-up finishes with RDP simplification
- [ ] Line tool (L key): click-click creates straight line segments
- [ ] RDP simplification runs on mouse-up with epsilon ~0.05, reducing point count significantly
- [ ] Orthographic canvas: origin at center, 1 unit = 1 grid cell, default 20x20 view
- [ ] Zoom via scroll wheel (0.1x to 10x range)
- [ ] Pan via middle-mouse drag
- [ ] Grid toggle via G key
- [ ] Outlines render as thin quads using `add_quad()`
- [ ] `cargo check --bin battle_editor` compiles with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor compiles with canvas_2d module`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles`
- `cmd`: `test -f src/game/asset_editor/canvas_2d.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `canvas_2d.rs file exists`
- `cmd`: `grep -c 'pub struct Canvas2D' src/game/asset_editor/canvas_2d.rs`
  `expect_gt`: 0
  `description`: `Canvas2D struct defined`
- `cmd`: `grep -c 'pub struct Outline2D' src/game/asset_editor/canvas_2d.rs`
  `expect_gt`: 0
  `description`: `Outline2D struct defined`
- `cmd`: `grep -c 'DrawTool' src/game/asset_editor/canvas_2d.rs`
  `expect_gt`: 0
  `description`: `DrawTool enum defined`
- `cmd`: `grep -c 'rdp_simplify\|ramer_douglas_peucker' src/game/asset_editor/canvas_2d.rs`
  `expect_gt`: 0
  `description`: `RDP simplification implemented`
- `cmd`: `grep -c 'pub mod canvas_2d' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `canvas_2d module registered in mod.rs`

## Success Looks Like
Running `cargo run --bin battle_editor` and entering Stage 1 (Draw2D, key 1) shows an orthographic canvas with a grid. Pressing D activates freehand: click-and-drag draws a smooth outline that snaps to a simplified version on release. Pressing L activates line: two clicks create a straight segment. Scroll zooms in/out, middle-mouse pans, G toggles the grid. Outlines appear as clean white lines on the dark canvas. Switching to another stage preserves the outlines.

## Dependencies
- Depends on: US-P4-001

## Complexity
- Complexity: normal
- Min iterations: 1
