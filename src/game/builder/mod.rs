//! Builder Module
//!
//! Building system with tools and modes for constructing hex-prism structures.

pub mod mode;
pub mod placement;
pub mod raycast;
pub mod toolbar;
pub mod tools;

pub use mode::{BuildCommand, BuilderMode};
pub use placement::{PlacementResult, calculate_bridge_segments, check_block_support};
pub use raycast::{
    calculate_adjacent_block_position, determine_hit_face, find_snap_position,
    ray_terrain_intersection, screen_to_ray, snap_to_grid,
};
pub use toolbar::{BlockInventory, BuildToolbar, StashedBlock};
pub use tools::{
    BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, BridgeTool, PHYSICS_CHECK_INTERVAL, SHAPE_NAMES,
    SelectedFace,
};
