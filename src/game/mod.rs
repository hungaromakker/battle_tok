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
pub mod physics;
pub mod input;

// New Stalberg-style building and economy systems
pub mod building;
pub mod economy;
pub mod population;
pub mod state;

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
pub use destruction::{Meteor, MeteorSpawner, spawn_meteor_impact};
pub use trees::{PlacedTree, generate_trees_on_terrain, generate_tree_mesh, generate_all_trees_mesh};
pub use terrain::{TerrainParams, WATER_LEVEL, get_terrain_params, set_terrain_params};
pub use terrain::{terrain_height_at, terrain_color_at, terrain_normal_at, is_inside_hexagon};
pub use terrain::{generate_elevated_hex_terrain, generate_water_plane, generate_lava_plane, generate_hex_platform};
pub use terrain::{BridgeConfig, BridgeAABB, generate_bridge, generate_bridge_collision, is_point_on_bridge, get_bridge_height};
pub use builder::{BuildCommand, BuilderMode, BuildToolbar, BridgeTool, SelectedFace};
pub use builder::{SHAPE_NAMES, BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, PHYSICS_CHECK_INTERVAL};
pub use builder::{screen_to_ray, determine_hit_face, calculate_adjacent_block_position, snap_to_grid, find_snap_position, ray_terrain_intersection};
pub use builder::{check_block_support, calculate_bridge_segments, PlacementResult};
pub use ui::{StartOverlay, TerrainEditorUI, UISlider, TopBar, TOP_BAR_HEIGHT, add_quad, draw_text, get_char_bitmap};
pub use render::{Uniforms, HexPrismModelUniforms, SdfCannonUniforms, SdfCannonData};
pub use render::{TerrainParams as TerrainShaderParams, LavaParams, SkyStormParams, FogPostParams, TonemapParams};
pub use render::{SHADER_SOURCE, MergedMeshBuffers, create_test_walls};
pub use render::{generate_hex_grid_overlay, calculate_ghost_color, GHOST_PREVIEW_COLOR, generate_block_preview_mesh};
pub use physics::{CollisionResult, AABB, check_capsule_aabb_collision, check_capsule_hex_collision};
pub use physics::{hex_to_world_position, world_to_hex_coords};
pub use physics::{HEX_NEIGHBORS, has_support, find_unsupported_cascade, check_falling_prism_collision};
pub use input::{InputAction, InputContext, MovementState, AimingState, MovementKey, AimingKey, map_key_to_action};

// Building system re-exports
pub use building::{DualGrid, GridCell, GridCorner, CornerType, BLOCK_SIZE, HALF_BLOCK};
pub use building::{Material, MaterialProperties, MATERIALS};
pub use building::{BuildingBlock, BlockShape, BlockLibrary};
pub use building::{DragBuilder, DragState, BuildEvent};
pub use building::{MeshCombiner, CombinedMesh, CombinedVertex};

// Economy system re-exports
pub use economy::{Resources, ResourceType, STARTING_RESOURCES};
pub use economy::{DayCycle, TimeOfDay, DAY_DURATION_SECONDS};
pub use economy::{ProductionBuilding, ProductionType};

// Population system re-exports
pub use population::{Villager, VillagerRole, VillagerStats, Population};
pub use population::{Morale, MoraleModifier, MoraleState};
pub use population::{JobAssignment, JobAI, JobPriority};

// Game state re-export
pub use state::GameState;
