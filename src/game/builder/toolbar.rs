//! Build Toolbar
//!
//! Minecraft-style hotbar for selecting building block shapes.

use glam::Vec3;
use crate::render::BuildingBlockShape;
use super::tools::{BridgeTool, SHAPE_NAMES, BLOCK_GRID_SIZE};

/// Build toolbar for selecting building block shapes
pub struct BuildToolbar {
    /// Whether the toolbar is visible
    pub visible: bool,
    /// Currently selected shape index (0-6, includes Bridge)
    pub selected_shape: usize,
    /// Available shapes (Bridge is special - handled differently)
    pub shapes: [BuildingBlockShape; 6],
    /// Currently selected material (0-9)
    pub selected_material: u8,
    /// Current build height level (adjusted with scroll/middle mouse)
    pub build_height: f32,
    /// Preview position (where block will be placed)
    pub preview_position: Option<Vec3>,
    /// Whether to show the preview
    pub show_preview: bool,
    /// Bridge tool state (for shape 7)
    pub bridge_tool: BridgeTool,
    /// Time since last physics support check
    pub physics_check_timer: f32,
}

impl Default for BuildToolbar {
    fn default() -> Self {
        Self {
            visible: false,
            selected_shape: 0,
            shapes: [
                BuildingBlockShape::Cube { half_extents: Vec3::splat(0.5) },
                BuildingBlockShape::Cylinder { radius: 0.5, height: 1.0 },
                BuildingBlockShape::Sphere { radius: 0.5 },
                BuildingBlockShape::Dome { radius: 0.5 },
                BuildingBlockShape::Arch { width: 1.0, height: 1.5, depth: 0.3 },
                BuildingBlockShape::Wedge { size: Vec3::ONE },
            ],
            selected_material: 0,
            build_height: 0.0,
            preview_position: None,
            show_preview: true,
            bridge_tool: BridgeTool::default(),
            physics_check_timer: 0.0,
        }
    }
}

impl BuildToolbar {
    /// Toggle visibility
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
        if self.visible {
            println!("=== BUILD TOOLBAR ===");
            println!("Tab/Up/Down: Change shape | 1-7: Select shape");
            println!("Scroll: Adjust height | Middle-click: Material");
            println!("Left-click: Place block | Double-click: Merge");
            println!("Shape 7 (Bridge): Click 2 faces to connect them!");
        }
        // Clear bridge selection when closing
        if !self.visible {
            self.bridge_tool.clear();
        }
    }
    
    /// Check if bridge tool is selected
    pub fn is_bridge_mode(&self) -> bool {
        self.selected_shape == 6
    }
    
    /// Cycle to next shape
    pub fn next_shape(&mut self) {
        self.selected_shape = (self.selected_shape + 1) % 7;
        self.on_shape_changed();
    }
    
    /// Cycle to previous shape
    pub fn prev_shape(&mut self) {
        self.selected_shape = if self.selected_shape == 0 { 6 } else { self.selected_shape - 1 };
        self.on_shape_changed();
    }
    
    /// Select a specific shape by index (0-6)
    pub fn select_shape(&mut self, index: usize) {
        if index < 7 {
            self.selected_shape = index;
            self.on_shape_changed();
        }
    }
    
    /// Called when shape changes
    fn on_shape_changed(&mut self) {
        println!("[BuildToolbar] Selected: {}", SHAPE_NAMES[self.selected_shape]);
        if self.is_bridge_mode() {
            self.bridge_tool.selecting = true;
            self.bridge_tool.clear();
            println!("[Bridge Mode] Click on block faces to select them");
        } else {
            self.bridge_tool.selecting = false;
        }
    }
    
    /// Get the currently selected shape (returns Cube for Bridge mode)
    pub fn get_selected_shape(&self) -> BuildingBlockShape {
        if self.selected_shape < 6 {
            self.shapes[self.selected_shape]
        } else {
            // Bridge mode - no direct shape
            BuildingBlockShape::Cube { half_extents: Vec3::splat(0.5) }
        }
    }
    
    /// Adjust build height
    pub fn adjust_height(&mut self, delta: f32) {
        self.build_height += delta * BLOCK_GRID_SIZE;
        println!("[BuildToolbar] Height: {:.1}", self.build_height);
    }
    
    /// Reset build height to 0
    pub fn reset_height(&mut self) {
        self.build_height = 0.0;
        println!("[BuildToolbar] Height reset to 0");
    }
    
    /// Change material
    pub fn next_material(&mut self) {
        self.selected_material = (self.selected_material + 1) % 10;
        println!("[BuildToolbar] Material: {}", self.selected_material);
    }
    
    pub fn prev_material(&mut self) {
        self.selected_material = if self.selected_material == 0 { 9 } else { self.selected_material - 1 };
        println!("[BuildToolbar] Material: {}", self.selected_material);
    }
}
