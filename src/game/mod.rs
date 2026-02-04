//! Game Module
//!
//! Contains game-specific systems that build on top of the engine.

pub mod battle_sphere;
pub mod player;

// New modular game systems
pub mod types;
pub mod arena_player;
pub mod arena_cannon;
pub mod destruction;
pub mod trees;
pub mod terrain;
pub mod builder;
pub mod ui;
pub mod render;

// Legacy re-exports
pub use battle_sphere::Cannon;
pub use player::{CameraDelta, KeyCode, MovementDirection, PlayerInput};

// Re-exports from new modules
pub use types::{Vertex, Mesh, Camera};
pub use types::{hash_2d, noise_2d, smoothstep, fbm_noise, ridged_noise, turbulent_noise};
pub use types::{generate_box, generate_oriented_box, generate_rotated_box, generate_sphere};

pub use arena_player::{Player, MovementKeys, AimingKeys};
pub use arena_player::{PLAYER_EYE_HEIGHT, PLAYER_WALK_SPEED, PLAYER_SPRINT_SPEED, PLAYER_GRAVITY, PLAYER_JUMP_VELOCITY};
pub use arena_cannon::{ArenaCannon, CANNON_SMOOTHING, CANNON_ROTATION_SPEED, generate_cannon_mesh};
pub use destruction::{FallingPrism, DebrisParticle, spawn_debris, get_material_color, GRAVITY};
pub use trees::{PlacedTree, generate_trees_on_terrain, generate_tree_mesh, generate_all_trees_mesh};
pub use terrain::{TerrainParams, WATER_LEVEL, get_terrain_params, set_terrain_params};
pub use terrain::{terrain_height_at, terrain_color_at, terrain_normal_at, is_inside_hexagon};
pub use terrain::{generate_elevated_hex_terrain, generate_water_plane, generate_hex_platform};
pub use builder::{BuildCommand, BuilderMode, BuildToolbar, BridgeTool, SelectedFace};
pub use builder::{SHAPE_NAMES, BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, PHYSICS_CHECK_INTERVAL};
pub use ui::{StartOverlay, TerrainEditorUI, UISlider, add_quad, draw_text, get_char_bitmap};
pub use render::{Uniforms, HexPrismModelUniforms, SdfCannonUniforms, SdfCannonData};
pub use render::{SHADER_SOURCE, MergedMeshBuffers, create_test_walls};
