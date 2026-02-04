//! Compute Pipeline Infrastructure Module (US-0M01)
//!
//! Creates and manages all compute pipelines used by the engine:
//! - SDF bake: Bakes SDF equations into 3D textures
//! - Froxel clear: Clears froxel SDF lists each frame
//! - Froxel assign: Assigns SDFs to froxels
//! - Tile culling: Builds per-tile entity lists

use crate::render::shader_loader::{load_shader_file, create_shader_module};

/// Paths to the compute shader files (relative to project root).
pub mod shader_paths {
    pub const SDF_BAKE: &str = "shaders/sdf_bake.wgsl";
    pub const FROXEL_CLEAR: &str = "shaders/froxel_clear.wgsl";
    pub const FROXEL_ASSIGN: &str = "shaders/froxel_assign.wgsl";
    pub const TILE_CULLING: &str = "shaders/tile_culling.wgsl";
}

/// Holds all compute pipelines and their associated bind group layouts.
pub struct ComputePipelines {
    /// SDF bake pipeline: bakes SDF equations into 64³ 3D textures.
    pub sdf_bake_pipeline: wgpu::ComputePipeline,
    /// Bind group layout for the SDF bake pipeline.
    pub sdf_bake_bind_group_layout: wgpu::BindGroupLayout,

    /// Froxel clear pipeline: resets froxel SDF counts to zero.
    pub froxel_clear_pipeline: wgpu::ComputePipeline,
    /// Bind group layout for the froxel clear pipeline.
    pub froxel_clear_bind_group_layout: wgpu::BindGroupLayout,

    /// Froxel assign pipeline: assigns SDFs to intersecting froxels.
    pub froxel_assign_pipeline: wgpu::ComputePipeline,
    /// Bind group layout for the froxel assign pipeline.
    pub froxel_assign_bind_group_layout: wgpu::BindGroupLayout,

    /// Tile culling pipeline: builds per-tile entity lists.
    pub tile_culling_pipeline: wgpu::ComputePipeline,
    /// Bind group layout for the tile culling pipeline.
    pub tile_culling_bind_group_layout: wgpu::BindGroupLayout,
}

impl ComputePipelines {
    /// Create all compute pipelines from their WGSL shader files.
    ///
    /// # Panics
    /// Panics if any shader file cannot be loaded.
    pub fn new(device: &wgpu::Device) -> Self {
        // Load all shader sources
        let sdf_bake_src = load_shader_file(shader_paths::SDF_BAKE)
            .expect("Failed to load sdf_bake.wgsl");
        let froxel_clear_src = load_shader_file(shader_paths::FROXEL_CLEAR)
            .expect("Failed to load froxel_clear.wgsl");
        let froxel_assign_src = load_shader_file(shader_paths::FROXEL_ASSIGN)
            .expect("Failed to load froxel_assign.wgsl");
        let tile_culling_src = load_shader_file(shader_paths::TILE_CULLING)
            .expect("Failed to load tile_culling.wgsl");

        // Create shader modules
        let sdf_bake_module = create_shader_module(device, "sdf_bake", &sdf_bake_src);
        let froxel_clear_module = create_shader_module(device, "froxel_clear", &froxel_clear_src);
        let froxel_assign_module = create_shader_module(device, "froxel_assign", &froxel_assign_src);
        let tile_culling_module = create_shader_module(device, "tile_culling", &tile_culling_src);

        // --- SDF Bake ---
        // @group(0) @binding(0): uniform BakeParams
        // @group(0) @binding(1): storage buffer (output SDF data)
        let sdf_bake_entries = [
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: Some(std::num::NonZeroU64::new(112).unwrap()),
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        let sdf_bake_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("sdf_bake_bind_group_layout"),
            entries: &sdf_bake_entries,
        });

        let sdf_bake_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sdf_bake_pipeline_layout"),
            bind_group_layouts: &[&sdf_bake_bind_group_layout],
            push_constant_ranges: &[],
        });

        let sdf_bake_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("sdf_bake_pipeline"),
            layout: Some(&sdf_bake_pipeline_layout),
            module: &sdf_bake_module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // --- Froxel Clear ---
        // @group(0) @binding(0): storage<read_write> FroxelSDFListBuffer
        let froxel_clear_entries = [
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        let froxel_clear_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("froxel_clear_bind_group_layout"),
            entries: &froxel_clear_entries,
        });

        let froxel_clear_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("froxel_clear_pipeline_layout"),
            bind_group_layouts: &[&froxel_clear_bind_group_layout],
            push_constant_ranges: &[],
        });

        let froxel_clear_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("froxel_clear_pipeline"),
            layout: Some(&froxel_clear_pipeline_layout),
            module: &froxel_clear_module,
            entry_point: Some("cs_clear_froxels"),
            compilation_options: Default::default(),
            cache: None,
        });

        // --- Froxel Assign ---
        // @group(0) @binding(0): storage<read> SdfBoundsBuffer
        // @group(0) @binding(1): storage<read> FroxelBoundsBuffer
        // @group(0) @binding(2): storage<read_write> FroxelSDFListBuffer
        // @group(0) @binding(3): uniform AssignmentUniforms
        let froxel_assign_entries = [
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        let froxel_assign_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("froxel_assign_bind_group_layout"),
            entries: &froxel_assign_entries,
        });

        let froxel_assign_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("froxel_assign_pipeline_layout"),
            bind_group_layouts: &[&froxel_assign_bind_group_layout],
            push_constant_ranges: &[],
        });

        let froxel_assign_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("froxel_assign_pipeline"),
            layout: Some(&froxel_assign_pipeline_layout),
            module: &froxel_assign_module,
            entry_point: Some("cs_assign_sdfs"),
            compilation_options: Default::default(),
            cache: None,
        });

        // --- Tile Culling ---
        // @group(0) @binding(0): storage<read_write> TileBuffer
        // @group(0) @binding(1): storage<read> EntityBuffer
        // @group(0) @binding(2): uniform CullingUniforms
        let tile_culling_entries = [
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ];
        let tile_culling_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tile_culling_bind_group_layout"),
            entries: &tile_culling_entries,
        });

        let tile_culling_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("tile_culling_pipeline_layout"),
            bind_group_layouts: &[&tile_culling_bind_group_layout],
            push_constant_ranges: &[],
        });

        let tile_culling_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("tile_culling_pipeline"),
            layout: Some(&tile_culling_pipeline_layout),
            module: &tile_culling_module,
            entry_point: Some("cs_cull_entities"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Validate compute pipeline bindings against shader expectations (US-P2-014)
        let compute_mismatches = super::binding_validator::validate_compute_bindings(
            &sdf_bake_entries,
            &froxel_clear_entries,
            &froxel_assign_entries,
            &tile_culling_entries,
        );
        if compute_mismatches > 0 {
            eprintln!("[ComputePipelines] {} compute binding mismatch(es) detected!", compute_mismatches);
        }

        Self {
            sdf_bake_pipeline,
            sdf_bake_bind_group_layout,
            froxel_clear_pipeline,
            froxel_clear_bind_group_layout,
            froxel_assign_pipeline,
            froxel_assign_bind_group_layout,
            tile_culling_pipeline,
            tile_culling_bind_group_layout,
        }
    }
}
