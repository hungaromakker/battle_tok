# US-P4-009: Vertex Color Painting

## Description
Create `src/game/asset_editor/paint.rs` with vertex color painting tools for Stage 4 (Color) of the asset editor pipeline. This module provides four tools — Brush, Fill, Gradient, and Eyedropper — that write directly to the `Vertex.color` field of the 3D mesh. A `ColorPalette` with HSV representation, presets for common asset categories, and a recent colors list accelerates the painting workflow. The asset editor is a **separate binary** (`battle_editor`); `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
Low-poly stylized games rely on vertex coloring rather than UV-mapped textures. Vertex colors are cheap to render (no texture sampling), scale infinitely, and produce a distinctive hand-painted look. Without a painting stage, artists would need to export meshes to external tools, destroying the rapid iteration loop the editor provides. The four tools cover every common painting workflow: precision brush work with falloff, quick region fills, smooth gradients for organic transitions, and eyedropper for color matching.

## Goal
Create `src/game/asset_editor/paint.rs` with a `PaintSystem` struct providing brush, fill, gradient, and eyedropper tools that modify `Vertex.color` directly on the mesh. Integrate into the `battle_editor` binary as Stage 4.

## Files to Create/Modify
- **Create** `src/game/asset_editor/paint.rs` — `PaintSystem`, `PaintTool`, `BrushParams`, `ColorPalette`, painting functions
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod paint;`, add `paint: PaintSystem` field to `AssetEditor`, route Stage 4 (Color) input/render to `PaintSystem`

## Implementation Steps
1. Define the `PaintTool` enum with four variants: `Brush`, `Fill`, `Gradient`, `Eyedropper`.
2. Create `BrushParams` struct with `radius: f32` (default 0.5, world-space), `opacity: f32` (0.0–1.0, default 1.0), `hardness: f32` (0.0 soft – 1.0 hard, default 0.8).
3. Create `ColorPalette` struct with:
   - `primary: [f32; 4]` — current painting color (RGBA)
   - `secondary: [f32; 4]` — alt color (X key to swap)
   - `recent: Vec<[f32; 4]>` — last 8 used colors
   - `presets: Vec<PalettePreset>` — named preset groups (Trees, Rock, Wood)
   - HSV representation fields (`hue: f32`, `saturation: f32`, `value: f32`) synced with primary
4. Implement `hsv_to_rgba(h, s, v) -> [f32; 4]` and `rgba_to_hsv(color) -> (f32, f32, f32)` conversion functions.
5. Implement `paint_brush(mesh, hit_point, brush, color)`:
   - For each vertex, compute distance to `hit_point`
   - If distance < `brush.radius`, compute falloff: `1.0 - (dist / radius).powf(2.0 / hardness)`
   - Lerp vertex color toward paint color by `falloff * opacity`
6. Implement `flood_fill(mesh, start_triangle_idx, target_color, tolerance)`:
   - Build triangle adjacency from index buffer (shared edges)
   - BFS from start triangle, spreading to neighbors whose average vertex color is within `tolerance` of original color
   - Set all visited triangle vertices to `target_color`
7. Implement gradient painting:
   - First click sets gradient start point + primary color
   - Drag/second click sets gradient end point + secondary color
   - For each vertex, project onto the start→end line, compute `t` (0.0–1.0), lerp between primary and secondary
8. Implement eyedropper: on click, find nearest vertex to hit point, copy its `color` into `palette.primary`, update HSV fields.
9. Wire into `mod.rs`: when `stage == EditorStage::Color`, delegate input to `PaintSystem`. Mesh vertices reflect painted colors in real-time during render.

## Code Patterns
```rust
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PaintTool {
    Brush,
    Fill,
    Gradient,
    Eyedropper,
}

#[derive(Clone, Debug)]
pub struct BrushParams {
    pub radius: f32,
    pub opacity: f32,
    pub hardness: f32,
}

pub struct ColorPalette {
    pub primary: [f32; 4],
    pub secondary: [f32; 4],
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    pub recent: Vec<[f32; 4]>,
    pub presets: Vec<PalettePreset>,
}

pub fn paint_brush(vertices: &mut [Vertex], hit_point: [f32; 3], brush: &BrushParams, color: [f32; 4]) {
    for v in vertices.iter_mut() {
        let dist = distance(v.position, hit_point);
        if dist < brush.radius {
            let falloff = (1.0 - (dist / brush.radius)).powf(2.0 / brush.hardness);
            let alpha = falloff * brush.opacity;
            for i in 0..4 {
                v.color[i] = v.color[i] * (1.0 - alpha) + color[i] * alpha;
            }
        }
    }
}
```

## Acceptance Criteria
- [ ] `paint.rs` exists with `PaintSystem`, `BrushParams`, `ColorPalette`, `PaintTool` types
- [ ] Brush tool paints vertex colors with radius, opacity, and hardness falloff
- [ ] Flood fill uses BFS on triangle adjacency to fill connected same-color regions
- [ ] Gradient tool applies linear color blend between two click points
- [ ] Eyedropper samples vertex color and sets it as primary
- [ ] HSV-to-RGB and RGB-to-HSV conversion functions are implemented
- [ ] Color palette includes presets for Trees, Rock, and Wood
- [ ] Recent colors list tracks last 8 used colors
- [ ] All painting writes to `Vertex.color` field (not a separate texture)
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo check --bin battle_arena` passes with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with paint module`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles unchanged`
- `cmd`: `test -f src/game/asset_editor/paint.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `paint.rs file exists`
- `cmd`: `grep -c 'PaintSystem\|BrushParams\|ColorPalette\|PaintTool' src/game/asset_editor/paint.rs`
  `expect_gt`: 0
  `description`: `Core paint types are defined`
- `cmd`: `grep -c 'flood_fill\|paint_brush\|eyedropper\|hsv_to_rgba' src/game/asset_editor/paint.rs`
  `expect_gt`: 0
  `description`: `Paint tool functions are implemented`
- `cmd`: `grep -c 'pub mod paint' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `paint module registered in mod.rs`

## Success Looks Like
The artist enters Stage 4 (Color) and sees their 3D mesh ready for painting. They select the brush tool, pick a dark green from the Trees preset, and paint the top of a tree mesh. They lower opacity and add lighter green highlights. They use flood fill to quickly color the trunk brown. They apply a subtle gradient from dark to light green across the foliage. They eyedrop a color from one area and reuse it elsewhere. Vertex colors update in real-time on the mesh preview. The painting feels responsive and intuitive.

## Dependencies
- Depends on: US-P4-006 (needs 3D mesh with vertices to paint on)

## Complexity
- Complexity: normal
- Min iterations: 1
