pub mod brick_tree;
pub mod cluster_physics;
pub mod connectivity;
pub mod damage;
pub mod shell_bake;
pub mod types;
pub mod ui_bridge;
pub mod worker;
pub mod world;

use glam::{IVec3, Vec3};

use self::brick_tree::BrickTree;
use self::cluster_physics::ClusterPhysics;
use self::connectivity::disconnected_components;
use self::damage::{apply_damage_at_hit, default_voxel_cell, oct_encode_from_normal};
use self::shell_bake::ShellBakeScheduler;
use self::world::VoxelWorld;

pub use self::types::{
    BrickLeaf64, BrickNode, BuildAudioEvent, BuildAudioEventKind, DamageSource,
    RaymarchQualityState, RenderDeltaBatch, ShellBakeJob, ShellBakeResult, ShellBlendState,
    VoxelAabb, VoxelCell, VoxelCoord, VoxelDamageResult, VoxelHit, VoxelMaterialId,
};
pub use self::ui_bridge::{BuildMode, VoxelHudState};
pub use self::world::VOXEL_SIZE_METERS;

pub struct VoxelBuildingRuntime {
    pub world: VoxelWorld,
    pub brick_tree: BrickTree,
    pub blend_state: ShellBlendState,
    bake_scheduler: ShellBakeScheduler,
    cluster_physics: ClusterPhysics,
    render_delta: RenderDeltaBatch,
    audio_events: Vec<BuildAudioEvent>,
    world_changed_since_sync: bool,
}

impl Default for VoxelBuildingRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl VoxelBuildingRuntime {
    pub fn new() -> Self {
        Self {
            world: VoxelWorld::new(),
            brick_tree: BrickTree::new(),
            blend_state: ShellBlendState::default(),
            bake_scheduler: ShellBakeScheduler::new(),
            cluster_physics: ClusterPhysics::new(),
            render_delta: RenderDeltaBatch::default(),
            audio_events: Vec::new(),
            world_changed_since_sync: false,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.cluster_physics.tick(dt, &mut self.audio_events);
        let dirty_chunks = self.world.drain_dirty_chunks();
        for chunk in &dirty_chunks {
            self.brick_tree.mark_chunk_dirty(*chunk);
        }
        if !dirty_chunks.is_empty() {
            self.world_changed_since_sync = true;
        }
        self.render_delta.dirty_chunks.extend(dirty_chunks);

        let jobs = self.bake_scheduler.tick(dt);
        if !jobs.is_empty() {
            self.blend_state.preview_active = true;
            self.blend_state.blend_t = 0.0;
            self.render_delta.bake_jobs.extend(jobs);
        }

        if self.blend_state.preview_active {
            self.blend_state.blend_t = (self.blend_state.blend_t
                + dt / self.blend_state.blend_duration_s.max(0.001))
            .clamp(0.0, 1.0);
            if self.blend_state.blend_t >= 1.0 {
                self.blend_state.preview_active = false;
            }
        }

        self.render_delta
            .bake_results
            .extend(self.bake_scheduler.drain_results());
    }

    pub fn raycast_voxel(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<VoxelHit> {
        self.world.raycast_voxel(origin, dir, max_dist)
    }

    pub fn place_voxel(&mut self, coord: VoxelCoord, material: VoxelMaterialId) -> bool {
        let cell = default_voxel_cell(material.0, [128, 128]);
        let placed = self.world.place(coord, cell);
        if placed {
            self.bake_scheduler.mark_voxel_dirty(coord);
        }
        placed
    }

    pub fn remove_voxel(&mut self, coord: VoxelCoord) -> bool {
        let removed = self.world.remove(coord).is_some();
        if removed {
            self.bake_scheduler.mark_voxel_dirty(coord);
        }
        removed
    }

    pub fn place_corner_brush(
        &mut self,
        anchor: VoxelCoord,
        face_normal: IVec3,
        radius_vox: u8,
        material: VoxelMaterialId,
    ) -> usize {
        let radius = radius_vox.max(1) as i32;
        let normal = Vec3::new(
            face_normal.x as f32,
            face_normal.y as f32,
            face_normal.z as f32,
        )
        .normalize_or_zero();
        let normal_oct = oct_encode_from_normal(normal);
        let mut placed = 0usize;
        for z in -radius..=radius {
            for y in -radius..=radius {
                for x in -radius..=radius {
                    let offs = Vec3::new(x as f32, y as f32, z as f32);
                    if offs.length_squared() > (radius as f32 + 0.25).powi(2) {
                        continue;
                    }
                    let dot = offs.dot(normal);
                    if dot < -0.25 {
                        continue;
                    }
                    let coord = VoxelCoord::new(anchor.x + x, anchor.y + y, anchor.z + z);
                    let cell = default_voxel_cell(material.0, normal_oct);
                    if self.world.place(coord, cell) {
                        self.bake_scheduler.mark_voxel_dirty(coord);
                        placed += 1;
                    }
                }
            }
        }
        placed
    }

    pub fn apply_damage_at_hit(
        &mut self,
        hit: VoxelHit,
        damage: f32,
        impulse: Vec3,
        source: DamageSource,
    ) -> VoxelDamageResult {
        let result =
            apply_damage_at_hit(&mut self.world, hit, damage, impulse, source, &mut self.audio_events);
        self.bake_scheduler.mark_voxel_dirty(hit.coord);

        if result.destroyed {
            let disconnected = disconnected_components(&self.world);
            if !disconnected.is_empty() {
                self.cluster_physics.spawn_components(
                    &mut self.world,
                    disconnected,
                    &mut self.audio_events,
                );
            }
        }

        result
    }

    pub fn rebuild_brick_tree(&mut self) {
        self.brick_tree.rebuild_from_world(&self.world);
    }

    pub fn take_world_change_flag(&mut self) -> bool {
        let changed = self.world_changed_since_sync;
        self.world_changed_since_sync = false;
        changed
    }

    pub fn drain_render_deltas(&mut self) -> RenderDeltaBatch {
        std::mem::take(&mut self.render_delta)
    }

    pub fn drain_audio_events(&mut self) -> Vec<BuildAudioEvent> {
        std::mem::take(&mut self.audio_events)
    }
}
