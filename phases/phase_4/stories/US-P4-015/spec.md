# US-P4-015: Editor UI Panels (Tool Palette + Color Picker)

## Description
Create `src/game/asset_editor/ui_panels.rs` with a tool palette (left side), property panel (right side), and HSV color picker. All UI rendering uses existing `add_quad()` and `draw_text()` primitives -- no external UI framework. The tool palette shows one icon per tool for the current editor stage with highlighted selection and keyboard shortcut hints. The property panel shows context-sensitive sliders that change based on which stage is active. The HSV color picker provides a saturation-value box (128x128), vertical hue bar, opacity slider, primary/secondary color swatches, and a recent colors palette (last 8). The editor is a separate binary (`cargo run --bin battle_editor`). `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
Every creative tool needs a way for the user to see what tools are available, adjust parameters, and pick colors. Without UI panels, the artist must memorize keyboard shortcuts and has no visual feedback for settings like brush radius or grid size. The tool palette makes the editor discoverable -- a new user can immediately see what tools exist for the current stage and how to activate them. The property panel provides precise control over parameters that would be tedious to adjust via keyboard alone (e.g., fine-tuning inflation amount or smoothing factor with a slider instead of repeated key presses). The HSV color picker is essential for Stage 4 (Color/Paint) where artists need to pick exact colors, blend between primary and secondary, and recall recently used colors. Together these three panels transform the editor from a keyboard-driven prototype into a visually discoverable, professionally usable creative application.

## Goal
Create `src/game/asset_editor/ui_panels.rs` with `EditorUI` struct providing a stage-aware tool palette on the left, context-sensitive property panel with sliders on the right, and a full HSV color picker with primary/secondary swatches and recent colors history.

## Files to Create/Modify
- Create `src/game/asset_editor/ui_panels.rs` -- EditorUI, ToolPalette, PropertyPanel, HsvColorPicker, slider rendering, HSV conversion utilities
- Modify `src/game/asset_editor/mod.rs` -- Add `pub mod ui_panels;`, add `ui: EditorUI` field, wire rendering and input, update panels on stage change

## Implementation Steps
1. Define core UI data structures:
   ```rust
   pub struct EditorUI {
       pub tool_palette: ToolPalette,
       pub property_panel: PropertyPanel,
       pub color_picker: HsvColorPicker,
       pub visible: bool,
   }

   pub struct ToolPalette {
       pub tools: Vec<ToolEntry>,
       pub selected_index: usize,
       pub panel_x: f32,         // left edge (0.0)
       pub panel_width: f32,     // 48.0
   }

   pub struct ToolEntry {
       pub name: String,
       pub shortcut: String,     // e.g. "D", "L", "R"
       pub icon_char: char,      // single char icon placeholder
   }

   pub struct PropertyPanel {
       pub sliders: Vec<SliderState>,
       pub panel_x: f32,         // right edge (screen_w - panel_width)
       pub panel_width: f32,     // 200.0
       pub scroll_offset: f32,
   }

   pub struct SliderState {
       pub label: String,
       pub value: f32,
       pub min: f32,
       pub max: f32,
       pub step: f32,
       pub dragging: bool,
   }
   ```

2. Define HSV color picker:
   ```rust
   pub struct HsvColorPicker {
       pub hue: f32,             // 0..360
       pub saturation: f32,      // 0..1
       pub value: f32,           // 0..1 (brightness)
       pub opacity: f32,         // 0..1
       pub primary: [f32; 4],    // RGBA primary color
       pub secondary: [f32; 4],  // RGBA secondary color
       pub recent_colors: Vec<[f32; 4]>,  // last 8 colors
       pub sv_box_size: f32,     // 128.0
       pub hue_bar_width: f32,   // 20.0
       pub visible: bool,
       pub dragging_sv: bool,
       pub dragging_hue: bool,
       pub dragging_opacity: bool,
   }
   ```

3. Implement stage-specific tool lists:
   ```rust
   impl ToolPalette {
       pub fn tools_for_stage(stage: &EditorStage) -> Vec<ToolEntry> {
           match stage {
               EditorStage::Draw2D => vec![
                   ToolEntry { name: "Draw".into(), shortcut: "D".into(), icon_char: '/' },
                   ToolEntry { name: "Line".into(), shortcut: "L".into(), icon_char: '-' },
                   ToolEntry { name: "Rectangle".into(), shortcut: "R".into(), icon_char: '#' },
                   ToolEntry { name: "Ellipse".into(), shortcut: "E".into(), icon_char: 'O' },
                   ToolEntry { name: "Select".into(), shortcut: "S".into(), icon_char: '+' },
                   ToolEntry { name: "Erase".into(), shortcut: "X".into(), icon_char: 'x' },
               ],
               EditorStage::Extrude => vec![
                   ToolEntry { name: "Extrude".into(), shortcut: "E".into(), icon_char: '^' },
                   ToolEntry { name: "Profile".into(), shortcut: "P".into(), icon_char: '~' },
                   ToolEntry { name: "Inflate".into(), shortcut: "I".into(), icon_char: 'O' },
               ],
               EditorStage::Sculpt => vec![
                   ToolEntry { name: "Push".into(), shortcut: "P".into(), icon_char: '>' },
                   ToolEntry { name: "Pull".into(), shortcut: "L".into(), icon_char: '<' },
                   ToolEntry { name: "Smooth".into(), shortcut: "S".into(), icon_char: '~' },
                   ToolEntry { name: "Flatten".into(), shortcut: "F".into(), icon_char: '_' },
               ],
               EditorStage::Color => vec![
                   ToolEntry { name: "Paint".into(), shortcut: "P".into(), icon_char: '*' },
                   ToolEntry { name: "Fill".into(), shortcut: "F".into(), icon_char: '#' },
                   ToolEntry { name: "Eyedrop".into(), shortcut: "I".into(), icon_char: '|' },
                   ToolEntry { name: "Gradient".into(), shortcut: "G".into(), icon_char: '=' },
               ],
               EditorStage::Save => vec![
                   ToolEntry { name: "Save".into(), shortcut: "S".into(), icon_char: 'S' },
                   ToolEntry { name: "Load".into(), shortcut: "L".into(), icon_char: 'L' },
                   ToolEntry { name: "Export".into(), shortcut: "E".into(), icon_char: 'X' },
               ],
           }
       }
   }
   ```

4. Implement stage-specific property sliders:
   ```rust
   impl PropertyPanel {
       pub fn sliders_for_stage(stage: &EditorStage) -> Vec<SliderState> {
           match stage {
               EditorStage::Draw2D => vec![
                   SliderState::new("Grid Size", 16.0, 4.0, 64.0, 1.0),
                   SliderState::new("Symmetry", 0.0, 0.0, 1.0, 1.0),  // 0=off, 1=on toggle
               ],
               EditorStage::Extrude => vec![
                   SliderState::new("Inflation", 0.0, -1.0, 2.0, 0.05),
                   SliderState::new("Thickness", 0.5, 0.05, 3.0, 0.05),
                   SliderState::new("Profile Curve", 0.5, 0.0, 1.0, 0.01),
               ],
               EditorStage::Sculpt => vec![
                   SliderState::new("Brush Radius", 1.0, 0.1, 5.0, 0.1),
                   SliderState::new("Smooth Factor", 0.5, 0.0, 1.0, 0.05),
               ],
               EditorStage::Color => vec![
                   SliderState::new("Brush Radius", 0.5, 0.05, 3.0, 0.05),
                   SliderState::new("Opacity", 1.0, 0.0, 1.0, 0.05),
                   SliderState::new("Hardness", 0.8, 0.0, 1.0, 0.05),
               ],
               EditorStage::Save => vec![
                   // Save stage uses text fields (category dropdown, name input, variety params)
                   // not sliders -- handled separately in save UI
               ],
           }
       }
   }
   ```

5. Implement tool palette rendering:
   ```rust
   impl ToolPalette {
       pub fn generate_vertices(
           &self,
           screen_h: f32,
       ) -> (Vec<Vertex>, Vec<u32>) {
           let mut verts = Vec::new();
           let mut idxs = Vec::new();

           // Panel background (left side, 48px wide, full height)
           add_quad(&mut verts, &mut idxs,
               0.0, 0.0, self.panel_width, screen_h,
               [0.1, 0.1, 0.12, 0.9]);

           // Tool buttons (40x40 each, 4px padding)
           let button_size = 40.0;
           let padding = 4.0;
           for (i, tool) in self.tools.iter().enumerate() {
               let y = padding + i as f32 * (button_size + padding);
               let is_selected = i == self.selected_index;

               // Button background (highlighted if selected)
               let bg = if is_selected {
                   [0.3, 0.5, 0.7, 0.9]
               } else {
                   [0.18, 0.18, 0.22, 0.8]
               };
               add_quad(&mut verts, &mut idxs,
                   padding, y, button_size, button_size, bg);

               // Icon character (centered in button)
               draw_text(&mut verts, &mut idxs,
                   &tool.icon_char.to_string(),
                   padding + 14.0, y + 10.0, 0.5, [1.0; 4]);

               // Keyboard shortcut hint (bottom-right corner, small)
               draw_text(&mut verts, &mut idxs,
                   &tool.shortcut,
                   padding + button_size - 12.0, y + button_size - 14.0,
                   0.25, [0.5, 0.5, 0.5, 0.7]);
           }

           (verts, idxs)
       }
   }
   ```

6. Implement property panel rendering with sliders:
   ```rust
   impl PropertyPanel {
       pub fn generate_vertices(
           &self,
           screen_w: f32,
           screen_h: f32,
       ) -> (Vec<Vertex>, Vec<u32>) {
           let mut verts = Vec::new();
           let mut idxs = Vec::new();

           let panel_left = screen_w - self.panel_width;

           // Panel background (right side, 200px wide, full height)
           add_quad(&mut verts, &mut idxs,
               panel_left, 0.0, self.panel_width, screen_h,
               [0.1, 0.1, 0.12, 0.9]);

           // Title
           draw_text(&mut verts, &mut idxs, "Properties",
               panel_left + 8.0, 8.0, 0.4, [0.8, 0.8, 0.8, 1.0]);

           // Sliders
           let slider_h = 32.0;
           let padding = 8.0;
           for (i, slider) in self.sliders.iter().enumerate() {
               let y = 40.0 + i as f32 * (slider_h + padding * 2.0) - self.scroll_offset;

               // Label
               draw_text(&mut verts, &mut idxs, &slider.label,
                   panel_left + padding, y, 0.3, [0.7, 0.7, 0.7, 1.0]);

               // Track background
               let track_y = y + 16.0;
               let track_w = self.panel_width - padding * 2.0;
               add_quad(&mut verts, &mut idxs,
                   panel_left + padding, track_y, track_w, 8.0,
                   [0.2, 0.2, 0.25, 0.8]);

               // Fill (proportional to current value within range)
               let t = (slider.value - slider.min) / (slider.max - slider.min);
               add_quad(&mut verts, &mut idxs,
                   panel_left + padding, track_y, track_w * t, 8.0,
                   [0.4, 0.6, 0.8, 0.9]);

               // Value text (right-aligned)
               draw_text(&mut verts, &mut idxs,
                   &format!("{:.2}", slider.value),
                   panel_left + self.panel_width - 50.0, y, 0.3, [0.9; 4]);
           }

           (verts, idxs)
       }
   }
   ```

7. Implement HSV color picker rendering:
   ```rust
   impl HsvColorPicker {
       pub fn generate_vertices(
           &self,
           base_x: f32,
           base_y: f32,
       ) -> (Vec<Vertex>, Vec<u32>) {
           let mut verts = Vec::new();
           let mut idxs = Vec::new();

           if !self.visible { return (verts, idxs); }

           // SV box (128x128): rendered as grid of colored quads
           // X-axis = saturation (0 left to 1 right)
           // Y-axis = value/brightness (1 top to 0 bottom)
           let sv_steps = 8;
           let cell = self.sv_box_size / sv_steps as f32;
           for sx in 0..sv_steps {
               for sy in 0..sv_steps {
                   let s = sx as f32 / sv_steps as f32;
                   let v = 1.0 - (sy as f32 / sv_steps as f32);
                   let color = hsv_to_rgba(self.hue, s, v, 1.0);
                   add_quad(&mut verts, &mut idxs,
                       base_x + sx as f32 * cell,
                       base_y + sy as f32 * cell,
                       cell, cell, color);
               }
           }

           // SV cursor (crosshair at current S,V position)
           let cursor_x = base_x + self.saturation * self.sv_box_size;
           let cursor_y = base_y + (1.0 - self.value) * self.sv_box_size;
           add_quad(&mut verts, &mut idxs,
               cursor_x - 4.0, cursor_y - 1.0, 8.0, 2.0, [1.0; 4]);
           add_quad(&mut verts, &mut idxs,
               cursor_x - 1.0, cursor_y - 4.0, 2.0, 8.0, [1.0; 4]);

           // Hue bar (vertical, 20px wide, beside SV box)
           let hue_x = base_x + self.sv_box_size + 8.0;
           let hue_steps = 12;
           let hue_cell_h = self.sv_box_size / hue_steps as f32;
           for i in 0..hue_steps {
               let h = (i as f32 / hue_steps as f32) * 360.0;
               let color = hsv_to_rgba(h, 1.0, 1.0, 1.0);
               add_quad(&mut verts, &mut idxs,
                   hue_x, base_y + i as f32 * hue_cell_h,
                   self.hue_bar_width, hue_cell_h, color);
           }

           // Hue cursor (horizontal line at current hue)
           let hue_cursor_y = base_y + (self.hue / 360.0) * self.sv_box_size;
           add_quad(&mut verts, &mut idxs,
               hue_x - 2.0, hue_cursor_y - 1.0,
               self.hue_bar_width + 4.0, 2.0, [1.0; 4]);

           // Opacity slider (horizontal, below SV box)
           let opacity_y = base_y + self.sv_box_size + 8.0;
           add_quad(&mut verts, &mut idxs,
               base_x, opacity_y, self.sv_box_size, 12.0,
               [0.2, 0.2, 0.2, 0.8]);
           add_quad(&mut verts, &mut idxs,
               base_x, opacity_y, self.sv_box_size * self.opacity, 12.0,
               hsv_to_rgba(self.hue, self.saturation, self.value, 1.0));
           draw_text(&mut verts, &mut idxs, "Opacity",
               base_x, opacity_y - 12.0, 0.25, [0.6; 4]);

           // Primary/Secondary swatches (two 24x24 squares, overlapping)
           let swatch_y = opacity_y + 24.0;
           add_quad(&mut verts, &mut idxs,
               base_x + 12.0, swatch_y + 4.0, 24.0, 24.0, self.secondary);
           add_quad(&mut verts, &mut idxs,
               base_x, swatch_y, 24.0, 24.0, self.primary);

           // Recent colors (last 8, horizontal row of 14x14 squares)
           let recent_y = swatch_y + 32.0;
           draw_text(&mut verts, &mut idxs, "Recent",
               base_x, recent_y, 0.25, [0.6; 4]);
           for (i, color) in self.recent_colors.iter().enumerate() {
               let rx = base_x + i as f32 * 18.0;
               add_quad(&mut verts, &mut idxs,
                   rx, recent_y + 14.0, 14.0, 14.0, *color);
           }

           (verts, idxs)
       }
   }
   ```

8. Implement HSV color picker input handling:
   ```rust
   impl HsvColorPicker {
       pub fn handle_mouse_down(&mut self, x: f32, y: f32, base_x: f32, base_y: f32) -> bool {
           if !self.visible { return false; }

           // Check SV box hit
           let sv_right = base_x + self.sv_box_size;
           let sv_bottom = base_y + self.sv_box_size;
           if x >= base_x && x <= sv_right && y >= base_y && y <= sv_bottom {
               self.dragging_sv = true;
               self.update_sv(x, y, base_x, base_y);
               return true;
           }

           // Check hue bar hit
           let hue_x = base_x + self.sv_box_size + 8.0;
           if x >= hue_x && x <= hue_x + self.hue_bar_width
               && y >= base_y && y <= sv_bottom {
               self.dragging_hue = true;
               self.update_hue(y, base_y);
               return true;
           }

           // Check opacity slider hit
           let opacity_y = base_y + self.sv_box_size + 8.0;
           if x >= base_x && x <= sv_right
               && y >= opacity_y && y <= opacity_y + 12.0 {
               self.dragging_opacity = true;
               self.update_opacity(x, base_x);
               return true;
           }

           // Check recent color swatch clicks
           let recent_y = base_y + self.sv_box_size + 64.0;
           for (i, color) in self.recent_colors.iter().enumerate() {
               let rx = base_x + i as f32 * 18.0;
               if x >= rx && x <= rx + 14.0 && y >= recent_y && y <= recent_y + 14.0 {
                   self.set_primary(*color);
                   return true;
               }
           }

           false
       }

       pub fn handle_mouse_drag(&mut self, x: f32, y: f32, base_x: f32, base_y: f32) {
           if self.dragging_sv { self.update_sv(x, y, base_x, base_y); }
           if self.dragging_hue { self.update_hue(y, base_y); }
           if self.dragging_opacity { self.update_opacity(x, base_x); }
       }

       pub fn handle_mouse_up(&mut self) {
           if self.dragging_sv || self.dragging_hue || self.dragging_opacity {
               self.push_recent(self.primary);
           }
           self.dragging_sv = false;
           self.dragging_hue = false;
           self.dragging_opacity = false;
       }

       fn update_sv(&mut self, x: f32, y: f32, base_x: f32, base_y: f32) {
           self.saturation = ((x - base_x) / self.sv_box_size).clamp(0.0, 1.0);
           self.value = 1.0 - ((y - base_y) / self.sv_box_size).clamp(0.0, 1.0);
           self.primary = hsv_to_rgba(self.hue, self.saturation, self.value, self.opacity);
       }

       fn update_hue(&mut self, y: f32, base_y: f32) {
           self.hue = ((y - base_y) / self.sv_box_size * 360.0).clamp(0.0, 359.99);
           self.primary = hsv_to_rgba(self.hue, self.saturation, self.value, self.opacity);
       }

       fn update_opacity(&mut self, x: f32, base_x: f32) {
           self.opacity = ((x - base_x) / self.sv_box_size).clamp(0.0, 1.0);
           self.primary = hsv_to_rgba(self.hue, self.saturation, self.value, self.opacity);
       }

       pub fn set_primary(&mut self, color: [f32; 4]) {
           self.primary = color;
           let (h, s, v) = rgba_to_hsv(color[0], color[1], color[2]);
           self.hue = h;
           self.saturation = s;
           self.value = v;
           self.opacity = color[3];
       }

       pub fn swap_colors(&mut self) {
           std::mem::swap(&mut self.primary, &mut self.secondary);
           self.set_primary(self.primary);
       }

       pub fn push_recent(&mut self, color: [f32; 4]) {
           self.recent_colors.retain(|c| c != &color);
           self.recent_colors.insert(0, color);
           if self.recent_colors.len() > 8 {
               self.recent_colors.truncate(8);
           }
       }
   }
   ```

9. Implement slider input handling:
   ```rust
   impl PropertyPanel {
       pub fn handle_mouse_down(&mut self, x: f32, y: f32, screen_w: f32) -> Option<usize> {
           let panel_left = screen_w - self.panel_width;
           if x < panel_left { return None; }

           let padding = 8.0;
           let slider_h = 32.0;
           for (i, slider) in self.sliders.iter_mut().enumerate() {
               let sy = 40.0 + i as f32 * (slider_h + padding * 2.0) - self.scroll_offset;
               let track_y = sy + 16.0;
               if y >= track_y && y <= track_y + 8.0 {
                   slider.dragging = true;
                   let t = ((x - panel_left - padding) / (self.panel_width - padding * 2.0))
                       .clamp(0.0, 1.0);
                   slider.value = slider.min + t * (slider.max - slider.min);
                   slider.value = (slider.value / slider.step).round() * slider.step;
                   return Some(i);
               }
           }
           None
       }

       pub fn handle_mouse_drag(&mut self, x: f32, screen_w: f32) {
           let panel_left = screen_w - self.panel_width;
           let padding = 8.0;
           for slider in &mut self.sliders {
               if slider.dragging {
                   let t = ((x - panel_left - padding) / (self.panel_width - padding * 2.0))
                       .clamp(0.0, 1.0);
                   slider.value = slider.min + t * (slider.max - slider.min);
                   slider.value = (slider.value / slider.step).round() * slider.step;
               }
           }
       }

       pub fn handle_mouse_up(&mut self) {
           for slider in &mut self.sliders {
               slider.dragging = false;
           }
       }
   }
   ```

10. Implement constructors and `Default`:
    ```rust
    impl SliderState {
        pub fn new(label: &str, value: f32, min: f32, max: f32, step: f32) -> Self {
            Self {
                label: label.to_string(),
                value,
                min,
                max,
                step,
                dragging: false,
            }
        }
    }

    impl Default for HsvColorPicker {
        fn default() -> Self {
            Self {
                hue: 0.0,
                saturation: 1.0,
                value: 1.0,
                opacity: 1.0,
                primary: [1.0, 0.0, 0.0, 1.0],
                secondary: [1.0, 1.0, 1.0, 1.0],
                recent_colors: Vec::new(),
                sv_box_size: 128.0,
                hue_bar_width: 20.0,
                visible: false,
                dragging_sv: false,
                dragging_hue: false,
                dragging_opacity: false,
            }
        }
    }

    impl Default for EditorUI {
        fn default() -> Self {
            Self {
                tool_palette: ToolPalette {
                    tools: Vec::new(),
                    selected_index: 0,
                    panel_x: 0.0,
                    panel_width: 48.0,
                },
                property_panel: PropertyPanel {
                    sliders: Vec::new(),
                    panel_x: 0.0,
                    panel_width: 200.0,
                    scroll_offset: 0.0,
                },
                color_picker: HsvColorPicker::default(),
                visible: true,
            }
        }
    }
    ```

11. Wire stage transitions in `mod.rs`:
    ```rust
    impl AssetEditor {
        pub fn switch_stage(&mut self, stage: EditorStage) {
            self.stage = stage;
            self.ui.tool_palette.tools = ToolPalette::tools_for_stage(&self.stage);
            self.ui.tool_palette.selected_index = 0;
            self.ui.property_panel.sliders = PropertyPanel::sliders_for_stage(&self.stage);
            // Show color picker only in Color stage
            self.ui.color_picker.visible = matches!(self.stage, EditorStage::Color);
        }
    }
    ```

12. Implement HSV conversion utilities:
    ```rust
    pub fn hsv_to_rgba(h: f32, s: f32, v: f32, a: f32) -> [f32; 4] {
        let c = v * s;
        let h_prime = h / 60.0;
        let x = c * (1.0 - ((h_prime % 2.0) - 1.0).abs());
        let m = v - c;
        let (r, g, b) = match h_prime as u32 {
            0 => (c, x, 0.0),
            1 => (x, c, 0.0),
            2 => (0.0, c, x),
            3 => (0.0, x, c),
            4 => (x, 0.0, c),
            _ => (c, 0.0, x),
        };
        [r + m, g + m, b + m, a]
    }

    pub fn rgba_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        let h = if delta == 0.0 {
            0.0
        } else if max == r {
            60.0 * (((g - b) / delta) % 6.0)
        } else if max == g {
            60.0 * (((b - r) / delta) + 2.0)
        } else {
            60.0 * (((r - g) / delta) + 4.0)
        };
        let h = if h < 0.0 { h + 360.0 } else { h };
        let s = if max == 0.0 { 0.0 } else { delta / max };
        (h, s, max)
    }
    ```

## Code Patterns
Follow the existing `add_quad()` + `draw_text()` rendering pattern used across the editor:
```rust
// All UI is quads and text -- no external UI library
add_quad(&mut verts, &mut idxs, x, y, w, h, color);
draw_text(&mut verts, &mut idxs, text, x, y, scale, color);
```

The `EditorUI` struct follows the same pattern as other editor subsystems (library, placement): it has a `generate_vertices()` method that returns `(Vec<Vertex>, Vec<u32>)` and separate input handler methods for mouse down/drag/up.

## Acceptance Criteria
- [ ] `ui_panels.rs` exists with `EditorUI`, `ToolPalette`, `PropertyPanel`, `HsvColorPicker` types
- [ ] Tool palette renders on left side with correct tools per stage
- [ ] Active tool is highlighted in the palette
- [ ] Keyboard shortcut hints are displayed on each tool button
- [ ] Property panel renders on right side with context-sensitive sliders
- [ ] Stage 1 shows: Grid Size, Symmetry toggle
- [ ] Stage 2 shows: Inflation, Thickness, Profile sliders
- [ ] Stage 3 shows: Brush Radius, Smooth Factor sliders
- [ ] Stage 4 shows: Brush Radius, Opacity, Hardness sliders + color picker
- [ ] Stage 5 shows: Category dropdown, Name input, Variety params
- [ ] HSV color picker has SV box (128x128), Hue bar (vertical), Opacity slider
- [ ] Primary/Secondary color swatches are displayed and swappable
- [ ] Recent colors palette tracks last 8 used colors
- [ ] Slider click-and-drag updates values with proper step snapping
- [ ] Color picker mouse interaction updates H, S, V, and opacity correctly
- [ ] `hsv_to_rgba()` and `rgba_to_hsv()` conversion functions work correctly
- [ ] `cargo check` passes with 0 errors

## Verification Commands
- `test -f /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs && echo EXISTS` -- expected: EXISTS
- `grep -c 'EditorUI\|ToolPalette\|PropertyPanel\|HsvColorPicker' /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs` -- expected: > 0
- `grep -c 'hsv_to_rgba\|rgba_to_hsv\|generate_vertices\|handle_mouse' /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs` -- expected: > 0
- `grep -c 'tools_for_stage\|sliders_for_stage' /home/hungaromakker/battle_tok/src/game/asset_editor/ui_panels.rs` -- expected: > 0
- `grep -c 'pub mod ui_panels' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs` -- expected: > 0
- `cd /home/hungaromakker/battle_tok && cargo check 2>&1; echo EXIT:$?` -- expected: EXIT:0

## Success Looks Like
The artist opens the editor and immediately sees a vertical tool palette on the left with icons for the current stage's tools. Each button shows its keyboard shortcut. They click the Line tool and it highlights. On the right, a property panel shows sliders relevant to the current stage -- in Stage 1 they see "Grid Size" and "Symmetry", in Stage 2 they see "Inflation", "Thickness", and "Profile Curve". When they switch to Stage 4 (Color), a color picker appears with the familiar SV gradient box (128x128) and a vertical hue rainbow bar. They click in the SV box to pick a green, drag the hue bar toward blue, and the brush color updates in real-time. They adjust the opacity slider below. Eight recently used colors appear as clickable swatches. They click the primary/secondary swatches to swap between their two chosen colors. The sliders respond smoothly to dragging and show precise values. The entire UI feels integrated and responsive without any external UI library.

## Dependencies
- Depends on: US-P4-001 (needs AssetEditor struct and EditorStage enum)

## Complexity
- Complexity: complex
- Min iterations: 2
