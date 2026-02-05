//! Terrain Module
//!
//! Procedural terrain generation with adjustable parameters.

pub mod bridge;
pub mod floating_island;
pub mod generation;
pub mod hex_terrain;
pub mod params;

pub use bridge::{
    BridgeAABB, BridgeConfig, generate_bridge, generate_bridge_collision, get_bridge_height,
    is_point_on_bridge,
};
pub use floating_island::{
    FloatingIslandConfig, IslandLayer, generate_floating_island, generate_lava_ocean,
};
pub use generation::{
    blend_colors, is_inside_hexagon, smooth_step, terrain_color_at, terrain_height_at,
    terrain_normal_at,
};
pub use hex_terrain::{
    generate_elevated_hex_terrain, generate_hex_platform, generate_lava_plane, generate_water_plane,
};
pub use params::{TerrainParams, WATER_LEVEL, get_terrain_params, set_terrain_params};
