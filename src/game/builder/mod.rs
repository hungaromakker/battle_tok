//! Builder Module
//!
//! Building system with tools and modes for constructing hex-prism structures.

pub mod mode;
pub mod tools;
pub mod toolbar;
pub mod raycast;
pub mod placement;

pub use mode::{BuildCommand, BuilderMode};
pub use tools::{SelectedFace, BridgeTool, SHAPE_NAMES, BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, PHYSICS_CHECK_INTERVAL};
pub use toolbar::{BuildToolbar, BlockInventory, StashedBlock};
pub use raycast::{screen_to_ray, determine_hit_face, calculate_adjacent_block_position, snap_to_grid, find_snap_position, ray_terrain_intersection};
pub use placement::{check_block_support, calculate_bridge_segments, PlacementResult};
