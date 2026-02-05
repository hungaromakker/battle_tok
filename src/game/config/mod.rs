//! Config Module
//!
//! Centralized configuration for arena layout and gameplay parameters.

pub mod arena_config;
pub mod input_config;
pub mod visual_config;

pub use arena_config::{ArenaConfig, BridgeConfig as ArenaBridgeConfig, IslandConfig};
pub use input_config::{
    AimingBindings, BuildingBindings, CameraBindings, CombatBindings, EditingBindings,
    InputCategory, InputConfig, MovementBindings, UIBindings,
};
pub use visual_config::VisualConfig;
