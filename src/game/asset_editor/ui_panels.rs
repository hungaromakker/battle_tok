//! Editor UI Panels
//!
//! Provides three UI panel systems for the asset editor:
//! - `ToolPalette` - Left-side tool buttons per stage with keyboard shortcuts
//! - `PropertyPanel` - Right-side sliders per stage for parameter adjustment
//! - `HsvColorPicker` - HSV color picker with SV box, hue bar, and recent colors

use crate::game::types::{Mesh, Vertex};
use crate::game::ui::slider::UISlider;
use crate::game::ui::text::{add_quad, draw_text};

use super::EditorStage;

// ============================================================================
// TOOL DEFINITIONS
// ============================================================================

/// A single tool entry in the palette.
#[derive(Debug, Clone, Copy)]
pub struct ToolDef {
    /// Display name of the tool
    pub name: &'static str,
    /// Single-character icon shown on the button
    pub icon: char,
    /// Keyboard shortcut character
    pub shortcut: char,
}

/// Return the tool list for a given editor stage.
fn tools_for_stage(stage: &EditorStage) -> &'static [ToolDef] {
    match stage {
        EditorStage::Draw2D => &[
            ToolDef {
                name: "Pen",
                icon: 'P',
                shortcut: 'P',
            },
            ToolDef {
                name: "Line",
                icon: 'L',
                shortcut: 'L',
            },
            ToolDef {
                name: "Eraser",
                icon: 'E',
                shortcut: 'E',
            },
            ToolDef {
                name: "Close Path",
                icon: 'C',
                shortcut: 'C',
            },
        ],
        EditorStage::Extrude => &[
            ToolDef {
                name: "Adjust Depth",
                icon: 'D',
                shortcut: 'D',
            },
            ToolDef {
                name: "Inflation",
                icon: 'I',
                shortcut: 'I',
            },
            ToolDef {
                name: "Reset",
                icon: 'R',
                shortcut: 'R',
            },
        ],
        EditorStage::Sculpt => &[
            ToolDef {
                name: "Pull",
                icon: 'P',
                shortcut: 'P',
            },
            ToolDef {
                name: "Push",
                icon: 'S',
                shortcut: 'S',
            },
            ToolDef {
                name: "Smooth",
                icon: 'M',
                shortcut: 'M',
            },
            ToolDef {
                name: "Flatten",
                icon: 'F',
                shortcut: 'F',
            },
        ],
        EditorStage::Color => &[
            ToolDef {
                name: "Brush",
                icon: 'B',
                shortcut: 'B',
            },
            ToolDef {
                name: "Fill",
                icon: 'F',
                shortcut: 'F',
            },
            ToolDef {
                name: "Gradient",
                icon: 'G',
                shortcut: 'G',
            },
            ToolDef {
                name: "Eyedropper",
                icon: 'I',
                shortcut: 'I',
            },
        ],
        EditorStage::Save => &[
            ToolDef {
                name: "Save",
                icon: 'S',
                shortcut: 'S',
            },
            ToolDef {
                name: "Load",
                icon: 'L',
                shortcut: 'L',
            },
            ToolDef {
                name: "Export",
                icon: 'E',
                shortcut: 'E',
            },
        ],
    }
}

// ============================================================================
// TOOL PALETTE
// ============================================================================

/// Left-side tool palette showing available tools for the current stage.
///
/// Each tool is rendered as a button with an icon letter and keyboard shortcut.
/// The selected tool is visually highlighted.
pub struct ToolPalette {
    /// Whether the palette is visible
    pub visible: bool,
    /// Index of the currently selected tool in the stage tool list
    pub selected_tool: usize,
    /// Left edge X position in pixels
    pub panel_x: f32,
    /// Top edge Y position in pixels
    pub panel_y: f32,
    /// Size of each tool button in pixels
    pub button_size: f32,
    /// Spacing between buttons in pixels
    pub button_spacing: f32,
}

impl Default for ToolPalette {
    fn default() -> Self {
        Self {
            visible: true,
            selected_tool: 0,
            panel_x: 10.0,
            panel_y: 60.0,
            button_size: 48.0,
            button_spacing: 8.0,
        }
    }
}

impl ToolPalette {
    /// Create a new tool palette at the default position.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the tool definitions for a given stage.
    pub fn tools_for_stage<'a>(&self, stage: &'a EditorStage) -> &'static [ToolDef] {
        tools_for_stage(stage)
    }

    /// Render the tool palette as a mesh overlay.
    ///
    /// Draws a vertical stack of tool buttons on the left side of the screen.
    /// Each button shows a large icon letter and a smaller shortcut hint.
    /// The selected tool is highlighted with a blue background.
    pub fn render(
        &self,
        stage: &EditorStage,
        screen_width: f32,
        screen_height: f32,
    ) -> Mesh {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        if !self.visible {
            return Mesh { vertices, indices };
        }

        let tools = tools_for_stage(stage);
        if tools.is_empty() {
            return Mesh { vertices, indices };
        }

        let to_ndc = |px: f32, py: f32| -> [f32; 3] {
            [
                (px / screen_width) * 2.0 - 1.0,
                1.0 - (py / screen_height) * 2.0,
                0.0,
            ]
        };

        // Panel background
        let panel_h = tools.len() as f32 * (self.button_size + self.button_spacing)
            + self.button_spacing;
        let bg_color = [0.10, 0.10, 0.13, 1.0];
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(self.panel_x - 4.0, self.panel_y - 4.0),
            to_ndc(self.panel_x + self.button_size + 4.0, self.panel_y - 4.0),
            to_ndc(
                self.panel_x + self.button_size + 4.0,
                self.panel_y + panel_h,
            ),
            to_ndc(self.panel_x - 4.0, self.panel_y + panel_h),
            bg_color,
        );

        // Draw title "TOOLS"
        draw_text(
            &mut vertices,
            &mut indices,
            "TOOLS",
            self.panel_x + 4.0,
            self.panel_y - 22.0,
            2.0,
            [0.9, 0.8, 0.4, 1.0],
            screen_width,
            screen_height,
        );

        // Draw each tool button
        for (i, tool) in tools.iter().enumerate() {
            let btn_y =
                self.panel_y + i as f32 * (self.button_size + self.button_spacing);

            // Button background: highlighted if selected
            let btn_color = if i == self.selected_tool {
                [0.3, 0.5, 0.8, 1.0] // Blue highlight
            } else {
                [0.20, 0.20, 0.22, 1.0] // Dark gray
            };

            add_quad(
                &mut vertices,
                &mut indices,
                to_ndc(self.panel_x, btn_y),
                to_ndc(self.panel_x + self.button_size, btn_y),
                to_ndc(
                    self.panel_x + self.button_size,
                    btn_y + self.button_size,
                ),
                to_ndc(self.panel_x, btn_y + self.button_size),
                btn_color,
            );

            // Icon letter (large, centered)
            let icon_buf = [tool.icon as u8; 1];
            let icon_text = std::str::from_utf8(&icon_buf).unwrap_or("?");
            draw_text(
                &mut vertices,
                &mut indices,
                icon_text,
                self.panel_x + 16.0,
                btn_y + 8.0,
                3.0,
                [1.0, 1.0, 1.0, 1.0],
                screen_width,
                screen_height,
            );

            // Shortcut hint (small, bottom-right)
            let shortcut_buf = [tool.shortcut as u8; 1];
            let shortcut_text =
                std::str::from_utf8(&shortcut_buf).unwrap_or("?");
            draw_text(
                &mut vertices,
                &mut indices,
                shortcut_text,
                self.panel_x + 34.0,
                btn_y + 34.0,
                1.5,
                [0.6, 0.6, 0.6, 1.0],
                screen_width,
                screen_height,
            );
        }

        Mesh { vertices, indices }
    }

    /// Handle a mouse click and return the index of the tool that was hit, if any.
    pub fn handle_click(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        stage: &EditorStage,
    ) -> Option<usize> {
        if !self.visible {
            return None;
        }

        let tools = tools_for_stage(stage);
        for (i, _tool) in tools.iter().enumerate() {
            let btn_y =
                self.panel_y + i as f32 * (self.button_size + self.button_spacing);

            if mouse_x >= self.panel_x
                && mouse_x <= self.panel_x + self.button_size
                && mouse_y >= btn_y
                && mouse_y <= btn_y + self.button_size
            {
                self.selected_tool = i;
                return Some(i);
            }
        }

        None
    }

    /// Reset the selected tool index to 0 (called when stage changes).
    pub fn reset_selection(&mut self) {
        self.selected_tool = 0;
    }
}

// ============================================================================
// PROPERTY PANEL
// ============================================================================

/// Right-side property panel with stage-specific sliders.
///
/// Rebuilds its slider list when the editor stage changes.
pub struct PropertyPanel {
    /// Whether the panel is visible
    pub visible: bool,
    /// Left X position of the panel
    pub panel_x: f32,
    /// Top Y position of the panel
    pub panel_y: f32,
    /// Panel width in pixels
    pub panel_width: f32,
    /// Stage-specific sliders
    pub sliders: Vec<UISlider>,
    /// Which slider is currently being dragged (-1 = none)
    pub dragging_slider: i32,
    /// Slider labels (parallel to sliders vec)
    pub slider_labels: Vec<&'static str>,
    /// Title text for the current stage section
    pub stage_title: &'static str,
}

impl Default for PropertyPanel {
    fn default() -> Self {
        let mut panel = Self {
            visible: true,
            panel_x: 0.0, // Will be set based on screen width
            panel_y: 60.0,
            panel_width: 220.0,
            sliders: Vec::new(),
            dragging_slider: -1,
            slider_labels: Vec::new(),
            stage_title: "DRAW 2D",
        };
        panel.rebuild_for_stage(&EditorStage::Draw2D);
        panel
    }
}

impl PropertyPanel {
    /// Create a new property panel positioned relative to the right edge.
    ///
    /// `screen_width` is used to position the panel on the right side.
    pub fn new(screen_width: f32) -> Self {
        let mut panel = Self {
            panel_x: screen_width - 240.0,
            ..Self::default()
        };
        panel.rebuild_for_stage(&EditorStage::Draw2D);
        panel
    }

    /// Update the panel X position when the window resizes.
    pub fn set_screen_width(&mut self, screen_width: f32) {
        self.panel_x = screen_width - 240.0;
        // Reposition sliders to the new X
        for slider in &mut self.sliders {
            slider.x = self.panel_x + 10.0;
        }
    }

    /// Rebuild the slider list for the given editor stage.
    ///
    /// Each stage has its own set of parameters exposed as sliders.
    pub fn rebuild_for_stage(&mut self, stage: &EditorStage) {
        self.sliders.clear();
        self.slider_labels.clear();
        self.dragging_slider = -1;

        let x = self.panel_x + 10.0;
        let base_y = self.panel_y + 40.0;
        let spacing = 50.0;

        match stage {
            EditorStage::Draw2D => {
                self.stage_title = "DRAW 2D";
                self.slider_labels.push("GRID SIZE");
                self.sliders.push(UISlider::new(
                    "Grid Size",
                    x,
                    base_y,
                    0.5, // default 32 out of 64 range
                    [0.4, 0.7, 1.0, 1.0],
                ));
                self.slider_labels.push("SNAP");
                self.sliders.push(UISlider::new(
                    "Snap Strength",
                    x,
                    base_y + spacing,
                    0.5,
                    [0.5, 0.8, 0.5, 1.0],
                ));
            }
            EditorStage::Extrude => {
                self.stage_title = "EXTRUDE";
                self.slider_labels.push("DEPTH");
                self.sliders.push(UISlider::new(
                    "Depth",
                    x,
                    base_y,
                    0.2, // 1.0 / 5.0
                    [0.7, 0.5, 0.3, 1.0],
                ));
                self.slider_labels.push("INFLATION");
                self.sliders.push(UISlider::new(
                    "Inflation",
                    x,
                    base_y + spacing,
                    0.5, // 0.0 mapped to center
                    [0.5, 0.5, 0.8, 1.0],
                ));
                self.slider_labels.push("THICKNESS");
                self.sliders.push(UISlider::new(
                    "Thickness",
                    x,
                    base_y + spacing * 2.0,
                    0.0,
                    [0.6, 0.6, 0.6, 1.0],
                ));
            }
            EditorStage::Sculpt => {
                self.stage_title = "SCULPT";
                self.slider_labels.push("RADIUS");
                self.sliders.push(UISlider::new(
                    "Brush Radius",
                    x,
                    base_y,
                    0.17, // 0.5 / 3.0
                    [0.8, 0.5, 0.3, 1.0],
                ));
                self.slider_labels.push("STRENGTH");
                self.sliders.push(UISlider::new(
                    "Strength",
                    x,
                    base_y + spacing,
                    0.3,
                    [0.3, 0.7, 0.5, 1.0],
                ));
                self.slider_labels.push("SMOOTH ITER");
                self.sliders.push(UISlider::new(
                    "Smooth Iterations",
                    x,
                    base_y + spacing * 2.0,
                    0.3, // 3 / 10
                    [0.5, 0.5, 0.8, 1.0],
                ));
            }
            EditorStage::Color => {
                self.stage_title = "COLOR";
                self.slider_labels.push("RADIUS");
                self.sliders.push(UISlider::new(
                    "Brush Radius",
                    x,
                    base_y,
                    0.17,
                    [0.8, 0.5, 0.3, 1.0],
                ));
                self.slider_labels.push("OPACITY");
                self.sliders.push(UISlider::new(
                    "Opacity",
                    x,
                    base_y + spacing,
                    1.0,
                    [0.7, 0.7, 0.7, 1.0],
                ));
                self.slider_labels.push("HARDNESS");
                self.sliders.push(UISlider::new(
                    "Hardness",
                    x,
                    base_y + spacing * 2.0,
                    0.5,
                    [0.5, 0.8, 0.5, 1.0],
                ));
            }
            EditorStage::Save => {
                self.stage_title = "SAVE";
                // Save stage uses text-button style rather than sliders,
                // but we expose a minimal slider for variety count.
                self.slider_labels.push("VARIETIES");
                self.sliders.push(UISlider::new(
                    "Varieties",
                    x,
                    base_y,
                    0.1, // 1 / 10
                    [0.6, 0.6, 0.8, 1.0],
                ));
            }
        }
    }

    /// Render the property panel as a mesh overlay.
    pub fn render(&self, screen_width: f32, screen_height: f32) -> Mesh {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        if !self.visible {
            return Mesh { vertices, indices };
        }

        let to_ndc = |px: f32, py: f32| -> [f32; 3] {
            [
                (px / screen_width) * 2.0 - 1.0,
                1.0 - (py / screen_height) * 2.0,
                0.0,
            ]
        };

        // Panel background
        let total_h = 40.0
            + self.sliders.len() as f32 * 50.0
            + 20.0;
        let bg_color = [0.10, 0.10, 0.13, 1.0];
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(self.panel_x, self.panel_y - 4.0),
            to_ndc(self.panel_x + self.panel_width, self.panel_y - 4.0),
            to_ndc(
                self.panel_x + self.panel_width,
                self.panel_y + total_h,
            ),
            to_ndc(self.panel_x, self.panel_y + total_h),
            bg_color,
        );

        // Stage title
        draw_text(
            &mut vertices,
            &mut indices,
            self.stage_title,
            self.panel_x + 10.0,
            self.panel_y + 8.0,
            2.5,
            [1.0, 0.9, 0.5, 1.0],
            screen_width,
            screen_height,
        );

        // Draw each slider with label
        for (i, slider) in self.sliders.iter().enumerate() {
            // Label above slider
            if let Some(label) = self.slider_labels.get(i) {
                draw_text(
                    &mut vertices,
                    &mut indices,
                    label,
                    slider.x,
                    slider.y - 18.0,
                    2.0,
                    [0.9, 0.9, 0.9, 1.0],
                    screen_width,
                    screen_height,
                );
            }

            // Slider track background
            let track_color = [0.25, 0.25, 0.3, 1.0];
            add_quad(
                &mut vertices,
                &mut indices,
                to_ndc(slider.x, slider.y),
                to_ndc(slider.x + slider.width, slider.y),
                to_ndc(slider.x + slider.width, slider.y + slider.height),
                to_ndc(slider.x, slider.y + slider.height),
                track_color,
            );

            // Value fill
            let fill_width = slider.width * slider.value;
            add_quad(
                &mut vertices,
                &mut indices,
                to_ndc(slider.x, slider.y),
                to_ndc(slider.x + fill_width, slider.y),
                to_ndc(slider.x + fill_width, slider.y + slider.height),
                to_ndc(slider.x, slider.y + slider.height),
                slider.color,
            );

            // Handle indicator
            let handle_x = slider.x + fill_width - 4.0;
            let handle_color = [1.0, 1.0, 1.0, 1.0];
            add_quad(
                &mut vertices,
                &mut indices,
                to_ndc(handle_x, slider.y - 2.0),
                to_ndc(handle_x + 8.0, slider.y - 2.0),
                to_ndc(handle_x + 8.0, slider.y + slider.height + 2.0),
                to_ndc(handle_x, slider.y + slider.height + 2.0),
                handle_color,
            );
        }

        Mesh { vertices, indices }
    }

    /// Handle mouse press. Returns true if the panel consumed the event.
    pub fn on_mouse_press(&mut self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }
        for (i, slider) in self.sliders.iter_mut().enumerate() {
            if slider.contains(x, y) {
                self.dragging_slider = i as i32;
                slider.value = slider.value_from_x(x);
                return true;
            }
        }
        false
    }

    /// Handle mouse release. Returns true if a slider was being dragged.
    pub fn on_mouse_release(&mut self) -> bool {
        let was_dragging = self.dragging_slider >= 0;
        self.dragging_slider = -1;
        was_dragging
    }

    /// Handle mouse move (drag). Updates the active slider value.
    pub fn on_mouse_move(&mut self, x: f32) {
        if self.dragging_slider >= 0 {
            let idx = self.dragging_slider as usize;
            if idx < self.sliders.len() {
                self.sliders[idx].value = self.sliders[idx].value_from_x(x);
            }
        }
    }
}

// ============================================================================
// HSV COLOR PICKER
// ============================================================================

/// Convert HSV color values to RGBA.
///
/// - `h`: hue in degrees (0.0..360.0)
/// - `s`: saturation (0.0..1.0)
/// - `v`: value/brightness (0.0..1.0)
///
/// Returns `[r, g, b, a]` with each component in 0.0..1.0, alpha = 1.0.
pub fn hsv_to_rgba(h: f32, s: f32, v: f32) -> [f32; 4] {
    let h = h % 360.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [r1 + m, g1 + m, b1 + m, 1.0]
}

/// HSV Color Picker for Stage 4 (Color).
///
/// Provides:
/// - A 128x128 SV (saturation-value) box rendered as a 16x16 grid of colored quads
/// - A vertical hue bar (128 tall, 20 wide) showing the rainbow spectrum
/// - Cursor indicators for current selection
/// - 8 recent color swatches below the SV box
pub struct HsvColorPicker {
    /// Whether the picker is visible (only in Stage 4)
    pub visible: bool,
    /// SV box top-left X position
    pub sv_box_x: f32,
    /// SV box top-left Y position
    pub sv_box_y: f32,
    /// SV box size (width and height, square)
    pub sv_box_size: f32,
    /// Hue bar left X position (to the right of SV box)
    pub hue_bar_x: f32,
    /// Hue bar width
    pub hue_bar_width: f32,
    /// Current hue (0.0..360.0)
    pub hue: f32,
    /// Current saturation (0.0..1.0)
    pub saturation: f32,
    /// Current value/brightness (0.0..1.0)
    pub value: f32,
    /// Recently used colors (up to 8)
    pub recent_colors: Vec<[f32; 4]>,
    /// Whether the user is dragging in the SV box
    pub dragging_sv: bool,
    /// Whether the user is dragging the hue bar
    pub dragging_hue: bool,
}

impl Default for HsvColorPicker {
    fn default() -> Self {
        Self {
            visible: false,
            sv_box_x: 0.0,
            sv_box_y: 0.0,
            sv_box_size: 128.0,
            hue_bar_x: 0.0,
            hue_bar_width: 20.0,
            hue: 0.0,
            saturation: 1.0,
            value: 1.0,
            recent_colors: vec![
                [1.0, 0.0, 0.0, 1.0],
                [0.0, 1.0, 0.0, 1.0],
                [0.0, 0.0, 1.0, 1.0],
                [1.0, 1.0, 0.0, 1.0],
                [1.0, 0.0, 1.0, 1.0],
                [0.0, 1.0, 1.0, 1.0],
                [1.0, 1.0, 1.0, 1.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            dragging_sv: false,
            dragging_hue: false,
        }
    }
}

impl HsvColorPicker {
    /// Create a new HSV color picker positioned relative to the right panel.
    pub fn new(panel_x: f32, panel_y: f32) -> Self {
        let sv_box_x = panel_x + 10.0;
        let sv_box_y = panel_y;
        Self {
            sv_box_x,
            sv_box_y,
            hue_bar_x: sv_box_x + 128.0 + 10.0,
            ..Self::default()
        }
    }

    /// Update positions when the parent panel moves (e.g., on resize).
    pub fn set_position(&mut self, panel_x: f32, panel_y: f32) {
        self.sv_box_x = panel_x + 10.0;
        self.sv_box_y = panel_y;
        self.hue_bar_x = self.sv_box_x + self.sv_box_size + 10.0;
    }

    /// Get the currently selected color as RGBA.
    pub fn current_color(&self) -> [f32; 4] {
        hsv_to_rgba(self.hue, self.saturation, self.value)
    }

    /// Push the current color onto the recent colors list.
    /// Keeps at most 8 entries, removing the oldest.
    pub fn push_recent_color(&mut self) {
        let color = self.current_color();
        // Avoid duplicating the same color at the front
        if let Some(first) = self.recent_colors.first() {
            if (first[0] - color[0]).abs() < 0.01
                && (first[1] - color[1]).abs() < 0.01
                && (first[2] - color[2]).abs() < 0.01
            {
                return;
            }
        }
        self.recent_colors.insert(0, color);
        if self.recent_colors.len() > 8 {
            self.recent_colors.pop();
        }
    }

    /// Render the HSV color picker as a mesh overlay.
    ///
    /// Draws:
    /// 1. SV box (128x128) as a 16x16 grid of colored quads
    /// 2. Hue bar (128 tall x 20 wide) as a vertical rainbow
    /// 3. Cursor indicators on SV box and hue bar
    /// 4. Current color preview swatch
    /// 5. 8 recent color swatches
    pub fn render(&self, screen_width: f32, screen_height: f32) -> Mesh {
        let mut vertices: Vec<Vertex> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();

        if !self.visible {
            return Mesh { vertices, indices };
        }

        let to_ndc = |px: f32, py: f32| -> [f32; 3] {
            [
                (px / screen_width) * 2.0 - 1.0,
                1.0 - (py / screen_height) * 2.0,
                0.0,
            ]
        };

        // Title
        draw_text(
            &mut vertices,
            &mut indices,
            "HSV PICKER",
            self.sv_box_x,
            self.sv_box_y - 22.0,
            2.0,
            [1.0, 0.9, 0.5, 1.0],
            screen_width,
            screen_height,
        );

        // ---- SV Box (16x16 grid) ----
        let grid_steps: u32 = 16;
        let step = self.sv_box_size / grid_steps as f32;

        for sy in 0..grid_steps {
            for sx in 0..grid_steps {
                let s = sx as f32 / (grid_steps - 1) as f32;
                let v = 1.0 - (sy as f32 / (grid_steps - 1) as f32);
                let color = hsv_to_rgba(self.hue, s, v);

                let qx = self.sv_box_x + sx as f32 * step;
                let qy = self.sv_box_y + sy as f32 * step;

                add_quad(
                    &mut vertices,
                    &mut indices,
                    to_ndc(qx, qy),
                    to_ndc(qx + step, qy),
                    to_ndc(qx + step, qy + step),
                    to_ndc(qx, qy + step),
                    color,
                );
            }
        }

        // SV cursor indicator (small white-bordered quad at current S,V)
        let cursor_x = self.sv_box_x + self.saturation * self.sv_box_size;
        let cursor_y = self.sv_box_y + (1.0 - self.value) * self.sv_box_size;
        let cursor_size = 6.0;

        // Outer ring (white)
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(cursor_x - cursor_size, cursor_y - cursor_size),
            to_ndc(cursor_x + cursor_size, cursor_y - cursor_size),
            to_ndc(cursor_x + cursor_size, cursor_y + cursor_size),
            to_ndc(cursor_x - cursor_size, cursor_y + cursor_size),
            [1.0, 1.0, 1.0, 1.0],
        );
        // Inner ring (black)
        let inner = cursor_size - 2.0;
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(cursor_x - inner, cursor_y - inner),
            to_ndc(cursor_x + inner, cursor_y - inner),
            to_ndc(cursor_x + inner, cursor_y + inner),
            to_ndc(cursor_x - inner, cursor_y + inner),
            [0.0, 0.0, 0.0, 1.0],
        );

        // ---- Hue Bar ----
        let hue_steps: u32 = 24;
        let hue_step_h = self.sv_box_size / hue_steps as f32;

        for i in 0..hue_steps {
            let h = (i as f32 / hue_steps as f32) * 360.0;
            let color = hsv_to_rgba(h, 1.0, 1.0);

            let bar_y = self.sv_box_y + i as f32 * hue_step_h;
            add_quad(
                &mut vertices,
                &mut indices,
                to_ndc(self.hue_bar_x, bar_y),
                to_ndc(self.hue_bar_x + self.hue_bar_width, bar_y),
                to_ndc(
                    self.hue_bar_x + self.hue_bar_width,
                    bar_y + hue_step_h,
                ),
                to_ndc(self.hue_bar_x, bar_y + hue_step_h),
                color,
            );
        }

        // Hue bar cursor (horizontal line at current hue)
        let hue_cursor_y =
            self.sv_box_y + (self.hue / 360.0) * self.sv_box_size;
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(self.hue_bar_x - 2.0, hue_cursor_y - 2.0),
            to_ndc(
                self.hue_bar_x + self.hue_bar_width + 2.0,
                hue_cursor_y - 2.0,
            ),
            to_ndc(
                self.hue_bar_x + self.hue_bar_width + 2.0,
                hue_cursor_y + 2.0,
            ),
            to_ndc(self.hue_bar_x - 2.0, hue_cursor_y + 2.0),
            [1.0, 1.0, 1.0, 1.0],
        );

        // ---- Current Color Preview ----
        let preview_y = self.sv_box_y + self.sv_box_size + 10.0;
        let preview_size = 32.0;
        let current = self.current_color();

        draw_text(
            &mut vertices,
            &mut indices,
            "COLOR",
            self.sv_box_x,
            preview_y - 2.0,
            1.5,
            [0.8, 0.8, 0.8, 1.0],
            screen_width,
            screen_height,
        );

        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(self.sv_box_x, preview_y + 12.0),
            to_ndc(self.sv_box_x + preview_size, preview_y + 12.0),
            to_ndc(
                self.sv_box_x + preview_size,
                preview_y + 12.0 + preview_size,
            ),
            to_ndc(self.sv_box_x, preview_y + 12.0 + preview_size),
            current,
        );

        // ---- Recent Colors (8 swatches) ----
        let swatches_y = preview_y + 12.0 + preview_size + 10.0;
        let swatch_size = 16.0;
        let swatch_spacing = 2.0;

        draw_text(
            &mut vertices,
            &mut indices,
            "RECENT",
            self.sv_box_x,
            swatches_y - 2.0,
            1.5,
            [0.8, 0.8, 0.8, 1.0],
            screen_width,
            screen_height,
        );

        for (i, color) in self.recent_colors.iter().enumerate() {
            let sx = self.sv_box_x
                + i as f32 * (swatch_size + swatch_spacing);
            let sy = swatches_y + 12.0;

            add_quad(
                &mut vertices,
                &mut indices,
                to_ndc(sx, sy),
                to_ndc(sx + swatch_size, sy),
                to_ndc(sx + swatch_size, sy + swatch_size),
                to_ndc(sx, sy + swatch_size),
                *color,
            );
        }

        Mesh { vertices, indices }
    }

    /// Handle mouse input for the color picker.
    ///
    /// - `mouse_x`, `mouse_y`: current mouse position in screen pixels
    /// - `pressed`: whether the mouse button is currently pressed
    ///
    /// Returns `Some(color)` if the selected color changed, `None` otherwise.
    pub fn handle_mouse(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        pressed: bool,
    ) -> Option<[f32; 4]> {
        if !self.visible {
            return None;
        }

        // On release, stop dragging
        if !pressed {
            let was_dragging = self.dragging_sv || self.dragging_hue;
            self.dragging_sv = false;
            self.dragging_hue = false;
            if was_dragging {
                return Some(self.current_color());
            }
            return None;
        }

        // Check if click/drag is in the SV box
        if self.dragging_sv
            || (mouse_x >= self.sv_box_x
                && mouse_x <= self.sv_box_x + self.sv_box_size
                && mouse_y >= self.sv_box_y
                && mouse_y <= self.sv_box_y + self.sv_box_size)
        {
            self.dragging_sv = true;
            self.dragging_hue = false;
            self.saturation = ((mouse_x - self.sv_box_x) / self.sv_box_size)
                .clamp(0.0, 1.0);
            self.value = (1.0
                - (mouse_y - self.sv_box_y) / self.sv_box_size)
                .clamp(0.0, 1.0);
            return Some(self.current_color());
        }

        // Check if click/drag is in the hue bar
        if self.dragging_hue
            || (mouse_x >= self.hue_bar_x
                && mouse_x <= self.hue_bar_x + self.hue_bar_width
                && mouse_y >= self.sv_box_y
                && mouse_y <= self.sv_box_y + self.sv_box_size)
        {
            self.dragging_hue = true;
            self.dragging_sv = false;
            self.hue = (((mouse_y - self.sv_box_y) / self.sv_box_size)
                .clamp(0.0, 1.0))
                * 360.0;
            return Some(self.current_color());
        }

        // Check if click is on a recent color swatch
        let swatches_y = self.sv_box_y
            + self.sv_box_size
            + 10.0
            + 12.0
            + 32.0
            + 10.0
            + 12.0;
        let swatch_size = 16.0;
        let swatch_spacing = 2.0;

        for (i, color) in self.recent_colors.iter().enumerate() {
            let sx =
                self.sv_box_x + i as f32 * (swatch_size + swatch_spacing);
            let sy = swatches_y;

            if mouse_x >= sx
                && mouse_x <= sx + swatch_size
                && mouse_y >= sy
                && mouse_y <= sy + swatch_size
            {
                // Set the picker to this color (approximate reverse HSV)
                let r = color[0];
                let g = color[1];
                let b = color[2];
                let (h, s, v) = rgba_to_hsv(r, g, b);
                self.hue = h;
                self.saturation = s;
                self.value = v;
                return Some(*color);
            }
        }

        None
    }
}

/// Convert RGB to HSV.
///
/// - `r`, `g`, `b` in 0.0..1.0
///
/// Returns `(hue, saturation, value)` where hue is in 0.0..360.0.
fn rgba_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;
    let s = if max > 0.0 { delta / max } else { 0.0 };

    let h = if delta < 1e-6 {
        0.0
    } else if (max - r).abs() < 1e-6 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 1e-6 {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };

    (h, s, v)
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hsv_to_rgba_red() {
        let color = hsv_to_rgba(0.0, 1.0, 1.0);
        assert!((color[0] - 1.0).abs() < 0.01);
        assert!(color[1].abs() < 0.01);
        assert!(color[2].abs() < 0.01);
        assert!((color[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hsv_to_rgba_green() {
        let color = hsv_to_rgba(120.0, 1.0, 1.0);
        assert!(color[0].abs() < 0.01);
        assert!((color[1] - 1.0).abs() < 0.01);
        assert!(color[2].abs() < 0.01);
    }

    #[test]
    fn test_hsv_to_rgba_blue() {
        let color = hsv_to_rgba(240.0, 1.0, 1.0);
        assert!(color[0].abs() < 0.01);
        assert!(color[1].abs() < 0.01);
        assert!((color[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hsv_to_rgba_white() {
        let color = hsv_to_rgba(0.0, 0.0, 1.0);
        assert!((color[0] - 1.0).abs() < 0.01);
        assert!((color[1] - 1.0).abs() < 0.01);
        assert!((color[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hsv_to_rgba_black() {
        let color = hsv_to_rgba(0.0, 0.0, 0.0);
        assert!(color[0].abs() < 0.01);
        assert!(color[1].abs() < 0.01);
        assert!(color[2].abs() < 0.01);
    }

    #[test]
    fn test_rgba_to_hsv_roundtrip() {
        let (h, s, v) = rgba_to_hsv(1.0, 0.0, 0.0);
        let color = hsv_to_rgba(h, s, v);
        assert!((color[0] - 1.0).abs() < 0.01);
        assert!(color[1].abs() < 0.01);
        assert!(color[2].abs() < 0.01);
    }

    #[test]
    fn test_tool_palette_tools_per_stage() {
        let palette = ToolPalette::new();
        assert_eq!(palette.tools_for_stage(&EditorStage::Draw2D).len(), 4);
        assert_eq!(palette.tools_for_stage(&EditorStage::Extrude).len(), 3);
        assert_eq!(palette.tools_for_stage(&EditorStage::Sculpt).len(), 4);
        assert_eq!(palette.tools_for_stage(&EditorStage::Color).len(), 4);
        assert_eq!(palette.tools_for_stage(&EditorStage::Save).len(), 3);
    }

    #[test]
    fn test_tool_palette_handle_click() {
        let mut palette = ToolPalette::new();
        // Click on the first button (at panel_x, panel_y)
        let result = palette.handle_click(
            palette.panel_x + 20.0,
            palette.panel_y + 20.0,
            &EditorStage::Draw2D,
        );
        assert_eq!(result, Some(0));
        assert_eq!(palette.selected_tool, 0);

        // Click on the second button
        let btn_y = palette.panel_y + palette.button_size + palette.button_spacing + 20.0;
        let result = palette.handle_click(
            palette.panel_x + 20.0,
            btn_y,
            &EditorStage::Draw2D,
        );
        assert_eq!(result, Some(1));
        assert_eq!(palette.selected_tool, 1);
    }

    #[test]
    fn test_property_panel_rebuild() {
        let mut panel = PropertyPanel::default();
        panel.panel_x = 800.0;

        panel.rebuild_for_stage(&EditorStage::Draw2D);
        assert_eq!(panel.sliders.len(), 2);
        assert_eq!(panel.stage_title, "DRAW 2D");

        panel.rebuild_for_stage(&EditorStage::Extrude);
        assert_eq!(panel.sliders.len(), 3);
        assert_eq!(panel.stage_title, "EXTRUDE");

        panel.rebuild_for_stage(&EditorStage::Sculpt);
        assert_eq!(panel.sliders.len(), 3);
        assert_eq!(panel.stage_title, "SCULPT");

        panel.rebuild_for_stage(&EditorStage::Color);
        assert_eq!(panel.sliders.len(), 3);
        assert_eq!(panel.stage_title, "COLOR");

        panel.rebuild_for_stage(&EditorStage::Save);
        assert_eq!(panel.sliders.len(), 1);
        assert_eq!(panel.stage_title, "SAVE");
    }

    #[test]
    fn test_color_picker_default() {
        let picker = HsvColorPicker::default();
        assert!(!picker.visible);
        assert_eq!(picker.recent_colors.len(), 8);
        assert_eq!(picker.sv_box_size, 128.0);
    }

    #[test]
    fn test_color_picker_push_recent() {
        let mut picker = HsvColorPicker::default();
        picker.hue = 180.0;
        picker.saturation = 0.5;
        picker.value = 0.8;
        picker.push_recent_color();
        // Should have inserted at front
        assert_eq!(picker.recent_colors.len(), 8); // still 8, oldest popped
    }
}
