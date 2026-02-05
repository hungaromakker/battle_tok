# US-P3-010: Wire InputConfig to Engine Input System

## Description
Create `src/game/config/input_config.rs` with key binding definitions that can be applied to the engine's input system. Currently all key bindings are hardcoded in the massive `handle_key()` and `handle_device_event()` methods of `battle_arena.rs` (~528 lines). This story doesn't rewrite those methods yet — it creates the config struct that Story 11 will use.

## The Core Concept / Why This Matters
The game has ~30 key bindings across movement (WASD), building (B, number keys), combat (Space, arrow keys), UI (T, F1, F11), and camera (V, R). These are all `match` arms in the event handler. By extracting them into an `InputConfig` struct, we enable: (1) remappable keys in the future, (2) documentation of all inputs in one place, (3) separation of "what key" from "what action".

Note: The game already has `src/game/input/` with `InputAction`, `MovementState`, `AimingState`, and `map_key_to_action()`. The `InputConfig` builds on these.

## Goal
Create `src/game/config/input_config.rs` that defines all key bindings as a data structure rather than inline match arms.

## Files to Create/Modify
- Create `src/game/config/input_config.rs` — InputConfig with all bindings
- Modify `src/game/config/mod.rs` — Add module and re-exports

## Implementation Steps
1. Catalog all key bindings from `battle_arena.rs`:
   - Movement: W/A/S/D, Space (jump), Shift (sprint)
   - Aiming: Arrow Up/Down/Left/Right
   - Building: B (toggle), 1-9 (select shape), mouse click (place)
   - Combat: Space (fire in free camera), mouse click
   - Camera: V (toggle FPS/free), R (reset)
   - UI: T (terrain editor), F1 (flat terrain), F11 (fullscreen)
   - Other: C (clear projectiles), Escape (exit)

2. Create the config struct:
   ```rust
   use winit::keyboard::KeyCode;

   #[derive(Clone, Debug)]
   pub struct MovementBindings {
       pub forward: KeyCode,
       pub backward: KeyCode,
       pub left: KeyCode,
       pub right: KeyCode,
       pub jump: KeyCode,
       pub sprint: KeyCode,
   }

   #[derive(Clone, Debug)]
   pub struct AimingBindings {
       pub up: KeyCode,
       pub down: KeyCode,
       pub left: KeyCode,
       pub right: KeyCode,
   }

   #[derive(Clone, Debug)]
   pub struct BuildingBindings {
       pub toggle_mode: KeyCode,
       pub shape_keys: [KeyCode; 9], // 1-9
   }

   #[derive(Clone, Debug)]
   pub struct CameraBindings {
       pub toggle_mode: KeyCode,
       pub reset: KeyCode,
   }

   #[derive(Clone, Debug)]
   pub struct UIBindings {
       pub terrain_editor: KeyCode,
       pub flat_terrain: KeyCode,
       pub fullscreen: KeyCode,
   }

   #[derive(Clone, Debug)]
   pub struct InputConfig {
       pub movement: MovementBindings,
       pub aiming: AimingBindings,
       pub building: BuildingBindings,
       pub camera: CameraBindings,
       pub ui: UIBindings,
       pub clear_projectiles: KeyCode,
       pub exit: KeyCode,
   }

   impl Default for InputConfig {
       fn default() -> Self { /* current key mappings */ }
   }
   ```

3. Add a helper method to check if a key belongs to a specific category:
   ```rust
   impl InputConfig {
       pub fn classify_key(&self, key: KeyCode) -> Option<InputCategory> { ... }
   }
   ```

4. Run `cargo check`.

## Code Patterns
The existing input module at `src/game/input/`:
```rust
pub enum InputAction {
    MoveForward, MoveBackward, MoveLeft, MoveRight,
    Jump, Sprint,
    AimUp, AimDown, AimLeft, AimRight,
    // ...
}

pub fn map_key_to_action(key: KeyCode) -> Option<InputAction> { ... }
```

InputConfig extends this by making the key→action mapping configurable.

## Acceptance Criteria
- [ ] All key bindings from `battle_arena.rs` are captured in `InputConfig`
- [ ] `InputConfig::default()` matches current hardcoded keys
- [ ] Struct derives `Clone` and `Debug`
- [ ] Uses `winit::keyboard::KeyCode` for key types
- [ ] Re-exported from config module
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
There's a single struct that documents every key binding in the game. When Story 11 rewrites the input handler, it can reference `input_config.movement.forward` instead of hardcoding `KeyCode::KeyW`.

## Dependencies
- Depends on: None
