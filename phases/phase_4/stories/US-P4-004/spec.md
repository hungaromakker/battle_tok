# US-P4-004: 2D Canvas — Arc Tool, Eraser, Mirror Symmetry

## Description
Extend the 2D canvas (from US-P4-003) with three additional tools: an arc tool for curved segments, an eraser for removing parts of outlines, and a mirror symmetry toggle that duplicates drawing across the Y axis. These tools round out the 2D drawing toolkit so artists can create smooth, symmetrical outlines efficiently.

## The Core Concept / Why This Matters
Freehand and line tools alone produce angular, rough outlines. The arc tool lets the artist draw smooth curves by specifying 3 points — the system computes the circumscribed circle and traces the arc between them. The eraser is essential for correcting mistakes without starting over: it removes points within a radius and splits outlines at the gap. Mirror symmetry is critical for natural assets (trees, characters, structures) where left/right balance is expected — the artist draws one side and the mirror produces the other, ensuring perfect symmetry.

## Goal
Add arc tool (A key), eraser tool (E key), and mirror symmetry toggle (M key) to the existing `Canvas2D` in `src/game/asset_editor/canvas_2d.rs`.

## Files to Create/Modify
- **Modify** `src/game/asset_editor/canvas_2d.rs` — Add `Arc` and `Eraser` to `DrawTool` enum, implement arc creation (3-click), eraser logic (circle cursor + point removal + outline splitting), mirror symmetry toggle with rendering
- **Modify** `src/bin/battle_editor.rs` — Forward A, E, M key events to canvas tool switching / mirror toggle

## Implementation Steps
1. Extend `DrawTool` enum:
   ```rust
   pub enum DrawTool { Freehand, Line, Arc, Eraser }
   ```
2. Add mirror state to `Canvas2D`:
   ```rust
   pub mirror_x: bool,      // default false
   arc_points: Vec<[f32; 2]>, // accumulates 3 clicks for arc
   eraser_radius: f32,      // default 0.5 canvas units
   ```
3. Implement arc tool (A key):
   - Click 1: record first point (arc start)
   - Click 2: record second point (arc passes through)
   - Click 3: record third point (arc end) → compute circumscribed circle → generate arc points
   - Circumscribed circle from 3 points:
     - Compute perpendicular bisectors of segments P1-P2 and P2-P3
     - Find intersection → circle center
     - Radius = distance from center to any point
   - Generate N intermediate points along the arc (N proportional to arc angle, ~1 point per 5 degrees)
   - Create `Outline2D` from the generated arc points
   - If 3 points are collinear, fall back to a straight line between P1 and P3
4. Implement eraser tool (E key):
   - Render circle cursor at mouse position with `eraser_radius`
   - On mouse-down + drag: for each outline, remove points within `eraser_radius` of cursor
   - When points are removed from the middle of an outline, split it into two separate outlines (before-gap and after-gap)
   - When all points of an outline are erased, remove the entire outline
   - Eraser radius adjustable with `[` and `]` keys (step ±0.1, clamp 0.1 to 3.0)
5. Implement mirror symmetry (M key toggles):
   - When `mirror_x` is true, render a dashed vertical line at x=0 (the mirror axis)
   - During rendering: for each outline, also render its mirror (negate all x coordinates)
   - During finalization (when leaving Draw2D stage): actually duplicate the mirrored points into the outline data
   - Mirror affects freehand, line, and arc tools equally
6. Render the dashed mirror line:
   - Vertical line at x=0, extending full canvas height
   - Rendered as short quads with gaps (dash pattern: 0.3 on, 0.15 off)
   - Color: `[0.8, 0.4, 1.0, 0.6]` (purple, semi-transparent)
7. Render the eraser cursor:
   - Circle approximated as 24-segment polygon of thin quads
   - Color: `[1.0, 0.3, 0.3, 0.7]` (red, semi-transparent)
8. Key bindings:
   - A → switch to Arc tool
   - E → switch to Eraser tool
   - M → toggle `mirror_x`
   - `[` → decrease eraser radius
   - `]` → increase eraser radius

## Code Patterns
Circumscribed circle from 3 points:
```rust
fn circumscribed_circle(p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]) -> Option<([f32; 2], f32)> {
    let ax = p1[0]; let ay = p1[1];
    let bx = p2[0]; let by = p2[1];
    let cx = p3[0]; let cy = p3[1];
    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-10 { return None; } // collinear
    let ux = ((ax * ax + ay * ay) * (by - cy) + (bx * bx + by * by) * (cy - ay) + (cx * cx + cy * cy) * (ay - by)) / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx) + (bx * bx + by * by) * (ax - cx) + (cx * cx + cy * cy) * (bx - ax)) / d;
    let r = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();
    Some(([ux, uy], r))
}
```

Outline splitting on erase:
```rust
fn erase_near(outline: &Outline2D, cursor: [f32; 2], radius: f32) -> Vec<Outline2D> {
    let mut segments: Vec<Vec<[f32; 2]>> = vec![vec![]];
    for &pt in &outline.points {
        let dist = distance_2d(pt, cursor);
        if dist > radius {
            segments.last_mut().unwrap().push(pt);
        } else if !segments.last().unwrap().is_empty() {
            segments.push(vec![]);
        }
    }
    segments.into_iter()
        .filter(|s| s.len() >= 2)
        .map(|points| Outline2D { points, closed: false })
        .collect()
}
```

## Acceptance Criteria
- [ ] `DrawTool` enum includes `Arc` and `Eraser` variants
- [ ] Arc tool (A key): 3 clicks define a circumscribed circle arc, intermediate points generated along the arc
- [ ] Collinear arc points fall back to a straight line (no crash)
- [ ] Eraser tool (E key): circle cursor removes points within radius, outlines split at gaps
- [ ] Eraser radius adjustable with `[` and `]` keys (0.1 to 3.0 range)
- [ ] Mirror symmetry (M key): toggles X-axis mirror, dashed vertical line at x=0
- [ ] Mirror rendering: outlines reflected across x=0 when mirror is active
- [ ] Mirror finalization: mirrored points materialized into outline data on stage exit
- [ ] All existing tools (Freehand, Line) still work correctly
- [ ] `cargo check --bin battle_editor` compiles with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor compiles with arc, eraser, and mirror additions`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles`
- `cmd`: `grep -c 'Arc\|Eraser' src/game/asset_editor/canvas_2d.rs`
  `expect_gt`: 1
  `description`: `Arc and Eraser tool variants exist`
- `cmd`: `grep -c 'mirror_x\|mirror' src/game/asset_editor/canvas_2d.rs`
  `expect_gt`: 1
  `description`: `Mirror symmetry field and logic exist`
- `cmd`: `grep -c 'circumscribed_circle\|circ_center' src/game/asset_editor/canvas_2d.rs`
  `expect_gt`: 0
  `description`: `Circumscribed circle computation implemented`
- `cmd`: `grep -c 'eraser_radius' src/game/asset_editor/canvas_2d.rs`
  `expect_gt`: 0
  `description`: `Eraser radius field exists`

## Success Looks Like
In Stage 1 (Draw2D), pressing A and clicking 3 points draws a smooth arc. Pressing E shows a red circle cursor that removes parts of outlines on click/drag, splitting them at the erasure. Pressing M shows a dashed purple vertical line at x=0, and all outlines are mirrored in real-time. Drawing on the right side automatically appears on the left. The `[` and `]` keys resize the eraser. All previous tools (D for freehand, L for line) continue to work.

## Dependencies
- Depends on: US-P4-003

## Complexity
- Complexity: normal
- Min iterations: 1
