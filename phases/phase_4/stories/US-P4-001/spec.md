# US-P4-001: Create battle_editor Binary + AssetEditor Module Skeleton

## Description
Create `src/bin/battle_editor.rs` as a standalone binary with its own winit event loop and wgpu initialization. Create `src/game/asset_editor/mod.rs` with the `AssetEditor` struct, `EditorStage` enum, and `AssetCategory` enum. This is the foundation that all other stories build on.

## The Core Concept / Why This Matters
The asset editor is a **separate binary** from the game — it doesn't touch `battle_arena.rs` at all. This keeps the game binary lean and focused on gameplay while the editor has its own window, event loop, and UI layout optimized for asset creation. The editor shares the engine library and game modules but runs independently. The window title should be "Battle Tök — Asset Editor".

## Goal
Create a runnable `battle_editor` binary that opens a wgpu window with stage switching (keys 1-5) and a dark editor background.

## Files to Create/Modify
- Create `src/bin/battle_editor.rs` — standalone binary: winit event loop, wgpu device/surface init, input routing to AssetEditor, render loop
- Create `src/game/asset_editor/mod.rs` — `AssetEditor` struct, `EditorStage` enum, `AssetCategory`, `AssetDraft` struct, update/render methods
- Modify `src/game/mod.rs` — add `pub mod asset_editor;`
- Modify `Cargo.toml` — add `[[bin]] name = "battle_editor" path = "src/bin/battle_editor.rs"`

## Implementation Steps
1. Add `[[bin]]` entry to `Cargo.toml` for `battle_editor`
2. Create `src/game/asset_editor/mod.rs` with core types:
   - `EditorStage` enum: `Draw2D`, `Extrude`, `Sculpt`, `Color`, `Save`
   - `AssetCategory` enum: `Tree`, `Grass`, `Rock`, `Structure`, `Prop`, `Decoration`
   - `AssetDraft` struct holding outlines, mesh, variety params
   - `AssetEditor` struct with `stage`, `draft`, `active` flag, GPU buffer fields
   - `update()` and `render()` methods (can be stubs initially)
3. Create `src/bin/battle_editor.rs`:
   - Initialize winit with `EventLoop::new()` and `WindowBuilder::new().with_title("Battle Tök — Asset Editor")`
   - Initialize wgpu: instance, surface, adapter, device, queue (follow `battle_arena.rs` pattern)
   - Create `AssetEditor::new()`
   - Event loop: handle `WindowEvent::KeyboardInput` to route keys 1-5 to stage switching
   - Render loop: clear to dark gray background (`0.12, 0.12, 0.14`), render stage name text
4. Add `pub mod asset_editor;` to `src/game/mod.rs`

## Code Patterns
Follow the existing `battle_arena.rs` binary pattern for wgpu initialization:
```rust
use winit::{event_loop::EventLoop, window::WindowBuilder};
use wgpu;
// Create surface, adapter, device, queue
// Main event loop with RedrawRequested
```

The `AssetEditor` struct follows the `GameState` pattern from `src/game/state.rs`.

## Acceptance Criteria
- [ ] `cargo check --bin battle_editor` compiles with 0 errors
- [ ] `cargo check --bin battle_arena` still compiles (no changes to it)
- [ ] `battle_editor` binary can be built and run, opening a window titled "Battle Tök — Asset Editor"
- [ ] Keys 1-5 switch between editor stages (visible in title bar or on-screen text)
- [ ] `AssetEditor` struct exists in `src/game/asset_editor/mod.rs` with all core enums
- [ ] `battle_arena.rs` is NOT modified

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles`
- `cmd`: `grep -c 'EditorStage' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `EditorStage enum exists in asset_editor module`
- `cmd`: `grep -c 'battle_editor' Cargo.toml`
  `expect_gt`: 0
  `description`: `battle_editor binary registered in Cargo.toml`
- `cmd`: `grep -c 'pub mod asset_editor' src/game/mod.rs`
  `expect_gt`: 0
  `description`: `asset_editor module registered`

## Success Looks Like
Running `cargo run --bin battle_editor` opens a window with "Battle Tök — Asset Editor" in the title. The window has a dark gray background. Pressing keys 1-5 changes which stage is shown (at minimum as text on screen). The game binary `battle_arena` is completely untouched.

## Dependencies
- Depends on: None

## Complexity
- Complexity: normal
- Min iterations: 2
