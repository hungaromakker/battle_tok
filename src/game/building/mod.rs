//! Building System - Stalberg-style organic castle building
//!
//! Features:
//! - Dual-grid system for organic corners (Oscar Stalberg technique)
//! - Click-and-drag continuous building
//! - Multi-layer overlapping grids for realistic structures
//! - 1 dm³ (0.1m) building blocks that auto-combine
//! - Material-based construction (wood, stone, iron, etc.)

pub mod blocks;
pub mod drag_builder;
pub mod dual_grid;
pub mod materials;
pub mod mesh_combine;

pub use blocks::{BlockLibrary, BlockShape, BuildingBlock};
pub use drag_builder::{BuildEvent, DragBuilder, DragState};
pub use dual_grid::{BLOCK_SIZE, CornerType, DualGrid, GridCell, GridCorner, HALF_BLOCK};
pub use materials::{
    MATERIAL_PHYSICS, MATERIALS, Material, MaterialPhysics, MaterialProperties,
    get_material_physics,
};
pub use mesh_combine::{CombinedMesh, CombinedVertex, MeshCombiner};
