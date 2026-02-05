//! Arena Configuration
//!
//! Centralized configuration for the battle arena layout.
//! Replaces hardcoded constants scattered across battle_arena.rs.

use glam::Vec3;

/// Configuration for a single floating island in the arena.
#[derive(Clone, Debug)]
pub struct IslandConfig {
    /// World-space center position of the island
    pub position: Vec3,
    /// Hexagonal radius of the island (meters)
    pub radius: f32,
    /// Height of the top surface above the island center
    pub surface_height: f32,
    /// Total depth below surface (how thick the island is)
    pub thickness: f32,
    /// How much the bottom tapers inward (0.0 = cylinder, 1.0 = cone)
    pub taper_amount: f32,
}

/// Configuration for the bridge connecting the two islands.
#[derive(Clone, Debug)]
pub struct BridgeConfig {
    /// Width of the bridge walkway (meters)
    pub width: f32,
    /// Height of chain rails above planks (meters)
    pub rail_height: f32,
}

/// Central configuration for the entire battle arena layout.
///
/// Captures island positions, lava ocean dimensions, meteor spawning,
/// and gameplay timing parameters. `Default` returns values matching
/// the current hardcoded constants in `battle_arena.rs`.
#[derive(Clone, Debug)]
pub struct ArenaConfig {
    /// Attacker island (positive Z side)
    pub island_attacker: IslandConfig,
    /// Defender island (negative Z side)
    pub island_defender: IslandConfig,
    /// Bridge connecting the two islands
    pub bridge: BridgeConfig,
    /// Size of the lava ocean plane (meters, square extent)
    pub lava_size: f32,
    /// Y-coordinate of the lava ocean surface
    pub lava_y: f32,
    /// Seconds between meteor spawns
    pub meteor_spawn_interval: f32,
    /// Radius around origin for meteor impacts (meters)
    pub meteor_spawn_radius: f32,
    /// Seconds between physics support checks
    pub physics_check_interval: f32,
    /// Length of a full day/night cycle (seconds)
    pub day_length_seconds: f32,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            island_attacker: IslandConfig {
                position: Vec3::new(0.0, 10.0, 45.0),
                radius: 30.0,
                surface_height: 5.0,
                thickness: 25.0,
                taper_amount: 0.6,
            },
            island_defender: IslandConfig {
                position: Vec3::new(0.0, 10.0, -45.0),
                radius: 30.0,
                surface_height: 5.0,
                thickness: 25.0,
                taper_amount: 0.6,
            },
            bridge: BridgeConfig {
                width: 2.5,
                rail_height: 1.0,
            },
            lava_size: 200.0,
            lava_y: -15.0,
            meteor_spawn_interval: 5.0,
            meteor_spawn_radius: 60.0,
            physics_check_interval: 5.0,
            day_length_seconds: 600.0,
        }
    }
}
