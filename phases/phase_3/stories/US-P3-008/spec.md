# US-P3-008: Create BuildingSystem

## Description
Extract building block placement, physics updates, merge workflow, and toolbar management from `battle_arena.rs` into a `BuildingSystem`. The building system is the most feature-rich game system — it handles Fortnite-style block placement with snapping, structural physics, and mesh merging for performance.

## The Core Concept / Why This Matters
The building system is the core gameplay mechanic of Battle Tök. Players build castles to protect their flag and attack enemy territories. Currently the code is spread across: `place_building_block()`, `calculate_block_placement_position()`, `snap_to_nearby_blocks()`, `update_block_preview()`, `handle_block_click()`, `handle_bridge_click()`, and the physics tick. There are 5 state fields in `BattleArenaApp` related to building: `build_toolbar`, `block_manager`, `block_physics`, `merge_workflow`, `_sculpting`. Consolidating these into a single `BuildingSystem` creates a clean interface for the rest of the game to interact with.

## Goal
Create `src/game/systems/building_system.rs` that owns all building-related state and provides placement, physics, and toolbar operations.

## Files to Create/Modify
- Create `src/game/systems/building_system.rs` — BuildingSystem struct
- Modify `src/game/systems/mod.rs` — Add module and re-exports

## Implementation Steps
1. Read the building code in `battle_arena.rs`:
   - `place_building_block()` (~line 1666) — creates block at position
   - `calculate_block_placement_position()` (~line 1510) — raycast to find placement
   - `snap_to_nearby_blocks()` (~line 1623) — magnetic snapping
   - `update_block_preview()` (~line 1657) — ghost block rendering data
   - Building physics tick: uses `BuildingPhysics` for structural checks
   - `MergeWorkflowManager`: merges adjacent blocks for GPU efficiency
   - `BuildToolbar`: UI state for selected block type

2. Create the struct:
   ```rust
   use glam::Vec3;
   use battle_tok_engine::render::{
       BuildingBlockManager, BuildingBlock, BuildingBlockShape,
       BuildingPhysics, MergeWorkflowManager, MergedMesh,
       SculptingManager,
   };
   use crate::game::builder::BuildToolbar;

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
       pub fn new(physics_check_interval: f32) -> Self;

       /// Place a block at the given position with current toolbar selection
       pub fn place_block(&mut self, position: Vec3) -> Option<u32>;

       /// Calculate where a block should be placed based on ray
       pub fn calculate_placement(
           &self,
           ray_origin: Vec3,
           ray_dir: Vec3,
           terrain_fn: &dyn Fn(f32, f32) -> f32,
       ) -> Option<Vec3>;

       /// Update building physics (structural integrity check)
       pub fn update_physics(&mut self, delta: f32) -> Vec<(i32, i32, i32)>;

       /// Get new merged meshes since last call
       pub fn drain_new_merges(&mut self) -> Vec<MergedMesh>;

       /// Access toolbar
       pub fn toolbar(&self) -> &BuildToolbar;
       pub fn toolbar_mut(&mut self) -> &mut BuildToolbar;

       /// Access blocks for collision checking
       pub fn blocks(&self) -> &BuildingBlockManager;
   }
   ```

3. The `update_physics()` method should:
   - Increment physics timer
   - When timer exceeds interval, run structural integrity check
   - Return coordinates of blocks that lost support (for destruction system)

4. Run `cargo check`.

## Code Patterns
The existing `BuildToolbar` in `src/game/builder/toolbar.rs`:
```rust
pub struct BuildToolbar {
    pub selected_shape: BuildingBlockShape,
    pub selected_material: usize,
    pub visible: bool,
    // ...
}
```

## Acceptance Criteria
- [ ] `BuildingSystem` owns all 5 building-related state fields
- [ ] `place_block()` wraps existing placement logic
- [ ] `calculate_placement()` wraps raycast + snap logic
- [ ] `update_physics()` runs periodic structural checks
- [ ] No `wgpu` imports
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
All building state is in one struct. The main game loop calls `building.place_block(pos)` to place, `building.update_physics(delta)` to check structures, and `building.toolbar()` for UI state. The ~180 lines of building code in `battle_arena.rs` consolidate to a few method calls.

## Dependencies
- Depends on: None
