# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build Commands

```bash
# Build all targets
cargo build

# Build release (LTO enabled)
cargo build --release

# Run the main arena game
cargo run --bin battle_arena

# Run the hex planet demo
cargo run --bin hex-planet

# Format code
cargo fmt

# Lint
cargo clippy

# Run tests
cargo test

# Run a specific test
cargo test test_name
```

## Project Architecture

**Battle Tök** is a hexagon-strategy game with Fortnite-inspired building mechanics. Custom SDF (Signed Distance Field) ray marching rendering engine built on wgpu.

### Two-Layer Structure

```
battle_tok/
├── engine/src/       # Core engine library (battle_tok_engine)
│   ├── render/       # wgpu pipeline, SDF baking, froxel culling
│   ├── input/        # Platform-agnostic input (decoupled from winit)
│   ├── camera/       # Camera controllers, raycasting
│   ├── physics/      # Custom ballistics & collision (no external libs)
│   ├── player/       # Movement controllers
│   └── world/        # Grid config, sky settings
│
├── src/game/         # Game-specific systems (injected into engine via path)
│   ├── terrain/      # Procedural hex terrain generation
│   ├── builder/      # Block placement, support physics, tools
│   ├── arena_player/ # First-person player controller
│   ├── arena_cannon/ # Cannon mechanics & ballistics
│   ├── destruction/  # Falling blocks, debris
│   ├── ui/           # Terrain editor, HUD
│   └── render/       # Game uniforms, previews
│
├── src/bin/          # Executables
│   ├── battle_arena.rs   # Main 1v1 arena (primary target)
│   └── hex_planet.rs     # Full planet demo
│
└── shaders/          # WGSL compute & render (19 files)
    ├── raymarcher.wgsl   # Main SDF ray marching
    ├── froxel_*.wgsl     # Froxel culling pipeline
    └── sdf_bake.wgsl     # SDF pre-computation
```

### Module Path Injection Pattern

Game code in `src/game/` is injected into the engine library via path attribute:
```rust
// engine/src/lib.rs
#[path = "../../src/game/mod.rs"]
pub mod game;
```

This allows game-specific code to be part of `battle_tok_engine` while living in the logical `src/game/` directory.

### GPU Data Pattern

All GPU-facing structs use `#[repr(C)]` + `bytemuck::Pod/Zeroable`:
```rust
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
}
```

## Key Systems

### Rendering
- SDF ray marching (equation-based, not mesh-based)
- Froxel culling for efficient SDF queries
- SDF baking for performance
- VSync disabled (`PresentMode::Immediate`) for max FPS

### Physics (Custom, no Rapier)
- Semi-implicit Euler integration for ballistics
- Quadratic drag: `F_drag = -0.5 * ρ * Cd * A * |v|² * v̂`
- Ray-AABB collision via slab method
- **1 unit = 1 meter** (SI units throughout)

### Input
- Platform-agnostic (doesn't depend on winit directly)
- `KeyboardState`, `MouseState`, `FpsMouseState` structs
- Input actions mapped via `InputAction` enum

## Battle Arena Controls

- **WASD**: Move
- **Mouse right-drag**: Look (FPS)
- **Space**: Jump / Fire cannon
- **Shift**: Sprint
- **V**: Toggle first-person ↔ free camera
- **Arrow keys**: Cannon aim (Up/Down: elevation, Left/Right: rotation)
- **B**: Builder mode
- **T**: Terrain editor
- **C**: Clear projectiles
- **F11**: Fullscreen

## Code Standards

- `cargo fmt` before commits
- `cargo clippy` and fix warnings
- Document public APIs
- Constants for all physics/gameplay tuning (module-level `const`)
- Minimal dependencies (6 direct crates)
