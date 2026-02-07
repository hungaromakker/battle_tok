//! Building block lifecycle management system.
//!
//! Owns block placement, structural physics, merge workflow, sculpting,
//! and toolbar state — providing a single interface for all building
//! operations with zero GPU coupling.

use std::collections::{HashMap, HashSet, VecDeque};

use glam::{IVec3, Vec3};

use crate::game::builder::{BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, BuildToolbar, SHAPE_NAMES};
use crate::game::systems::building_v2::BuildingSystemV2;
use crate::game::systems::voxel_building::{
    BuildAudioEvent, CastleToolParams, DamageSource, RenderDeltaBatch, SupportReason,
    SupportSolveResult, VoxelBatchResult, VoxelBuildingRuntime, VoxelCoord, VoxelDamageResult,
    VoxelEditBatch, VoxelHit, VoxelMaterialId, VOXEL_SIZE_METERS,
};
use crate::render::{
    BuildingBlock, BuildingBlockManager, BuildingBlockShape, BuildingPhysics, MergeWorkflowManager,
    MergedMesh, SculptingManager,
};

/// Enable per-block placement debug logs.
const VERBOSE_BUILD_LOGS: bool = false;
/// Runtime debug stream for placement/hit/destruction telemetry.
const DEBUG_BLOCK_EVENTS: bool = true;
/// Crack phase count before full obliteration.
const CRACK_PHASES_BEFORE_OBLITERATE: u8 = 5;
/// Maximum number of simulated fracture rubble blocks kept alive at once.
const MAX_DYNAMIC_RUBBLE_BLOCKS: usize = 192;
/// Maximum number of persistent rubble pile nodes.
const MAX_RUBBLE_PILES: usize = 256;
/// Size divisor for fracture pieces per axis (2 => up to 8 pieces).
const FRACTURE_SUBDIVISIONS: i32 = 2;
/// Minimum/maximum fracture cube half-extent.
const FRACTURE_MIN_HALF_EXTENT: f32 = 0.11;
const FRACTURE_MAX_HALF_EXTENT: f32 = 0.28;
/// Radius used when compacting settled loose cubes into nearby piles.
const RUBBLE_COMPACTION_RADIUS: f32 = 1.25;
/// One full inventory cube per ~unit cube rubble volume.
const RUBBLE_PICKUP_UNIT_VOLUME: f32 = 1.0;

/// Manages the full lifecycle of building blocks.
///
/// Encapsulates placement (with grid/block snapping), periodic structural
/// physics, SDF merge workflows, sculpting, and the build toolbar so that
/// callers interact through a small set of high-level methods.
pub struct BuildingSystem {
    pub voxel_runtime: VoxelBuildingRuntime,
    pub block_manager: BuildingBlockManager,
    pub block_physics: BuildingPhysics,
    pub statics_v2: BuildingSystemV2,
    pub merge_workflow: MergeWorkflowManager,
    pub sculpting: SculptingManager,
    pub toolbar: BuildToolbar,
    damage_accumulated: HashMap<u32, f32>,
    crack_stage: HashMap<u32, u8>,
    joint_overstress: HashMap<u32, f32>,
    joint_blocks: HashSet<u32>,
    dynamic_rubble_ids: HashSet<u32>,
    dynamic_rubble_order: VecDeque<u32>,
    rubble_piles: HashMap<u32, RubblePileNode>,
    rubble_pile_order: VecDeque<u32>,
    next_rubble_pile_id: u32,
    joint_support_model: JointSupportModel,
    voxel_by_block_id: HashMap<u32, VoxelCoord>,
    block_id_by_voxel: HashMap<VoxelCoord, u32>,
    pending_voxel_audio: Vec<BuildAudioEvent>,
}

#[derive(Debug, Clone, Copy)]
pub struct RubblePileNode {
    pub id: u32,
    pub material: u8,
    pub position: Vec3,
    pub velocity_xz: Vec3,
    pub mass_units: f32,
    pub top_radius: f32,
    pub top_height: f32,
    pub pickup_units: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct DestroyedBlock {
    pub id: u32,
    pub position: Vec3,
    pub material: u8,
}

#[derive(Debug, Clone, Copy)]
pub struct DamageOutcome {
    pub integrity_ratio: f32,
    pub crack_stage: u8,
    pub crack_stage_advanced: bool,
    pub fracture_spawned: usize,
    pub destroyed: Option<DestroyedBlock>,
}

/// Forts-inspired 3D joint support mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointSupportModel {
    /// Choose profile from material family.
    Auto,
    /// Compression-heavy masonry behavior (stone-like).
    Compression,
    /// Truss-like behavior (wood-like).
    Truss,
    /// Frame behavior (metal-like).
    Frame,
}

impl BuildingSystem {
    /// Create a new building system.
    ///
    /// `_physics_check_interval` is kept for API compatibility; physics now
    /// advances every frame for stable support/fall behaviour.
    pub fn new(_physics_check_interval: f32) -> Self {
        Self {
            voxel_runtime: VoxelBuildingRuntime::new(),
            block_manager: BuildingBlockManager::new(),
            block_physics: BuildingPhysics::new(),
            statics_v2: BuildingSystemV2::new(),
            merge_workflow: MergeWorkflowManager::new(),
            sculpting: SculptingManager::new(),
            toolbar: BuildToolbar::default(),
            damage_accumulated: HashMap::new(),
            crack_stage: HashMap::new(),
            joint_overstress: HashMap::new(),
            joint_blocks: HashSet::new(),
            dynamic_rubble_ids: HashSet::new(),
            dynamic_rubble_order: VecDeque::new(),
            rubble_piles: HashMap::new(),
            rubble_pile_order: VecDeque::new(),
            next_rubble_pile_id: 1,
            joint_support_model: JointSupportModel::Auto,
            voxel_by_block_id: HashMap::new(),
            block_id_by_voxel: HashMap::new(),
            pending_voxel_audio: Vec::new(),
        }
    }

    // ------------------------------------------------------------------
    // Placement
    // ------------------------------------------------------------------

    /// Place a block at `position` using the current toolbar shape and material.
    ///
    /// Returns the new block's ID, or `None` if the toolbar is not visible.
    pub fn place_block(&mut self, position: Vec3) -> Option<u32> {
        self.place_block_with_ground_hint(position, None)
    }

    /// Place a block and optionally provide a terrain ground height hint.
    ///
    /// If `ground_height_hint` is close to the block's bottom face, the block is
    /// immediately registered as grounded to prevent false "unsupported" falls on
    /// elevated terrain.
    pub fn place_block_with_ground_hint(
        &mut self,
        position: Vec3,
        ground_height_hint: Option<f32>,
    ) -> Option<u32> {
        let shape = self.toolbar.get_selected_shape();
        let material = self.toolbar.selected_material;
        self.place_block_shape_with_ground_hint(shape, position, material, ground_height_hint)
    }

    /// Place a block with explicit shape/material (used by quick-build templates).
    pub fn place_block_shape_with_ground_hint(
        &mut self,
        shape: BuildingBlockShape,
        position: Vec3,
        material: u8,
        _ground_height_hint: Option<f32>,
    ) -> Option<u32> {
        if !self.toolbar.visible {
            return None;
        }

        let block = BuildingBlock::new(shape, position, material);
        let voxel_coord = Self::world_to_voxel_coord(position);
        if self.voxel_runtime.world.get(voxel_coord).is_some() {
            return None;
        }

        let block_id = self.block_manager.add_block(block);
        if !self
            .voxel_runtime
            .place_voxel(voxel_coord, VoxelMaterialId(material))
        {
            self.block_manager.remove_block(block_id);
            return None;
        }

        self.damage_accumulated.insert(block_id, 0.0);
        self.crack_stage.insert(block_id, 0);
        self.joint_overstress.insert(block_id, 0.0);
        if Self::is_joint_shape(shape) {
            self.joint_blocks.insert(block_id);
        }        

        if DEBUG_BLOCK_EVENTS {
            println!(
                "[BlockPlace] id={} material={} shape={:?} world=({:.3},{:.3},{:.3}) voxel=({}, {}, {})",
                block_id,
                material,
                shape,
                position.x,
                position.y,
                position.z,
                voxel_coord.x,
                voxel_coord.y,
                voxel_coord.z,
            );
        }

        if VERBOSE_BUILD_LOGS {
            println!(
                "[Build] Placed {} at ({:.1}, {:.1}, {:.1}) ID={}",
                SHAPE_NAMES[self.toolbar.selected_shape],
                position.x,
                position.y,
                position.z,
                block_id,
            );
        }

        self.voxel_by_block_id.insert(block_id, voxel_coord);
        self.block_id_by_voxel.insert(voxel_coord, block_id);
        self.block_manager.mark_mesh_dirty();

        Some(block_id)
    }

    /// Calculate where a block should be placed based on a camera ray.
    ///
    /// Steps along the ray looking for terrain or existing-block intersections,
    /// snaps to the block grid, then snaps to nearby blocks if within
    /// [`BLOCK_SNAP_DISTANCE`].
    ///
    /// `terrain_fn(x, z)` returns `Some(ground_height)` when valid terrain exists
    /// at (x, z), or `None` outside buildable terrain.
    pub fn calculate_placement(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        terrain_fn: &dyn Fn(f32, f32) -> Option<f32>,
    ) -> Option<Vec3> {
        let selected_shape = self.toolbar.get_selected_shape();
        let (shape_min_y, shape_max_y) = Self::shape_vertical_extents(selected_shape);
        let ground_offset = -shape_min_y;

        let max_dist = 50.0;
        let step_size = 0.25;
        let mut t = 1.0;

        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            let ground_height = terrain_fn(p.x, p.z);

            // Terrain hit
            if let Some(ground_height) = ground_height
                && p.y <= ground_height
            {
                let snapped_x = (p.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                let snapped_z = (p.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                let snapped_ground = terrain_fn(snapped_x, snapped_z).unwrap_or(ground_height);

                let y = snapped_ground + ground_offset + self.toolbar.build_height;
                let position = Vec3::new(snapped_x, y, snapped_z);

                return Some(self.snap_to_nearby_blocks(position));
            }

            // Existing-block hit (for stacking)
            if let Some((block_id, dist)) = self.block_manager.find_closest(p, 1.0) {
                if dist < 0.5 {
                    if let Some(block) = self.block_manager.get_block(block_id) {
                        let aabb = block.aabb();
                        let block_center = (aabb.min + aabb.max) * 0.5;
                        let hit_offset = p - block_center;
                        let abs_offset =
                            Vec3::new(hit_offset.x.abs(), hit_offset.y.abs(), hit_offset.z.abs());

                        let mut placement_pos = if abs_offset.y > abs_offset.x
                            && abs_offset.y > abs_offset.z
                        {
                            if hit_offset.y > 0.0 {
                                Vec3::new(block_center.x, aabb.max.y - shape_min_y, block_center.z)
                            } else {
                                Vec3::new(block_center.x, aabb.min.y - shape_max_y, block_center.z)
                            }
                        } else if abs_offset.x > abs_offset.z {
                            let side_y = aabb.min.y - shape_min_y;
                            if hit_offset.x > 0.0 {
                                Vec3::new(aabb.max.x + 0.5, side_y, block_center.z)
                            } else {
                                Vec3::new(aabb.min.x - 0.5, side_y, block_center.z)
                            }
                        } else {
                            let side_y = aabb.min.y - shape_min_y;
                            if hit_offset.z > 0.0 {
                                Vec3::new(block_center.x, side_y, aabb.max.z + 0.5)
                            } else {
                                Vec3::new(block_center.x, side_y, aabb.min.z - 0.5)
                            }
                        };

                        // Grid-snap XZ
                        placement_pos.x =
                            (placement_pos.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                        placement_pos.z =
                            (placement_pos.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;

                        return Some(placement_pos);
                    }
                }
            }

            t += step_size;
        }

        // Nothing hit — place at a reasonable distance along the ray
        let p = ray_origin + ray_dir * 10.0;
        let snapped_x = (p.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
        let snapped_z = (p.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
        let ground = terrain_fn(snapped_x, snapped_z)?;

        Some(Vec3::new(
            snapped_x,
            ground + ground_offset + self.toolbar.build_height,
            snapped_z,
        ))
    }

    // ------------------------------------------------------------------
    // Physics
    // ------------------------------------------------------------------

    /// Advance structural physics every frame.
    ///
    /// Returns the block IDs that were removed due to lost support (so the
    /// destruction system can spawn debris).
    pub fn update_physics(&mut self, delta: f32) -> Vec<u32> {
        let active_before = self.block_physics.has_active_simulation();
        if !active_before {
            return Vec::new();
        }
        self.block_manager.mark_mesh_dirty();
        self.block_physics.update(delta, &mut self.block_manager);
        // Collision-driven breakage (ground/stack impacts).
        let _ = self.block_physics.check_all_impact_thresholds();
        let removed = self.block_physics.take_blocks_to_remove();
        if !removed.is_empty() {
            self.block_manager.mark_mesh_dirty();
        }
        self.compact_dynamic_rubble_if_over_budget();
        removed
    }

    /// Apply projectile/impact damage to a block using material-based toughness.
    pub fn apply_block_damage(
        &mut self,
        block_id: u32,
        damage: f32,
        impulse: Vec3,
        fracture_on_destroy: bool,
    ) -> DamageOutcome {
        let Some((position, material, shape)) = self
            .block_manager
            .get_block(block_id)
            .map(|b| (b.position, b.material, b.shape))
        else {
            return DamageOutcome {
                integrity_ratio: 0.0,
                crack_stage: 0,
                crack_stage_advanced: false,
                fracture_spawned: 0,
                destroyed: None,
            };
        };

        if let Some(voxel_coord) = self.voxel_by_block_id.get(&block_id).copied() {
            let voxel_result = self.voxel_runtime.apply_damage_at_hit(
                VoxelHit {
                    coord: voxel_coord,
                    world_pos: position,
                    normal: IVec3::ZERO,
                },
                damage,
                impulse,
                DamageSource::Cannonball,
            );
            self.pending_voxel_audio
                .extend(self.voxel_runtime.drain_audio_events());

            let ratio = if let Some(cell) = self.voxel_runtime.world.get(voxel_coord) {
                1.0 - (cell.hp as f32 / cell.max_hp.max(1) as f32)
            } else {
                1.0
            };
            let previous_stage = *self.crack_stage.get(&block_id).unwrap_or(&0);
            let mut new_stage = Self::crack_stage_from_ratio(ratio);
            if voxel_result.destroyed {
                new_stage = CRACK_PHASES_BEFORE_OBLITERATE + 1;
            }
            let stage_advanced = new_stage > previous_stage;
            if stage_advanced {
                self.crack_stage.insert(block_id, new_stage);
            }
            if voxel_result.destroyed {
                self.remove_block(block_id);
                let fracture_spawned = if fracture_on_destroy {
                    self.spawn_fracture_rubble(shape, position, material, impulse)
                } else {
                    0
                };
                return DamageOutcome {
                    integrity_ratio: 1.0,
                    crack_stage: new_stage,
                    crack_stage_advanced: stage_advanced,
                    fracture_spawned,
                    destroyed: Some(DestroyedBlock {
                        id: block_id,
                        position,
                        material,
                    }),
                };
            }

            return DamageOutcome {
                integrity_ratio: ratio.clamp(0.0, 1.0),
                crack_stage: new_stage,
                crack_stage_advanced: stage_advanced,
                fracture_spawned: 0,
                destroyed: None,
            };
        }

        self.block_physics.apply_impulse(block_id, impulse);
        self.block_physics.trigger_support_check(block_id);

        let max_health = Self::material_health(material);
        let toughness = Self::material_toughness(material);
        let applied = (damage.max(0.0) / toughness).max(0.01);

        let previous_stage = *self.crack_stage.get(&block_id).unwrap_or(&0);
        let total_damage = {
            let entry = self.damage_accumulated.entry(block_id).or_insert(0.0);
            *entry += applied;
            *entry
        };
        let ratio = (total_damage / max_health).clamp(0.0, 1.0);

        let mut new_stage = Self::crack_stage_from_ratio(ratio);
        let obliterate = ratio >= 1.0;
        if obliterate {
            new_stage = CRACK_PHASES_BEFORE_OBLITERATE + 1;
        }
        let stage_advanced = new_stage > previous_stage;
        if stage_advanced {
            self.crack_stage.insert(block_id, new_stage);
            if VERBOSE_BUILD_LOGS || DEBUG_BLOCK_EVENTS {
                println!(
                    "[BlockHit] id={} world=({:.3},{:.3},{:.3}) material={} dmg={:.2} ratio={:.3} stage {}->{} impulse=({:.3},{:.3},{:.3})",
                    block_id,
                    position.x,
                    position.y,
                    position.z,
                    material,
                    damage,
                    ratio,
                    previous_stage,
                    new_stage,
                    impulse.x,
                    impulse.y,
                    impulse.z
                );
            }
        } else if DEBUG_BLOCK_EVENTS {
            println!(
                "[BlockHit] id={} world=({:.3},{:.3},{:.3}) material={} dmg={:.2} ratio={:.3} stage={}",
                block_id, position.x, position.y, position.z, material, damage, ratio, new_stage
            );
        }

        if obliterate {
            self.remove_block(block_id);
            let fracture_spawned = if fracture_on_destroy {
                self.spawn_fracture_rubble(shape, position, material, impulse)
            } else {
                0
            };
            if DEBUG_BLOCK_EVENTS {
                println!(
                    "[BlockObliterate] id={} world=({:.3},{:.3},{:.3}) material={} fracture_spawned={}",
                    block_id, position.x, position.y, position.z, material, fracture_spawned
                );
            }
            return DamageOutcome {
                integrity_ratio: 1.0,
                crack_stage: CRACK_PHASES_BEFORE_OBLITERATE + 1,
                crack_stage_advanced: stage_advanced,
                fracture_spawned,
                destroyed: Some(DestroyedBlock {
                    id: block_id,
                    position,
                    material,
                }),
            };
        }

        DamageOutcome {
            integrity_ratio: ratio,
            crack_stage: new_stage,
            crack_stage_advanced: stage_advanced,
            fracture_spawned: 0,
            destroyed: None,
        }
    }

    /// Incremental integrity pass for large structures (budgeted for performance).
    pub fn run_integrity_pass(&mut self, delta: f32) -> Vec<DestroyedBlock> {
        let _ = delta;
        // Hard cutover: passive integrity scans are disabled. Structural work is
        // now event-driven from voxel destruction only.
        Vec::new()
    }

    /// Re-check structural pressure for a focused set of blocks (typically
    /// blocks affected by an explosion) and apply additional delayed damage.
    pub fn recheck_integrity_for_blocks(&mut self, focus_block_ids: &[u32]) -> Vec<DestroyedBlock> {
        let _ = focus_block_ids;
        // Hard cutover: no passive neighborhood integrity recheck in the voxel path.
        Vec::new()
    }

    /// Remove a block from managers and v2 statics graph.
    ///
    /// Returns currently unstable block IDs after the removal.
    pub fn remove_block(&mut self, block_id: u32) -> Vec<u32> {
        let Some(existing) = self.block_manager.get_block(block_id).map(|b| (b.position, b.material)) else {
            return Vec::new();
        };
        if let Some(coord) = self.voxel_by_block_id.remove(&block_id) {
            self.block_id_by_voxel.remove(&coord);
            let _ = self.voxel_runtime.remove_voxel(coord);
        }
        if DEBUG_BLOCK_EVENTS {
            println!(
                "[BlockRemove] id={} world=({:.3},{:.3},{:.3}) material={}",
                block_id, existing.0.x, existing.0.y, existing.0.z, existing.1
            );
        }

        if self.dynamic_rubble_ids.remove(&block_id) {
            self.dynamic_rubble_order.retain(|id| *id != block_id);
            self.block_physics.unregister_block(block_id);
            self.block_manager.remove_block(block_id);
            self.damage_accumulated.remove(&block_id);
            self.crack_stage.remove(&block_id);
            self.joint_overstress.remove(&block_id);
            self.joint_blocks.remove(&block_id);
            return Vec::new();
        }

        self.block_physics.unregister_block(block_id);
        self.block_manager.remove_block(block_id);
        self.damage_accumulated.remove(&block_id);
        self.crack_stage.remove(&block_id);
        self.joint_overstress.remove(&block_id);
        self.joint_blocks.remove(&block_id);
        self.block_manager.mark_mesh_dirty();
        Vec::new()
    }

    /// Register externally-added static geometry blocks (e.g. bridge segments)
    /// into the v2 structural graph.
    pub fn register_external_grounded_block(
        &mut self,
        block_id: u32,
        position: Vec3,
        material: u8,
    ) {
        let cell = BuildingSystemV2::world_to_cell(position, BLOCK_GRID_SIZE);
        if let Err(err) = self
            .statics_v2
            .insert_block(block_id, cell, material, true, false)
        {
            if VERBOSE_BUILD_LOGS {
                println!(
                    "[BuildV2] External block registration failed for ID {} at {:?}: {:?}",
                    block_id, cell, err
                );
            }
        }
        self.damage_accumulated.insert(block_id, 0.0);
        self.crack_stage.insert(block_id, 0);
        self.joint_overstress.insert(block_id, 0.0);
    }

    // ------------------------------------------------------------------
    // Merge workflow
    // ------------------------------------------------------------------

    /// Read-only access to all merged meshes (for GPU buffer creation).
    pub fn merged_meshes(&self) -> &[MergedMesh] {
        self.merge_workflow.merged_meshes()
    }

    // ------------------------------------------------------------------
    // Toolbar accessors
    // ------------------------------------------------------------------

    /// Read-only access to the build toolbar.
    pub fn toolbar(&self) -> &BuildToolbar {
        &self.toolbar
    }

    /// Mutable access to the build toolbar.
    pub fn toolbar_mut(&mut self) -> &mut BuildToolbar {
        &mut self.toolbar
    }

    /// Select the 3D joint support model used by integrity calculations.
    pub fn set_joint_support_model(&mut self, model: JointSupportModel) {
        self.joint_support_model = model;
    }

    /// Current 3D joint support model.
    pub fn joint_support_model(&self) -> JointSupportModel {
        self.joint_support_model
    }

    // ------------------------------------------------------------------
    // Block accessors
    // ------------------------------------------------------------------

    /// Read-only access to the block manager (for collision checks, etc.).
    pub fn blocks(&self) -> &BuildingBlockManager {
        &self.block_manager
    }

    // ------------------------------------------------------------------
    // Voxel-first runtime API (hard-cutover path)
    // ------------------------------------------------------------------

    pub fn tick(&mut self, dt: f32) {
        self.voxel_runtime.tick(dt);
        if self.voxel_runtime.take_world_change_flag() {
            let changed = self.voxel_runtime.drain_changed_coords();
            if !changed.is_empty() {
                self.sync_voxel_proxies_for_coords(&changed);
            }
        }
        self.pending_voxel_audio
            .extend(self.voxel_runtime.drain_audio_events());
    }

    pub fn raycast_voxel(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<VoxelHit> {
        self.voxel_runtime.raycast_voxel(origin, dir, max_dist)
    }

    pub fn raycast_voxel_segment(&self, start: Vec3, end: Vec3, radius: f32) -> Option<VoxelHit> {
        self.voxel_runtime.raycast_voxel_segment(start, end, radius)
    }

    pub fn place_voxel(&mut self, coord: VoxelCoord, material: VoxelMaterialId) -> bool {
        if !self.voxel_runtime.place_voxel(coord, material) {
            return false;
        }
        self.ensure_voxel_proxy(coord, material.0);
        true
    }

    pub fn remove_voxel(&mut self, coord: VoxelCoord) -> bool {
        if !self.voxel_runtime.remove_voxel(coord) {
            return false;
        }
        if let Some(block_id) = self.block_id_by_voxel.remove(&coord) {
            self.voxel_by_block_id.remove(&block_id);
            self.block_manager.remove_block(block_id);
            self.block_physics.unregister_block(block_id);
            self.damage_accumulated.remove(&block_id);
            self.crack_stage.remove(&block_id);
            self.joint_overstress.remove(&block_id);
            self.joint_blocks.remove(&block_id);
            self.block_manager.mark_mesh_dirty();
        }
        true
    }

    pub fn place_corner_brush(
        &mut self,
        anchor: VoxelCoord,
        face_normal: IVec3,
        radius_vox: u8,
        material: VoxelMaterialId,
    ) -> usize {
        let placed =
            self.voxel_runtime
                .place_corner_brush(anchor, face_normal, radius_vox, material);
        if placed == 0 {
            return 0;
        }

        let radius = radius_vox.max(1) as i32;
        for z in -radius..=radius {
            for y in -radius..=radius {
                for x in -radius..=radius {
                    let coord = VoxelCoord::new(anchor.x + x, anchor.y + y, anchor.z + z);
                    if self.voxel_runtime.world.get(coord).is_some() {
                        self.ensure_voxel_proxy(coord, material.0);
                    }
                }
            }
        }
        self.block_manager.mark_mesh_dirty();
        placed
    }

    pub fn apply_voxel_batch(&mut self, batch: &VoxelEditBatch) -> VoxelBatchResult {
        let result = self.voxel_runtime.apply_voxel_batch(batch);
        if !result.changed_coords.is_empty() {
            self.sync_voxel_proxies_for_coords(&result.changed_coords);
            self.block_manager.mark_mesh_dirty();
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
        let result = self
            .voxel_runtime
            .build_base_plate_rect(anchor_a, anchor_b, material, params);
        if !result.changed_coords.is_empty() {
            self.sync_voxel_proxies_for_coords(&result.changed_coords);
            self.block_manager.mark_mesh_dirty();
        }
        result
    }

    pub fn build_base_plate_circle(
        &mut self,
        center: VoxelCoord,
        radius_vox: u8,
        material: VoxelMaterialId,
        params: CastleToolParams,
    ) -> VoxelBatchResult {
        let result = self
            .voxel_runtime
            .build_base_plate_circle(center, radius_vox, material, params);
        if !result.changed_coords.is_empty() {
            self.sync_voxel_proxies_for_coords(&result.changed_coords);
            self.block_manager.mark_mesh_dirty();
        }
        result
    }

    pub fn build_wall_line(
        &mut self,
        anchor_a: VoxelCoord,
        anchor_b: VoxelCoord,
        material: VoxelMaterialId,
        params: CastleToolParams,
    ) -> VoxelBatchResult {
        let result = self
            .voxel_runtime
            .build_wall_line(anchor_a, anchor_b, material, params);
        if !result.changed_coords.is_empty() {
            self.sync_voxel_proxies_for_coords(&result.changed_coords);
            self.block_manager.mark_mesh_dirty();
        }
        result
    }

    pub fn build_wall_ring(
        &mut self,
        center: VoxelCoord,
        radius_vox: u8,
        material: VoxelMaterialId,
        params: CastleToolParams,
    ) -> VoxelBatchResult {
        let result = self
            .voxel_runtime
            .build_wall_ring(center, radius_vox, material, params);
        if !result.changed_coords.is_empty() {
            self.sync_voxel_proxies_for_coords(&result.changed_coords);
            self.block_manager.mark_mesh_dirty();
        }
        result
    }

    pub fn build_joint_column(
        &mut self,
        anchor: VoxelCoord,
        height_vox: u8,
        radius_vox: u8,
        material: VoxelMaterialId,
    ) -> VoxelBatchResult {
        let result = self
            .voxel_runtime
            .build_joint_column(anchor, height_vox, radius_vox, material);
        if !result.changed_coords.is_empty() {
            self.sync_voxel_proxies_for_coords(&result.changed_coords);
            self.block_manager.mark_mesh_dirty();
        }
        result
    }

    pub fn queue_support_recheck(&mut self, changed: &[VoxelCoord], reason: SupportReason) {
        self.voxel_runtime.queue_support_recheck(changed, reason);
    }

    pub fn poll_support_results(&mut self) -> Option<SupportSolveResult> {
        self.voxel_runtime.poll_support_results()
    }

    pub fn apply_damage_at_hit(
        &mut self,
        hit: VoxelHit,
        damage: f32,
        impulse: Vec3,
        source: DamageSource,
    ) -> VoxelDamageResult {
        let result = self
            .voxel_runtime
            .apply_damage_at_hit(hit, damage, impulse, source);
        self.pending_voxel_audio
            .extend(self.voxel_runtime.drain_audio_events());
        if result.destroyed {
            if let Some(block_id) = self.block_id_by_voxel.remove(&hit.coord) {
                self.voxel_by_block_id.remove(&block_id);
                self.block_manager.remove_block(block_id);
                self.block_physics.unregister_block(block_id);
                self.damage_accumulated.remove(&block_id);
                self.crack_stage.remove(&block_id);
                self.joint_overstress.remove(&block_id);
                self.joint_blocks.remove(&block_id);
                self.block_manager.mark_mesh_dirty();
            }
        }
        result
    }

    pub fn drain_render_deltas(&mut self) -> RenderDeltaBatch {
        self.voxel_runtime.drain_render_deltas()
    }

    pub fn drain_audio_events(&mut self) -> Vec<BuildAudioEvent> {
        let mut events: Vec<BuildAudioEvent> = std::mem::take(&mut self.pending_voxel_audio);
        events.extend(self.voxel_runtime.drain_audio_events());
        events
    }

    /// Crack stage for rendering/debug (0..=6).
    pub fn crack_stage_for_block(&self, block_id: u32) -> u8 {
        *self.crack_stage.get(&block_id).unwrap_or(&0)
    }

    /// Update persistent rubble piles (slow lateral settling by material).
    pub fn update_rubble_piles(&mut self, dt: f32) {
        if dt <= 0.0 || self.rubble_piles.is_empty() {
            return;
        }
        let ids: Vec<u32> = self.rubble_piles.keys().copied().collect();
        for id in ids {
            let Some(pile) = self.rubble_piles.get_mut(&id) else {
                continue;
            };
            let mobility = Self::material_rubble_mobility(pile.material);
            let damping = (1.0 - (2.6 - mobility * 1.5) * dt).clamp(0.0, 1.0);
            pile.velocity_xz.x *= damping;
            pile.velocity_xz.z *= damping;
            pile.position.x += pile.velocity_xz.x * dt * mobility;
            pile.position.z += pile.velocity_xz.z * dt * mobility;
            if Vec3::new(pile.velocity_xz.x, 0.0, pile.velocity_xz.z).length() < 0.01 {
                pile.velocity_xz = Vec3::ZERO;
            }
        }
    }

    /// Iterate persistent rubble pile nodes.
    pub fn rubble_piles(&self) -> impl Iterator<Item = &RubblePileNode> {
        self.rubble_piles.values()
    }

    /// Find nearest pile under a query point.
    pub fn find_rubble_pile_near(&self, point: Vec3, radius: f32) -> Option<(u32, f32)> {
        let mut best: Option<(u32, f32)> = None;
        for pile in self.rubble_piles.values() {
            let top_y = pile.position.y + pile.top_height;
            let dx = point.x - pile.position.x;
            let dz = point.z - pile.position.z;
            let horizontal = (dx * dx + dz * dz).sqrt();
            let vertical_ok = point.y >= pile.position.y - 0.15 && point.y <= top_y + 0.35;
            if !vertical_ok || horizontal > pile.top_radius + radius {
                continue;
            }
            let dist = horizontal + (point.y - top_y).abs() * 0.2;
            match best {
                Some((_, best_dist)) if dist >= best_dist => {}
                _ => best = Some((pile.id, dist)),
            }
        }
        best
    }

    /// Hold-pick from a rubble pile into inventory as standard cube blocks.
    pub fn try_pickup_rubble_pile(&mut self, pile_id: u32, max_units: usize) -> usize {
        let Some(snapshot) = self.rubble_piles.get(&pile_id).copied() else {
            return 0;
        };
        if max_units == 0 || self.toolbar.inventory.is_full() {
            return 0;
        }

        let mut picked = 0usize;
        let mut units_remaining = snapshot.pickup_units.max(0.0);
        while picked < max_units && units_remaining >= 1.0 && !self.toolbar.inventory.is_full() {
            let stashed = self.toolbar.inventory.stash(
                BuildingBlockShape::Cube {
                    half_extents: Vec3::splat(0.5),
                },
                snapshot.material,
            );
            if !stashed {
                break;
            }
            picked += 1;
            units_remaining -= 1.0;
        }
        if picked == 0 {
            return 0;
        }

        if let Some(pile) = self.rubble_piles.get_mut(&pile_id) {
            pile.pickup_units = units_remaining.max(0.0);
            pile.mass_units = pile.mass_units.max(pile.pickup_units);
            Self::refresh_rubble_pile_shape(pile);
        }
        if self
            .rubble_piles
            .get(&pile_id)
            .is_some_and(|pile| pile.pickup_units < 0.2)
        {
            self.rubble_piles.remove(&pile_id);
            self.rubble_pile_order.retain(|id| *id != pile_id);
        }
        picked
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn world_to_voxel_coord(position: Vec3) -> VoxelCoord {
        VoxelCoord::new(
            (position.x / VOXEL_SIZE_METERS).floor() as i32,
            (position.y / VOXEL_SIZE_METERS).floor() as i32,
            (position.z / VOXEL_SIZE_METERS).floor() as i32,
        )
    }

    fn voxel_coord_to_world_center(coord: VoxelCoord) -> Vec3 {
        Vec3::new(
            (coord.x as f32 + 0.5) * VOXEL_SIZE_METERS,
            (coord.y as f32 + 0.5) * VOXEL_SIZE_METERS,
            (coord.z as f32 + 0.5) * VOXEL_SIZE_METERS,
        )
    }

    fn ensure_voxel_proxy(&mut self, coord: VoxelCoord, material: u8) -> u32 {
        if let Some(existing_id) = self.block_id_by_voxel.get(&coord).copied() {
            if let Some(block) = self.block_manager.get_block_mut(existing_id) {
                block.material = material;
            }
            return existing_id;
        }

        let pos = Self::voxel_coord_to_world_center(coord);
        let block = BuildingBlock::new(
            BuildingBlockShape::Cube {
                half_extents: Vec3::splat(VOXEL_SIZE_METERS * 0.5),
            },
            pos,
            material,
        );
        let block_id = self.block_manager.add_block(block);
        self.block_physics.register_grounded_block(block_id);
        self.damage_accumulated.insert(block_id, 0.0);
        self.crack_stage.insert(block_id, 0);
        self.joint_overstress.insert(block_id, 0.0);
        self.voxel_by_block_id.insert(block_id, coord);
        self.block_id_by_voxel.insert(coord, block_id);
        self.block_manager.mark_mesh_dirty();
        block_id
    }

    #[allow(dead_code)]
    fn sync_voxel_proxies(&mut self) {
        let mut remove_ids = Vec::new();
        for (block_id, coord) in &self.voxel_by_block_id {
            if self.voxel_runtime.world.get(*coord).is_none() {
                remove_ids.push(*block_id);
            }
        }
        for id in remove_ids {
            if let Some(coord) = self.voxel_by_block_id.remove(&id) {
                self.block_id_by_voxel.remove(&coord);
            }
            self.block_manager.remove_block(id);
            self.block_physics.unregister_block(id);
            self.damage_accumulated.remove(&id);
            self.crack_stage.remove(&id);
            self.joint_overstress.remove(&id);
            self.joint_blocks.remove(&id);
            self.block_manager.mark_mesh_dirty();
        }

        for (coord, cell) in self.voxel_runtime.world.occupied_cells_snapshot() {
            self.ensure_voxel_proxy(coord, cell.material);
        }
    }

    fn sync_voxel_proxies_for_coords(&mut self, changed_coords: &[VoxelCoord]) {
        if changed_coords.is_empty() {
            return;
        }

        for &coord in changed_coords {
            if let Some(cell) = self.voxel_runtime.world.get(coord) {
                self.ensure_voxel_proxy(coord, cell.material);
            } else if let Some(block_id) = self.block_id_by_voxel.remove(&coord) {
                self.voxel_by_block_id.remove(&block_id);
                self.block_manager.remove_block(block_id);
                self.block_physics.unregister_block(block_id);
                self.damage_accumulated.remove(&block_id);
                self.crack_stage.remove(&block_id);
                self.joint_overstress.remove(&block_id);
                self.joint_blocks.remove(&block_id);
                self.block_manager.mark_mesh_dirty();
            }
        }
    }

    fn spawn_fracture_rubble(
        &mut self,
        shape: BuildingBlockShape,
        position: Vec3,
        material: u8,
        source_impulse: Vec3,
    ) -> usize {
        let BuildingBlockShape::Cube { half_extents } = shape else {
            return 0;
        };

        if half_extents.min_element() < FRACTURE_MIN_HALF_EXTENT * 1.35 {
            return 0;
        }

        let subdivisions = FRACTURE_SUBDIVISIONS.max(1);
        let pieces_per_block = (subdivisions * subdivisions * subdivisions) as usize;
        self.ensure_dynamic_rubble_budget(pieces_per_block);

        let mut piece_half = half_extents / subdivisions as f32 * 0.92;
        piece_half = Vec3::new(
            piece_half.x.clamp(FRACTURE_MIN_HALF_EXTENT, FRACTURE_MAX_HALF_EXTENT),
            piece_half.y.clamp(FRACTURE_MIN_HALF_EXTENT, FRACTURE_MAX_HALF_EXTENT),
            piece_half.z.clamp(FRACTURE_MIN_HALF_EXTENT, FRACTURE_MAX_HALF_EXTENT),
        );

        let step = Vec3::new(
            (half_extents.x * 2.0) / subdivisions as f32,
            (half_extents.y * 2.0) / subdivisions as f32,
            (half_extents.z * 2.0) / subdivisions as f32,
        );
        let start = position - half_extents + step * 0.5;
        let density = crate::render::building_physics::get_material_density(material);
        let impulse_len = source_impulse.length();

        let mut spawned = 0usize;
        for ix in 0..subdivisions {
            for iy in 0..subdivisions {
                for iz in 0..subdivisions {
                    if self.dynamic_rubble_ids.len() >= MAX_DYNAMIC_RUBBLE_BLOCKS {
                        return spawned;
                    }

                    let piece_pos = Vec3::new(
                        start.x + ix as f32 * step.x,
                        start.y + iy as f32 * step.y,
                        start.z + iz as f32 * step.z,
                    );

                    let piece = BuildingBlock::new(
                        BuildingBlockShape::Cube {
                            half_extents: piece_half,
                        },
                        piece_pos,
                        material,
                    );
                    let piece_id = self.block_manager.add_block(piece);
                    let voxel_coord = Self::world_to_voxel_coord(piece_pos);
                    let _ = self
                        .voxel_runtime
                        .place_voxel(voxel_coord, VoxelMaterialId(material));
                    self.voxel_by_block_id.insert(piece_id, voxel_coord);
                    self.block_id_by_voxel.insert(voxel_coord, piece_id);

                    let volume =
                        (piece_half.x * 2.0) * (piece_half.y * 2.0) * (piece_half.z * 2.0);
                    self.block_physics.register_block_with_physics(
                        piece_id,
                        material,
                        volume.max(0.01),
                        density,
                    );
                    if let Some(state) = self.block_physics.get_state_mut(piece_id) {
                        state.is_loose = true;
                        state.grounded = false;
                        state.structurally_supported = false;
                        state.terrain_anchored = false;
                    }
                    self.block_physics.trigger_fall(piece_id);

                    let radial = (piece_pos - position).normalize_or_zero();
                    let lateral = if radial.length_squared() > 1e-6 {
                        Vec3::new(radial.x, 0.0, radial.z).normalize_or_zero()
                    } else {
                        Vec3::ZERO
                    };
                    let seed = piece_id as f32 * 0.173;
                    let jitter = Vec3::new(
                        (seed * 12.9898).sin() * 1.2,
                        (seed * 78.233).sin().abs() * 0.9,
                        (seed * 37.719).cos() * 1.2,
                    );
                    let upward_impulse = Vec3::Y * (1.35 + impulse_len * 0.08);
                    let lateral_impulse = lateral * (0.40 + impulse_len * 0.05);
                    let jitter_impulse =
                        Vec3::new(jitter.x * 0.18, jitter.y * 0.12, jitter.z * 0.18);
                    let piece_impulse = source_impulse * 0.12
                        + upward_impulse
                        + lateral_impulse
                        + jitter_impulse;
                    self.block_physics.apply_impulse(piece_id, piece_impulse);

                    self.damage_accumulated.insert(piece_id, 0.0);
                    self.crack_stage.insert(piece_id, 0);
                    self.joint_overstress.insert(piece_id, 0.0);
                    if self.dynamic_rubble_ids.insert(piece_id) {
                        self.dynamic_rubble_order.push_back(piece_id);
                    }

                    spawned += 1;
                }
            }
        }

        spawned
    }

    fn ensure_dynamic_rubble_budget(&mut self, incoming: usize) {
        let mut scan_remaining = self.dynamic_rubble_order.len();
        while self.dynamic_rubble_ids.len().saturating_add(incoming) > MAX_DYNAMIC_RUBBLE_BLOCKS
            && scan_remaining > 0
        {
            let Some(candidate_id) = self.dynamic_rubble_order.pop_front() else {
                break;
            };
            if !self.dynamic_rubble_ids.contains(&candidate_id) {
                scan_remaining -= 1;
                continue;
            }
            if self.block_physics.is_loose_resting(candidate_id) {
                let _ = self.compact_dynamic_rubble_block_to_pile(candidate_id);
            } else {
                self.dynamic_rubble_order.push_back(candidate_id);
            }
            scan_remaining -= 1;
        }

        while self.dynamic_rubble_ids.len().saturating_add(incoming) > MAX_DYNAMIC_RUBBLE_BLOCKS {
            let Some(oldest_id) = self.dynamic_rubble_order.pop_front() else {
                break;
            };
            if !self.dynamic_rubble_ids.contains(&oldest_id) {
                continue;
            }
            if !self.compact_dynamic_rubble_block_to_pile(oldest_id) {
                self.remove_dynamic_rubble_block(oldest_id);
            }
        }
    }

    fn compact_dynamic_rubble_if_over_budget(&mut self) {
        self.ensure_dynamic_rubble_budget(0);
    }

    fn compact_dynamic_rubble_block_to_pile(&mut self, block_id: u32) -> bool {
        let Some((position, material, shape)) = self
            .block_manager
            .get_block(block_id)
            .map(|b| (b.position, b.material, b.shape))
        else {
            return false;
        };
        let units = (Self::shape_volume(shape) / RUBBLE_PICKUP_UNIT_VOLUME).max(0.15);
        let velocity_xz = self
            .block_physics
            .get_state(block_id)
            .map(|s| Vec3::new(s.velocity.x, 0.0, s.velocity.z))
            .unwrap_or(Vec3::ZERO);
        self.remove_dynamic_rubble_block(block_id);
        self.add_units_to_rubble_pile(material, position, units, velocity_xz);
        true
    }

    fn remove_dynamic_rubble_block(&mut self, block_id: u32) {
        if !self.dynamic_rubble_ids.remove(&block_id) {
            return;
        }
        self.dynamic_rubble_order.retain(|id| *id != block_id);
        if let Some(coord) = self.voxel_by_block_id.remove(&block_id) {
            self.block_id_by_voxel.remove(&coord);
            let _ = self.voxel_runtime.remove_voxel(coord);
        }
        self.block_physics.unregister_block(block_id);
        self.block_manager.remove_block(block_id);
        self.damage_accumulated.remove(&block_id);
        self.crack_stage.remove(&block_id);
        self.joint_overstress.remove(&block_id);
        self.joint_blocks.remove(&block_id);
    }

    fn add_units_to_rubble_pile(
        &mut self,
        material: u8,
        position: Vec3,
        units: f32,
        velocity_xz: Vec3,
    ) {
        if units <= 0.0 {
            return;
        }

        let mut target_id: Option<u32> = None;
        let mut best_dist_sq = RUBBLE_COMPACTION_RADIUS * RUBBLE_COMPACTION_RADIUS;
        for pile in self.rubble_piles.values() {
            if pile.material != material {
                continue;
            }
            let d_sq = (pile.position - position).length_squared();
            if d_sq <= best_dist_sq {
                best_dist_sq = d_sq;
                target_id = Some(pile.id);
            }
        }

        if let Some(id) = target_id
            && let Some(pile) = self.rubble_piles.get_mut(&id)
        {
            let total = (pile.mass_units + units).max(0.001);
            pile.position = (pile.position * pile.mass_units + position * units) / total;
            pile.mass_units = total;
            pile.pickup_units += units;
            pile.velocity_xz += velocity_xz * Self::material_rubble_mobility(material) * 0.25;
            Self::refresh_rubble_pile_shape(pile);
            return;
        }

        if self.rubble_piles.len() >= MAX_RUBBLE_PILES {
            if let Some(oldest_id) = self.rubble_pile_order.pop_front() {
                self.rubble_piles.remove(&oldest_id);
            } else {
                return;
            }
        }

        let pile_id = self.next_rubble_pile_id;
        self.next_rubble_pile_id = self.next_rubble_pile_id.wrapping_add(1).max(1);
        let mut pile = RubblePileNode {
            id: pile_id,
            material,
            position,
            velocity_xz,
            mass_units: units,
            top_radius: 0.3,
            top_height: 0.08,
            pickup_units: units,
        };
        Self::refresh_rubble_pile_shape(&mut pile);
        self.rubble_piles.insert(pile.id, pile);
        self.rubble_pile_order.push_back(pile.id);
    }

    fn refresh_rubble_pile_shape(pile: &mut RubblePileNode) {
        let mass = pile.mass_units.max(0.05);
        pile.top_radius = (0.22 + mass.sqrt() * 0.18).clamp(0.20, 1.75);
        pile.top_height = (0.05 + mass * 0.06).clamp(0.06, 0.85);
    }

    fn material_rubble_mobility(material: u8) -> f32 {
        match material {
            1 | 6 => 1.0,               // wood/moss: lightest
            0 | 3 | 4 | 5 => 0.35,      // stone/sandstone/slate/brick
            7 | 8 | 9 => 0.16,          // metal/marble/obsidian
            _ => 0.30,
        }
    }

    /// Snap `position` to an adjacent grid slot of any nearby block if within
    /// [`BLOCK_SNAP_DISTANCE`].
    fn snap_to_nearby_blocks(&self, position: Vec3) -> Vec3 {
        let selected_shape = self.toolbar.get_selected_shape();
        let (shape_min_y, _shape_max_y) = Self::shape_vertical_extents(selected_shape);

        let mut best_pos = position;
        let mut best_dist = BLOCK_SNAP_DISTANCE;

        for block in self.block_manager.blocks() {
            let aabb = block.aabb();
            let block_center = (aabb.min + aabb.max) * 0.5;

            let snap_positions = [
                Vec3::new(block_center.x + BLOCK_GRID_SIZE, position.y, block_center.z),
                Vec3::new(block_center.x - BLOCK_GRID_SIZE, position.y, block_center.z),
                Vec3::new(block_center.x, position.y, block_center.z + BLOCK_GRID_SIZE),
                Vec3::new(block_center.x, position.y, block_center.z - BLOCK_GRID_SIZE),
                Vec3::new(block_center.x, aabb.max.y - shape_min_y, block_center.z),
            ];

            for snap_pos in snap_positions {
                let dist = (snap_pos - position).length();
                if dist < best_dist {
                    best_dist = dist;
                    best_pos = snap_pos;
                }
            }
        }

        best_pos
    }

    fn is_joint_shape(shape: BuildingBlockShape) -> bool {
        matches!(
            shape,
            BuildingBlockShape::Sphere { .. } | BuildingBlockShape::Cylinder { .. }
        )
    }

    fn shape_vertical_extents(shape: BuildingBlockShape) -> (f32, f32) {
        match shape {
            BuildingBlockShape::Cube { half_extents } => (-half_extents.y, half_extents.y),
            BuildingBlockShape::Cylinder { height, .. } => (-height * 0.5, height * 0.5),
            BuildingBlockShape::Sphere { radius } => (-radius, radius),
            BuildingBlockShape::Dome { radius } => (0.0, radius),
            BuildingBlockShape::Arch { height, .. } => (0.0, height),
            BuildingBlockShape::Wedge { size } => (-size.y * 0.5, size.y * 0.5),
        }
    }

    fn material_health(material: u8) -> f32 {
        match material {
            0 => 220.0, // Stone
            1 => 95.0,  // Wood
            2 => 260.0, // Dark stone
            3 => 150.0, // Sandstone
            4 => 180.0, // Slate
            5 => 140.0, // Brick
            6 => 80.0,  // Moss
            7 => 320.0, // Metal
            8 => 210.0, // Marble
            9 => 160.0, // Obsidian
            _ => 170.0,
        }
    }

    fn material_toughness(material: u8) -> f32 {
        match material {
            0 => 1.0,
            1 => 0.70,
            2 => 1.20,
            3 => 0.85,
            4 => 0.95,
            5 => 0.80,
            6 => 0.55,
            7 => 1.45,
            8 => 1.05,
            9 => 0.90,
            _ => 0.95,
        }
    }

    fn crack_stage_from_ratio(ratio: f32) -> u8 {
        if ratio >= 0.80 {
            5
        } else if ratio >= 0.65 {
            4
        } else if ratio >= 0.50 {
            3
        } else if ratio >= 0.35 {
            2
        } else if ratio >= 0.18 {
            1
        } else {
            0
        }
    }

    fn shape_volume(shape: BuildingBlockShape) -> f32 {
        match shape {
            BuildingBlockShape::Cube { half_extents } => {
                (half_extents.x * 2.0) * (half_extents.y * 2.0) * (half_extents.z * 2.0)
            }
            BuildingBlockShape::Cylinder { radius, height } => {
                std::f32::consts::PI * radius * radius * height
            }
            BuildingBlockShape::Sphere { radius } => {
                (4.0 / 3.0) * std::f32::consts::PI * radius * radius * radius
            }
            BuildingBlockShape::Dome { radius } => {
                0.5 * (4.0 / 3.0) * std::f32::consts::PI * radius * radius * radius
            }
            BuildingBlockShape::Arch {
                width,
                height,
                depth,
            } => width * height * depth * 0.55,
            BuildingBlockShape::Wedge { size } => size.x * size.y * size.z * 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube_shape() -> BuildingBlockShape {
        BuildingBlockShape::Cube {
            half_extents: Vec3::splat(0.5),
        }
    }

    #[test]
    fn passive_integrity_pass_is_disabled_after_voxel_cutover() {
        let mut system = BuildingSystem::new(0.1);
        system.toolbar.visible = true;

        for x in -1..=1 {
            let placed = system.place_block_shape_with_ground_hint(
                cube_shape(),
                Vec3::new(x as f32, 0.5, 0.0),
                0,
                Some(0.0),
            );
            let Some(_block_id) = placed else {
                panic!("base placement failed at x={x}");
            };
        }

        let destroyed = system.run_integrity_pass(0.25);
        assert!(
            destroyed.is_empty(),
            "passive integrity pass must be disabled in voxel cutover"
        );
        assert!(
            system
                .damage_accumulated
                .values()
                .all(|damage| damage.abs() <= f32::EPSILON),
            "disabled passive integrity must not apply damage"
        );
    }
}
