//! Undo/Redo System for the Asset Editor
//!
//! Provides a command-based undo/redo stack that records editor operations
//! and allows stepping backwards and forwards through history.
//!
//! # Usage
//!
//! ```ignore
//! use battle_tok_engine::game::asset_editor::undo::{UndoStack, UndoCommand};
//!
//! let mut stack = UndoStack::new();
//! stack.push(UndoCommand::AddOutline {
//!     index: 0,
//!     outline: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
//! });
//!
//! if let Some(cmd) = stack.undo() {
//!     // Apply inverse of cmd to editor state
//! }
//! ```

// ============================================================================
// TYPES
// ============================================================================

/// Vertex data stored in mesh snapshots for undo/redo.
///
/// Captures the full vertex state so that mesh edits can be precisely reversed.
#[derive(Debug, Clone)]
pub struct UndoVertex {
    /// Position in 3D space
    pub position: [f32; 3],
    /// Surface normal
    pub normal: [f32; 3],
    /// RGBA color
    pub color: [f32; 4],
}

/// An undoable editor command.
///
/// Each variant captures enough state to reverse (undo) or re-apply (redo)
/// a single editor operation.
#[derive(Debug, Clone)]
pub enum UndoCommand {
    /// A new outline was added at the given index.
    AddOutline {
        /// Index where the outline was inserted
        index: usize,
        /// The outline points that were added
        outline: Vec<[f32; 2]>,
    },

    /// An outline was removed from the given index.
    RemoveOutline {
        /// Index the outline was removed from
        index: usize,
        /// The outline points that were removed (kept for redo)
        outline: Vec<[f32; 2]>,
    },

    /// An existing outline was modified.
    ModifyOutline {
        /// Index of the modified outline
        index: usize,
        /// The outline points before modification
        old: Vec<[f32; 2]>,
        /// The outline points after modification
        new: Vec<[f32; 2]>,
    },

    /// Extrusion parameters were changed.
    ChangeExtrudeParams {
        /// Previous extrusion depth
        old_depth: f32,
        /// Previous inflation amount
        old_inflation: f32,
        /// New extrusion depth
        new_depth: f32,
        /// New inflation amount
        new_inflation: f32,
    },

    /// A full mesh snapshot (before and after) for sculpt operations.
    MeshSnapshot {
        /// Vertices before the edit
        old_vertices: Vec<UndoVertex>,
        /// Triangle indices before the edit
        old_indices: Vec<u32>,
        /// Vertices after the edit
        new_vertices: Vec<UndoVertex>,
        /// Triangle indices after the edit
        new_indices: Vec<u32>,
    },

    /// Vertex colors were changed.
    VertexColors {
        /// Colors before the edit
        old_colors: Vec<[f32; 4]>,
        /// Colors after the edit
        new_colors: Vec<[f32; 4]>,
    },
}

// ============================================================================
// UNDO STACK
// ============================================================================

/// Maximum number of commands stored in the undo stack.
/// When exceeded, the oldest commands are dropped.
const MAX_UNDO_SIZE: usize = 50;

/// A bounded undo/redo stack.
///
/// Stores a linear history of [`UndoCommand`]s with a cursor that tracks
/// the current position. Pushing a new command after undoing discards the
/// redo history (standard undo/redo semantics).
///
/// The stack enforces a maximum size of 50 entries, dropping the oldest
/// commands when the limit is reached.
#[derive(Debug)]
pub struct UndoStack {
    /// The command history buffer.
    commands: Vec<UndoCommand>,
    /// Points to the next command index. Commands at `[0..cursor]` are undoable,
    /// commands at `[cursor..len]` are redoable.
    cursor: usize,
    /// Maximum number of commands to retain.
    max_size: usize,
}

impl Default for UndoStack {
    fn default() -> Self {
        Self::new()
    }
}

impl UndoStack {
    /// Create a new empty undo stack with the default max size (50).
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
            cursor: 0,
            max_size: MAX_UNDO_SIZE,
        }
    }

    /// Push a new command onto the stack.
    ///
    /// - Any redo history (commands after the cursor) is discarded.
    /// - If the stack exceeds [`MAX_UNDO_SIZE`], the oldest command is dropped.
    pub fn push(&mut self, command: UndoCommand) {
        // Truncate redo history: discard everything after the cursor
        self.commands.truncate(self.cursor);

        // Push the new command
        self.commands.push(command);
        self.cursor = self.commands.len();

        // Enforce maximum size by dropping the oldest entry
        if self.commands.len() > self.max_size {
            let excess = self.commands.len() - self.max_size;
            self.commands.drain(0..excess);
            self.cursor = self.commands.len();
        }
    }

    /// Undo the most recent command.
    ///
    /// Returns a reference to the command that should be reversed,
    /// or `None` if there is nothing to undo.
    pub fn undo(&mut self) -> Option<&UndoCommand> {
        if self.cursor == 0 {
            return None;
        }
        self.cursor -= 1;
        Some(&self.commands[self.cursor])
    }

    /// Redo the next command.
    ///
    /// Returns a reference to the command that should be re-applied,
    /// or `None` if there is nothing to redo.
    pub fn redo(&mut self) -> Option<&UndoCommand> {
        if self.cursor >= self.commands.len() {
            return None;
        }
        let cmd = &self.commands[self.cursor];
        self.cursor += 1;
        Some(cmd)
    }

    /// Returns `true` if there is at least one command that can be undone.
    pub fn can_undo(&self) -> bool {
        self.cursor > 0
    }

    /// Returns `true` if there is at least one command that can be redone.
    pub fn can_redo(&self) -> bool {
        self.cursor < self.commands.len()
    }

    /// Return the number of undoable commands (commands before the cursor).
    pub fn undo_count(&self) -> usize {
        self.cursor
    }

    /// Return the number of redoable commands (commands after the cursor).
    pub fn redo_count(&self) -> usize {
        self.commands.len() - self.cursor
    }

    /// Clear all undo/redo history.
    pub fn clear(&mut self) {
        self.commands.clear();
        self.cursor = 0;
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a simple AddOutline command.
    fn add_outline_cmd(index: usize) -> UndoCommand {
        UndoCommand::AddOutline {
            index,
            outline: vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0]],
        }
    }

    #[test]
    fn test_new_stack_is_empty() {
        let stack = UndoStack::new();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 0);
        assert_eq!(stack.redo_count(), 0);
    }

    #[test]
    fn test_push_and_undo() {
        let mut stack = UndoStack::new();
        stack.push(add_outline_cmd(0));

        assert!(stack.can_undo());
        assert!(!stack.can_redo());

        let cmd = stack.undo();
        assert!(cmd.is_some());
        assert!(!stack.can_undo());
        assert!(stack.can_redo());
    }

    #[test]
    fn test_undo_and_redo() {
        let mut stack = UndoStack::new();
        stack.push(add_outline_cmd(0));
        stack.push(add_outline_cmd(1));

        // Undo both
        assert!(stack.undo().is_some());
        assert!(stack.undo().is_some());
        assert!(stack.undo().is_none()); // Nothing left to undo

        // Redo both
        assert!(stack.redo().is_some());
        assert!(stack.redo().is_some());
        assert!(stack.redo().is_none()); // Nothing left to redo
    }

    #[test]
    fn test_push_truncates_redo_history() {
        let mut stack = UndoStack::new();
        stack.push(add_outline_cmd(0));
        stack.push(add_outline_cmd(1));
        stack.push(add_outline_cmd(2));

        // Undo two commands
        stack.undo();
        stack.undo();
        assert_eq!(stack.redo_count(), 2);

        // Push a new command -- should discard redo history
        stack.push(add_outline_cmd(99));
        assert!(!stack.can_redo());
        assert_eq!(stack.undo_count(), 2); // cmd(0) + cmd(99)
    }

    #[test]
    fn test_max_size_enforcement() {
        let mut stack = UndoStack::new();

        // Push 55 commands (exceeds max of 50)
        for i in 0..55 {
            stack.push(add_outline_cmd(i));
        }

        // Should have exactly 50 commands
        assert_eq!(stack.undo_count(), 50);
        assert_eq!(stack.redo_count(), 0);

        // The oldest 5 commands should have been dropped
        // Undo all 50
        for _ in 0..50 {
            assert!(stack.undo().is_some());
        }
        assert!(stack.undo().is_none());
    }

    #[test]
    fn test_clear() {
        let mut stack = UndoStack::new();
        stack.push(add_outline_cmd(0));
        stack.push(add_outline_cmd(1));
        stack.clear();
        assert!(!stack.can_undo());
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_all_command_variants() {
        let mut stack = UndoStack::new();

        stack.push(UndoCommand::AddOutline {
            index: 0,
            outline: vec![[0.0, 0.0]],
        });
        stack.push(UndoCommand::RemoveOutline {
            index: 0,
            outline: vec![[1.0, 1.0]],
        });
        stack.push(UndoCommand::ModifyOutline {
            index: 0,
            old: vec![[0.0, 0.0]],
            new: vec![[2.0, 2.0]],
        });
        stack.push(UndoCommand::ChangeExtrudeParams {
            old_depth: 1.0,
            old_inflation: 0.5,
            new_depth: 2.0,
            new_inflation: 0.8,
        });
        stack.push(UndoCommand::MeshSnapshot {
            old_vertices: vec![UndoVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            }],
            old_indices: vec![0, 1, 2],
            new_vertices: vec![UndoVertex {
                position: [1.0, 1.0, 1.0],
                normal: [0.0, 1.0, 0.0],
                color: [1.0, 0.0, 0.0, 1.0],
            }],
            new_indices: vec![0, 1, 2],
        });
        stack.push(UndoCommand::VertexColors {
            old_colors: vec![[1.0, 1.0, 1.0, 1.0]],
            new_colors: vec![[0.0, 0.0, 1.0, 1.0]],
        });

        assert_eq!(stack.undo_count(), 6);

        // Undo all
        for _ in 0..6 {
            assert!(stack.undo().is_some());
        }
        assert!(!stack.can_undo());

        // Redo all
        for _ in 0..6 {
            assert!(stack.redo().is_some());
        }
        assert!(!stack.can_redo());
    }

    #[test]
    fn test_default_impl() {
        let stack = UndoStack::default();
        assert_eq!(stack.undo_count(), 0);
    }
}
