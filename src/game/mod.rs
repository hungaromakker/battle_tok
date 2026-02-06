//! Game Module
//!
//! Contains game-specific systems that build on top of the engine.

pub mod battle_sphere;
pub mod player;

// New modular game systems
pub mod arena_cannon;
pub mod arena_player;
pub mod builder;
pub mod destruction;
pub mod input;
pub mod physics;
pub mod render;
pub mod terrain;
pub mod trees;
pub mod types;
pub mod ui;

// Self-contained game systems
pub mod systems;

// Arena configuration
pub mod config;

// Asset editor (standalone binary module)
pub mod asset_editor;

// New Stalberg-style building and economy systems
pub mod building;
pub mod economy;
pub mod population;
pub mod state;

// Legacy re-exports
pub use battle_sphere::Cannon;
pub use player::{CameraDelta, KeyCode, MovementDirection, PlayerInput};

// Re-exports from new modules
pub use types::{Camera, Mesh, Vertex};
pub use types::{fbm_noise, hash_2d, noise_2d, ridged_noise, smoothstep, turbulent_noise};
pub use types::{generate_box, generate_oriented_box, generate_rotated_box, generate_sphere};

pub use arena_cannon::{
    ArenaCannon, CANNON_GRAB_OFFSET, CANNON_GRAB_RANGE, CANNON_TERRAIN_OFFSET, generate_cannon_mesh,
};
pub use arena_player::{AimingKeys, ArenaGround, BridgeDef, IslandDef, MovementKeys, Player};
pub use arena_player::{
    PLAYER_EYE_HEIGHT, PLAYER_GRAVITY, PLAYER_JUMP_VELOCITY, PLAYER_SPRINT_SPEED, PLAYER_WALK_SPEED,
};
pub use builder::{BLOCK_GRID_SIZE, BLOCK_SNAP_DISTANCE, PHYSICS_CHECK_INTERVAL, SHAPE_NAMES};
pub use builder::{BridgeTool, BuildCommand, BuildToolbar, BuilderMode, SelectedFace};
pub use builder::{PlacementResult, calculate_bridge_segments, check_block_support};
pub use builder::{
    calculate_adjacent_block_position, determine_hit_face, find_snap_position,
    ray_terrain_intersection, screen_to_ray, snap_to_grid,
};
pub use destruction::{DebrisParticle, FallingPrism, GRAVITY, get_material_color, spawn_debris};
pub use destruction::{Meteor, MeteorSpawner, spawn_meteor_impact};
pub use input::{
    AimingKey, AimingState, InputAction, InputContext, MovementKey, MovementState,
    map_key_to_action,
};
pub use physics::{
    AABB, CollisionResult, check_capsule_aabb_collision, check_capsule_hex_collision,
};
pub use physics::{
    HEX_NEIGHBORS, check_falling_prism_collision, find_unsupported_cascade, has_support,
};
pub use physics::{hex_to_world_position, world_to_hex_coords};
pub use render::{
    FogPostParams, LavaParams, SkyStormParams, TerrainParams as TerrainShaderParams, TonemapParams,
};
pub use render::{
    GHOST_PREVIEW_COLOR, calculate_ghost_color, generate_block_preview_mesh,
    generate_hex_grid_overlay,
};
pub use render::{HexPrismModelUniforms, SdfCannonData, SdfCannonUniforms, Uniforms};
pub use render::{MergedMeshBuffers, SHADER_SOURCE, create_test_walls};
pub use terrain::{
    BridgeAABB, BridgeConfig, generate_bridge, generate_bridge_collision, get_bridge_height,
    is_point_on_bridge,
};
pub use terrain::{
    FloatingIslandConfig, IslandLayer, generate_floating_island, generate_lava_ocean,
};
pub use terrain::{TerrainParams, WATER_LEVEL, get_terrain_params, set_terrain_params};
pub use terrain::{
    generate_elevated_hex_terrain, generate_hex_platform, generate_lava_plane, generate_water_plane,
};
pub use terrain::{is_inside_hexagon, terrain_color_at, terrain_height_at, terrain_normal_at};
pub use trees::{
    PlacedTree, generate_all_trees_mesh, generate_tree_mesh, generate_trees_on_terrain,
};
pub use ui::{
    StartOverlay, TOP_BAR_HEIGHT, TerrainEditorUI, TopBar, UISlider, add_quad, draw_text,
    get_char_bitmap,
};

// Building system re-exports
pub use building::{BLOCK_SIZE, CornerType, DualGrid, GridCell, GridCorner, HALF_BLOCK};
pub use building::{BlockLibrary, BlockShape, BuildingBlock};
pub use building::{BuildEvent, DragBuilder, DragState};
pub use building::{CombinedMesh, CombinedVertex, MeshCombiner};
pub use building::{MATERIALS, Material, MaterialProperties};

// Economy system re-exports
pub use economy::{DAY_DURATION_SECONDS, DayCycle, TimeOfDay};
pub use economy::{ProductionBuilding, ProductionType};
pub use economy::{ResourceType, Resources, STARTING_RESOURCES};

// Population system re-exports
pub use population::{JobAI, JobAssignment, JobPriority};
pub use population::{Morale, MoraleModifier, MoraleState};
pub use population::{Population, Villager, VillagerRole, VillagerStats};

// Scenes
pub mod scenes;

// Game state re-export
pub use state::GameState;

// Systems re-exports
pub use systems::{
    BuildingSystem, BuildingSystemV2, CollisionSystem, ProjectileKind, ProjectileSystem,
    ProjectileUpdate,
};

// Scene re-exports
pub use scenes::{BattleScene, ExplosionEvent, WeaponMode};

// Config re-exports
pub use config::VisualConfig;
pub use config::{ArenaBridgeConfig, ArenaConfig, IslandConfig};
