//! Camera Module
//!
//! Provides camera control and raycasting functionality for the engine.
//! This module is window-system agnostic - it only deals with camera state and math.

pub mod controller;
pub mod fps_controller;
pub mod raycast;

pub use controller::{
    CameraController, CameraMode, CameraTransition,
    SpringConfig, CameraCollisionConfig,
};
pub use fps_controller::FPSCameraController;
pub use raycast::{raycast_to_ground, raycast_to_plane, get_ray_direction, RaycastConfig};
