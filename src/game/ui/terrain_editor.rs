//! Terrain Editor UI
//!
//! UI panel for adjusting terrain generation parameters in real-time.

use super::slider::UISlider;
use super::text::{add_quad, draw_text};
use crate::game::terrain::{TerrainParams, get_terrain_params, set_terrain_params};
use crate::game::types::Mesh;

/// The terrain editor UI panel
pub struct TerrainEditorUI {
    /// Whether the UI is visible
    pub visible: bool,
    /// Panel position
    pub panel_x: f32,
    pub panel_y: f32,
    /// The sliders
    pub sliders: [UISlider; 6],
    /// Which slider is being dragged (-1 = none)
    pub dragging_slider: i32,
    /// Apply button bounds
    pub apply_button: (f32, f32, f32, f32), // x, y, w, h
}

impl Default for TerrainEditorUI {
    fn default() -> Self {
        let params = get_terrain_params();
        let panel_x = 20.0;
        let panel_y = 80.0;
        let spacing = 50.0; // More spacing to fit labels above sliders

        Self {
            visible: false,
            panel_x,
            panel_y,
            sliders: [
                UISlider::new(
                    "Height",
                    panel_x,
                    panel_y,
                    params.height_scale,
                    [0.4, 0.7, 1.0, 1.0],
                ),
                UISlider::new(
                    "Mountains",
                    panel_x,
                    panel_y + spacing,
                    params.mountains,
                    [0.7, 0.5, 0.3, 1.0],
                ),
                UISlider::new(
                    "Rocks",
                    panel_x,
                    panel_y + spacing * 2.0,
                    params.rocks,
                    [0.6, 0.6, 0.6, 1.0],
                ),
                UISlider::new(
                    "Hills",
                    panel_x,
                    panel_y + spacing * 3.0,
                    params.hills,
                    [0.5, 0.8, 0.4, 1.0],
                ),
                UISlider::new(
                    "Detail",
                    panel_x,
                    panel_y + spacing * 4.0,
                    params.detail,
                    [0.8, 0.7, 0.3, 1.0],
                ),
                UISlider::new(
                    "Water",
                    panel_x,
                    panel_y + spacing * 5.0,
                    params.water,
                    [0.3, 0.6, 0.9, 1.0],
                ),
            ],
            dragging_slider: -1,
            apply_button: (panel_x, panel_y + spacing * 6.0 + 20.0, 200.0, 30.0),
        }
    }
}

impl TerrainEditorUI {
    /// Toggle visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            // Sync slider values with current terrain params
            let params = get_terrain_params();
            self.sliders[0].value = params.height_scale;
            self.sliders[1].value = params.mountains;
            self.sliders[2].value = params.rocks;
            self.sliders[3].value = params.hills;
            self.sliders[4].value = params.detail;
            self.sliders[5].value = params.water;
        }
    }

    /// Handle mouse press, returns true if UI consumed the event
    pub fn on_mouse_press(&mut self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }

        // Check sliders
        for (i, slider) in self.sliders.iter_mut().enumerate() {
            if slider.contains(x, y) {
                self.dragging_slider = i as i32;
                slider.value = slider.value_from_x(x);
                return true;
            }
        }

        // Check apply button
        let (bx, by, bw, bh) = self.apply_button;
        if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
            return true; // Will trigger apply in release
        }

        false
    }

    /// Handle mouse release, returns true if should rebuild terrain
    pub fn on_mouse_release(&mut self, x: f32, y: f32) -> bool {
        if !self.visible {
            return false;
        }

        let was_dragging = self.dragging_slider >= 0;
        self.dragging_slider = -1;

        // Check apply button
        let (bx, by, bw, bh) = self.apply_button;
        if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
            // Apply changes to terrain
            self.apply_to_terrain();
            return true; // Trigger rebuild
        }

        was_dragging
    }

    /// Handle mouse drag
    pub fn on_mouse_move(&mut self, x: f32, _y: f32) {
        if self.dragging_slider >= 0 && self.dragging_slider < 6 {
            let idx = self.dragging_slider as usize;
            self.sliders[idx].value = self.sliders[idx].value_from_x(x);
        }
    }

    /// Apply slider values to terrain params
    pub fn apply_to_terrain(&self) {
        let params = TerrainParams {
            height_scale: self.sliders[0].value,
            mountains: self.sliders[1].value,
            rocks: self.sliders[2].value,
            hills: self.sliders[3].value,
            detail: self.sliders[4].value,
            water: self.sliders[5].value,
        };
        set_terrain_params(params);
        println!(
            "Applied terrain settings: H={:.0}% M={:.0}% R={:.0}% L={:.0}% D={:.0}% W={:.0}%",
            params.height_scale * 100.0,
            params.mountains * 100.0,
            params.rocks * 100.0,
            params.hills * 100.0,
            params.detail * 100.0,
            params.water * 100.0
        );
    }

    /// Generate mesh for UI rendering (2D quads)
    pub fn generate_ui_mesh(&self, screen_width: f32, screen_height: f32) -> Mesh {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        if !self.visible {
            return Mesh { vertices, indices };
        }

        // Helper to convert screen coords to NDC
        let to_ndc = |x: f32, y: f32| -> [f32; 3] {
            [
                (x / screen_width) * 2.0 - 1.0,
                1.0 - (y / screen_height) * 2.0, // Y is flipped
                0.0,
            ]
        };

        // Slider labels (names)
        let labels = ["HEIGHT", "MOUNTAIN", "ROCKS", "HILLS", "DETAIL", "WATER"];
        let text_color = [0.9, 0.9, 0.9, 1.0]; // Light gray text
        let title_color = [1.0, 0.9, 0.5, 1.0]; // Gold title

        // Draw panel background (wider for labels)
        let panel_w = 280.0;
        let panel_h = 340.0;
        let bg_color = [0.12, 0.12, 0.18, 1.0]; // Dark panel
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(self.panel_x - 10.0, self.panel_y - 50.0),
            to_ndc(self.panel_x + panel_w, self.panel_y - 50.0),
            to_ndc(self.panel_x + panel_w, self.panel_y + panel_h),
            to_ndc(self.panel_x - 10.0, self.panel_y + panel_h),
            bg_color,
        );

        // Draw title "TERRAIN"
        draw_text(
            &mut vertices,
            &mut indices,
            "TERRAIN",
            self.panel_x + 80.0,
            self.panel_y - 35.0,
            2.5,
            title_color,
            screen_width,
            screen_height,
        );

        // Draw each slider with label
        for (i, slider) in self.sliders.iter().enumerate() {
            // Draw label above slider
            draw_text(
                &mut vertices,
                &mut indices,
                labels[i],
                slider.x,
                slider.y - 18.0,
                2.0,
                text_color,
                screen_width,
                screen_height,
            );

            // Background track
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

        // Draw apply button
        let (bx, by, bw, bh) = self.apply_button;
        let button_color = [0.3, 0.7, 0.4, 1.0];
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(bx, by),
            to_ndc(bx + bw, by),
            to_ndc(bx + bw, by + bh),
            to_ndc(bx, by + bh),
            button_color,
        );

        // Draw "APPLY" text on button
        draw_text(
            &mut vertices,
            &mut indices,
            "APPLY",
            bx + 70.0,
            by + 8.0,
            2.0,
            [1.0, 1.0, 1.0, 1.0],
            screen_width,
            screen_height,
        );

        Mesh { vertices, indices }
    }
}
