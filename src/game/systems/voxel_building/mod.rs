pub mod brick_tree;
pub mod cluster_physics;
pub mod connectivity;
pub mod damage;
pub mod shell_bake;
pub mod types;
pub mod ui_bridge;
pub mod worker;
pub mod world;

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};

use glam::{IVec3, Vec3};

use self::brick_tree::BrickTree;
use self::cluster_physics::ClusterPhysics;
use self::connectivity::{neighbors6, unsupported_from_region};
use self::damage::{apply_damage_at_hit, default_voxel_cell, oct_encode_from_normal};
use self::shell_bake::ShellBakeScheduler;
use self::worker::{VoxelWorker, WorkerCommand, WorkerEvent};
use self::world::VoxelWorld;

pub use self::types::{
    BrickLeaf64, BrickNode, BuildAudioEvent, BuildAudioEventKind, CastleToolParams, DamageSource,
    RaymarchQualityState, RenderDeltaBatch, ShellBakeJob, ShellBakeResult, ShellBlendState,
    SupportReason, SupportSolveJob, SupportSolveResult, VoxelAabb, VoxelBatchResult, VoxelCell,
    VoxelCoord, VoxelDamageResult, VoxelEdit, VoxelEditBatch, VoxelEditOp, VoxelHit,
    VoxelMaterialId, VOXEL_FLAG_RIB_MEMBER, VOXEL_FLAG_RIGID_JOINT, VOXEL_FLAG_TERRAIN_ANCHORED,
};
pub use self::ui_bridge::{BuildMode, VoxelHudState};
pub use self::world::VOXEL_SIZE_METERS;

const SUPPORT_REGION_EXPAND_VOX: i32 = 2;
const SUPPORT_REGION_CELL_CAP: usize = 8_192;

pub struct VoxelBuildingRuntime {
    pub world: VoxelWorld,
    pub brick_tree: BrickTree,
    pub blend_state: ShellBlendState,
    bake_scheduler: ShellBakeScheduler,
    cluster_physics: ClusterPhysics,
    render_delta: RenderDeltaBatch,
    audio_events: Vec<BuildAudioEvent>,
    world_changed_since_sync: bool,
    world_revision: u64,
    support_worker: Option<VoxelWorker>,
    support_job_in_flight: bool,
    pending_support_coords: Vec<VoxelCoord>,
    pending_support_reason: Option<SupportReason>,
    applied_support_results: Vec<SupportSolveResult>,
    changed_coords: Vec<VoxelCoord>,
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
            world_revision: 1,
            support_worker: {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    Some(VoxelWorker::spawn())
                }
                #[cfg(target_arch = "wasm32")]
                {
                    None
                }
            },
            support_job_in_flight: false,
            pending_support_coords: Vec::new(),
            pending_support_reason: None,
            applied_support_results: Vec::new(),
            changed_coords: Vec::new(),
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.cluster_physics.tick(dt, &mut self.audio_events);
        self.pump_support_worker();

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

    pub fn raycast_voxel_segment(&self, start: Vec3, end: Vec3, radius: f32) -> Option<VoxelHit> {
        let seg = end - start;
        let seg_len = seg.length();
        if seg_len <= 1e-5 {
            return None;
        }
        let dir = seg / seg_len;
        let mut best = self.world.raycast_voxel(start, dir, seg_len);

        if radius > 0.0 {
            let ref_axis = if dir.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
            let right = dir.cross(ref_axis).normalize_or_zero();
            let up = right.cross(dir).normalize_or_zero();
            for offs in [right * radius, -right * radius, up * radius, -up * radius] {
                if let Some(hit) = self.world.raycast_voxel(start + offs, dir, seg_len) {
                    best = nearer_hit(start, best, Some(hit));
                }
            }
        }

        best
    }

    pub fn place_voxel(&mut self, coord: VoxelCoord, material: VoxelMaterialId) -> bool {
        let cell = default_voxel_cell(material.0, [128, 128]);
        let placed = self.world.place(coord, cell);
        if placed {
            self.bump_revision();
            self.bake_scheduler.mark_voxel_dirty(coord);
            self.changed_coords.push(coord);
        }
        placed
    }

    pub fn remove_voxel(&mut self, coord: VoxelCoord) -> bool {
        let removed = self.world.remove(coord).is_some();
        if removed {
            self.bump_revision();
            self.bake_scheduler.mark_voxel_dirty(coord);
            self.queue_support_recheck(&[coord], SupportReason::Remove);
            self.changed_coords.push(coord);
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
        let mut edits = Vec::new();

        for z in -radius..=radius {
            for y in -radius..=radius {
                for x in -radius..=radius {
                    let offs = Vec3::new(x as f32, y as f32, z as f32);
                    if offs.length_squared() > (radius as f32 + 0.25).powi(2) {
                        continue;
                    }
                    if offs.dot(normal) < -0.25 {
                        continue;
                    }
                    edits.push(VoxelEdit::place(
                        VoxelCoord::new(anchor.x + x, anchor.y + y, anchor.z + z),
                        material,
                        normal_oct,
                        0,
                    ));
                }
            }
        }

        let batch = VoxelEditBatch {
            edits,
            request_support_check: false,
            support_reason: None,
        };
        self.apply_voxel_batch(&batch).placed
    }

    pub fn apply_voxel_batch(&mut self, batch: &VoxelEditBatch) -> VoxelBatchResult {
        if batch.edits.is_empty() {
            return VoxelBatchResult::default();
        }

        let mut edits = batch.edits.clone();
        edits.sort_by(|a, b| {
            let lhs = (a.coord.x, a.coord.y, a.coord.z);
            let rhs = (b.coord.x, b.coord.y, b.coord.z);
            lhs.cmp(&rhs).then_with(|| {
                let ao = if a.op == VoxelEditOp::Remove { 0 } else { 1 };
                let bo = if b.op == VoxelEditOp::Remove { 0 } else { 1 };
                ao.cmp(&bo)
            })
        });

        let mut changed = BTreeSet::new();
        let mut result = VoxelBatchResult::default();
        let mut destructive = false;

        for edit in edits {
            match edit.op {
                VoxelEditOp::Place => {
                    let mut cell = default_voxel_cell(edit.material.0, edit.normal_oct);
                    cell.flags = edit.flags;
                    let was_empty = self.world.get(edit.coord).is_none();
                    if self.world.place(edit.coord, cell) {
                        result.applied += 1;
                        if was_empty {
                            result.placed += 1;
                        }
                        changed.insert(edit.coord);
                        self.bake_scheduler.mark_voxel_dirty(edit.coord);
                        self.changed_coords.push(edit.coord);
                    }
                }
                VoxelEditOp::Remove => {
                    if self.world.remove(edit.coord).is_some() {
                        result.applied += 1;
                        result.removed += 1;
                        destructive = true;
                        changed.insert(edit.coord);
                        self.bake_scheduler.mark_voxel_dirty(edit.coord);
                        self.changed_coords.push(edit.coord);
                    }
                }
            }
        }

        result.changed_coords = changed.into_iter().collect();
        if !result.changed_coords.is_empty() {
            self.bump_revision();
        }

        if (destructive || batch.request_support_check) && !result.changed_coords.is_empty() {
            let reason = batch
                .support_reason
                .unwrap_or(SupportReason::BatchDestructive);
            self.queue_support_recheck(&result.changed_coords, reason);
        }

        result
    }

    pub fn build_base_plate_rect(
        &mut self,
        anchor_a: VoxelCoord,
        anchor_b: VoxelCoord,
        material: VoxelMaterialId,
        params: CastleToolParams,
    ) -> VoxelBatchResult {
        let min_x = anchor_a.x.min(anchor_b.x);
        let max_x = anchor_a.x.max(anchor_b.x);
        let min_z = anchor_a.z.min(anchor_b.z);
        let max_z = anchor_a.z.max(anchor_b.z);
        let hint_y = anchor_a.y.min(anchor_b.y);
        let thickness = params.plate_thickness_vox.max(1) as i32;

        let mut edits = Vec::new();
        for z in min_z..=max_z {
            for x in min_x..=max_x {
                let base_y = self.terrain_conform_y(x, z, hint_y);
                for dy in 0..thickness {
                    let flags = if dy == 0 {
                        VOXEL_FLAG_TERRAIN_ANCHORED
                    } else {
                        0
                    };
                    edits.push(VoxelEdit::place(
                        VoxelCoord::new(x, base_y + dy, z),
                        material,
                        [128, 128],
                        flags,
                    ));
                }
            }
        }

        self.apply_voxel_batch(&VoxelEditBatch {
            edits,
            request_support_check: false,
            support_reason: None,
        })
    }

    pub fn build_base_plate_circle(
        &mut self,
        center: VoxelCoord,
        radius_vox: u8,
        material: VoxelMaterialId,
        params: CastleToolParams,
    ) -> VoxelBatchResult {
        let radius = radius_vox.max(1) as i32;
        let thickness = params.plate_thickness_vox.max(1) as i32;
        let mut edits = Vec::new();

        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dz * dz > radius * radius {
                    continue;
                }
                let x = center.x + dx;
                let z = center.z + dz;
                let base_y = self.terrain_conform_y(x, z, center.y);
                for dy in 0..thickness {
                    let flags = if dy == 0 {
                        VOXEL_FLAG_TERRAIN_ANCHORED
                    } else {
                        0
                    };
                    edits.push(VoxelEdit::place(
                        VoxelCoord::new(x, base_y + dy, z),
                        material,
                        [128, 128],
                        flags,
                    ));
                }
            }
        }

        self.apply_voxel_batch(&VoxelEditBatch {
            edits,
            request_support_check: false,
            support_reason: None,
        })
    }

    pub fn build_wall_line(
        &mut self,
        anchor_a: VoxelCoord,
        anchor_b: VoxelCoord,
        material: VoxelMaterialId,
        params: CastleToolParams,
    ) -> VoxelBatchResult {
        let spine = supercover_line_xz((anchor_a.x, anchor_a.z), (anchor_b.x, anchor_b.z));
        self.build_wall_from_spine(spine, anchor_a.y.min(anchor_b.y), material, params)
    }

    pub fn build_wall_ring(
        &mut self,
        center: VoxelCoord,
        radius_vox: u8,
        material: VoxelMaterialId,
        params: CastleToolParams,
    ) -> VoxelBatchResult {
        let radius = radius_vox.max(1) as i32;
        let half_band = (params.wall_thickness_vox.max(1) as f32 * 0.5).max(0.5);
        let mut ring_cells = Vec::new();

        for z in (center.z - radius - params.wall_thickness_vox as i32)
            ..=(center.z + radius + params.wall_thickness_vox as i32)
        {
            for x in (center.x - radius - params.wall_thickness_vox as i32)
                ..=(center.x + radius + params.wall_thickness_vox as i32)
            {
                let dx = (x - center.x) as f32;
                let dz = (z - center.z) as f32;
                let dist = (dx * dx + dz * dz).sqrt();
                if (dist - radius as f32).abs() <= half_band {
                    let angle = dz.atan2(dx);
                    ring_cells.push((angle, x, z));
                }
            }
        }

        ring_cells.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
        let mut spine = Vec::new();
        let mut seen = HashSet::new();
        for (_, x, z) in ring_cells {
            if seen.insert((x, z)) {
                spine.push((x, z));
            }
        }
        if spine.is_empty() {
            return VoxelBatchResult::default();
        }

        self.build_wall_from_spine(spine, center.y, material, params)
    }

    pub fn build_joint_column(
        &mut self,
        anchor: VoxelCoord,
        height_vox: u8,
        radius_vox: u8,
        material: VoxelMaterialId,
    ) -> VoxelBatchResult {
        let height = height_vox.max(1) as i32;
        let radius = radius_vox.max(1) as i32;
        let mut edits = Vec::new();
        self.emit_joint_column_edits(anchor.x, anchor.z, anchor.y, height, radius, material, &mut edits);
        self.apply_voxel_batch(&VoxelEditBatch {
            edits,
            request_support_check: false,
            support_reason: None,
        })
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
            self.bump_revision();
            self.queue_support_recheck(&[hit.coord], SupportReason::Damage);
            self.changed_coords.push(hit.coord);
        }

        result
    }

    pub fn queue_support_recheck(&mut self, changed: &[VoxelCoord], reason: SupportReason) {
        if changed.is_empty() {
            return;
        }
        self.pending_support_coords.extend_from_slice(changed);
        self.pending_support_reason = Some(reason);
        self.kick_support_job_if_idle();
    }

    pub fn poll_support_results(&mut self) -> Option<SupportSolveResult> {
        if self.applied_support_results.is_empty() {
            None
        } else {
            Some(self.applied_support_results.remove(0))
        }
    }

    pub fn rebuild_brick_tree(&mut self) {
        self.brick_tree.rebuild_from_world(&self.world);
    }

    pub fn take_world_change_flag(&mut self) -> bool {
        let changed = self.world_changed_since_sync;
        self.world_changed_since_sync = false;
        changed
    }

    pub fn drain_changed_coords(&mut self) -> Vec<VoxelCoord> {
        std::mem::take(&mut self.changed_coords)
    }

    pub fn drain_render_deltas(&mut self) -> RenderDeltaBatch {
        std::mem::take(&mut self.render_delta)
    }

    pub fn drain_audio_events(&mut self) -> Vec<BuildAudioEvent> {
        std::mem::take(&mut self.audio_events)
    }

    fn build_wall_from_spine(
        &mut self,
        spine: Vec<(i32, i32)>,
        ground_hint_y: i32,
        material: VoxelMaterialId,
        params: CastleToolParams,
    ) -> VoxelBatchResult {
        if spine.is_empty() {
            return VoxelBatchResult::default();
        }
        let wall_height = params.wall_height_vox.max(1) as i32;
        let wall_thickness = params.wall_thickness_vox.max(1) as i32;
        let half = wall_thickness / 2;
        let shell_only = wall_thickness > 2;

        let mut edits = Vec::new();
        for (x, z) in &spine {
            let base_y = self.terrain_conform_y(*x, *z, ground_hint_y);
            for dz in -half..=half {
                for dx in -half..=half {
                    for dy in 0..wall_height {
                        let on_border = dx.abs() == half
                            || dz.abs() == half
                            || dy == 0
                            || dy == wall_height - 1;
                        if shell_only && !on_border {
                            continue;
                        }
                        let flags = if dy == 0 {
                            VOXEL_FLAG_TERRAIN_ANCHORED
                        } else {
                            0
                        };
                        edits.push(VoxelEdit::place(
                            VoxelCoord::new(*x + dx, base_y + dy, *z + dz),
                            material,
                            [128, 128],
                            flags,
                        ));
                    }
                }
            }
        }

        let spacing = params.joint_spacing_vox.max(1) as usize;
        for (idx, (x, z)) in spine.iter().enumerate() {
            if idx == 0 || idx + 1 == spine.len() || idx % spacing == 0 {
                let base_y = self.terrain_conform_y(*x, *z, ground_hint_y);
                self.emit_joint_column_edits(
                    *x,
                    *z,
                    base_y,
                    wall_height,
                    params.joint_radius_vox.max(1) as i32,
                    material,
                    &mut edits,
                );
            }
        }

        let rib_spacing = params.rib_spacing_vox.max(1) as usize;
        for rib_ratio in params.rib_levels {
            let y_off = ((wall_height as f32 - 1.0) * rib_ratio).round() as i32;
            for (idx, (x, z)) in spine.iter().enumerate() {
                if idx % rib_spacing != 0 {
                    continue;
                }
                let base_y = self.terrain_conform_y(*x, *z, ground_hint_y);
                for dz in -half..=half {
                    for dx in -half..=half {
                        edits.push(VoxelEdit::place(
                            VoxelCoord::new(*x + dx, base_y + y_off, *z + dz),
                            material,
                            [128, 128],
                            VOXEL_FLAG_RIB_MEMBER,
                        ));
                    }
                }
            }
        }

        self.apply_voxel_batch(&VoxelEditBatch {
            edits,
            request_support_check: false,
            support_reason: None,
        })
    }

    fn emit_joint_column_edits(
        &self,
        center_x: i32,
        center_z: i32,
        base_y: i32,
        height: i32,
        radius: i32,
        material: VoxelMaterialId,
        out: &mut Vec<VoxelEdit>,
    ) {
        let r2 = radius * radius;
        for y in 0..height {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx * dx + dz * dz > r2 {
                        continue;
                    }
                    out.push(VoxelEdit::place(
                        VoxelCoord::new(center_x + dx, base_y + y, center_z + dz),
                        material,
                        [128, 128],
                        VOXEL_FLAG_RIGID_JOINT,
                    ));
                }
            }
        }
    }

    fn terrain_conform_y(&self, x: i32, z: i32, hint_y: i32) -> i32 {
        let min_y = hint_y - 24;
        let max_y = hint_y + 24;
        for y in (min_y..=max_y).rev() {
            if self.world.get(VoxelCoord::new(x, y, z)).is_some() {
                return y + 1;
            }
        }
        hint_y
    }

    fn bump_revision(&mut self) {
        self.world_revision = self.world_revision.wrapping_add(1).max(1);
    }

    fn pump_support_worker(&mut self) {
        if self.support_worker.is_some() {
            loop {
                let next_event = self
                    .support_worker
                    .as_ref()
                    .and_then(|worker| worker.try_recv());
                let Some(event) = next_event else {
                    break;
                };
                if let WorkerEvent::SupportSolved(result) = event {
                    self.support_job_in_flight = false;
                    if result.revision == self.world_revision {
                        self.apply_support_result(&result);
                        self.applied_support_results.push(result);
                    }
                }
            }
            self.kick_support_job_if_idle();
            return;
        }

        if self.pending_support_coords.is_empty() {
            return;
        }
        let reason = self
            .pending_support_reason
            .take()
            .unwrap_or(SupportReason::ExplicitValidation);
        let coords = std::mem::take(&mut self.pending_support_coords);
        let job = self.build_support_job(&coords, reason);
        let result = solve_support_job_inline(job);
        self.apply_support_result(&result);
        self.applied_support_results.push(result);
    }

    fn kick_support_job_if_idle(&mut self) {
        if self.support_job_in_flight || self.pending_support_coords.is_empty() {
            return;
        }

        if self.support_worker.is_none() {
            return;
        }

        let reason = self
            .pending_support_reason
            .take()
            .unwrap_or(SupportReason::ExplicitValidation);
        let coords = std::mem::take(&mut self.pending_support_coords);
        let job = self.build_support_job(&coords, reason);
        let sent = self
            .support_worker
            .as_ref()
            .is_some_and(|worker| worker.send(WorkerCommand::SupportSolve(job)));
        if sent {
            self.support_job_in_flight = true;
        } else {
            self.support_job_in_flight = false;
        }
    }

    fn build_support_job(&self, changed: &[VoxelCoord], reason: SupportReason) -> SupportSolveJob {
        let mut uniq = BTreeSet::new();
        for coord in changed {
            uniq.insert(*coord);
        }
        let changed_coords: Vec<VoxelCoord> = uniq.into_iter().collect();

        let mut min = IVec3::splat(i32::MAX);
        let mut max = IVec3::splat(i32::MIN);
        for coord in &changed_coords {
            let v = coord.as_ivec3();
            min = min.min(v);
            max = max.max(v);
        }
        if changed_coords.is_empty() {
            min = IVec3::ZERO;
            max = IVec3::ZERO;
        }

        min -= IVec3::splat(SUPPORT_REGION_EXPAND_VOX);
        max += IVec3::splat(SUPPORT_REGION_EXPAND_VOX);

        let in_region = |coord: VoxelCoord| {
            coord.x >= min.x
                && coord.y >= min.y
                && coord.z >= min.z
                && coord.x <= max.x
                && coord.y <= max.y
                && coord.z <= max.z
        };

        let occupied_snapshot = self.world.occupied_cells_snapshot();
        let occupied_region: Vec<(VoxelCoord, u8)> = occupied_snapshot
            .iter()
            .filter_map(|(coord, cell)| in_region(*coord).then_some((*coord, cell.flags)))
            .collect();

        let mut boundary_supported = Vec::new();
        for (coord, _) in &occupied_region {
            for n in neighbors6(*coord) {
                if in_region(n) {
                    continue;
                }
                if self.world.get(n).is_some() {
                    boundary_supported.push(*coord);
                    break;
                }
            }
        }

        let full_world_fallback = if occupied_region.len() > SUPPORT_REGION_CELL_CAP {
            Some(
                occupied_snapshot
                    .iter()
                    .map(|(coord, cell)| (*coord, cell.flags))
                    .collect(),
            )
        } else {
            None
        };

        SupportSolveJob {
            revision: self.world_revision,
            reason,
            changed_coords,
            region_min: min,
            region_max: max,
            occupied_region,
            boundary_supported,
            full_world_fallback,
        }
    }

    fn apply_support_result(&mut self, result: &SupportSolveResult) {
        if result.unsupported.is_empty() {
            return;
        }

        let supported: Vec<VoxelCoord> = result
            .unsupported
            .iter()
            .copied()
            .filter(|coord| self.world.get(*coord).is_some())
            .collect();
        if supported.is_empty() {
            return;
        }

        self.cluster_physics
            .spawn_components(&mut self.world, vec![supported.clone()], &mut self.audio_events);
        for coord in supported {
            self.bake_scheduler.mark_voxel_dirty(coord);
            self.changed_coords.push(coord);
        }
        self.world_changed_since_sync = true;
    }
}

fn nearer_hit(origin: Vec3, a: Option<VoxelHit>, b: Option<VoxelHit>) -> Option<VoxelHit> {
    match (a, b) {
        (Some(ha), Some(hb)) => {
            let da = ha.world_pos.distance_squared(origin);
            let db = hb.world_pos.distance_squared(origin);
            if db < da { Some(hb) } else { Some(ha) }
        }
        (Some(ha), None) => Some(ha),
        (None, Some(hb)) => Some(hb),
        (None, None) => None,
    }
}

fn supercover_line_xz(start: (i32, i32), end: (i32, i32)) -> Vec<(i32, i32)> {
    let dx = (end.0 - start.0).abs();
    let dz = (end.1 - start.1).abs();
    let steps = dx.max(dz).max(1);
    let mut out = Vec::with_capacity((steps + 1) as usize);
    let mut seen = HashSet::new();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let x = (start.0 as f32 + (end.0 - start.0) as f32 * t).round() as i32;
        let z = (start.1 as f32 + (end.1 - start.1) as f32 * t).round() as i32;
        if seen.insert((x, z)) {
            out.push((x, z));
        }
    }
    out
}

fn solve_support_job_inline(job: SupportSolveJob) -> SupportSolveResult {
    let (occupied_cells, used_full_world) = if let Some(full) = job.full_world_fallback.clone() {
        (full, true)
    } else {
        (job.occupied_region.clone(), false)
    };

    let occupied_region: HashSet<VoxelCoord> = occupied_cells.iter().map(|(coord, _)| *coord).collect();
    let anchored_region: HashSet<VoxelCoord> = occupied_cells
        .iter()
        .filter_map(|(coord, flags)| {
            if (*flags & VOXEL_FLAG_TERRAIN_ANCHORED) != 0 || coord.y <= 0 {
                Some(*coord)
            } else {
                None
            }
        })
        .collect();
    let boundary_supported: HashSet<VoxelCoord> = job.boundary_supported.iter().copied().collect();
    let unsupported = unsupported_from_region(&occupied_region, &anchored_region, &boundary_supported);

    SupportSolveResult {
        revision: job.revision,
        reason: Some(job.reason),
        unsupported,
        used_full_world,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_plate_rect_sets_anchored_bottom_layer() {
        let mut runtime = VoxelBuildingRuntime::new();
        let params = CastleToolParams::default();
        let result = runtime.build_base_plate_rect(
            VoxelCoord::new(0, 0, 0),
            VoxelCoord::new(1, 0, 1),
            VoxelMaterialId(2),
            params,
        );
        assert!(result.applied > 0);
        let bottom = runtime.world.get(VoxelCoord::new(0, 0, 0)).copied();
        let top = runtime.world.get(VoxelCoord::new(0, 1, 0)).copied();
        assert!(bottom.is_some());
        assert!(top.is_some());
        assert_ne!(
            bottom.unwrap().flags & VOXEL_FLAG_TERRAIN_ANCHORED,
            0,
            "bottom layer must be terrain anchored"
        );
        assert_eq!(
            top.unwrap().flags & VOXEL_FLAG_TERRAIN_ANCHORED,
            0,
            "upper layer must not be terrain anchored"
        );
    }

    #[test]
    fn wall_line_builds_non_empty_shell() {
        let mut runtime = VoxelBuildingRuntime::new();
        let params = CastleToolParams::default();
        let result = runtime.build_wall_line(
            VoxelCoord::new(0, 0, 0),
            VoxelCoord::new(6, 0, 0),
            VoxelMaterialId(1),
            params,
        );
        assert!(result.applied > 0);
        assert!(
            runtime.world.get(VoxelCoord::new(0, 0, 0)).is_some()
                || runtime.world.get(VoxelCoord::new(0, 1, 0)).is_some()
        );
    }

    #[test]
    fn raycast_voxel_segment_hits_placed_voxel() {
        let mut runtime = VoxelBuildingRuntime::new();
        let _ = runtime.place_voxel(VoxelCoord::new(2, 0, 0), VoxelMaterialId(0));
        let hit = runtime.raycast_voxel_segment(
            Vec3::new(0.1, 0.1, 0.1),
            Vec3::new(2.0, 0.1, 0.1),
            0.0,
        );
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().coord, VoxelCoord::new(2, 0, 0));
    }
}
