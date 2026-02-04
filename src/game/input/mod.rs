//! Input Module
//!
//! Input handling, action definitions, and keyboard mapping.

pub mod actions;
pub mod keyboard;

pub use actions::{InputAction, InputContext, MovementState, AimingState, MovementKey, AimingKey};
pub use keyboard::map_key_to_action;
