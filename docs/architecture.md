# Battle Tök Engine Architecture

## Overview

The engine is organized into modular components that separate concerns:

```
engine/src/
├── camera/          # Camera control and raycasting
├── input/           # Input handling and key bindings
├── physics/         # Collision detection, ballistics
├── player/          # Player movement controller
├── render/          # Core rendering infrastructure
│   ├── gpu_context.rs      # GPU resource management
│   ├── render_pass.rs      # Render pass abstraction
│   ├── scene_coordinator.rs # High-level scene management
│   ├── ui_pass.rs          # UI rendering
│   ├── mesh_pass.rs        # 3D mesh rendering
│   └── ... (specialized passes)
└── world/           # World configuration

src/game/            # Game-specific systems
├── config/          # Configuration structs (ArenaConfig, VisualConfig, InputConfig)
├── systems/         # Game logic systems (collision, projectile, destruction, etc.)
├── scenes/          # Scene composition (BattleScene — GPU-agnostic game state)
├── builder/         # Build mode UI, toolbar, and placement tools
├── building/        # Stalberg-style building blocks and mesh generation
├── battle_sphere/   # Battle sphere mechanics
├── economy/         # Resources, day cycle, production chains
├── input/           # Game input actions and aiming state
├── physics/         # Game collision types and support checks
├── player/          # Arena player controller
├── population/      # Villagers, morale, job AI
├── render/          # Game shaders, uniforms, and previews
├── terrain/         # Procedural terrain, floating islands, bridges
└── ui/              # UI components (top bar, toolbar, overlays)
```

## Render System

### Layer Architecture

1. **GpuContext** - Low-level GPU resource management
   - Device, queue, surface configuration
   - Buffer creation helpers
   - Depth texture management

2. **RenderPass** - Trait-based pass abstraction
   - `name()` - Unique identifier
   - `priority()` - Execution order
   - `initialize()` - GPU resource setup
   - `render()` - Frame execution

3. **RenderPassManager** - Coordinates multiple passes
   - Automatic priority sorting
   - Enable/disable individual passes
   - Batch initialization and rendering

4. **SceneCoordinator** - High-level scene management
   - Camera state
   - Frame timing (FPS, delta time)
   - Command buffer submission

### Render Pass Priorities

```rust
enum RenderPassPriority {
    Background = 0,    // Skybox
    Geometry = 100,    // Terrain, meshes
    Translucent = 200, // Particles, alpha objects
    PostProcess = 300, // Fog, tonemapping
    UI = 400,          // UI overlay
}
```

### Available Passes

| Pass | Priority | Description |
|------|----------|-------------|
| ApocalypticSky | Background | Volumetric clouds, lightning |
| MeshRenderPass | Geometry | Terrain, walls, trees |
| ParticleSystem | Translucent | Embers, debris |
| FogPostPass | PostProcess | Depth-based fog |
| UiRenderPass | UI | HUD, overlays |

## Input System

### GameAction Enum

```rust
pub enum GameAction {
    // Movement
    MoveForward, MoveBackward, MoveLeft, MoveRight,
    MoveUp, MoveDown, Sprint, Jump,

    // Building
    ToggleBuildMode, PlaceBlock, RemoveBlock,
    SelectShape1..7, NextShape, PrevShape,

    // Camera
    AimUp, AimDown, AimLeft, AimRight,

    // UI
    ToggleTerrainEditor, ToggleFullscreen,
}
```

### Key Bindings

Default bindings can be customized via `InputState::bind()`:

```rust
let mut input = InputState::new();
input.bind(KeyCode::KeyW, GameAction::MoveForward);
```

## Usage Example

```rust
use battle_tok_engine::render::{
    GpuContext, GpuContextConfig,
    SceneCoordinator, RenderPassManager,
    UiRenderPass, MeshRenderPass,
};
use battle_tok_engine::input::GameInputState;

// Create GPU context
let gpu_config = GpuContextConfig {
    vsync: false,
    high_performance: true,
    ..Default::default()
};
let mut scene = SceneCoordinator::new(window, gpu_config);

// Add render passes
scene.passes_mut().add_pass(Box::new(MeshRenderPass::new()));
scene.passes_mut().add_pass(Box::new(UiRenderPass::new()));
scene.initialize_passes();

// Create input handler
let mut input = GameInputState::new();

// Game loop
loop {
    // Handle input
    input.handle_key(key, pressed);

    // Update
    let delta = scene.update();
    scene.set_camera(position, view, projection);

    // Render
    scene.render()?;

    input.end_frame();
}
```

## Migration Guide

To migrate code from battle_arena.rs to the new component system:

1. **GPU Resources**: Replace manual device/queue management with `GpuContext`
2. **Render Passes**: Extract each render pass into its own struct implementing `RenderPass`
3. **Input**: Use `GameInputState` instead of manual key tracking
4. **Uniforms**: Each pass manages its own uniforms internally

### Before (monolithic)

```rust
struct BattleArenaApp {
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    pipeline: Option<wgpu::RenderPipeline>,
    // ... 50+ fields
}

fn render(&mut self) {
    // 500+ lines of render pass code
}
```

### After (modular)

```rust
struct BattleArenaApp {
    scene: SceneCoordinator,
    input: GameInputState,
    game_state: GameState,
}

fn render(&mut self) {
    self.scene.render()?;
}
```

## Phase 3: Game State Architecture

Phase 3 extracted game logic from the monolithic `battle_arena.rs` into modular, GPU-agnostic components:

### Configuration Layer (`src/game/config/`)

| Struct | Purpose |
|--------|---------|
| `ArenaConfig` | Island geometry, bridge layout, spawn positions |
| `VisualConfig` | Fog, lighting, lava, torch parameters |
| `InputConfig` | All key bindings as data (remappable) |

### Systems Layer (`src/game/systems/`)

Stateless or self-contained game logic with no GPU coupling:

| System | Purpose |
|--------|---------|
| `CollisionSystem` | Player-block and player-hex collision detection |
| `ProjectileSystem` | Ballistic simulation with drag model |
| `DestructionSystem` | Prism destroy → cascade → fall → debris lifecycle |
| `MeteorSystem` | Random meteor spawning and impact |
| `CannonSystem` | Aiming interpolation, fire coordination, mesh-dirty tracking |
| `BuildingSystem` | Block placement, structural physics, merge workflows |

### Scene Layer (`src/game/scenes/`)

| Struct | Purpose |
|--------|---------|
| `BattleScene` | GPU-agnostic game state container; owns all systems, player, terrain, hex grid |

`BattleScene::update()` runs a deterministic 10-step frame update:
1. Player movement → 2. Camera sync → 3. Projectile physics → 4. Projectile-wall collision →
5. Building physics → 6. Destruction update → 7. Meteor update → 8. Player-block collision →
9. Player-hex collision → 10. Economy tick

### Application Layer (`src/bin/battle_arena.rs`)

`BattleArenaApp` is the GPU-coupled shell:
- Holds `scene: Option<BattleScene>` + `gpu: Option<GpuResources>`
- Handles wgpu surface, render passes, buffer uploads
- Translates winit events into scene method calls

## File Locations

| Component | Location |
|-----------|----------|
| GPU Context | `engine/src/render/gpu_context.rs` |
| Render Pass Trait | `engine/src/render/render_pass.rs` |
| Scene Coordinator | `engine/src/render/scene_coordinator.rs` |
| UI Pass | `engine/src/render/ui_pass.rs` |
| Mesh Pass | `engine/src/render/mesh_pass.rs` |
| Input Handler | `engine/src/input/handler.rs` |
| Arena Config | `src/game/config/arena_config.rs` |
| Visual Config | `src/game/config/visual_config.rs` |
| Input Config | `src/game/config/input_config.rs` |
| Battle Scene | `src/game/scenes/battle_scene.rs` |
| Collision System | `src/game/systems/collision_system.rs` |
| Projectile System | `src/game/systems/projectile_system.rs` |
| Destruction System | `src/game/systems/destruction_system.rs` |
| Meteor System | `src/game/systems/meteor_system.rs` |
| Cannon System | `src/game/systems/cannon_system.rs` |
| Building System | `src/game/systems/building_system.rs` |
| GPU Resources | `src/bin/battle_arena.rs` (GpuResources struct) |
