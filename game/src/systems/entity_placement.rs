//! Entity Placement System
//!
//! This module provides functions for placing and managing entities in the game world.
//! It handles grid snapping, map bounds clamping, and GPU buffer updates.

use glam::Vec3;

// Re-export entity types from engine for convenience
// Note: In a full build, this would use: use engine::render::entities::*;
// For now, we define the necessary types inline until the crate structure is finalized.

/// Configuration for grid and map placement
#[derive(Clone, Copy, Debug)]
pub struct PlacementConfig {
    /// Small grid cell size (Unity-style: 1.0)
    pub small_grid_size: f32,
    /// Large grid cell size (Unity-style: 10.0)
    pub large_grid_size: f32,
    /// Map bounds (-map_size to +map_size)
    pub map_size: f32,
    /// Grid snapping on/off
    pub snap_enabled: bool,
    /// Height for air placement mode
    pub placement_height: f32,
}

impl Default for PlacementConfig {
    fn default() -> Self {
        Self {
            small_grid_size: 1.0,
            large_grid_size: 10.0,
            map_size: 50.0,
            snap_enabled: true,
            placement_height: 0.0, // Default: place on ground
        }
    }
}

/// Entity placement state manager
pub struct EntityPlacementSystem {
    /// Current placement configuration
    pub config: PlacementConfig,
    /// Current entity type to place (0=Sphere, 1=Box, 2=Capsule, 3=Torus, 4=Cylinder)
    pub current_entity_type: u32,
    /// Current scale multiplier for new entities
    pub current_scale: f32,
    /// Index for cycling through predefined colors
    pub color_index: usize,
    /// Number of placed entities
    pub entity_count: usize,
}

impl Default for EntityPlacementSystem {
    fn default() -> Self {
        Self {
            config: PlacementConfig::default(),
            current_entity_type: 0,
            current_scale: 0.5,
            color_index: 0,
            entity_count: 0,
        }
    }
}

impl EntityPlacementSystem {
    /// Create a new entity placement system with default configuration
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new entity placement system with custom configuration
    pub fn with_config(config: PlacementConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Snap a position to the grid if snapping is enabled
    pub fn snap_to_grid(&self, pos: Vec3) -> Vec3 {
        if !self.config.snap_enabled {
            return pos;
        }
        let grid_size = self.config.small_grid_size;
        Vec3::new(
            (pos.x / grid_size).round() * grid_size,
            pos.y, // Don't snap Y
            (pos.z / grid_size).round() * grid_size,
        )
    }

    /// Clamp a position to stay within map bounds
    pub fn clamp_to_map(&self, pos: Vec3) -> Vec3 {
        let bounds = self.config.map_size;
        Vec3::new(
            pos.x.clamp(-bounds, bounds),
            pos.y,
            pos.z.clamp(-bounds, bounds),
        )
    }

    /// Get the name of the current entity type
    pub fn get_entity_type_name(&self) -> &'static str {
        match self.current_entity_type {
            0 => "Sphere",
            1 => "Box",
            2 => "Capsule",
            3 => "Torus",
            4 => "Cylinder",
            _ => "Unknown",
        }
    }

    /// Check if we can place more entities (max 64)
    pub fn can_place_entity(&self) -> bool {
        self.entity_count < 64
    }
}

/// Result of placing an entity
#[derive(Debug, Clone)]
pub struct PlacementResult {
    /// Final world position after snapping and clamping
    pub position: Vec3,
    /// Entity type that was placed
    pub entity_type: u32,
    /// Scale of the placed entity
    pub scale: f32,
    /// Packed color value
    pub color_packed: u32,
    /// Whether grid snapping was applied
    pub was_snapped: bool,
}

/// Place an entity at the given world position
///
/// This function handles:
/// - Grid snapping (if enabled)
/// - Map bounds clamping
/// - Placement height adjustment
/// - Y position adjustment so entity sits ON the grid level
///
/// # Arguments
/// * `system` - The entity placement system with configuration
/// * `position` - The raw world position where the user clicked
/// * `color_packed` - The packed RGB color for the entity
///
/// # Returns
/// A `PlacementResult` containing the final position and entity data,
/// or `None` if the maximum number of entities has been reached.
pub fn place_entity_at(
    system: &mut EntityPlacementSystem,
    position: Vec3,
    color_packed: u32,
) -> Option<PlacementResult> {
    if !system.can_place_entity() {
        return None;
    }

    // Apply grid snapping
    let mut snapped_pos = system.snap_to_grid(position);

    // Apply placement height (for building in the air)
    snapped_pos.y = system.config.placement_height;

    // Clamp to map bounds
    let clamped_pos = system.clamp_to_map(snapped_pos);

    // Calculate final Y position so entity sits ON the grid level
    let final_y = clamped_pos.y + system.current_scale * 0.5;
    let final_pos = Vec3::new(clamped_pos.x, final_y, clamped_pos.z);

    // Update state
    system.entity_count += 1;
    system.color_index += 1;

    Some(PlacementResult {
        position: final_pos,
        entity_type: system.current_entity_type,
        scale: system.current_scale,
        color_packed,
        was_snapped: system.config.snap_enabled,
    })
}

/// Clear all placed entities and reset the placement system state
///
/// This function resets:
/// - Entity count to 0
/// - Color index to 0
///
/// Note: The caller is responsible for updating the GPU buffer after calling this.
pub fn clear_entities(system: &mut EntityPlacementSystem) {
    system.entity_count = 0;
    system.color_index = 0;
}

/// Remove the last placed entity
///
/// Returns `true` if an entity was removed, `false` if there were no entities.
pub fn remove_last_entity(system: &mut EntityPlacementSystem) -> bool {
    if system.entity_count > 0 {
        system.entity_count -= 1;
        true
    } else {
        false
    }
}

/// Adjust the current entity scale within bounds
pub fn adjust_scale(system: &mut EntityPlacementSystem, delta: f32, min: f32, max: f32) {
    system.current_scale = (system.current_scale + delta).clamp(min, max);
}

/// Cycle to the next entity type (wraps around)
pub fn next_entity_type(system: &mut EntityPlacementSystem) {
    system.current_entity_type = (system.current_entity_type + 1) % 5;
}

/// Set a specific entity type
pub fn set_entity_type(system: &mut EntityPlacementSystem, entity_type: u32) {
    system.current_entity_type = entity_type.min(4);
}

/// Toggle grid snapping
pub fn toggle_snap(system: &mut EntityPlacementSystem) -> bool {
    system.config.snap_enabled = !system.config.snap_enabled;
    system.config.snap_enabled
}

/// Adjust placement height
pub fn adjust_placement_height(system: &mut EntityPlacementSystem, delta: f32, min: f32, max: f32) {
    system.config.placement_height = (system.config.placement_height + delta).clamp(min, max);
}

/// Reset placement height to ground level
pub fn reset_placement_height(system: &mut EntityPlacementSystem) {
    system.config.placement_height = 0.0;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snap_to_grid() {
        let system = EntityPlacementSystem::new();
        let pos = Vec3::new(1.3, 0.0, 2.7);
        let snapped = system.snap_to_grid(pos);
        assert_eq!(snapped.x, 1.0);
        assert_eq!(snapped.z, 3.0);
    }

    #[test]
    fn test_snap_disabled() {
        let mut system = EntityPlacementSystem::new();
        system.config.snap_enabled = false;
        let pos = Vec3::new(1.3, 0.0, 2.7);
        let result = system.snap_to_grid(pos);
        assert_eq!(result, pos);
    }

    #[test]
    fn test_clamp_to_map() {
        let system = EntityPlacementSystem::new();
        let pos = Vec3::new(100.0, 0.0, -100.0);
        let clamped = system.clamp_to_map(pos);
        assert_eq!(clamped.x, 50.0);
        assert_eq!(clamped.z, -50.0);
    }

    #[test]
    fn test_place_entity() {
        let mut system = EntityPlacementSystem::new();
        let result = place_entity_at(&mut system, Vec3::new(1.3, 0.0, 2.7), 0xFF8800);
        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.position.x, 1.0); // Snapped
        assert_eq!(result.position.z, 3.0); // Snapped
        assert_eq!(system.entity_count, 1);
    }

    #[test]
    fn test_clear_entities() {
        let mut system = EntityPlacementSystem::new();
        place_entity_at(&mut system, Vec3::ZERO, 0xFF8800);
        place_entity_at(&mut system, Vec3::ONE, 0x00FF88);
        assert_eq!(system.entity_count, 2);
        clear_entities(&mut system);
        assert_eq!(system.entity_count, 0);
        assert_eq!(system.color_index, 0);
    }

    #[test]
    fn test_max_entities() {
        let mut system = EntityPlacementSystem::new();
        for _ in 0..64 {
            let result = place_entity_at(&mut system, Vec3::ZERO, 0xFF8800);
            assert!(result.is_some());
        }
        // 65th entity should fail
        let result = place_entity_at(&mut system, Vec3::ZERO, 0xFF8800);
        assert!(result.is_none());
    }

    #[test]
    fn test_entity_type_names() {
        let mut system = EntityPlacementSystem::new();
        system.current_entity_type = 0;
        assert_eq!(system.get_entity_type_name(), "Sphere");
        system.current_entity_type = 1;
        assert_eq!(system.get_entity_type_name(), "Box");
        system.current_entity_type = 2;
        assert_eq!(system.get_entity_type_name(), "Capsule");
        system.current_entity_type = 3;
        assert_eq!(system.get_entity_type_name(), "Torus");
        system.current_entity_type = 4;
        assert_eq!(system.get_entity_type_name(), "Cylinder");
    }
}
