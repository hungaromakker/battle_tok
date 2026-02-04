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

    /// Generate UI mesh for the toolbar (Minecraft-style hotbar)
    pub fn generate_ui_mesh(&self, screen_width: f32, screen_height: f32) -> crate::game::types::Mesh {
        use crate::game::types::Mesh;
        use crate::game::ui::{add_quad, draw_text};
        
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
        
        // Toolbar dimensions (7 slots now)
        let slot_size = 44.0;
        let slot_spacing = 6.0;
        let toolbar_width = 7.0 * slot_size + 6.0 * slot_spacing + 20.0;
        let toolbar_height = slot_size + 40.0;
        let toolbar_x = (screen_width - toolbar_width) / 2.0;
        let toolbar_y = screen_height - toolbar_height - 20.0;
        
        // Draw toolbar background
        let bg_color = [0.1, 0.1, 0.15, 0.9];
        add_quad(&mut vertices, &mut indices,
            to_ndc(toolbar_x, toolbar_y),
            to_ndc(toolbar_x + toolbar_width, toolbar_y),
            to_ndc(toolbar_x + toolbar_width, toolbar_y + toolbar_height),
            to_ndc(toolbar_x, toolbar_y + toolbar_height),
            bg_color);
        
        // Draw each slot (7 shapes now)
        for i in 0..7 {
            let slot_x = toolbar_x + 10.0 + (i as f32) * (slot_size + slot_spacing);
            let slot_y = toolbar_y + 10.0;
            
            let slot_color = if i == self.selected_shape {
                [0.4, 0.6, 0.9, 1.0]
            } else {
                [0.2, 0.2, 0.25, 1.0]
            };
            
            add_quad(&mut vertices, &mut indices,
                to_ndc(slot_x, slot_y),
                to_ndc(slot_x + slot_size, slot_y),
                to_ndc(slot_x + slot_size, slot_y + slot_size),
                to_ndc(slot_x, slot_y + slot_size),
                slot_color);
            
            // Draw shape icons
            let icon_color = [0.9, 0.9, 0.9, 1.0];
            let center_x = slot_x + slot_size / 2.0;
            let center_y = slot_y + slot_size / 2.0;
            let icon_size = slot_size * 0.6;
            
            match i {
                0 => { // Cube
                    let half = icon_size / 2.0;
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - half, center_y - half),
                        to_ndc(center_x + half, center_y - half),
                        to_ndc(center_x + half, center_y + half),
                        to_ndc(center_x - half, center_y + half),
                        icon_color);
                }
                1 => { // Cylinder
                    let w = icon_size * 0.4;
                    let h = icon_size * 0.8;
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y + h/2.0),
                        to_ndc(center_x - w/2.0, center_y + h/2.0),
                        icon_color);
                }
                2 => { // Sphere
                    let r = icon_size * 0.4;
                    let segments = 8;
                    for j in 0..segments {
                        let a1 = (j as f32 / segments as f32) * std::f32::consts::TAU;
                        let a2 = ((j + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                        add_quad(&mut vertices, &mut indices,
                            to_ndc(center_x, center_y),
                            to_ndc(center_x + a1.cos() * r, center_y + a1.sin() * r),
                            to_ndc(center_x + a2.cos() * r, center_y + a2.sin() * r),
                            to_ndc(center_x, center_y),
                            icon_color);
                    }
                }
                3 => { // Dome
                    let r = icon_size * 0.4;
                    let segments = 6;
                    for j in 0..segments {
                        let a1 = (j as f32 / segments as f32) * std::f32::consts::PI;
                        let a2 = ((j + 1) as f32 / segments as f32) * std::f32::consts::PI;
                        add_quad(&mut vertices, &mut indices,
                            to_ndc(center_x, center_y + r * 0.3),
                            to_ndc(center_x + a1.cos() * r, center_y + r * 0.3 - a1.sin() * r),
                            to_ndc(center_x + a2.cos() * r, center_y + r * 0.3 - a2.sin() * r),
                            to_ndc(center_x, center_y + r * 0.3),
                            icon_color);
                    }
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - icon_size * 0.4, center_y + r * 0.3),
                        to_ndc(center_x + icon_size * 0.4, center_y + r * 0.3),
                        to_ndc(center_x + icon_size * 0.4, center_y + r * 0.3 + 3.0),
                        to_ndc(center_x - icon_size * 0.4, center_y + r * 0.3 + 3.0),
                        icon_color);
                }
                4 => { // Arch
                    let w = icon_size * 0.7;
                    let h = icon_size * 0.8;
                    let thickness = icon_size * 0.15;
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - w/2.0, center_y - h/2.0),
                        to_ndc(center_x - w/2.0 + thickness, center_y - h/2.0),
                        to_ndc(center_x - w/2.0 + thickness, center_y + h/2.0),
                        to_ndc(center_x - w/2.0, center_y + h/2.0),
                        icon_color);
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x + w/2.0 - thickness, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y + h/2.0),
                        to_ndc(center_x + w/2.0 - thickness, center_y + h/2.0),
                        icon_color);
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y - h/2.0 + thickness),
                        to_ndc(center_x - w/2.0, center_y - h/2.0 + thickness),
                        icon_color);
                }
                5 => { // Wedge
                    let half = icon_size * 0.4;
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - half, center_y + half),
                        to_ndc(center_x + half, center_y + half),
                        to_ndc(center_x, center_y - half),
                        to_ndc(center_x - half, center_y + half),
                        icon_color);
                }
                6 => { // Bridge
                    let s = icon_size * 0.25;
                    let gap = icon_size * 0.3;
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - gap - s, center_y - s),
                        to_ndc(center_x - gap, center_y - s),
                        to_ndc(center_x - gap, center_y + s),
                        to_ndc(center_x - gap - s, center_y + s),
                        icon_color);
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x + gap, center_y - s),
                        to_ndc(center_x + gap + s, center_y - s),
                        to_ndc(center_x + gap + s, center_y + s),
                        to_ndc(center_x + gap, center_y + s),
                        icon_color);
                    let line_h = s * 0.4;
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - gap, center_y - line_h),
                        to_ndc(center_x + gap, center_y - line_h),
                        to_ndc(center_x + gap, center_y + line_h),
                        to_ndc(center_x - gap, center_y + line_h),
                        icon_color);
                }
                _ => {}
            }
            
            // Draw number label
            let num_str = format!("{}", i + 1);
            draw_text(&mut vertices, &mut indices, &num_str,
                slot_x + slot_size / 2.0 - 3.0, slot_y + slot_size + 5.0,
                1.5, [0.7, 0.7, 0.7, 1.0], screen_width, screen_height);
        }
        
        // Draw info panel
        let info_x = toolbar_x + toolbar_width + 15.0;
        
        let mat_text = format!("MAT {}", self.selected_material);
        draw_text(&mut vertices, &mut indices, &mat_text,
            info_x, toolbar_y + 10.0,
            1.5, [0.8, 0.8, 0.5, 1.0], screen_width, screen_height);
        
        let height_text = format!("H {:.0}", self.build_height);
        draw_text(&mut vertices, &mut indices, &height_text,
            info_x, toolbar_y + 28.0,
            1.5, [0.5, 0.8, 0.5, 1.0], screen_width, screen_height);
        
        if self.is_bridge_mode() {
            let bridge_text = if self.bridge_tool.first_face.is_some() {
                if self.bridge_tool.second_face.is_some() { "READY" } else { "FACE 1" }
            } else {
                "SELECT"
            };
            draw_text(&mut vertices, &mut indices, bridge_text,
                info_x, toolbar_y + 46.0,
                1.5, [1.0, 0.6, 0.2, 1.0], screen_width, screen_height);
        }
        
        Mesh { vertices, indices }
    }
}
