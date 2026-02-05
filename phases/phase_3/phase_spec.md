# Phase 3: Engine Refactoring - Modular Battle Arena

## Problem Statement

`battle_arena.rs` is a **3,880-line monolithic file** containing all game logic, rendering, input handling, and GPU resource management mixed together. This violates separation of concerns and makes the codebase:

1. **Hard to maintain** - A single bug fix requires understanding the entire file
2. **Hard to test** - Logic is tightly coupled to wgpu types
3. **Hard to extend** - Adding features requires modifying the monolith
4. **Inconsistent** - Modular infrastructure exists in `engine/` but isn't used

The engine already has well-designed modular components (`GpuContext`, `RenderPass`, `SceneCoordinator`, `GameInputState`) documented in `docs/architecture.md`, but `battle_arena.rs` doesn't use them. Instead, it has:

- ~750 lines of inline GPU/buffer setup in `initialize()`
- ~500 lines of multi-pass render code in `render()`
- ~300 lines of game state updates in `update()`
- ~528 lines of keyboard/mouse event handling
- Hardcoded constants scattered throughout (island positions, radii, colors, etc.)

**Goal:** Reduce `battle_arena.rs` from 3,880 lines to ~300-500 lines by extracting logic into modular components and using the existing engine infrastructure.

---

## Solution Overview

### Architecture After Refactoring

```
src/bin/battle_arena.rs (~400 lines)
├── main() - Entry point
├── BattleArenaApp - Thin ApplicationHandler
│   ├── scene: SceneCoordinator      ← From engine
│   ├── input: GameInputState        ← From engine
│   ├── game_state: GameState        ← From game module
│   └── arena_config: ArenaConfig    ← NEW: Config system
└── window_event() - Delegates to components

src/game/
├── config/
│   ├── arena_config.rs   # Island positions, sizes, gameplay
│   ├── visual_config.rs  # Colors, fog, lighting
│   └── input_config.rs   # Key bindings
│
├── systems/
│   ├── collision_system.rs    # All collision logic
│   ├── projectile_system.rs   # Projectile updates
│   ├── destruction_system.rs  # Falling prisms, debris
│   ├── cannon_system.rs       # Cannon aiming and firing
│   ├── building_system.rs     # Block placement, physics
│   └── meteor_system.rs       # Meteor spawning, impacts
│
├── components/
│   ├── cannon.rs       # Cannon state + rendering
│   ├── terrain.rs      # Floating island generation
│   ├── trees.rs        # Tree generation + buffers
│   └── sky.rs          # Apocalyptic sky wrapper
│
└── scenes/
    └── battle_scene.rs # High-level scene composition
```

### Key Principles

1. **Configuration over hardcoding** - All magic numbers move to config structs
2. **System-based updates** - Game logic in discrete systems (ECS-lite pattern)
3. **Component composition** - GPU resources encapsulated in components
4. **Thin main file** - `battle_arena.rs` only wires components together

---

## Data Structures / Layouts

### ArenaConfig (NEW)

```rust
// src/game/config/arena_config.rs

#[derive(Clone)]
pub struct ArenaConfig {
    // Island layout
    pub island_attacker: IslandConfig,
    pub island_defender: IslandConfig,
    pub bridge: BridgeConfig,

    // Lava ocean
    pub lava_size: f32,  // 200.0
    pub lava_y: f32,     // -15.0

    // Gameplay
    pub meteor_spawn_interval: f32,  // 2.5 seconds
    pub physics_check_interval: f32, // 5.0 seconds
    pub day_length_seconds: f32,     // 600.0 (10 min)
}

pub struct IslandConfig {
    pub position: Vec3,
    pub radius: f32,
    pub surface_height: f32,
    pub thickness: f32,
}

impl Default for ArenaConfig {
    fn default() -> Self {
        Self {
            island_attacker: IslandConfig {
                position: Vec3::new(-30.0, 10.0, 0.0),
                radius: 30.0,
                surface_height: 5.0,
                thickness: 25.0,
            },
            island_defender: IslandConfig {
                position: Vec3::new(30.0, 10.0, 0.0),
                radius: 30.0,
                surface_height: 5.0,
                thickness: 25.0,
            },
            bridge: BridgeConfig::default(),
            lava_size: 200.0,
            lava_y: -15.0,
            meteor_spawn_interval: 2.5,
            physics_check_interval: 5.0,
            day_length_seconds: 600.0,
        }
    }
}
```

### VisualConfig (NEW)

```rust
// src/game/config/visual_config.rs

pub struct VisualConfig {
    // Sky
    pub sky: ApocalypticSkyConfig,

    // Fog
    pub fog_density: f32,    // 0.008
    pub fog_color: Vec3,     // (0.4, 0.25, 0.35)

    // Lighting
    pub sun_direction: Vec3, // normalized
    pub sun_color: Vec3,     // (1.2, 0.6, 0.35)
    pub ambient: f32,        // 0.15

    // Torches
    pub torch_intensity: f32,
    pub torch_flicker_speed: f32,
}
```

### CollisionSystem (NEW)

```rust
// src/game/systems/collision_system.rs

pub struct CollisionSystem;

impl CollisionSystem {
    /// Check player collision against all building blocks
    pub fn player_block_collision(
        player: &mut Player,
        blocks: &BuildingBlockManager,
    ) -> Vec<CollisionResult> { ... }

    /// Check player collision against hex prism walls
    pub fn player_hex_collision(
        player: &mut Player,
        hex_grid: &HexPrismGrid,
    ) -> Vec<CollisionResult> { ... }

    /// Check projectile collision against walls
    pub fn projectile_wall_collision(
        projectile: &Projectile,
        prev_pos: Vec3,
        hex_grid: &HexPrismGrid,
    ) -> Option<WallHit> { ... }
}
```

### ProjectileSystem (NEW)

```rust
// src/game/systems/projectile_system.rs

pub struct ProjectileSystem {
    pub projectiles: Vec<Projectile>,
    pub config: BallisticsConfig,
}

impl ProjectileSystem {
    pub fn new() -> Self { ... }

    /// Spawn a new projectile from cannon
    pub fn fire(&mut self, cannon: &Cannon) { ... }

    /// Update all projectiles, returns list of wall hits
    pub fn update(&mut self, delta: f32, hex_grid: &HexPrismGrid) -> Vec<WallHit> { ... }

    /// Clear all projectiles
    pub fn clear(&mut self) { ... }

    /// Generate mesh for all active projectiles
    pub fn generate_mesh(&self) -> Mesh { ... }
}
```

### DestructionSystem (NEW)

```rust
// src/game/systems/destruction_system.rs

pub struct DestructionSystem {
    pub falling_prisms: Vec<FallingPrism>,
    pub debris: Vec<DebrisParticle>,
}

impl DestructionSystem {
    /// Destroy a prism and trigger cascade
    pub fn destroy_prism(
        &mut self,
        coord: (i32, i32, i32),
        hex_grid: &mut HexPrismGrid,
    ) { ... }

    /// Update physics for all falling prisms and debris
    pub fn update(&mut self, delta: f32, hex_grid: &mut HexPrismGrid) { ... }

    /// Generate mesh for falling prisms
    pub fn generate_prism_mesh(&self) -> Mesh { ... }

    /// Generate mesh for debris particles
    pub fn generate_debris_mesh(&self) -> Mesh { ... }
}
```

### BattleScene (NEW)

```rust
// src/game/scenes/battle_scene.rs

pub struct BattleScene {
    // Config
    pub config: ArenaConfig,
    pub visuals: VisualConfig,

    // State (non-GPU)
    pub game_state: GameState,
    pub player: Player,
    pub cannon: Cannon,
    pub hex_grid: HexPrismGrid,
    pub block_manager: BuildingBlockManager,
    pub block_physics: BuildingPhysics,

    // Systems
    pub collision: CollisionSystem,
    pub projectiles: ProjectileSystem,
    pub destruction: DestructionSystem,
    pub meteors: MeteorSystem,

    // Trees
    pub trees_attacker: Vec<PlacedTree>,
    pub trees_defender: Vec<PlacedTree>,
}

impl BattleScene {
    /// Create scene from config
    pub fn new(config: ArenaConfig, visuals: VisualConfig) -> Self { ... }

    /// Update all game logic (called from BattleArenaApp::update)
    pub fn update(&mut self, delta: f32, input: &GameInputState) { ... }

    /// Generate all meshes for rendering
    pub fn generate_meshes(&self) -> SceneMeshes { ... }
}
```

---

## What Changes vs What Stays

### Changed Files

| File | What Changes |
|------|-------------|
| `src/bin/battle_arena.rs` | **Major rewrite**: 3880→~400 lines, delegates to scene/systems |
| `src/game/mod.rs` | Add exports for new `config/`, `systems/`, `scenes/` modules |
| `src/game/state.rs` | Remove terrain/builder logic, keep only economy/population state |

### New Files

| File | Purpose |
|------|---------|
| `src/game/config/mod.rs` | Config module root |
| `src/game/config/arena_config.rs` | Arena layout, gameplay constants |
| `src/game/config/visual_config.rs` | Colors, lighting, fog |
| `src/game/config/input_config.rs` | Key binding presets |
| `src/game/systems/mod.rs` | Systems module root |
| `src/game/systems/collision_system.rs` | Player/projectile collision |
| `src/game/systems/projectile_system.rs` | Projectile spawning, updates |
| `src/game/systems/destruction_system.rs` | Prism destruction, debris |
| `src/game/systems/cannon_system.rs` | Cannon aiming, firing |
| `src/game/systems/building_system.rs` | Block placement, physics |
| `src/game/systems/meteor_system.rs` | Meteor spawning, impacts |
| `src/game/scenes/mod.rs` | Scenes module root |
| `src/game/scenes/battle_scene.rs` | High-level scene composition |

### Unchanged Files

| File | Why Unchanged |
|------|--------------|
| `engine/src/render/*.rs` | Already modular, just needs to be used |
| `engine/src/input/*.rs` | Already modular, just needs to be used |
| `src/game/building/*.rs` | Already extracted, no changes needed |
| `src/game/economy/*.rs` | Already extracted, no changes needed |
| `src/game/population/*.rs` | Already extracted, no changes needed |
| `src/game/terrain/*.rs` | Already extracted, no changes needed |
| `src/game/ui/*.rs` | Already extracted, no changes needed |
| `shaders/*.wgsl` | No shader changes in this phase |

---

## Stories

### Story 1: Create Config Module with ArenaConfig

**What:** Create `src/game/config/mod.rs` and `src/game/config/arena_config.rs` with `ArenaConfig` struct that holds all arena layout constants currently hardcoded in `battle_arena.rs`.

**Files:**
- Create `src/game/config/mod.rs`
- Create `src/game/config/arena_config.rs`
- Update `src/game/mod.rs` to export `config`

**Extract these constants from battle_arena.rs:**
- Island positions: `(-30.0, 10.0, 0.0)` and `(30.0, 10.0, 0.0)`
- Island radius: `30.0`
- Island surface height: `5.0`
- Island thickness: `25.0`
- Lava plane size: `200.0`
- Lava Y position: `-15.0`
- Meteor spawn interval: `2.5`
- Physics check interval: `5.0` (from `PHYSICS_CHECK_INTERVAL`)
- Day length: `600.0` seconds

**Acceptance:**
- `ArenaConfig::default()` returns sensible defaults
- `cargo check` passes
- Constants are documented with comments

---

### Story 2: Create VisualConfig

**What:** Create `src/game/config/visual_config.rs` with `VisualConfig` struct for all visual settings.

**Files:**
- Create `src/game/config/visual_config.rs`
- Update `src/game/config/mod.rs`

**Extract these settings from battle_arena.rs:**
- Fog density: `0.008`
- Fog color: `(0.4, 0.25, 0.35)`
- Sun direction (low horizon for rim lighting)
- Sun color: `(1.2, 0.6, 0.35)`
- Ambient: `0.15`
- Torch intensity and flicker settings

**Acceptance:**
- `VisualConfig::default()` matches current hardcoded values
- `cargo check` passes

---

### Story 3: Create CollisionSystem

**What:** Create `src/game/systems/collision_system.rs` extracting collision logic from `battle_arena.rs` methods `player_block_collision()` (lines ~2025-2100) and `player_hex_collision()` (lines ~2100-2160).

**Files:**
- Create `src/game/systems/mod.rs`
- Create `src/game/systems/collision_system.rs`
- Update `src/game/mod.rs`

**Extract functions:**
- `player_block_collision()` → `CollisionSystem::check_player_blocks()`
- `player_hex_collision()` → `CollisionSystem::check_player_hexes()`
- `projectile_wall_collision()` (from update loop) → `CollisionSystem::check_projectile_walls()`

**Signature:**
```rust
impl CollisionSystem {
    pub fn check_player_blocks(
        player: &mut Player,
        blocks: &BuildingBlockManager,
        delta: f32,
    ) -> bool;  // Returns true if collision occurred

    pub fn check_player_hexes(
        player: &mut Player,
        hex_grid: &HexPrismGrid,
    ) -> bool;

    pub fn check_projectile_walls(
        projectile: &Projectile,
        prev_pos: Vec3,
        hex_grid: &HexPrismGrid,
    ) -> Option<(Vec3, (i32, i32, i32))>;  // hit_pos, prism_coord
}
```

**Acceptance:**
- Functions compile standalone
- No wgpu dependencies (pure game logic)
- `cargo check` passes

---

### Story 4: Create ProjectileSystem

**What:** Create `src/game/systems/projectile_system.rs` extracting projectile management from `battle_arena.rs`.

**Files:**
- Create `src/game/systems/projectile_system.rs`
- Update `src/game/systems/mod.rs`

**Extract logic from:**
- `fire_projectile()` method (~lines 1470-1510)
- Projectile update loop in `update()` (~lines 1194-1236)
- `generate_sphere()` calls for projectile meshes

**Struct:**
```rust
pub struct ProjectileSystem {
    projectiles: Vec<Projectile>,
    config: BallisticsConfig,
}

impl ProjectileSystem {
    pub fn new(config: BallisticsConfig) -> Self;
    pub fn fire(&mut self, position: Vec3, direction: Vec3, speed: f32);
    pub fn update(&mut self, delta: f32) -> Vec<(Projectile, ProjectileState)>;
    pub fn clear(&mut self);
    pub fn active_count(&self) -> usize;
    pub fn iter(&self) -> impl Iterator<Item = &Projectile>;
}
```

**Acceptance:**
- Projectile physics logic isolated from rendering
- `cargo check` passes
- No wgpu dependencies

---

### Story 5: Create DestructionSystem

**What:** Create `src/game/systems/destruction_system.rs` extracting destruction logic from `battle_arena.rs`.

**Files:**
- Create `src/game/systems/destruction_system.rs`
- Update `src/game/systems/mod.rs`

**Extract methods:**
- `destroy_prism_with_physics()` (~lines 1261-1278)
- `check_support_cascade()` (~lines 1280-1330)
- `update_falling_prisms()` (~lines 1380-1440)
- `update_debris_particles()` (~lines 1440-1470)

**Struct:**
```rust
pub struct DestructionSystem {
    falling_prisms: Vec<FallingPrism>,
    debris: Vec<DebrisParticle>,
    prisms_destroyed: u32,
}

impl DestructionSystem {
    pub fn new() -> Self;
    pub fn destroy_prism(&mut self, coord: (i32, i32, i32), hex_grid: &mut HexPrismGrid);
    pub fn update(&mut self, delta: f32, hex_grid: &mut HexPrismGrid);
    pub fn falling_prisms(&self) -> &[FallingPrism];
    pub fn debris(&self) -> &[DebrisParticle];
    pub fn total_destroyed(&self) -> u32;
}
```

**Acceptance:**
- All destruction logic extracted
- `cargo check` passes
- No wgpu dependencies

---

### Story 6: Create MeteorSystem

**What:** Create `src/game/systems/meteor_system.rs` extracting meteor logic from `battle_arena.rs`.

**Files:**
- Create `src/game/systems/meteor_system.rs`
- Update `src/game/systems/mod.rs`

**Extract:**
- `MeteorSpawner` usage and `update_meteors()` method
- Meteor impact debris spawning

**Struct:**
```rust
pub struct MeteorSystem {
    meteors: Vec<Meteor>,
    spawner: MeteorSpawner,
}

impl MeteorSystem {
    pub fn new(center: Vec3, radius: f32, spawn_interval: f32) -> Self;
    pub fn update(&mut self, delta: f32) -> Vec<MeteorImpact>;  // Returns impacts for debris
    pub fn iter(&self) -> impl Iterator<Item = &Meteor>;
}
```

**Acceptance:**
- Meteor spawning and physics isolated
- `cargo check` passes

---

### Story 7: Create CannonSystem

**What:** Create `src/game/systems/cannon_system.rs` extracting cannon aiming from `battle_arena.rs`.

**Files:**
- Create `src/game/systems/cannon_system.rs`
- Update `src/game/systems/mod.rs`

**Extract:**
- Cannon aiming logic from update() (~lines 1176-1192)
- `fire_projectile()` integration

**Struct:**
```rust
pub struct CannonSystem {
    cannon: Cannon,
    rotation_speed: f32,
}

impl CannonSystem {
    pub fn new() -> Self;
    pub fn aim(&mut self, input: &AimingState, delta: f32);
    pub fn update(&mut self, delta: f32);  // Smooth interpolation
    pub fn fire(&self) -> (Vec3, Vec3, f32);  // position, direction, speed
    pub fn cannon(&self) -> &Cannon;
}
```

**Acceptance:**
- Cannon logic separated from input handling
- `cargo check` passes

---

### Story 8: Create BuildingSystem

**What:** Create `src/game/systems/building_system.rs` extracting building logic from `battle_arena.rs`.

**Files:**
- Create `src/game/systems/building_system.rs`
- Update `src/game/systems/mod.rs`

**Extract methods:**
- `place_building_block()` (~lines 1666-1693)
- `calculate_block_placement_position()` (~lines 1510-1621)
- `snap_to_nearby_blocks()` (~lines 1623-1654)
- `update_block_preview()` (~lines 1657-1664)
- `update_building_physics()`
- `handle_block_click()` for merge workflow
- `handle_bridge_click()` for bridge mode

**Struct:**
```rust
pub struct BuildingSystem {
    block_manager: BuildingBlockManager,
    block_physics: BuildingPhysics,
    merge_workflow: MergeWorkflowManager,
    toolbar: BuildToolbar,
}

impl BuildingSystem {
    pub fn new() -> Self;
    pub fn place_block(&mut self, position: Vec3) -> Option<u32>;  // Returns block ID
    pub fn calculate_placement(&self, ray_origin: Vec3, ray_dir: Vec3, terrain_fn: impl Fn(f32, f32) -> f32) -> Option<Vec3>;
    pub fn update_physics(&mut self, delta: f32, check_support: bool);
    pub fn handle_click(&mut self, mouse_pos: (f32, f32)) -> bool;  // Returns true if consumed
    pub fn toolbar(&self) -> &BuildToolbar;
    pub fn toolbar_mut(&mut self) -> &mut BuildToolbar;
}
```

**Acceptance:**
- Building logic works standalone
- `cargo check` passes

---

### Story 9: Create BattleScene

**What:** Create `src/game/scenes/battle_scene.rs` that composes all systems into a cohesive scene.

**Files:**
- Create `src/game/scenes/mod.rs`
- Create `src/game/scenes/battle_scene.rs`
- Update `src/game/mod.rs`

**Struct:**
```rust
pub struct BattleScene {
    // Config
    pub config: ArenaConfig,
    pub visuals: VisualConfig,

    // Player state
    pub player: Player,
    pub first_person_mode: bool,

    // Terrain
    pub hex_grid: HexPrismGrid,
    pub trees_attacker: Vec<PlacedTree>,
    pub trees_defender: Vec<PlacedTree>,

    // Systems
    pub collision: CollisionSystem,
    pub projectiles: ProjectileSystem,
    pub destruction: DestructionSystem,
    pub meteors: MeteorSystem,
    pub cannon: CannonSystem,
    pub building: BuildingSystem,

    // Game state
    pub game_state: GameState,
}

impl BattleScene {
    pub fn new(config: ArenaConfig, visuals: VisualConfig) -> Self;

    /// Main update - processes all game logic
    pub fn update(&mut self, delta: f32, movement: &MovementState, aiming: &AimingState);

    /// Generate terrain mesh (call once or on terrain change)
    pub fn generate_terrain_mesh(&self) -> Mesh;

    /// Generate tree mesh
    pub fn generate_tree_mesh(&self) -> Mesh;

    /// Generate dynamic meshes (projectiles, debris, etc)
    pub fn generate_dynamic_mesh(&self) -> Mesh;
}
```

**Acceptance:**
- Scene can be constructed with default configs
- `update()` calls all subsystem updates
- `cargo check` passes

---

### Story 10: Wire InputConfig to Engine Input System

**What:** Create `src/game/config/input_config.rs` and connect it to the engine's `GameInputState`.

**Files:**
- Create `src/game/config/input_config.rs`
- Update `src/game/config/mod.rs`

**Content:**
```rust
use battle_tok_engine::input::{GameInputState, GameAction};
use winit::keyboard::KeyCode;

pub struct InputConfig {
    pub movement: MovementBindings,
    pub building: BuildingBindings,
    pub combat: CombatBindings,
}

impl InputConfig {
    pub fn apply_to(&self, input: &mut GameInputState) {
        // Apply all bindings
        input.bind(self.movement.forward, GameAction::MoveForward);
        // ... etc
    }
}

impl Default for InputConfig {
    fn default() -> Self {
        Self {
            movement: MovementBindings {
                forward: KeyCode::KeyW,
                backward: KeyCode::KeyS,
                left: KeyCode::KeyA,
                right: KeyCode::KeyD,
                jump: KeyCode::Space,
                sprint: KeyCode::ShiftLeft,
            },
            // ... etc
        }
    }
}
```

**Acceptance:**
- All key bindings from `handle_key()` extracted
- `cargo check` passes

---

### Story 11: Refactor battle_arena.rs to Use BattleScene

**What:** Rewrite `BattleArenaApp` to use `BattleScene` and engine components, removing inline game logic.

**Files:**
- Modify `src/bin/battle_arena.rs`

**Changes:**
1. Replace individual state fields with:
   ```rust
   struct BattleArenaApp {
       window: Option<Arc<Window>>,
       scene: Option<BattleScene>,
       // GPU components (still needed for rendering)
       gpu: Option<GpuResources>,
       // Input
       input_config: InputConfig,
       movement: MovementState,
       aiming: AimingState,
   }
   ```

2. Change `initialize()`:
   - Keep GPU setup
   - Create `BattleScene::new(ArenaConfig::default(), VisualConfig::default())`
   - Generate initial meshes from scene

3. Change `update()`:
   - Call `self.scene.update(delta, &self.movement, &self.aiming)`
   - Regenerate dynamic meshes from scene

4. Keep `render()` largely the same (GPU code stays)

**Target:** Reduce from 3,880 lines to ~1,500 lines in this story

**Acceptance:**
- Game still runs and plays the same
- `cargo check` passes
- `cargo build --bin battle_arena` succeeds
- No regression in functionality

---

### Story 12: Extract GPU Resource Management to GpuResources Struct

**What:** Create a `GpuResources` struct in `battle_arena.rs` to group all GPU-related fields.

**Files:**
- Modify `src/bin/battle_arena.rs`

**Extract these fields into `GpuResources`:**
```rust
struct GpuResources {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,

    // Pipelines
    main_pipeline: wgpu::RenderPipeline,
    sdf_cannon_pipeline: wgpu::RenderPipeline,
    ui_pipeline: wgpu::RenderPipeline,

    // Buffers
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    static_vertex_buffer: wgpu::Buffer,
    static_index_buffer: wgpu::Buffer,
    dynamic_vertex_buffer: wgpu::Buffer,
    dynamic_index_buffer: wgpu::Buffer,
    // ... etc

    // Depth
    depth_texture: wgpu::TextureView,
    depth_texture_raw: wgpu::Texture,
}

impl GpuResources {
    fn new(window: Arc<Window>) -> Self { ... }
    fn resize(&mut self, new_size: PhysicalSize<u32>) { ... }
    fn update_static_buffers(&mut self, vertices: &[Vertex], indices: &[u32]) { ... }
    fn update_dynamic_buffers(&mut self, vertices: &[Vertex], indices: &[u32]) { ... }
}
```

**Target:** Move ~400 lines of GPU code into `GpuResources`

**Acceptance:**
- All GPU operations go through `GpuResources`
- `cargo check` passes
- Game renders correctly

---

### Story 13: Extract Render Passes to Methods

**What:** Break the monolithic `render()` method into discrete render pass methods.

**Files:**
- Modify `src/bin/battle_arena.rs`

**Create methods:**
```rust
impl BattleArenaApp {
    fn render(&mut self) {
        let gpu = self.gpu.as_ref().unwrap();
        let scene = self.scene.as_ref().unwrap();

        let output = gpu.surface.get_current_texture().unwrap();
        let view = output.texture.create_view(&Default::default());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        self.render_sky(&mut encoder, &view);
        self.render_terrain(&mut encoder, &view, &gpu.depth_view);
        self.render_dynamic(&mut encoder, &view, &gpu.depth_view);
        self.render_ui(&mut encoder, &view);

        gpu.queue.submit([encoder.finish()]);
        output.present();
    }

    fn render_sky(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) { ... }
    fn render_terrain(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, depth: &wgpu::TextureView) { ... }
    fn render_dynamic(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, depth: &wgpu::TextureView) { ... }
    fn render_ui(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) { ... }
}
```

**Target:** Break ~500 lines of render code into 4-5 smaller methods (~100 lines each)

**Acceptance:**
- Same visual output
- `cargo check` passes
- Render code is more maintainable

---

### Story 14: Final Cleanup and Documentation

**What:** Clean up remaining inline code, add documentation, ensure visual output matches reference image.

**Files:**
- Modify `src/bin/battle_arena.rs`
- Update `docs/architecture.md`

**Tasks:**
1. Remove any remaining duplicated logic
2. Add doc comments to public types
3. Update architecture diagram in docs
4. Verify visual output matches the apocalyptic reference image
5. Run `cargo clippy` and fix warnings
6. Update CLAUDE.md with new module structure

**Acceptance:**
- `battle_arena.rs` is under 500 lines
- `cargo clippy` passes with no warnings
- Game runs with same visuals as before
- Documentation is current

---

## Technical Considerations

### Patterns to Follow

1. **Systems are stateless functions** where possible - take references, return results
2. **Components own their data** - BuildingSystem owns BlockManager, not BattleScene
3. **Config structs are Clone + Default** - easy to create variants
4. **No wgpu in game logic** - keep rendering separate from simulation

### Performance Notes

- Don't regenerate meshes every frame unless data changed
- Use dirty flags for terrain/trees (already exist)
- Systems should be pauseable (for menu, etc)

### Testing Strategy

Systems extracted without wgpu dependencies can be unit tested:
```rust
#[test]
fn test_collision_system() {
    let mut player = Player::default();
    let mut blocks = BuildingBlockManager::new();
    blocks.add_block(BuildingBlock::new(...));

    CollisionSystem::check_player_blocks(&mut player, &blocks, 0.016);
    assert!(player.velocity.y <= 0.0);  // Collision stopped fall
}
```

---

## Non-Goals (This Phase)

- **No ECS framework** - Keep it simple with System structs
- **No networking** - Single-player only
- **No new features** - Pure refactoring, same gameplay
- **No shader changes** - Visual shaders stay as-is
- **No engine changes** - Only use existing engine components

---

## Success Criteria

1. `battle_arena.rs` reduced from 3,880 lines to under 500 lines
2. All game logic in discrete, testable systems
3. Configuration externalized to config structs
4. Game plays identically to before refactoring
5. Visual output matches the apocalyptic reference image
6. `cargo clippy` passes with no warnings
7. Documentation updated to reflect new structure
