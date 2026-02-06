# US-P4-010: Undo/Redo System

## Description
Create `src/game/asset_editor/undo.rs` with a command-based undo/redo system that tracks all editor operations across every stage. Each operation is captured as an `UndoCommand` variant that stores enough data to reverse or reapply the change. An `UndoStack` manages a linear history with a cursor, supporting up to 50 operations. Ctrl+Z undoes, Ctrl+Y redoes. The asset editor is a **separate binary** (`battle_editor`); `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
Creative tools without undo are unusable. Artists experiment constantly — drawing outlines, adjusting extrusion, sculpting details, painting colors — and need to freely revert mistakes without fear of losing work. The command pattern captures each operation as a reversible action, enabling both undo and redo. A 50-operation limit keeps memory bounded while covering typical editing sessions. This system spans all five editor stages, making the entire pipeline feel safe and forgiving.

## Goal
Create `src/game/asset_editor/undo.rs` with `UndoStack` and `UndoCommand` supporting undo/redo across all editor stages, integrated with Ctrl+Z/Ctrl+Y keyboard shortcuts in the `battle_editor` binary.

## Files to Create/Modify
- **Create** `src/game/asset_editor/undo.rs` — `UndoCommand` enum, `UndoStack` struct, push/undo/redo logic
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod undo;`, add `undo_stack: UndoStack` field to `AssetEditor`, integrate undo/redo calls
- **Modify** `src/bin/battle_editor.rs` — Route Ctrl+Z and Ctrl+Y key events to `AssetEditor` undo/redo methods

## Implementation Steps
1. Define `UndoCommand` enum with variants for each editor operation:
   ```rust
   pub enum UndoCommand {
       AddOutline { index: usize, outline: Vec<[f32; 2]> },
       RemoveOutline { index: usize, outline: Vec<[f32; 2]> },
       ModifyOutline { index: usize, old: Vec<[f32; 2]>, new: Vec<[f32; 2]> },
       ChangeExtrudeParams { old_depth: f32, old_inflation: f32, new_depth: f32, new_inflation: f32 },
       MeshSnapshot { old_vertices: Vec<Vertex>, old_indices: Vec<u32>, new_vertices: Vec<Vertex>, new_indices: Vec<u32> },
       VertexColors { old_colors: Vec<[f32; 4]>, new_colors: Vec<[f32; 4]> },
   }
   ```
2. Create `UndoStack` struct:
   - `commands: Vec<UndoCommand>` — the history buffer
   - `cursor: usize` — points to the next available slot (everything before cursor is undoable)
   - `max_size: usize` — defaults to 50
3. Implement `push(&mut self, cmd: UndoCommand)`:
   - Truncate any redo history: `self.commands.truncate(self.cursor)`
   - Push the new command
   - Increment cursor
   - If `commands.len() > max_size`, remove oldest (index 0) and decrement cursor
4. Implement `undo(&mut self) -> Option<&UndoCommand>`:
   - If `cursor == 0`, return `None` (nothing to undo)
   - Decrement cursor, return reference to `commands[cursor]`
5. Implement `redo(&mut self) -> Option<&UndoCommand>`:
   - If `cursor >= commands.len()`, return `None` (nothing to redo)
   - Return reference to `commands[cursor]`, increment cursor
6. Implement `can_undo(&self) -> bool` and `can_redo(&self) -> bool` helpers.
7. In `mod.rs`, add `apply_undo(cmd)` and `apply_redo(cmd)` methods on `AssetEditor` that interpret each `UndoCommand` variant and modify state accordingly.
8. In `battle_editor.rs`, detect `Ctrl+Z` and `Ctrl+Y` key combinations in the keyboard event handler and call the appropriate undo/redo method.

## Code Patterns
```rust
pub struct UndoStack {
    commands: Vec<UndoCommand>,
    cursor: usize,
    max_size: usize,
}

impl UndoStack {
    pub fn new(max_size: usize) -> Self {
        Self { commands: Vec::new(), cursor: 0, max_size }
    }

    pub fn push(&mut self, cmd: UndoCommand) {
        self.commands.truncate(self.cursor);
        self.commands.push(cmd);
        self.cursor += 1;
        if self.commands.len() > self.max_size {
            self.commands.remove(0);
            self.cursor -= 1;
        }
    }

    pub fn undo(&mut self) -> Option<&UndoCommand> {
        if self.cursor == 0 { return None; }
        self.cursor -= 1;
        Some(&self.commands[self.cursor])
    }

    pub fn redo(&mut self) -> Option<&UndoCommand> {
        if self.cursor >= self.commands.len() { return None; }
        let cmd = &self.commands[self.cursor];
        self.cursor += 1;
        Some(cmd)
    }
}
```

## Acceptance Criteria
- [ ] `undo.rs` exists with `UndoStack` and `UndoCommand` types
- [ ] `UndoCommand` has variants for canvas (AddOutline, RemoveOutline, ModifyOutline), extrude (ChangeExtrudeParams), sculpt (MeshSnapshot), and paint (VertexColors) operations
- [ ] Stack enforces maximum of 50 operations, dropping oldest when exceeded
- [ ] `push()` truncates redo history when a new command is pushed after undo
- [ ] `undo()` returns the command to reverse and decrements cursor position
- [ ] `redo()` returns the command to reapply and increments cursor position
- [ ] `can_undo()` and `can_redo()` correctly report availability
- [ ] Ctrl+Z triggers undo in `battle_editor.rs` keyboard handling
- [ ] Ctrl+Y triggers redo in `battle_editor.rs` keyboard handling
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo check --bin battle_arena` passes with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with undo module`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles unchanged`
- `cmd`: `test -f src/game/asset_editor/undo.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `undo.rs file exists`
- `cmd`: `grep -c 'UndoStack\|UndoCommand' src/game/asset_editor/undo.rs`
  `expect_gt`: 0
  `description`: `Core undo types are defined`
- `cmd`: `grep -c 'AddOutline\|RemoveOutline\|MeshSnapshot\|VertexColors\|ChangeExtrudeParams' src/game/asset_editor/undo.rs`
  `expect_gt`: 0
  `description`: `All UndoCommand variants exist`
- `cmd`: `grep -c 'pub mod undo' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `undo module registered in mod.rs`

## Success Looks Like
The artist draws three outlines on the canvas. They press Ctrl+Z and the last outline disappears. They press Ctrl+Z again and the second outline disappears. They press Ctrl+Y and the second outline reappears. They switch to sculpt mode, deform the mesh, then undo — the mesh snaps back to its pre-sculpt state. They paint some vertices, undo, and the colors revert. The undo/redo feels instant and reliable across all stages. The artist never worries about making mistakes because they can always go back.

## Dependencies
- Depends on: US-P4-001 (needs editor skeleton with stage system and keyboard event handling)

## Complexity
- Complexity: normal
- Min iterations: 1
