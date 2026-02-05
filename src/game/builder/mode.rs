//! Builder Mode
//!
//! Fallout 4-style building mode with undo/redo support.

use crate::render::hex_prism::{DEFAULT_HEX_HEIGHT, DEFAULT_HEX_RADIUS};
use crate::render::{HexPrism, HexPrismGrid};

/// Build command for undo/redo
#[derive(Clone)]
pub enum BuildCommand {
    /// Place a single prism at coordinates
    Place {
        coord: (i32, i32, i32),
        material: u8,
    },
    /// Remove a prism from coordinates (stores material for redo)
    Remove {
        coord: (i32, i32, i32),
        material: u8,
    },
    /// Batch of commands (for paste operations)
    Batch { commands: Vec<BuildCommand> },
}

/// Builder mode state machine for Fallout 4-style building
pub struct BuilderMode {
    /// Whether builder mode is active (B key toggles)
    pub enabled: bool,
    /// Current cursor position in axial coordinates (q, r, level)
    pub cursor_coord: Option<(i32, i32, i32)>,
    /// Currently selected material (1-8 keys)
    pub selected_material: u8,
    /// Current build height level (scroll wheel adjusts)
    pub build_level: i32,
    /// Ghost preview visibility
    pub show_preview: bool,

    // Advanced features
    /// Clipboard for copy/paste (relative coordinates)
    pub clipboard: Vec<((i32, i32, i32), u8)>,
    /// Undo stack
    pub undo_stack: Vec<BuildCommand>,
    /// Redo stack
    pub redo_stack: Vec<BuildCommand>,
    /// Rotation for paste (0, 1, 2, 3 = 0°, 60°, 120°, 180° for hex)
    pub paste_rotation: u8,
    /// Ctrl key held
    pub ctrl_held: bool,
}

impl Default for BuilderMode {
    fn default() -> Self {
        Self {
            enabled: false,
            cursor_coord: None,
            selected_material: 0, // Stone gray
            build_level: 0,
            show_preview: true,
            clipboard: Vec::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            paste_rotation: 0,
            ctrl_held: false,
        }
    }
}

impl BuilderMode {
    /// Toggle builder mode on/off
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if self.enabled {
            println!("[Builder Mode] ENABLED - Left-click to place, Right-click to remove");
            println!("  Materials: 1-8 | Scroll: height | Ctrl+Z: undo | Ctrl+C/V: copy/paste");
        } else {
            println!("[Builder Mode] DISABLED");
        }
    }

    /// Select material by index (0-7)
    pub fn select_material(&mut self, material: u8) {
        self.selected_material = material.min(7);
        let names = [
            "Stone Gray",
            "Stone Light",
            "Stone Dark",
            "Wood Brown",
            "Wood Light",
            "Wood Dark",
            "Metal Iron",
            "Metal Bronze",
        ];
        println!(
            "[Builder Mode] Material: {} ({})",
            self.selected_material + 1,
            names[self.selected_material as usize]
        );
    }

    /// Adjust build height level
    pub fn adjust_level(&mut self, delta: i32) {
        self.build_level = (self.build_level + delta).max(0);
        println!("[Builder Mode] Build level: {}", self.build_level);
    }

    /// Execute undo
    pub fn undo(&mut self, grid: &mut HexPrismGrid) {
        if let Some(cmd) = self.undo_stack.pop() {
            let redo_cmd = self.execute_inverse(&cmd, grid);
            self.redo_stack.push(redo_cmd);
            println!("[Builder Mode] Undo");
        }
    }

    /// Execute redo
    pub fn redo(&mut self, grid: &mut HexPrismGrid) {
        if let Some(cmd) = self.redo_stack.pop() {
            let undo_cmd = self.execute_command(&cmd, grid);
            self.undo_stack.push(undo_cmd);
            println!("[Builder Mode] Redo");
        }
    }

    /// Execute a build command and return its inverse for undo
    pub fn execute_command(&self, cmd: &BuildCommand, grid: &mut HexPrismGrid) -> BuildCommand {
        match cmd {
            BuildCommand::Place { coord, material } => {
                let prism = HexPrism::new(DEFAULT_HEX_HEIGHT, DEFAULT_HEX_RADIUS, *material);
                grid.insert(coord.0, coord.1, coord.2, prism);
                BuildCommand::Remove {
                    coord: *coord,
                    material: *material,
                }
            }
            BuildCommand::Remove { coord, material } => {
                grid.remove(coord.0, coord.1, coord.2);
                BuildCommand::Place {
                    coord: *coord,
                    material: *material,
                }
            }
            BuildCommand::Batch { commands } => {
                let inverse_cmds: Vec<_> = commands
                    .iter()
                    .map(|c| self.execute_command(c, grid))
                    .collect();
                BuildCommand::Batch {
                    commands: inverse_cmds,
                }
            }
        }
    }

    /// Execute inverse of a command (for undo)
    pub fn execute_inverse(&self, cmd: &BuildCommand, grid: &mut HexPrismGrid) -> BuildCommand {
        match cmd {
            BuildCommand::Place { coord, material } => {
                grid.remove(coord.0, coord.1, coord.2);
                BuildCommand::Place {
                    coord: *coord,
                    material: *material,
                }
            }
            BuildCommand::Remove { coord, material } => {
                let prism = HexPrism::new(DEFAULT_HEX_HEIGHT, DEFAULT_HEX_RADIUS, *material);
                grid.insert(coord.0, coord.1, coord.2, prism);
                BuildCommand::Remove {
                    coord: *coord,
                    material: *material,
                }
            }
            BuildCommand::Batch { commands } => {
                // Execute in reverse order for batch
                let inverse_cmds: Vec<_> = commands
                    .iter()
                    .rev()
                    .map(|c| self.execute_inverse(c, grid))
                    .collect();
                BuildCommand::Batch {
                    commands: inverse_cmds,
                }
            }
        }
    }

    /// Place a prism at the cursor position
    pub fn place_at_cursor(&mut self, grid: &mut HexPrismGrid) -> bool {
        if let Some(coord) = self.cursor_coord {
            // Check if position is already occupied
            if grid.contains(coord.0, coord.1, coord.2) {
                return false;
            }

            let cmd = BuildCommand::Place {
                coord,
                material: self.selected_material,
            };
            self.execute_command(&cmd, grid);
            self.undo_stack.push(cmd);
            self.redo_stack.clear(); // Clear redo on new action
            println!(
                "[Builder Mode] Placed prism at ({}, {}, {})",
                coord.0, coord.1, coord.2
            );
            true
        } else {
            false
        }
    }

    /// Remove prism at cursor position  
    pub fn remove_at_cursor(&mut self, grid: &mut HexPrismGrid) -> bool {
        if let Some(coord) = self.cursor_coord {
            if let Some(prism) = grid.get(coord.0, coord.1, coord.2) {
                let material = prism.material;
                let cmd = BuildCommand::Remove { coord, material };
                self.execute_command(&cmd, grid);
                self.undo_stack.push(cmd);
                self.redo_stack.clear();
                println!(
                    "[Builder Mode] Removed prism at ({}, {}, {})",
                    coord.0, coord.1, coord.2
                );
                return true;
            }
        }
        false
    }

    /// Copy prisms in area around cursor to clipboard
    pub fn copy_area(&mut self, grid: &HexPrismGrid, radius: i32) {
        self.clipboard.clear();
        if let Some(center) = self.cursor_coord {
            for (coord, prism) in grid.iter() {
                let dq = (coord.0 - center.0).abs();
                let dr = (coord.1 - center.1).abs();
                if dq <= radius && dr <= radius {
                    // Store relative coordinates
                    let rel = (coord.0 - center.0, coord.1 - center.1, coord.2 - center.2);
                    self.clipboard.push((rel, prism.material));
                }
            }
            println!("[Builder Mode] Copied {} prisms", self.clipboard.len());
        }
    }

    /// Paste clipboard at cursor
    pub fn paste(&mut self, grid: &mut HexPrismGrid) -> bool {
        if self.clipboard.is_empty() {
            return false;
        }

        if let Some(center) = self.cursor_coord {
            let mut commands = Vec::new();
            for (rel, material) in &self.clipboard {
                // Apply rotation (hex rotation by 60 degrees)
                let (rq, rr) = self.rotate_hex(*rel, self.paste_rotation);
                let coord = (center.0 + rq, center.1 + rr, center.2 + rel.2);

                if !grid.contains(coord.0, coord.1, coord.2) {
                    commands.push(BuildCommand::Place {
                        coord,
                        material: *material,
                    });
                }
            }

            if !commands.is_empty() {
                let batch = BuildCommand::Batch {
                    commands: commands.clone(),
                };
                for cmd in &commands {
                    self.execute_command(cmd, grid);
                }
                self.undo_stack.push(batch);
                self.redo_stack.clear();
                println!("[Builder Mode] Pasted {} prisms", commands.len());
                return true;
            }
        }
        false
    }

    /// Rotate hex coordinates by 60 degrees * rotation_steps
    fn rotate_hex(&self, rel: (i32, i32, i32), steps: u8) -> (i32, i32) {
        let mut q = rel.0;
        let mut r = rel.1;
        for _ in 0..(steps % 6) {
            // Rotate 60 degrees clockwise in axial coordinates
            // (q, r) -> (-r, q + r)
            let new_q = -r;
            let new_r = q + r;
            q = new_q;
            r = new_r;
        }
        (q, r)
    }

    /// Rotate paste selection
    pub fn rotate_selection(&mut self) {
        self.paste_rotation = (self.paste_rotation + 1) % 6;
        println!(
            "[Builder Mode] Rotation: {}°",
            self.paste_rotation as i32 * 60
        );
    }
}
