//! Player Module
//!
//! Provides player character movement and control systems.
//!
//! # Components
//!
//! - [`PlayerMovementController`] - Physics-based movement with camera-relative WASD controls
//!   - Includes jump and gravity system with coyote time support
//!   - Supports spherical world gravity with radial gravity toward planet center
//! - [`CrouchController`] - Stance management with smooth height transitions
//! - [`SphericalGravityConfig`] - Configuration for spherical planet gravity
//! - [`GravityMode`] - Gravity mode selection (flat or spherical)

pub mod crouch;
pub mod movement_controller;

pub use crouch::{
    CROUCH_HEIGHT, CROUCH_SPEED_MULTIPLIER, CrouchController, PRONE_HEIGHT, PRONE_SPEED_MULTIPLIER,
    STANDING_HEIGHT, Stance, TRANSITION_DURATION,
};
pub use movement_controller::{
    ACCELERATION, COYOTE_TIME, DECELERATION, GRAVITY, GravityMode, JUMP_VELOCITY,
    PlayerMovementController, SPRINT_SPEED, SphericalGravityConfig, WALK_SPEED,
};
