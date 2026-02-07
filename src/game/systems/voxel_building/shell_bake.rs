use glam::Vec3;

use super::types::{ShellBakeJob, ShellBakeResult, VoxelAabb, VoxelCoord};
use super::world::VoxelWorld;

pub const FULL_SHELL_BAKE_INTERVAL_S: f32 = 10.0;

#[derive(Default)]
pub struct ShellBakeScheduler {
    pending_local_bounds: Option<VoxelAabb>,
    full_bake_timer_s: f32,
    results: Vec<ShellBakeResult>,
}

impl ShellBakeScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_voxel_dirty(&mut self, coord: VoxelCoord) {
        let center = VoxelWorld::voxel_to_world_center(coord);
        let half = Vec3::splat(super::world::VOXEL_SIZE_METERS * 0.55);
        let min = center - half;
        let max = center + half;
        if let Some(bounds) = &mut self.pending_local_bounds {
            bounds.include_point(min);
            bounds.include_point(max);
        } else {
            self.pending_local_bounds = Some(VoxelAabb { min, max });
        }
    }

    pub fn tick(&mut self, dt: f32) -> Vec<ShellBakeJob> {
        let mut jobs = Vec::new();
        if let Some(bounds) = self.pending_local_bounds.take() {
            jobs.push(ShellBakeJob {
                dirty_aabb: bounds,
                priority: 3,
                reason: "local_dirty",
            });
        }

        self.full_bake_timer_s += dt.max(0.0);
        if self.full_bake_timer_s >= FULL_SHELL_BAKE_INTERVAL_S {
            self.full_bake_timer_s = 0.0;
            let world_span = VoxelAabb {
                min: Vec3::splat(-4096.0),
                max: Vec3::splat(4096.0),
            };
            jobs.push(ShellBakeJob {
                dirty_aabb: world_span,
                priority: 1,
                reason: "full_consolidation",
            });
        }

        jobs
    }

    pub fn push_result(&mut self, result: ShellBakeResult) {
        self.results.push(result);
    }

    pub fn drain_results(&mut self) -> Vec<ShellBakeResult> {
        std::mem::take(&mut self.results)
    }
}
