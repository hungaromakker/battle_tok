//! Building System - Stalberg-style organic castle building
//!
//! Features:
//! - Dual-grid system for organic corners (Oscar Stalberg technique)
//! - Click-and-drag continuous building
//! - Multi-layer overlapping grids for realistic structures
//! - 1 dm³ (0.1m) building blocks that auto-combine
//! - Material-based construction (wood, stone, iron, etc.)

pub mod dual_grid;
pub mod materials;
pub mod blocks;
pub mod drag_builder;
pub mod mesh_combine;

pub use dual_grid::{DualGrid, GridCell, GridCorner, CornerType, BLOCK_SIZE, HALF_BLOCK};
pub use materials::{Material, MaterialProperties, MATERIALS};
pub use blocks::{BuildingBlock, BlockShape, BlockLibrary};
pub use drag_builder::{DragBuilder, DragState, BuildEvent};
pub use mesh_combine::{MeshCombiner, CombinedMesh, CombinedVertex};
