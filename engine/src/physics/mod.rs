//! Physics module for Magic Engine
//!
//! This module provides custom physics implementation for the Battle Sphere game.
//! Built from scratch without external physics library dependencies (no Rapier).
//!
//! # Philosophy
//!
//! Study reference implementations, understand algorithms, build our own.
//! This gives full control over performance and deep understanding of physics math.
//!
//! # Unit System
//!
//! **1 unit = 1 meter** (SI units throughout)
//!
//! - Distances in meters
//! - Velocities in m/s
//! - Accelerations in m/s²
//! - Mass in kg
//! - Air density in kg/m³
//!
//! # Submodules
//!
//! - [`types`] - Core mathematical types (Vec3, Quat) re-exported from glam
//! - [`ballistics`] - Projectile physics and trajectory calculations
//! - [`collision`] - Ray-AABB collision detection for hex-prism structures
//!
//! # Phase 1 Status
//!
//! - Projectile types: Complete (Projectile, BallisticsConfig, ProjectileState)
//! - Collision types: Complete (HitInfo, ray_aabb_intersect)
//! - Ballistics integration: Pending (US-007)
//! - HexPrismGrid integration: Pending
//!
//! See `engine/src/physics/README.md` for full architecture documentation.

pub mod ballistics;
pub mod collision;
pub mod types;

// Re-export commonly used types at the physics module level
pub use ballistics::{BallisticsConfig, Projectile, ProjectileState};
pub use collision::{HexPrism, HexPrismGrid, HitInfo, aabb_surface_normal, ray_aabb_intersect};
pub use types::{Quat, Vec3};
