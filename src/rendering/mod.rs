//! Rendering Module
//!
//! This module contains rendering-related types for the Battle Sphere game.
//! These are game-specific rendering components separate from the engine's core rendering.
//!
//! # Modules
//!
//! - [`sdf_objects`] - SDF primitive types for defining siege weapons and other objects
//! - [`hex_prism`] - Hexagonal prism voxel data structures for building walls and fortifications

pub mod hex_prism;
pub mod sdf_objects;

// Re-export commonly used types
pub use hex_prism::{axial_to_world, world_to_axial, HexPrism, HexPrismGrid};
pub use sdf_objects::{PositionedPrimitive, SdfObject, SdfOperation, SdfPrimitive};
