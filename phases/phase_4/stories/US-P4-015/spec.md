# US-P4-015: Editor UI Panels (Tool Palette + Color Picker)

## Description
Create `src/game/asset_editor/ui_panels.rs` with three UI panel systems for the asset editor: a **tool palette** on the left side showing available tools for the current stage with keyboard shortcut labels, a **property panel** on the right side with stage-specific sliders for tuning parameters, and an **HSV color picker** for Stage 4 (Color) with a saturation/value box (128x128 pixels), vertical hue bar, opacity slider, primary/secondary color swatches, and 8 recent color swatches. All rendering uses existing `add_quad()` and `draw_text()` primitives from `src/game/ui/text.rs`. No external GUI library. The editor is a separate binary (`cargo run --bin battle_editor`). `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
A creative tool without visual UI panels forces users to memorize keyboard shortcuts and mentally track invisible parameters. The tool palette provides **discoverability** -- artists can see what tools are available at each stage and their keyboard shortcuts at a glance. The property panel provides **tunability** -- sliders for brush radius, extrusion thickness, sculpt strength give real-time visual feedback as values change. The HSV color picker is essential for Stage 4 painting because selecting colors from a preset palette alone is too limiting for artistic expression.

Together, these panels transform the editor from a keyboard-driven technical tool into a visual creative application. The key design decision is rendering everything with the game's own quad and text primitives (`add_quad()` + `draw_text()`), keeping the editor entirely self-contained without pulling in egui, imgui, or any external GUI crate. This means the UI is lightweight, has zero additional dependencies, and renders in the same visual style as the rest of the game.

The HSV color model is specifically chosen because it separates hue (what color), saturation (how vivid), and value (how bright) -- which maps directly to how artists think about color. The SV box lets them pick shade and intensity in one 2D gesture, while the hue bar selects the base color.

## Goal
Create `src/game/asset_editor/ui_panels.rs` with `EditorUI` struct containing a tool palette, property panel with stage-specific sliders, and a full HSV color picker, all rendered using `add_quad()` and `draw_text()`.

## Files to Create/Modify
- **Create** `src/game/asset_editor/ui_panels.rs` -- `EditorUI`, `ToolDef`, `PropertyParams`, `ColorPickerState`, `SliderID`, `UIAction`, tool palette rendering, property panel rendering, HSV color picker rendering, click handling for all UI elements
- **Modify** `src/game/asset_editor/mod.rs` -- Add `pub mod ui_panels;`, add `ui: EditorUI` field to `AssetEditor`, render UI panels in each stage, route mouse events to UI before canvas/3D viewport

## Implementation Steps

1. Define the `EditorUI` struct that owns all panel state:
   ```rust
   pub struct EditorUI {
       pub tool_palette_visible: bool,     // Default true
       pub property_panel_visible: bool,   // Default true
       pub color_picker_visible: bool,     // Auto-enabled in Stage 4 (Color)
       pub hovered_tool: Option<usize>,    // For hover highlight effect
       pub active_slider: Option<SliderID>, // Currently being dragged
   }
   ```

2. Define tool definitions per stage with names, shortcuts, and icon characters:
   ```rust
   pub struct ToolDef {
       pub name: &'static str,
       pub shortcut: char,
       pub icon_char: char,   // Simple text-based icon
   }

   pub fn tools_for_stage(stage: EditorStage) -> Vec<ToolDef> {
       match stage {
           EditorStage::Draw2D => vec![
               ToolDef { name: "Freehand", shortcut: 'D', icon_char: '~' },
               ToolDef { name: "Line",     shortcut: 'L', icon_char: '/' },
               ToolDef { name: "Arc",      shortcut: 'A', icon_char: ')' },
               ToolDef { name: "Eraser",   shortcut: 'E', icon_char: 'x' },
               ToolDef { name: "Mirror",   shortcut: 'M', icon_char: '|' },
           ],
           EditorStage::Extrude => vec![
               ToolDef { name: "Pump",     shortcut: 'P', icon_char: 'O' },
               ToolDef { name: "Linear",   shortcut: 'L', icon_char: '|' },
               ToolDef { name: "Lathe",    shortcut: 'T', icon_char: '@' },
           ],
           EditorStage::Sculpt => vec![
               ToolDef { name: "Smooth",   shortcut: 'S', icon_char: '~' },
               ToolDef { name: "Add",      shortcut: 'A', icon_char: '+' },
               ToolDef { name: "Subtract", shortcut: 'D', icon_char: '-' },
               ToolDef { name: "Face Sel", shortcut: 'F', icon_char: '#' },
               ToolDef { name: "Vertex",   shortcut: 'V', icon_char: '.' },
               ToolDef { name: "Edge",     shortcut: 'G', icon_char: '_' },
           ],
           EditorStage::Color => vec![
               ToolDef { name: "Brush",    shortcut: 'B', icon_char: 'o' },
               ToolDef { name: "Fill",     shortcut: 'F', icon_char: '#' },
               ToolDef { name: "Gradient", shortcut: 'G', icon_char: '>' },
               ToolDef { name: "Eyedrop",  shortcut: 'I', icon_char: '?' },
           ],
           EditorStage::Save => vec![
               ToolDef { name: "Save",    shortcut: 'S', icon_char: 'W' },
               ToolDef { name: "Load",    shortcut: 'L', icon_char: 'R' },
               ToolDef { name: "Library", shortcut: 'B', icon_char: '#' },
           ],
       }
   }
   ```

3. Implement tool palette rendering (left side, vertical strip):
   ```rust
   impl EditorUI {
       pub fn generate_tool_palette(
           &self,
           stage: EditorStage,
           active_tool: usize,
           screen_h: f32,
       ) -> (Vec<Vertex>, Vec<u32>) {
           let mut verts = Vec::new();
           let mut idxs = Vec::new();
           if !self.tool_palette_visible { return (verts, idxs); }

           let panel_w = 48.0;
           add_quad(&mut verts, &mut idxs,
               [0.0, 0.0, 0.0], [panel_w, 0.0, 0.0],
               [panel_w, screen_h, 0.0], [0.0, screen_h, 0.0],
               [0.1, 0.1, 0.15, 0.85]);

           let tools = tools_for_stage(stage);
           for (i, tool) in tools.iter().enumerate() {
               let y_offset = 10.0 + i as f32 * 40.0;
               let bg = if i == active_tool {
                   [0.3, 0.4, 0.6, 0.9]
               } else if self.hovered_tool == Some(i) {
                   [0.2, 0.25, 0.35, 0.8]
               } else {
                   [0.0, 0.0, 0.0, 0.0]
               };
               if bg[3] > 0.0 {
                   add_quad(&mut verts, &mut idxs,
                       [0.0, y_offset, 0.0], [panel_w, y_offset, 0.0],
                       [panel_w, y_offset + 36.0, 0.0], [0.0, y_offset + 36.0, 0.0],
                       bg);
               }
               let label = format!("{} {}", tool.shortcut, tool.name);
               draw_text(&mut verts, &mut idxs, &label,
                   4.0, y_offset + 8.0, 0.35, [0.9, 0.9, 0.9, 1.0]);
           }
           (verts, idxs)
       }
   }
   ```

4. Define property parameters and slider identifiers:
   ```rust
   pub struct PropertyParams {
       pub grid_size: f32,        // Stage 1
       pub snap_enabled: bool,
       pub inflation: f32,        // Stage 2
       pub thickness: f32,
       pub profile_index: usize,
       pub resolution: u32,
       pub sculpt_radius: f32,    // Stage 3
       pub sculpt_strength: f32,
       pub smooth_k: f32,
       pub paint_radius: f32,     // Stage 4
       pub paint_opacity: f32,
       pub paint_hardness: f32,
   }

   #[derive(Clone, Copy, Debug, PartialEq)]
   pub enum SliderID {
       GridSize,
       Inflation, Thickness, Resolution,
       SculptRadius, SculptStrength, SmoothK,
       PaintRadius, PaintOpacity, PaintHardness,
       HueBar, SVBox, OpacitySlider,
   }
   ```

5. Implement property panel rendering (right side, 200px wide, stage-specific sliders):
   ```rust
   impl EditorUI {
       pub fn generate_property_panel(
           &self, stage: EditorStage, params: &PropertyParams,
           screen_w: f32, screen_h: f32,
       ) -> (Vec<Vertex>, Vec<u32>) {
           // Panel background on right, title "Properties",
           // stage-specific sliders using render_slider() helper
           // Stage 1: GridSize
           // Stage 2: Inflation, Thickness, Resolution
           // Stage 3: SculptRadius, SculptStrength, SmoothK
           // Stage 4: PaintRadius, PaintOpacity, PaintHardness
           // Stage 5: (no sliders)
       }
   }

   fn render_slider(verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>, slider: &UISliderDef) {
       // Label text
       draw_text(verts, idxs, slider.label, slider.x, slider.y, 0.3, [0.8; 4]);
       // Track: dark bar
       let track_y = slider.y + 14.0;
       add_quad(verts, idxs, /* track quad [0.2, 0.2, 0.25, 1.0] */);
       // Fill: blue bar proportional to value
       let t = (slider.value - slider.min) / (slider.max - slider.min);
       add_quad(verts, idxs, /* fill quad [0.4, 0.6, 0.9, 1.0] */);
       // Value text right-aligned
       draw_text(verts, idxs, &format!("{:.2}", slider.value), /* ... */);
   }
   ```

6. Define HSV color picker state:
   ```rust
   pub struct ColorPickerState {
       pub hue: f32,             // 0-360 degrees
       pub saturation: f32,      // 0-1
       pub value: f32,           // 0-1
       pub opacity: f32,         // 0-1
       pub primary: [f32; 4],    // Selected primary color (RGBA)
       pub secondary: [f32; 4],  // Alt-click secondary color (RGBA)
       pub recent_colors: Vec<[f32; 4]>,  // Last 8 used colors
   }
   ```

7. Implement HSV color picker rendering:
   ```rust
   impl EditorUI {
       pub fn generate_color_picker(
           &self, picker: &ColorPickerState, x: f32, y: f32,
       ) -> (Vec<Vertex>, Vec<u32>) {
           if !self.color_picker_visible { return (Vec::new(), Vec::new()); }

           // SV box: 128x128, rendered as 16x16 grid of colored quads
           for row in 0..16 {
               for col in 0..16 {
                   let s = col as f32 / 16.0;
                   let v = 1.0 - row as f32 / 16.0;
                   let (r, g, b) = hsv_to_rgb(picker.hue, s, v);
                   add_quad(/* cell quad with [r, g, b, 1.0] */);
               }
           }
           // SV crosshair at current position

           // Hue bar: 20x128 vertical, 32 color steps
           for i in 0..32 {
               let h = i as f32 / 32.0 * 360.0;
               let (r, g, b) = hsv_to_rgb(h, 1.0, 1.0);
               add_quad(/* step quad with [r, g, b, 1.0] */);
           }
           // Hue indicator line

           // Opacity slider below SV box
           // Primary/secondary swatches (24x24 each)
           // Recent colors: 8 swatches (16x16 each)
       }
   }
   ```

8. Implement click handling for all UI elements:
   ```rust
   pub enum UIAction {
       SelectTool(usize),
       AdjustSlider(SliderID, f32),
       SetHue(f32),
       SetSaturationValue(f32, f32),
       SetOpacity(f32),
       SelectRecentColor(usize),
       SwapPrimarySecondary,
   }

   impl EditorUI {
       pub fn handle_click(
           &mut self, x: f32, y: f32, stage: EditorStage,
           screen_w: f32, _screen_h: f32,
       ) -> Option<UIAction> {
           // Check tool palette (left, 48px)
           // Check property panel (right, 200px)
           // Check color picker (SV box, hue bar, opacity, recent)
           None // if no UI hit, pass to canvas/viewport
       }

       pub fn handle_hover(&mut self, x: f32, y: f32, stage: EditorStage) {
           // Update hovered_tool for highlight effect
       }
   }
   ```

9. Implement HSV-to-RGB helper:
   ```rust
   pub fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
       let c = v * s;
       let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
       let m = v - c;
       let (r1, g1, b1) = match (h / 60.0) as u32 {
           0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x),
           3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x),
       };
       (r1 + m, g1 + m, b1 + m)
   }
   ```

10. Wire into `mod.rs`: add `pub mod ui_panels;`, add `ui: EditorUI` field, render panels each frame as overlay, route mouse to UI first (if UI consumes click, do not pass to canvas/viewport).

## Code Patterns
UI rendering follows the existing pattern from `terrain_editor.rs` -- generate vertex/index data, then render in the UI pass:

```rust
// Each frame, collect all UI geometry
let (palette_v, palette_i) = editor.ui.generate_tool_palette(stage, active_tool, screen_h);
let (props_v, props_i) = editor.ui.generate_property_panel(stage, &params, screen_w, screen_h);
let (picker_v, picker_i) = editor.ui.generate_color_picker(&color_state, picker_x, picker_y);
// Merge and upload to GPU, render with no depth test
```

The HSV color picker SV box uses the quads-as-pixels approach:
```rust
for row in 0..16 {
    for col in 0..16 {
        let s = col as f32 / 16.0;
        let v = 1.0 - row as f32 / 16.0;
        let (r, g, b) = hsv_to_rgb(current_hue, s, v);
        add_quad(/* colored quad */);
    }
}
```

## Acceptance Criteria
- [ ] `ui_panels.rs` exists with `EditorUI`, `ToolDef`, `PropertyParams`, `ColorPickerState`, `SliderID`, `UIAction` types
- [ ] Tool palette renders on left side with correct tools per stage (Draw2D: 5 tools, Extrude: 3, Sculpt: 6, Color: 4, Save: 3)
- [ ] Active tool is visually highlighted with a different background color
- [ ] Hovered tool shows a subtle highlight (different from active)
- [ ] Property panel shows context-sensitive sliders appropriate to each stage
- [ ] Sliders render with label, track, fill bar, and value text
- [ ] HSV color picker renders SV box (128x128 pixels as 16x16 colored quads)
- [ ] Hue bar renders as a 20x128 vertical strip cycling through all hues
- [ ] Opacity slider renders below SV box
- [ ] Primary and secondary color swatches are displayed
- [ ] Recent colors track the last 8 used colors as 16x16 swatches
- [ ] Click handling correctly identifies tool selection, slider interaction, SV box click, hue bar click, and recent color selection
- [ ] `UIAction` enum provides structured results from all click interactions
- [ ] All rendering uses `add_quad()` and `draw_text()` primitives (no external GUI library)
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check` passes with 0 errors

## Verification Commands
- `cmd`: `test -f /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `ui_panels.rs module exists`
- `cmd`: `grep -c 'EditorUI\|generate_tool_palette\|generate_property_panel\|generate_color_picker' /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs`
  `expect_gt`: 0
  `description`: `UI rendering functions defined`
- `cmd`: `grep -c 'ColorPickerState\|UIAction\|PropertyParams\|SliderID' /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs`
  `expect_gt`: 0
  `description`: `UI state types defined`
- `cmd`: `grep -c 'tools_for_stage\|hsv_to_rgb\|render_slider' /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs`
  `expect_gt`: 0
  `description`: `Helper functions implemented`
- `cmd`: `grep -c 'pub mod ui_panels' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `ui_panels module registered in mod.rs`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `Project compiles`

## Success Looks Like
When the editor opens, a tool palette appears on the left showing available tools with keyboard shortcuts (e.g., "D Freehand", "L Line"). The active tool has a bright blue background. Hovering over other tools shows a subtle highlight. Switching stages (keys 1-5) updates the palette to show the correct tools for that stage.

A property panel on the right shows sliders for the current stage -- "Grid Size" in Draw2D mode, "Inflation" and "Thickness" in Extrude mode, "Radius" and "Strength" in Sculpt mode. Clicking and dragging a slider adjusts the value smoothly, with the fill bar and value text updating in real time.

In Stage 4 (Color), the HSV color picker appears with a saturation/value gradient box, a vertical rainbow hue bar, and an opacity slider. Clicking in the SV box picks a shade, clicking the hue bar changes the base color. The currently selected color shows in a primary swatch. The last 8 used colors appear as small swatches for quick re-selection. The UI feels integrated and responsive, rendered with the same visual style as the rest of the game.

## Dependencies
- Depends on: US-P4-001 (needs editor skeleton with stage system and EditorStage enum)

## Complexity
- Complexity: complex
- Min iterations: 2
