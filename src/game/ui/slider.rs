//! UI Slider
//!
//! A simple slider widget for terrain editing.

/// A single UI slider for terrain editing
#[derive(Clone, Copy)]
pub struct UISlider {
    /// Label for this slider
    pub label: &'static str,
    /// Screen position (pixels from top-left)
    pub x: f32,
    pub y: f32,
    /// Slider dimensions
    pub width: f32,
    pub height: f32,
    /// Current value (0.0 to 1.0)
    pub value: f32,
    /// Color of the slider bar
    pub color: [f32; 4],
}

impl UISlider {
    pub fn new(label: &'static str, x: f32, y: f32, value: f32, color: [f32; 4]) -> Self {
        Self {
            label,
            x,
            y,
            width: 200.0,
            height: 24.0,
            value,
            color,
        }
    }
    
    /// Check if a point is within this slider
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width &&
        py >= self.y && py <= self.y + self.height
    }
    
    /// Get value from mouse X position within slider
    pub fn value_from_x(&self, px: f32) -> f32 {
        ((px - self.x) / self.width).clamp(0.0, 1.0)
    }
}
