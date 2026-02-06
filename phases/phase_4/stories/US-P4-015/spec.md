# US-P4-015: Editor UI Panels (Tool Palette + Color Picker)

## Description
Create `src/game/asset_editor/ui_panels.rs` with the visual interface panels for the asset editor: a tool palette on the left side showing available tools per stage with keyboard shortcuts, a property panel on the right side with stage-appropriate sliders, and an HSV color picker (SV box, hue bar, opacity slider, recent color swatches) for Stage 4. All rendering uses `add_quad()` and `draw_text()` from the existing UI system. The editor is a separate binary (`cargo run --bin battle_editor`) with its own winit window and event loop. `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
A creative tool without visual UI panels forces users to memorize keyboard shortcuts and mentally track invisible parameters. The tool palette provides discoverability -- artists can see what tools are available at each stage and their shortcuts. The property panel provides tunability -- sliders for brush radius, extrusion thickness, sculpt strength give immediate feedback. The HSV color picker is essential for Stage 4 painting because selecting colors from a palette of presets alone is too limiting. Together, these panels transform the editor from a keyboard-driven technical tool into a visual creative application. The panels are rendered using the game's own quad and text primitives, keeping the editor self-contained without external GUI libraries.

## Goal
Create `src/game/asset_editor/ui_panels.rs` with `EditorUI` struct containing a tool palette, property panel, and HSV color picker, all rendered using existing `add_quad()` and `draw_text()` UI primitives.

## Files to Create/Modify
- Create `src/game/asset_editor/ui_panels.rs` -- EditorUI, tool palette, property panel, color picker
- Modify `src/game/asset_editor/mod.rs` -- Add `pub mod ui_panels;`, add `ui: EditorUI` field, render UI when editor active

## Implementation Steps
1. Define `EditorUI` struct:
   ```rust
   pub struct EditorUI {
       pub tool_palette_visible: bool,    // default true
       pub property_panel_visible: bool,  // default true
       pub color_picker_visible: bool,    // auto-enabled in Stage 4
       pub hovered_tool: Option<usize>,   // for hover highlight
       pub active_slider: Option<SliderID>, // currently dragged slider
   }
   ```

2. Define tool definitions per stage:
   ```rust
   pub struct ToolDef {
       pub name: &'static str,
       pub shortcut: char,
       pub icon_char: char,   // for simple text icon
   }

   pub fn tools_for_stage(stage: EditorStage) -> Vec<ToolDef> {
       match stage {
           EditorStage::Draw2D => vec![
               ToolDef { name: "Freehand", shortcut: 'D', icon_char: '~' },
               ToolDef { name: "Line", shortcut: 'L', icon_char: '/' },
               ToolDef { name: "Arc", shortcut: 'A', icon_char: ')' },
               ToolDef { name: "Eraser", shortcut: 'E', icon_char: 'x' },
               ToolDef { name: "Mirror", shortcut: 'M', icon_char: '|' },
           ],
           EditorStage::Extrude => vec![
               ToolDef { name: "Pump", shortcut: 'P', icon_char: 'O' },
               ToolDef { name: "Linear", shortcut: 'L', icon_char: '|' },
               ToolDef { name: "Lathe", shortcut: 'T', icon_char: '@' },
           ],
           EditorStage::Sculpt => vec![
               ToolDef { name: "Smooth", shortcut: 'S', icon_char: '~' },
               ToolDef { name: "Add", shortcut: 'A', icon_char: '+' },
               ToolDef { name: "Subtract", shortcut: 'D', icon_char: '-' },
               ToolDef { name: "Face Sel", shortcut: 'F', icon_char: '#' },
               ToolDef { name: "Vertex", shortcut: 'V', icon_char: '.' },
               ToolDef { name: "Edge", shortcut: 'G', icon_char: '_' },
           ],
           EditorStage::Color => vec![
               ToolDef { name: "Brush", shortcut: 'B', icon_char: 'o' },
               ToolDef { name: "Fill", shortcut: 'F', icon_char: '#' },
               ToolDef { name: "Gradient", shortcut: 'G', icon_char: '>' },
               ToolDef { name: "Eyedrop", shortcut: 'I', icon_char: '?' },
           ],
           EditorStage::Save => vec![
               ToolDef { name: "Save", shortcut: 'S', icon_char: 'W' },
               ToolDef { name: "Load", shortcut: 'L', icon_char: 'R' },
               ToolDef { name: "Library", shortcut: 'B', icon_char: '#' },
           ],
       }
   }
   ```

3. Implement tool palette rendering:
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

           // Left panel background, 48px wide
           let panel_w = 48.0;
           add_quad(&mut verts, &mut idxs,
               [-1.0, 1.0, 0.0], [ndc_x(panel_w, screen_h), 1.0, 0.0],
               [ndc_x(panel_w, screen_h), -1.0, 0.0], [-1.0, -1.0, 0.0],
               [0.1, 0.1, 0.15, 0.85]);

           let tools = tools_for_stage(stage);
           for (i, tool) in tools.iter().enumerate() {
               let y_offset = 10.0 + i as f32 * 40.0;

               // Highlight active tool
               let bg = if i == active_tool {
                   [0.3, 0.4, 0.6, 0.9]
               } else if self.hovered_tool == Some(i) {
                   [0.2, 0.25, 0.35, 0.8]
               } else {
                   [0.0, 0.0, 0.0, 0.0] // transparent
               };

               if bg[3] > 0.0 {
                   add_quad(&mut verts, &mut idxs, /* tool row highlight quad */);
               }

               // Shortcut key + tool name
               let label = format!("{} {}", tool.shortcut, tool.name);
               draw_text(&mut verts, &mut idxs, &label,
                   4.0, y_offset, 0.35, [0.9, 0.9, 0.9, 1.0]);
           }

           (verts, idxs)
       }
   }
   ```

4. Implement property panel rendering:
   ```rust
   pub struct PropertyParams {
       // Stage 1
       pub grid_size: f32,
       pub snap_enabled: bool,
       // Stage 2
       pub inflation: f32,
       pub thickness: f32,
       pub profile_index: usize,
       pub resolution: u32,
       // Stage 3
       pub sculpt_radius: f32,
       pub sculpt_strength: f32,
       pub smooth_k: f32,
       // Stage 4
       pub paint_radius: f32,
       pub paint_opacity: f32,
       pub paint_hardness: f32,
   }

   #[derive(Clone, Copy, Debug, PartialEq)]
   pub enum SliderID {
       GridSize, Inflation, Thickness, Resolution,
       SculptRadius, SculptStrength, SmoothK,
       PaintRadius, PaintOpacity, PaintHardness,
       HueBar, SVBox, OpacitySlider,
   }

   pub struct UISlider {
       pub id: SliderID,
       pub label: &'static str,
       pub min: f32,
       pub max: f32,
       pub value: f32,
       pub x: f32,
       pub y: f32,
       pub width: f32,
   }

   impl EditorUI {
       pub fn generate_property_panel(
           &self,
           stage: EditorStage,
           params: &PropertyParams,
           screen_w: f32,
           screen_h: f32,
       ) -> (Vec<Vertex>, Vec<u32>) {
           let mut verts = Vec::new();
           let mut idxs = Vec::new();

           if !self.property_panel_visible { return (verts, idxs); }

           // Right panel background, 200px wide
           let panel_left = screen_w - 200.0;
           add_quad(&mut verts, &mut idxs, /* panel background quad */);

           // Title
           draw_text(&mut verts, &mut idxs, "Properties",
               panel_left + 10.0, 10.0, 0.4, [1.0; 4]);

           // Stage-specific sliders
           let sliders = match stage {
               EditorStage::Draw2D => vec![
                   UISlider { id: SliderID::GridSize, label: "Grid Size", min: 0.1, max: 2.0, value: params.grid_size, x: panel_left + 10.0, y: 40.0, width: 170.0 },
               ],
               EditorStage::Extrude => vec![
                   UISlider { id: SliderID::Inflation, label: "Inflation", min: 0.0, max: 1.0, value: params.inflation, x: panel_left + 10.0, y: 40.0, width: 170.0 },
                   UISlider { id: SliderID::Thickness, label: "Thickness", min: 0.1, max: 5.0, value: params.thickness, x: panel_left + 10.0, y: 80.0, width: 170.0 },
                   UISlider { id: SliderID::Resolution, label: "Resolution", min: 4.0, max: 64.0, value: params.resolution as f32, x: panel_left + 10.0, y: 120.0, width: 170.0 },
               ],
               EditorStage::Sculpt => vec![
                   UISlider { id: SliderID::SculptRadius, label: "Radius", min: 0.1, max: 5.0, value: params.sculpt_radius, x: panel_left + 10.0, y: 40.0, width: 170.0 },
                   UISlider { id: SliderID::SculptStrength, label: "Strength", min: 0.01, max: 1.0, value: params.sculpt_strength, x: panel_left + 10.0, y: 80.0, width: 170.0 },
                   UISlider { id: SliderID::SmoothK, label: "Smooth K", min: 0.1, max: 1.0, value: params.smooth_k, x: panel_left + 10.0, y: 120.0, width: 170.0 },
               ],
               EditorStage::Color => vec![
                   UISlider { id: SliderID::PaintRadius, label: "Radius", min: 0.05, max: 3.0, value: params.paint_radius, x: panel_left + 10.0, y: 40.0, width: 170.0 },
                   UISlider { id: SliderID::PaintOpacity, label: "Opacity", min: 0.0, max: 1.0, value: params.paint_opacity, x: panel_left + 10.0, y: 80.0, width: 170.0 },
                   UISlider { id: SliderID::PaintHardness, label: "Hardness", min: 0.0, max: 1.0, value: params.paint_hardness, x: panel_left + 10.0, y: 120.0, width: 170.0 },
               ],
               EditorStage::Save => vec![],
           };

           for slider in &sliders {
               render_slider(&mut verts, &mut idxs, slider);
           }

           (verts, idxs)
       }
   }
   ```

5. Implement HSV color picker:
   ```rust
   pub struct ColorPickerState {
       pub hue: f32,         // 0-360
       pub saturation: f32,  // 0-1
       pub value: f32,       // 0-1
       pub opacity: f32,     // 0-1
       pub recent_colors: Vec<[f32; 4]>,  // last 8 used colors
   }

   impl EditorUI {
       pub fn generate_color_picker(
           &self,
           picker: &ColorPickerState,
           x: f32,  // panel left x
           y: f32,  // panel top y
       ) -> (Vec<Vertex>, Vec<u32>) {
           let mut verts = Vec::new();
           let mut idxs = Vec::new();

           if !self.color_picker_visible { return (verts, idxs); }

           // SV box: 128x128 pixels
           // Divide into 16x16 grid of colored quads
           let sv_size = 128.0;
           let sv_cells = 16;
           let cell_size = sv_size / sv_cells as f32;

           for row in 0..sv_cells {
               for col in 0..sv_cells {
                   let s = col as f32 / sv_cells as f32;
                   let v = 1.0 - (row as f32 / sv_cells as f32);
                   let rgb = hsv_to_rgb(picker.hue, s, v);
                   let cx = x + col as f32 * cell_size;
                   let cy = y + row as f32 * cell_size;
                   add_quad(&mut verts, &mut idxs,
                       [cx, cy, 0.0], [cx + cell_size, cy, 0.0],
                       [cx + cell_size, cy + cell_size, 0.0], [cx, cy + cell_size, 0.0],
                       [rgb[0], rgb[1], rgb[2], 1.0]);
               }
           }

           // SV crosshair indicator
           let cross_x = x + picker.saturation * sv_size;
           let cross_y = y + (1.0 - picker.value) * sv_size;
           // Small white cross at current S,V position

           // Hue bar: 20x128 vertical strip
           let hue_x = x + sv_size + 8.0;
           let hue_h = sv_size;
           let hue_steps = 32;
           let step_h = hue_h / hue_steps as f32;

           for i in 0..hue_steps {
               let h = i as f32 / hue_steps as f32 * 360.0;
               let rgb = hsv_to_rgb(h, 1.0, 1.0);
               let cy = y + i as f32 * step_h;
               add_quad(&mut verts, &mut idxs,
                   [hue_x, cy, 0.0], [hue_x + 20.0, cy, 0.0],
                   [hue_x + 20.0, cy + step_h, 0.0], [hue_x, cy + step_h, 0.0],
                   [rgb[0], rgb[1], rgb[2], 1.0]);
           }

           // Hue indicator line
           let hue_indicator_y = y + (picker.hue / 360.0) * hue_h;

           // Opacity slider below SV box
           let opa_y = y + sv_size + 8.0;
           // Gradient from transparent to opaque

           // Recent colors: 8 swatches, 16x16 each
           let swatch_y = opa_y + 24.0;
           for (i, color) in picker.recent_colors.iter().take(8).enumerate() {
               let sx = x + i as f32 * 18.0;
               add_quad(&mut verts, &mut idxs,
                   [sx, swatch_y, 0.0], [sx + 16.0, swatch_y, 0.0],
                   [sx + 16.0, swatch_y + 16.0, 0.0], [sx, swatch_y + 16.0, 0.0],
                   *color);
           }

           (verts, idxs)
       }
   }
   ```

6. Implement click handling:
   ```rust
   pub enum UIAction {
       SelectTool(usize),
       AdjustSlider(SliderID, f32),
       SetHue(f32),
       SetSaturationValue(f32, f32),
       SetOpacity(f32),
       SelectRecentColor(usize),
   }

   impl EditorUI {
       pub fn handle_click(
           &mut self,
           x: f32,
           y: f32,
           stage: EditorStage,
           screen_w: f32,
           screen_h: f32,
       ) -> Option<UIAction> {
           // Check tool palette (left side, 48px wide)
           if x < 48.0 && self.tool_palette_visible {
               let tools = tools_for_stage(stage);
               let tool_index = ((y - 10.0) / 40.0) as usize;
               if tool_index < tools.len() {
                   return Some(UIAction::SelectTool(tool_index));
               }
           }

           // Check property panel (right side, 200px wide)
           if x > screen_w - 200.0 && self.property_panel_visible {
               // Determine which slider was clicked based on y position
               // Return UIAction::AdjustSlider with computed value
           }

           // Check color picker (when visible)
           if self.color_picker_visible {
               // Check SV box, hue bar, opacity slider, recent swatches
           }

           None
       }
   }
   ```

7. Implement slider rendering helper:
   ```rust
   fn render_slider(verts: &mut Vec<Vertex>, idxs: &mut Vec<u32>, slider: &UISlider) {
       // Label
       draw_text(verts, idxs, slider.label, slider.x, slider.y, 0.3, [0.8, 0.8, 0.8, 1.0]);

       // Track (thin dark bar)
       let track_y = slider.y + 14.0;
       add_quad(verts, idxs,
           [slider.x, track_y, 0.0], [slider.x + slider.width, track_y, 0.0],
           [slider.x + slider.width, track_y + 4.0, 0.0], [slider.x, track_y + 4.0, 0.0],
           [0.2, 0.2, 0.25, 1.0]);

       // Fill (colored portion)
       let t = (slider.value - slider.min) / (slider.max - slider.min);
       let fill_w = t * slider.width;
       add_quad(verts, idxs,
           [slider.x, track_y, 0.0], [slider.x + fill_w, track_y, 0.0],
           [slider.x + fill_w, track_y + 4.0, 0.0], [slider.x, track_y + 4.0, 0.0],
           [0.4, 0.6, 0.9, 1.0]);

       // Value text
       let val_str = format!("{:.2}", slider.value);
       draw_text(verts, idxs, &val_str,
           slider.x + slider.width - 30.0, slider.y, 0.3, [0.6, 0.6, 0.6, 1.0]);
   }
   ```

## Acceptance Criteria
- [ ] `ui_panels.rs` exists with `EditorUI`, `PropertyParams`, `ColorPickerState`, `UIAction` types
- [ ] Tool palette renders on left side with correct tools per stage (Draw2D, Extrude, Sculpt, Color, Save)
- [ ] Active tool is visually highlighted in the palette
- [ ] Property panel shows context-sensitive sliders appropriate to each stage
- [ ] HSV color picker renders SV box (128x128), hue bar (20x128), opacity slider, and 8 recent color swatches
- [ ] Click handling correctly identifies tool selection, slider interaction, and color picker interaction
- [ ] `UIAction` enum returns structured results from click handling
- [ ] Recent colors track the last 8 used colors
- [ ] All rendering uses `add_quad()` and `draw_text()` primitives (no external GUI library)
- [ ] `cargo check` passes with 0 errors

## Verification Commands
- `test -f /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs && echo EXISTS` -- expected: EXISTS
- `grep -c 'EditorUI\|generate_tool_palette\|generate_property_panel\|generate_color_picker' /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs` -- expected: > 0
- `grep -c 'ColorPickerState\|UIAction\|PropertyParams\|SliderID' /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs` -- expected: > 0
- `grep -c 'pub mod ui_panels' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs` -- expected: > 0
- `cd /home/hungaromakker/battle_tok && cargo check 2>&1; echo EXIT:$?` -- expected: EXIT:0

## Success Looks Like
When the editor opens, a tool palette appears on the left showing available tools with keyboard shortcuts (e.g., "D Freehand", "L Line"). Switching stages updates the palette to show the correct tools. A property panel on the right shows sliders for the current stage -- brush radius in sculpt mode, inflation in extrude mode. In Stage 4, a color picker appears with a saturation/value box, hue bar, and recent color swatches. Clicking a tool highlights it. Dragging a slider adjusts the value smoothly. The UI feels integrated and responsive, rendered with the same visual style as the rest of the game.

## Dependencies
- Depends on: US-P4-001 (needs editor skeleton with stage system)

## Complexity
- Complexity: complex
- Min iterations: 2
