//! Building block lifecycle management system.
//!
//! Owns block placement, structural physics, merge workflow, sculpting,
//! and toolbar state — providing a single interface for all building
//! operations with zero GPU coupling.

use glam::Vec3;

use crate::game::builder::{BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, BuildToolbar, SHAPE_NAMES};
use crate::render::{
    BuildingBlock, BuildingBlockManager, BuildingBlockShape, BuildingPhysics, MergeWorkflowManager,
    MergedMesh, SculptingManager,
};

/// Manages the full lifecycle of building blocks.
///
/// Encapsulates placement (with grid/block snapping), periodic structural
/// physics, SDF merge workflows, sculpting, and the build toolbar so that
/// callers interact through a small set of high-level methods.
pub struct BuildingSystem {
    pub block_manager: BuildingBlockManager,
    pub block_physics: BuildingPhysics,
    pub merge_workflow: MergeWorkflowManager,
    pub sculpting: SculptingManager,
    pub toolbar: BuildToolbar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitialSupportKind {
    Terrain,
    Structure,
    None,
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
            merge_workflow: MergeWorkflowManager::new(),
            sculpting: SculptingManager::new(),
            toolbar: BuildToolbar::default(),
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
        if !self.toolbar.visible {
            return None;
        }

        let shape = self.toolbar.get_selected_shape();
        let block = BuildingBlock::new(shape, position, self.toolbar.selected_material);
        let support = self.classify_immediate_support(&block, ground_height_hint);

        if support == InitialSupportKind::None {
            println!("[Build] Placement blocked: no immediate terrain/structure support");
            return None;
        }

        let block_id = self.block_manager.add_block(block);

        match support {
            InitialSupportKind::Terrain => self.block_physics.register_grounded_block(block_id),
            InitialSupportKind::Structure => {
                self.block_physics
                    .register_structurally_supported_block(block_id);
            }
            InitialSupportKind::None => unreachable!("checked above"),
        }

        println!(
            "[Build] Placed {} at ({:.1}, {:.1}, {:.1}) ID={}",
            SHAPE_NAMES[self.toolbar.selected_shape], position.x, position.y, position.z, block_id,
        );

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
                            if hit_offset.x > 0.0 {
                                Vec3::new(aabb.max.x + 0.5, block_center.y, block_center.z)
                            } else {
                                Vec3::new(aabb.min.x - 0.5, block_center.y, block_center.z)
                            }
                        } else if hit_offset.z > 0.0 {
                            Vec3::new(block_center.x, block_center.y, aabb.max.z + 0.5)
                        } else {
                            Vec3::new(block_center.x, block_center.y, aabb.min.z - 0.5)
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

    fn classify_immediate_support(
        &self,
        block: &BuildingBlock,
        ground_height_hint: Option<f32>,
    ) -> InitialSupportKind {
        let aabb = block.aabb();
        let bottom_y = aabb.min.y;

        if let Some(ground_y) = ground_height_hint
            && (bottom_y - ground_y).abs() <= 0.16
        {
            return InitialSupportKind::Terrain;
        }

        for other in self.block_manager.blocks() {
            let other_aabb = other.aabb();
            let top_close =
                other_aabb.max.y >= bottom_y - 0.16 && other_aabb.max.y <= bottom_y + 0.16;
            if !top_close {
                continue;
            }

            let overlap_x =
                aabb.min.x < other_aabb.max.x - 0.02 && aabb.max.x > other_aabb.min.x + 0.02;
            let overlap_z =
                aabb.min.z < other_aabb.max.z - 0.02 && aabb.max.z > other_aabb.min.z + 0.02;
            if overlap_x && overlap_z {
                return InitialSupportKind::Structure;
            }
        }

        InitialSupportKind::None
    }
}
