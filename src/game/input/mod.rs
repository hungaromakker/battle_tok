//! Input Module
//!
//! Input handling, action definitions, and keyboard mapping.

pub mod actions;
pub mod keyboard;

pub use actions::{AimingKey, AimingState, InputAction, InputContext, MovementKey, MovementState};
pub use keyboard::map_key_to_action;
