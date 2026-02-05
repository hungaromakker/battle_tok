# US-P3-014: Final Cleanup and Documentation

## Description
The final polish story: remove dead code, add documentation, run clippy, verify the game plays identically to before the refactoring, and update project docs to reflect the new architecture.

## The Core Concept / Why This Matters
After 13 stories of extraction and refactoring, there will be unused imports, dead helper functions, missing doc comments, and possibly clippy warnings. This story ensures the codebase is clean, documented, and passes all quality checks. It's also the final validation that the refactoring didn't break anything — the game must look and play exactly as it did before Phase 3 started.

## Goal
Clean codebase, passing clippy, complete documentation, and verified identical gameplay.

## Files to Create/Modify
- Modify `src/bin/battle_arena.rs` — Remove unused code, add docs
- Modify `src/game/mod.rs` — Clean up re-exports
- Modify `src/game/systems/mod.rs` — Add doc comments
- Modify `src/game/scenes/mod.rs` — Add doc comments
- Modify `src/game/config/mod.rs` — Add doc comments
- Update `docs/architecture.md` — Document new module structure

## Implementation Steps
1. **Run clippy and fix all warnings:**
   ```bash
   cargo clippy --bin battle_arena -- -W clippy::all
   ```
   Fix: unused imports, unused variables, redundant clones, etc.

2. **Add module-level documentation:**
   ```rust
   //! # Config Module
   //!
   //! Centralized configuration for the battle arena.
   //! All gameplay constants, visual settings, and input bindings
   //! are defined here rather than hardcoded in game code.
   ```

3. **Add doc comments to public types and methods** in:
   - `ArenaConfig`, `VisualConfig`, `InputConfig`
   - All System structs and their public methods
   - `BattleScene` and its public methods

4. **Verify line count:**
   ```bash
   wc -l src/bin/battle_arena.rs
   # Target: under 500 lines
   ```

5. **Update docs/architecture.md** with new module diagram:
   ```
   src/game/
   ├── config/     # Configuration structs (arena, visual, input)
   ├── systems/    # Game logic systems (collision, projectile, etc.)
   ├── scenes/     # Scene composition (BattleScene)
   ├── builder/    # Building mode tools
   ├── building/   # Stalberg-style building blocks
   ├── economy/    # Resources and day cycle
   ├── population/ # Villagers and job AI
   ├── terrain/    # Island and terrain generation
   ├── ui/         # HUD and overlay rendering
   └── ...
   ```

6. **Run the game and verify:**
   - Movement works (WASD, jump, sprint)
   - Camera works (mouse look, V toggle)
   - Cannon fires (arrow keys, space)
   - Building works (B mode, block placement)
   - Destruction works (projectiles destroy walls)
   - Meteors fall
   - UI displays (top bar, toolbar)
   - Visual look matches (apocalyptic sky, lava, floating islands)

7. **Run full quality check:**
   ```bash
   cargo check
   cargo clippy --bin battle_arena -- -W clippy::all
   cargo build --bin battle_arena --release
   ```

## Acceptance Criteria
- [ ] `battle_arena.rs` is under 500 lines
- [ ] `cargo clippy` passes with no warnings
- [ ] `cargo build --bin battle_arena --release` succeeds
- [ ] All public types have doc comments
- [ ] `docs/architecture.md` updated with new structure
- [ ] Game runs with identical behavior and visuals
- [ ] No dead code or unused imports
- [ ] `cargo check` passes (typecheck)

## Success Looks Like
The refactoring is complete. `battle_arena.rs` is a clean, thin wrapper under 500 lines. The game compiles without warnings. The game plays and looks exactly the same as before Phase 3. The architecture is documented. A new developer can understand the codebase structure from the module layout alone.

## Dependencies
- Depends on: US-P3-013 (render passes extracted)
