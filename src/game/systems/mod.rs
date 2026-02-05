//! Game systems — self-contained modules that own state and logic.

pub mod building_system;
pub mod collision_system;
pub mod destruction_system;
pub mod projectile_system;

pub use building_system::BuildingSystem;
pub use collision_system::CollisionSystem;
pub use destruction_system::DestructionSystem;
pub use projectile_system::{ProjectileSystem, ProjectileUpdate};
