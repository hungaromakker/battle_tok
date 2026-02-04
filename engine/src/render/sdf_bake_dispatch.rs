//! SDF Bake Compute Dispatcher (US-0M03)
//!
//! Connects the BakeQueue to the GPU compute pipeline, dispatching sdf_bake.wgsl
//! to fill 64³ 3D texture slots. Rate-limited to MAX_BAKES_PER_FRAME per frame.
//!
//! Entities use equation-based rendering until their bake completes (fallback path).

use std::collections::VecDeque;
use wgpu;
use wgpu::util::DeviceExt;

use super::bake_queue::{BakeJob, EntityId, MAX_BAKES_PER_FRAME};
use super::compute_pipelines::ComputePipelines;
use super::sdf_baker::BrickCache;

/// Size of the BakeParams uniform buffer in bytes (must match shader struct).
/// See shaders/sdf_bake.wgsl BakeParams: 96 bytes total.
const BAKE_PARAMS_SIZE: usize = 112;

/// GPU-side bake parameters matching the BakeParams struct in sdf_bake.wgsl.
///
/// Layout (112 bytes):
/// - position: vec3<f32> + sdf_type: u32      (16 bytes)
/// - scale: vec3<f32> + _pad0: u32            (16 bytes)
/// - rotation: vec4<f32>                       (16 bytes)
/// - bounds_min: vec3<f32> + _pad1: u32       (16 bytes)
/// - bounds_max: vec3<f32> + _pad2: u32       (16 bytes)
/// - noise_amplitude: f32 + noise_frequency: f32 + noise_octaves: u32 + use_noise: u32 (16 bytes)
/// - brick_offset: u32 + _pad3: [u32; 3]     (16 bytes)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuBakeParams {
    pub position: [f32; 3],
    pub sdf_type: u32,
    pub scale: [f32; 3],
    pub _pad0: u32,
    pub rotation: [f32; 4],
    pub bounds_min: [f32; 3],
    pub _pad1: u32,
    pub bounds_max: [f32; 3],
    pub _pad2: u32,
    pub noise_amplitude: f32,
    pub noise_frequency: f32,
    pub noise_octaves: u32,
    pub use_noise: u32,
    pub brick_offset: u32,
    pub _pad3: [u32; 3],
}

// Compile-time size check
const _: () = assert!(std::mem::size_of::<GpuBakeParams>() == BAKE_PARAMS_SIZE);

/// Tracks which entities are still using equation-based (fallback) rendering
/// because their bake has not yet completed.
pub struct FallbackState {
    /// Set of entity IDs that are pending bake and should use equation rendering.
    pending_entities: std::collections::HashSet<EntityId>,
}

impl FallbackState {
    pub fn new() -> Self {
        Self {
            pending_entities: std::collections::HashSet::new(),
        }
    }

    /// Returns true if the entity should use equation-based rendering (bake not done).
    pub fn uses_equation_fallback(&self, entity_id: EntityId) -> bool {
        self.pending_entities.contains(&entity_id)
    }

    /// Mark an entity as pending bake (uses equation fallback).
    pub fn mark_pending(&mut self, entity_id: EntityId) {
        self.pending_entities.insert(entity_id);
    }

    /// Mark an entity's bake as complete (no longer needs fallback).
    pub fn mark_complete(&mut self, entity_id: EntityId) {
        self.pending_entities.remove(&entity_id);
    }
}

impl Default for FallbackState {
    fn default() -> Self {
        Self::new()
    }
}

/// Dispatches SDF bake compute passes, connecting BakeQueue jobs to the GPU pipeline.
///
/// Usage:
/// 1. Call `queue_bake()` to add entities for baking
/// 2. Call `flush_bakes()` each frame to dispatch up to 5 compute passes
/// 3. Check `fallback_state()` to know which entities still need equation rendering
pub struct SdfBakeDispatcher {
    /// Pending bake requests (entity_id + params, before slot allocation)
    queue: VecDeque<(EntityId, GpuBakeParams)>,
    /// Tracks which entities are using equation-based fallback rendering
    fallback: FallbackState,
}

impl SdfBakeDispatcher {
    /// Create a new dispatcher.
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            fallback: FallbackState::new(),
        }
    }

    /// Queue an entity for baking. The entity will use equation rendering until done.
    pub fn queue_bake(&mut self, entity_id: EntityId, bake_params: GpuBakeParams) {
        self.fallback.mark_pending(entity_id);
        self.queue.push_back((entity_id, bake_params));
    }

    /// Convenience: queue a bake from a BakeJob (converts to GpuBakeParams).
    pub fn queue_bake_from_job(&mut self, job: &BakeJob) {
        let max_scale = job.scale[0].max(job.scale[1]).max(job.scale[2]);
        let margin = max_scale * 1.2; // 20% margin around the entity

        let (noise_amplitude, noise_frequency, noise_octaves, use_noise) =
            if let Some(ref noise) = job.noise_params {
                (noise.amplitude, noise.frequency, noise.octaves, 1u32)
            } else {
                (0.0, 0.0, 0, 0u32)
            };

        let params = GpuBakeParams {
            position: job.position,
            sdf_type: job.sdf_type,
            scale: job.scale,
            _pad0: 0,
            rotation: job.rotation,
            bounds_min: [-margin, -margin, -margin],
            _pad1: 0,
            bounds_max: [margin, margin, margin],
            _pad2: 0,
            noise_amplitude,
            noise_frequency,
            noise_octaves,
            use_noise,
            brick_offset: 0, // set by flush_bakes after slot allocation
            _pad3: [0; 3],
        };

        self.queue_bake(job.entity_id, params);
    }

    /// Flush pending bakes as compute passes. Rate-limited to MAX_BAKES_PER_FRAME.
    ///
    /// Returns a list of (entity_id, slot_id) for completed dispatches.
    /// The caller should update entity bake state accordingly.
    pub fn flush_bakes(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        pipelines: &ComputePipelines,
        brick_cache: &mut BrickCache,
    ) -> Vec<(EntityId, u32)> {
        let mut completed = Vec::new();
        let mut dispatched = 0;

        while dispatched < MAX_BAKES_PER_FRAME {
            let (entity_id, params) = match self.queue.pop_front() {
                Some(item) => item,
                None => break,
            };

            // Allocate SDF slot
            let slot_id = match brick_cache.allocate_sdf_slot() {
                Some(id) => id,
                None => {
                    // No slots available, put back and stop
                    self.queue.push_front((entity_id, params));
                    println!("[SdfBakeDispatcher] No SDF slots available, deferring");
                    break;
                }
            };

            // Set brick_offset based on allocated slot
            let mut params = params;
            params.brick_offset = slot_id * 262_144;

            // Create uniform buffer with bake params
            let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("bake_params_buffer"),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            // Create bind group for this dispatch
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("sdf_bake_bind_group"),
                layout: &pipelines.sdf_bake_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: brick_cache.buffer().as_entire_binding(),
                    },
                ],
            });

            // Dispatch compute pass
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("sdf_bake_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&pipelines.sdf_bake_pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(16, 16, 16); // 64/4 = 16 per dimension
            }

            self.fallback.mark_complete(entity_id);
            completed.push((entity_id, slot_id));
            dispatched += 1;

            println!(
                "[SdfBakeDispatcher] Dispatched bake for entity {} -> slot {}",
                entity_id, slot_id
            );
        }

        completed
    }

    /// Get the fallback state to check which entities need equation rendering.
    pub fn fallback_state(&self) -> &FallbackState {
        &self.fallback
    }

    /// Get the number of pending bakes in the queue.
    pub fn pending_count(&self) -> usize {
        self.queue.len()
    }
}

impl Default for SdfBakeDispatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_bake_params_size() {
        assert_eq!(std::mem::size_of::<GpuBakeParams>(), 112);
    }

    #[test]
    fn test_dispatcher_queue() {
        let mut dispatcher = SdfBakeDispatcher::new();

        let params = GpuBakeParams {
            position: [0.0, 0.0, 0.0],
            sdf_type: 0,
            scale: [1.0, 1.0, 1.0],
            _pad0: 0,
            rotation: [0.0, 0.0, 0.0, 1.0],
            bounds_min: [-1.2, -1.2, -1.2],
            _pad1: 0,
            bounds_max: [1.2, 1.2, 1.2],
            _pad2: 0,
            noise_amplitude: 0.0,
            noise_frequency: 0.0,
            noise_octaves: 0,
            use_noise: 0,
            brick_offset: 0,
            _pad3: [0; 3],
        };

        dispatcher.queue_bake(42, params);
        assert_eq!(dispatcher.pending_count(), 1);
        assert!(dispatcher.fallback_state().uses_equation_fallback(42));
    }

    #[test]
    fn test_queue_from_bake_job() {
        let mut dispatcher = SdfBakeDispatcher::new();

        let job = BakeJob::new(
            10,
            0,
            [5.0, 0.0, 3.0],
            [2.0, 2.0, 2.0],
            [0.0, 0.0, 0.0, 1.0],
        );
        dispatcher.queue_bake_from_job(&job);

        assert_eq!(dispatcher.pending_count(), 1);
        assert!(dispatcher.fallback_state().uses_equation_fallback(10));
    }

    #[test]
    fn test_fallback_state() {
        let mut fallback = FallbackState::new();

        assert!(!fallback.uses_equation_fallback(1));

        fallback.mark_pending(1);
        assert!(fallback.uses_equation_fallback(1));

        fallback.mark_complete(1);
        assert!(!fallback.uses_equation_fallback(1));
    }
}
