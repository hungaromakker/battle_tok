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
    CrouchController, Stance,
    STANDING_HEIGHT, CROUCH_HEIGHT, PRONE_HEIGHT,
    TRANSITION_DURATION, CROUCH_SPEED_MULTIPLIER, PRONE_SPEED_MULTIPLIER,
};
pub use movement_controller::{
    PlayerMovementController,
    SphericalGravityConfig, GravityMode,
    WALK_SPEED, SPRINT_SPEED, ACCELERATION, DECELERATION,
    JUMP_VELOCITY, GRAVITY, COYOTE_TIME,
};
