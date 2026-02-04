//! World Module
//!
//! Contains world-space configuration, sky/weather systems, and utilities.
//!
//! ## Default World
//! The default world is a 10km x 10km spherical planet with visible curvature.
//! Walking to one edge wraps you to the opposite edge, like a real planet.

pub mod grid;
pub mod sky;

pub use grid::{GridConfig, WorldType, clamp_to_map, snap_to_grid};
pub use sky::{MoonPhase, Season, SkySettings, WeatherType};
