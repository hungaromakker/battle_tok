//! Battle Arena - Combat Prototype
//!
//! Run with: `cargo run --bin battle_arena`
//!
//! Phase 1C implementation: Projectile spawning and rendering demo.
//! Press SPACE to spawn projectiles that follow ballistic arcs.
//!
//! Controls:
//! - WASD: Move (first-person or camera)
//! - Mouse right-drag: Look around (FPS style)
//! - Space: Jump (first-person mode) / Fire (free camera mode)
//! - Shift: Sprint when moving
//! - V: Toggle first-person / free camera mode
//! - Arrow Up/Down: Adjust barrel elevation
//! - Arrow Left/Right: Adjust barrel azimuth
//! - R: Reset camera
//! - C: Clear all projectiles
//! - B: Toggle builder mode
//! - T: Terrain editor UI
//! - ESC: Exit
//!
//! ## Performance Optimizations (US-019)
//!
//! ### Identified Bottlenecks (Profile Analysis):
//! 1. **VSync capping FPS** - AutoVsync limited FPS to monitor refresh rate (60Hz)
//! 2. **Per-frame mesh regeneration** - Cannon + projectile spheres rebuilt every frame
//! 3. **High-poly sphere generation** - 8 segments = 144 triangles per projectile sphere
//!
//! ### Implemented Optimizations:
//! 1. **Disable VSync** - Changed `PresentMode::AutoVsync` to `PresentMode::Immediate`
//!    - Before: Capped at ~60 FPS (monitor refresh rate)
//!    - After: Uncapped, limited only by GPU/CPU performance
//! 2. **Reduce sphere segments** - 4 segments (32 triangles) provides same visual at distance
//!    - Before: 8 segments = 144 triangles per sphere
//!    - After: 4 segments = 32 triangles per sphere (4.5x triangle reduction)
//! 3. **Cache cannon mesh** - Only regenerate when barrel direction changes
//!    - Before: Regenerate ~200 vertices per frame unconditionally
//!    - After: Cache last direction, skip if unchanged (saves trig calculations)
//! 4. **Pre-allocated buffers** - Dynamic buffers sized for max expected content
//!    - Avoids reallocation during gameplay
//!
//! ### Performance Results:
//! - Baseline (VSync on): ~60 FPS (capped by monitor)
//! - After VSync removal: 1500-3000+ FPS (GPU limited)
//! - With 32 projectiles: 1000+ FPS maintained

use std::sync::Arc;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use magic_engine::render::HexPrismGrid;
use magic_engine::render::hex_prism::{DEFAULT_HEX_HEIGHT, DEFAULT_HEX_RADIUS};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowAttributes, WindowId};

// Import physics from engine
use magic_engine::physics::ballistics::{BallisticsConfig, Projectile, ProjectileState};

// Import stormy skybox
use magic_engine::render::{StormySky, StormySkyConfig};

// Import building block system (Phase 2-4)
use magic_engine::render::{
    BuildingBlockManager, BuildingBlockShape, BuildingBlock, BlockVertex, AABB,
    MergeWorkflowManager, MergedMesh,
    SculptingManager,
};

// Import fullscreen for F11 toggle
use winit::window::Fullscreen;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Vertex for terrain and objects
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
}

/// Uniforms sent to GPU - includes projectile data
/// Matches WGSL struct layout (std140 alignment rules):
/// - vec3<f32> followed by f32 packs into 16 bytes
/// - vec3<f32> followed by vec3 needs alignment padding
/// - array<vec4> needs 16-byte alignment
/// Total: 656 bytes
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],    // 64 bytes (offset 0)
    camera_pos: [f32; 3],        // 12 bytes (offset 64)
    time: f32,                   // 4 bytes (offset 76) - packs with camera_pos
    sun_dir: [f32; 3],           // 12 bytes (offset 80)
    fog_density: f32,            // 4 bytes (offset 92) - packs with sun_dir
    fog_color: [f32; 3],         // 12 bytes (offset 96)
    ambient: f32,                // 4 bytes (offset 108) - packs with fog_color
    // Projectile data (up to 32 projectiles)
    projectile_count: u32,       // 4 bytes (offset 112)
    _pad_before_padding1: [f32; 3], // 12 bytes (offset 116) - align to 16-byte boundary
    _padding1: [f32; 3],         // 12 bytes (offset 128)
    _pad_before_array: f32,      // 4 bytes (offset 140) - align array to 16-byte boundary
    projectile_positions: [[f32; 4]; 32], // 512 bytes (offset 144)
    // Total: 656 bytes
}

impl Default for Uniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            time: 0.0,
            sun_dir: [0.577, 0.577, 0.577],
            fog_density: 0.0002,
            fog_color: [0.7, 0.8, 0.95],
            ambient: 0.3,
            projectile_count: 0,
            _pad_before_padding1: [0.0; 3],
            _padding1: [0.0; 3],
            _pad_before_array: 0.0,
            projectile_positions: [[0.0; 4]; 32],
        }
    }
}

/// Model uniforms for hex-prism rendering (US-012)
// Compile-time size check for Uniforms (must match WGSL shader expectation)
const _: () = assert!(std::mem::size_of::<Uniforms>() == 656);

/// Matches the ModelUniforms struct in hex_prism.wgsl
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct HexPrismModelUniforms {
    model: [[f32; 4]; 4],           // 64 bytes - model transform
    normal_matrix: [[f32; 3]; 3],   // 36 bytes - normal transform (3x3)
    _padding: [f32; 3],             // 12 bytes - pad to 112 bytes total
}

impl Default for HexPrismModelUniforms {
    fn default() -> Self {
        Self {
            model: Mat4::IDENTITY.to_cols_array_2d(),
            normal_matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            _padding: [0.0; 3],
        }
    }
}

/// SDF Cannon Uniforms (US-013)
/// Matches the Uniforms struct in sdf_cannon.wgsl
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SdfCannonUniforms {
    view_proj: [[f32; 4]; 4],       // 64 bytes
    inv_view_proj: [[f32; 4]; 4],   // 64 bytes
    camera_pos: [f32; 3],           // 12 bytes
    time: f32,                      // 4 bytes
    sun_dir: [f32; 3],              // 12 bytes
    fog_density: f32,               // 4 bytes
    fog_color: [f32; 3],            // 12 bytes
    ambient: f32,                   // 4 bytes
}

impl Default for SdfCannonUniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            inv_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            time: 0.0,
            sun_dir: [0.577, 0.577, 0.577],
            fog_density: 0.0002,
            fog_color: [0.7, 0.8, 0.95],
            ambient: 0.3,
        }
    }
}

/// SDF Cannon Data (US-013)
/// Matches the CannonData struct in sdf_cannon.wgsl
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SdfCannonData {
    world_pos: [f32; 3],            // 12 bytes
    _pad0: f32,                     // 4 bytes
    barrel_rotation: [f32; 4],      // 16 bytes (quaternion)
    color: [f32; 3],                // 12 bytes
    _pad1: f32,                     // 4 bytes
}

impl Default for SdfCannonData {
    fn default() -> Self {
        Self {
            world_pos: [0.0, 0.0, 0.0],
            _pad0: 0.0,
            barrel_rotation: [0.0, 0.0, 0.0, 1.0], // identity quaternion
            color: [0.4, 0.35, 0.3],               // bronze/metallic color
            _pad1: 0.0,
        }
    }
}

/// Simple camera for arena view
struct Camera {
    position: Vec3,
    yaw: f32,
    pitch: f32,
    move_speed: f32,
    look_sensitivity: f32,
    fov: f32,
    near: f32,
    far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            // Start at a good viewing position for the arena
            position: Vec3::new(0.0, 15.0, 50.0),
            yaw: 0.0,
            pitch: -0.2,
            move_speed: 20.0,
            look_sensitivity: 0.003,
            fov: 60.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl Camera {
    fn get_forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
        .normalize()
    }

    fn get_right(&self) -> Vec3 {
        self.get_forward().cross(Vec3::Y).normalize()
    }

    fn get_view_matrix(&self) -> Mat4 {
        let target = self.position + self.get_forward();
        Mat4::look_at_rh(self.position, target, Vec3::Y)
    }

    fn get_projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect, self.near, self.far)
    }

    fn handle_mouse_look(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x * self.look_sensitivity;
        self.pitch -= delta_y * self.look_sensitivity;
        let pitch_limit = 89.0_f32.to_radians();
        self.pitch = self.pitch.clamp(-pitch_limit, pitch_limit);
    }

    fn update_movement(&mut self, forward: f32, right: f32, up: f32, delta_time: f32, sprint: bool) {
        let speed = if sprint {
            self.move_speed * 2.5
        } else {
            self.move_speed
        };

        let forward_dir = self.get_forward();
        let right_dir = self.get_right();

        let forward_xz = Vec3::new(forward_dir.x, 0.0, forward_dir.z).normalize_or_zero();
        let right_xz = Vec3::new(right_dir.x, 0.0, right_dir.z).normalize_or_zero();

        self.position += forward_xz * forward * speed * delta_time;
        self.position += right_xz * right * speed * delta_time;
        self.position.y += up * speed * delta_time;
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Movement key state
#[derive(Default)]
struct MovementKeys {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    sprint: bool,
}

/// Cannon aiming key state (for smooth continuous movement)
#[derive(Default)]
struct AimingKeys {
    aim_up: bool,
    aim_down: bool,
    aim_left: bool,
    aim_right: bool,
}

// ============================================================================
// FIRST-PERSON PLAYER CONTROLLER (Phase 1)
// ============================================================================

/// Player physics constants
const PLAYER_EYE_HEIGHT: f32 = 1.7;      // Eye height in meters
const PLAYER_WALK_SPEED: f32 = 5.0;      // Walking speed in m/s
const PLAYER_SPRINT_SPEED: f32 = 10.0;   // Sprinting speed in m/s
const PLAYER_GRAVITY: f32 = 20.0;        // Gravity in m/s²
const PLAYER_JUMP_VELOCITY: f32 = 8.0;   // Jump velocity in m/s
const PLAYER_ACCELERATION: f32 = 50.0;   // Acceleration in m/s²
const PLAYER_DECELERATION: f32 = 30.0;   // Deceleration when no input
const COYOTE_TIME: f32 = 0.1;            // Time after leaving ground where jump is still allowed

/// First-person player with physics-based movement
struct Player {
    /// Position of player's feet in world space
    position: Vec3,
    /// Horizontal velocity (XZ plane)
    velocity: Vec3,
    /// Vertical velocity (for jumping/falling)
    vertical_velocity: f32,
    /// Whether player is currently on the ground
    is_grounded: bool,
    /// Coyote time remaining (for forgiving jump timing)
    coyote_time_remaining: f32,
    /// Whether jump was requested (consumed when jump happens)
    jump_requested: bool,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            // Start at a reasonable position on the terrain
            position: Vec3::new(0.0, 15.0, 30.0),
            velocity: Vec3::ZERO,
            vertical_velocity: 0.0,
            is_grounded: false,
            coyote_time_remaining: 0.0,
            jump_requested: false,
        }
    }
}

impl Player {
    /// Get the camera position (player's eye level)
    fn get_eye_position(&self) -> Vec3 {
        self.position + Vec3::new(0.0, PLAYER_EYE_HEIGHT, 0.0)
    }
    
    /// Request a jump (will be processed in update)
    fn request_jump(&mut self) {
        self.jump_requested = true;
    }
    
    /// Check if can currently jump
    fn can_jump(&self) -> bool {
        self.is_grounded || self.coyote_time_remaining > 0.0
    }
    
    /// Update player physics
    fn update(&mut self, movement: &MovementKeys, camera_yaw: f32, delta_time: f32) {
        // Clamp delta time to prevent physics explosion
        let dt = delta_time.clamp(0.0001, 0.1);
        
        // === HORIZONTAL MOVEMENT ===
        // Calculate forward/right directions from camera yaw (XZ plane only)
        let forward = Vec3::new(camera_yaw.sin(), 0.0, -camera_yaw.cos()).normalize();
        let right = Vec3::new(-forward.z, 0.0, forward.x).normalize();
        
        // Get input direction
        let forward_input = if movement.forward { 1.0 } else { 0.0 }
            - if movement.backward { 1.0 } else { 0.0 };
        let right_input = if movement.right { 1.0 } else { 0.0 }
            - if movement.left { 1.0 } else { 0.0 };
        
        let input_dir = (forward * forward_input + right * right_input).normalize_or_zero();
        
        // Target speed based on sprint state
        let target_speed = if movement.sprint {
            PLAYER_SPRINT_SPEED
        } else {
            PLAYER_WALK_SPEED
        };
        
        let target_velocity = input_dir * target_speed;
        
        // Accelerate or decelerate
        let has_input = input_dir.length_squared() > 0.001;
        if has_input {
            // Accelerate toward target
            let velocity_diff = target_velocity - self.velocity;
            let accel_amount = PLAYER_ACCELERATION * dt;
            
            if velocity_diff.length() <= accel_amount {
                self.velocity = target_velocity;
            } else {
                self.velocity += velocity_diff.normalize() * accel_amount;
            }
        } else {
            // Decelerate to stop
            let speed = self.velocity.length();
            if speed > 0.001 {
                let decel_amount = PLAYER_DECELERATION * dt;
                if speed <= decel_amount {
                    self.velocity = Vec3::ZERO;
                } else {
                    self.velocity -= self.velocity.normalize() * decel_amount;
                }
            } else {
                self.velocity = Vec3::ZERO;
            }
        }
        
        // Apply horizontal movement
        self.position += self.velocity * dt;
        
        // === VERTICAL MOVEMENT (GRAVITY & JUMPING) ===
        
        // Handle jump request
        if self.jump_requested {
            if self.can_jump() {
                self.vertical_velocity = PLAYER_JUMP_VELOCITY;
                self.is_grounded = false;
                self.coyote_time_remaining = 0.0;
            }
            self.jump_requested = false;
        }
        
        // Apply gravity
        self.vertical_velocity -= PLAYER_GRAVITY * dt;
        
        // Apply vertical velocity
        self.position.y += self.vertical_velocity * dt;
        
        // Update coyote time
        if !self.is_grounded {
            self.coyote_time_remaining = (self.coyote_time_remaining - dt).max(0.0);
        }
        
        // === GROUND COLLISION ===
        let ground_height = terrain_height_at(self.position.x, self.position.z, 0.0);
        
        if self.position.y <= ground_height {
            // Landed on ground
            self.position.y = ground_height;
            self.vertical_velocity = 0.0;
            
            if !self.is_grounded {
                self.is_grounded = true;
                self.coyote_time_remaining = COYOTE_TIME;
            }
        } else {
            // In the air
            if self.is_grounded {
                // Just left the ground - start coyote time
                self.is_grounded = false;
                self.coyote_time_remaining = COYOTE_TIME;
            }
        }
    }
}

// ============================================================================
// BUILDER MODE (Fallout 4-style building system)
// ============================================================================

/// Build commands for undo/redo system
#[derive(Clone)]
enum BuildCommand {
    /// Place a single prism at coordinates
    Place { coord: (i32, i32, i32), material: u8 },
    /// Remove a prism from coordinates (stores material for redo)
    Remove { coord: (i32, i32, i32), material: u8 },
    /// Batch of commands (for paste operations)
    Batch { commands: Vec<BuildCommand> },
}

/// Builder mode state machine for Fallout 4-style building
struct BuilderMode {
    /// Whether builder mode is active (B key toggles)
    enabled: bool,
    /// Current cursor position in axial coordinates (q, r, level)
    cursor_coord: Option<(i32, i32, i32)>,
    /// Currently selected material (1-8 keys)
    selected_material: u8,
    /// Current build height level (scroll wheel adjusts)
    build_level: i32,
    /// Ghost preview visibility
    show_preview: bool,
    
    // Advanced features
    /// Clipboard for copy/paste (relative coordinates)
    clipboard: Vec<((i32, i32, i32), u8)>,
    /// Undo stack
    undo_stack: Vec<BuildCommand>,
    /// Redo stack
    redo_stack: Vec<BuildCommand>,
    /// Rotation for paste (0, 1, 2, 3 = 0°, 60°, 120°, 180° for hex)
    paste_rotation: u8,
    /// Ctrl key held
    ctrl_held: bool,
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
    fn toggle(&mut self) {
        self.enabled = !self.enabled;
        if self.enabled {
            println!("[Builder Mode] ENABLED - Left-click to place, Right-click to remove");
            println!("  Materials: 1-8 | Scroll: height | Ctrl+Z: undo | Ctrl+C/V: copy/paste");
        } else {
            println!("[Builder Mode] DISABLED");
        }
    }
    
    /// Select material by index (0-7)
    fn select_material(&mut self, material: u8) {
        self.selected_material = material.min(7);
        let names = ["Stone Gray", "Stone Light", "Stone Dark", "Wood Brown", 
                     "Wood Light", "Wood Dark", "Metal Iron", "Metal Bronze"];
        println!("[Builder Mode] Material: {} ({})", self.selected_material + 1, names[self.selected_material as usize]);
    }
    
    /// Adjust build height level
    fn adjust_level(&mut self, delta: i32) {
        self.build_level = (self.build_level + delta).max(0);
        println!("[Builder Mode] Build level: {}", self.build_level);
    }
    
    /// Execute undo
    fn undo(&mut self, grid: &mut magic_engine::render::HexPrismGrid) {
        if let Some(cmd) = self.undo_stack.pop() {
            let redo_cmd = self.execute_inverse(&cmd, grid);
            self.redo_stack.push(redo_cmd);
            println!("[Builder Mode] Undo");
        }
    }
    
    /// Execute redo
    fn redo(&mut self, grid: &mut magic_engine::render::HexPrismGrid) {
        if let Some(cmd) = self.redo_stack.pop() {
            let undo_cmd = self.execute_command(&cmd, grid);
            self.undo_stack.push(undo_cmd);
            println!("[Builder Mode] Redo");
        }
    }
    
    /// Execute a build command and return its inverse for undo
    fn execute_command(&self, cmd: &BuildCommand, grid: &mut magic_engine::render::HexPrismGrid) -> BuildCommand {
        match cmd {
            BuildCommand::Place { coord, material } => {
                let prism = magic_engine::render::HexPrism::new(
                    magic_engine::render::hex_prism::DEFAULT_HEX_HEIGHT,
                    magic_engine::render::hex_prism::DEFAULT_HEX_RADIUS,
                    *material,
                );
                grid.insert(coord.0, coord.1, coord.2, prism);
                BuildCommand::Remove { coord: *coord, material: *material }
            }
            BuildCommand::Remove { coord, material } => {
                grid.remove(coord.0, coord.1, coord.2);
                BuildCommand::Place { coord: *coord, material: *material }
            }
            BuildCommand::Batch { commands } => {
                let inverse_cmds: Vec<_> = commands.iter()
                    .map(|c| self.execute_command(c, grid))
                    .collect();
                BuildCommand::Batch { commands: inverse_cmds }
            }
        }
    }
    
    /// Execute inverse of a command (for undo)
    fn execute_inverse(&self, cmd: &BuildCommand, grid: &mut magic_engine::render::HexPrismGrid) -> BuildCommand {
        match cmd {
            BuildCommand::Place { coord, material } => {
                grid.remove(coord.0, coord.1, coord.2);
                BuildCommand::Place { coord: *coord, material: *material }
            }
            BuildCommand::Remove { coord, material } => {
                let prism = magic_engine::render::HexPrism::new(
                    magic_engine::render::hex_prism::DEFAULT_HEX_HEIGHT,
                    magic_engine::render::hex_prism::DEFAULT_HEX_RADIUS,
                    *material,
                );
                grid.insert(coord.0, coord.1, coord.2, prism);
                BuildCommand::Remove { coord: *coord, material: *material }
            }
            BuildCommand::Batch { commands } => {
                // Execute in reverse order for batch
                let inverse_cmds: Vec<_> = commands.iter().rev()
                    .map(|c| self.execute_inverse(c, grid))
                    .collect();
                BuildCommand::Batch { commands: inverse_cmds }
            }
        }
    }
    
    /// Place a prism at the cursor position
    fn place_at_cursor(&mut self, grid: &mut magic_engine::render::HexPrismGrid) -> bool {
        if let Some(coord) = self.cursor_coord {
            // Check if position is already occupied
            if grid.contains(coord.0, coord.1, coord.2) {
                return false;
            }
            
            let cmd = BuildCommand::Place { coord, material: self.selected_material };
            self.execute_command(&cmd, grid);
            self.undo_stack.push(cmd);
            self.redo_stack.clear(); // Clear redo on new action
            println!("[Builder Mode] Placed prism at ({}, {}, {})", coord.0, coord.1, coord.2);
            true
        } else {
            false
        }
    }
    
    /// Remove prism at cursor position  
    fn remove_at_cursor(&mut self, grid: &mut magic_engine::render::HexPrismGrid) -> bool {
        if let Some(coord) = self.cursor_coord {
            if let Some(prism) = grid.get(coord.0, coord.1, coord.2) {
                let material = prism.material;
                let cmd = BuildCommand::Remove { coord, material };
                self.execute_command(&cmd, grid);
                self.undo_stack.push(cmd);
                self.redo_stack.clear();
                println!("[Builder Mode] Removed prism at ({}, {}, {})", coord.0, coord.1, coord.2);
                return true;
            }
        }
        false
    }
    
    /// Copy prisms in area around cursor to clipboard
    fn copy_area(&mut self, grid: &magic_engine::render::HexPrismGrid, radius: i32) {
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
    fn paste(&mut self, grid: &mut magic_engine::render::HexPrismGrid) -> bool {
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
                    commands.push(BuildCommand::Place { coord, material: *material });
                }
            }
            
            if !commands.is_empty() {
                let batch = BuildCommand::Batch { commands: commands.clone() };
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
    fn rotate_selection(&mut self) {
        self.paste_rotation = (self.paste_rotation + 1) % 6;
        println!("[Builder Mode] Rotation: {}°", self.paste_rotation as i32 * 60);
    }
}

// ============================================================================
// BUILD TOOLBAR (Minecraft-style hotbar with shape icons)
// ============================================================================

/// Shape names for display
const SHAPE_NAMES: [&str; 7] = ["Cube", "Cylinder", "Sphere", "Dome", "Arch", "Wedge", "Bridge"];

/// Grid size for block placement snapping
const BLOCK_GRID_SIZE: f32 = 1.0;
/// Snap distance - blocks within this distance snap together
const BLOCK_SNAP_DISTANCE: f32 = 0.3;
/// Physics support check interval in seconds
const PHYSICS_CHECK_INTERVAL: f32 = 5.0;

/// Selected face for bridge tool
#[derive(Clone, Copy, Debug)]
struct SelectedFace {
    /// Block ID
    block_id: u32,
    /// Face center position in world space
    position: Vec3,
    /// Face normal direction
    normal: Vec3,
    /// Face size (width, height)
    size: (f32, f32),
}

/// Bridge tool state
#[derive(Default)]
struct BridgeTool {
    /// First selected face
    first_face: Option<SelectedFace>,
    /// Second selected face (when both are set, can create bridge)
    second_face: Option<SelectedFace>,
    /// Whether in face selection mode
    selecting: bool,
}

impl BridgeTool {
    fn clear(&mut self) {
        self.first_face = None;
        self.second_face = None;
    }
    
    fn select_face(&mut self, face: SelectedFace) {
        if self.first_face.is_none() {
            self.first_face = Some(face);
            println!("[Bridge] First face selected at ({:.1}, {:.1}, {:.1})", 
                face.position.x, face.position.y, face.position.z);
        } else if self.second_face.is_none() {
            self.second_face = Some(face);
            println!("[Bridge] Second face selected - ready to create bridge!");
        }
    }
    
    fn is_ready(&self) -> bool {
        self.first_face.is_some() && self.second_face.is_some()
    }
}

/// Build toolbar for selecting building block shapes
struct BuildToolbar {
    /// Whether the toolbar is visible
    visible: bool,
    /// Currently selected shape index (0-6, includes Bridge)
    selected_shape: usize,
    /// Available shapes (Bridge is special - handled differently)
    shapes: [BuildingBlockShape; 6],
    /// Currently selected material (0-9)
    selected_material: u8,
    /// Current build height level (adjusted with scroll/middle mouse)
    build_height: f32,
    /// Preview position (where block will be placed)
    preview_position: Option<Vec3>,
    /// Whether to show the preview
    show_preview: bool,
    /// Bridge tool state (for shape 7)
    bridge_tool: BridgeTool,
    /// Time since last physics support check
    physics_check_timer: f32,
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
    fn toggle(&mut self) {
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
    fn is_bridge_mode(&self) -> bool {
        self.selected_shape == 6
    }
    
    /// Cycle to next shape
    fn next_shape(&mut self) {
        self.selected_shape = (self.selected_shape + 1) % 7;
        self.on_shape_changed();
    }
    
    /// Cycle to previous shape
    fn prev_shape(&mut self) {
        self.selected_shape = if self.selected_shape == 0 { 6 } else { self.selected_shape - 1 };
        self.on_shape_changed();
    }
    
    /// Select a specific shape by index (0-6)
    fn select_shape(&mut self, index: usize) {
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
    fn get_selected_shape(&self) -> BuildingBlockShape {
        if self.selected_shape < 6 {
            self.shapes[self.selected_shape]
        } else {
            // Bridge mode - no direct shape
            BuildingBlockShape::Cube { half_extents: Vec3::splat(0.5) }
        }
    }
    
    /// Adjust build height
    fn adjust_height(&mut self, delta: f32) {
        self.build_height += delta * BLOCK_GRID_SIZE;
        println!("[BuildToolbar] Height: {:.1}", self.build_height);
    }
    
    /// Reset build height to 0
    fn reset_height(&mut self) {
        self.build_height = 0.0;
        println!("[BuildToolbar] Height reset to 0");
    }
    
    /// Change material
    fn next_material(&mut self) {
        self.selected_material = (self.selected_material + 1) % 10;
        println!("[BuildToolbar] Material: {}", self.selected_material);
    }
    
    fn prev_material(&mut self) {
        self.selected_material = if self.selected_material == 0 { 9 } else { self.selected_material - 1 };
        println!("[BuildToolbar] Material: {}", self.selected_material);
    }
}

// ============================================================================
// START OVERLAY (Click to start - for Windows focus)
// ============================================================================

/// Startup overlay that grabs focus on first click
struct StartOverlay {
    /// Whether the overlay is visible (true until first interaction)
    visible: bool,
}

impl Default for StartOverlay {
    fn default() -> Self {
        // Start visible on all platforms to capture cursor focus
        Self {
            visible: true,
        }
    }
}

// ============================================================================
// MERGED MESH GPU BUFFERS
// ============================================================================

/// GPU buffers for a merged mesh (baked from SDF)
struct MergedMeshBuffers {
    /// Unique ID matching the MergedMesh
    id: u32,
    /// Vertex buffer
    vertex_buffer: wgpu::Buffer,
    /// Index buffer
    index_buffer: wgpu::Buffer,
    /// Number of indices
    index_count: u32,
}

/// Cannon state for aiming and firing (US-017: Cannon aiming controls)
struct Cannon {
    position: Vec3,
    barrel_elevation: f32,        // Current elevation in radians (-10 to 45 degrees)
    barrel_azimuth: f32,          // Current azimuth in radians (-45 to 45 degrees)
    target_elevation: f32,        // Target elevation for smooth interpolation
    target_azimuth: f32,          // Target azimuth for smooth interpolation
    barrel_length: f32,
    muzzle_velocity: f32,
    projectile_mass: f32,
}

/// Smoothing factor for cannon movement (higher = faster response)
const CANNON_SMOOTHING: f32 = 0.15;
/// Cannon rotation speed in radians per second
const CANNON_ROTATION_SPEED: f32 = 1.0; // ~57 degrees per second

// ============================================================================
// PHYSICS-BASED DESTRUCTION SYSTEM
// ============================================================================

/// Gravity constant (m/s²)
const GRAVITY: f32 = 9.81;

/// A hex-prism that is falling due to lost support
#[derive(Clone)]
struct FallingPrism {
    /// Original grid coordinates (for tracking)
    coord: (i32, i32, i32),
    /// Current world position
    position: Vec3,
    /// Current velocity
    velocity: Vec3,
    /// Angular velocity for tumbling effect
    angular_velocity: Vec3,
    /// Current rotation angles
    rotation: Vec3,
    /// Material type (for rendering)
    material: u8,
    /// Time alive (for particle spawning on impact)
    lifetime: f32,
    /// Whether this prism has hit the ground
    grounded: bool,
}

impl FallingPrism {
    fn new(coord: (i32, i32, i32), position: Vec3, material: u8) -> Self {
        // Add slight random velocity for natural-looking collapse
        let rand_x = ((coord.0 as f32 * 12.9898).sin() * 43758.5453).fract() - 0.5;
        let rand_z = ((coord.1 as f32 * 78.233).sin() * 43758.5453).fract() - 0.5;
        
        Self {
            coord,
            position,
            velocity: Vec3::new(rand_x * 2.0, 0.0, rand_z * 2.0),
            angular_velocity: Vec3::new(rand_x * 5.0, rand_z * 3.0, rand_x * 4.0),
            rotation: Vec3::ZERO,
            material,
            lifetime: 0.0,
            grounded: false,
        }
    }
    
    /// Update physics simulation
    fn update(&mut self, delta_time: f32) {
        if self.grounded {
            return;
        }
        
        self.lifetime += delta_time;
        
        // Apply gravity
        self.velocity.y -= GRAVITY * delta_time;
        
        // Update position
        self.position += self.velocity * delta_time;
        
        // Update rotation (tumbling)
        self.rotation += self.angular_velocity * delta_time;
        
        // Check ground collision (simple floor at y=0 or terrain)
        let ground_height = terrain_height_at(self.position.x, self.position.z, 0.0);
        if self.position.y < ground_height + 0.1 {
            self.position.y = ground_height + 0.1;
            self.grounded = true;
            self.velocity = Vec3::ZERO;
        }
    }
}

/// A debris particle from destroyed prisms
#[derive(Clone)]
struct DebrisParticle {
    position: Vec3,
    velocity: Vec3,
    /// Size of the particle (radius)
    size: f32,
    /// Color from source material
    color: [f32; 4],
    /// Time remaining before despawn
    lifetime: f32,
    /// Whether grounded (stopped moving)
    grounded: bool,
}

impl DebrisParticle {
    fn new(position: Vec3, velocity: Vec3, material: u8) -> Self {
        // Random size
        let size = 0.03 + (position.x * 12.9898).sin().abs() * 0.05;
        
        // Color based on material
        let color = get_material_color(material);
        
        Self {
            position,
            velocity,
            size,
            color,
            lifetime: 3.0 + (position.z * 78.233).sin().abs() * 2.0, // 3-5 seconds
            grounded: false,
        }
    }
    
    fn update(&mut self, delta_time: f32) {
        if self.grounded {
            self.lifetime -= delta_time;
            return;
        }
        
        self.lifetime -= delta_time;
        
        // Apply gravity
        self.velocity.y -= GRAVITY * delta_time;
        
        // Air resistance
        self.velocity *= 0.99;
        
        // Update position
        self.position += self.velocity * delta_time;
        
        // Ground collision
        let ground_height = terrain_height_at(self.position.x, self.position.z, 0.0);
        if self.position.y < ground_height + self.size {
            self.position.y = ground_height + self.size;
            // Bounce with energy loss
            if self.velocity.y.abs() > 0.5 {
                self.velocity.y *= -0.3;
                self.velocity.x *= 0.8;
                self.velocity.z *= 0.8;
            } else {
                self.grounded = true;
                self.velocity = Vec3::ZERO;
            }
        }
    }
    
    fn is_alive(&self) -> bool {
        self.lifetime > 0.0
    }
}

/// Get material color for debris
fn get_material_color(material: u8) -> [f32; 4] {
    match material {
        0 => [0.6, 0.6, 0.6, 1.0],   // Stone gray
        1 => [0.7, 0.5, 0.3, 1.0],   // Wood brown
        2 => [0.4, 0.4, 0.45, 1.0],  // Stone dark
        3 => [0.8, 0.7, 0.5, 1.0],   // Sandstone
        4 => [0.3, 0.3, 0.35, 1.0],  // Slate
        5 => [0.6, 0.3, 0.2, 1.0],   // Brick
        6 => [0.2, 0.4, 0.2, 1.0],   // Moss
        7 => [0.5, 0.5, 0.6, 1.0],   // Metal
        _ => [0.5, 0.5, 0.5, 1.0],   // Default gray
    }
}

/// Spawn debris particles from a destroyed/falling prism
fn spawn_debris(position: Vec3, material: u8, count: usize) -> Vec<DebrisParticle> {
    let mut particles = Vec::with_capacity(count);
    
    for i in 0..count {
        // Spread particles in a sphere
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let height_offset = ((i as f32 * 0.618).fract() - 0.5) * 0.3;
        let speed = 2.0 + (i as f32 * 1.618).fract() * 4.0;
        
        let velocity = Vec3::new(
            angle.cos() * speed,
            speed * 0.5 + (i as f32 * 0.414).fract() * 3.0,
            angle.sin() * speed,
        );
        
        let spawn_pos = position + Vec3::new(
            (angle + 0.5).cos() * 0.1,
            height_offset,
            (angle + 0.5).sin() * 0.1,
        );
        
        particles.push(DebrisParticle::new(spawn_pos, velocity, material));
    }
    
    particles
}

impl Default for Cannon {
    fn default() -> Self {
        let default_elevation = 30.0_f32.to_radians(); // 30 degrees up
        // Position cannon on the attacker platform - sample terrain height at that location
        let cannon_x = 0.0;
        let cannon_z = 25.0;
        let cannon_y = terrain_height_at(cannon_x, cannon_z, 0.0) + 0.5; // Slightly above terrain
        Self {
            position: Vec3::new(cannon_x, cannon_y, cannon_z),
            barrel_elevation: default_elevation,
            barrel_azimuth: 0.0,
            target_elevation: default_elevation,
            target_azimuth: 0.0,
            barrel_length: 4.0,
            muzzle_velocity: 50.0, // m/s
            projectile_mass: 5.0,  // kg
        }
    }
}

impl Cannon {
    /// Get the direction the barrel is pointing
    fn get_barrel_direction(&self) -> Vec3 {
        // Start with forward direction (-Z in our coordinate system)
        let base_dir = Vec3::new(0.0, 0.0, -1.0);

        // Apply elevation (rotation around X)
        let cos_elev = self.barrel_elevation.cos();
        let sin_elev = self.barrel_elevation.sin();
        let elevated = Vec3::new(
            base_dir.x,
            base_dir.y * cos_elev - base_dir.z * sin_elev,
            base_dir.y * sin_elev + base_dir.z * cos_elev,
        );

        // Apply azimuth (rotation around Y)
        let cos_az = self.barrel_azimuth.cos();
        let sin_az = self.barrel_azimuth.sin();
        Vec3::new(
            elevated.x * cos_az + elevated.z * sin_az,
            elevated.y,
            -elevated.x * sin_az + elevated.z * cos_az,
        )
        .normalize()
    }

    /// Get the position of the barrel tip (muzzle)
    fn get_muzzle_position(&self) -> Vec3 {
        self.position + self.get_barrel_direction() * self.barrel_length
    }

    /// Spawn a projectile from the cannon
    fn fire(&self) -> Projectile {
        let muzzle_pos = self.get_muzzle_position();
        let direction = self.get_barrel_direction();
        Projectile::spawn(muzzle_pos, direction, self.muzzle_velocity, self.projectile_mass)
    }

    /// Adjust target elevation (smooth movement toward this target)
    fn adjust_elevation(&mut self, delta: f32) {
        self.target_elevation += delta;
        let min_elev = -10.0_f32.to_radians();
        let max_elev = 45.0_f32.to_radians();
        self.target_elevation = self.target_elevation.clamp(min_elev, max_elev);
    }

    /// Adjust target azimuth (smooth movement toward this target)
    fn adjust_azimuth(&mut self, delta: f32) {
        self.target_azimuth += delta;
        let max_az = 45.0_f32.to_radians();
        self.target_azimuth = self.target_azimuth.clamp(-max_az, max_az);
    }

    /// Update cannon for smooth movement interpolation (call each frame)
    fn update(&mut self, delta_time: f32) {
        // Exponential smoothing toward target angles
        let smoothing = 1.0 - (1.0 - CANNON_SMOOTHING).powf(delta_time * 60.0);
        self.barrel_elevation += (self.target_elevation - self.barrel_elevation) * smoothing;
        self.barrel_azimuth += (self.target_azimuth - self.barrel_azimuth) * smoothing;
    }

    /// Check if cannon is currently moving toward target
    fn is_aiming(&self) -> bool {
        let elev_diff = (self.target_elevation - self.barrel_elevation).abs();
        let az_diff = (self.target_azimuth - self.barrel_azimuth).abs();
        elev_diff > 0.001 || az_diff > 0.001
    }
}

// ============================================================================
// MESH GENERATION
// ============================================================================

struct Mesh {
    vertices: Vec<Vertex>,
    indices: Vec<u32>,
}

impl Mesh {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn merge(&mut self, other: &Mesh) {
        let base_idx = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base_idx));
    }
}

// ============================================================================
// PROCEDURAL NOISE FOR TERRAIN (Builder Mode)
// ============================================================================

/// Simple hash function for noise generation
fn hash_2d(x: f32, y: f32) -> f32 {
    let n = (x * 127.1 + y * 311.7).sin() * 43758.5453;
    n.fract()
}

/// Smoothstep interpolation
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// 2D value noise for terrain height
fn noise_2d(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;
    
    // Four corners
    let v00 = hash_2d(ix, iy);
    let v10 = hash_2d(ix + 1.0, iy);
    let v01 = hash_2d(ix, iy + 1.0);
    let v11 = hash_2d(ix + 1.0, iy + 1.0);
    
    // Bilinear interpolation with smoothstep
    let sx = smoothstep(fx);
    let sy = smoothstep(fy);
    
    let v0 = v00 + sx * (v10 - v00);
    let v1 = v01 + sx * (v11 - v01);
    
    v0 + sy * (v1 - v0)
}

/// Fractal Brownian Motion noise for natural terrain
fn fbm_noise(x: f32, z: f32, octaves: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    
    for _ in 0..octaves {
        value += amplitude * noise_2d(x * frequency, z * frequency);
        max_value += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    
    value / max_value
}

/// Ridged noise for rocky mountain formations
fn ridged_noise(x: f32, z: f32, octaves: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    
    for _ in 0..octaves {
        // Ridged: take absolute value and invert
        let n = 1.0 - noise_2d(x * frequency, z * frequency).abs();
        value += amplitude * n * n; // Square for sharper ridges
        max_value += amplitude;
        amplitude *= 0.5;
        frequency *= 2.2; // Slightly different scaling for variety
    }
    
    value / max_value
}

/// Turbulent noise for detailed rocky surfaces
fn turbulent_noise(x: f32, z: f32, octaves: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    
    for _ in 0..octaves {
        value += amplitude * noise_2d(x * frequency, z * frequency).abs();
        max_value += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    
    value / max_value
}

/// Water level constant (height below which is water)
const WATER_LEVEL: f32 = 0.5;

/// Terrain generation parameters - adjustable via UI
/// Values scaled from 0.0 to 1.0 for UI sliders
#[derive(Clone, Copy)]
struct TerrainParams {
    // Overall height multiplier (0.0 = flat, 1.0 = full height)
    pub height_scale: f32,
    
    // Mountains at edges (0.0 = none, 1.0 = dramatic peaks)
    pub mountains: f32,
    
    // Rocky formations (0.0 = smooth, 1.0 = very rocky)
    pub rocks: f32,
    
    // Rolling hills (0.0 = flat, 1.0 = hilly)
    pub hills: f32,
    
    // Surface detail (0.0 = smooth, 1.0 = detailed)
    pub detail: f32,
    
    // Water level (0.0 = no water, 1.0 = high water)
    pub water: f32,
}

impl Default for TerrainParams {
    fn default() -> Self {
        // User's preferred settings: H=58% M=52% R=37% L=100% D=100% W=46%
        Self {
            height_scale: 0.58,  // 58% of max height
            mountains: 0.52,     // Strong mountains
            rocks: 0.37,         // Medium rocky texture
            hills: 1.0,          // Full rolling hills
            detail: 1.0,         // Full surface detail
            water: 0.46,         // 46% water level
        }
    }
}

/// Global terrain params - can be modified at runtime
/// User's preferred settings: H=58% M=52% R=37% L=100% D=100% W=46%
static mut TERRAIN_PARAMS: TerrainParams = TerrainParams {
    height_scale: 0.58,
    mountains: 0.52,
    rocks: 0.37,
    hills: 1.0,
    detail: 1.0,
    water: 0.46,
};

fn get_terrain_params() -> TerrainParams {
    unsafe { TERRAIN_PARAMS }
}

// ============================================================================
// ON-SCREEN UI SLIDER SYSTEM
// ============================================================================

/// A single UI slider for terrain editing
#[derive(Clone, Copy)]
struct UISlider {
    /// Label for this slider
    label: &'static str,
    /// Screen position (pixels from top-left)
    x: f32,
    y: f32,
    /// Slider dimensions
    width: f32,
    height: f32,
    /// Current value (0.0 to 1.0)
    value: f32,
    /// Color of the slider bar
    color: [f32; 4],
}

impl UISlider {
    fn new(label: &'static str, x: f32, y: f32, value: f32, color: [f32; 4]) -> Self {
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
    fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width &&
        py >= self.y && py <= self.y + self.height
    }
    
    /// Get value from mouse X position within slider
    fn value_from_x(&self, px: f32) -> f32 {
        ((px - self.x) / self.width).clamp(0.0, 1.0)
    }
}

/// The terrain editor UI panel
struct TerrainEditorUI {
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
                UISlider::new("Height", panel_x, panel_y, params.height_scale, [0.4, 0.7, 1.0, 1.0]),
                UISlider::new("Mountains", panel_x, panel_y + spacing, params.mountains, [0.7, 0.5, 0.3, 1.0]),
                UISlider::new("Rocks", panel_x, panel_y + spacing * 2.0, params.rocks, [0.6, 0.6, 0.6, 1.0]),
                UISlider::new("Hills", panel_x, panel_y + spacing * 3.0, params.hills, [0.5, 0.8, 0.4, 1.0]),
                UISlider::new("Detail", panel_x, panel_y + spacing * 4.0, params.detail, [0.8, 0.7, 0.3, 1.0]),
                UISlider::new("Water", panel_x, panel_y + spacing * 5.0, params.water, [0.3, 0.6, 0.9, 1.0]),
            ],
            dragging_slider: -1,
            apply_button: (panel_x, panel_y + spacing * 6.0 + 20.0, 200.0, 30.0),
        }
    }
}

impl TerrainEditorUI {
    /// Toggle visibility
    fn toggle(&mut self) {
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
    fn on_mouse_press(&mut self, x: f32, y: f32) -> bool {
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
    fn on_mouse_release(&mut self, x: f32, y: f32) -> bool {
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
    fn on_mouse_move(&mut self, x: f32, _y: f32) {
        if self.dragging_slider >= 0 && self.dragging_slider < 6 {
            let idx = self.dragging_slider as usize;
            self.sliders[idx].value = self.sliders[idx].value_from_x(x);
        }
    }
    
    /// Apply slider values to terrain params
    fn apply_to_terrain(&self) {
        let params = TerrainParams {
            height_scale: self.sliders[0].value,
            mountains: self.sliders[1].value,
            rocks: self.sliders[2].value,
            hills: self.sliders[3].value,
            detail: self.sliders[4].value,
            water: self.sliders[5].value,
        };
        set_terrain_params(params);
        println!("Applied terrain settings: H={:.0}% M={:.0}% R={:.0}% L={:.0}% D={:.0}% W={:.0}%",
            params.height_scale * 100.0,
            params.mountains * 100.0,
            params.rocks * 100.0,
            params.hills * 100.0,
            params.detail * 100.0,
            params.water * 100.0);
    }
    
    /// Generate mesh for UI rendering (2D quads)
    fn generate_ui_mesh(&self, screen_width: f32, screen_height: f32) -> Mesh {
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
                0.0
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
        add_quad(&mut vertices, &mut indices, 
            to_ndc(self.panel_x - 10.0, self.panel_y - 50.0),
            to_ndc(self.panel_x + panel_w, self.panel_y - 50.0),
            to_ndc(self.panel_x + panel_w, self.panel_y + panel_h),
            to_ndc(self.panel_x - 10.0, self.panel_y + panel_h),
            bg_color);
        
        // Draw title "TERRAIN"
        draw_text(&mut vertices, &mut indices, "TERRAIN", 
            self.panel_x + 80.0, self.panel_y - 35.0, 2.5, title_color, screen_width, screen_height);
        
        // Draw each slider with label
        for (i, slider) in self.sliders.iter().enumerate() {
            // Draw label above slider
            draw_text(&mut vertices, &mut indices, labels[i],
                slider.x, slider.y - 18.0, 2.0, text_color, screen_width, screen_height);
            
            // Background track
            let track_color = [0.25, 0.25, 0.3, 1.0];
            add_quad(&mut vertices, &mut indices,
                to_ndc(slider.x, slider.y),
                to_ndc(slider.x + slider.width, slider.y),
                to_ndc(slider.x + slider.width, slider.y + slider.height),
                to_ndc(slider.x, slider.y + slider.height),
                track_color);
            
            // Value fill
            let fill_width = slider.width * slider.value;
            add_quad(&mut vertices, &mut indices,
                to_ndc(slider.x, slider.y),
                to_ndc(slider.x + fill_width, slider.y),
                to_ndc(slider.x + fill_width, slider.y + slider.height),
                to_ndc(slider.x, slider.y + slider.height),
                slider.color);
            
            // Handle indicator
            let handle_x = slider.x + fill_width - 4.0;
            let handle_color = [1.0, 1.0, 1.0, 1.0];
            add_quad(&mut vertices, &mut indices,
                to_ndc(handle_x, slider.y - 2.0),
                to_ndc(handle_x + 8.0, slider.y - 2.0),
                to_ndc(handle_x + 8.0, slider.y + slider.height + 2.0),
                to_ndc(handle_x, slider.y + slider.height + 2.0),
                handle_color);
        }
        
        // Draw apply button
        let (bx, by, bw, bh) = self.apply_button;
        let button_color = [0.3, 0.7, 0.4, 1.0];
        add_quad(&mut vertices, &mut indices,
            to_ndc(bx, by),
            to_ndc(bx + bw, by),
            to_ndc(bx + bw, by + bh),
            to_ndc(bx, by + bh),
            button_color);
        
        // Draw "APPLY" text on button
        draw_text(&mut vertices, &mut indices, "APPLY",
            bx + 70.0, by + 8.0, 2.0, [1.0, 1.0, 1.0, 1.0], screen_width, screen_height);
        
        Mesh { vertices, indices }
    }
}

impl BuildToolbar {
    /// Generate UI mesh for the toolbar (Minecraft-style hotbar)
    fn generate_ui_mesh(&self, screen_width: f32, screen_height: f32) -> Mesh {
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
        let toolbar_height = slot_size + 40.0; // Extra space for labels
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
            
            // Slot background (highlight if selected)
            let slot_color = if i == self.selected_shape {
                [0.4, 0.6, 0.9, 1.0] // Blue highlight for selected
            } else {
                [0.2, 0.2, 0.25, 1.0] // Dark gray for unselected
            };
            
            add_quad(&mut vertices, &mut indices,
                to_ndc(slot_x, slot_y),
                to_ndc(slot_x + slot_size, slot_y),
                to_ndc(slot_x + slot_size, slot_y + slot_size),
                to_ndc(slot_x, slot_y + slot_size),
                slot_color);
            
            // Draw shape icon
            let icon_color = [0.9, 0.9, 0.9, 1.0];
            let center_x = slot_x + slot_size / 2.0;
            let center_y = slot_y + slot_size / 2.0;
            let icon_size = slot_size * 0.6;
            
            match i {
                0 => { // Cube - square
                    let half = icon_size / 2.0;
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - half, center_y - half),
                        to_ndc(center_x + half, center_y - half),
                        to_ndc(center_x + half, center_y + half),
                        to_ndc(center_x - half, center_y + half),
                        icon_color);
                }
                1 => { // Cylinder - tall rectangle
                    let w = icon_size * 0.4;
                    let h = icon_size * 0.8;
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y + h/2.0),
                        to_ndc(center_x - w/2.0, center_y + h/2.0),
                        icon_color);
                    // Top ellipse hint
                    let ew = w * 0.8;
                    let eh = h * 0.15;
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - ew, center_y - h/2.0 - eh),
                        to_ndc(center_x + ew, center_y - h/2.0 - eh),
                        to_ndc(center_x + w/2.0, center_y - h/2.0),
                        to_ndc(center_x - w/2.0, center_y - h/2.0),
                        icon_color);
                }
                2 => { // Sphere - circle approximated by octagon
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
                3 => { // Dome - half circle
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
                    // Base line
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - icon_size * 0.4, center_y + r * 0.3),
                        to_ndc(center_x + icon_size * 0.4, center_y + r * 0.3),
                        to_ndc(center_x + icon_size * 0.4, center_y + r * 0.3 + 3.0),
                        to_ndc(center_x - icon_size * 0.4, center_y + r * 0.3 + 3.0),
                        icon_color);
                }
                4 => { // Arch - inverted U shape
                    let w = icon_size * 0.7;
                    let h = icon_size * 0.8;
                    let thickness = icon_size * 0.15;
                    // Left pillar
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - w/2.0, center_y - h/2.0),
                        to_ndc(center_x - w/2.0 + thickness, center_y - h/2.0),
                        to_ndc(center_x - w/2.0 + thickness, center_y + h/2.0),
                        to_ndc(center_x - w/2.0, center_y + h/2.0),
                        icon_color);
                    // Right pillar
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x + w/2.0 - thickness, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y + h/2.0),
                        to_ndc(center_x + w/2.0 - thickness, center_y + h/2.0),
                        icon_color);
                    // Top bar
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y - h/2.0),
                        to_ndc(center_x + w/2.0, center_y - h/2.0 + thickness),
                        to_ndc(center_x - w/2.0, center_y - h/2.0 + thickness),
                        icon_color);
                }
                5 => { // Wedge - triangle
                    let half = icon_size * 0.4;
                    // Draw as two triangles forming a triangle (using quads)
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - half, center_y + half),
                        to_ndc(center_x + half, center_y + half),
                        to_ndc(center_x, center_y - half),
                        to_ndc(center_x - half, center_y + half), // degenerate to make triangle
                        icon_color);
                }
                6 => { // Bridge - two squares connected by a line
                    let s = icon_size * 0.25;
                    let gap = icon_size * 0.3;
                    // Left square
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x - gap - s, center_y - s),
                        to_ndc(center_x - gap, center_y - s),
                        to_ndc(center_x - gap, center_y + s),
                        to_ndc(center_x - gap - s, center_y + s),
                        icon_color);
                    // Right square
                    add_quad(&mut vertices, &mut indices,
                        to_ndc(center_x + gap, center_y - s),
                        to_ndc(center_x + gap + s, center_y - s),
                        to_ndc(center_x + gap + s, center_y + s),
                        to_ndc(center_x + gap, center_y + s),
                        icon_color);
                    // Connecting line
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
            
            // Draw number label below slot
            let num_str = format!("{}", i + 1);
            draw_text(&mut vertices, &mut indices, &num_str,
                slot_x + slot_size / 2.0 - 3.0, slot_y + slot_size + 5.0,
                1.5, [0.7, 0.7, 0.7, 1.0], screen_width, screen_height);
        }
        
        // Draw info panel on the right
        let info_x = toolbar_x + toolbar_width + 15.0;
        
        // Material indicator
        let mat_text = format!("MAT {}", self.selected_material);
        draw_text(&mut vertices, &mut indices, &mat_text,
            info_x, toolbar_y + 10.0,
            1.5, [0.8, 0.8, 0.5, 1.0], screen_width, screen_height);
        
        // Height indicator
        let height_text = format!("H {:.0}", self.build_height);
        draw_text(&mut vertices, &mut indices, &height_text,
            info_x, toolbar_y + 28.0,
            1.5, [0.5, 0.8, 0.5, 1.0], screen_width, screen_height);
        
        // Bridge mode indicator
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
            draw_text(&mut vertices, &mut indices, bridge_text,
                info_x, toolbar_y + 46.0,
                1.5, [1.0, 0.6, 0.2, 1.0], screen_width, screen_height);
        }
        
        Mesh { vertices, indices }
    }
}

impl StartOverlay {
    /// Generate UI mesh for the start overlay
    fn generate_ui_mesh(&self, screen_width: f32, screen_height: f32) -> Mesh {
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

/// Helper to add a quad to the mesh
fn add_quad(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    tl: [f32; 3], tr: [f32; 3], br: [f32; 3], bl: [f32; 3],
    color: [f32; 4]
) {
    let base = vertices.len() as u32;
    let normal = [0.0, 0.0, 1.0];
    
    vertices.push(Vertex { position: tl, normal, color });
    vertices.push(Vertex { position: tr, normal, color });
    vertices.push(Vertex { position: br, normal, color });
    vertices.push(Vertex { position: bl, normal, color });
    
    indices.push(base);
    indices.push(base + 1);
    indices.push(base + 2);
    indices.push(base);
    indices.push(base + 2);
    indices.push(base + 3);
}

// ============================================================================
// SIMPLE PIXEL FONT FOR UI TEXT
// ============================================================================
// Each character is 5x7 pixels, stored as a bitmask array
// 1 = pixel on, 0 = pixel off

fn get_char_bitmap(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
        'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
        'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        ' ' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000],
        _   => [0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b11111, 0b11111], // Unknown = filled box
    }
}

/// Draw text at screen position using pixel font
fn draw_text(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    text: &str,
    x: f32,
    y: f32,
    scale: f32,
    color: [f32; 4],
    screen_width: f32,
    screen_height: f32,
) {
    let to_ndc = |px: f32, py: f32| -> [f32; 3] {
        [
            (px / screen_width) * 2.0 - 1.0,
            1.0 - (py / screen_height) * 2.0,
            0.0
        ]
    };
    
    let pixel_size = scale;
    let char_width = 6.0 * scale; // 5 pixels + 1 spacing
    
    for (char_idx, c) in text.chars().enumerate() {
        let bitmap = get_char_bitmap(c);
        let char_x = x + (char_idx as f32) * char_width;
        
        for (row, &row_bits) in bitmap.iter().enumerate() {
            for col in 0..5 {
                // Check if pixel is set (bit is 1)
                if (row_bits >> (4 - col)) & 1 == 1 {
                    let px = char_x + (col as f32) * pixel_size;
                    let py = y + (row as f32) * pixel_size;
                    
                    add_quad(
                        vertices, indices,
                        to_ndc(px, py),
                        to_ndc(px + pixel_size, py),
                        to_ndc(px + pixel_size, py + pixel_size),
                        to_ndc(px, py + pixel_size),
                        color
                    );
                }
            }
        }
    }
}

fn set_terrain_params(params: TerrainParams) {
    unsafe { TERRAIN_PARAMS = params; }
}

/// Sample terrain height using UI-adjustable parameters
fn terrain_height_at(x: f32, z: f32, base_y: f32) -> f32 {
    let params = get_terrain_params();
    
    // Max amplitudes (when slider = 1.0)
    const MAX_MOUNTAIN: f32 = 8.0;  // Reduced from 15
    const MAX_ROCK: f32 = 3.0;      // Reduced from 8
    const MAX_HILL: f32 = 2.0;      // Reduced from 3
    const MAX_DETAIL: f32 = 0.5;    // Reduced from 0.8
    
    // Distance from center for edge effects
    let dist_from_center = (x * x + z * z).sqrt() / 30.0;
    let edge_factor = (dist_from_center * 1.2).min(1.0);
    
    let mut height = 0.0;
    
    // Layer 1: Mountains at edges
    if params.mountains > 0.01 {
        let mountain_noise = ridged_noise(x * 0.04 + 100.0, z * 0.04 + 100.0, 4);
        height += mountain_noise * MAX_MOUNTAIN * params.mountains * edge_factor;
    }
    
    // Layer 2: Rocky formations
    if params.rocks > 0.01 {
        let rock_noise = turbulent_noise(x * 0.1 + 50.0, z * 0.1 + 50.0, 3);
        height += rock_noise * MAX_ROCK * params.rocks;
    }
    
    // Layer 3: Gentle hills
    if params.hills > 0.01 {
        let hill_noise = fbm_noise(x * 0.08, z * 0.08, 3);
        height += hill_noise * MAX_HILL * params.hills;
    }
    
    // Layer 4: Surface detail
    if params.detail > 0.01 {
        let detail_noise = fbm_noise(x * 0.3 + 200.0, z * 0.3 + 200.0, 2);
        height += detail_noise * MAX_DETAIL * params.detail;
    }
    
    // Apply global height scale
    height *= params.height_scale;
    
    base_y + height
}

/// UE5-style terrain color with procedural variation
/// Uses multi-layer blending based on height, slope, and noise
fn terrain_color_at(height: f32, normal: Vec3, base_y: f32) -> [f32; 4] {
    let params = get_terrain_params();
    let relative_height = height - base_y;
    let slope = 1.0 - normal.y.abs(); // 0 = flat, 1 = vertical
    
    // Water level based on params
    let water_level = params.water * 2.0;
    
    // ============================================================
    // UE5-STYLE COLOR PALETTE (photorealistic, slightly desaturated)
    // ============================================================
    
    // Grass layers (with variation based on moisture)
    let grass_dark = [0.12, 0.22, 0.06, 1.0];      // Dark grass (wet/shadowed)
    let grass_mid = [0.18, 0.32, 0.10, 1.0];       // Medium grass
    let grass_light = [0.28, 0.42, 0.15, 1.0];     // Grass tips (sunlit)
    let grass_dry = [0.35, 0.33, 0.18, 1.0];       // Dry/dead grass patches
    
    // Rock layers
    let rock_dark = [0.15, 0.13, 0.11, 1.0];       // Dark rock crevices
    let rock_mid = [0.32, 0.29, 0.26, 1.0];        // Mid-tone rock
    let rock_light = [0.48, 0.45, 0.42, 1.0];      // Exposed rock faces
    let rock_moss = [0.20, 0.26, 0.14, 1.0];       // Mossy rock
    
    // Sand/dirt
    let sand_wet = [0.30, 0.25, 0.18, 1.0];        // Wet sand (near water)
    let sand_dry = [0.55, 0.48, 0.38, 1.0];        // Dry sand
    let dirt_base = [0.24, 0.18, 0.12, 1.0];       // Rich soil
    let mud_wet = [0.16, 0.13, 0.10, 1.0];         // Wet mud
    
    // Water
    let water_shallow = [0.12, 0.30, 0.35, 0.90];  // Shallow water (teal)
    let water_deep = [0.08, 0.18, 0.25, 0.95];     // Deep water
    
    // ============================================================
    // PROCEDURAL VARIATION (position-based noise)
    // ============================================================
    // Use simple hash for variation based on world position
    let px = (height * 7.3 + relative_height * 13.7).sin() * 0.5 + 0.5;
    let py = (relative_height * 11.1 + slope * 17.3).cos() * 0.5 + 0.5;
    let noise = (px + py) * 0.5; // 0-1 variation
    
    // ============================================================
    // WATER RENDERING
    // ============================================================
    if relative_height < water_level && params.water > 0.01 {
        let depth = (water_level - relative_height) / 2.0;
        let water_blend = depth.clamp(0.0, 1.0);
        return blend_colors(&water_shallow, &water_deep, water_blend);
    }
    
    // ============================================================
    // BEACH/SHORE ZONE (sand near water)
    // ============================================================
    let beach_width = 0.8;
    let beach_zone = ((relative_height - water_level) / beach_width).clamp(0.0, 1.0);
    if beach_zone < 1.0 && params.water > 0.01 {
        // Near water: wet sand transitioning to dry
        let sand = blend_colors(&sand_wet, &sand_dry, beach_zone);
        // Add some mud near the very edge
        let mud_factor = (1.0 - beach_zone) * 0.4;
        return blend_colors(&sand, &mud_wet, mud_factor);
    }
    
    // ============================================================
    // HEIGHT AND SLOPE FACTORS
    // ============================================================
    let height_factor = ((relative_height - water_level) / 8.0).clamp(0.0, 1.0);
    let slope_sharp = smooth_step(0.35, 0.65, slope); // Smooth rock transition
    
    // ============================================================
    // GRASS LAYER (flat areas)
    // ============================================================
    // Grass with color variation
    let grass_variation = noise;
    let mut grass = blend_colors(&grass_dark, &grass_mid, grass_variation);
    grass = blend_colors(&grass, &grass_light, (noise * 0.7).clamp(0.0, 1.0));
    
    // Add dry patches at higher elevations and steep areas
    let dry_factor = (height_factor * 0.6 + slope * 0.3 + (noise - 0.5).abs() * 0.4).clamp(0.0, 1.0);
    grass = blend_colors(&grass, &grass_dry, dry_factor * 0.5);
    
    // ============================================================
    // ROCK LAYER (steep slopes and high altitude)
    // ============================================================
    // Rock with crack/variation
    let mut rock = blend_colors(&rock_dark, &rock_mid, noise);
    rock = blend_colors(&rock, &rock_light, ((noise - 0.3) * 2.0).clamp(0.0, 1.0));
    
    // Add moss on north-facing and moist rock
    let north_facing = (-normal.z * 0.5 + 0.5).clamp(0.0, 1.0);
    let moisture = (1.0 - height_factor) * (1.0 - slope); // Low and flat = moist
    let moss_factor = north_facing * moisture * 0.6;
    rock = blend_colors(&rock, &rock_moss, moss_factor);
    
    // ============================================================
    // DIRT/TRANSITION LAYER
    // ============================================================
    let dirt = blend_colors(&dirt_base, &sand_dry, noise * 0.3);
    
    // ============================================================
    // FINAL BLENDING
    // ============================================================
    // Steep slopes get rock
    let mut result = blend_colors(&grass, &rock, slope_sharp);
    
    // High altitude gets more rock and dirt
    let altitude_rock = (height_factor * 1.5).clamp(0.0, 1.0);
    result = blend_colors(&result, &rock, altitude_rock * (1.0 - slope_sharp) * 0.5);
    
    // Very high gets dirt/sand
    let altitude_dirt = ((height_factor - 0.6) * 2.5).clamp(0.0, 1.0);
    result = blend_colors(&result, &dirt, altitude_dirt * 0.4);
    
    result
}

/// Smooth step function for natural transitions
fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Helper to blend two colors
fn blend_colors(a: &[f32; 4], b: &[f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Compute normal from terrain height gradient
fn terrain_normal_at(x: f32, z: f32, base_y: f32) -> Vec3 {
    let epsilon = 0.2; // Slightly larger for smoother normals
    let h_center = terrain_height_at(x, z, base_y);
    let h_dx = terrain_height_at(x + epsilon, z, base_y);
    let h_dz = terrain_height_at(x, z + epsilon, base_y);
    
    let tangent_x = Vec3::new(epsilon, h_dx - h_center, 0.0);
    let tangent_z = Vec3::new(0.0, h_dz - h_center, epsilon);
    
    tangent_z.cross(tangent_x).normalize()
}

/// Check if a point (relative to hex center) is inside a regular hexagon
/// Uses a simple circular approximation for smoother terrain edges
fn is_inside_hexagon(dx: f32, dz: f32, radius: f32) -> bool {
    // Simple circular check gives smoother terrain without jagged edges
    // Use a slightly larger radius to ensure full coverage
    let dist_sq = dx * dx + dz * dz;
    dist_sq <= radius * radius
}

/// Generate an elevated hexagonal terrain with procedural mountains, rocks, and water
/// Uses a proper subdivided plane approach with dynamic height-based coloring
fn generate_elevated_hex_terrain(center: Vec3, radius: f32, _color: [f32; 4], subdivisions: u32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Use higher subdivisions for detailed terrain (64 for good detail)
    let actual_subdivisions = subdivisions.max(64);
    
    // Create a dense grid that covers the hex area
    let grid_count = actual_subdivisions + 1;
    let cell_size = (radius * 2.0) / (actual_subdivisions as f32);
    let half_size = radius;
    
    // Generate vertices in a regular grid pattern with dynamic coloring
    for gz in 0..grid_count {
        for gx in 0..grid_count {
            // Calculate world position relative to center
            let local_x = (gx as f32) * cell_size - half_size;
            let local_z = (gz as f32) * cell_size - half_size;
            let x = center.x + local_x;
            let z = center.z + local_z;
            
            // Sample terrain height and normal
            let y = terrain_height_at(x, z, center.y);
            let normal = terrain_normal_at(x, z, center.y);
            
            // Get dynamic color based on height and slope
            let color = terrain_color_at(y, normal, center.y);
            
            vertices.push(Vertex {
                position: [x, y, z],
                normal: normal.to_array(),
                color,
            });
        }
    }
    
    // Generate triangles (two per quad)
    // Each quad connects vertices: (x,z), (x+1,z), (x,z+1), (x+1,z+1)
    for gz in 0..actual_subdivisions {
        for gx in 0..actual_subdivisions {
            let i00 = gz * grid_count + gx;           // bottom-left
            let i10 = gz * grid_count + (gx + 1);     // bottom-right
            let i01 = (gz + 1) * grid_count + gx;     // top-left
            let i11 = (gz + 1) * grid_count + (gx + 1); // top-right
            
            // Calculate center of this quad to check if inside hex
            let cx = center.x + ((gx as f32 + 0.5) * cell_size - half_size);
            let cz = center.z + ((gz as f32 + 0.5) * cell_size - half_size);
            let dx = cx - center.x;
            let dz = cz - center.z;
            
            // Only add triangles if quad center is within the hexagonal boundary
            if is_inside_hexagon(dx, dz, radius) {
                // Triangle 1: bottom-left, top-left, bottom-right
                indices.push(i00);
                indices.push(i01);
                indices.push(i10);
                
                // Triangle 2: bottom-right, top-left, top-right
                indices.push(i10);
                indices.push(i01);
                indices.push(i11);
            }
        }
    }
    
    Mesh { vertices, indices }
}

/// Generate a flat water plane at the water level
fn generate_water_plane(center: Vec3, radius: f32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    let water_color = [0.3, 0.65, 0.6, 0.85]; // Teal, slightly transparent
    let water_y = center.y + WATER_LEVEL;
    let normal = [0.0, 1.0, 0.0]; // Flat water pointing up
    
    let subdivisions = 32u32;
    let grid_count = subdivisions + 1;
    let cell_size = (radius * 2.0) / (subdivisions as f32);
    let half_size = radius;
    
    // Generate water vertices
    for gz in 0..grid_count {
        for gx in 0..grid_count {
            let local_x = (gx as f32) * cell_size - half_size;
            let local_z = (gz as f32) * cell_size - half_size;
            let x = center.x + local_x;
            let z = center.z + local_z;
            
            vertices.push(Vertex {
                position: [x, water_y, z],
                normal,
                color: water_color,
            });
        }
    }
    
    // Generate triangles for water
    for gz in 0..subdivisions {
        for gx in 0..subdivisions {
            let i00 = gz * grid_count + gx;
            let i10 = gz * grid_count + (gx + 1);
            let i01 = (gz + 1) * grid_count + gx;
            let i11 = (gz + 1) * grid_count + (gx + 1);
            
            let cx = center.x + ((gx as f32 + 0.5) * cell_size - half_size);
            let cz = center.z + ((gz as f32 + 0.5) * cell_size - half_size);
            let dx = cx - center.x;
            let dz = cz - center.z;
            
            if is_inside_hexagon(dx, dz, radius * 0.95) {
                indices.push(i00);
                indices.push(i01);
                indices.push(i10);
                indices.push(i10);
                indices.push(i01);
                indices.push(i11);
            }
        }
    }
    
    Mesh { vertices, indices }
}

// ============================================================================
// PROCEDURAL TREE GENERATION
// ============================================================================

/// Simple tree structure for harvesting
#[derive(Clone)]
struct PlacedTree {
    position: Vec3,
    height: f32,
    trunk_radius: f32,
    foliage_radius: f32,
    harvested: bool,
}

/// Generate procedural trees on terrain using noise-based distribution
fn generate_trees_on_terrain(center: Vec3, radius: f32, density: f32, seed_offset: f32) -> Vec<PlacedTree> {
    let mut trees = Vec::new();
    
    // Use noise to determine tree placement - creates natural-looking clusters
    let spacing = 3.0; // Minimum spacing between potential tree positions
    let grid_size = (radius * 2.0 / spacing) as i32;
    
    for gz in -grid_size..=grid_size {
        for gx in -grid_size..=grid_size {
            let base_x = center.x + (gx as f32) * spacing;
            let base_z = center.z + (gz as f32) * spacing;
            
            // Add jitter for natural look
            let jitter_x = fbm_noise((base_x + seed_offset) * 0.5, base_z * 0.5, 2) * spacing * 0.4;
            let jitter_z = fbm_noise(base_x * 0.5, (base_z + seed_offset) * 0.5, 2) * spacing * 0.4;
            let x = base_x + jitter_x;
            let z = base_z + jitter_z;
            
            // Check if inside hexagonal terrain boundary
            let dx = x - center.x;
            let dz = z - center.z;
            // Use hexagonal check with slightly smaller radius to keep trees away from edges
            if !is_inside_hexagon(dx, dz, radius * 0.85) {
                continue; // Keep trees inside hex and away from edges
            }
            
            // Use noise to decide if tree should be placed here
            let tree_noise = fbm_noise((x + seed_offset * 2.0) * 0.1, z * 0.1, 3);
            if tree_noise < density {
                continue; // No tree at this location
            }
            
            // Get terrain height
            let terrain_y = terrain_height_at(x, z, center.y);
            
            // Skip if below water level - no underwater trees!
            let relative_height = terrain_y - center.y;
            if relative_height < WATER_LEVEL + 0.5 {
                continue;
            }
            
            // Only place trees on relatively flat areas (slope < 0.5)
            let normal = terrain_normal_at(x, z, center.y);
            let slope = 1.0 - normal.y.abs();
            if slope > 0.5 {
                continue; // No trees on steep cliffs
            }
            
            // Tree size variation based on noise
            let size_noise = fbm_noise(x * 0.2, z * 0.2, 2);
            let height = 2.0 + size_noise * 3.0; // 2-5 meters tall
            let trunk_radius = 0.15 + size_noise * 0.1;
            let foliage_radius = 0.8 + size_noise * 0.6;
            
            trees.push(PlacedTree {
                position: Vec3::new(x, terrain_y, z),
                height,
                trunk_radius,
                foliage_radius,
                harvested: false,
            });
        }
    }
    
    trees
}

/// Generate mesh for a single tree (trunk + foliage cone)
fn generate_tree_mesh(tree: &PlacedTree) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    let trunk_color = [0.4, 0.25, 0.15, 1.0]; // Brown
    let foliage_color = [0.2, 0.5, 0.2, 1.0]; // Dark green
    
    let pos = tree.position;
    let segments = 6; // Hexagonal cross-section for efficiency
    
    // === TRUNK (cylinder) ===
    let trunk_height = tree.height * 0.4;
    let trunk_base_idx = vertices.len() as u32;
    
    // Bottom ring
    for i in 0..segments {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let nx = angle.cos();
        let nz = angle.sin();
        let x = pos.x + nx * tree.trunk_radius;
        let z = pos.z + nz * tree.trunk_radius;
        vertices.push(Vertex {
            position: [x, pos.y, z],
            normal: [nx, 0.0, nz],
            color: trunk_color,
        });
    }
    
    // Top ring
    for i in 0..segments {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let nx = angle.cos();
        let nz = angle.sin();
        let x = pos.x + nx * tree.trunk_radius;
        let z = pos.z + nz * tree.trunk_radius;
        vertices.push(Vertex {
            position: [x, pos.y + trunk_height, z],
            normal: [nx, 0.0, nz],
            color: trunk_color,
        });
    }
    
    // Trunk side triangles
    for i in 0..segments {
        let i0 = trunk_base_idx + i as u32;
        let i1 = trunk_base_idx + ((i + 1) % segments) as u32;
        let i2 = trunk_base_idx + segments as u32 + i as u32;
        let i3 = trunk_base_idx + segments as u32 + ((i + 1) % segments) as u32;
        
        indices.extend_from_slice(&[i0, i2, i1]);
        indices.extend_from_slice(&[i1, i2, i3]);
    }
    
    // === FOLIAGE (cone) ===
    let foliage_base_y = pos.y + trunk_height;
    let foliage_top_y = pos.y + tree.height;
    let foliage_base_idx = vertices.len() as u32;
    
    // Foliage base ring
    for i in 0..segments {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let nx = angle.cos();
        let nz = angle.sin();
        let x = pos.x + nx * tree.foliage_radius;
        let z = pos.z + nz * tree.foliage_radius;
        
        // Normal points outward and upward for cone
        let normal = Vec3::new(nx, 0.5, nz).normalize();
        vertices.push(Vertex {
            position: [x, foliage_base_y, z],
            normal: normal.to_array(),
            color: foliage_color,
        });
    }
    
    // Foliage apex (top point)
    let apex_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [pos.x, foliage_top_y, pos.z],
        normal: [0.0, 1.0, 0.0],
        color: foliage_color,
    });
    
    // Foliage triangles (cone sides)
    for i in 0..segments {
        let i0 = foliage_base_idx + i as u32;
        let i1 = foliage_base_idx + ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[i0, apex_idx, i1]);
    }
    
    // Foliage bottom cap
    let bottom_center_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [pos.x, foliage_base_y, pos.z],
        normal: [0.0, -1.0, 0.0],
        color: foliage_color,
    });
    
    for i in 0..segments {
        let i0 = foliage_base_idx + i as u32;
        let i1 = foliage_base_idx + ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[bottom_center_idx, i1, i0]);
    }
    
    Mesh { vertices, indices }
}

/// Generate combined mesh for all trees
fn generate_all_trees_mesh(trees: &[PlacedTree]) -> Mesh {
    let mut combined = Mesh::new();
    for tree in trees {
        if !tree.harvested {
            let tree_mesh = generate_tree_mesh(tree);
            combined.merge(&tree_mesh);
        }
    }
    combined
}

/// Generate a flat hexagonal platform (kept for reference/simple cases)
#[allow(dead_code)]
fn generate_hex_platform(center: Vec3, radius: f32, color: [f32; 4]) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Center vertex
    vertices.push(Vertex {
        position: [center.x, center.y, center.z],
        normal: [0.0, 1.0, 0.0],
        color,
    });

    // 6 corner vertices
    for i in 0..6 {
        let angle = (i as f32) * std::f32::consts::PI / 3.0;
        let x = center.x + radius * angle.cos();
        let z = center.z + radius * angle.sin();
        vertices.push(Vertex {
            position: [x, center.y, z],
            normal: [0.0, 1.0, 0.0],
            color,
        });
    }

    // 6 triangles (counter-clockwise winding for front-face when viewed from above)
    for i in 0..6 {
        indices.push(0);
        indices.push(((i + 1) % 6 + 1) as u32);  // next corner first
        indices.push((i + 1) as u32);             // current corner second
    }

    Mesh { vertices, indices }
}

/// Generate cannon mesh (simplified box + cylinder representation)
fn generate_cannon_mesh(cannon: &Cannon) -> Mesh {
    let mut mesh = Mesh::new();
    let pos = cannon.position;
    let dir = cannon.get_barrel_direction();
    let color = [0.3, 0.3, 0.3, 1.0]; // Gray metal

    // Cannon body (box)
    let body_size = Vec3::new(1.0, 0.5, 1.5);
    let body_mesh = generate_box(pos, body_size, color);
    mesh.merge(&body_mesh);

    // Barrel (elongated box for simplicity)
    let barrel_center = pos + dir * (cannon.barrel_length / 2.0);
    let barrel_up = Vec3::Y;
    let _barrel_right = dir.cross(barrel_up).normalize();
    let barrel_size = Vec3::new(0.3, 0.3, cannon.barrel_length);

    // Generate rotated barrel
    let barrel_mesh = generate_oriented_box(barrel_center, barrel_size, dir, barrel_up, [0.2, 0.2, 0.2, 1.0]);
    mesh.merge(&barrel_mesh);

    mesh
}

/// Generate a simple axis-aligned box
fn generate_box(center: Vec3, half_extents: Vec3, color: [f32; 4]) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let (hx, hy, hz) = (half_extents.x, half_extents.y, half_extents.z);

    // 8 corners
    let corners = [
        Vec3::new(-hx, -hy, -hz),
        Vec3::new(hx, -hy, -hz),
        Vec3::new(hx, hy, -hz),
        Vec3::new(-hx, hy, -hz),
        Vec3::new(-hx, -hy, hz),
        Vec3::new(hx, -hy, hz),
        Vec3::new(hx, hy, hz),
        Vec3::new(-hx, hy, hz),
    ];

    // 6 faces with normals
    let faces = [
        ([0, 1, 2, 3], Vec3::new(0.0, 0.0, -1.0)), // Front
        ([5, 4, 7, 6], Vec3::new(0.0, 0.0, 1.0)),  // Back
        ([4, 0, 3, 7], Vec3::new(-1.0, 0.0, 0.0)), // Left
        ([1, 5, 6, 2], Vec3::new(1.0, 0.0, 0.0)),  // Right
        ([3, 2, 6, 7], Vec3::new(0.0, 1.0, 0.0)),  // Top
        ([4, 5, 1, 0], Vec3::new(0.0, -1.0, 0.0)), // Bottom
    ];

    for (face_indices, normal) in &faces {
        let base = vertices.len() as u32;
        for &i in face_indices {
            let pos = center + corners[i];
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [normal.x, normal.y, normal.z],
                color,
            });
        }
        // Two triangles per face
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh { vertices, indices }
}

/// Generate an oriented box (for barrel)
fn generate_oriented_box(
    center: Vec3,
    size: Vec3,
    forward: Vec3,
    up: Vec3,
    color: [f32; 4],
) -> Mesh {
    let right = forward.cross(up).normalize();
    let up = right.cross(forward).normalize();

    let (hx, hy, hz) = (size.x / 2.0, size.y / 2.0, size.z / 2.0);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Local to world transform
    let transform = |local: Vec3| -> Vec3 { center + right * local.x + up * local.y + forward * local.z };

    // 8 corners in local space
    let corners = [
        Vec3::new(-hx, -hy, -hz),
        Vec3::new(hx, -hy, -hz),
        Vec3::new(hx, hy, -hz),
        Vec3::new(-hx, hy, -hz),
        Vec3::new(-hx, -hy, hz),
        Vec3::new(hx, -hy, hz),
        Vec3::new(hx, hy, hz),
        Vec3::new(-hx, hy, hz),
    ];

    // 6 faces with local normals
    let faces = [
        ([0, 1, 2, 3], -forward), // Back
        ([5, 4, 7, 6], forward),  // Front
        ([4, 0, 3, 7], -right),   // Left
        ([1, 5, 6, 2], right),    // Right
        ([3, 2, 6, 7], up),       // Top
        ([4, 5, 1, 0], -up),      // Bottom
    ];

    for (face_indices, normal) in &faces {
        let base = vertices.len() as u32;
        for &i in face_indices {
            let pos = transform(corners[i]);
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [normal.x, normal.y, normal.z],
                color,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh { vertices, indices }
}

/// Generate a rotated box (for falling prisms)
fn generate_rotated_box(center: Vec3, half_extents: Vec3, rotation: Vec3, color: [f32; 4]) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Build rotation matrix from euler angles (simple XYZ rotation)
    let (sx, cx) = rotation.x.sin_cos();
    let (sy, cy) = rotation.y.sin_cos();
    let (sz, cz) = rotation.z.sin_cos();
    
    // Combined rotation matrix (ZYX order)
    let rotate = |v: Vec3| -> Vec3 {
        // Rotate around X
        let y1 = v.y * cx - v.z * sx;
        let z1 = v.y * sx + v.z * cx;
        let x1 = v.x;
        // Rotate around Y
        let x2 = x1 * cy + z1 * sy;
        let z2 = -x1 * sy + z1 * cy;
        let y2 = y1;
        // Rotate around Z
        let x3 = x2 * cz - y2 * sz;
        let y3 = x2 * sz + y2 * cz;
        let z3 = z2;
        Vec3::new(x3, y3, z3)
    };
    
    let (hx, hy, hz) = (half_extents.x, half_extents.y, half_extents.z);
    
    // 8 corners in local space
    let corners: [Vec3; 8] = [
        Vec3::new(-hx, -hy, -hz),
        Vec3::new( hx, -hy, -hz),
        Vec3::new( hx,  hy, -hz),
        Vec3::new(-hx,  hy, -hz),
        Vec3::new(-hx, -hy,  hz),
        Vec3::new( hx, -hy,  hz),
        Vec3::new( hx,  hy,  hz),
        Vec3::new(-hx,  hy,  hz),
    ];
    
    // Transform corners to world space
    let world_corners: Vec<Vec3> = corners.iter().map(|&c| center + rotate(c)).collect();
    
    // Face definitions with local normals
    let faces: [([usize; 4], Vec3); 6] = [
        ([0, 3, 2, 1], Vec3::new(0.0, 0.0, -1.0)), // Back
        ([4, 5, 6, 7], Vec3::new(0.0, 0.0, 1.0)),  // Front
        ([0, 4, 7, 3], Vec3::new(-1.0, 0.0, 0.0)), // Left
        ([1, 2, 6, 5], Vec3::new(1.0, 0.0, 0.0)),  // Right
        ([3, 7, 6, 2], Vec3::new(0.0, 1.0, 0.0)),  // Top
        ([0, 1, 5, 4], Vec3::new(0.0, -1.0, 0.0)), // Bottom
    ];
    
    for (face_indices, local_normal) in &faces {
        let base = vertices.len() as u32;
        let world_normal = rotate(*local_normal);
        
        for &i in face_indices {
            let pos = world_corners[i];
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [world_normal.x, world_normal.y, world_normal.z],
                color,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    
    Mesh { vertices, indices }
}

/// Generate a sphere mesh for projectiles
fn generate_sphere(center: Vec3, radius: f32, color: [f32; 4], segments: u32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Generate vertices
    for lat in 0..=segments {
        let theta = (lat as f32) * std::f32::consts::PI / (segments as f32);
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=segments {
            let phi = (lon as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            let pos = center + Vec3::new(x, y, z) * radius;
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [x, y, z],
                color,
            });
        }
    }

    // Generate indices
    for lat in 0..segments {
        for lon in 0..segments {
            let first = lat * (segments + 1) + lon;
            let second = first + segments + 1;

            indices.push(first);
            indices.push(second);
            indices.push(first + 1);

            indices.push(second);
            indices.push(second + 1);
            indices.push(first + 1);
        }
    }

    Mesh { vertices, indices }
}

// ============================================================================
// WGSL SHADER
// ============================================================================

const SHADER_SOURCE: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_dir: vec3<f32>,
    fog_density: f32,
    fog_color: vec3<f32>,
    ambient: f32,
    projectile_count: u32,
    _padding1: vec3<f32>,
    projectile_positions: array<vec4<f32>, 32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) screen_uv: vec2<f32>,
}

// ============================================================================
// ACES FILMIC TONEMAPPING (Unreal Engine 5 style)
// ============================================================================
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Gamma correction (linear to sRGB)
fn gamma_correct(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(1.0 / 2.2));
}

// Vignette effect
fn apply_vignette(color: vec3<f32>, uv: vec2<f32>, intensity: f32) -> vec3<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(uv, center);
    let vignette = 1.0 - smoothstep(0.3, 0.9, dist * intensity);
    return color * vignette;
}

// Film grain for cinematic feel
fn film_grain(color: vec3<f32>, uv: vec2<f32>, time: f32, intensity: f32) -> vec3<f32> {
    let noise = fract(sin(dot(uv + time * 0.1, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    return color + (noise - 0.5) * intensity;
}

// ============================================================================
// PBR-LIKE LIGHTING (UE5 inspired)
// ============================================================================

// GGX Normal Distribution Function
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (3.14159265 * denom * denom);
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// Geometry Smith
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx1 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx2 = geometry_schlick_ggx(n_dot_l, roughness);
    return ggx1 * ggx2;
}

// Subsurface scattering approximation for vegetation
fn sss_approx(view_dir: vec3<f32>, light_dir: vec3<f32>, normal: vec3<f32>, thickness: f32) -> f32 {
    // Light passing through from behind
    let sss_dot = clamp(dot(-view_dir, light_dir), 0.0, 1.0);
    let sss = pow(sss_dot, 3.0) * thickness;
    // Wrap lighting contribution
    let wrap = clamp(dot(normal, light_dir) * 0.5 + 0.5, 0.0, 1.0);
    return sss * wrap;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.world_pos = in.position;
    out.normal = in.normal;
    out.color = in.color;
    // Calculate screen UV for post-processing
    out.screen_uv = (out.clip_position.xy / out.clip_position.w) * 0.5 + 0.5;
    out.screen_uv.y = 1.0 - out.screen_uv.y;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.normal);
    let sun_dir = normalize(uniforms.sun_dir);
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let half_dir = normalize(view_dir + sun_dir);

    // Base color from vertex (terrain colors already in linear space)
    let albedo = in.color.rgb;
    
    // Detect grass/vegetation by green channel dominance
    let is_grass = albedo.g > albedo.r * 1.3 && albedo.g > albedo.b * 1.5;
    
    // Material properties (vary by surface type)
    let roughness = select(0.6, 0.85, is_grass);  // Grass is rougher
    let metallic = 0.0;  // Terrain is non-metallic
    let f0 = vec3<f32>(0.04);  // Dielectric base reflectivity
    
    // ============================================================
    // PBR LIGHTING
    // ============================================================
    let n_dot_l = max(dot(normal, sun_dir), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.0);
    let n_dot_h = max(dot(normal, half_dir), 0.0);
    let h_dot_v = max(dot(half_dir, view_dir), 0.0);
    
    // Cook-Torrance BRDF
    let ndf = distribution_ggx(n_dot_h, roughness);
    let g = geometry_smith(normal, view_dir, sun_dir, roughness);
    let f = fresnel_schlick(h_dot_v, f0);
    
    let numerator = ndf * g * f;
    let denominator = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = numerator / denominator;
    
    // Energy conservation
    let ks = f;
    let kd = (vec3<f32>(1.0) - ks) * (1.0 - metallic);
    
    // Sun light
    let sun_color = vec3<f32>(1.0, 0.95, 0.85);  // Warm sunlight
    let sun_intensity = 2.5;  // HDR intensity
    let radiance = sun_color * sun_intensity;
    
    // Direct lighting contribution
    var direct_light = (kd * albedo / 3.14159265 + specular) * radiance * n_dot_l;
    
    // ============================================================
    // SUBSURFACE SCATTERING (grass/vegetation)
    // ============================================================
    if (is_grass) {
        let sss = sss_approx(view_dir, sun_dir, normal, 0.4);
        let sss_color = vec3<f32>(0.4, 0.55, 0.15) * 2.0;  // Bright yellow-green
        direct_light = direct_light + sss_color * sss * albedo;
    }
    
    // ============================================================
    // AMBIENT / HEMISPHERE LIGHTING
    // ============================================================
    let sky_color = vec3<f32>(0.45, 0.55, 0.75);
    let ground_color = vec3<f32>(0.25, 0.20, 0.15);
    let sky_blend = normal.y * 0.5 + 0.5;
    let ambient_color = mix(ground_color, sky_color, sky_blend);
    
    // Simple AO based on normal (facing down = more occluded)
    let ao = normal.y * 0.3 + 0.7;
    
    let ambient_light = albedo * ambient_color * 0.35 * ao;
    
    // ============================================================
    // RIM LIGHT (edge highlighting)
    // ============================================================
    let rim = pow(1.0 - n_dot_v, 4.0);
    let rim_color = vec3<f32>(0.6, 0.7, 0.85) * rim * 0.15;
    
    // ============================================================
    // COMBINE LIGHTING
    // ============================================================
    var color = direct_light + ambient_light + rim_color;
    
    // ============================================================
    // ATMOSPHERIC FOG (UE5-style height fog)
    // ============================================================
    let dist = length(in.world_pos - uniforms.camera_pos);
    let height = in.world_pos.y;
    
    // Height-based fog density (denser at low altitudes)
    let height_falloff = 0.04;
    let height_factor = exp(-max(height, 0.0) * height_falloff);
    
    // Distance fog with height modulation
    let fog_amount = (1.0 - exp(-dist * uniforms.fog_density * height_factor)) * 0.85;
    
    // Fog color varies with distance (blue haze)
    let fog_near = vec3<f32>(0.55, 0.65, 0.80);
    let fog_far = vec3<f32>(0.45, 0.50, 0.60);
    let fog_blend = clamp(dist / 100.0, 0.0, 1.0);
    let final_fog_color = mix(fog_near, fog_far, fog_blend);
    
    color = mix(color, final_fog_color, fog_amount);
    
    // ============================================================
    // POST-PROCESSING (Unreal-style cinematic look)
    // ============================================================
    
    // ACES Tonemapping
    color = aces_tonemap(color);
    
    // Subtle color grading (warm shadows, cool highlights)
    let lift = vec3<f32>(0.015, 0.01, 0.02);
    let gain = vec3<f32>(1.03, 1.01, 0.98);
    color = color * gain + lift;
    
    // Vignette
    color = apply_vignette(color, in.screen_uv, 1.0);
    
    // Very subtle film grain
    color = film_grain(color, in.screen_uv, uniforms.time, 0.015);
    
    // Gamma correction
    color = gamma_correct(color);
    
    return vec4<f32>(color, in.color.a);
}
"#;

// ============================================================================
// HEX-PRISM WALLS (US-012)
// ============================================================================

/// Create test hex-prism walls for the battle arena
/// Creates a simple wall: 5 prisms in a row, 3 layers high on the defender hex
#[allow(dead_code)]  // Will be used in US-012 when hex-prism rendering is integrated
fn create_test_walls() -> HexPrismGrid {
    let mut grid = HexPrismGrid::new();
    // Build a wall: 5 prisms wide, 3 layers tall, material 0 = stone gray
    grid.create_wall(0, 0, 5, 3, 0);
    // Add variety wall with material 2 = stone dark
    grid.create_wall(-2, 2, 3, 2, 2);
    println!("[Battle Arena] Created hex-prism walls: {} prisms", grid.len());
    grid
}

// ============================================================================
// APPLICATION
// ============================================================================

struct BattleArenaApp {
    window: Option<Arc<Window>>,

    // GPU resources
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    surface: Option<wgpu::Surface<'static>>,
    surface_config: Option<wgpu::SurfaceConfiguration>,
    pipeline: Option<wgpu::RenderPipeline>,
    uniform_buffer: Option<wgpu::Buffer>,
    uniform_bind_group: Option<wgpu::BindGroup>,

    // Static mesh buffers (terrain, cannon base)
    static_vertex_buffer: Option<wgpu::Buffer>,
    static_index_buffer: Option<wgpu::Buffer>,
    static_index_count: u32,

    // Dynamic mesh buffers (cannon barrel, projectiles)
    dynamic_vertex_buffer: Option<wgpu::Buffer>,
    dynamic_index_buffer: Option<wgpu::Buffer>,
    dynamic_index_count: u32,

    // Hex-prism walls for collision detection (US-015) and destruction (US-016)
    hex_wall_grid: HexPrismGrid,
    hex_wall_vertex_buffer: Option<wgpu::Buffer>,
    hex_wall_index_buffer: Option<wgpu::Buffer>,
    hex_wall_index_count: u32,
    prisms_destroyed: u32, // Total prisms destroyed (US-016)

    // SDF cannon rendering (US-013)
    sdf_cannon_pipeline: Option<wgpu::RenderPipeline>,
    sdf_cannon_uniform_buffer: Option<wgpu::Buffer>,
    sdf_cannon_data_buffer: Option<wgpu::Buffer>,
    sdf_cannon_bind_group: Option<wgpu::BindGroup>,
    depth_texture: Option<wgpu::TextureView>,
    depth_texture_raw: Option<wgpu::Texture>,
    
    // UI pipeline (no depth testing, for 2D overlay)
    ui_pipeline: Option<wgpu::RenderPipeline>,
    ui_uniform_buffer: Option<wgpu::Buffer>,
    ui_bind_group: Option<wgpu::BindGroup>,

    // Dark and stormy skybox
    stormy_sky: Option<StormySky>,

    // First-person player controller (Phase 1)
    player: Player,
    first_person_mode: bool, // Toggle between FPS mode and free camera
    
    // Camera and input
    camera: Camera,
    movement: MovementKeys,
    aiming: AimingKeys, // Cannon aiming keys (US-017)
    mouse_pressed: bool,
    left_mouse_pressed: bool, // For builder mode placement
    last_mouse_pos: Option<(f32, f32)>,
    current_mouse_pos: Option<(f32, f32)>, // For raycast

    // Builder mode (Fallout 4-style building)
    builder_mode: BuilderMode,
    // Ghost and grid rendering uses dynamic_mesh buffer for simplicity
    
    // New building block system (Phase 2-4)
    build_toolbar: BuildToolbar,
    block_manager: BuildingBlockManager,
    merge_workflow: MergeWorkflowManager,
    sculpting: SculptingManager,
    merged_mesh_buffers: Vec<MergedMeshBuffers>,
    block_vertex_buffer: Option<wgpu::Buffer>,
    block_index_buffer: Option<wgpu::Buffer>,
    block_index_count: u32,
    
    // Windows focus overlay
    start_overlay: StartOverlay,

    // Procedural trees (harvestable for building materials)
    trees_attacker: Vec<PlacedTree>,
    trees_defender: Vec<PlacedTree>,
    tree_vertex_buffer: Option<wgpu::Buffer>,
    tree_index_buffer: Option<wgpu::Buffer>,
    tree_index_count: u32,
    wood_harvested: u32, // Total wood collected

    // Cannon and projectiles
    cannon: Cannon,
    projectiles: Vec<Projectile>,
    ballistics_config: BallisticsConfig,

    // Physics-based destruction system
    falling_prisms: Vec<FallingPrism>,
    debris_particles: Vec<DebrisParticle>,

    // Terrain editor UI (on-screen sliders)
    terrain_ui: TerrainEditorUI,
    terrain_needs_rebuild: bool,

    // Timing
    start_time: Instant,
    last_frame: Instant,
    frame_count: u64,
    fps: f32,
    last_fps_update: Instant,
}

impl BattleArenaApp {
    fn new() -> Self {
        Self {
            window: None,
            device: None,
            queue: None,
            surface: None,
            surface_config: None,
            pipeline: None,
            uniform_buffer: None,
            uniform_bind_group: None,
            static_vertex_buffer: None,
            static_index_buffer: None,
            static_index_count: 0,
            dynamic_vertex_buffer: None,
            dynamic_index_buffer: None,
            dynamic_index_count: 0,
            hex_wall_grid: create_test_walls(),
            hex_wall_vertex_buffer: None,
            hex_wall_index_buffer: None,
            hex_wall_index_count: 0,
            prisms_destroyed: 0,
            sdf_cannon_pipeline: None,
            sdf_cannon_uniform_buffer: None,
            sdf_cannon_data_buffer: None,
            sdf_cannon_bind_group: None,
            depth_texture: None,
            depth_texture_raw: None,
            ui_pipeline: None,
            ui_uniform_buffer: None,
            ui_bind_group: None,
            stormy_sky: None,
            player: Player::default(),
            first_person_mode: true, // Start in first-person mode
            camera: Camera::default(),
            movement: MovementKeys::default(),
            aiming: AimingKeys::default(),
            mouse_pressed: false,
            left_mouse_pressed: false,
            last_mouse_pos: None,
            current_mouse_pos: None,
            builder_mode: BuilderMode::default(),
            build_toolbar: BuildToolbar::default(),
            block_manager: BuildingBlockManager::new(),
            merge_workflow: MergeWorkflowManager::new(),
            sculpting: SculptingManager::new(),
            merged_mesh_buffers: Vec::new(),
            block_vertex_buffer: None,
            block_index_buffer: None,
            block_index_count: 0,
            start_overlay: StartOverlay::default(),
            trees_attacker: Vec::new(),
            trees_defender: Vec::new(),
            tree_vertex_buffer: None,
            tree_index_buffer: None,
            tree_index_count: 0,
            wood_harvested: 0,
            cannon: Cannon::default(),
            projectiles: Vec::new(),
            ballistics_config: BallisticsConfig::default(),
            falling_prisms: Vec::new(),
            debris_particles: Vec::new(),
            terrain_ui: TerrainEditorUI::default(),
            terrain_needs_rebuild: false,
            start_time: Instant::now(),
            last_frame: Instant::now(),
            frame_count: 0,
            fps: 0.0,
            last_fps_update: Instant::now(),
        }
    }

    fn initialize(&mut self, window: Arc<Window>) {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(Arc::clone(&window)).unwrap();

        // Request adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find adapter");

        // Create device and queue
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Battle Arena Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                ..Default::default()
            },
        ))
        .expect("Failed to create device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Prefer Immediate (uncapped FPS) for performance testing, fallback to Mailbox
        let present_mode = if surface_caps.present_modes.contains(&wgpu::PresentMode::Immediate) {
            wgpu::PresentMode::Immediate
        } else if surface_caps.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::AutoVsync
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Battle Arena Shader"),
            source: wgpu::ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create bind group
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        
        // ============================================
        // UI PIPELINE (no depth testing, for 2D overlay)
        // ============================================
        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for UI
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None, // NO depth testing for UI overlay
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        
        // Create UI uniform buffer with identity matrix (pre-initialized, never changes)
        let ui_uniforms = Uniforms {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            time: 0.0,
            sun_dir: [0.0, 1.0, 0.0],
            fog_density: 0.0, // No fog for UI
            fog_color: [0.0, 0.0, 0.0],
            ambient: 1.0, // Full brightness for UI
            projectile_count: 0,
            _pad_before_padding1: [0.0; 3],
            _padding1: [0.0; 3],
            _pad_before_array: 0.0,
            projectile_positions: [[0.0; 4]; 32],
        };
        let ui_uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Uniform Buffer"),
            contents: bytemuck::bytes_of(&ui_uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        
        // Create UI bind group (uses same layout as main pipeline)
        let ui_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("UI Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ui_uniform_buffer.as_entire_binding(),
            }],
        });

        // ============================================
        // SDF CANNON RENDERING SETUP (US-013)
        // ============================================
        // Load SDF cannon shader from file
        let sdf_cannon_shader_source = std::fs::read_to_string("shaders/sdf_cannon.wgsl")
            .expect("Failed to load sdf_cannon.wgsl shader");
        let sdf_cannon_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SDF Cannon Shader"),
            source: wgpu::ShaderSource::Wgsl(sdf_cannon_shader_source.into()),
        });

        // Create SDF cannon uniform buffer (camera, time, lighting)
        let sdf_cannon_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SDF Cannon Uniform Buffer"),
            size: std::mem::size_of::<SdfCannonUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create SDF cannon data buffer (position, rotation, color)
        let sdf_cannon_data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("SDF Cannon Data Buffer"),
            size: std::mem::size_of::<SdfCannonData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create SDF cannon bind group layout
        let sdf_cannon_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("SDF Cannon Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create SDF cannon bind group
        let sdf_cannon_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SDF Cannon Bind Group"),
            layout: &sdf_cannon_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sdf_cannon_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: sdf_cannon_data_buffer.as_entire_binding(),
                },
            ],
        });

        // Create SDF cannon pipeline layout
        let sdf_cannon_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("SDF Cannon Pipeline Layout"),
            bind_group_layouts: &[&sdf_cannon_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create depth texture for proper depth testing
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: size.width.max(1),
                height: size.height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let depth_texture_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Create SDF cannon render pipeline (fullscreen triangle for ray marching)
        let sdf_cannon_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SDF Cannon Pipeline"),
            layout: Some(&sdf_cannon_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sdf_cannon_shader,
                entry_point: Some("vs_main"),
                buffers: &[], // Fullscreen triangle uses vertex_index
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sdf_cannon_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for fullscreen triangle
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        println!("[US-013] SDF cannon pipeline initialized");

        // Create static mesh (terrain platforms with procedural elevation)
        let mut static_mesh = Mesh::new();

        // Attacker hex platform (where cannon is) - detailed mountainous terrain
        let attacker_platform = generate_elevated_hex_terrain(
            Vec3::new(0.0, 0.0, 25.0),
            30.0,
            [0.4, 0.5, 0.3, 1.0], // Color determined dynamically by height
            64, // High subdivision for detailed mountains
        );
        static_mesh.merge(&attacker_platform);
        
        // Water plane for attacker platform
        let attacker_water = generate_water_plane(
            Vec3::new(0.0, 0.0, 25.0),
            30.0,
        );
        static_mesh.merge(&attacker_water);

        // Defender hex platform (target) - detailed mountainous terrain
        let defender_platform = generate_elevated_hex_terrain(
            Vec3::new(0.0, 0.0, -25.0),
            30.0,
            [0.5, 0.4, 0.35, 1.0], // Color determined dynamically by height
            64, // High subdivision for detailed mountains
        );
        static_mesh.merge(&defender_platform);
        
        // Water plane for defender platform
        let defender_water = generate_water_plane(
            Vec3::new(0.0, 0.0, -25.0),
            30.0,
        );
        static_mesh.merge(&defender_water);
        
        println!("[Builder Mode] Generated detailed terrain with mountains, rocks, and water");
        
        // ============================================
        // PROCEDURAL TREES (harvestable)
        // ============================================
        // Generate trees on both platforms using noise-based distribution
        self.trees_attacker = generate_trees_on_terrain(
            Vec3::new(0.0, 0.0, 25.0),
            28.0, // Slightly smaller than terrain
            0.3,  // Density threshold (higher = more trees)
            0.0,  // Seed offset
        );
        self.trees_defender = generate_trees_on_terrain(
            Vec3::new(0.0, 0.0, -25.0),
            28.0,
            0.35, // Slightly more trees on defender side
            100.0, // Different seed for variety
        );
        
        // Generate combined tree mesh
        let mut all_trees = self.trees_attacker.clone();
        all_trees.extend(self.trees_defender.clone());
        let tree_mesh = generate_all_trees_mesh(&all_trees);
        
        // Create tree buffers
        let max_tree_vertices = 2000 * 50; // Max ~2000 trees, ~50 vertices each
        let max_tree_indices = 2000 * 80;
        
        let tree_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tree Vertex Buffer"),
            size: (max_tree_vertices * std::mem::size_of::<Vertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        let tree_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tree Index Buffer"),
            size: (max_tree_indices * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        
        if !tree_mesh.vertices.is_empty() {
            queue.write_buffer(&tree_vertex_buffer, 0, bytemuck::cast_slice(&tree_mesh.vertices));
            queue.write_buffer(&tree_index_buffer, 0, bytemuck::cast_slice(&tree_mesh.indices));
        }
        
        let tree_index_count = tree_mesh.indices.len() as u32;
        println!("[Trees] Generated {} trees ({} attacker, {} defender)", 
            self.trees_attacker.len() + self.trees_defender.len(),
            self.trees_attacker.len(),
            self.trees_defender.len()
        );

        // Create static buffers
        let static_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Static Vertex Buffer"),
            contents: bytemuck::cast_slice(&static_mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let static_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Static Index Buffer"),
            contents: bytemuck::cast_slice(&static_mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Create dynamic buffers (will be updated each frame)
        let dynamic_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Vertex Buffer"),
            size: 1024 * 1024, // 1MB should be plenty
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let dynamic_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dynamic Index Buffer"),
            size: 256 * 1024, // 256KB
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ============================================
        // HEX-PRISM WALLS (US-012, US-016)
        // ============================================
        // Generate mesh from hex-prism wall grid
        let (hex_vertices, hex_indices) = self.hex_wall_grid.generate_combined_mesh();

        // Convert HexPrismVertex to Vertex (same layout: position, normal, color)
        let hex_wall_vertices: Vec<Vertex> = hex_vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            })
            .collect();

        let hex_wall_index_count = hex_indices.len() as u32;

        // US-016: Create buffers with COPY_DST to allow updates when prisms are destroyed
        // Pre-allocate space for max 500 prisms (each = 38 vertices, 72 indices)
        let max_hex_vertex_bytes = 500 * 38 * std::mem::size_of::<Vertex>();
        let max_hex_index_bytes = 500 * 72 * std::mem::size_of::<u32>();

        let hex_wall_vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Hex Wall Vertex Buffer"),
            size: max_hex_vertex_bytes as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let hex_wall_index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Hex Wall Index Buffer"),
            size: max_hex_index_bytes as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Write initial mesh data to buffers
        if !hex_wall_vertices.is_empty() {
            queue.write_buffer(&hex_wall_vertex_buffer, 0, bytemuck::cast_slice(&hex_wall_vertices));
            queue.write_buffer(&hex_wall_index_buffer, 0, bytemuck::cast_slice(&hex_indices));
        }

        // Initialize dark and stormy skybox (before device is moved)
        let stormy_sky = StormySky::with_config(&device, surface_format, StormySkyConfig::default());

        self.window = Some(window);
        self.device = Some(device);
        self.queue = Some(queue);
        self.surface = Some(surface);
        self.surface_config = Some(surface_config);
        self.pipeline = Some(pipeline);
        self.uniform_buffer = Some(uniform_buffer);
        self.uniform_bind_group = Some(uniform_bind_group);
        self.static_vertex_buffer = Some(static_vertex_buffer);
        self.static_index_buffer = Some(static_index_buffer);
        self.static_index_count = static_mesh.indices.len() as u32;
        self.dynamic_vertex_buffer = Some(dynamic_vertex_buffer);
        self.dynamic_index_buffer = Some(dynamic_index_buffer);
        self.hex_wall_vertex_buffer = Some(hex_wall_vertex_buffer);
        self.hex_wall_index_buffer = Some(hex_wall_index_buffer);
        self.hex_wall_index_count = hex_wall_index_count;

        // Store SDF cannon resources (US-013)
        self.sdf_cannon_pipeline = Some(sdf_cannon_pipeline);
        self.sdf_cannon_uniform_buffer = Some(sdf_cannon_uniform_buffer);
        self.sdf_cannon_data_buffer = Some(sdf_cannon_data_buffer);
        self.sdf_cannon_bind_group = Some(sdf_cannon_bind_group);
        self.depth_texture = Some(depth_texture_view);
        self.depth_texture_raw = Some(depth_texture);
        self.ui_pipeline = Some(ui_pipeline);
        self.ui_uniform_buffer = Some(ui_uniform_buffer);
        self.ui_bind_group = Some(ui_bind_group);
        self.stormy_sky = Some(stormy_sky);
        
        // Store tree buffers
        self.tree_vertex_buffer = Some(tree_vertex_buffer);
        self.tree_index_buffer = Some(tree_index_buffer);
        self.tree_index_count = tree_index_count;

        println!(
            "[Battle Arena] Hex-prism walls: {} vertices, {} indices",
            hex_wall_vertices.len(),
            hex_indices.len()
        );
    }

    fn update(&mut self, delta_time: f32) {
        // Check if terrain needs rebuilding
        if self.terrain_needs_rebuild {
            self.rebuild_terrain();
            self.terrain_needs_rebuild = false;
        }
        
        // Update block placement preview
        self.update_block_preview();
        
        // Physics support check every N seconds (blocks without support fall)
        if self.build_toolbar.visible {
            self.build_toolbar.physics_check_timer += delta_time;
            if self.build_toolbar.physics_check_timer >= PHYSICS_CHECK_INTERVAL {
                self.build_toolbar.physics_check_timer = 0.0;
                self.check_physics_support();
            }
        }
        
        // Update movement based on mode
        if self.first_person_mode {
            // First-person mode: Player physics with terrain collision
            self.player.update(&self.movement, self.camera.yaw, delta_time);
            
            // Check collision with building blocks
            self.player_block_collision();
            
            // Check collision with hex prism walls
            self.player_hex_collision();
            
            // Camera follows player's eye position
            self.camera.position = self.player.get_eye_position();
        } else {
            // Free camera mode: Direct camera movement (original behavior)
            let forward = if self.movement.forward { 1.0 } else { 0.0 }
                - if self.movement.backward { 1.0 } else { 0.0 };
            let right = if self.movement.right { 1.0 } else { 0.0 }
                - if self.movement.left { 1.0 } else { 0.0 };
            let up = if self.movement.up { 1.0 } else { 0.0 }
                - if self.movement.down { 1.0 } else { 0.0 };

            self.camera
                .update_movement(forward, right, up, delta_time, self.movement.sprint);
        }

        // Update cannon aiming based on held arrow keys (US-017: smooth movement)
        let aim_delta = CANNON_ROTATION_SPEED * delta_time;
        if self.aiming.aim_up {
            self.cannon.adjust_elevation(aim_delta);
        }
        if self.aiming.aim_down {
            self.cannon.adjust_elevation(-aim_delta);
        }
        if self.aiming.aim_left {
            self.cannon.adjust_azimuth(-aim_delta);
        }
        if self.aiming.aim_right {
            self.cannon.adjust_azimuth(aim_delta);
        }

        // Smooth interpolation for cannon barrel movement
        self.cannon.update(delta_time);

        // Update projectiles with ballistics and collision detection (US-015)
        // Collect hit information first, then apply destruction (US-016)
        let mut prisms_to_destroy: Vec<(i32, i32, i32)> = Vec::new();

        self.projectiles.retain_mut(|projectile| {
            // Store position before integration for collision ray
            let prev_position = projectile.position;

            // Integrate physics
            let state = projectile.integrate(&self.ballistics_config, delta_time);

            // If still flying, check for collision with hex-prism walls
            if matches!(state, ProjectileState::Flying) {
                // Cast ray from previous position to current position
                let ray_dir = projectile.position - prev_position;
                let ray_length = ray_dir.length();

                if ray_length > 0.001 {
                    let ray_dir_normalized = ray_dir / ray_length;

                    // Check collision against hex-prism grid
                    if let Some(hit) = self.hex_wall_grid.ray_cast(prev_position, ray_dir_normalized, ray_length) {
                        // Projectile hit a wall!
                        projectile.position = hit.position;
                        projectile.active = false;

                        // US-016: Queue prism for destruction
                        prisms_to_destroy.push(hit.prism_coord);

                        println!(
                            "[US-016] Projectile HIT wall at ({:.2}, {:.2}, {:.2}), destroying prism {:?}",
                            hit.position.x, hit.position.y, hit.position.z, hit.prism_coord
                        );
                        return false; // Remove projectile (state = Hit)
                    }
                }

                true // Keep flying
            } else {
                // Ground hit or expired
                false
            }
        });

        // US-016: Apply hex-prism destruction with physics cascade for all hits this frame
        for coord in prisms_to_destroy {
            self.destroy_prism_with_physics(coord);
        }

        // Physics simulation for falling prisms
        self.update_falling_prisms(delta_time);
        
        // Physics simulation for debris particles
        self.update_debris_particles(delta_time);

        // US-016: Regenerate hex-prism mesh if any prisms were destroyed or modified
        if self.hex_wall_grid.needs_mesh_update() {
            self.regenerate_hex_wall_mesh();
        }
        
        // Builder mode: Update cursor position from mouse
        self.update_builder_cursor();
    }
    
    /// Destroy a prism and check for unsupported prisms above/behind it
    fn destroy_prism_with_physics(&mut self, coord: (i32, i32, i32)) {
        // Remove the hit prism and spawn debris
        if let Some(prism) = self.hex_wall_grid.remove_by_coord(coord) {
            self.prisms_destroyed += 1;
            
            // Spawn debris particles at the destroyed prism's location
            let debris = spawn_debris(prism.center, prism.material, 8);
            self.debris_particles.extend(debris);
            
            println!(
                "[Physics] Prism DESTROYED at {:?}, spawned {} debris (total destroyed: {})",
                coord, 8, self.prisms_destroyed
            );
            
            // Check for cascade - prisms that lost support
            self.check_support_cascade(coord);
        }
    }
    
    /// Check if prisms above or nearby have lost support and should fall
    fn check_support_cascade(&mut self, destroyed_coord: (i32, i32, i32)) {
        let (q, r, level) = destroyed_coord;
        
        // Collect all prisms that need to fall (can't modify grid while iterating)
        let mut prisms_to_fall: Vec<(i32, i32, i32)> = Vec::new();
        
        // Check prism directly above
        if self.hex_wall_grid.contains(q, r, level + 1) {
            if !self.has_support(q, r, level + 1) {
                prisms_to_fall.push((q, r, level + 1));
            }
        }
        
        // Check prisms in adjacent columns that might have lost support
        // Hex neighbors (axial coordinates)
        let neighbors = [
            (q + 1, r),     // E
            (q - 1, r),     // W
            (q, r + 1),     // SE
            (q, r - 1),     // NW
            (q + 1, r - 1), // NE
            (q - 1, r + 1), // SW
        ];
        
        for (nq, nr) in neighbors {
            // Check if neighbor at same level or above has lost support
            for check_level in level..=level + 3 {
                if self.hex_wall_grid.contains(nq, nr, check_level) {
                    if !self.has_support(nq, nr, check_level) {
                        prisms_to_fall.push((nq, nr, check_level));
                    }
                }
            }
        }
        
        // Convert unsupported prisms to falling prisms
        for coord in prisms_to_fall {
            if let Some(prism) = self.hex_wall_grid.remove_by_coord(coord) {
                println!("[Physics] Prism at {:?} lost support - FALLING!", coord);
                self.falling_prisms.push(FallingPrism::new(coord, prism.center, prism.material));
                
                // Recursively check for more cascade
                self.check_support_cascade(coord);
            }
        }
    }
    
    /// Check if a prism has support (something below it or on ground level)
    fn has_support(&self, q: i32, r: i32, level: i32) -> bool {
        // Ground level always has support
        if level <= 0 {
            return true;
        }
        
        // Check if there's a prism directly below
        if self.hex_wall_grid.contains(q, r, level - 1) {
            return true;
        }
        
        // Check for diagonal support from neighbors (structural support)
        // A prism can be supported if at least 2 adjacent neighbors at same level exist
        let neighbors = [
            (q + 1, r),
            (q - 1, r),
            (q, r + 1),
            (q, r - 1),
            (q + 1, r - 1),
            (q - 1, r + 1),
        ];
        
        let mut neighbor_support = 0;
        for (nq, nr) in neighbors {
            // Check neighbor at same level (horizontal structural support)
            if self.hex_wall_grid.contains(nq, nr, level) {
                neighbor_support += 1;
            }
            // Check neighbor below (diagonal support)
            if self.hex_wall_grid.contains(nq, nr, level - 1) {
                neighbor_support += 1;
            }
        }
        
        // Need at least 2 supports to stay standing (structural integrity)
        neighbor_support >= 2
    }
    
    /// Update falling prisms physics
    fn update_falling_prisms(&mut self, delta_time: f32) {
        // Update physics for each falling prism
        for prism in &mut self.falling_prisms {
            prism.update(delta_time);
        }
        
        // Check for collisions with remaining wall prisms
        let mut new_debris: Vec<DebrisParticle> = Vec::new();
        let mut prisms_to_destroy: Vec<(i32, i32, i32)> = Vec::new();
        
        self.falling_prisms.retain(|prism| {
            if prism.grounded {
                // Prism hit the ground - convert to debris
                new_debris.extend(spawn_debris(prism.position, prism.material, 12));
                println!("[Physics] Falling prism IMPACTED ground at ({:.1}, {:.1}, {:.1})", 
                    prism.position.x, prism.position.y, prism.position.z);
                return false; // Remove falling prism
            }
            
            // Check if falling prism collides with another wall prism
            // Convert world position to approximate grid coords
            let approx_q = (prism.position.x / (DEFAULT_HEX_RADIUS * 1.732)).round() as i32;
            let approx_r = (prism.position.z / (DEFAULT_HEX_RADIUS * 1.5)).round() as i32;
            let approx_level = (prism.position.y / DEFAULT_HEX_HEIGHT).floor() as i32;
            
            // Check collision with nearby prisms
            for dq in -1..=1 {
                for dr in -1..=1 {
                    for dl in -1..=1 {
                        let check_coord = (approx_q + dq, approx_r + dr, approx_level + dl);
                        if self.hex_wall_grid.contains(check_coord.0, check_coord.1, check_coord.2) {
                            // Collision! Destroy the stationary prism too
                            prisms_to_destroy.push(check_coord);
                            new_debris.extend(spawn_debris(prism.position, prism.material, 6));
                            return false;
                        }
                    }
                }
            }
            
            // Keep falling if lifetime < 10 seconds (safety limit)
            prism.lifetime < 10.0
        });
        
        // Add spawned debris
        self.debris_particles.extend(new_debris);
        
        // Destroy prisms that were hit by falling debris
        for coord in prisms_to_destroy {
            self.destroy_prism_with_physics(coord);
        }
    }
    
    /// Update debris particles
    fn update_debris_particles(&mut self, delta_time: f32) {
        for particle in &mut self.debris_particles {
            particle.update(delta_time);
        }
        
        // Remove dead particles
        self.debris_particles.retain(|p| p.is_alive());
    }

    /// US-016: Regenerate hex-prism wall mesh and update GPU buffers after destruction
    fn regenerate_hex_wall_mesh(&mut self) {
        let Some(ref queue) = self.queue else { return };
        let Some(ref hex_wall_vb) = self.hex_wall_vertex_buffer else { return };
        let Some(ref hex_wall_ib) = self.hex_wall_index_buffer else { return };

        // Generate new mesh from remaining prisms
        let (hex_vertices, hex_indices) = self.hex_wall_grid.generate_combined_mesh();

        // Convert HexPrismVertex to Vertex
        let hex_wall_vertices: Vec<Vertex> = hex_vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            })
            .collect();

        // Update index count for rendering
        self.hex_wall_index_count = hex_indices.len() as u32;

        // Write updated mesh data to GPU buffers
        if !hex_wall_vertices.is_empty() {
            queue.write_buffer(hex_wall_vb, 0, bytemuck::cast_slice(&hex_wall_vertices));
            queue.write_buffer(hex_wall_ib, 0, bytemuck::cast_slice(&hex_indices));
        }

        // Clear the dirty flag
        self.hex_wall_grid.clear_mesh_dirty();

        println!(
            "[US-016] Mesh regenerated: {} prisms remaining ({} vertices, {} indices)",
            self.hex_wall_grid.len(),
            hex_wall_vertices.len(),
            hex_indices.len()
        );
    }

    fn fire_projectile(&mut self) {
        if self.projectiles.len() < 32 {
            let projectile = self.cannon.fire();
            self.projectiles.push(projectile);
            println!(
                "Fired projectile! Elevation: {:.1}°, Azimuth: {:.1}°, Active: {}",
                self.cannon.barrel_elevation.to_degrees(),
                self.cannon.barrel_azimuth.to_degrees(),
                self.projectiles.len()
            );
        }
    }
    
    /// Calculate the snapped position for block placement
    fn calculate_block_placement_position(&self) -> Option<Vec3> {
        let config = self.surface_config.as_ref()?;
        
        // Use mouse cursor position for placement (like an editor)
        // This allows precise selection of where to place
        let mouse_pos = self.current_mouse_pos.unwrap_or((
            config.width as f32 / 2.0,
            config.height as f32 / 2.0
        ));
        
        // Convert screen position to ray
        let aspect = config.width as f32 / config.height as f32;
        let half_fov = (self.camera.fov / 2.0).tan();
        
        let ndc_x = (mouse_pos.0 / config.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (mouse_pos.1 / config.height as f32) * 2.0;
        
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = forward.cross(right).normalize();
        
        // Use same formula as hex grid's screen_to_ray (which works)
        let ray_dir = (forward + right * ndc_x * half_fov * aspect + up * ndc_y * half_fov)
            .normalize();
        
        let ray_origin = self.camera.position;
        
        // Find intersection with terrain or existing blocks
        let max_dist = 50.0;
        let step_size = 0.25;
        let mut t = 1.0;
        
        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            let ground_height = terrain_height_at(p.x, p.z, 0.0);
            
            // Check if we hit terrain
            if p.y <= ground_height {
                // Snap to grid
                let snapped_x = (p.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                let snapped_z = (p.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                let snapped_ground = terrain_height_at(snapped_x, snapped_z, 0.0);
                
                // Apply build height offset
                let y = snapped_ground + 0.5 + self.build_toolbar.build_height;
                let mut position = Vec3::new(snapped_x, y, snapped_z);
                
                // Try to snap to nearby existing blocks
                position = self.snap_to_nearby_blocks(position);
                
                return Some(position);
            }
            
            // Check if we hit an existing block (for stacking)
            if let Some((block_id, dist)) = self.block_manager.find_closest(p, 1.0) {
                if dist < 0.5 {
                    // Hit a block - place on top/side of it
                    if let Some(block) = self.block_manager.get_block(block_id) {
                        let aabb = block.aabb();
                        
                        // Determine which face we hit
                        let block_center = (aabb.min + aabb.max) * 0.5;
                        let hit_offset = p - block_center;
                        
                        // Find dominant axis
                        let abs_offset = Vec3::new(hit_offset.x.abs(), hit_offset.y.abs(), hit_offset.z.abs());
                        
                        let mut placement_pos;
                        if abs_offset.y > abs_offset.x && abs_offset.y > abs_offset.z {
                            // Top or bottom
                            if hit_offset.y > 0.0 {
                                placement_pos = Vec3::new(block_center.x, aabb.max.y + 0.5, block_center.z);
                            } else {
                                placement_pos = Vec3::new(block_center.x, aabb.min.y - 0.5, block_center.z);
                            }
                        } else if abs_offset.x > abs_offset.z {
                            // Left or right
                            if hit_offset.x > 0.0 {
                                placement_pos = Vec3::new(aabb.max.x + 0.5, block_center.y, block_center.z);
                            } else {
                                placement_pos = Vec3::new(aabb.min.x - 0.5, block_center.y, block_center.z);
                            }
                        } else {
                            // Front or back
                            if hit_offset.z > 0.0 {
                                placement_pos = Vec3::new(block_center.x, block_center.y, aabb.max.z + 0.5);
                            } else {
                                placement_pos = Vec3::new(block_center.x, block_center.y, aabb.min.z - 0.5);
                            }
                        }
                        
                        // Snap to grid
                        placement_pos.x = (placement_pos.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                        placement_pos.z = (placement_pos.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                        
                        return Some(placement_pos);
                    }
                }
            }
            
            t += step_size;
        }
        
        // If nothing hit, place at a reasonable distance
        let p = ray_origin + ray_dir * 10.0;
        let snapped_x = (p.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
        let snapped_z = (p.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
        let ground = terrain_height_at(snapped_x, snapped_z, 0.0);
        
        Some(Vec3::new(snapped_x, ground + 0.5 + self.build_toolbar.build_height, snapped_z))
    }
    
    /// Snap position to nearby existing blocks if close enough
    fn snap_to_nearby_blocks(&self, position: Vec3) -> Vec3 {
        let mut best_pos = position;
        let mut best_dist = BLOCK_SNAP_DISTANCE;
        
        for block in self.block_manager.blocks() {
            let aabb = block.aabb();
            let block_center = (aabb.min + aabb.max) * 0.5;
            
            // Check if we should snap to this block's grid
            let dx = position.x - block_center.x;
            let dz = position.z - block_center.z;
            
            // Possible snap positions (adjacent to this block)
            let snap_positions = [
                Vec3::new(block_center.x + BLOCK_GRID_SIZE, position.y, block_center.z), // Right
                Vec3::new(block_center.x - BLOCK_GRID_SIZE, position.y, block_center.z), // Left
                Vec3::new(block_center.x, position.y, block_center.z + BLOCK_GRID_SIZE), // Front
                Vec3::new(block_center.x, position.y, block_center.z - BLOCK_GRID_SIZE), // Back
                Vec3::new(block_center.x, aabb.max.y + 0.5, block_center.z), // Top
            ];
            
            for snap_pos in snap_positions {
                let dist = (snap_pos - position).length();
                if dist < best_dist {
                    best_dist = dist;
                    best_pos = snap_pos;
                }
            }
        }
        
        best_pos
    }
    
    /// Update the preview position for block placement
    fn update_block_preview(&mut self) {
        if self.build_toolbar.visible {
            self.build_toolbar.preview_position = self.calculate_block_placement_position();
        } else {
            self.build_toolbar.preview_position = None;
        }
    }
    
    /// Place a building block at the preview position
    fn place_building_block(&mut self) {
        // Use the preview position if available
        let position = match self.build_toolbar.preview_position {
            Some(pos) => pos,
            None => {
                // Fallback: calculate position now
                match self.calculate_block_placement_position() {
                    Some(pos) => pos,
                    None => return,
                }
            }
        };
        
        // Create block with selected shape
        let shape = self.build_toolbar.get_selected_shape();
        let block = BuildingBlock::new(shape, position, self.build_toolbar.selected_material);
        let block_id = self.block_manager.add_block(block);
        
        println!("[Build] Placed {} at ({:.1}, {:.1}, {:.1}) ID={}", 
            SHAPE_NAMES[self.build_toolbar.selected_shape],
            position.x, position.y, position.z, block_id);
        
        self.regenerate_block_mesh();
    }
    
    /// Handle click in bridge mode - select faces
    fn handle_bridge_click(&mut self) -> bool {
        if !self.build_toolbar.is_bridge_mode() {
            return false;
        }
        
        // Raycast to find which block face is clicked
        if let Some(face) = self.raycast_to_face() {
            self.build_toolbar.bridge_tool.select_face(face);
            
            // If both faces selected, create the bridge
            if self.build_toolbar.bridge_tool.is_ready() {
                self.create_bridge();
                self.build_toolbar.bridge_tool.clear();
            }
            return true;
        }
        false
    }
    
    /// Raycast to find which face of a block is pointed at
    fn raycast_to_face(&self) -> Option<SelectedFace> {
        let mouse_pos = self.current_mouse_pos?;
        let config = self.surface_config.as_ref()?;
        
        let aspect = config.width as f32 / config.height as f32;
        let half_fov = (self.camera.fov / 2.0).tan();
        
        let ndc_x = (mouse_pos.0 / config.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (mouse_pos.1 / config.height as f32) * 2.0;
        
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = forward.cross(right).normalize();
        
        // Use same formula as hex grid's screen_to_ray (which works)
        let ray_dir = (forward + right * ndc_x * half_fov * aspect + up * ndc_y * half_fov)
            .normalize();
        
        let ray_origin = self.camera.position;
        let max_dist = 50.0;
        let step_size = 0.1;
        let mut t = 0.5;
        
        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            
            // Check if we hit a block
            if let Some((block_id, dist)) = self.block_manager.find_closest(p, 1.0) {
                if dist < 0.3 {
                    if let Some(block) = self.block_manager.get_block(block_id) {
                        let aabb = block.aabb();
                        let center = (aabb.min + aabb.max) * 0.5;
                        let half_size = (aabb.max - aabb.min) * 0.5;
                        
                        // Determine which face was hit
                        let offset = p - center;
                        let abs_offset = Vec3::new(offset.x.abs(), offset.y.abs(), offset.z.abs());
                        
                        let (normal, face_pos, size) = if abs_offset.x > abs_offset.y && abs_offset.x > abs_offset.z {
                            // X face (left/right)
                            let n = Vec3::new(offset.x.signum(), 0.0, 0.0);
                            let pos = center + n * half_size.x;
                            (n, pos, (half_size.z * 2.0, half_size.y * 2.0))
                        } else if abs_offset.y > abs_offset.z {
                            // Y face (top/bottom)
                            let n = Vec3::new(0.0, offset.y.signum(), 0.0);
                            let pos = center + n * half_size.y;
                            (n, pos, (half_size.x * 2.0, half_size.z * 2.0))
                        } else {
                            // Z face (front/back)
                            let n = Vec3::new(0.0, 0.0, offset.z.signum());
                            let pos = center + n * half_size.z;
                            (n, pos, (half_size.x * 2.0, half_size.y * 2.0))
                        };
                        
                        return Some(SelectedFace {
                            block_id,
                            position: face_pos,
                            normal,
                            size,
                        });
                    }
                }
            }
            t += step_size;
        }
        None
    }
    
    /// Create a bridge between two selected faces
    fn create_bridge(&mut self) {
        let first = match self.build_toolbar.bridge_tool.first_face {
            Some(f) => f,
            None => return,
        };
        let second = match self.build_toolbar.bridge_tool.second_face {
            Some(f) => f,
            None => return,
        };
        
        // Calculate bridge parameters
        let start = first.position;
        let end = second.position;
        let direction = end - start;
        let length = direction.length();
        
        if length < 0.1 {
            println!("[Bridge] Faces too close together!");
            return;
        }
        
        // Create multiple connected blocks along the bridge
        let num_segments = (length / BLOCK_GRID_SIZE).ceil() as i32;
        let segment_length = length / num_segments as f32;
        let dir_normalized = direction.normalize();
        
        // Average the face sizes for the bridge cross-section
        let bridge_width = (first.size.0 + second.size.0) * 0.5;
        let bridge_height = (first.size.1 + second.size.1) * 0.5;
        
        println!("[Bridge] Creating bridge with {} segments, length={:.1}", num_segments, length);
        
        for i in 0..=num_segments {
            let t = i as f32 / num_segments as f32;
            let pos = start + direction * t;
            
            // Interpolate size from first to second face
            let w = first.size.0 * (1.0 - t) + second.size.0 * t;
            let h = first.size.1 * (1.0 - t) + second.size.1 * t;
            
            // Create a cube block at this position with interpolated size
            let half_extents = Vec3::new(
                segment_length * 0.6,
                h * 0.5,
                w * 0.5,
            );
            
            let shape = BuildingBlockShape::Cube { half_extents };
            let block = BuildingBlock::new(shape, pos, self.build_toolbar.selected_material);
            self.block_manager.add_block(block);
        }
        
        println!("[Bridge] Bridge created between blocks {} and {}", first.block_id, second.block_id);
        self.regenerate_block_mesh();
    }
    
    /// Check physics support for all blocks - unsupported blocks fall
    fn check_physics_support(&mut self) {
        // Collect blocks that need to fall
        let mut blocks_to_fall: Vec<(u32, Vec3, u8)> = Vec::new();
        
        // Ground level threshold
        let ground_threshold = 0.1;
        
        // First pass: identify unsupported blocks
        for block in self.block_manager.blocks() {
            let aabb = block.aabb();
            let bottom_y = aabb.min.y;
            
            // Check if block is on ground
            let ground_height = terrain_height_at(block.position.x, block.position.z, 0.0);
            if bottom_y <= ground_height + ground_threshold {
                continue; // On ground, supported
            }
            
            // Check if block is supported by another block
            let mut has_support = false;
            let check_pos = Vec3::new(block.position.x, aabb.min.y - 0.1, block.position.z);
            
            for other_block in self.block_manager.blocks() {
                if other_block.id == block.id {
                    continue;
                }
                let other_aabb = other_block.aabb();
                
                // Check if other block is below and overlapping
                if other_aabb.max.y >= aabb.min.y - 0.2 && other_aabb.max.y < aabb.min.y + 0.1 {
                    // Check XZ overlap
                    if aabb.min.x < other_aabb.max.x && aabb.max.x > other_aabb.min.x &&
                       aabb.min.z < other_aabb.max.z && aabb.max.z > other_aabb.min.z {
                        has_support = true;
                        break;
                    }
                }
            }
            
            if !has_support {
                blocks_to_fall.push((block.id, block.position, block.material));
            }
        }
        
        // Remove unsupported blocks and create falling prisms
        for (block_id, position, material) in blocks_to_fall {
            self.block_manager.remove_block(block_id);
            
            // Create a falling prism for visual effect
            let falling = FallingPrism::new((0, 0, 0), position, material);
            self.falling_prisms.push(falling);
            
            println!("[Physics] Block {} lost support and is falling!", block_id);
        }
        
        // Regenerate mesh if any blocks fell
        if !self.falling_prisms.is_empty() {
            self.regenerate_block_mesh();
        }
    }
    
    /// Raycast from camera to find which building block is hit
    fn raycast_to_block(&self) -> Option<u32> {
        let mouse_pos = self.current_mouse_pos?;
        let config = self.surface_config.as_ref()?;
        
        // Convert screen position to ray
        let aspect = config.width as f32 / config.height as f32;
        let half_fov = (self.camera.fov / 2.0).tan();
        
        // NDC coordinates (-1 to 1)
        let ndc_x = (mouse_pos.0 / config.width as f32) * 2.0 - 1.0;
        let ndc_y = 1.0 - (mouse_pos.1 / config.height as f32) * 2.0;
        
        // Calculate ray direction
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = forward.cross(right).normalize();
        
        // Use same formula as hex grid's screen_to_ray (which works)
        let ray_dir = (forward + right * ndc_x * half_fov * aspect + up * ndc_y * half_fov)
            .normalize();
        
        let ray_origin = self.camera.position;
        
        // Find closest block by marching along ray and checking SDF
        let max_dist = 100.0;
        let step_size = 0.25;
        let mut t = 0.5;
        
        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            
            // Check if we're inside any block
            if let Some((block_id, dist)) = self.block_manager.find_closest(p, 0.5) {
                if dist < 0.5 {
                    return Some(block_id);
                }
            }
            
            t += step_size;
        }
        
        None
    }
    
    /// Handle block click for double-click merging
    fn handle_block_click(&mut self) -> bool {
        if let Some(block_id) = self.raycast_to_block() {
            // Check for double-click
            if let Some(blocks_to_merge) = self.merge_workflow.on_block_click(block_id, &self.block_manager) {
                // Double-click detected! Merge connected blocks
                println!("[Merge] Double-click on block {}, merging {} connected blocks", 
                    block_id, blocks_to_merge.len());
                
                // Get material color based on first block
                let color = if let Some(block) = self.block_manager.get_block(blocks_to_merge[0]) {
                    get_material_color(block.material)
                } else {
                    [0.5, 0.5, 0.5, 1.0]
                };
                
                // Perform merge
                if let Some(merged) = self.merge_workflow.merge_blocks(
                    &blocks_to_merge, 
                    &mut self.block_manager,
                    color
                ) {
                    println!("[Merge] Created merged mesh with {} vertices", merged.vertices.len());
                    self.create_merged_mesh_buffers(merged);
                }
                
                // Regenerate remaining block mesh
                self.regenerate_block_mesh();
                return true;
            }
        }
        false
    }
    
    /// Check player collision with building blocks and push out if overlapping
    fn player_block_collision(&mut self) {
        let player_pos = self.player.position;
        let player_top = player_pos.y + PLAYER_EYE_HEIGHT + 0.2; // Player capsule top
        
        // Check each block for collision
        for block in self.block_manager.blocks() {
            let aabb = block.aabb();
            
            // Simple capsule-AABB collision
            // Check if player cylinder overlaps with block AABB
            let closest_x = player_pos.x.clamp(aabb.min.x, aabb.max.x);
            let closest_z = player_pos.z.clamp(aabb.min.z, aabb.max.z);
            
            let dx = player_pos.x - closest_x;
            let dz = player_pos.z - closest_z;
            let horizontal_dist = (dx * dx + dz * dz).sqrt();
            
            // Player capsule radius
            let player_radius = 0.3;
            
            // Check horizontal overlap
            if horizontal_dist < player_radius {
                // Check vertical overlap
                let in_vertical_range = player_pos.y < aabb.max.y && player_top > aabb.min.y;
                
                if in_vertical_range {
                    // Collision! Push player out horizontally
                    if horizontal_dist > 0.001 {
                        let push_dir = Vec3::new(dx, 0.0, dz).normalize();
                        let penetration = player_radius - horizontal_dist;
                        self.player.position += push_dir * (penetration + 0.01);
                        
                        // Also stop velocity in that direction
                        let vel_dot = self.player.velocity.dot(push_dir);
                        if vel_dot < 0.0 {
                            self.player.velocity -= push_dir * vel_dot;
                        }
                    } else {
                        // Player is inside block, push toward center of AABB's nearest face
                        let block_center = (aabb.min + aabb.max) * 0.5;
                        let to_player = player_pos - block_center;
                        let push_dir = Vec3::new(to_player.x.signum(), 0.0, to_player.z.signum()).normalize_or_zero();
                        self.player.position += push_dir * (player_radius + 0.1);
                    }
                }
                
                // Check if player is standing on top of block
                if player_pos.y >= aabb.max.y - 0.1 && player_pos.y <= aabb.max.y + 0.5 {
                    let on_top_xz = player_pos.x >= aabb.min.x - player_radius 
                        && player_pos.x <= aabb.max.x + player_radius
                        && player_pos.z >= aabb.min.z - player_radius 
                        && player_pos.z <= aabb.max.z + player_radius;
                    
                    if on_top_xz && self.player.vertical_velocity <= 0.0 {
                        // Land on top of block
                        self.player.position.y = aabb.max.y;
                        self.player.vertical_velocity = 0.0;
                        self.player.is_grounded = true;
                        self.player.coyote_time_remaining = COYOTE_TIME;
                    }
                }
            }
        }
    }
    
    /// Check player collision with hex prism walls
    fn player_hex_collision(&mut self) {
        let player_pos = self.player.position;
        let player_radius = 0.3;
        
        // Iterate through nearby hex prisms
        for ((q, r, level), prism) in self.hex_wall_grid.iter() {
            // Convert hex coordinates to world position
            let hex_x = (*q as f32) * DEFAULT_HEX_RADIUS * 1.5;
            let hex_z = (*r as f32) * DEFAULT_HEX_RADIUS * 3.0_f32.sqrt() 
                + (*q as f32).abs() % 2.0 * DEFAULT_HEX_RADIUS * 3.0_f32.sqrt() * 0.5;
            let hex_y = (*level as f32) * DEFAULT_HEX_HEIGHT;
            
            // Simple distance check for collision
            let dx = player_pos.x - hex_x;
            let dz = player_pos.z - hex_z;
            let horizontal_dist = (dx * dx + dz * dz).sqrt();
            
            // Hex prism collision radius (inscribed circle)
            let hex_collision_radius = DEFAULT_HEX_RADIUS * 0.866; // cos(30°) ≈ 0.866
            
            if horizontal_dist < hex_collision_radius + player_radius {
                // Check vertical overlap
                let hex_bottom = hex_y;
                let hex_top = hex_y + DEFAULT_HEX_HEIGHT;
                let player_top = player_pos.y + PLAYER_EYE_HEIGHT + 0.2;
                
                let in_vertical_range = player_pos.y < hex_top && player_top > hex_bottom;
                
                if in_vertical_range {
                    // Push player out
                    if horizontal_dist > 0.001 {
                        let push_dir = Vec3::new(dx, 0.0, dz).normalize();
                        let penetration = (hex_collision_radius + player_radius) - horizontal_dist;
                        self.player.position += push_dir * (penetration + 0.01);
                        
                        // Stop velocity in push direction
                        let vel_dot = self.player.velocity.dot(push_dir);
                        if vel_dot < 0.0 {
                            self.player.velocity -= push_dir * vel_dot;
                        }
                    }
                }
                
                // Check if player is standing on top of hex prism
                if player_pos.y >= hex_top - 0.1 && player_pos.y <= hex_top + 0.5 {
                    if horizontal_dist < hex_collision_radius + player_radius 
                        && self.player.vertical_velocity <= 0.0 
                    {
                        // Land on top of hex prism
                        self.player.position.y = hex_top;
                        self.player.vertical_velocity = 0.0;
                        self.player.is_grounded = true;
                        self.player.coyote_time_remaining = COYOTE_TIME;
                    }
                }
            }
        }
    }
    
    /// Create GPU buffers for a merged mesh
    fn create_merged_mesh_buffers(&mut self, merged: MergedMesh) {
        let device = match &self.device {
            Some(d) => d,
            None => return,
        };
        
        if merged.vertices.is_empty() {
            return;
        }
        
        // Convert to Vertex format
        let mesh_vertices: Vec<Vertex> = merged.vertices.iter().map(|v| Vertex {
            position: v.position,
            normal: v.normal,
            color: v.color,
        }).collect();
        
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Merged Mesh Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Merged Mesh Index Buffer"),
            contents: bytemuck::cast_slice(&merged.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        
        self.merged_mesh_buffers.push(MergedMeshBuffers {
            id: merged.id,
            vertex_buffer,
            index_buffer,
            index_count: merged.indices.len() as u32,
        });
        
        println!("[Merge] Created GPU buffers for merged mesh {}", merged.id);
    }
    
    /// Regenerate the building block mesh buffer
    fn regenerate_block_mesh(&mut self) {
        let device = match &self.device {
            Some(d) => d,
            None => return,
        };
        let queue = match &self.queue {
            Some(q) => q,
            None => return,
        };
        
        // Generate combined mesh from all blocks
        let (vertices, indices) = self.block_manager.generate_combined_mesh();
        
        if vertices.is_empty() {
            self.block_index_count = 0;
            return;
        }
        
        // Convert BlockVertex to Vertex (same layout)
        let mesh_vertices: Vec<Vertex> = vertices.iter().map(|v| Vertex {
            position: v.position,
            normal: v.normal,
            color: v.color,
        }).collect();
        
        // Create or update buffers
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Block Vertex Buffer"),
            contents: bytemuck::cast_slice(&mesh_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Block Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        
        self.block_vertex_buffer = Some(vertex_buffer);
        self.block_index_buffer = Some(index_buffer);
        self.block_index_count = indices.len() as u32;
        
        println!("[Build] Mesh updated: {} vertices, {} indices", 
            mesh_vertices.len(), indices.len());
    }
    
    // ========================================================================
    // TERRAIN REBUILD (called when UI applies changes)
    // ========================================================================
    
    fn rebuild_terrain(&mut self) {
        let device = self.device.as_ref().expect("Device not initialized");
        let queue = self.queue.as_ref().expect("Queue not initialized");
        
        // Regenerate terrain mesh
        let mut static_mesh = Mesh::new();
        
        // Attacker platform
        let attacker_platform = generate_elevated_hex_terrain(
            Vec3::new(0.0, 0.0, 25.0),
            30.0,
            [0.4, 0.5, 0.3, 1.0],
            64,
        );
        static_mesh.merge(&attacker_platform);
        
        // Water for attacker
        let params = get_terrain_params();
        if params.water > 0.01 {
            let attacker_water = generate_water_plane(Vec3::new(0.0, 0.0, 25.0), 30.0);
            static_mesh.merge(&attacker_water);
        }
        
        // Defender platform
        let defender_platform = generate_elevated_hex_terrain(
            Vec3::new(0.0, 0.0, -25.0),
            30.0,
            [0.5, 0.4, 0.35, 1.0],
            64,
        );
        static_mesh.merge(&defender_platform);
        
        // Water for defender
        if params.water > 0.01 {
            let defender_water = generate_water_plane(Vec3::new(0.0, 0.0, -25.0), 30.0);
            static_mesh.merge(&defender_water);
        }
        
        // Update buffers
        self.static_vertex_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Static Vertex Buffer"),
            contents: bytemuck::cast_slice(&static_mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        }));
        
        self.static_index_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Static Index Buffer"),
            contents: bytemuck::cast_slice(&static_mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        }));
        
        self.static_index_count = static_mesh.indices.len() as u32;
        
        println!("Terrain rebuilt! {} vertices, {} triangles", 
            static_mesh.vertices.len(), 
            static_mesh.indices.len() / 3);
        
        // Also regenerate trees (they depend on terrain height)
        self.trees_attacker = generate_trees_on_terrain(
            Vec3::new(0.0, 0.0, 25.0), 28.0, 0.3, 0.0,
        );
        self.trees_defender = generate_trees_on_terrain(
            Vec3::new(0.0, 0.0, -25.0), 28.0, 0.35, 100.0,
        );
        
        let mut all_trees = self.trees_attacker.clone();
        all_trees.extend(self.trees_defender.clone());
        let tree_mesh = generate_all_trees_mesh(&all_trees);
        
        if !tree_mesh.vertices.is_empty() {
            self.tree_vertex_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tree Vertex Buffer"),
                contents: bytemuck::cast_slice(&tree_mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }));
            self.tree_index_buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tree Index Buffer"),
                contents: bytemuck::cast_slice(&tree_mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            }));
            self.tree_index_count = tree_mesh.indices.len() as u32;
        }
        
        println!("[Trees] Regenerated {} trees", all_trees.len());
    }
    
    // ========================================================================
    // BUILDER MODE: Raycast and grid positioning
    // ========================================================================
    
    /// Convert screen coordinates to a world-space ray
    fn screen_to_ray(&self, screen_x: f32, screen_y: f32) -> (Vec3, Vec3) {
        let Some(ref config) = self.surface_config else {
            return (self.camera.position, self.camera.get_forward());
        };
        
        let width = config.width as f32;
        let height = config.height as f32;
        
        // Convert to normalized device coordinates (-1 to 1)
        let ndc_x = (2.0 * screen_x / width) - 1.0;
        let ndc_y = 1.0 - (2.0 * screen_y / height); // Flip Y
        
        // Get camera vectors
        let forward = self.camera.get_forward();
        let right = self.camera.get_right();
        let up = right.cross(forward).normalize();
        
        // Calculate ray direction based on FOV and aspect ratio
        let aspect = width / height;
        let half_fov_tan = (self.camera.fov / 2.0).tan();
        
        let ray_dir = (forward + right * ndc_x * half_fov_tan * aspect + up * ndc_y * half_fov_tan).normalize();
        
        (self.camera.position, ray_dir)
    }
    
    /// Raycast from screen position to find build position on terrain or existing prisms
    fn raycast_to_build_position(&self, screen_x: f32, screen_y: f32) -> Option<(i32, i32, i32)> {
        let (ray_origin, ray_dir) = self.screen_to_ray(screen_x, screen_y);
        
        // First, check if we hit an existing prism (to build on top)
        if let Some(hit) = self.hex_wall_grid.ray_cast(ray_origin, ray_dir, 200.0) {
            // Place on top of the hit prism
            let above = (hit.prism_coord.0, hit.prism_coord.1, hit.prism_coord.2 + 1);
            return Some(above);
        }
        
        // Otherwise, raycast against the terrain (both hex platforms)
        // Use an iterative approach to find where the ray intersects the terrain
        let attacker_center = Vec3::new(0.0, 0.0, 25.0);
        let defender_center = Vec3::new(0.0, 0.0, -25.0);
        let platform_radius = 30.0;
        
        // March along the ray to find terrain intersection
        let max_dist = 200.0;
        let step_size = 0.5;
        let mut t = 0.0;
        
        while t < max_dist {
            let point = ray_origin + ray_dir * t;
            
            // Check if point is within either hex platform bounds
            let in_attacker = (point.x - attacker_center.x).powi(2) + (point.z - attacker_center.z).powi(2) < platform_radius * platform_radius;
            let in_defender = (point.x - defender_center.x).powi(2) + (point.z - defender_center.z).powi(2) < platform_radius * platform_radius;
            
            if in_attacker || in_defender {
                let base_y = if in_attacker { attacker_center.y } else { defender_center.y };
                let terrain_y = terrain_height_at(point.x, point.z, base_y);
                
                // Check if ray has crossed below terrain
                if point.y <= terrain_y + 0.1 {
                    // Found intersection! Convert to grid coordinates
                    let build_level = self.builder_mode.build_level;
                    let (q, r, _) = magic_engine::render::hex_prism::world_to_axial(point);
                    return Some((q, r, build_level));
                }
            }
            
            t += step_size;
        }
        
        None
    }
    
    /// Update builder mode cursor position from current mouse position
    fn update_builder_cursor(&mut self) {
        if !self.builder_mode.enabled {
            self.builder_mode.cursor_coord = None;
            return;
        }
        
        if let Some((mx, my)) = self.current_mouse_pos {
            self.builder_mode.cursor_coord = self.raycast_to_build_position(mx, my);
        }
    }
    
    /// Generate ghost preview mesh for builder mode
    fn generate_ghost_preview_mesh(&self, time: f32) -> Option<Mesh> {
        if !self.builder_mode.enabled || !self.builder_mode.show_preview {
            return None;
        }
        
        let coord = self.builder_mode.cursor_coord?;
        
        // Don't show ghost if position is already occupied
        if self.hex_wall_grid.contains(coord.0, coord.1, coord.2) {
            return None;
        }
        
        // Create a ghost prism at the cursor position
        let world_pos = magic_engine::render::hex_prism::axial_to_world(coord.0, coord.1, coord.2);
        let prism = magic_engine::render::HexPrism::with_center(
            world_pos,
            magic_engine::render::hex_prism::DEFAULT_HEX_HEIGHT,
            magic_engine::render::hex_prism::DEFAULT_HEX_RADIUS,
            self.builder_mode.selected_material,
        );
        
        let (hex_vertices, hex_indices) = prism.generate_mesh();
        
        // Convert to our Vertex format with pulsing green color
        let pulse = 0.5 + (time * 4.0).sin() * 0.3;
        let ghost_color = [0.3, 0.9, 0.4, pulse]; // Green with pulsing alpha
        
        let vertices: Vec<Vertex> = hex_vertices
            .iter()
            .map(|v| Vertex {
                position: v.position,
                normal: v.normal,
                color: ghost_color,
            })
            .collect();
        
        Some(Mesh { vertices, indices: hex_indices })
    }
    
    /// Generate grid overlay mesh for builder mode
    /// Shows hex grid cells around the cursor position
    fn generate_grid_overlay_mesh(&self) -> Option<Mesh> {
        if !self.builder_mode.enabled {
            return None;
        }
        
        let Some(coord) = self.builder_mode.cursor_coord else {
            return None;
        };
        
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        
        // Grid parameters
        let line_thickness = 0.04;
        let grid_radius: i32 = 3;
        let hex_radius = magic_engine::render::hex_prism::DEFAULT_HEX_RADIUS;
        
        // Generate hex outlines around cursor
        for dq in -grid_radius..=grid_radius {
            for dr in -grid_radius..=grid_radius {
                // Hex distance check (axial coordinates)
                let ds = -dq - dr;
                let hex_dist = (dq.abs() + dr.abs() + ds.abs()) / 2;
                if hex_dist > grid_radius {
                    continue;
                }
                
                let q = coord.0 + dq;
                let r = coord.1 + dr;
                let world_pos = magic_engine::render::hex_prism::axial_to_world(q, r, coord.2);
                
                // Color - highlight cursor cell differently
                let is_cursor = dq == 0 && dr == 0;
                let cell_color = if is_cursor {
                    [0.2, 1.0, 0.4, 0.9] // Bright green for cursor
                } else {
                    [0.3, 0.7, 1.0, 0.4] // Cyan for others
                };
                
                // Generate 6 edge quads for the hex outline
                for i in 0..6 {
                    let angle1 = (i as f32) * std::f32::consts::PI / 3.0;
                    let angle2 = ((i + 1) % 6) as f32 * std::f32::consts::PI / 3.0;
                    
                    // Hex vertices (pointy-top orientation)
                    let x1 = world_pos.x + hex_radius * angle1.sin();
                    let z1 = world_pos.z + hex_radius * angle1.cos();
                    let x2 = world_pos.x + hex_radius * angle2.sin();
                    let z2 = world_pos.z + hex_radius * angle2.cos();
                    
                    // Sample terrain height + small offset above terrain
                    let y1 = terrain_height_at(x1, z1, 0.0) + 0.1;
                    let y2 = terrain_height_at(x2, z2, 0.0) + 0.1;
                    
                    // Calculate perpendicular offset for line thickness
                    let edge_dx = x2 - x1;
                    let edge_dz = z2 - z1;
                    let edge_len = (edge_dx * edge_dx + edge_dz * edge_dz).sqrt();
                    if edge_len < 0.001 { continue; }
                    
                    let perp_x = -edge_dz / edge_len * line_thickness;
                    let perp_z = edge_dx / edge_len * line_thickness;
                    
                    // Create quad for this edge (4 vertices)
                    let base_idx = vertices.len() as u32;
                    
                    vertices.push(Vertex { 
                        position: [x1 - perp_x, y1, z1 - perp_z], 
                        normal: [0.0, 1.0, 0.0], 
                        color: cell_color 
                    });
                    vertices.push(Vertex { 
                        position: [x1 + perp_x, y1, z1 + perp_z], 
                        normal: [0.0, 1.0, 0.0], 
                        color: cell_color 
                    });
                    vertices.push(Vertex { 
                        position: [x2 + perp_x, y2, z2 + perp_z], 
                        normal: [0.0, 1.0, 0.0], 
                        color: cell_color 
                    });
                    vertices.push(Vertex { 
                        position: [x2 - perp_x, y2, z2 - perp_z], 
                        normal: [0.0, 1.0, 0.0], 
                        color: cell_color 
                    });
                    
                    // Two triangles per quad
                    indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
                    indices.extend_from_slice(&[base_idx, base_idx + 2, base_idx + 3]);
                }
            }
        }
        
        if vertices.is_empty() {
            None
        } else {
            Some(Mesh { vertices, indices })
        }
    }

    fn render(&mut self) {
        let Some(ref device) = self.device else { return };
        let Some(ref queue) = self.queue else { return };
        let Some(ref surface) = self.surface else { return };
        let Some(ref config) = self.surface_config else { return };
        let Some(ref pipeline) = self.pipeline else { return };
        let Some(ref uniform_buffer) = self.uniform_buffer else { return };
        let Some(ref bind_group) = self.uniform_bind_group else { return };
        let Some(ref static_vb) = self.static_vertex_buffer else { return };
        let Some(ref static_ib) = self.static_index_buffer else { return };
        let Some(ref dynamic_vb) = self.dynamic_vertex_buffer else { return };
        let Some(ref dynamic_ib) = self.dynamic_index_buffer else { return };
        let Some(ref hex_wall_vb) = self.hex_wall_vertex_buffer else { return };
        let Some(ref hex_wall_ib) = self.hex_wall_index_buffer else { return };
        let Some(ref depth_view) = self.depth_texture else { return };

        // Get surface texture
        let output = match surface.get_current_texture() {
            Ok(t) => t,
            Err(_) => return,
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Get time for animations
        let time = self.start_time.elapsed().as_secs_f32();
        
        // Build dynamic mesh (projectiles + builder mode preview)
        let mut dynamic_mesh = Mesh::new();

        // Projectile meshes (spheres)
        for projectile in &self.projectiles {
            let sphere = generate_sphere(projectile.position, projectile.radius, [0.2, 0.2, 0.2, 1.0], 4); // US-019: Reduced from 8 to 4 segments (4.5x fewer triangles)
            dynamic_mesh.merge(&sphere);
        }
        
        // Falling prisms (physics-based destruction)
        for falling in &self.falling_prisms {
            let color = get_material_color(falling.material);
            // Render as a simple box for performance (actual hex would need rotation math)
            let half_size = Vec3::new(DEFAULT_HEX_RADIUS * 0.8, DEFAULT_HEX_HEIGHT * 0.5, DEFAULT_HEX_RADIUS * 0.8);
            let box_mesh = generate_rotated_box(falling.position, half_size, falling.rotation, color);
            dynamic_mesh.merge(&box_mesh);
        }
        
        // Debris particles (small cubes for visibility)
        for particle in &self.debris_particles {
            if particle.is_alive() {
                // Fade out based on lifetime
                let alpha = (particle.lifetime / 3.0).min(1.0);
                let mut color = particle.color;
                color[3] = alpha;
                let cube = generate_sphere(particle.position, particle.size, color, 2); // Low-poly sphere
                dynamic_mesh.merge(&cube);
            }
        }
        
        // Builder mode: Ghost preview at cursor position
        if let Some(ghost_mesh) = self.generate_ghost_preview_mesh(time) {
            dynamic_mesh.merge(&ghost_mesh);
        }
        
        // Builder mode: Grid overlay around cursor
        if let Some(grid_mesh) = self.generate_grid_overlay_mesh() {
            dynamic_mesh.merge(&grid_mesh);
        }

        // Update dynamic buffers
        if !dynamic_mesh.vertices.is_empty() {
            queue.write_buffer(dynamic_vb, 0, bytemuck::cast_slice(&dynamic_mesh.vertices));
            queue.write_buffer(dynamic_ib, 0, bytemuck::cast_slice(&dynamic_mesh.indices));
        }
        self.dynamic_index_count = dynamic_mesh.indices.len() as u32;

        // Update uniforms
        let aspect = config.width as f32 / config.height as f32;
        let view_mat = self.camera.get_view_matrix();
        let proj_mat = self.camera.get_projection_matrix(aspect);
        let view_proj = proj_mat * view_mat;

        let mut uniforms = Uniforms::default();
        uniforms.view_proj = view_proj.to_cols_array_2d();
        uniforms.camera_pos = self.camera.position.to_array();
        uniforms.time = time;
        uniforms.projectile_count = self.projectiles.len() as u32;

        for (i, projectile) in self.projectiles.iter().enumerate().take(32) {
            uniforms.projectile_positions[i] = [
                projectile.position.x,
                projectile.position.y,
                projectile.position.z,
                projectile.radius,
            ];
        }

        queue.write_buffer(uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // ============================================
        // US-013: Update SDF cannon uniforms
        // ============================================
        if let (Some(sdf_uniform_buffer), Some(sdf_data_buffer)) =
            (&self.sdf_cannon_uniform_buffer, &self.sdf_cannon_data_buffer)
        {
            let inv_view_proj = view_proj.inverse();

            // SDF cannon uniforms (camera, time, lighting)
            let sdf_uniforms = SdfCannonUniforms {
                view_proj: view_proj.to_cols_array_2d(),
                inv_view_proj: inv_view_proj.to_cols_array_2d(),
                camera_pos: self.camera.position.to_array(),
                time,
                sun_dir: [0.577, 0.577, 0.577], // Normalized (1,1,1)
                fog_density: 0.0002,
                fog_color: [0.7, 0.8, 0.95],
                ambient: 0.3,
            };
            queue.write_buffer(sdf_uniform_buffer, 0, bytemuck::cast_slice(&[sdf_uniforms]));

            // SDF cannon data (position, rotation, color)
            // Convert barrel rotation from euler angles to quaternion
            let (sin_az, cos_az) = (self.cannon.barrel_azimuth * 0.5).sin_cos();
            let (sin_elev, cos_elev) = (self.cannon.barrel_elevation * 0.5).sin_cos();

            // Quaternion for Y rotation (azimuth) then X rotation (elevation)
            // q = q_y * q_x (apply elevation first, then azimuth)
            let quat_x = [sin_elev, 0.0, 0.0, cos_elev]; // Rotation around X
            let quat_y = [0.0, sin_az, 0.0, cos_az];     // Rotation around Y

            // Quaternion multiplication: q_y * q_x
            let barrel_rotation = [
                quat_y[3] * quat_x[0] + quat_y[0] * quat_x[3] + quat_y[1] * quat_x[2] - quat_y[2] * quat_x[1],
                quat_y[3] * quat_x[1] - quat_y[0] * quat_x[2] + quat_y[1] * quat_x[3] + quat_y[2] * quat_x[0],
                quat_y[3] * quat_x[2] + quat_y[0] * quat_x[1] - quat_y[1] * quat_x[0] + quat_y[2] * quat_x[3],
                quat_y[3] * quat_x[3] - quat_y[0] * quat_x[0] - quat_y[1] * quat_x[1] - quat_y[2] * quat_x[2],
            ];

            let sdf_cannon_data = SdfCannonData {
                world_pos: self.cannon.position.to_array(),
                _pad0: 0.0,
                barrel_rotation,
                color: [0.4, 0.35, 0.3], // Bronze/metallic color
                _pad1: 0.0,
            };
            queue.write_buffer(sdf_data_buffer, 0, bytemuck::cast_slice(&[sdf_cannon_data]));
        }

        // ============================================
        // Update stormy skybox uniforms
        // ============================================
        if let Some(ref mut stormy_sky) = self.stormy_sky {
            stormy_sky.update(
                queue,
                view_proj,
                self.camera.position,
                time,
                (config.width, config.height),
            );
        }

        // Create command encoder
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // ============================================
        // PASS 0: Stormy skybox (background)
        // ============================================
        if let Some(ref stormy_sky) = self.stormy_sky {
            stormy_sky.render_to_view(&mut encoder, &view);
        }

        // ============================================
        // PASS 1: Mesh rendering (terrain, walls, projectiles)
        // ============================================
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Mesh Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Load sky background (don't clear)
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(pipeline);
            render_pass.set_bind_group(0, bind_group, &[]);

            // Draw static mesh (terrain)
            render_pass.set_vertex_buffer(0, static_vb.slice(..));
            render_pass.set_index_buffer(static_ib.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.static_index_count, 0, 0..1);

            // Draw dynamic mesh (projectiles)
            if self.dynamic_index_count > 0 {
                render_pass.set_vertex_buffer(0, dynamic_vb.slice(..));
                render_pass.set_index_buffer(dynamic_ib.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..self.dynamic_index_count, 0, 0..1);
            }

            // Draw hex-prism walls (US-012)
            if self.hex_wall_index_count > 0 {
                render_pass.set_vertex_buffer(0, hex_wall_vb.slice(..));
                render_pass.set_index_buffer(hex_wall_ib.slice(..), wgpu::IndexFormat::Uint32);
                render_pass.draw_indexed(0..self.hex_wall_index_count, 0, 0..1);
            }
            
            // Draw building blocks (new system)
            if let (Some(block_vb), Some(block_ib)) = (&self.block_vertex_buffer, &self.block_index_buffer) {
                if self.block_index_count > 0 {
                    render_pass.set_vertex_buffer(0, block_vb.slice(..));
                    render_pass.set_index_buffer(block_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.block_index_count, 0, 0..1);
                }
            }
            
            // Draw block placement preview (Fallout 4 / Fortnite style ghost)
            if self.build_toolbar.visible && self.build_toolbar.show_preview && !self.build_toolbar.is_bridge_mode() {
                if let Some(preview_pos) = self.build_toolbar.preview_position {
                    // Generate preview mesh
                    let shape = self.build_toolbar.get_selected_shape();
                    let preview_block = BuildingBlock::new(shape, preview_pos, 0);
                    let (preview_verts, preview_indices) = preview_block.generate_mesh();
                    
                    // Pulsing effect based on time
                    let pulse = (self.start_time.elapsed().as_secs_f32() * 3.0).sin() * 0.5 + 0.5;
                    
                    // Bright green/yellow for valid placement (Fortnite style)
                    let r = 0.2 + pulse * 0.3;
                    let g = 0.9;
                    let b = 0.2 + pulse * 0.2;
                    let highlight_color = [r, g, b, 0.85];
                    
                    let preview_vertices: Vec<Vertex> = preview_verts.iter().map(|v| Vertex {
                        position: v.position,
                        normal: v.normal,
                        color: highlight_color,
                    }).collect();
                    
                    if !preview_vertices.is_empty() && !preview_indices.is_empty() {
                        let preview_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Preview Vertex Buffer"),
                            contents: bytemuck::cast_slice(&preview_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                        let preview_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Preview Index Buffer"),
                            contents: bytemuck::cast_slice(&preview_indices),
                            usage: wgpu::BufferUsages::INDEX,
                        });
                        
                        render_pass.set_vertex_buffer(0, preview_vb.slice(..));
                        render_pass.set_index_buffer(preview_ib.slice(..), wgpu::IndexFormat::Uint32);
                        render_pass.draw_indexed(0..preview_indices.len() as u32, 0, 0..1);
                    }
                    
                    // Draw wireframe outline (slightly larger, bright white edges)
                    let outline_scale = 1.05;
                    let outline_color = [1.0, 1.0, 1.0, 0.9];
                    let outline_vertices: Vec<Vertex> = preview_verts.iter().map(|v| {
                        // Scale position slightly outward from center
                        let dir = Vec3::from(v.position) - preview_pos;
                        let scaled_pos = preview_pos + dir * outline_scale;
                        Vertex {
                            position: scaled_pos.to_array(),
                            normal: v.normal,
                            color: outline_color,
                        }
                    }).collect();
                    
                    // Generate wireframe indices (edges only)
                    let mut wireframe_indices: Vec<u32> = Vec::new();
                    for chunk in preview_indices.chunks(3) {
                        if chunk.len() == 3 {
                            // Triangle edges
                            wireframe_indices.push(chunk[0]);
                            wireframe_indices.push(chunk[1]);
                            wireframe_indices.push(chunk[1]);
                            wireframe_indices.push(chunk[2]);
                            wireframe_indices.push(chunk[2]);
                            wireframe_indices.push(chunk[0]);
                        }
                    }
                    
                    if !outline_vertices.is_empty() {
                        let outline_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                            label: Some("Outline Vertex Buffer"),
                            contents: bytemuck::cast_slice(&outline_vertices),
                            usage: wgpu::BufferUsages::VERTEX,
                        });
                        
                        // For wireframe, we still use the triangle indices but render as lines
                        // Note: wgpu doesn't have LINE mode in the main pipeline, so we render as triangles
                        // but with the outline effect from scaling
                        render_pass.set_vertex_buffer(0, outline_vb.slice(..));
                        render_pass.draw_indexed(0..preview_indices.len() as u32, 0, 0..1);
                    }
                }
            }
            
            // Draw merged meshes (baked from SDF)
            for merged in &self.merged_mesh_buffers {
                if merged.index_count > 0 {
                    render_pass.set_vertex_buffer(0, merged.vertex_buffer.slice(..));
                    render_pass.set_index_buffer(merged.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..merged.index_count, 0, 0..1);
                }
            }
            
            // Draw trees
            if let (Some(tree_vb), Some(tree_ib)) = (&self.tree_vertex_buffer, &self.tree_index_buffer) {
                if self.tree_index_count > 0 {
                    render_pass.set_vertex_buffer(0, tree_vb.slice(..));
                    render_pass.set_index_buffer(tree_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..self.tree_index_count, 0, 0..1);
                }
            }
        }

        // ============================================
        // PASS 2: SDF Cannon rendering (US-013)
        // ============================================
        if let (Some(sdf_pipeline), Some(sdf_bind_group)) =
            (&self.sdf_cannon_pipeline, &self.sdf_cannon_bind_group)
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("SDF Cannon Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Preserve previous pass
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load, // Use existing depth
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(sdf_pipeline);
            render_pass.set_bind_group(0, sdf_bind_group, &[]);
            // Fullscreen triangle - 3 vertices, no vertex buffer needed
            render_pass.draw(0..3, 0..1);
        }
        
        // ============================================
        // PASS 3: UI Overlay (terrain editor sliders)
        // ============================================
        if self.terrain_ui.visible {
            if let (Some(ui_pipeline), Some(ui_bg)) = (&self.ui_pipeline, &self.ui_bind_group) {
                let config = self.surface_config.as_ref().unwrap();
                let ui_mesh = self.terrain_ui.generate_ui_mesh(
                    config.width as f32,
                    config.height as f32
                );
                
                if !ui_mesh.vertices.is_empty() {
                    // Create temporary UI vertex/index buffers
                    let ui_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("UI Vertex Buffer"),
                        contents: bytemuck::cast_slice(&ui_mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let ui_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("UI Index Buffer"),
                        contents: bytemuck::cast_slice(&ui_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    
                    // Render UI with depth testing disabled (render on top)
                    // Uses separate UI bind group with identity matrix (no 3D transform)
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("UI Render Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load, // Preserve previous passes
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None, // No depth testing for UI
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    
                    render_pass.set_pipeline(ui_pipeline);
                    render_pass.set_bind_group(0, ui_bg, &[]); // Use UI bind group with identity matrix
                    render_pass.set_vertex_buffer(0, ui_vertex_buffer.slice(..));
                    render_pass.set_index_buffer(ui_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..ui_mesh.indices.len() as u32, 0, 0..1);
                }
            }
        }
        
        // ============================================
        // PASS 4: Selection Cursor (when in build mode)
        // ============================================
        if self.build_toolbar.visible && !self.start_overlay.visible {
            if let (Some(ui_pipeline), Some(ui_bg)) = (&self.ui_pipeline, &self.ui_bind_group) {
                let config = self.surface_config.as_ref().unwrap();
                let w = config.width as f32;
                let h = config.height as f32;
                
                // Use mouse position for cursor, fallback to center
                let (cx, cy) = self.current_mouse_pos.unwrap_or((w / 2.0, h / 2.0));
                
                // Selection cursor dimensions (larger for visibility)
                let size = 25.0;
                let thickness = 4.0;
                let gap = 8.0;
                
                // Pulsing bright green/yellow color
                let pulse = (self.start_time.elapsed().as_secs_f32() * 3.0).sin() * 0.3 + 0.7;
                let crosshair_color = [pulse, 1.0, 0.2, 0.95];
                
                let to_ndc = |x: f32, y: f32| -> [f32; 3] {
                    [(x / w) * 2.0 - 1.0, 1.0 - (y / h) * 2.0, 0.0]
                };
                
                let mut cross_verts = Vec::new();
                let mut cross_indices = Vec::new();
                
                // Helper to add a quad
                let add_cross_quad = |verts: &mut Vec<Vertex>, indices: &mut Vec<u32>, 
                    x1: f32, y1: f32, x2: f32, y2: f32, color: [f32; 4]| {
                    let base = verts.len() as u32;
                    let normal = [0.0, 0.0, 1.0];
                    verts.push(Vertex { position: to_ndc(x1, y1), normal, color });
                    verts.push(Vertex { position: to_ndc(x2, y1), normal, color });
                    verts.push(Vertex { position: to_ndc(x2, y2), normal, color });
                    verts.push(Vertex { position: to_ndc(x1, y2), normal, color });
                    indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
                };
                
                // Horizontal line (left part)
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx - size - gap, cy - thickness/2.0,
                    cx - gap, cy + thickness/2.0, crosshair_color);
                // Horizontal line (right part)
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx + gap, cy - thickness/2.0,
                    cx + size + gap, cy + thickness/2.0, crosshair_color);
                // Vertical line (top part)
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx - thickness/2.0, cy - size - gap,
                    cx + thickness/2.0, cy - gap, crosshair_color);
                // Vertical line (bottom part)
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx - thickness/2.0, cy + gap,
                    cx + thickness/2.0, cy + size + gap, crosshair_color);
                // Center dot
                add_cross_quad(&mut cross_verts, &mut cross_indices,
                    cx - 2.0, cy - 2.0, cx + 2.0, cy + 2.0, [1.0, 1.0, 1.0, 1.0]);
                
                if !cross_verts.is_empty() {
                    let cross_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Crosshair VB"),
                        contents: bytemuck::cast_slice(&cross_verts),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let cross_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Crosshair IB"),
                        contents: bytemuck::cast_slice(&cross_indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Crosshair Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store, },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    
                    render_pass.set_pipeline(ui_pipeline);
                    render_pass.set_bind_group(0, ui_bg, &[]);
                    render_pass.set_vertex_buffer(0, cross_vb.slice(..));
                    render_pass.set_index_buffer(cross_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..cross_indices.len() as u32, 0, 0..1);
                }
            }
        }
        
        // ============================================
        // PASS 5: Build Toolbar UI
        // ============================================
        if self.build_toolbar.visible {
            if let (Some(ui_pipeline), Some(ui_bg)) = (&self.ui_pipeline, &self.ui_bind_group) {
                let config = self.surface_config.as_ref().unwrap();
                let toolbar_mesh = self.build_toolbar.generate_ui_mesh(
                    config.width as f32,
                    config.height as f32
                );
                
                if !toolbar_mesh.vertices.is_empty() {
                    let toolbar_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Toolbar Vertex Buffer"),
                        contents: bytemuck::cast_slice(&toolbar_mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let toolbar_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Toolbar Index Buffer"),
                        contents: bytemuck::cast_slice(&toolbar_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Build Toolbar Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    
                    render_pass.set_pipeline(ui_pipeline);
                    render_pass.set_bind_group(0, ui_bg, &[]);
                    render_pass.set_vertex_buffer(0, toolbar_vb.slice(..));
                    render_pass.set_index_buffer(toolbar_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..toolbar_mesh.indices.len() as u32, 0, 0..1);
                }
            }
        }
        
        // ============================================
        // PASS 6: Start Overlay (Windows focus)
        // ============================================
        if self.start_overlay.visible {
            if let (Some(ui_pipeline), Some(ui_bg)) = (&self.ui_pipeline, &self.ui_bind_group) {
                let config = self.surface_config.as_ref().unwrap();
                let overlay_mesh = self.start_overlay.generate_ui_mesh(
                    config.width as f32,
                    config.height as f32
                );
                
                if !overlay_mesh.vertices.is_empty() {
                    let overlay_vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Overlay Vertex Buffer"),
                        contents: bytemuck::cast_slice(&overlay_mesh.vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                    let overlay_ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Overlay Index Buffer"),
                        contents: bytemuck::cast_slice(&overlay_mesh.indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
                    
                    let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("Start Overlay Pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Load,
                                store: wgpu::StoreOp::Store,
                            },
                            depth_slice: None,
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    
                    render_pass.set_pipeline(ui_pipeline);
                    render_pass.set_bind_group(0, ui_bg, &[]);
                    render_pass.set_vertex_buffer(0, overlay_vb.slice(..));
                    render_pass.set_index_buffer(overlay_ib.slice(..), wgpu::IndexFormat::Uint32);
                    render_pass.draw_indexed(0..overlay_mesh.indices.len() as u32, 0, 0..1);
                }
            }
        }

        queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }

    fn handle_key(&mut self, key: KeyCode, pressed: bool) {
        match key {
            // Movement
            KeyCode::KeyW => self.movement.forward = pressed,
            KeyCode::KeyS => self.movement.backward = pressed,
            KeyCode::KeyA => self.movement.left = pressed,
            KeyCode::KeyD => self.movement.right = pressed,
            KeyCode::Space => {
                if pressed {
                    if self.first_person_mode {
                        // Jump in first-person mode
                        self.player.request_jump();
                    } else if !self.builder_mode.enabled {
                        // Fire projectile in free camera mode
                        self.fire_projectile();
                    }
                }
                self.movement.up = pressed;
            }
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.movement.sprint = pressed;
                self.movement.down = pressed;
            }
            
            // Ctrl key for copy/paste/undo
            KeyCode::ControlLeft | KeyCode::ControlRight => {
                self.builder_mode.ctrl_held = pressed;
            }
            
            // Builder mode toggle (B key)
            KeyCode::KeyB if pressed => {
                self.builder_mode.toggle();
                self.build_toolbar.toggle();
                
                // In build mode: show cursor for selection
                // Out of build mode: hide cursor for FPS look
                if let Some(window) = &self.window {
                    if self.build_toolbar.visible {
                        // Show cursor for selection
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                        println!("[Build Mode] Cursor visible - click to place blocks");
                    } else {
                        // Hide cursor for FPS mode
                        if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                        }
                        window.set_cursor_visible(false);
                    }
                }
            }
            
            // Tab - cycle through shapes in build toolbar
            KeyCode::Tab if pressed => {
                if self.build_toolbar.visible {
                    self.build_toolbar.next_shape();
                }
            }
            
            // Number keys 1-6 for direct shape selection
            KeyCode::Digit1 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(0);
            }
            KeyCode::Digit2 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(1);
            }
            KeyCode::Digit3 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(2);
            }
            KeyCode::Digit4 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(3);
            }
            KeyCode::Digit5 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(4);
            }
            KeyCode::Digit6 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(5);
            }
            KeyCode::Digit7 if pressed && self.build_toolbar.visible => {
                self.build_toolbar.select_shape(6); // Bridge tool
            }
            
            // Arrow keys for shape selection in build mode
            KeyCode::ArrowUp if pressed && self.build_toolbar.visible => {
                self.build_toolbar.prev_shape();
            }
            KeyCode::ArrowDown if pressed && self.build_toolbar.visible => {
                self.build_toolbar.next_shape();
            }
            
            // F11 - Fullscreen toggle
            KeyCode::F11 if pressed => {
                if let Some(window) = &self.window {
                    let current = window.fullscreen();
                    window.set_fullscreen(if current.is_some() {
                        None // Back to windowed
                    } else {
                        Some(Fullscreen::Borderless(None)) // Borderless fullscreen
                    });
                    println!("Fullscreen: {}", current.is_none());
                }
            }
            
            // Terrain editor toggle (T key) - shows on-screen UI sliders
            KeyCode::KeyT if pressed => {
                self.terrain_ui.toggle();
                if self.terrain_ui.visible {
                    println!("=== TERRAIN EDITOR UI OPEN ===");
                    println!("Click and drag sliders to adjust terrain");
                    println!("Click APPLY button to rebuild terrain");
                } else {
                    println!("Terrain editor closed");
                }
            }
            
            // Toggle first-person mode (V key)
            KeyCode::KeyV if pressed => {
                self.first_person_mode = !self.first_person_mode;
                if self.first_person_mode {
                    // Sync player position to current camera when entering FPS mode
                    self.player.position = self.camera.position - Vec3::new(0.0, PLAYER_EYE_HEIGHT, 0.0);
                    println!("=== FIRST-PERSON MODE ===");
                    println!("WASD: Move, Mouse: Look, Space: Jump, Shift: Sprint");
                } else {
                    println!("=== FREE CAMERA MODE ===");
                    println!("WASD: Move, QE: Up/Down, Mouse: Look, Space: Fire");
                }
            }
            
            // Terrain presets (F1-F4) work anytime
            KeyCode::F1 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 0.1, mountains: 0.0, rocks: 0.0, hills: 0.2, detail: 0.1, water: 0.0,
                });
                self.terrain_needs_rebuild = true;
                println!("PRESET: FLAT - Perfect for building!");
            }
            KeyCode::F2 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 0.3, mountains: 0.1, rocks: 0.1, hills: 0.4, detail: 0.2, water: 0.0,
                });
                self.terrain_needs_rebuild = true;
                println!("PRESET: GENTLE HILLS");
            }
            KeyCode::F3 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 0.5, mountains: 0.3, rocks: 0.5, hills: 0.3, detail: 0.4, water: 0.2,
                });
                self.terrain_needs_rebuild = true;
                println!("PRESET: ROCKY TERRAIN");
            }
            KeyCode::F4 if pressed => {
                set_terrain_params(TerrainParams {
                    height_scale: 1.0, mountains: 0.8, rocks: 0.6, hills: 0.3, detail: 0.5, water: 0.3,
                });
                self.terrain_needs_rebuild = true;
                println!("PRESET: MOUNTAINS");
            }
            
            // Material selection (1-8 keys) - only when builder mode active AND terrain UI NOT visible
            KeyCode::Digit1 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(0);
            }
            KeyCode::Digit2 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(1);
            }
            KeyCode::Digit3 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(2);
            }
            KeyCode::Digit4 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(3);
            }
            KeyCode::Digit5 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(4);
            }
            KeyCode::Digit6 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(5);
            }
            KeyCode::Digit7 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(6);
            }
            KeyCode::Digit8 if pressed && self.builder_mode.enabled && !self.terrain_ui.visible => {
                self.builder_mode.select_material(7);
            }
            
            // Undo/Redo (Ctrl+Z / Ctrl+Y)
            KeyCode::KeyZ if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled => {
                self.builder_mode.undo(&mut self.hex_wall_grid);
            }
            KeyCode::KeyY if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled => {
                self.builder_mode.redo(&mut self.hex_wall_grid);
            }
            
            // Copy/Paste (Ctrl+C / Ctrl+V)
            KeyCode::KeyC if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled => {
                self.builder_mode.copy_area(&self.hex_wall_grid, 3);
            }
            KeyCode::KeyV if pressed && self.builder_mode.ctrl_held && self.builder_mode.enabled => {
                self.builder_mode.paste(&mut self.hex_wall_grid);
            }
            
            // Rotate paste selection
            KeyCode::KeyR if pressed && self.builder_mode.enabled => {
                self.builder_mode.rotate_selection();
            }
            
            // Camera/cannon controls (when NOT in builder mode or Ctrl not held)
            KeyCode::KeyR if pressed && !self.builder_mode.enabled => self.camera.reset(),
            KeyCode::KeyC if pressed && !self.builder_mode.ctrl_held => {
                self.projectiles.clear();
                println!("Cleared all projectiles");
            }
            
            // Arrow keys: continuous aiming with smooth movement (US-017)
            KeyCode::ArrowUp => {
                self.aiming.aim_up = pressed;
                if pressed && !self.cannon.is_aiming() {
                    println!("Aiming UP (hold for continuous movement)");
                }
            }
            KeyCode::ArrowDown => {
                self.aiming.aim_down = pressed;
                if pressed && !self.cannon.is_aiming() {
                    println!("Aiming DOWN (hold for continuous movement)");
                }
            }
            KeyCode::ArrowLeft => {
                self.aiming.aim_left = pressed;
                if pressed && !self.cannon.is_aiming() {
                    println!("Aiming LEFT (hold for continuous movement)");
                }
            }
            KeyCode::ArrowRight => {
                self.aiming.aim_right = pressed;
                if pressed && !self.cannon.is_aiming() {
                    println!("Aiming RIGHT (hold for continuous movement)");
                }
            }
            _ => {}
        }
    }
}

impl ApplicationHandler for BattleArenaApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = WindowAttributes::default()
                .with_title("Battle Sphere - Combat Arena [T: Terrain Editor]")
                .with_inner_size(PhysicalSize::new(1920, 1080)); // Full HD
            let window = Arc::new(event_loop.create_window(attrs).unwrap());
            self.initialize(window);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if key == KeyCode::Escape && event.state == ElementState::Pressed {
                        event_loop.exit();
                        return;
                    }
                    self.handle_key(key, event.state == ElementState::Pressed);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let mouse_pos = self.current_mouse_pos.unwrap_or((0.0, 0.0));
                
                // Handle start overlay - dismiss on any click
                if self.start_overlay.visible && state == ElementState::Pressed {
                    self.start_overlay.visible = false;
                    // Grab cursor on Windows
                    if let Some(window) = &self.window {
                        if window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                            let _ = window.set_cursor_grab(CursorGrabMode::Confined);
                        }
                        window.set_cursor_visible(false);
                    }
                    println!("=== GAME STARTED ===");
                    return; // Consume this click
                }
                
                match button {
                    MouseButton::Left => {
                        let pressed = state == ElementState::Pressed;
                        self.left_mouse_pressed = pressed;
                        
                        // Check if terrain UI wants the event
                        if pressed {
                            if self.terrain_ui.on_mouse_press(mouse_pos.0, mouse_pos.1) {
                                // UI consumed the event
                                return;
                            }
                        } else {
                            // Mouse release
                            if self.terrain_ui.on_mouse_release(mouse_pos.0, mouse_pos.1) {
                                // Rebuild terrain
                                self.terrain_needs_rebuild = true;
                            }
                        }
                        
                        // New building block system
                        if self.build_toolbar.visible && pressed {
                            // Bridge mode: select faces
                            if self.build_toolbar.is_bridge_mode() {
                                self.handle_bridge_click();
                            }
                            // Normal mode: First check for double-click (merge), then place
                            else if !self.handle_block_click() {
                                self.place_building_block();
                            }
                        }
                        // Legacy hex prism builder mode
                        else if self.builder_mode.enabled && pressed {
                            if self.builder_mode.place_at_cursor(&mut self.hex_wall_grid) {
                                // Mesh needs regeneration
                            }
                        }
                    }
                    MouseButton::Right => {
                        // Right-click removes blocks in builder mode
                        if self.builder_mode.enabled && state == ElementState::Pressed {
                            if self.builder_mode.remove_at_cursor(&mut self.hex_wall_grid) {
                                // Mesh needs regeneration
                            }
                        }
                        // Track right mouse state (for legacy compatibility)
                        self.mouse_pressed = state == ElementState::Pressed;
                    }
                    MouseButton::Middle => {
                        if state == ElementState::Pressed {
                            if self.build_toolbar.visible {
                                // Middle-click in build mode: cycle material
                                self.build_toolbar.next_material();
                            }
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let current = (position.x as f32, position.y as f32);
                
                // Always track current mouse position for builder raycast
                self.current_mouse_pos = Some(current);
                
                // Update terrain UI slider dragging
                if self.left_mouse_pressed && self.terrain_ui.visible {
                    self.terrain_ui.on_mouse_move(current.0, current.1);
                }
                
                // Note: Camera look is now handled by DeviceEvent::MouseMotion 
                // for proper cross-platform support (especially Windows)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scroll = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 100.0,
                };
                
                // Build toolbar: scroll adjusts height
                if self.build_toolbar.visible {
                    self.build_toolbar.adjust_height(scroll);
                    // Update preview immediately
                    self.update_block_preview();
                }
                // Builder mode (hex prisms): scroll adjusts build height level
                else if self.builder_mode.enabled {
                    let delta = if scroll > 0.0 { 1 } else { -1 };
                    self.builder_mode.adjust_level(delta);
                } else {
                    // Normal mode: scroll zooms camera
                    self.camera.position += self.camera.get_forward() * scroll * 5.0;
                }
            }
            WindowEvent::Resized(new_size) => {
                if let (Some(surface), Some(config), Some(device)) =
                    (&self.surface, &mut self.surface_config, &self.device)
                {
                    config.width = new_size.width.max(1);
                    config.height = new_size.height.max(1);
                    surface.configure(device, config);

                    // Recreate depth texture for new size (US-013)
                    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("Depth Texture"),
                        size: wgpu::Extent3d {
                            width: config.width,
                            height: config.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Depth32Float,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    });
                    self.depth_texture = Some(depth_texture.create_view(&wgpu::TextureViewDescriptor::default()));
                    self.depth_texture_raw = Some(depth_texture);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let delta_time = now.duration_since(self.last_frame).as_secs_f32();
                self.last_frame = now;

                // Update FPS counter
                self.frame_count += 1;
                if now.duration_since(self.last_fps_update).as_secs_f32() >= 1.0 {
                    self.fps = self.frame_count as f32
                        / now.duration_since(self.last_fps_update).as_secs_f32();
                    self.frame_count = 0;
                    self.last_fps_update = now;

                    if let Some(window) = &self.window {
                        let mode_str = if self.builder_mode.enabled {
                            format!("BUILDER (Mat: {})", self.builder_mode.selected_material + 1)
                        } else {
                            "Combat".to_string()
                        };
                        window.set_title(&format!(
                            "Battle Sphere - {} | FPS: {:.0} | Prisms: {}",
                            mode_str,
                            self.fps,
                            self.hex_wall_grid.len()
                        ));
                    }
                }

                self.update(delta_time);
                self.render();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    // Handle raw device events - crucial for Windows mouse look
    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        // Skip if start overlay is visible (not yet focused)
        if self.start_overlay.visible {
            return;
        }
        
        if let DeviceEvent::MouseMotion { delta } = event {
            // In build mode: mouse moves cursor for selection (camera doesn't rotate)
            // Camera can still rotate with right-click held
            if self.build_toolbar.visible {
                // Only rotate camera if right mouse is held
                if self.mouse_pressed {
                    self.camera.handle_mouse_look(delta.0 as f32, delta.1 as f32);
                }
            } else {
                // Normal mode: free camera look
                self.camera.handle_mouse_look(delta.0 as f32, delta.1 as f32);
            }
        }
    }
}

// Buffer init helper
use wgpu::util::DeviceExt;

fn main() {
    println!("===========================================");
    println!("   Battle Sphere - Combat Arena");
    println!("===========================================");
    println!();
    println!("*** Click anywhere to start ***");
    println!();
    println!("General Controls:");
    println!("  Mouse       - Look around (free look)");
    println!("  WASD        - Move");
    println!("  SPACE       - Jump (FPS) / Fire (Free)");
    println!("  V           - Toggle First-Person / Free Camera");
    println!("  F11         - Toggle Fullscreen");
    println!("  ESC         - Exit");
    println!();
    println!("Build Toolbar (press B to toggle):");
    println!("  Tab/Up/Down - Cycle through shapes");
    println!("  1-7         - Select shape (7=Bridge tool)");
    println!("  Scroll      - Adjust height level");
    println!("  Middle-click- Change material");
    println!("  Left-click  - Place block / Select face (Bridge)");
    println!("  Double-click- Merge connected blocks into one");
    println!();
    println!("  Bridge Tool: Click 2 faces to connect them!");
    println!("  Physics: Unsupported blocks fall every 5 sec");
    println!();
    println!("Combat Controls:");
    println!("  Arrow Keys  - Aim cannon");
    println!("  C           - Clear projectiles");
    println!("  R           - Reset camera");
    println!();
    println!("Legacy Builder (hex prisms):");
    println!("  Left-click  - Place prism");
    println!("  Right-click - Remove prism");
    println!("  Scroll      - Adjust build height");
    println!("  Ctrl+Z/Y    - Undo/Redo");
    println!();
    println!("Terrain Editor (press T):");
    println!("  F1 = FLAT | F2 = HILLS | F3 = ROCKY | F4 = MOUNTAINS");
    println!();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = BattleArenaApp::new();
    event_loop.run_app(&mut app).unwrap();
}
