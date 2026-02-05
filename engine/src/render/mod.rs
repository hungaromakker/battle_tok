//! Render Module
//!
//! This module contains the core rendering infrastructure for the Magic Engine.
//! It provides wgpu-based rendering with VSync-off support for maximum performance.

pub mod adaptive_step;
pub mod binding_validator;
pub mod building_blocks;
pub mod building_physics;
pub mod compute_pipelines;
pub mod marching_cubes;
pub mod sdf_operations;
pub mod sculpting;
pub mod bake_queue;
pub mod culling;
pub mod entities;
pub mod froxel_assignment;
pub mod froxel_bounds;
pub mod froxel_buffers;
pub mod froxel_config;
pub mod froxel_cpu;
pub mod hex_prism;
pub mod instancing;
pub mod pipeline;
pub mod rebake_tracker;
pub mod froxel_dispatch;
pub mod sdf_bake_dispatch;
pub mod sdf_baker;
pub mod shader_loader;
pub mod tile_cull_dispatch;
pub mod sky_bake_dispatch;
pub mod sky;
pub mod sky_cubemap;
pub mod stormy_sky;
pub mod castle_material;
pub mod point_lights;
pub mod uniforms;

// Re-export commonly used types for convenience
pub use pipeline::{RenderConfig, RenderState, detect_software_renderer, get_recommended_resolution};
pub use shader_loader::{create_shader_module, load_shader_file, ShaderSource};
pub use uniforms::{
    EntityBufferData, PlacedEntity, Season, SkySettings, TestUniforms, WeatherType,
    pack_color, ENTITY_COLORS,
};
pub use instancing::{
    CreatureInstance, MAX_CREATURE_INSTANCES, INSTANCE_BUFFER_SIZE,
    create_instance_buffer, create_instance_buffer_init, update_instance_buffer,
    instance_buffer_layout, pack_rgba, unpack_rgba,
};

// Also re-export from entities module for direct access
pub use entities::{
    entity_type, pack_color as pack_color_rgb, unpack_color,
    ENTITY_COLORS as ENTITY_COLOR_PALETTE,
    // Advanced 96-byte entity struct for raymarcher.wgsl
    GpuEntity, GpuEntityBuffer,
};

// Re-export sky rendering types
pub use sky::{CloudTexture, CLOUD_TEXTURE_SIZE};
pub use sky_cubemap::SkyCubemap;
pub use stormy_sky::{StormySky, StormySkyConfig};

// Re-export castle material types (Phase 2: Castle Stone Shader)
pub use castle_material::{CastleMaterial, CastleMaterialConfig};

// Re-export SDF baker types
pub use sdf_baker::{BrickCache, SDF_RESOLUTION, MAX_BAKED_SDFS};

// Re-export tile-based culling types
pub use culling::{
    TileBuffer, TileBufferHeader, TileData,
    TILE_SIZE, MAX_ENTITIES_PER_TILE, TILES_X_1080P, TILES_Y_1080P, TOTAL_TILES_1080P,
};

// Re-export bake queue types for entity baking on spawn (US-023)
pub use bake_queue::{
    BakeQueue, BakeJob, BakeState, NoiseParams, EntityId,
    MAX_BAKES_PER_FRAME, TRANSITION_DURATION,
};

// Re-export rebake tracker types for entity re-baking on transform change (US-024)
pub use rebake_tracker::{
    RebakeTracker, ShapeParams, DirtyEntity,
};

// Re-export froxel configuration types for froxel-based culling (US-028)
pub use froxel_config::{
    FROXEL_TILES_X, FROXEL_TILES_Y, FROXEL_DEPTH_SLICES,
    MAX_SDFS_PER_FROXEL, TOTAL_FROXELS, depth_slice_bounds,
};

// Re-export froxel buffer types for froxel GPU data (US-029)
pub use froxel_buffers::{
    FroxelBounds, FroxelSDFList, FroxelBoundsBuffer, FroxelSDFListBuffer,
    create_froxel_bounds_buffer, create_froxel_sdf_list_buffer,
    write_froxel_bounds, write_froxel_sdf_lists,
    FROXEL_BOUNDS_SIZE, FROXEL_SDF_LIST_SIZE,
    FROXEL_BOUNDS_BUFFER_SIZE, FROXEL_SDF_LIST_BUFFER_SIZE,
};

// Re-export froxel bounds calculation types for perspective projection (US-030)
pub use froxel_bounds::{
    CameraProjection, FroxelBoundsTracker, calculate_froxel_bounds,
};

// Re-export adaptive step function for distance-based ray marching (US-032)
pub use adaptive_step::base_step_for_distance;

// Re-export compute pipeline infrastructure (US-0M01)
pub use compute_pipelines::ComputePipelines;

// Re-export froxel clear dispatcher (US-0M04)
pub use froxel_dispatch::{dispatch_froxel_clear, dispatch_froxel_assign};

// Re-export SDF bake dispatcher types (US-0M03)
pub use sdf_bake_dispatch::{SdfBakeDispatcher, GpuBakeParams, FallbackState};

// Re-export froxel assignment types for SDF-to-froxel culling (US-033)
pub use froxel_assignment::{
    SdfBounds, SdfBoundsBuffer, AssignmentUniforms,
    create_assignment_bind_group_layout, create_assignment_buffers, create_assignment_bind_group,
    write_sdf_bounds, write_assignment_uniforms,
    MAX_SDF_COUNT, SDF_BOUNDS_SIZE, SDF_BOUNDS_BUFFER_SIZE, ASSIGNMENT_UNIFORMS_SIZE,
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
pub use hex_prism::{HexPrism, HexPrismGrid, HexPrismVertex, axial_to_world, world_to_axial};
pub use hex_prism::materials as hex_prism_materials;
pub use crate::physics::collision::HitInfo;

// Re-export building block types (Phase 2: Advanced Building System)
pub use building_blocks::{
    BuildingBlockShape, BuildingBlock, BuildingBlockManager,
    BlockVertex, AABB,
    // SDF primitives
    sdf_box, sdf_cylinder, sdf_sphere, sdf_dome, sdf_arch, sdf_wedge,
    // SDF operations
    sdf_smooth_union, sdf_union, sdf_intersection, sdf_subtraction,
};

// Re-export building physics types (Phase 5: Realistic Building Physics)
pub use building_physics::{
    BuildingPhysics, BlockPhysicsState, PhysicsConfig,
};

// Re-export Marching Cubes types (Phase 3: SDF-to-Mesh Conversion)
pub use marching_cubes::{MarchingCubes, generate_merged_mesh};

// Re-export SDF operations types (Phase 3: Merge Workflow)
pub use sdf_operations::{
    MergeState, MergeWorkflowManager, MergedMesh, DoubleClickDetector,
    smooth_union as sdf_smooth_union_op, union as sdf_union_op,
    intersection as sdf_intersection_op, subtraction as sdf_subtraction_op,
    smooth_subtraction, smooth_intersection,
};

// Re-export sculpting types (Phase 4: Extrusion and Edge Pulling)
pub use sculpting::{
    SculptingManager, SculptMode, SculptState, SelectionType,
    FaceSelection, EdgeSelection, VertexSelection, FaceDirection,
    ExtrusionOperation, ExtrusionStep,
};
