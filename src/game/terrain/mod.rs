//! Terrain Module
//!
//! Procedural terrain generation with adjustable parameters.

pub mod params;
pub mod generation;
pub mod hex_terrain;
pub mod bridge;

pub use params::{TerrainParams, WATER_LEVEL, get_terrain_params, set_terrain_params};
pub use generation::{terrain_height_at, terrain_color_at, terrain_normal_at, is_inside_hexagon, smooth_step, blend_colors};
pub use hex_terrain::{generate_elevated_hex_terrain, generate_water_plane, generate_lava_plane, generate_hex_platform};
pub use bridge::{BridgeConfig, BridgeAABB, generate_bridge, generate_bridge_collision, is_point_on_bridge, get_bridge_height};
