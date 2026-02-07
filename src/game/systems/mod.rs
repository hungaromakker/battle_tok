//! Game systems — self-contained modules that own state and logic.

pub mod building_system;
pub mod building_v2;
pub mod cannon_system;
pub mod collision_system;
pub mod destruction_system;
pub mod meteor_system;
pub mod projectile_system;
pub mod voxel_building;

pub use building_system::BuildingSystem;
pub use building_v2::{BuildingSystemV2, PlaceError as BuildingV2PlaceError};
pub use cannon_system::CannonSystem;
pub use collision_system::CollisionSystem;
pub use destruction_system::DestructionSystem;
pub use meteor_system::{MeteorImpact, MeteorSystem};
pub use projectile_system::{ProjectileKind, ProjectileSystem, ProjectileUpdate};
pub use voxel_building::{
    BuildAudioEvent, BuildAudioEventKind, BuildMode, DamageSource, RaymarchQualityState,
    RenderDeltaBatch, ShellBakeJob, ShellBakeResult, VoxelBuildingRuntime, VoxelCell, VoxelCoord,
    VoxelDamageResult, VoxelHit, VoxelHudState, VoxelMaterialId,
};
