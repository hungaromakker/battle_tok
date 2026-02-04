//! Start Overlay
//!
//! Initial overlay screen prompting user to click to start.

use crate::game::types::Mesh;
use super::text::{add_quad, draw_text};

/// Startup overlay that grabs focus on first click
pub struct StartOverlay {
    /// Whether the overlay is visible (true until first interaction)
    pub visible: bool,
}

impl Default for StartOverlay {
    fn default() -> Self {
        // Start visible on all platforms to capture cursor focus
        Self {
            visible: true,
        }
    }
}

impl StartOverlay {
    /// Generate UI mesh for the start overlay
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
                1.0 - (y / screen_height) * 2.0,
                0.0
            ]
        };
        
        // Semi-transparent overlay
        let overlay_color = [0.0, 0.0, 0.0, 0.7];
        add_quad(&mut vertices, &mut indices,
            to_ndc(0.0, 0.0),
            to_ndc(screen_width, 0.0),
            to_ndc(screen_width, screen_height),
            to_ndc(0.0, screen_height),
            overlay_color);
        
        // "CLICK TO START" text in center
        let text = "CLICK TO START";
        let text_width = text.len() as f32 * 6.0 * 3.0; // Approximate width
        draw_text(&mut vertices, &mut indices, text,
            (screen_width - text_width) / 2.0, screen_height / 2.0 - 20.0,
            3.0, [1.0, 1.0, 1.0, 1.0], screen_width, screen_height);
        
        // Subtitle
        let subtitle = "WASD MOVE  SPACE JUMP  B BUILD  V CAMERA";
        let sub_width = subtitle.len() as f32 * 6.0 * 1.5;
        draw_text(&mut vertices, &mut indices, subtitle,
            (screen_width - sub_width) / 2.0, screen_height / 2.0 + 30.0,
            1.5, [0.7, 0.7, 0.7, 1.0], screen_width, screen_height);
        
        Mesh { vertices, indices }
    }
}
