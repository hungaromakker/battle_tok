//! Game systems — self-contained modules that own state and logic.

pub mod projectile_system;

pub use projectile_system::{ProjectileSystem, ProjectileUpdate};
