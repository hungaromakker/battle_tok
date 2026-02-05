//! Config Module
//!
//! Centralized configuration for arena layout and gameplay parameters.

pub mod arena_config;
pub mod visual_config;

pub use arena_config::{ArenaConfig, IslandConfig, BridgeConfig as ArenaBridgeConfig};
pub use visual_config::VisualConfig;
