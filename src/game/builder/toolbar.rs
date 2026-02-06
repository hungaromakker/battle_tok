//! Build Toolbar
//!
//! Minecraft-style hotbar for selecting building block shapes.
//! Also includes block inventory for pickup/stash system.

use super::tools::{BLOCK_GRID_SIZE, BridgeTool, SHAPE_NAMES};
use crate::render::BuildingBlockShape;
use glam::{IVec3, Vec3};

/// Forts-style quick-build structure presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickStructurePreset {
    Stairs,
    WallShort,
    WallTall,
    TowerCore,
    Gate,
    Rampart,
    FiringNest,
}

pub const QUICK_STRUCTURE_NAMES: [&str; 7] = [
    "Stairs",
    "Window Wall",
    "Window Wall Tall",
    "Tower Core",
    "Gatehouse",
    "Loophole Rampart",
    "Gun Emplacement",
];

/// A stashed block that can be placed later
#[derive(Debug, Clone, Copy)]
pub struct StashedBlock {
    /// The shape of the stashed block
    pub shape: BuildingBlockShape,
    /// Material index (0-9)
    pub material: u8,
}

/// Inventory for picked up blocks
#[derive(Debug, Clone)]
pub struct BlockInventory {
    /// Stashed blocks ready to be placed
    pub stashed_blocks: Vec<StashedBlock>,
    /// Maximum capacity
    pub max_capacity: usize,
}

impl Default for BlockInventory {
    fn default() -> Self {
        Self {
            stashed_blocks: Vec::new(),
            max_capacity: 10,
        }
    }
}

impl BlockInventory {
    /// Create a new inventory with specified capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            stashed_blocks: Vec::with_capacity(capacity),
            max_capacity: capacity,
        }
    }

    /// Stash a block into the inventory
    /// Returns true if successful, false if inventory is full
    pub fn stash(&mut self, shape: BuildingBlockShape, material: u8) -> bool {
        if self.stashed_blocks.len() >= self.max_capacity {
            return false;
        }
        self.stashed_blocks.push(StashedBlock { shape, material });
        true
    }

    /// Take a block from the inventory (LIFO - takes most recently stashed)
    pub fn take(&mut self) -> Option<StashedBlock> {
        self.stashed_blocks.pop()
    }

    /// Peek at the next block that would be taken without removing it
    pub fn peek(&self) -> Option<&StashedBlock> {
        self.stashed_blocks.last()
    }

    /// Get current count of stashed blocks
    pub fn count(&self) -> usize {
        self.stashed_blocks.len()
    }

    /// Check if inventory is empty
    pub fn is_empty(&self) -> bool {
        self.stashed_blocks.is_empty()
    }

    /// Check if inventory is full
    pub fn is_full(&self) -> bool {
        self.stashed_blocks.len() >= self.max_capacity
    }

    /// Clear all stashed blocks
    pub fn clear(&mut self) {
        self.stashed_blocks.clear();
    }
}

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
    /// Inventory for picked up blocks
    pub inventory: BlockInventory,
    /// Time the mouse has been held down (for pickup detection)
    pub mouse_hold_time: f32,
    /// Whether we're currently trying to pick up a block
    pub pickup_in_progress: bool,
    /// Quick-build mode places structure templates instead of single blocks.
    pub quick_mode: bool,
    /// Selected structure preset index (0-6)
    pub selected_structure: usize,
}

impl Default for BuildToolbar {
    fn default() -> Self {
        Self {
            visible: false,
            selected_shape: 0,
            shapes: [
                BuildingBlockShape::Cube {
                    half_extents: Vec3::splat(0.5),
                },
                BuildingBlockShape::Cylinder {
                    radius: 0.5,
                    height: 1.0,
                },
                BuildingBlockShape::Sphere { radius: 0.5 },
                BuildingBlockShape::Dome { radius: 0.5 },
                BuildingBlockShape::Arch {
                    width: 1.0,
                    height: 1.5,
                    depth: 0.3,
                },
                BuildingBlockShape::Wedge { size: Vec3::ONE },
            ],
            selected_material: 0,
            build_height: 0.0,
            preview_position: None,
            show_preview: true,
            bridge_tool: BridgeTool::default(),
            physics_check_timer: 0.0,
            inventory: BlockInventory::default(),
            mouse_hold_time: 0.0,
            pickup_in_progress: false,
            quick_mode: true,
            selected_structure: 0,
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
            println!("Q/M: Toggle quick-build structure mode");
            println!("Scroll: Adjust height | Middle-click: Material");
            println!("Left-click: Place block");
            println!("Shape 7 (Bridge): Click 2 faces to connect them!");
            if self.quick_mode {
                println!(
                    "[BuildToolbar] Quick build ACTIVE -> {} (press Q/M for primitive mode)",
                    self.quick_structure_name()
                );
            } else {
                println!(
                    "[BuildToolbar] Primitive mode ACTIVE -> {} (press Q/M for quick structures)",
                    SHAPE_NAMES[self.selected_shape]
                );
            }
        }
        // Clear bridge selection when closing
        if !self.visible {
            self.bridge_tool.clear();
        }
    }

    /// Check if bridge tool is selected
    pub fn is_bridge_mode(&self) -> bool {
        !self.quick_mode && self.selected_shape == 6
    }

    /// Cycle to next shape
    pub fn next_shape(&mut self) {
        self.selected_shape = (self.selected_shape + 1) % 7;
        self.on_shape_changed();
    }

    /// Cycle to previous shape
    pub fn prev_shape(&mut self) {
        self.selected_shape = if self.selected_shape == 0 {
            6
        } else {
            self.selected_shape - 1
        };
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
        if self.quick_mode {
            return;
        }
        println!(
            "[BuildToolbar] Selected: {}",
            SHAPE_NAMES[self.selected_shape]
        );
        if self.is_bridge_mode() {
            self.bridge_tool.selecting = true;
            self.bridge_tool.clear();
            println!("[Bridge Mode] Click on block faces to select them");
        } else {
            self.bridge_tool.selecting = false;
        }
    }

    /// Toggle quick-build structure mode.
    pub fn toggle_quick_mode(&mut self) {
        self.quick_mode = !self.quick_mode;
        self.bridge_tool.clear();
        self.bridge_tool.selecting = false;
        if self.quick_mode {
            println!(
                "[BuildToolbar] Quick build ON -> {}",
                self.quick_structure_name()
            );
        } else {
            println!("[BuildToolbar] Quick build OFF -> primitive mode");
            self.on_shape_changed();
        }
    }

    /// Cycle to the next quick structure preset.
    pub fn next_structure(&mut self) {
        self.selected_structure = (self.selected_structure + 1) % 7;
        if self.quick_mode {
            println!("[BuildToolbar] Structure: {}", self.quick_structure_name());
        }
    }

    /// Cycle to the previous quick structure preset.
    pub fn prev_structure(&mut self) {
        self.selected_structure = if self.selected_structure == 0 {
            6
        } else {
            self.selected_structure - 1
        };
        if self.quick_mode {
            println!("[BuildToolbar] Structure: {}", self.quick_structure_name());
        }
    }

    /// Select a specific structure preset by index (0-6).
    pub fn select_structure(&mut self, index: usize) {
        if index < 7 {
            self.selected_structure = index;
            if self.quick_mode {
                println!("[BuildToolbar] Structure: {}", self.quick_structure_name());
            }
        }
    }

    /// Name of the currently selected structure preset.
    pub fn quick_structure_name(&self) -> &'static str {
        QUICK_STRUCTURE_NAMES[self.selected_structure]
    }

    /// Grid-offset + shape layout for the selected structure preset.
    pub fn selected_structure_layout(&self) -> Vec<(IVec3, BuildingBlockShape)> {
        let mut out = Vec::new();

        let push_cube = |out: &mut Vec<(IVec3, BuildingBlockShape)>, offset: IVec3| {
            out.push((
                offset,
                BuildingBlockShape::Cube {
                    half_extents: Vec3::splat(0.5),
                },
            ));
        };

        match self.selected_structure {
            // Wide stairs (auto-step + climb friendly).
            0 => {
                for step in 0..6 {
                    for x in -1..=1 {
                        for y in 0..=step {
                            push_cube(&mut out, IVec3::new(x, y, step - 2));
                        }
                    }
                }
            }
            // Window wall (single center opening).
            1 => {
                for x in -2..=2 {
                    for y in 0..4 {
                        if x == 0 && y == 1 {
                            continue;
                        }
                        push_cube(&mut out, IVec3::new(x, y, 0));
                    }
                }
            }
            // Tall window wall (multiple shooting openings).
            2 => {
                for x in -3..=3 {
                    for y in 0..5 {
                        if (y == 1 || y == 2) && (x == -1 || x == 1) {
                            continue;
                        }
                        push_cube(&mut out, IVec3::new(x, y, 0));
                    }
                }
            }
            // Tower core (3x3 ring, 5 high) with one embrasure.
            3 => {
                for y in 0..5 {
                    for x in -1i32..=1 {
                        for z in -1i32..=1 {
                            if x == 0 && z == 1 && y == 2 {
                                continue;
                            }
                            if x.abs() == 1 || z.abs() == 1 {
                                push_cube(&mut out, IVec3::new(x, y, z));
                            }
                        }
                    }
                }
            }
            // Gatehouse: side columns + beam + arch opening.
            4 => {
                for y in 0..4 {
                    push_cube(&mut out, IVec3::new(-2, y, 0));
                    push_cube(&mut out, IVec3::new(2, y, 0));
                }
                for x in -2..=2 {
                    push_cube(&mut out, IVec3::new(x, 4, 0));
                }
                out.push((
                    IVec3::new(0, 0, 0),
                    BuildingBlockShape::Arch {
                        width: 4.0,
                        height: 3.0,
                        depth: 1.0,
                    },
                ));
            }
            // Loophole rampart (front wall with alternating firing slits).
            5 => {
                for x in -3..=3 {
                    for y in 0..3 {
                        if y == 1 && x % 2 != 0 {
                            continue;
                        }
                        push_cube(&mut out, IVec3::new(x, y, 0));
                    }
                }
            }
            // Gun emplacement: U-shaped cover + inner plinth + rear access step.
            6 => {
                for x in -2..=2 {
                    for z in -2..=2 {
                        push_cube(&mut out, IVec3::new(x, 0, z));
                    }
                }
                for x in -2..=2 {
                    for y in 1..=2 {
                        if x == 0 && y == 1 {
                            continue;
                        }
                        push_cube(&mut out, IVec3::new(x, y, 2));
                    }
                }
                for z in -1..=1 {
                    push_cube(&mut out, IVec3::new(-2, 1, z));
                    push_cube(&mut out, IVec3::new(2, 1, z));
                }
                push_cube(&mut out, IVec3::new(0, 1, 0));
                out.push((
                    IVec3::new(0, 2, 0),
                    BuildingBlockShape::Cylinder {
                        radius: 0.5,
                        height: 1.0,
                    },
                ));
                push_cube(&mut out, IVec3::new(0, 1, -3));
            }
            _ => {
                push_cube(&mut out, IVec3::ZERO);
            }
        }

        out
    }

    fn active_slot_index(&self) -> usize {
        if self.quick_mode {
            self.selected_structure
        } else {
            self.selected_shape
        }
    }

    /// Get the currently selected shape (returns Cube for Bridge mode)
    pub fn get_selected_shape(&self) -> BuildingBlockShape {
        if self.selected_shape < 6 {
            self.shapes[self.selected_shape]
        } else {
            // Bridge mode - no direct shape
            BuildingBlockShape::Cube {
                half_extents: Vec3::splat(0.5),
            }
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
        self.selected_material = if self.selected_material == 0 {
            9
        } else {
            self.selected_material - 1
        };
        println!("[BuildToolbar] Material: {}", self.selected_material);
    }

    /// Generate UI mesh for the toolbar (Minecraft-style hotbar)
    pub fn generate_ui_mesh(
        &self,
        screen_width: f32,
        screen_height: f32,
    ) -> crate::game::types::Mesh {
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
                0.0,
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
        add_quad(
            &mut vertices,
            &mut indices,
            to_ndc(toolbar_x, toolbar_y),
            to_ndc(toolbar_x + toolbar_width, toolbar_y),
            to_ndc(toolbar_x + toolbar_width, toolbar_y + toolbar_height),
            to_ndc(toolbar_x, toolbar_y + toolbar_height),
            bg_color,
        );

        // Draw each slot (shape slots or quick-structure slots)
        let active_slot = self.active_slot_index();
        for i in 0..7 {
            let slot_x = toolbar_x + 10.0 + (i as f32) * (slot_size + slot_spacing);
            let slot_y = toolbar_y + 10.0;

            let slot_color = if i == active_slot {
                [0.4, 0.6, 0.9, 1.0]
            } else {
                [0.2, 0.2, 0.25, 1.0]
            };

            add_quad(
                &mut vertices,
                &mut indices,
                to_ndc(slot_x, slot_y),
                to_ndc(slot_x + slot_size, slot_y),
                to_ndc(slot_x + slot_size, slot_y + slot_size),
                to_ndc(slot_x, slot_y + slot_size),
                slot_color,
            );

            // Draw shape/structure icons
            let icon_color = [0.9, 0.9, 0.9, 1.0];
            let center_x = slot_x + slot_size / 2.0;
            let center_y = slot_y + slot_size / 2.0;
            let icon_size = slot_size * 0.6;

            if self.quick_mode {
                let label = match i {
                    0 => "S",
                    1 => "W",
                    2 => "H",
                    3 => "T",
                    4 => "G",
                    5 => "R",
                    6 => "F",
                    _ => "?",
                };
                draw_text(
                    &mut vertices,
                    &mut indices,
                    label,
                    center_x - 4.0,
                    center_y - 5.0,
                    2.0,
                    icon_color,
                    screen_width,
                    screen_height,
                );
            } else {
                match i {
                    0 => {
                        // Cube
                        let half = icon_size / 2.0;
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x - half, center_y - half),
                            to_ndc(center_x + half, center_y - half),
                            to_ndc(center_x + half, center_y + half),
                            to_ndc(center_x - half, center_y + half),
                            icon_color,
                        );
                    }
                    1 => {
                        // Cylinder
                        let w = icon_size * 0.4;
                        let h = icon_size * 0.8;
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x - w / 2.0, center_y - h / 2.0),
                            to_ndc(center_x + w / 2.0, center_y - h / 2.0),
                            to_ndc(center_x + w / 2.0, center_y + h / 2.0),
                            to_ndc(center_x - w / 2.0, center_y + h / 2.0),
                            icon_color,
                        );
                    }
                    2 => {
                        // Sphere
                        let r = icon_size * 0.4;
                        let segments = 8;
                        for j in 0..segments {
                            let a1 = (j as f32 / segments as f32) * std::f32::consts::TAU;
                            let a2 = ((j + 1) as f32 / segments as f32) * std::f32::consts::TAU;
                            add_quad(
                                &mut vertices,
                                &mut indices,
                                to_ndc(center_x, center_y),
                                to_ndc(center_x + a1.cos() * r, center_y + a1.sin() * r),
                                to_ndc(center_x + a2.cos() * r, center_y + a2.sin() * r),
                                to_ndc(center_x, center_y),
                                icon_color,
                            );
                        }
                    }
                    3 => {
                        // Dome
                        let r = icon_size * 0.4;
                        let segments = 6;
                        for j in 0..segments {
                            let a1 = (j as f32 / segments as f32) * std::f32::consts::PI;
                            let a2 = ((j + 1) as f32 / segments as f32) * std::f32::consts::PI;
                            add_quad(
                                &mut vertices,
                                &mut indices,
                                to_ndc(center_x, center_y + r * 0.3),
                                to_ndc(center_x + a1.cos() * r, center_y + r * 0.3 - a1.sin() * r),
                                to_ndc(center_x + a2.cos() * r, center_y + r * 0.3 - a2.sin() * r),
                                to_ndc(center_x, center_y + r * 0.3),
                                icon_color,
                            );
                        }
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x - icon_size * 0.4, center_y + r * 0.3),
                            to_ndc(center_x + icon_size * 0.4, center_y + r * 0.3),
                            to_ndc(center_x + icon_size * 0.4, center_y + r * 0.3 + 3.0),
                            to_ndc(center_x - icon_size * 0.4, center_y + r * 0.3 + 3.0),
                            icon_color,
                        );
                    }
                    4 => {
                        // Arch
                        let w = icon_size * 0.7;
                        let h = icon_size * 0.8;
                        let thickness = icon_size * 0.15;
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x - w / 2.0, center_y - h / 2.0),
                            to_ndc(center_x - w / 2.0 + thickness, center_y - h / 2.0),
                            to_ndc(center_x - w / 2.0 + thickness, center_y + h / 2.0),
                            to_ndc(center_x - w / 2.0, center_y + h / 2.0),
                            icon_color,
                        );
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x + w / 2.0 - thickness, center_y - h / 2.0),
                            to_ndc(center_x + w / 2.0, center_y - h / 2.0),
                            to_ndc(center_x + w / 2.0, center_y + h / 2.0),
                            to_ndc(center_x + w / 2.0 - thickness, center_y + h / 2.0),
                            icon_color,
                        );
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x - w / 2.0, center_y - h / 2.0),
                            to_ndc(center_x + w / 2.0, center_y - h / 2.0),
                            to_ndc(center_x + w / 2.0, center_y - h / 2.0 + thickness),
                            to_ndc(center_x - w / 2.0, center_y - h / 2.0 + thickness),
                            icon_color,
                        );
                    }
                    5 => {
                        // Wedge
                        let half = icon_size * 0.4;
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x - half, center_y + half),
                            to_ndc(center_x + half, center_y + half),
                            to_ndc(center_x, center_y - half),
                            to_ndc(center_x - half, center_y + half),
                            icon_color,
                        );
                    }
                    6 => {
                        // Bridge
                        let s = icon_size * 0.25;
                        let gap = icon_size * 0.3;
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x - gap - s, center_y - s),
                            to_ndc(center_x - gap, center_y - s),
                            to_ndc(center_x - gap, center_y + s),
                            to_ndc(center_x - gap - s, center_y + s),
                            icon_color,
                        );
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x + gap, center_y - s),
                            to_ndc(center_x + gap + s, center_y - s),
                            to_ndc(center_x + gap + s, center_y + s),
                            to_ndc(center_x + gap, center_y + s),
                            icon_color,
                        );
                        let line_h = s * 0.4;
                        add_quad(
                            &mut vertices,
                            &mut indices,
                            to_ndc(center_x - gap, center_y - line_h),
                            to_ndc(center_x + gap, center_y - line_h),
                            to_ndc(center_x + gap, center_y + line_h),
                            to_ndc(center_x - gap, center_y + line_h),
                            icon_color,
                        );
                    }
                    _ => {}
                }
            }

            // Draw number label
            let num_str = format!("{}", i + 1);
            draw_text(
                &mut vertices,
                &mut indices,
                &num_str,
                slot_x + slot_size / 2.0 - 3.0,
                slot_y + slot_size + 5.0,
                1.5,
                [0.7, 0.7, 0.7, 1.0],
                screen_width,
                screen_height,
            );
        }

        // Draw info panel
        let info_x = toolbar_x + toolbar_width + 15.0;

        let mat_text = format!("MAT {}", self.selected_material);
        draw_text(
            &mut vertices,
            &mut indices,
            &mat_text,
            info_x,
            toolbar_y + 10.0,
            1.5,
            [0.8, 0.8, 0.5, 1.0],
            screen_width,
            screen_height,
        );

        let height_text = format!("H {:.0}", self.build_height);
        draw_text(
            &mut vertices,
            &mut indices,
            &height_text,
            info_x,
            toolbar_y + 28.0,
            1.5,
            [0.5, 0.8, 0.5, 1.0],
            screen_width,
            screen_height,
        );

        let mode_text = if self.quick_mode {
            format!("Q {}", self.quick_structure_name())
        } else {
            format!("S {}", SHAPE_NAMES[self.selected_shape])
        };
        draw_text(
            &mut vertices,
            &mut indices,
            &mode_text,
            info_x,
            toolbar_y + 46.0,
            1.5,
            [0.7, 0.9, 0.7, 1.0],
            screen_width,
            screen_height,
        );

        if self.is_bridge_mode() {
            let bridge_text = if self.bridge_tool.first_face.is_some() {
                if self.bridge_tool.second_face.is_some() {
                    "READY"
                } else {
                    "FACE 1"
                }
            } else {
                "SELECT"
            };
            draw_text(
                &mut vertices,
                &mut indices,
                bridge_text,
                info_x,
                toolbar_y + 64.0,
                1.5,
                [1.0, 0.6, 0.2, 1.0],
                screen_width,
                screen_height,
            );
        }

        Mesh { vertices, indices }
    }
}
