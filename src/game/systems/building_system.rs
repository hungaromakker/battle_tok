//! Building block lifecycle management system.
//!
//! Owns block placement, structural physics, merge workflow, sculpting,
//! and toolbar state — providing a single interface for all building
//! operations with zero GPU coupling.

use std::collections::{HashMap, HashSet};

use glam::{IVec3, Vec3};

use crate::game::builder::{BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, BuildToolbar, SHAPE_NAMES};
use crate::game::systems::building_v2::{BuildingSystemV2, PlaceError};
use crate::render::{
    BuildingBlock, BuildingBlockManager, BuildingBlockShape, BuildingPhysics, MergeWorkflowManager,
    MergedMesh, SculptingManager,
};

/// Enable per-block placement debug logs.
const VERBOSE_BUILD_LOGS: bool = false;

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
    integrity_timer: f32,
    integrity_scan_cursor: usize,
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
    pub destroyed: Option<DestroyedBlock>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitialSupportKind {
    Terrain,
    Structure,
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
            integrity_timer: 0.0,
            integrity_scan_cursor: 0,
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
        self.block_physics.update(delta, &mut self.block_manager);
        self.block_physics.take_blocks_to_remove()
    }

    /// Apply projectile/impact damage to a block using material-based toughness.
    pub fn apply_block_damage(
        &mut self,
        block_id: u32,
        damage: f32,
        impulse: Vec3,
    ) -> DamageOutcome {
        let Some((position, material)) = self
            .block_manager
            .get_block(block_id)
            .map(|b| (b.position, b.material))
        else {
            return DamageOutcome {
                integrity_ratio: 0.0,
                crack_stage: 0,
                crack_stage_advanced: false,
                destroyed: None,
            };
        };

        self.block_physics.apply_impulse(block_id, impulse);
        self.block_physics.trigger_support_check(block_id);

        let max_health = Self::material_health(material);
        let toughness = Self::material_toughness(material);
        let applied = (damage.max(0.0) / toughness).max(0.01);

        let total_damage = {
            let entry = self.damage_accumulated.entry(block_id).or_insert(0.0);
            *entry += applied;
            *entry
        };
        let ratio = (total_damage / max_health).clamp(0.0, 1.0);

        let new_stage = if ratio >= 0.95 {
            3
        } else if ratio >= 0.65 {
            2
        } else if ratio >= 0.35 {
            1
        } else {
            0
        };

        let previous_stage = *self.crack_stage.get(&block_id).unwrap_or(&0);
        let stage_advanced = new_stage > previous_stage;
        if stage_advanced {
            self.crack_stage.insert(block_id, new_stage);
            if VERBOSE_BUILD_LOGS {
                println!(
                    "[BuildDamage] Block {} crack stage {} ({:.0}% integrity used)",
                    block_id,
                    new_stage,
                    ratio * 100.0
                );
            }
        }

        if ratio >= 1.0 {
            self.remove_block(block_id);
            return DamageOutcome {
                integrity_ratio: 1.0,
                crack_stage: 3,
                crack_stage_advanced: stage_advanced,
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
        for i in 0..budget {
            let idx = (start + i) % total;
            let (block_id, _position, shape, material, cell) = snapshot[idx];

            if self
                .block_physics
                .get_state(block_id)
                .is_some_and(|s| s.terrain_anchored)
            {
                continue;
            }

            let below = IVec3::new(cell.x, cell.y - 1, cell.z);
            let below_support = by_cell.contains_key(&below);

            let side_neighbors = [
                IVec3::new(cell.x + 1, cell.y, cell.z),
                IVec3::new(cell.x - 1, cell.y, cell.z),
                IVec3::new(cell.x, cell.y, cell.z + 1),
                IVec3::new(cell.x, cell.y, cell.z - 1),
            ]
            .iter()
            .filter(|neighbor| by_cell.contains_key(neighbor))
            .count() as f32;

            let support_score = (if below_support { 1.0 } else { 0.0 }) + side_neighbors * 0.22;
            let support_score = if support_score > 0.0 {
                support_score
            } else {
                0.22
            };

            let volume = Self::shape_volume(shape).max(0.05);
            let density = crate::render::building_physics::get_material_density(material);
            let weight_newtons = volume * density * self.block_physics.config.gravity.abs();
            let pressure = weight_newtons / support_score;
            let limit = Self::material_load_limit(material);

            if pressure > limit {
                let overload = ((pressure - limit) / limit).clamp(0.02, 3.0);
                let damage = Self::material_health(material) * 0.12 * overload;
                let impulse = Vec3::new(0.0, -weight_newtons * 0.0012 * overload, 0.0);
                pending_damage.push((block_id, damage, impulse));
            }
        }

        let mut destroyed = Vec::new();
        for (block_id, damage, impulse) in pending_damage {
            let outcome = self.apply_block_damage(block_id, damage, impulse);
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
        for (block_id, _position, shape, material, cell) in snapshot {
            if !target_ids.contains(&block_id) {
                continue;
            }

            if self
                .block_physics
                .get_state(block_id)
                .is_some_and(|s| s.terrain_anchored)
            {
                continue;
            }

            let below = IVec3::new(cell.x, cell.y - 1, cell.z);
            let below_support = by_cell.contains_key(&below);

            let side_neighbors = [
                IVec3::new(cell.x + 1, cell.y, cell.z),
                IVec3::new(cell.x - 1, cell.y, cell.z),
                IVec3::new(cell.x, cell.y, cell.z + 1),
                IVec3::new(cell.x, cell.y, cell.z - 1),
            ]
            .iter()
            .filter(|neighbor| by_cell.contains_key(neighbor))
            .count() as f32;

            let support_score = (if below_support { 1.0 } else { 0.0 }) + side_neighbors * 0.22;
            let support_score = support_score.max(0.18);

            let volume = Self::shape_volume(shape).max(0.05);
            let density = crate::render::building_physics::get_material_density(material);
            let weight_newtons = volume * density * self.block_physics.config.gravity.abs();
            let pressure = weight_newtons / support_score;
            let limit = Self::material_load_limit(material);

            if pressure > limit {
                let overload = ((pressure - limit) / limit).clamp(0.05, 3.0);
                // Delayed pressure check should be meaningful, so use higher damage factor.
                let damage = Self::material_health(material) * 0.22 * overload;
                let impulse = Vec3::new(0.0, -weight_newtons * 0.0018 * overload, 0.0);
                pending_damage.push((block_id, damage, impulse));
            }
        }

        let mut destroyed = Vec::new();
        for (block_id, damage, impulse) in pending_damage {
            let outcome = self.apply_block_damage(block_id, damage, impulse);
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
        if self.block_manager.get_block(block_id).is_none() {
            return Vec::new();
        }

        self.block_physics.unregister_block(block_id);
        self.block_manager.remove_block(block_id);
        self.damage_accumulated.remove(&block_id);
        self.crack_stage.remove(&block_id);

        let unstable = self.statics_v2.remove_block(block_id);
        for unstable_id in &unstable {
            if *unstable_id != block_id {
                self.block_physics.trigger_fall(*unstable_id);
            }
        }
        unstable
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

    // ------------------------------------------------------------------
    // Block accessors
    // ------------------------------------------------------------------

    /// Read-only access to the block manager (for collision checks, etc.).
    pub fn blocks(&self) -> &BuildingBlockManager {
        &self.block_manager
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

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

    fn material_load_limit(material: u8) -> f32 {
        crate::render::building_physics::get_break_threshold(material) * 8.0
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
