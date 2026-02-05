//! Render Pipeline Module
//!
//! Contains the core RenderState struct that manages wgpu rendering setup.
//! Extracted from sdf_core_test.rs for reuse across the engine.

use std::sync::Arc;
use wgpu;
use winit::window::Window;

use super::compute_pipelines::ComputePipelines;
use super::culling::TileBufferHeader;
use super::froxel_buffers::{
    FroxelBoundsBuffer, FroxelSDFListBuffer, create_froxel_bounds_buffer,
    create_froxel_sdf_list_buffer,
};
use super::sdf_baker::BrickCache;
use super::sky_bake_dispatch::SkyBakePipeline;
use super::sky_cubemap::SkyCubemap;

/// Core render state holding all wgpu resources.
///
/// This struct owns the device, queue, surface, and pipeline configuration
/// needed for GPU rendering with VSync disabled for maximum performance.
pub struct RenderState {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub pipeline: wgpu::RenderPipeline,
    pub uniform_buffer: wgpu::Buffer,
    pub entity_buffer: wgpu::Buffer,
    pub sky_buffer: wgpu::Buffer,
    pub sky_cubemap: SkyCubemap,
    pub froxel_bounds_buffer: wgpu::Buffer,
    pub froxel_sdf_list_buffer: wgpu::Buffer,
    pub tile_data_buffer: wgpu::Buffer,
    pub compute_pipelines: ComputePipelines,
    pub bind_group: wgpu::BindGroup,
    pub brick_cache: BrickCache,
    pub sky_bake_pipeline: Option<SkyBakePipeline>,
}

/// Configuration for initializing the render pipeline.
pub struct RenderConfig {
    /// Window width in pixels
    pub width: u32,
    /// Window height in pixels
    pub height: u32,
    /// Enable VSync (false = Immediate present mode for uncapped FPS)
    pub vsync: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            vsync: false, // VSync OFF by default for maximum FPS
        }
    }
}

/// Detect if the system will use a software renderer (llvmpipe, lavapipe, etc.)
/// Returns true if software renderer is detected, false if real GPU available.
pub fn detect_software_renderer() -> bool {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });

    let adapter_result =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: None,
            force_fallback_adapter: false,
        }));

    match adapter_result {
        Ok(adapter) => {
            let info = adapter.get_info();
            let name_lower = info.name.to_lowercase();

            // Check for software renderer indicators
            let is_software = name_lower.contains("llvmpipe")
                || name_lower.contains("lavapipe")
                || name_lower.contains("swiftshader")
                || info.device_type == wgpu::DeviceType::Cpu;

            if is_software {
                println!(
                    "[RenderState] Software renderer detected ({}) - using 1280x720 and Mailbox",
                    info.name
                );
            } else {
                println!(
                    "[RenderState] GPU detected ({}) - using 1920x1080 and Immediate",
                    info.name
                );
            }

            is_software
        }
        Err(_) => {
            println!("[RenderState] No adapter found, assuming software renderer");
            true
        }
    }
}

/// Get recommended resolution based on whether software renderer is detected.
pub fn get_recommended_resolution(is_software: bool) -> (u32, u32) {
    if is_software {
        (1280, 720) // Lower resolution for software renderer
    } else {
        (1920, 1080) // Full HD for real GPU
    }
}

impl RenderState {
    /// Initialize the render pipeline with the given window and configuration.
    ///
    /// This sets up:
    /// - wgpu instance, adapter, device, and queue
    /// - Surface configuration (VSync off by default)
    /// - Shader module from the provided source
    /// - Uniform, entity, and sky buffers
    /// - Bind group layout and render pipeline
    pub fn new(
        window: Arc<Window>,
        config: RenderConfig,
        shader_source: &str,
        uniform_buffer_size: u64,
        entity_buffer_size: u64,
        sky_buffer_size: u64,
    ) -> Self {
        let size = window.inner_size();
        let width = if size.width > 0 {
            size.width
        } else {
            config.width
        };
        let height = if size.height > 0 {
            size.height
        } else {
            config.height
        };

        println!("[RenderState] Window size: {}x{}", width, height);

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(window.clone()).unwrap();

        // Request adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find suitable adapter");

        let adapter_info = adapter.get_info();
        let name_lower = adapter_info.name.to_lowercase();

        // Detect if this is a software renderer
        let is_software_renderer = name_lower.contains("llvmpipe")
            || name_lower.contains("lavapipe")
            || name_lower.contains("swiftshader")
            || adapter_info.device_type == wgpu::DeviceType::Cpu;

        let renderer_type = if is_software_renderer {
            "SOFTWARE RENDERER"
        } else {
            "GPU"
        };
        println!(
            "[RenderState] Using adapter: {} ({:?}) [{}]",
            adapter_info.name, adapter_info.backend, renderer_type
        );

        // Request device
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Magic Engine Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .expect("Failed to create device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        println!("[RenderState] Surface format: {:?}", surface_format);

        // Choose present mode based on VSync setting and renderer capabilities
        let present_mode = if config.vsync {
            wgpu::PresentMode::AutoVsync
        } else if is_software_renderer {
            // Software renderers typically don't support Immediate, use Mailbox
            if surface_caps
                .present_modes
                .contains(&wgpu::PresentMode::Mailbox)
            {
                println!("[RenderState] Using Mailbox present mode (software renderer)");
                wgpu::PresentMode::Mailbox
            } else {
                println!("[RenderState] Mailbox not available, falling back to Fifo");
                wgpu::PresentMode::Fifo
            }
        } else {
            // Real GPU - try Immediate first (uncapped FPS), fall back to Mailbox
            if surface_caps
                .present_modes
                .contains(&wgpu::PresentMode::Immediate)
            {
                println!("[RenderState] Using Immediate present mode (uncapped FPS)");
                wgpu::PresentMode::Immediate
            } else if surface_caps
                .present_modes
                .contains(&wgpu::PresentMode::Mailbox)
            {
                println!("[RenderState] Immediate not available, using Mailbox");
                wgpu::PresentMode::Mailbox
            } else {
                println!("[RenderState] Using Fifo present mode (VSync)");
                wgpu::PresentMode::Fifo
            }
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Main Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: uniform_buffer_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create entity storage buffer
        let entity_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Entity Buffer"),
            size: entity_buffer_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sky settings buffer
        let sky_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Sky Buffer"),
            size: sky_buffer_size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let group0_entries = [
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 3,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 4,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 5,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 8,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::Cube,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 9,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ];
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Bind Group Layout"),
            entries: &group0_entries,
        });

        // Create brick cache (SSBO-based SDF storage for @group(1))
        let brick_cache = BrickCache::new(&device);

        // Create sky cubemap
        let sky_cubemap = SkyCubemap::new(&device, 512);

        // Create froxel buffers
        let froxel_bounds_buffer = create_froxel_bounds_buffer(&device);
        let froxel_sdf_list_buffer = create_froxel_sdf_list_buffer(&device);

        // Create tile data buffer for @group(0) @binding(5)
        // TileBuffer: header (16 bytes) + tiles array (~1.06 MB at 1080p)
        let header = TileBufferHeader::for_resolution(config.width, config.height);
        let tile_count = header.total_tiles as usize;
        let tile_data_size = std::mem::size_of::<TileBufferHeader>() + tile_count * 136; // TileData = 136 bytes each
        let tile_data_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Tile Data Buffer"),
            size: tile_data_size as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create compute pipelines
        let compute_pipelines = ComputePipelines::new(&device);

        // Create bind group
        let cubemap_entries = sky_cubemap.get_bind_group_entries(8);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: entity_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: sky_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: froxel_bounds_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: froxel_sdf_list_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: tile_data_buffer.as_entire_binding(),
                },
                cubemap_entries[0].clone(),
                cubemap_entries[1].clone(),
            ],
        });

        // Create pipeline layout (group 0 = main, group 1 = BrickCache SSBO)
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout, brick_cache.bind_group_layout()],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Validate bind group layouts against expected shader bindings (US-P2-014)
        println!("[BindingValidator] Validating shader bindings against bind group layouts...");
        let group1_entries = [wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        }];
        let render_mismatches =
            super::binding_validator::validate_render_bindings(&group0_entries, &group1_entries);
        if render_mismatches == 0 {
            println!(
                "[BindingValidator] Render pipeline bindings validated OK (group 0 + group 1)"
            );
        } else {
            eprintln!(
                "[BindingValidator] WARNING: {} render binding mismatch(es) found!",
                render_mismatches
            );
        }

        println!(
            "[RenderState] Initialization complete (VSync: {})",
            config.vsync
        );

        Self {
            device,
            queue,
            surface,
            config: surface_config,
            pipeline,
            uniform_buffer,
            entity_buffer,
            sky_buffer,
            sky_cubemap,
            froxel_bounds_buffer,
            froxel_sdf_list_buffer,
            tile_data_buffer,
            compute_pipelines,
            bind_group,
            brick_cache,
            sky_bake_pipeline: None,
        }
    }

    /// Resize the surface when the window size changes.
    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        if new_width > 0 && new_height > 0 {
            self.config.width = new_width;
            self.config.height = new_height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// Get the current surface texture for rendering.
    pub fn get_current_texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    /// Write data to the uniform buffer.
    pub fn write_uniforms<T: bytemuck::Pod>(&self, data: &T) {
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(data));
    }

    /// Write data to the entity buffer.
    pub fn write_entities<T: bytemuck::Pod>(&self, data: &T) {
        self.queue
            .write_buffer(&self.entity_buffer, 0, bytemuck::bytes_of(data));
    }

    /// Write data to the sky buffer.
    pub fn write_sky<T: bytemuck::Pod>(&self, data: &T) {
        self.queue
            .write_buffer(&self.sky_buffer, 0, bytemuck::bytes_of(data));
    }

    /// Write froxel bounds data to the GPU buffer.
    pub fn write_froxel_bounds(&self, data: &FroxelBoundsBuffer) {
        self.queue
            .write_buffer(&self.froxel_bounds_buffer, 0, bytemuck::bytes_of(data));
    }

    /// Write froxel SDF list data to the GPU buffer.
    pub fn write_froxel_sdf_lists(&self, data: &FroxelSDFListBuffer) {
        self.queue
            .write_buffer(&self.froxel_sdf_list_buffer, 0, bytemuck::bytes_of(data));
    }

    /// Create a command encoder for recording GPU commands.
    pub fn create_encoder(&self, label: &str) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some(label) })
    }

    /// Submit encoded commands to the GPU queue.
    pub fn submit(&self, encoder: wgpu::CommandEncoder) {
        self.queue.submit(std::iter::once(encoder.finish()));
    }

    /// Initialize the sky bake pipeline from shader source.
    pub fn init_sky_bake_pipeline(&mut self, sky_bake_source: &str) {
        self.sky_bake_pipeline = Some(SkyBakePipeline::new(&self.device, sky_bake_source));
    }

    /// Dispatch sky bake if pipeline is initialized, rendering all 6 cubemap faces.
    pub fn dispatch_sky_bake(&self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(ref pipeline) = self.sky_bake_pipeline {
            super::sky_bake_dispatch::dispatch_sky_bake(
                encoder,
                &self.device,
                pipeline,
                &self.sky_cubemap,
                &self.sky_buffer,
            );
        }
    }
}
