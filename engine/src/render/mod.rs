//! Render Module
//!
//! This module contains the core rendering infrastructure for the Magic Engine.
//! It provides wgpu-based rendering with VSync-off support for maximum performance.
//!
//! ## Architecture
//!
//! The render system is organized into several layers:
//!
//! - **GPU Context** (`gpu_context`): Manages wgpu device, queue, surface, and common buffers
//! - **Render Pass** (`render_pass`): Trait-based abstraction for individual render passes
//! - **Scene Coordinator** (`scene_coordinator`): High-level scene management and frame submission
//! - **Specialized Passes**: UI, Mesh, Sky, etc. - each implementing the RenderPass trait

// Core rendering infrastructure (new modular system)
pub mod gpu_context;
pub mod mesh_pass;
pub mod render_pass;
pub mod scene_coordinator;
pub mod ui_pass;

pub mod adaptive_step;
pub mod apocalyptic_sky;
pub mod bake_queue;
pub mod binding_validator;
pub mod bridge_materials;
pub mod building_blocks;
pub mod building_physics;
pub mod castle_material;
pub mod compute_pipelines;
pub mod cubemap_skybox;
pub mod culling;
pub mod entities;
pub mod flag_material;
pub mod fog_post;
pub mod froxel_assignment;
pub mod froxel_bounds;
pub mod froxel_buffers;
pub mod froxel_config;
pub mod froxel_cpu;
pub mod froxel_dispatch;
pub mod hex_prism;
pub mod instancing;
pub mod marching_cubes;
pub mod material_system;
pub mod particles;
pub mod pipeline;
pub mod point_lights;
pub mod rebake_tracker;
pub mod sculpting;
pub mod sdf_bake_dispatch;
pub mod sdf_baker;
pub mod sdf_operations;
pub mod shader_loader;
pub mod sky;
pub mod sky_bake_dispatch;
pub mod sky_cubemap;
pub mod stormy_sky;
pub mod tile_cull_dispatch;
pub mod uniforms;

// Re-export commonly used types for convenience
pub use instancing::{
    CreatureInstance, INSTANCE_BUFFER_SIZE, MAX_CREATURE_INSTANCES, create_instance_buffer,
    create_instance_buffer_init, instance_buffer_layout, pack_rgba, unpack_rgba,
    update_instance_buffer,
};
pub use pipeline::{
    RenderConfig, RenderState, detect_software_renderer, get_recommended_resolution,
};
pub use shader_loader::{ShaderSource, create_shader_module, load_shader_file};
pub use uniforms::{
    ENTITY_COLORS, EntityBufferData, PlacedEntity, Season, SkySettings, TestUniforms, WeatherType,
    pack_color,
};

// Also re-export from entities module for direct access
pub use entities::{
    ENTITY_COLORS as ENTITY_COLOR_PALETTE,
    // Advanced 96-byte entity struct for raymarcher.wgsl
    GpuEntity,
    GpuEntityBuffer,
    entity_type,
    pack_color as pack_color_rgb,
    unpack_color,
};

// Re-export sky rendering types
pub use apocalyptic_sky::{ApocalypticSky, ApocalypticSkyConfig};
pub use cubemap_skybox::CubemapSkybox;
pub use sky::{CLOUD_TEXTURE_SIZE, CloudTexture};
pub use sky_cubemap::SkyCubemap;
pub use stormy_sky::{StormySky, StormySkyConfig};

// Re-export fog post-pass types (Phase 2: Depth-Based Fog Post-Pass)
pub use fog_post::{FogPostConfig, FogPostPass, LavaSteamConfig};

// Re-export castle material types (Phase 2: Castle Stone Shader)
pub use castle_material::{CastleMaterial, CastleMaterialConfig};

// Re-export flag material types (Phase 2: Team Flag Shader)
pub use flag_material::{FlagMaterial, FlagMaterialConfig, FlagTeam, FlagVertex};

// Re-export bridge material types (Phase 2: Chain Bridge Material Shaders)
pub use bridge_materials::{
    ChainMetalConfig, ChainMetalMaterial, WoodPlankConfig, WoodPlankMaterial,
};

// Re-export point light types (Phase 2: Torch Lighting System)
pub use point_lights::{
    LIGHT_COUNT_BUFFER_SIZE, MAX_POINT_LIGHTS, POINT_LIGHT_BUFFER_SIZE, PointLight,
    PointLightManager,
};

// Re-export particle system types (Phase 2: Ember/Ash Particle System)
pub use particles::{
    GPU_PARTICLE_SIZE, GpuParticle, MAX_PARTICLES, PARTICLE_BUFFER_SIZE, PARTICLE_UNIFORMS_SIZE,
    Particle, ParticleSystem, ParticleUniforms,
};

// Re-export SDF baker types
pub use sdf_baker::{BrickCache, MAX_BAKED_SDFS, SDF_RESOLUTION};

// Re-export tile-based culling types
pub use culling::{
    MAX_ENTITIES_PER_TILE, TILE_SIZE, TILES_X_1080P, TILES_Y_1080P, TOTAL_TILES_1080P, TileBuffer,
    TileBufferHeader, TileData,
};

// Re-export bake queue types for entity baking on spawn (US-023)
pub use bake_queue::{
    BakeJob, BakeQueue, BakeState, EntityId, MAX_BAKES_PER_FRAME, NoiseParams, TRANSITION_DURATION,
};

// Re-export rebake tracker types for entity re-baking on transform change (US-024)
pub use rebake_tracker::{DirtyEntity, RebakeTracker, ShapeParams};

// Re-export froxel configuration types for froxel-based culling (US-028)
pub use froxel_config::{
    FROXEL_DEPTH_SLICES, FROXEL_TILES_X, FROXEL_TILES_Y, MAX_SDFS_PER_FROXEL, TOTAL_FROXELS,
    depth_slice_bounds,
};

// Re-export froxel buffer types for froxel GPU data (US-029)
pub use froxel_buffers::{
    FROXEL_BOUNDS_BUFFER_SIZE, FROXEL_BOUNDS_SIZE, FROXEL_SDF_LIST_BUFFER_SIZE,
    FROXEL_SDF_LIST_SIZE, FroxelBounds, FroxelBoundsBuffer, FroxelSDFList, FroxelSDFListBuffer,
    create_froxel_bounds_buffer, create_froxel_sdf_list_buffer, write_froxel_bounds,
    write_froxel_sdf_lists,
};

// Re-export froxel bounds calculation types for perspective projection (US-030)
pub use froxel_bounds::{CameraProjection, FroxelBoundsTracker, calculate_froxel_bounds};

// Re-export adaptive step function for distance-based ray marching (US-032)
pub use adaptive_step::base_step_for_distance;

// Re-export compute pipeline infrastructure (US-0M01)
pub use compute_pipelines::ComputePipelines;

// Re-export froxel clear dispatcher (US-0M04)
pub use froxel_dispatch::{dispatch_froxel_assign, dispatch_froxel_clear};

// Re-export SDF bake dispatcher types (US-0M03)
pub use sdf_bake_dispatch::{FallbackState, GpuBakeParams, SdfBakeDispatcher};

// Re-export froxel assignment types for SDF-to-froxel culling (US-033)
pub use froxel_assignment::{
    ASSIGNMENT_UNIFORMS_SIZE, AssignmentUniforms, MAX_SDF_COUNT, SDF_BOUNDS_BUFFER_SIZE,
    SDF_BOUNDS_SIZE, SdfBounds, SdfBoundsBuffer, create_assignment_bind_group,
    create_assignment_bind_group_layout, create_assignment_buffers, write_assignment_uniforms,
    write_sdf_bounds,
};

// Re-export CPU-side froxel assignment fallback (US-0M10)
pub use froxel_cpu::assign_sdfs_to_froxels;

// Re-export sky bake dispatch types (US-0S04)
pub use sky_bake_dispatch::{SkyBakePipeline, dispatch_sky_bake};

// Re-export tile culling dispatch types (US-0M06)
pub use tile_cull_dispatch::{TileCullUniforms, dispatch_tile_culling};

// Re-export hex-prism voxel types (US-002, US-008, US-012)
// Note: Only exporting types that currently exist. Additional exports like
// HexPrismVertex, axial_to_world, world_to_axial, materials will be added by US-008.
pub use crate::physics::collision::HitInfo;
pub use hex_prism::materials as hex_prism_materials;
pub use hex_prism::{HexPrism, HexPrismGrid, HexPrismVertex, axial_to_world, world_to_axial};

// Re-export building block types (Phase 2: Advanced Building System)
pub use building_blocks::{
    AABB,
    BlockVertex,
    BuildingBlock,
    BuildingBlockManager,
    BuildingBlockShape,
    sdf_arch,
    // SDF primitives
    sdf_box,
    sdf_cylinder,
    sdf_dome,
    sdf_intersection,
    // SDF operations
    sdf_smooth_union,
    sdf_sphere,
    sdf_subtraction,
    sdf_union,
    sdf_wedge,
};

// Re-export building physics types (Phase 5: Realistic Building Physics)
pub use building_physics::{BlockPhysicsState, BuildingPhysics, PhysicsConfig};

// Re-export Marching Cubes types (Phase 3: SDF-to-Mesh Conversion)
pub use marching_cubes::{MarchingCubes, generate_merged_mesh};

// Re-export SDF operations types (Phase 3: Merge Workflow)
pub use sdf_operations::{
    DoubleClickDetector, MergeState, MergeWorkflowManager, MergedMesh,
    intersection as sdf_intersection_op, smooth_intersection, smooth_subtraction,
    smooth_union as sdf_smooth_union_op, subtraction as sdf_subtraction_op, union as sdf_union_op,
};

// Re-export sculpting types (Phase 4: Extrusion and Edge Pulling)
pub use sculpting::{
    EdgeSelection, ExtrusionOperation, ExtrusionStep, FaceDirection, FaceSelection, SculptMode,
    SculptState, SculptingManager, SelectionType, VertexSelection,
};

// Re-export material system types (Phase 2: Material System Coordinator)
pub use material_system::{
    MaterialEntry, MaterialSystem, MaterialType, SceneConfig, SceneUniforms,
};

// Re-export core rendering infrastructure types
pub use gpu_context::{GpuContext, GpuContextConfig};
pub use mesh_pass::{MeshBuffer, MeshRenderPass, MeshUniforms, MeshVertex, draw_mesh_buffer};
pub use render_pass::{
    FrameContext, RenderContext, RenderPass, RenderPassManager, RenderPassPriority,
};
pub use scene_coordinator::{CameraState, SceneCoordinator};
pub use ui_pass::{UiComponent, UiMesh, UiRenderPass, UiVertex};
