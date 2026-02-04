//! Grid Configuration Module
//!
//! Contains grid and map configuration for world-space operations.
//! Extracted from sdf_core_test.rs for reuse across the engine and game code.
//!
//! ## World Size
//! Default world is 10km x 10km with spherical wrapping.
//! - map_size = 5000.0 means bounds from -5000m to +5000m = 10km
//! - 1 unit = 1 meter (SI units)
//!
//! ## Planet Curvature
//! For a 10km planet circumference: radius = circumference / (2π) ≈ 1591m
//! This creates visible horizon curvature and spherical wrapping.

use glam::Vec3;
use std::f32::consts::PI;

/// World geometry type
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WorldType {
    /// Flat world with hard boundaries (classic)
    Flat,
    /// Spherical world with wrapping (walk to one end, appear at other)
    Spherical,
}

impl Default for WorldType {
    fn default() -> Self {
        WorldType::Spherical
    }
}

/// Grid and map configuration for world-space operations.
///
/// Controls grid snapping behavior and map boundaries.
#[derive(Clone, Copy, Debug)]
pub struct GridConfig {
    /// Small grid cell size (Unity: 1.0)
    pub small_grid_size: f32,
    /// Large grid cell size (Unity: 10.0)
    pub large_grid_size: f32,
    /// Map bounds (-map_size to +map_size). For 10km world, this is 5000.0
    pub map_size: f32,
    /// Grid snapping on/off
    pub snap_enabled: bool,
    /// 3D volume grid (like Minecraft creative mode)
    pub volume_grid_visible: bool,
    /// Height for air placement mode
    pub placement_height: f32,
    /// World geometry type (flat or spherical)
    pub world_type: WorldType,
    /// Planet radius for spherical worlds (calculated from map_size)
    /// For 10km circumference: radius = 10000 / (2π) ≈ 1591m
    pub planet_radius: f32,
}

impl Default for GridConfig {
    fn default() -> Self {
        // 10km x 10km world = map_size of 5000 (-5000 to +5000)
        let map_size = 5000.0;
        // Planet circumference = 2 * map_size * 2 = 4 * map_size (to wrap both X and Z)
        // Actually for a flat-projected spherical world:
        // We treat map_size * 2 as half the circumference
        // circumference = map_size * 4 = 20000m for 10km world
        // radius = circumference / (2π) ≈ 3183m
        let circumference = map_size * 4.0;
        let planet_radius = circumference / (2.0 * PI);

        Self {
            small_grid_size: 1.0,
            large_grid_size: 10.0,
            map_size,
            snap_enabled: true,
            volume_grid_visible: false,
            placement_height: 0.0, // Default: place on ground
            world_type: WorldType::Spherical,
            planet_radius,
        }
    }
}

impl GridConfig {
    /// Create a new GridConfig with custom settings.
    ///
    /// Calculates planet_radius based on map_size assuming spherical world.
    /// For flat worlds, planet_radius is still calculated but not used.
    pub fn new(
        small_grid_size: f32,
        large_grid_size: f32,
        map_size: f32,
    ) -> Self {
        // Calculate planet radius from map size
        // circumference = map_size * 4, radius = circumference / (2π)
        let circumference = map_size * 4.0;
        let planet_radius = circumference / (2.0 * PI);

        Self {
            small_grid_size,
            large_grid_size,
            map_size,
            snap_enabled: true,
            volume_grid_visible: false,
            placement_height: 0.0,
            world_type: WorldType::Spherical,
            planet_radius,
        }
    }

    /// Snap a position to the small grid if snapping is enabled.
    ///
    /// Only snaps X and Z coordinates; Y is preserved.
    pub fn snap_to_grid(&self, pos: Vec3) -> Vec3 {
        if !self.snap_enabled {
            return pos;
        }
        let grid_size = self.small_grid_size;
        Vec3::new(
            (pos.x / grid_size).round() * grid_size,
            pos.y, // Don't snap Y
            (pos.z / grid_size).round() * grid_size,
        )
    }

    /// Clamp a position to the map boundaries.
    ///
    /// Clamps X and Z to [-map_size, +map_size]; Y is preserved.
    pub fn clamp_to_map(&self, pos: Vec3) -> Vec3 {
        let bounds = self.map_size;
        Vec3::new(
            pos.x.clamp(-bounds, bounds),
            pos.y,
            pos.z.clamp(-bounds, bounds),
        )
    }

    /// Snap and clamp a position in one operation.
    ///
    /// First snaps to grid, then applies world boundaries (clamp for flat, wrap for spherical).
    pub fn snap_and_clamp(&self, pos: Vec3) -> Vec3 {
        self.apply_world_bounds(self.snap_to_grid(pos))
    }

    /// Create a flat world config (no wrapping, hard boundaries).
    pub fn flat(map_size: f32) -> Self {
        Self {
            small_grid_size: 1.0,
            large_grid_size: 10.0,
            map_size,
            snap_enabled: true,
            volume_grid_visible: false,
            placement_height: 0.0,
            world_type: WorldType::Flat,
            planet_radius: 0.0,
        }
    }

    /// Create a spherical world config with custom size.
    ///
    /// # Arguments
    /// * `diameter_km` - World diameter in kilometers (e.g., 10.0 for 10km)
    pub fn spherical(diameter_km: f32) -> Self {
        let map_size = diameter_km * 1000.0 / 2.0; // Half the diameter in meters
        let circumference = map_size * 4.0;
        let planet_radius = circumference / (2.0 * PI);

        Self {
            small_grid_size: 1.0,
            large_grid_size: 10.0,
            map_size,
            snap_enabled: true,
            volume_grid_visible: false,
            placement_height: 0.0,
            world_type: WorldType::Spherical,
            planet_radius,
        }
    }

    /// Get the world diameter in meters.
    pub fn world_diameter(&self) -> f32 {
        self.map_size * 2.0
    }

    /// Get the world diameter in kilometers.
    pub fn world_diameter_km(&self) -> f32 {
        self.world_diameter() / 1000.0
    }

    /// Wrap a position around the world (for spherical worlds).
    ///
    /// When you walk past +map_size, you appear at -map_size (like Earth wrapping).
    /// Y is preserved.
    pub fn wrap_position(&self, pos: Vec3) -> Vec3 {
        let bounds = self.map_size;
        let world_size = bounds * 2.0;

        // Wrap X: if pos.x > bounds, subtract world_size; if pos.x < -bounds, add world_size
        let mut x = pos.x;
        while x > bounds {
            x -= world_size;
        }
        while x < -bounds {
            x += world_size;
        }

        // Wrap Z similarly
        let mut z = pos.z;
        while z > bounds {
            z -= world_size;
        }
        while z < -bounds {
            z += world_size;
        }

        Vec3::new(x, pos.y, z)
    }

    /// Apply world boundaries based on world type.
    ///
    /// For flat worlds: clamps to boundaries.
    /// For spherical worlds: wraps around.
    pub fn apply_world_bounds(&self, pos: Vec3) -> Vec3 {
        match self.world_type {
            WorldType::Flat => self.clamp_to_map(pos),
            WorldType::Spherical => self.wrap_position(pos),
        }
    }

    /// Calculate the curvature drop at a given distance from the observer.
    ///
    /// This is how much the ground "drops" due to planet curvature.
    /// Formula: drop = distance² / (2 * radius)
    ///
    /// For a 10km planet (radius ≈ 3183m):
    /// - At 100m: drop ≈ 1.6m
    /// - At 500m: drop ≈ 39m
    /// - At 1km: drop ≈ 157m
    pub fn curvature_drop(&self, distance: f32) -> f32 {
        if self.world_type == WorldType::Flat || self.planet_radius <= 0.0 {
            return 0.0;
        }
        // Standard formula for Earth curvature drop
        (distance * distance) / (2.0 * self.planet_radius)
    }

    /// Check if a position is visible over the horizon.
    ///
    /// Returns true if the position is above the horizon from the observer's viewpoint.
    pub fn is_visible_over_horizon(&self, observer: Vec3, target: Vec3) -> bool {
        if self.world_type == WorldType::Flat {
            return true; // Everything visible in flat world
        }

        let distance = ((target.x - observer.x).powi(2) + (target.z - observer.z).powi(2)).sqrt();
        let drop = self.curvature_drop(distance);

        // Target is visible if its height is above the curvature drop
        // (accounting for observer height)
        target.y > drop - observer.y
    }
}

/// Standalone function to snap a position to a grid.
///
/// Useful when you don't have a GridConfig but need basic snapping.
pub fn snap_to_grid(pos: Vec3, grid_size: f32) -> Vec3 {
    Vec3::new(
        (pos.x / grid_size).round() * grid_size,
        pos.y,
        (pos.z / grid_size).round() * grid_size,
    )
}

/// Standalone function to clamp a position to map boundaries.
///
/// Useful when you don't have a GridConfig but need basic clamping.
pub fn clamp_to_map(pos: Vec3, bounds: f32) -> Vec3 {
    Vec3::new(
        pos.x.clamp(-bounds, bounds),
        pos.y,
        pos.z.clamp(-bounds, bounds),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GridConfig::default();
        assert_eq!(config.small_grid_size, 1.0);
        assert_eq!(config.large_grid_size, 10.0);
        assert_eq!(config.map_size, 5000.0);
        assert!(config.snap_enabled);
        assert!(!config.volume_grid_visible);
        assert_eq!(config.placement_height, 0.0);
        assert_eq!(config.world_type, WorldType::Spherical);
        // planet_radius = 20000 / (2π) ≈ 3183.1
        assert!((config.planet_radius - 3183.1).abs() < 1.0);
        // World diameter should be 10km
        assert_eq!(config.world_diameter_km(), 10.0);
    }

    #[test]
    fn test_snap_to_grid() {
        let config = GridConfig::default();

        // Test basic snapping
        let pos = Vec3::new(1.3, 5.0, 2.7);
        let snapped = config.snap_to_grid(pos);
        assert_eq!(snapped.x, 1.0);
        assert_eq!(snapped.y, 5.0); // Y unchanged
        assert_eq!(snapped.z, 3.0);
    }

    #[test]
    fn test_snap_disabled() {
        let mut config = GridConfig::default();
        config.snap_enabled = false;

        let pos = Vec3::new(1.3, 5.0, 2.7);
        let snapped = config.snap_to_grid(pos);
        assert_eq!(snapped, pos); // No change when disabled
    }

    #[test]
    fn test_clamp_to_map() {
        // Create flat config with map_size = 50.0 for easy testing
        let config = GridConfig::flat(50.0);

        // Test clamping
        let pos = Vec3::new(100.0, 25.0, -75.0);
        let clamped = config.clamp_to_map(pos);
        assert_eq!(clamped.x, 50.0);
        assert_eq!(clamped.y, 25.0); // Y unchanged
        assert_eq!(clamped.z, -50.0);
    }

    #[test]
    fn test_wrap_position() {
        // Create spherical config with map_size = 50.0 for testing
        let config = GridConfig::spherical(0.1); // 100m diameter = map_size 50

        // Test wrapping - going past +50 should wrap to -50 side
        let pos = Vec3::new(60.0, 10.0, -30.0);
        let wrapped = config.wrap_position(pos);
        assert!((wrapped.x - (-40.0)).abs() < 0.1); // 60 - 100 = -40
        assert_eq!(wrapped.y, 10.0); // Y unchanged
        assert_eq!(wrapped.z, -30.0); // Z within bounds, unchanged
    }

    #[test]
    fn test_curvature_drop() {
        let config = GridConfig::default(); // 10km world, radius ≈ 3183m

        // At 100m distance
        let drop_100m = config.curvature_drop(100.0);
        // drop = 100² / (2 * 3183) ≈ 1.57m
        assert!((drop_100m - 1.57).abs() < 0.1);

        // At 500m distance
        let drop_500m = config.curvature_drop(500.0);
        // drop = 500² / (2 * 3183) ≈ 39.3m
        assert!((drop_500m - 39.3).abs() < 1.0);
    }

    #[test]
    fn test_flat_world_no_curvature() {
        let config = GridConfig::flat(50.0);
        assert_eq!(config.curvature_drop(100.0), 0.0);
        assert_eq!(config.curvature_drop(1000.0), 0.0);
    }

    #[test]
    fn test_standalone_functions() {
        let pos = Vec3::new(1.6, 10.0, 3.2);

        let snapped = snap_to_grid(pos, 1.0);
        assert_eq!(snapped.x, 2.0);
        assert_eq!(snapped.z, 3.0);

        let clamped = clamp_to_map(Vec3::new(100.0, 5.0, -100.0), 50.0);
        assert_eq!(clamped.x, 50.0);
        assert_eq!(clamped.z, -50.0);
    }
}
