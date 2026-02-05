# US-P3-011: Refactor battle_arena.rs to Use BattleScene

## Description
This is the main refactoring story. Rewrite `BattleArenaApp` to use `BattleScene` for all game logic, replacing the dozens of individual state fields and inline logic with delegation to the scene. This is where the 3,880-line monolith starts shrinking dramatically.

## The Core Concept / Why This Matters
Stories 1-10 created all the modular components. This story wires them in. The `BattleArenaApp` struct currently has ~50 fields for game state, and its `update()` method is ~300 lines of inline logic. After this story, `BattleArenaApp` will have: `scene: BattleScene`, GPU resources, and input state. The `update()` method will be: `self.scene.update(delta, &movement, &aiming)` plus buffer uploads.

This is the riskiest story because it touches the entire file. The key principle: **change nothing about behavior, only change where code lives**. The game must play identically after this refactoring.

## Goal
Replace `BattleArenaApp`'s game state fields with `BattleScene` and delegate all game logic to it. Target: reduce from 3,880 lines to ~1,500 lines.

## Files to Create/Modify
- Modify `src/bin/battle_arena.rs` — Major rewrite of struct and methods

## Implementation Steps
1. **Replace state fields in `BattleArenaApp`:**
   ```rust
   struct BattleArenaApp {
       window: Option<Arc<Window>>,

       // Scene (holds ALL game state)
       scene: Option<BattleScene>,

       // GPU resources (stays here - needs wgpu types)
       device: Option<wgpu::Device>,
       queue: Option<wgpu::Queue>,
       surface: Option<wgpu::Surface<'static>>,
       // ... all GPU fields stay

       // Input state (stays here - needs winit types)
       movement: MovementKeys,
       aiming: AimingKeys,
       mouse_pressed: bool,
       left_mouse_pressed: bool,
       current_mouse_pos: Option<(f32, f32)>,

       // Timing (stays here)
       start_time: Instant,
       last_frame: Instant,
       frame_count: u64,
       fps: f32,
   }
   ```

   Remove these fields (now in BattleScene):
   - `player`, `first_person_mode`
   - `cannon`, `projectiles`, `ballistics_config`
   - `hex_wall_grid`, `prisms_destroyed`
   - `falling_prisms`, `debris_particles`
   - `meteors`, `meteor_spawner`
   - `block_manager`, `block_physics`, `merge_workflow`, `_sculpting`
   - `build_toolbar`, `builder_mode`
   - `trees_attacker`, `trees_defender`
   - `terrain_ui`, `terrain_needs_rebuild`
   - `game_state`

2. **Update `initialize()`:**
   - Create `BattleScene::new(ArenaConfig::default(), VisualConfig::default())`
   - Generate initial meshes from `scene.generate_terrain_mesh()` etc.
   - Keep all GPU buffer creation

3. **Update `update()`:**
   - Replace ~300 lines of inline logic with:
     ```rust
     let scene = self.scene.as_mut().unwrap();
     scene.update(delta, &movement_state, &aiming_state);
     ```
   - After scene update, regenerate dynamic meshes for GPU upload
   - Keep GPU buffer write operations

4. **Update `render()`:**
   - Access scene for mesh data: `self.scene.as_ref().unwrap()`
   - Keep all wgpu render pass code (that's Story 13)

5. **Update input handlers:**
   - Route to scene: `scene.building.toolbar_mut()`, `scene.player`, etc.
   - Use `InputConfig` for key classification where possible

6. **Test thoroughly:**
   - `cargo build --bin battle_arena` must succeed
   - Run the game and verify: movement, shooting, building, destruction all work

## Code Patterns
The pattern is delegation:
```rust
// Before (inline):
self.cannon.aim(delta, &self.aiming);
for projectile in &mut self.projectiles { projectile.update(delta); }

// After (delegation):
let scene = self.scene.as_mut().unwrap();
scene.update(delta, &movement, &aiming);
// Scene internally calls cannon.aim() and projectiles.update()
```

## Acceptance Criteria
- [ ] `BattleArenaApp` has `scene: Option<BattleScene>` field
- [ ] All game state removed from `BattleArenaApp` (moved to scene)
- [ ] `update()` delegates to `scene.update()`
- [ ] Game compiles: `cargo build --bin battle_arena`
- [ ] Game runs with same behavior as before
- [ ] File reduced to ~1,500 lines (from 3,880)
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
The game plays exactly the same, but `BattleArenaApp` is dramatically simpler. The `update()` method is ~20 lines instead of ~300. All game logic is in `BattleScene` and its subsystems. The file is under 1,500 lines.

## Dependencies
- Depends on: US-P3-009 (BattleScene), US-P3-010 (InputConfig)
