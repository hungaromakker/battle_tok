//! Entity Re-bake Tracker
//!
//! US-024: Tracks dirty flags for entity shape-affecting parameters and queues
//! re-bakes when parameters change.
//!
//! # Shape-Affecting Parameters (Dirty Params)
//!
//! These parameters affect the baked SDF shape and require re-baking when changed:
//! - `scale` (any component: scale_x, scale_y, scale_z)
//! - `noise_amplitude`
//! - `noise_frequency`
//! - `noise_octaves`
//!
//! # Non-Dirty Parameters
//!
//! These parameters do NOT affect the SDF shape and don't require re-baking:
//! - `position` (translation is applied at runtime)
//! - `rotation` (rotation is applied at runtime)
//! - `color`, `roughness`, `metallic` (material properties)
//! - `selected`, `lod_octaves` (rendering hints)
//!
//! # Example
//!
//! ```ignore
//! use battle_tok_engine::render::rebake_tracker::{RebakeTracker, ShapeParams};
//!
//! let mut tracker = RebakeTracker::new();
//!
//! // Register an entity when spawned
//! let entity_id = 1;
//! let params = ShapeParams {
//!     scale: [1.0, 1.0, 1.0],
//!     noise_amplitude: 0.0,
//!     noise_frequency: 1.0,
//!     noise_octaves: 4,
//!     sdf_type: 0,
//! };
//! tracker.register_entity(entity_id, params, Some(baked_sdf_id));
//!
//! // Update params and check if dirty
//! let new_params = ShapeParams { scale: [2.0, 2.0, 2.0], ..params };
//! tracker.update_params(entity_id, new_params);
//!
//! // Check for dirty entities and queue re-bakes
//! let dirty_entities = tracker.take_dirty_entities();
//! for (id, params, old_sdf_id) in dirty_entities {
//!     // Queue re-bake job
//!     // Store old_sdf_id to free after new bake completes
//! }
//! ```

use std::collections::HashMap;

use super::bake_queue::EntityId;

/// Tolerance for floating-point comparison of shape parameters.
/// Changes smaller than this are considered insignificant and won't trigger re-bake.
const PARAM_EPSILON: f32 = 0.0001;

/// Shape-affecting parameters that require re-baking when changed.
///
/// These parameters directly affect the SDF shape stored in the baked 3D texture.
/// Changing any of these requires generating a new baked SDF.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeParams {
    /// Scale in each axis (affects SDF shape)
    pub scale: [f32; 3],
    /// Noise displacement amplitude (affects SDF shape)
    pub noise_amplitude: f32,
    /// Noise displacement frequency (affects SDF shape)
    pub noise_frequency: f32,
    /// Number of noise octaves (affects SDF shape)
    pub noise_octaves: u32,
    /// SDF primitive type (affects SDF shape)
    pub sdf_type: u32,
}

impl ShapeParams {
    /// Create shape parameters for a simple entity without noise.
    pub fn new(scale: [f32; 3], sdf_type: u32) -> Self {
        Self {
            scale,
            noise_amplitude: 0.0,
            noise_frequency: 1.0,
            noise_octaves: 4,
            sdf_type,
        }
    }

    /// Create shape parameters with noise displacement.
    pub fn with_noise(
        scale: [f32; 3],
        sdf_type: u32,
        noise_amplitude: f32,
        noise_frequency: f32,
        noise_octaves: u32,
    ) -> Self {
        Self {
            scale,
            noise_amplitude,
            noise_frequency,
            noise_octaves,
            sdf_type,
        }
    }

    /// Check if two ShapeParams are approximately equal.
    /// Returns true if all shape-affecting parameters are within PARAM_EPSILON.
    pub fn approx_eq(&self, other: &Self) -> bool {
        // Check SDF type (exact match)
        if self.sdf_type != other.sdf_type {
            return false;
        }

        // Check noise octaves (exact match for integer)
        if self.noise_octaves != other.noise_octaves {
            return false;
        }

        // Check scale components
        for i in 0..3 {
            if (self.scale[i] - other.scale[i]).abs() > PARAM_EPSILON {
                return false;
            }
        }

        // Check noise parameters
        if (self.noise_amplitude - other.noise_amplitude).abs() > PARAM_EPSILON {
            return false;
        }

        if (self.noise_frequency - other.noise_frequency).abs() > PARAM_EPSILON {
            return false;
        }

        true
    }
}

impl Default for ShapeParams {
    fn default() -> Self {
        Self {
            scale: [1.0, 1.0, 1.0],
            noise_amplitude: 0.0,
            noise_frequency: 1.0,
            noise_octaves: 4,
            sdf_type: 0, // Sphere
        }
    }
}

/// Tracked entity state including parameters and baked SDF reference.
#[derive(Clone, Debug)]
struct TrackedEntity {
    /// Last known shape parameters
    params: ShapeParams,
    /// Current baked SDF slot ID (None if not yet baked)
    baked_sdf_id: Option<u32>,
    /// Whether the entity is marked as dirty (needs re-bake)
    dirty: bool,
}

/// Result of taking dirty entities from the tracker.
///
/// Contains all information needed to queue a re-bake job and free the old SDF.
#[derive(Clone, Debug)]
pub struct DirtyEntity {
    /// Entity ID
    pub entity_id: EntityId,
    /// New shape parameters to bake
    pub params: ShapeParams,
    /// Old baked SDF slot ID to free after new bake completes
    pub old_sdf_id: Option<u32>,
}

/// Tracks entity shape parameters and detects changes that require re-baking.
///
/// The tracker maintains a record of each entity's shape-affecting parameters
/// and flags entities as "dirty" when those parameters change. The caller can
/// then take the dirty entities and queue re-bake jobs.
///
/// # Thread Safety
///
/// This struct is NOT thread-safe. Use a mutex or other synchronization if
/// accessed from multiple threads.
pub struct RebakeTracker {
    /// Map of entity ID to tracked state
    entities: HashMap<EntityId, TrackedEntity>,
}

impl RebakeTracker {
    /// Create a new empty rebake tracker.
    pub fn new() -> Self {
        Self {
            entities: HashMap::new(),
        }
    }

    /// Register a new entity with initial shape parameters.
    ///
    /// Call this when an entity spawns and is queued for initial baking.
    /// The entity starts as not-dirty since it's being baked for the first time.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - Unique identifier for the entity
    /// * `params` - Initial shape parameters
    /// * `baked_sdf_id` - Initial baked SDF slot ID (None if not yet baked)
    pub fn register_entity(
        &mut self,
        entity_id: EntityId,
        params: ShapeParams,
        baked_sdf_id: Option<u32>,
    ) {
        self.entities.insert(
            entity_id,
            TrackedEntity {
                params,
                baked_sdf_id,
                dirty: false,
            },
        );
    }

    /// Update the baked SDF ID for an entity after baking completes.
    ///
    /// Call this when a bake job finishes to store the new SDF slot ID.
    /// This also clears the dirty flag.
    pub fn set_baked_sdf_id(&mut self, entity_id: EntityId, baked_sdf_id: u32) {
        if let Some(tracked) = self.entities.get_mut(&entity_id) {
            tracked.baked_sdf_id = Some(baked_sdf_id);
            tracked.dirty = false;
        }
    }

    /// Update shape parameters for an entity.
    ///
    /// If the parameters differ from the stored values, the entity is marked as dirty.
    /// Position and rotation changes do NOT call this method - they are handled
    /// separately at runtime without re-baking.
    ///
    /// # Arguments
    ///
    /// * `entity_id` - Entity to update
    /// * `new_params` - New shape parameters
    ///
    /// # Returns
    ///
    /// `true` if the entity was marked dirty, `false` if params unchanged or entity not found.
    pub fn update_params(&mut self, entity_id: EntityId, new_params: ShapeParams) -> bool {
        if let Some(tracked) = self.entities.get_mut(&entity_id) {
            // Check if parameters actually changed
            if !tracked.params.approx_eq(&new_params) {
                tracked.params = new_params;
                tracked.dirty = true;
                return true;
            }
        }
        false
    }

    /// Update individual scale values for an entity.
    ///
    /// Convenience method for changing scale without constructing full ShapeParams.
    /// Marks entity as dirty if scale changed.
    pub fn update_scale(&mut self, entity_id: EntityId, scale: [f32; 3]) -> bool {
        if let Some(tracked) = self.entities.get_mut(&entity_id) {
            let mut new_params = tracked.params;
            new_params.scale = scale;
            return self.update_params(entity_id, new_params);
        }
        false
    }

    /// Update noise parameters for an entity.
    ///
    /// Convenience method for changing noise params without constructing full ShapeParams.
    /// Marks entity as dirty if noise params changed.
    pub fn update_noise(
        &mut self,
        entity_id: EntityId,
        amplitude: f32,
        frequency: f32,
        octaves: u32,
    ) -> bool {
        if let Some(tracked) = self.entities.get_mut(&entity_id) {
            let mut new_params = tracked.params;
            new_params.noise_amplitude = amplitude;
            new_params.noise_frequency = frequency;
            new_params.noise_octaves = octaves;
            return self.update_params(entity_id, new_params);
        }
        false
    }

    /// Check if an entity is marked as dirty (needs re-bake).
    pub fn is_dirty(&self, entity_id: EntityId) -> bool {
        self.entities
            .get(&entity_id)
            .map(|t| t.dirty)
            .unwrap_or(false)
    }

    /// Get the number of dirty entities waiting for re-bake.
    pub fn dirty_count(&self) -> usize {
        self.entities.values().filter(|t| t.dirty).count()
    }

    /// Take all dirty entities, clearing their dirty flags.
    ///
    /// Returns a list of DirtyEntity structs containing:
    /// - Entity ID
    /// - New shape parameters to bake
    /// - Old SDF slot ID to free after new bake completes
    ///
    /// The caller should:
    /// 1. Queue a re-bake job for each entity
    /// 2. Store the old_sdf_id to free after the new bake completes
    pub fn take_dirty_entities(&mut self) -> Vec<DirtyEntity> {
        let mut dirty = Vec::new();

        for (&entity_id, tracked) in &mut self.entities {
            if tracked.dirty {
                dirty.push(DirtyEntity {
                    entity_id,
                    params: tracked.params,
                    old_sdf_id: tracked.baked_sdf_id,
                });
                // Note: We don't clear dirty flag here - it's cleared when the new
                // bake completes (in set_baked_sdf_id). This allows re-trying if
                // bake fails.
            }
        }

        dirty
    }

    /// Remove an entity from tracking.
    ///
    /// Call this when an entity is despawned.
    /// Returns the old baked SDF ID if one was stored (caller should free it).
    pub fn remove_entity(&mut self, entity_id: EntityId) -> Option<u32> {
        self.entities.remove(&entity_id).and_then(|t| t.baked_sdf_id)
    }

    /// Get the current shape parameters for an entity.
    pub fn get_params(&self, entity_id: EntityId) -> Option<ShapeParams> {
        self.entities.get(&entity_id).map(|t| t.params)
    }

    /// Get the current baked SDF ID for an entity.
    pub fn get_baked_sdf_id(&self, entity_id: EntityId) -> Option<u32> {
        self.entities.get(&entity_id).and_then(|t| t.baked_sdf_id)
    }

    /// Clear all tracked entities.
    ///
    /// Returns a list of (entity_id, baked_sdf_id) pairs for all entities that
    /// had baked SDFs (caller should free these slots).
    pub fn clear(&mut self) -> Vec<(EntityId, u32)> {
        let to_free: Vec<_> = self
            .entities
            .iter()
            .filter_map(|(&id, t)| t.baked_sdf_id.map(|sdf_id| (id, sdf_id)))
            .collect();

        self.entities.clear();
        to_free
    }

    /// Get the total number of tracked entities.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }
}

impl Default for RebakeTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shape_params_approx_eq() {
        let p1 = ShapeParams::new([1.0, 1.0, 1.0], 0);
        let p2 = ShapeParams::new([1.0, 1.0, 1.0], 0);
        assert!(p1.approx_eq(&p2));

        // Very small difference should be equal
        let p3 = ShapeParams::new([1.00001, 1.0, 1.0], 0);
        assert!(p1.approx_eq(&p3));

        // Larger difference should not be equal
        let p4 = ShapeParams::new([1.1, 1.0, 1.0], 0);
        assert!(!p1.approx_eq(&p4));

        // Different SDF type should not be equal
        let p5 = ShapeParams::new([1.0, 1.0, 1.0], 1);
        assert!(!p1.approx_eq(&p5));
    }

    #[test]
    fn test_shape_params_noise() {
        let p1 = ShapeParams::with_noise([1.0, 1.0, 1.0], 0, 0.1, 2.0, 4);
        let p2 = ShapeParams::with_noise([1.0, 1.0, 1.0], 0, 0.1, 2.0, 4);
        assert!(p1.approx_eq(&p2));

        // Different amplitude
        let p3 = ShapeParams::with_noise([1.0, 1.0, 1.0], 0, 0.2, 2.0, 4);
        assert!(!p1.approx_eq(&p3));

        // Different frequency
        let p4 = ShapeParams::with_noise([1.0, 1.0, 1.0], 0, 0.1, 3.0, 4);
        assert!(!p1.approx_eq(&p4));

        // Different octaves
        let p5 = ShapeParams::with_noise([1.0, 1.0, 1.0], 0, 0.1, 2.0, 8);
        assert!(!p1.approx_eq(&p5));
    }

    #[test]
    fn test_tracker_register_entity() {
        let mut tracker = RebakeTracker::new();
        let params = ShapeParams::new([1.0, 1.0, 1.0], 0);

        tracker.register_entity(1, params, None);
        assert_eq!(tracker.entity_count(), 1);
        assert!(!tracker.is_dirty(1));
        assert_eq!(tracker.get_params(1), Some(params));
        assert_eq!(tracker.get_baked_sdf_id(1), None);
    }

    #[test]
    fn test_tracker_set_baked_sdf_id() {
        let mut tracker = RebakeTracker::new();
        let params = ShapeParams::new([1.0, 1.0, 1.0], 0);

        tracker.register_entity(1, params, None);
        tracker.set_baked_sdf_id(1, 42);
        assert_eq!(tracker.get_baked_sdf_id(1), Some(42));
    }

    #[test]
    fn test_tracker_update_params_dirty() {
        let mut tracker = RebakeTracker::new();
        let params = ShapeParams::new([1.0, 1.0, 1.0], 0);

        tracker.register_entity(1, params, Some(10));
        assert!(!tracker.is_dirty(1));

        // Update with different scale - should become dirty
        let new_params = ShapeParams::new([2.0, 2.0, 2.0], 0);
        let became_dirty = tracker.update_params(1, new_params);
        assert!(became_dirty);
        assert!(tracker.is_dirty(1));
        assert_eq!(tracker.dirty_count(), 1);

        // Update with same params - should not change dirty state
        let same_became_dirty = tracker.update_params(1, new_params);
        assert!(!same_became_dirty);
        assert!(tracker.is_dirty(1)); // Still dirty
    }

    #[test]
    fn test_tracker_update_scale() {
        let mut tracker = RebakeTracker::new();
        let params = ShapeParams::new([1.0, 1.0, 1.0], 0);

        tracker.register_entity(1, params, Some(10));

        // Update scale
        let became_dirty = tracker.update_scale(1, [2.0, 2.0, 2.0]);
        assert!(became_dirty);
        assert!(tracker.is_dirty(1));
    }

    #[test]
    fn test_tracker_update_noise() {
        let mut tracker = RebakeTracker::new();
        let params = ShapeParams::new([1.0, 1.0, 1.0], 0);

        tracker.register_entity(1, params, Some(10));

        // Update noise params
        let became_dirty = tracker.update_noise(1, 0.5, 3.0, 8);
        assert!(became_dirty);
        assert!(tracker.is_dirty(1));

        let stored = tracker.get_params(1).unwrap();
        assert_eq!(stored.noise_amplitude, 0.5);
        assert_eq!(stored.noise_frequency, 3.0);
        assert_eq!(stored.noise_octaves, 8);
    }

    #[test]
    fn test_tracker_take_dirty_entities() {
        let mut tracker = RebakeTracker::new();

        // Register and make dirty
        tracker.register_entity(1, ShapeParams::new([1.0, 1.0, 1.0], 0), Some(10));
        tracker.register_entity(2, ShapeParams::new([1.0, 1.0, 1.0], 1), Some(20));
        tracker.register_entity(3, ShapeParams::new([1.0, 1.0, 1.0], 2), Some(30));

        // Make entities 1 and 3 dirty
        tracker.update_scale(1, [2.0, 2.0, 2.0]);
        tracker.update_noise(3, 0.1, 2.0, 4);

        assert_eq!(tracker.dirty_count(), 2);

        // Take dirty entities
        let dirty = tracker.take_dirty_entities();
        assert_eq!(dirty.len(), 2);

        // Check entity 1
        let e1 = dirty.iter().find(|e| e.entity_id == 1).unwrap();
        assert_eq!(e1.old_sdf_id, Some(10));
        assert_eq!(e1.params.scale, [2.0, 2.0, 2.0]);

        // Check entity 3
        let e3 = dirty.iter().find(|e| e.entity_id == 3).unwrap();
        assert_eq!(e3.old_sdf_id, Some(30));
        assert_eq!(e3.params.noise_amplitude, 0.1);

        // Entities still dirty until set_baked_sdf_id is called
        assert!(tracker.is_dirty(1));
        assert!(tracker.is_dirty(3));
    }

    #[test]
    fn test_tracker_clear_dirty_after_rebake() {
        let mut tracker = RebakeTracker::new();

        tracker.register_entity(1, ShapeParams::new([1.0, 1.0, 1.0], 0), Some(10));
        tracker.update_scale(1, [2.0, 2.0, 2.0]);
        assert!(tracker.is_dirty(1));

        // Simulate rebake completion - set new SDF ID
        tracker.set_baked_sdf_id(1, 11);
        assert!(!tracker.is_dirty(1));
        assert_eq!(tracker.get_baked_sdf_id(1), Some(11));
    }

    #[test]
    fn test_tracker_remove_entity() {
        let mut tracker = RebakeTracker::new();

        tracker.register_entity(1, ShapeParams::new([1.0, 1.0, 1.0], 0), Some(10));
        assert_eq!(tracker.entity_count(), 1);

        let old_sdf = tracker.remove_entity(1);
        assert_eq!(old_sdf, Some(10));
        assert_eq!(tracker.entity_count(), 0);
    }

    #[test]
    fn test_tracker_clear() {
        let mut tracker = RebakeTracker::new();

        tracker.register_entity(1, ShapeParams::new([1.0, 1.0, 1.0], 0), Some(10));
        tracker.register_entity(2, ShapeParams::new([1.0, 1.0, 1.0], 1), Some(20));
        tracker.register_entity(3, ShapeParams::new([1.0, 1.0, 1.0], 2), None); // No baked SDF

        let to_free = tracker.clear();
        assert_eq!(to_free.len(), 2);
        assert!(to_free.contains(&(1, 10)));
        assert!(to_free.contains(&(2, 20)));
        assert_eq!(tracker.entity_count(), 0);
    }

    #[test]
    fn test_position_rotation_do_not_affect_dirty() {
        // This test documents that position and rotation are NOT tracked
        // They are applied at runtime and don't require re-baking
        let mut tracker = RebakeTracker::new();
        let params = ShapeParams::new([1.0, 1.0, 1.0], 0);

        tracker.register_entity(1, params, Some(10));

        // There's no update_position or update_rotation method intentionally
        // Position/rotation changes should NOT go through the tracker
        assert!(!tracker.is_dirty(1));

        // Only shape-affecting params trigger dirty
        tracker.update_scale(1, [2.0, 2.0, 2.0]);
        assert!(tracker.is_dirty(1));
    }
}
