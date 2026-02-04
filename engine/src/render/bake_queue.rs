//! Bake Queue System
//!
//! US-023: Implements entity baking on spawn with async queue processing.
//! US-024: Extended with re-bake support for entity transform changes.
//!
//! When entities spawn, they are queued for SDF baking. The queue processes up to
//! MAX_BAKES_PER_FRAME bakes per frame asynchronously. Entities use equation-based
//! rendering until their bake completes, then smoothly transition to baked SDF.
//!
//! # Features
//!
//! - Queue SDF bake jobs when entities spawn
//! - Process up to 5 bake jobs per frame asynchronously
//! - Smooth transition from equation to baked SDF (no visual pop)
//! - Entity stores `baked_sdf_id` after bake completes
//! - (US-024) Support re-baking when shape parameters change (scale, noise)
//! - (US-024) Free old SDF slots after new bake completes
//!
//! # Example
//!
//! ```ignore
//! use battle_tok_engine::render::bake_queue::{BakeQueue, BakeJob, EntityId};
//!
//! let mut bake_queue = BakeQueue::new();
//!
//! // Queue an entity for baking when it spawns
//! let entity_id = 42;
//! let job = BakeJob::new(entity_id, sdf_type, position, scale, rotation);
//! bake_queue.queue_bake(job);
//!
//! // Each frame, process up to 5 bakes
//! let completed = bake_queue.process_frame(&device, &queue, &mut sdf_manager);
//!
//! // Check transition progress for smooth blending
//! if let Some(progress) = bake_queue.get_transition_progress(entity_id) {
//!     // Use progress (0.0-1.0) for smooth blending
//! }
//!
//! // Queue a re-bake when entity scale/noise changes (US-024)
//! let rebake_job = RebakeJob::new(entity_id, old_sdf_id, sdf_type, position, scale, rotation);
//! bake_queue.queue_rebake(rebake_job);
//! ```

use std::collections::{HashMap, VecDeque};
use wgpu;

/// Maximum number of SDF bakes to process per frame
pub const MAX_BAKES_PER_FRAME: usize = 5;

/// Duration of the transition from equation to baked SDF (in seconds)
pub const TRANSITION_DURATION: f32 = 0.3;

/// Unique identifier for an entity in the bake queue system
pub type EntityId = u64;

/// Represents a single SDF bake job in the queue.
///
/// Contains all the parameters needed to bake an entity's SDF into a 3D texture.
#[derive(Clone, Debug)]
pub struct BakeJob {
    /// Unique ID of the entity to bake
    pub entity_id: EntityId,
    /// SDF primitive type (0=sphere, 1=box, 2=capsule, etc.)
    pub sdf_type: u32,
    /// Entity position in world space
    pub position: [f32; 3],
    /// Entity scale (uniform or per-axis)
    pub scale: [f32; 3],
    /// Entity rotation as quaternion (x, y, z, w)
    pub rotation: [f32; 4],
    /// Noise parameters (amplitude, frequency, octaves, enabled)
    pub noise_params: Option<NoiseParams>,
}

/// Noise displacement parameters for baking
#[derive(Clone, Debug)]
pub struct NoiseParams {
    pub amplitude: f32,
    pub frequency: f32,
    pub octaves: u32,
}

impl BakeJob {
    /// Create a new bake job for a simple entity without noise
    pub fn new(
        entity_id: EntityId,
        sdf_type: u32,
        position: [f32; 3],
        scale: [f32; 3],
        rotation: [f32; 4],
    ) -> Self {
        Self {
            entity_id,
            sdf_type,
            position,
            scale,
            rotation,
            noise_params: None,
        }
    }

    /// Create a new bake job with noise displacement
    pub fn with_noise(
        entity_id: EntityId,
        sdf_type: u32,
        position: [f32; 3],
        scale: [f32; 3],
        rotation: [f32; 4],
        amplitude: f32,
        frequency: f32,
        octaves: u32,
    ) -> Self {
        Self {
            entity_id,
            sdf_type,
            position,
            scale,
            rotation,
            noise_params: Some(NoiseParams {
                amplitude,
                frequency,
                octaves,
            }),
        }
    }
}

/// Represents a re-bake job for an entity whose shape parameters changed (US-024).
///
/// A re-bake job is similar to a BakeJob but also stores the old SDF slot ID
/// that should be freed after the new bake completes.
#[derive(Clone, Debug)]
pub struct RebakeJob {
    /// The bake job containing new parameters
    pub bake_job: BakeJob,
    /// Old baked SDF slot ID to free after new bake completes
    pub old_sdf_id: Option<u32>,
}

impl RebakeJob {
    /// Create a new re-bake job for an entity without noise
    pub fn new(
        entity_id: EntityId,
        old_sdf_id: Option<u32>,
        sdf_type: u32,
        position: [f32; 3],
        scale: [f32; 3],
        rotation: [f32; 4],
    ) -> Self {
        Self {
            bake_job: BakeJob::new(entity_id, sdf_type, position, scale, rotation),
            old_sdf_id,
        }
    }

    /// Create a new re-bake job with noise displacement
    pub fn with_noise(
        entity_id: EntityId,
        old_sdf_id: Option<u32>,
        sdf_type: u32,
        position: [f32; 3],
        scale: [f32; 3],
        rotation: [f32; 4],
        amplitude: f32,
        frequency: f32,
        octaves: u32,
    ) -> Self {
        Self {
            bake_job: BakeJob::with_noise(
                entity_id, sdf_type, position, scale, rotation,
                amplitude, frequency, octaves,
            ),
            old_sdf_id,
        }
    }
}

/// State of an entity's bake/transition
#[derive(Clone, Debug)]
pub enum BakeState {
    /// Entity is waiting in the queue to be baked
    Pending,
    /// Entity is currently being baked
    Baking,
    /// Entity bake completed, transitioning from equation to baked
    /// Contains: (baked_sdf_id, transition_start_time, transition_progress)
    Transitioning {
        baked_sdf_id: u32,
        start_time: f32,
    },
    /// Entity is fully baked and using baked SDF
    Baked {
        baked_sdf_id: u32,
    },
}

/// Manages the queue of SDF bake jobs and entity transitions.
///
/// The BakeQueue handles:
/// - Queuing new bake jobs when entities spawn
/// - Processing a limited number of bakes per frame
/// - Tracking transition progress for smooth blending
/// - Managing the lifecycle of baked SDF slots
/// - (US-024) Queuing re-bake jobs when shape parameters change
/// - (US-024) Freeing old SDF slots after new bake completes
pub struct BakeQueue {
    /// Queue of pending bake jobs (FIFO)
    pending_jobs: VecDeque<BakeJob>,
    /// Map of entity ID to current bake state
    entity_states: HashMap<EntityId, BakeState>,
    /// Current elapsed time (updated each frame)
    current_time: f32,
    /// Counter for generating unique entity IDs
    next_entity_id: EntityId,
    /// (US-024) Map of entity ID to old SDF slot ID waiting to be freed
    /// after new bake completes
    pending_sdf_frees: HashMap<EntityId, u32>,
}

impl BakeQueue {
    /// Create a new empty bake queue
    pub fn new() -> Self {
        Self {
            pending_jobs: VecDeque::new(),
            entity_states: HashMap::new(),
            current_time: 0.0,
            next_entity_id: 1,
            pending_sdf_frees: HashMap::new(),
        }
    }

    /// Generate a new unique entity ID
    pub fn generate_entity_id(&mut self) -> EntityId {
        let id = self.next_entity_id;
        self.next_entity_id += 1;
        id
    }

    /// Queue an entity for SDF baking.
    ///
    /// The entity will use equation-based rendering until the bake completes.
    pub fn queue_bake(&mut self, job: BakeJob) {
        let entity_id = job.entity_id;
        self.pending_jobs.push_back(job);
        self.entity_states.insert(entity_id, BakeState::Pending);
    }

    /// Queue a re-bake job for an entity whose shape parameters changed (US-024).
    ///
    /// This queues a new bake job and stores the old SDF slot ID to free after
    /// the new bake completes. The entity continues using its current baked SDF
    /// until the new one is ready, then transitions smoothly.
    ///
    /// # Arguments
    ///
    /// * `rebake_job` - Re-bake job containing new parameters and old SDF ID
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Entity 42 changed scale, queue re-bake
    /// let job = RebakeJob::new(42, Some(old_slot), sdf_type, pos, new_scale, rot);
    /// bake_queue.queue_rebake(job);
    /// ```
    pub fn queue_rebake(&mut self, rebake_job: RebakeJob) {
        let entity_id = rebake_job.bake_job.entity_id;

        // Store old SDF ID to free after new bake completes
        if let Some(old_id) = rebake_job.old_sdf_id {
            self.pending_sdf_frees.insert(entity_id, old_id);
        }

        // Queue the new bake job
        self.pending_jobs.push_back(rebake_job.bake_job);

        // Mark as pending (will transition through Baking -> Transitioning -> Baked)
        // Note: Entity keeps using old SDF until new one enters Transitioning state
        self.entity_states.insert(entity_id, BakeState::Pending);

        println!(
            "[BakeQueue] Queued re-bake for entity {} (old SDF: {:?})",
            entity_id, rebake_job.old_sdf_id
        );
    }

    /// Check if an entity has a pending SDF slot to free (US-024).
    pub fn has_pending_free(&self, entity_id: EntityId) -> bool {
        self.pending_sdf_frees.contains_key(&entity_id)
    }

    /// Get the pending SDF slot ID to free for an entity (US-024).
    pub fn get_pending_free(&self, entity_id: EntityId) -> Option<u32> {
        self.pending_sdf_frees.get(&entity_id).copied()
    }

    /// Get the number of pending bake jobs
    pub fn pending_count(&self) -> usize {
        self.pending_jobs.len()
    }

    /// Check if an entity is queued or being baked
    pub fn is_entity_pending(&self, entity_id: EntityId) -> bool {
        matches!(
            self.entity_states.get(&entity_id),
            Some(BakeState::Pending) | Some(BakeState::Baking)
        )
    }

    /// Get the baked SDF ID for an entity, if baking is complete.
    ///
    /// Returns None if the entity hasn't been baked or is still transitioning.
    /// During transition, returns the baked_sdf_id for use with blending.
    pub fn get_baked_sdf_id(&self, entity_id: EntityId) -> Option<u32> {
        match self.entity_states.get(&entity_id) {
            Some(BakeState::Transitioning { baked_sdf_id, .. }) => Some(*baked_sdf_id),
            Some(BakeState::Baked { baked_sdf_id }) => Some(*baked_sdf_id),
            _ => None,
        }
    }

    /// Get the transition progress for an entity (0.0 to 1.0).
    ///
    /// Returns:
    /// - `Some(0.0)` if pending/baking (use equation only)
    /// - `Some(0.0..1.0)` during transition (blend equation and baked)
    /// - `Some(1.0)` if fully baked (use baked only)
    /// - `None` if entity is not tracked
    pub fn get_transition_progress(&self, entity_id: EntityId) -> Option<f32> {
        match self.entity_states.get(&entity_id) {
            Some(BakeState::Pending) | Some(BakeState::Baking) => Some(0.0),
            Some(BakeState::Transitioning { start_time, .. }) => {
                let elapsed = self.current_time - start_time;
                let progress = (elapsed / TRANSITION_DURATION).min(1.0);
                Some(progress)
            }
            Some(BakeState::Baked { .. }) => Some(1.0),
            None => None,
        }
    }

    /// Update the current time and advance transitions.
    ///
    /// Call this once per frame before processing bakes.
    pub fn update(&mut self, elapsed_time: f32) {
        self.current_time = elapsed_time;

        // Advance transitioning entities to fully baked state
        let mut completed_transitions = Vec::new();
        for (entity_id, state) in &self.entity_states {
            if let BakeState::Transitioning { baked_sdf_id, start_time } = state {
                let elapsed = self.current_time - start_time;
                if elapsed >= TRANSITION_DURATION {
                    completed_transitions.push((*entity_id, *baked_sdf_id));
                }
            }
        }

        // Update completed transitions to Baked state
        for (entity_id, baked_sdf_id) in completed_transitions {
            self.entity_states.insert(entity_id, BakeState::Baked { baked_sdf_id });
        }
    }

    /// Process up to MAX_BAKES_PER_FRAME bake jobs this frame.
    ///
    /// Returns a list of (entity_id, baked_sdf_id) pairs for entities that
    /// completed baking this frame. The caller should update the entity's
    /// `baked_sdf_id` field accordingly.
    ///
    /// For re-bake jobs (US-024), this also frees the old SDF slot after the
    /// new bake completes and the transition to using the new SDF begins.
    ///
    /// # Arguments
    ///
    /// * `device` - wgpu device for creating compute pipeline
    /// * `queue` - wgpu queue for submitting commands
    /// * `sdf_manager` - BrickCache for allocating SDF slots
    ///
    /// Note: This is a placeholder implementation. Full GPU compute baking
    /// requires setting up a compute pipeline with the sdf_bake.wgsl shader.
    pub fn process_frame(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        sdf_manager: &mut super::sdf_baker::BrickCache,
    ) -> Vec<(EntityId, u32)> {
        let mut completed = Vec::new();

        // Process up to MAX_BAKES_PER_FRAME jobs
        for _ in 0..MAX_BAKES_PER_FRAME {
            if let Some(job) = self.pending_jobs.pop_front() {
                // Try to allocate an SDF slot
                if let Some(slot_id) = sdf_manager.allocate_sdf_slot() {
                    // Mark as baking
                    self.entity_states.insert(job.entity_id, BakeState::Baking);

                    // TODO: In a full implementation, we would:
                    // 1. Create a compute pipeline with sdf_bake.wgsl
                    // 2. Set up the BakeParams uniform buffer
                    // 3. Create a 3D texture for output
                    // 4. Dispatch the compute shader (16, 16, 16 workgroups)
                    // 5. Copy the result to the SDF texture array
                    //
                    // For now, we simulate an instant bake by just allocating the slot
                    // and marking the entity as transitioning.

                    // Simulate bake completion - in reality this would happen async
                    // after the GPU compute shader finishes
                    self.entity_states.insert(
                        job.entity_id,
                        BakeState::Transitioning {
                            baked_sdf_id: slot_id,
                            start_time: self.current_time,
                        },
                    );

                    // US-024: Free old SDF slot now that new bake is starting transition
                    // The entity will blend from old to new during transition
                    if let Some(old_sdf_id) = self.pending_sdf_frees.remove(&job.entity_id) {
                        sdf_manager.free_sdf_slot(old_sdf_id);
                        println!(
                            "[BakeQueue] Freed old SDF slot {} for entity {} (re-bake completed)",
                            old_sdf_id, job.entity_id
                        );
                    }

                    completed.push((job.entity_id, slot_id));

                    println!(
                        "[BakeQueue] Baked entity {} into slot {} (transition started)",
                        job.entity_id, slot_id
                    );
                } else {
                    // No slots available, put the job back at the front
                    self.pending_jobs.push_front(job);
                    println!("[BakeQueue] No SDF slots available, deferring bake");
                    break;
                }
            } else {
                // Queue is empty
                break;
            }
        }

        completed
    }

    /// Free the baked SDF slot for an entity that's being removed.
    ///
    /// Call this when an entity is despawned to reclaim the SDF texture slot.
    /// Also frees any pending old SDF slot from a re-bake (US-024).
    pub fn free_entity(&mut self, entity_id: EntityId, sdf_manager: &mut super::sdf_baker::BrickCache) {
        if let Some(state) = self.entity_states.remove(&entity_id) {
            match state {
                BakeState::Transitioning { baked_sdf_id, .. } |
                BakeState::Baked { baked_sdf_id } => {
                    sdf_manager.free_sdf_slot(baked_sdf_id);
                    println!(
                        "[BakeQueue] Freed entity {} SDF slot {}",
                        entity_id, baked_sdf_id
                    );
                }
                _ => {
                    // Entity was pending or baking, nothing to free
                }
            }
        }

        // US-024: Also free any pending old SDF slot from a re-bake
        if let Some(old_sdf_id) = self.pending_sdf_frees.remove(&entity_id) {
            sdf_manager.free_sdf_slot(old_sdf_id);
            println!(
                "[BakeQueue] Freed pending old SDF slot {} for removed entity {}",
                old_sdf_id, entity_id
            );
        }

        // Also remove from pending queue if present
        self.pending_jobs.retain(|job| job.entity_id != entity_id);
    }

    /// Clear all pending jobs and free all allocated SDF slots.
    ///
    /// Also frees all pending old SDF slots from re-bakes (US-024).
    pub fn clear(&mut self, sdf_manager: &mut super::sdf_baker::BrickCache) {
        // Free all allocated SDF slots (this also handles pending_sdf_frees)
        let entity_ids: Vec<EntityId> = self.entity_states.keys().copied().collect();
        for entity_id in entity_ids {
            self.free_entity(entity_id, sdf_manager);
        }

        self.pending_jobs.clear();
        self.entity_states.clear();
        self.pending_sdf_frees.clear(); // Clear any remaining (should be empty)
    }

    /// Get the number of pending SDF slots to free (US-024).
    pub fn pending_free_count(&self) -> usize {
        self.pending_sdf_frees.len()
    }
}

impl Default for BakeQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bake_job_creation() {
        let job = BakeJob::new(1, 0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(job.entity_id, 1);
        assert_eq!(job.sdf_type, 0);
        assert!(job.noise_params.is_none());
    }

    #[test]
    fn test_bake_job_with_noise() {
        let job = BakeJob::with_noise(
            2, 1, [1.0, 2.0, 3.0], [0.5, 0.5, 0.5], [0.0, 0.0, 0.0, 1.0],
            0.1, 2.0, 4,
        );
        assert_eq!(job.entity_id, 2);
        assert!(job.noise_params.is_some());
        let noise = job.noise_params.unwrap();
        assert_eq!(noise.amplitude, 0.1);
        assert_eq!(noise.frequency, 2.0);
        assert_eq!(noise.octaves, 4);
    }

    #[test]
    fn test_bake_queue_basic() {
        let mut queue = BakeQueue::new();

        // Generate unique IDs
        let id1 = queue.generate_entity_id();
        let id2 = queue.generate_entity_id();
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);

        // Queue jobs
        let job1 = BakeJob::new(id1, 0, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 1.0]);
        let job2 = BakeJob::new(id2, 1, [5.0, 0.0, 0.0], [2.0, 2.0, 2.0], [0.0, 0.0, 0.0, 1.0]);

        queue.queue_bake(job1);
        queue.queue_bake(job2);

        assert_eq!(queue.pending_count(), 2);
        assert!(queue.is_entity_pending(id1));
        assert!(queue.is_entity_pending(id2));
    }

    #[test]
    fn test_transition_progress() {
        let mut queue = BakeQueue::new();

        let id = queue.generate_entity_id();
        queue.entity_states.insert(id, BakeState::Pending);

        // Pending should return 0.0
        assert_eq!(queue.get_transition_progress(id), Some(0.0));

        // Simulate transition start
        queue.entity_states.insert(
            id,
            BakeState::Transitioning {
                baked_sdf_id: 1,
                start_time: 0.0,
            },
        );

        // At time 0, progress should be 0
        queue.current_time = 0.0;
        assert_eq!(queue.get_transition_progress(id), Some(0.0));

        // At half transition time, progress should be ~0.5
        queue.current_time = TRANSITION_DURATION / 2.0;
        let progress = queue.get_transition_progress(id).unwrap();
        assert!((progress - 0.5).abs() < 0.01);

        // At full transition time, progress should be 1.0
        queue.current_time = TRANSITION_DURATION;
        assert_eq!(queue.get_transition_progress(id), Some(1.0));

        // After update, entity should be fully baked
        queue.update(TRANSITION_DURATION);
        assert!(matches!(queue.entity_states.get(&id), Some(BakeState::Baked { .. })));
        assert_eq!(queue.get_transition_progress(id), Some(1.0));
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_BAKES_PER_FRAME, 5);
        assert_eq!(TRANSITION_DURATION, 0.3);
    }

    // US-024: Tests for re-bake functionality

    #[test]
    fn test_rebake_job_creation() {
        let job = RebakeJob::new(
            42,
            Some(10), // Old SDF slot ID
            0,
            [0.0, 0.0, 0.0],
            [2.0, 2.0, 2.0], // New scale
            [0.0, 0.0, 0.0, 1.0],
        );
        assert_eq!(job.bake_job.entity_id, 42);
        assert_eq!(job.old_sdf_id, Some(10));
        assert_eq!(job.bake_job.scale, [2.0, 2.0, 2.0]);
    }

    #[test]
    fn test_rebake_job_with_noise() {
        let job = RebakeJob::with_noise(
            42,
            Some(10),
            0,
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.0, 0.0, 0.0, 1.0],
            0.2,  // New amplitude
            3.0,  // New frequency
            8,    // New octaves
        );
        assert_eq!(job.bake_job.entity_id, 42);
        assert_eq!(job.old_sdf_id, Some(10));
        let noise = job.bake_job.noise_params.unwrap();
        assert_eq!(noise.amplitude, 0.2);
        assert_eq!(noise.frequency, 3.0);
        assert_eq!(noise.octaves, 8);
    }

    #[test]
    fn test_queue_rebake() {
        let mut queue = BakeQueue::new();

        // First, simulate an entity that was already baked
        let entity_id = 42;
        let old_sdf_id = 10;

        // Queue a re-bake job
        let rebake_job = RebakeJob::new(
            entity_id,
            Some(old_sdf_id),
            0,
            [0.0, 0.0, 0.0],
            [2.0, 2.0, 2.0],
            [0.0, 0.0, 0.0, 1.0],
        );
        queue.queue_rebake(rebake_job);

        // Verify the job is pending and old SDF ID is tracked
        assert_eq!(queue.pending_count(), 1);
        assert!(queue.is_entity_pending(entity_id));
        assert!(queue.has_pending_free(entity_id));
        assert_eq!(queue.get_pending_free(entity_id), Some(old_sdf_id));
    }

    #[test]
    fn test_rebake_no_old_sdf() {
        let mut queue = BakeQueue::new();

        // Re-bake job without old SDF (entity never finished initial bake)
        let rebake_job = RebakeJob::new(
            42,
            None, // No old SDF
            0,
            [0.0, 0.0, 0.0],
            [2.0, 2.0, 2.0],
            [0.0, 0.0, 0.0, 1.0],
        );
        queue.queue_rebake(rebake_job);

        // Should not have pending free
        assert!(!queue.has_pending_free(42));
        assert_eq!(queue.get_pending_free(42), None);
    }

    #[test]
    fn test_pending_free_count() {
        let mut queue = BakeQueue::new();

        // Queue multiple re-bake jobs
        queue.queue_rebake(RebakeJob::new(1, Some(10), 0, [0.0; 3], [1.0; 3], [0.0, 0.0, 0.0, 1.0]));
        queue.queue_rebake(RebakeJob::new(2, Some(20), 0, [0.0; 3], [1.0; 3], [0.0, 0.0, 0.0, 1.0]));
        queue.queue_rebake(RebakeJob::new(3, None, 0, [0.0; 3], [1.0; 3], [0.0, 0.0, 0.0, 1.0])); // No old SDF

        assert_eq!(queue.pending_free_count(), 2); // Only 2 have old SDFs
    }
}
