//! Magic Engine Library
//!
//! A modular SDF-based game engine using equation-based graphics.
//! This library provides the core rendering infrastructure and utilities
//! for building applications with procedural graphics.
//!
//! # Modules
//!
//! - [`render`] - Core rendering pipeline with wgpu, VSync control, and shader loading
//! - [`input`] - Platform-agnostic input handling for keyboard and mouse
//! - [`camera`] - Camera control and raycasting
//! - [`world`] - World-space configuration (grid, map bounds)
//!
//! # Example
//!
//! ```ignore
//! use battle_tok_engine::render::{RenderState, RenderConfig, TestUniforms};
//! use battle_tok_engine::input::{KeyboardState, MouseState, KeyCode};
//! use battle_tok_engine::world::GridConfig;
//!
//! // Create render state with VSync off
//! let config = RenderConfig {
//!     width: 1920,
//!     height: 1080,
//!     vsync: false,
//! };
//!
//! // Create grid configuration
//! let grid = GridConfig::default();
//!
//! // Create input state
//! let mut keyboard = KeyboardState::new();
//! let mut mouse = MouseState::new();
//!
//! // Handle keyboard input
//! keyboard.handle_key(KeyCode::W, true);
//! if keyboard.movement.forward {
//!     // Move forward
//! }
//!
//! // Load shader and create render state
//! let shader_source = include_str!("path/to/shader.wgsl");
//! let render_state = RenderState::new(
//!     window,
//!     config,
//!     shader_source,
//!     std::mem::size_of::<TestUniforms>() as u64,
//!     entity_buffer_size,
//!     sky_buffer_size,
//! );
//! ```

pub mod camera;
pub mod input;
pub mod physics;
pub mod player;
pub mod render;
pub mod world;

// Game-specific modules (located in src/game/ directory)
#[path = "../../src/game/mod.rs"]
pub mod game;

// Battle Sphere rendering components (located in src/rendering/ directory)
#[path = "../../src/rendering/mod.rs"]
pub mod rendering;

// Re-export the render module contents at crate level for convenience
pub use render::*;
// Re-export world types for convenience
pub use world::{GridConfig, clamp_to_map, snap_to_grid};
// Re-export commonly used input types
pub use input::{InputState, KeyCode, KeyboardState, MouseButton, MouseState};
// Re-export player types
pub use player::PlayerMovementController;
