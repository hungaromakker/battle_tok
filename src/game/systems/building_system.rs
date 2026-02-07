//! Building block lifecycle management system.
//!
//! Owns block placement, structural physics, merge workflow, sculpting,
//! and toolbar state — providing a single interface for all building
//! operations with zero GPU coupling.

use std::collections::{HashMap, HashSet, VecDeque};

use glam::{IVec3, Vec3};

use crate::game::builder::{BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, BuildToolbar, SHAPE_NAMES};
use crate::game::systems::building_v2::{BuildingSystemV2, PlaceError};
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
/// Experimental Forts-like outline mode (disabled by default).
///
/// Strict joint-only gating can block normal stacked templates, so this is
/// opt-in and only rejects isolated non-joint placements.
const ENFORCE_JOINT_OUTLINE_BUILD: bool = false;
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
/// Overload ratio where static blocks are detached into dynamic collapse.
const JOINT_DETACH_THRESHOLD: f32 = 1.08;
/// Hard overload cap used for damage scaling.
const MAX_JOINT_OVERLOAD: f32 = 3.5;
/// Overstress accumulation needed before joint collapse is triggered.
const JOINT_OVERSTRESS_TO_COLLAPSE: f32 = 1.2;
const JOINT_OVERSTRESS_GAIN: f32 = 0.58;
const JOINT_OVERSTRESS_DECAY: f32 = 0.82;
/// Convert material density into gameplay structural load units.
///
/// The runtime physics system still uses full densities for impacts. These
/// values only tune the static integrity solver so normal supported walls do
/// not self-crush under gravity every few frames.
const JOINT_STRUCTURAL_DENSITY_SCALE: f32 = 0.10;
const JOINT_STRUCTURAL_DENSITY_MIN: f32 = 80.0;
const JOINT_STRUCTURAL_DENSITY_MAX: f32 = 360.0;
/// Limit a single collapse event to a local area so one impact cannot erase an
/// entire castle instantly.
const JOINT_COLLAPSE_RADIUS_CELLS: i32 = 5;
const JOINT_COLLAPSE_MAX_BLOCKS: usize = 128;

/// Manages the full lifecycle of building blocks.
///
/// Encapsulates placement (with grid/block snapping), periodic structural
/// physics, SDF merge workflows, sculpting, and the build toolbar so that
/// callers interact through a small set of high-level methods.
pub struct BuildingSystem {
    pub block_manager: BuildingBlockManager,
    pub block_physics: BuildingPhysics,
    pub statics_v2: BuildingSystemV2,
    pub merge_workflow: MergeWorkflowManager,
    pub sculpting: SculptingManager,
    pub toolbar: BuildToolbar,
    damage_accumulated: HashMap<u32, f32>,
    crack_stage: HashMap<u32, u8>,
    joint_overstress: HashMap<u32, f32>,
    dynamic_rubble_ids: HashSet<u32>,
    dynamic_rubble_order: VecDeque<u32>,
    rubble_piles: HashMap<u32, RubblePileNode>,
    rubble_pile_order: VecDeque<u32>,
    next_rubble_pile_id: u32,
    integrity_timer: f32,
    integrity_scan_cursor: usize,
    joint_support_model: JointSupportModel,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitialSupportKind {
    Terrain,
    Structure,
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

#[derive(Debug, Clone, Copy)]
struct JointMaterialProfile {
    compression_n: f32,
    tension_n: f32,
    shear_n: f32,
}

#[derive(Debug, Clone, Copy)]
struct JointStressOutcome {
    block_id: u32,
    overload_ratio: f32,
    overload: f32,
    weight_newtons: f32,
    unsupported: bool,
}

impl BuildingSystem {
    /// Create a new building system.
    ///
    /// `_physics_check_interval` is kept for API compatibility; physics now
    /// advances every frame for stable support/fall behaviour.
    pub fn new(_physics_check_interval: f32) -> Self {
        Self {
            block_manager: BuildingBlockManager::new(),
            block_physics: BuildingPhysics::new(),
            statics_v2: BuildingSystemV2::new(),
            merge_workflow: MergeWorkflowManager::new(),
            sculpting: SculptingManager::new(),
            toolbar: BuildToolbar::default(),
            damage_accumulated: HashMap::new(),
            crack_stage: HashMap::new(),
            joint_overstress: HashMap::new(),
            dynamic_rubble_ids: HashSet::new(),
            dynamic_rubble_order: VecDeque::new(),
            rubble_piles: HashMap::new(),
            rubble_pile_order: VecDeque::new(),
            next_rubble_pile_id: 1,
            integrity_timer: 0.0,
            integrity_scan_cursor: 0,
            joint_support_model: JointSupportModel::Auto,
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
        ground_height_hint: Option<f32>,
    ) -> Option<u32> {
        if !self.toolbar.visible {
            return None;
        }

        let block = BuildingBlock::new(shape, position, material);
        let terrain_anchor = Self::is_terrain_anchor(&block, ground_height_hint);
        let cell = BuildingSystemV2::world_to_cell(position, BLOCK_GRID_SIZE);

        if ENFORCE_JOINT_OUTLINE_BUILD
            && !terrain_anchor
            && !Self::is_joint_shape(shape)
            && !self.has_structural_neighbor(cell)
        {
            if DEBUG_BLOCK_EVENTS {
                println!(
                    "[BuildReject] outline-first requires adjacent support/joint: shape={:?} world=({:.3},{:.3},{:.3}) cell=({}, {}, {})",
                    shape, position.x, position.y, position.z, cell.x, cell.y, cell.z
                );
            }
            return None;
        }

        match self.statics_v2.can_place(cell, terrain_anchor) {
            Ok(()) => {}
            Err(PlaceError::Occupied) | Err(PlaceError::NeedsSupport) => return None,
        }

        let block_id = self.block_manager.add_block(block);
        if let Err(reason) = self
            .statics_v2
            .insert_block(block_id, cell, material, terrain_anchor)
        {
            self.block_manager.remove_block(block_id);
            if VERBOSE_BUILD_LOGS {
                println!("[BuildV2] Placement rollback at {:?}: {:?}", cell, reason);
            }
            return None;
        }

        let support = if terrain_anchor {
            InitialSupportKind::Terrain
        } else {
            InitialSupportKind::Structure
        };

        match support {
            InitialSupportKind::Terrain => self.block_physics.register_grounded_block(block_id),
            InitialSupportKind::Structure => {
                self.block_physics
                    .register_structurally_supported_block(block_id);
            }
        }

        self.damage_accumulated.insert(block_id, 0.0);
        self.crack_stage.insert(block_id, 0);
        self.joint_overstress.insert(block_id, 0.0);

        if DEBUG_BLOCK_EVENTS {
            println!(
                "[BlockPlace] id={} material={} shape={:?} world=({:.3},{:.3},{:.3}) cell=({}, {}, {}) anchor={}",
                block_id,
                material,
                shape,
                position.x,
                position.y,
                position.z,
                cell.x,
                cell.y,
                cell.z,
                terrain_anchor
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
        self.integrity_timer += delta;
        if self.integrity_timer < 0.24 {
            return Vec::new();
        }
        self.integrity_timer = 0.0;

        let blocks = self.block_manager.blocks();
        let total = blocks.len();
        if total == 0 {
            self.integrity_scan_cursor = 0;
            return Vec::new();
        }

        let snapshot: Vec<(u32, Vec3, BuildingBlockShape, u8, IVec3)> = blocks
            .iter()
            .map(|b| {
                (
                    b.id,
                    b.position,
                    b.shape,
                    b.material,
                    BuildingSystemV2::world_to_cell(b.position, BLOCK_GRID_SIZE),
                )
            })
            .collect();
        let by_cell: HashMap<IVec3, u32> = snapshot
            .iter()
            .map(|(id, _, _, _, cell)| (*cell, *id))
            .collect();

        let budget = total.min(24);
        let start = self.integrity_scan_cursor % total;
        self.integrity_scan_cursor = (start + budget) % total;

        let mut pending_damage: Vec<(u32, f32, Vec3)> = Vec::new();
        let mut collapse_roots: HashSet<u32> = HashSet::new();
        for i in 0..budget {
            let idx = (start + i) % total;
            let (block_id, _position, shape, material, cell) = snapshot[idx];
            let Some(stress) =
                self.evaluate_joint_stress(block_id, shape, material, cell, &by_cell)
            else {
                continue;
            };
            let accum = self.update_joint_overstress(stress.block_id, stress.overload);
            if stress.overload_ratio <= 1.0 {
                continue;
            }
            if stress.unsupported
                && stress.overload_ratio >= JOINT_DETACH_THRESHOLD
                && accum >= JOINT_OVERSTRESS_TO_COLLAPSE
            {
                collapse_roots.insert(stress.block_id);
            }
            let damage = Self::material_health(material) * 0.10 * stress.overload;
            let impulse = Vec3::new(0.0, -stress.weight_newtons * 0.0016 * stress.overload, 0.0);
            pending_damage.push((stress.block_id, damage, impulse));
        }

        for root in collapse_roots {
            self.trigger_joint_collapse(root);
        }

        let mut destroyed = Vec::new();
        for (block_id, damage, impulse) in pending_damage {
            let outcome = self.apply_block_damage(block_id, damage, impulse, false);
            if let Some(block) = outcome.destroyed {
                destroyed.push(block);
            }
        }
        destroyed
    }

    /// Re-check structural pressure for a focused set of blocks (typically
    /// blocks affected by an explosion) and apply additional delayed damage.
    pub fn recheck_integrity_for_blocks(&mut self, focus_block_ids: &[u32]) -> Vec<DestroyedBlock> {
        if focus_block_ids.is_empty() {
            return Vec::new();
        }

        let blocks = self.block_manager.blocks();
        if blocks.is_empty() {
            return Vec::new();
        }

        let snapshot: Vec<(u32, Vec3, BuildingBlockShape, u8, IVec3)> = blocks
            .iter()
            .map(|b| {
                (
                    b.id,
                    b.position,
                    b.shape,
                    b.material,
                    BuildingSystemV2::world_to_cell(b.position, BLOCK_GRID_SIZE),
                )
            })
            .collect();

        let by_cell: HashMap<IVec3, u32> = snapshot
            .iter()
            .map(|(id, _, _, _, cell)| (*cell, *id))
            .collect();

        let focus: HashSet<u32> = focus_block_ids.iter().copied().collect();
        let mut target_ids: HashSet<u32> = HashSet::new();

        for (block_id, _position, _shape, _material, cell) in &snapshot {
            if !focus.contains(block_id) {
                continue;
            }
            target_ids.insert(*block_id);

            // Also check direct neighbors so cracks can propagate into weak joints.
            for neighbor in [
                IVec3::new(cell.x + 1, cell.y, cell.z),
                IVec3::new(cell.x - 1, cell.y, cell.z),
                IVec3::new(cell.x, cell.y + 1, cell.z),
                IVec3::new(cell.x, cell.y - 1, cell.z),
                IVec3::new(cell.x, cell.y, cell.z + 1),
                IVec3::new(cell.x, cell.y, cell.z - 1),
            ] {
                if let Some(neighbor_id) = by_cell.get(&neighbor) {
                    target_ids.insert(*neighbor_id);
                }
            }
        }

        let mut pending_damage: Vec<(u32, f32, Vec3)> = Vec::new();
        let mut collapse_roots: HashSet<u32> = HashSet::new();
        for (block_id, _position, shape, material, cell) in snapshot {
            if !target_ids.contains(&block_id) {
                continue;
            }
            let Some(stress) =
                self.evaluate_joint_stress(block_id, shape, material, cell, &by_cell)
            else {
                continue;
            };
            let accum = self.update_joint_overstress(stress.block_id, stress.overload);
            if stress.overload_ratio <= 1.0 {
                continue;
            }
            if stress.unsupported
                && stress.overload_ratio >= JOINT_DETACH_THRESHOLD
                && accum >= JOINT_OVERSTRESS_TO_COLLAPSE
            {
                collapse_roots.insert(stress.block_id);
            }
            let damage = Self::material_health(material) * 0.16 * stress.overload;
            let impulse = Vec3::new(0.0, -stress.weight_newtons * 0.0020 * stress.overload, 0.0);
            pending_damage.push((stress.block_id, damage, impulse));
        }

        for root in collapse_roots {
            self.trigger_joint_collapse(root);
        }

        let mut destroyed = Vec::new();
        for (block_id, damage, impulse) in pending_damage {
            let outcome = self.apply_block_damage(block_id, damage, impulse, false);
            if let Some(block) = outcome.destroyed {
                destroyed.push(block);
            }
        }
        destroyed
    }

    /// Remove a block from managers and v2 statics graph.
    ///
    /// Returns currently unstable block IDs after the removal.
    pub fn remove_block(&mut self, block_id: u32) -> Vec<u32> {
        let Some(existing) = self.block_manager.get_block(block_id).map(|b| (b.position, b.material)) else {
            return Vec::new();
        };
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
            return Vec::new();
        }

        self.block_physics.unregister_block(block_id);
        self.block_manager.remove_block(block_id);
        self.damage_accumulated.remove(&block_id);
        self.crack_stage.remove(&block_id);
        self.joint_overstress.remove(&block_id);

        let unstable = self.statics_v2.remove_block(block_id);
        let detached = self.detach_unstable_static_chain(unstable);
        for unstable_id in &detached {
            if *unstable_id != block_id && self.block_manager.get_block(*unstable_id).is_some() {
                self.block_physics.trigger_fall(*unstable_id);
            }
        }
        detached
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
        if let Err(err) = self.statics_v2.insert_block(block_id, cell, material, true) {
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

    fn support_model_for_material(&self, material: u8) -> JointSupportModel {
        match self.joint_support_model {
            JointSupportModel::Auto => match material {
                1 | 6 => JointSupportModel::Truss,       // wood/moss
                7 | 8 | 9 => JointSupportModel::Frame,   // metal/marble/obsidian
                _ => JointSupportModel::Compression,     // stone-like
            },
            explicit => explicit,
        }
    }

    fn joint_profile_for(&self, material: u8) -> JointMaterialProfile {
        match self.support_model_for_material(material) {
            JointSupportModel::Compression => JointMaterialProfile {
                compression_n: 6200.0,
                tension_n: 2200.0,
                shear_n: 2600.0,
            },
            JointSupportModel::Truss => JointMaterialProfile {
                compression_n: 2600.0,
                tension_n: 3600.0,
                shear_n: 1900.0,
            },
            JointSupportModel::Frame => JointMaterialProfile {
                compression_n: 7600.0,
                tension_n: 7200.0,
                shear_n: 5200.0,
            },
            JointSupportModel::Auto => JointMaterialProfile {
                compression_n: 4800.0,
                tension_n: 3200.0,
                shear_n: 2800.0,
            },
        }
    }

    fn evaluate_joint_stress(
        &self,
        block_id: u32,
        shape: BuildingBlockShape,
        material: u8,
        cell: IVec3,
        by_cell: &HashMap<IVec3, u32>,
    ) -> Option<JointStressOutcome> {
        let state = self.block_physics.get_state(block_id)?;
        if state.terrain_anchored || state.is_loose {
            return None;
        }

        let profile = self.joint_profile_for(material);
        let volume = Self::shape_volume(shape).max(0.05);
        let raw_density = crate::render::building_physics::get_material_density(material);
        let structural_density = (raw_density * JOINT_STRUCTURAL_DENSITY_SCALE)
            .clamp(JOINT_STRUCTURAL_DENSITY_MIN, JOINT_STRUCTURAL_DENSITY_MAX);
        let weight_newtons = volume * structural_density * self.block_physics.config.gravity.abs();

        let below_count = usize::from(by_cell.contains_key(&IVec3::new(cell.x, cell.y - 1, cell.z)));
        let side_count = [
            IVec3::new(cell.x + 1, cell.y, cell.z),
            IVec3::new(cell.x - 1, cell.y, cell.z),
            IVec3::new(cell.x, cell.y, cell.z + 1),
            IVec3::new(cell.x, cell.y, cell.z - 1),
        ]
        .iter()
        .filter(|neighbor| by_cell.contains_key(neighbor))
        .count();

        let above_column = (1..=6)
            .take_while(|step| by_cell.contains_key(&IVec3::new(cell.x, cell.y + *step, cell.z)))
            .count() as f32;
        let column_factor = 1.0 + above_column * 0.42;
        let total_vertical_load = weight_newtons * column_factor;

        let down_support = below_count.max(1) as f32;
        let lateral_ties = side_count as f32;
        let unsupported = below_count == 0;

        let compression_capacity = profile.compression_n * down_support * (1.0 + lateral_ties * 0.18);
        let tie_capacity = profile.tension_n * (0.20 + lateral_ties * 0.34);
        let shear_capacity = profile.shear_n * (down_support * 0.70 + lateral_ties * 0.45).max(0.35);

        let compression_ratio = total_vertical_load / compression_capacity.max(1.0);
        let cantilever_ratio = if unsupported {
            (total_vertical_load * (1.05 + (1.0 - (lateral_ties * 0.25).clamp(0.0, 1.0))))
                / tie_capacity.max(1.0)
        } else {
            0.0
        };
        let impact_ratio = state.peak_impact / (shear_capacity * 1.15).max(1.0);
        let flight_ratio = if state.velocity.y > 0.4 {
            state.velocity.y / 2.8
        } else {
            0.0
        };

        let overload_ratio = compression_ratio
            .max(cantilever_ratio)
            .max(impact_ratio)
            .max(flight_ratio);
        let overload = (overload_ratio - 1.0).clamp(0.0, MAX_JOINT_OVERLOAD);

        Some(JointStressOutcome {
            block_id,
            overload_ratio,
            overload,
            weight_newtons,
            unsupported,
        })
    }

    fn update_joint_overstress(&mut self, block_id: u32, overload: f32) -> f32 {
        let entry = self.joint_overstress.entry(block_id).or_insert(0.0);
        if overload > 0.0 {
            *entry += overload * JOINT_OVERSTRESS_GAIN;
        } else {
            *entry = (*entry * JOINT_OVERSTRESS_DECAY).max(0.0);
            if *entry < 0.02 {
                *entry = 0.0;
            }
        }
        *entry
    }

    fn trigger_joint_collapse(&mut self, root_id: u32) {
        if !self.statics_v2.contains_block_id(root_id) {
            return;
        }

        let Some(root_cell) = self
            .block_manager
            .get_block(root_id)
            .map(|b| BuildingSystemV2::world_to_cell(b.position, BLOCK_GRID_SIZE))
        else {
            return;
        };

        let mut queue = VecDeque::from([root_id]);
        let mut visited: HashSet<u32> = HashSet::new();
        let mut detached: Vec<u32> = Vec::new();

        while let Some(candidate_id) = queue.pop_front() {
            if detached.len() >= JOINT_COLLAPSE_MAX_BLOCKS {
                break;
            }
            if !visited.insert(candidate_id) {
                continue;
            }
            if !self.statics_v2.contains_block_id(candidate_id) {
                continue;
            }
            let Some(candidate_cell) = self
                .block_manager
                .get_block(candidate_id)
                .map(|b| BuildingSystemV2::world_to_cell(b.position, BLOCK_GRID_SIZE))
            else {
                continue;
            };
            let dx = (candidate_cell.x - root_cell.x).abs();
            let dy = (candidate_cell.y - root_cell.y).abs();
            let dz = (candidate_cell.z - root_cell.z).abs();
            if dx.max(dy).max(dz) > JOINT_COLLAPSE_RADIUS_CELLS {
                continue;
            }

            self.statics_v2.detach_block(candidate_id);
            detached.push(candidate_id);

            for neighbor in [
                IVec3::new(candidate_cell.x + 1, candidate_cell.y, candidate_cell.z),
                IVec3::new(candidate_cell.x - 1, candidate_cell.y, candidate_cell.z),
                IVec3::new(candidate_cell.x, candidate_cell.y + 1, candidate_cell.z),
                IVec3::new(candidate_cell.x, candidate_cell.y - 1, candidate_cell.z),
                IVec3::new(candidate_cell.x, candidate_cell.y, candidate_cell.z + 1),
                IVec3::new(candidate_cell.x, candidate_cell.y, candidate_cell.z - 1),
            ] {
                if let Some(neighbor_id) = self.statics_v2.block_id_at_cell(neighbor) {
                    queue.push_back(neighbor_id);
                }
            }
        }

        detached.sort_unstable();
        detached.dedup();

        for block_id in detached {
            if self.block_manager.get_block(block_id).is_none() {
                continue;
            }
            self.block_physics.trigger_fall(block_id);
            self.block_physics.trigger_support_check(block_id);
            self.joint_overstress.insert(block_id, 0.0);
            if DEBUG_BLOCK_EVENTS {
                println!("[JointCollapse] detached block_id={}", block_id);
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
        self.block_physics.unregister_block(block_id);
        self.block_manager.remove_block(block_id);
        self.damage_accumulated.remove(&block_id);
        self.crack_stage.remove(&block_id);
        self.joint_overstress.remove(&block_id);
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

    fn detach_unstable_static_chain(&mut self, initial_unstable: Vec<u32>) -> Vec<u32> {
        if initial_unstable.is_empty() {
            return Vec::new();
        }

        let mut detached = HashSet::new();
        let mut queue = VecDeque::from(initial_unstable);

        while let Some(block_id) = queue.pop_front() {
            if !detached.insert(block_id) {
                continue;
            }
            if !self.statics_v2.contains_block_id(block_id) {
                continue;
            }

            let newly_unstable = self.statics_v2.detach_block(block_id);
            for next in newly_unstable {
                if !detached.contains(&next) {
                    queue.push_back(next);
                }
            }
        }

        detached.into_iter().collect()
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

    fn has_structural_neighbor(&self, cell: IVec3) -> bool {
        const OFFSETS: [IVec3; 6] = [
            IVec3::new(1, 0, 0),
            IVec3::new(-1, 0, 0),
            IVec3::new(0, 1, 0),
            IVec3::new(0, -1, 0),
            IVec3::new(0, 0, 1),
            IVec3::new(0, 0, -1),
        ];
        for offset in OFFSETS {
            let neighbor_cell = cell + offset;
            if self.statics_v2.block_id_at_cell(neighbor_cell).is_some() {
                return true;
            }
        }
        false
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

    fn is_terrain_anchor(block: &BuildingBlock, ground_height_hint: Option<f32>) -> bool {
        let aabb = block.aabb();
        let bottom_y = aabb.min.y;

        if let Some(ground_y) = ground_height_hint
            && (bottom_y - ground_y).abs() <= 0.16
        {
            return true;
        }
        false
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
    fn supported_window_wall_cells_do_not_start_overloaded() {
        let mut system = BuildingSystem::new(0.1);
        system.toolbar.visible = true;

        for x in -2..=2 {
            let placed = system.place_block_shape_with_ground_hint(
                cube_shape(),
                Vec3::new(x as f32, 0.5, 0.0),
                0,
                Some(0.0),
            );
            assert!(placed.is_some(), "base placement failed at x={x}");
        }

        for y in 1..=3 {
            for x in [-2, -1, 1, 2] {
                let placed = system.place_block_shape_with_ground_hint(
                    cube_shape(),
                    Vec3::new(x as f32, 0.5 + y as f32, 0.0),
                    0,
                    None,
                );
                assert!(
                    placed.is_some(),
                    "window-wall placement failed at ({x}, {y})"
                );
            }
        }

        let snapshot: Vec<(u32, BuildingBlockShape, u8, IVec3)> = system
            .block_manager
            .blocks()
            .iter()
            .map(|block| {
                (
                    block.id,
                    block.shape,
                    block.material,
                    BuildingSystemV2::world_to_cell(block.position, BLOCK_GRID_SIZE),
                )
            })
            .collect();
        let by_cell: HashMap<IVec3, u32> = snapshot
            .iter()
            .map(|(id, _, _, cell)| (*cell, *id))
            .collect();

        let inspected_cell = IVec3::new(-1, 2, 0);
        let inspected_id = *by_cell
            .get(&inspected_cell)
            .expect("expected window-wall test block at (-1,2,0)");
        let (shape, material) = snapshot
            .iter()
            .find(|(id, _, _, _)| *id == inspected_id)
            .map(|(_, shape, material, _)| (*shape, *material))
            .expect("expected matching block snapshot");

        let stress = system
            .evaluate_joint_stress(inspected_id, shape, material, inspected_cell, &by_cell)
            .expect("expected stress data for non-anchored block");
        assert!(!stress.unsupported);
        assert!(
            stress.overload_ratio <= 1.0,
            "supported wall block overloaded at placement (ratio={:.3})",
            stress.overload_ratio
        );
    }
}
