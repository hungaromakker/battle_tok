//! Building block lifecycle management system.
//!
//! Owns block placement, structural physics, merge workflow, sculpting,
//! and toolbar state — providing a single interface for all building
//! operations with zero GPU coupling.

use glam::Vec3;

use crate::render::{
    BuildingBlock, BuildingBlockManager,
    BuildingPhysics, MergeWorkflowManager, MergedMesh,
    SculptingManager,
};
use crate::game::builder::{
    BuildToolbar, BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, SHAPE_NAMES,
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
    physics_timer: f32,
    physics_check_interval: f32,
}

impl BuildingSystem {
    /// Create a new building system.
    ///
    /// `physics_check_interval` controls how often (in seconds) the
    /// structural integrity check runs.
    pub fn new(physics_check_interval: f32) -> Self {
        Self {
            block_manager: BuildingBlockManager::new(),
            block_physics: BuildingPhysics::new(),
            merge_workflow: MergeWorkflowManager::new(),
            sculpting: SculptingManager::new(),
            toolbar: BuildToolbar::default(),
            physics_timer: 0.0,
            physics_check_interval,
        }
    }

    // ------------------------------------------------------------------
    // Placement
    // ------------------------------------------------------------------

    /// Place a block at `position` using the current toolbar shape and material.
    ///
    /// Returns the new block's ID, or `None` if the toolbar is not visible.
    pub fn place_block(&mut self, position: Vec3) -> Option<u32> {
        if !self.toolbar.visible {
            return None;
        }

        let shape = self.toolbar.get_selected_shape();
        let block = BuildingBlock::new(shape, position, self.toolbar.selected_material);
        let block_id = self.block_manager.add_block(block);

        self.block_physics.register_block(block_id);

        println!(
            "[Build] Placed {} at ({:.1}, {:.1}, {:.1}) ID={}",
            SHAPE_NAMES[self.toolbar.selected_shape],
            position.x,
            position.y,
            position.z,
            block_id,
        );

        Some(block_id)
    }

    /// Calculate where a block should be placed based on a camera ray.
    ///
    /// Steps along the ray looking for terrain or existing-block intersections,
    /// snaps to the block grid, then snaps to nearby blocks if within
    /// [`BLOCK_SNAP_DISTANCE`].
    ///
    /// `terrain_fn(x, z)` returns the ground height at (x, z).
    pub fn calculate_placement(
        &self,
        ray_origin: Vec3,
        ray_dir: Vec3,
        terrain_fn: &dyn Fn(f32, f32) -> f32,
    ) -> Option<Vec3> {
        let max_dist = 50.0;
        let step_size = 0.25;
        let mut t = 1.0;

        while t < max_dist {
            let p = ray_origin + ray_dir * t;
            let ground_height = terrain_fn(p.x, p.z);

            // Terrain hit
            if p.y <= ground_height {
                let snapped_x = (p.x / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                let snapped_z = (p.z / BLOCK_GRID_SIZE).round() * BLOCK_GRID_SIZE;
                let snapped_ground = terrain_fn(snapped_x, snapped_z);

                let y = snapped_ground + 0.5 + self.toolbar.build_height;
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
                        let abs_offset = Vec3::new(
                            hit_offset.x.abs(),
                            hit_offset.y.abs(),
                            hit_offset.z.abs(),
                        );

                        let mut placement_pos =
                            if abs_offset.y > abs_offset.x && abs_offset.y > abs_offset.z {
                                if hit_offset.y > 0.0 {
                                    Vec3::new(block_center.x, aabb.max.y + 0.5, block_center.z)
                                } else {
                                    Vec3::new(block_center.x, aabb.min.y - 0.5, block_center.z)
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
        let ground = terrain_fn(snapped_x, snapped_z);

        Some(Vec3::new(
            snapped_x,
            ground + 0.5 + self.toolbar.build_height,
            snapped_z,
        ))
    }

    // ------------------------------------------------------------------
    // Physics
    // ------------------------------------------------------------------

    /// Advance the structural physics timer and, when the interval elapses,
    /// run a full integrity check.
    ///
    /// Returns the block IDs that were removed due to lost support (so the
    /// destruction system can spawn debris).
    pub fn update_physics(&mut self, delta: f32) -> Vec<u32> {
        self.physics_timer += delta;

        if self.physics_timer >= self.physics_check_interval {
            self.physics_timer -= self.physics_check_interval;

            // Run the full structural update
            self.block_physics.update(delta, &mut self.block_manager);

            // Drain blocks that the physics engine flagged for removal
            self.block_physics.take_blocks_to_remove()
        } else {
            Vec::new()
        }
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
                Vec3::new(block_center.x, aabb.max.y + 0.5, block_center.z),
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
}
