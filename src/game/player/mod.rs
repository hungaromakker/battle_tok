//! Player Module
//!
//! Contains player-related systems including input handling and movement.

pub mod input;
pub mod movement;

pub use input::{CameraDelta, KeyCode, MovementDirection, PlayerInput};
pub use movement::{MovementConfig, PlayerMovement};
