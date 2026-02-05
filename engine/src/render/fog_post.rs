//! Fog Post-Process Pass Module
//!
//! Fullscreen post-process fog that applies depth-based atmospheric fog:
//! - Distance fog (exponential falloff with distance from camera)
//! - Height fog (thicker near ground level, thinner at elevation)
//! - World position reconstruction from depth buffer
//!
//! # Usage
//!
//! ```rust,ignore
//! // Initialize
//! let fog_pass = FogPostPass::new(&device, surface_format);
//!
//! // Each frame - update with camera matrices
//! fog_pass.update(&queue, &camera, &config);
//!
//! // Render (reads scene color + depth, outputs fogged color)
//! fog_pass.render(&mut encoder, &scene_color_view, &depth_view, &output_view);
//! ```

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// Fog post-pass configuration
#[derive(Clone, Copy, Debug)]
pub struct FogPostConfig {
    /// Fog color (stormy purple-brown atmosphere)
    pub fog_color: Vec3,
    /// Distance fog density (0.015..0.04) - higher = denser fog
    pub density: f32,
    /// Y level where height fog starts (below this is foggier)
    pub height_fog_start: f32,
    /// Height fog density (0.05..0.15) - higher = thicker ground fog
    pub height_fog_density: f32,
}

impl Default for FogPostConfig {
    fn default() -> Self {
        Self {
            fog_color: Vec3::new(0.55, 0.45, 0.70), // Stormy purple
            density: 0.025,
            height_fog_start: 2.0,
            height_fog_density: 0.08,
        }
    }
}

impl FogPostConfig {
    /// Create a preset for the battle arena atmosphere
    /// Matches the stormy purple-brown environment from concept art
    pub fn battle_arena() -> Self {
        Self {
            fog_color: Vec3::new(0.6, 0.4, 0.55), // Warmer purple with more saturation
            density: 0.008,                        // Much lighter distance fog
            height_fog_start: 2.0,                 // Lower start
            height_fog_density: 0.04,              // Lighter height fog
        }
    }

    /// Create a preset for heavy fog (reduced visibility)
    pub fn heavy() -> Self {
        Self {
            fog_color: Vec3::new(0.45, 0.38, 0.55),
            density: 0.04,
            height_fog_start: 8.0,
            height_fog_density: 0.15,
        }
    }

    /// Create a preset for light atmospheric haze
    pub fn light() -> Self {
        Self {
            fog_color: Vec3::new(0.6, 0.5, 0.7),
            density: 0.012,
            height_fog_start: 0.0,
            height_fog_density: 0.03,
        }
    }
}

/// GPU uniform buffer layout (must match WGSL struct FogParams)
/// Total size: 112 bytes
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FogUniforms {
    fog_color: [f32; 3],       // 12 bytes (offset 0)
    density: f32,              // 4 bytes (offset 12) - total 16
    height_fog_start: f32,     // 4 bytes (offset 16)
    height_fog_density: f32,   // 4 bytes (offset 20)
    _pad0: [f32; 2],           // 8 bytes (offset 24) - align to 32
    inv_view_proj: [[f32; 4]; 4], // 64 bytes (offset 32) - total 96
    camera_pos: [f32; 3],      // 12 bytes (offset 96)
    _pad1: f32,                // 4 bytes (offset 108) - total 112
}

// Verify struct size at compile time
const _: () = assert!(std::mem::size_of::<FogUniforms>() == 112);

/// Fog Post-Process Pass renderer
/// Applies depth-based distance and height fog to the scene
pub struct FogPostPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    config: FogPostConfig,
}

impl FogPostPass {
    /// Create a new fog post-process pass
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self::with_config(device, surface_format, FogPostConfig::default())
    }

    /// Create a new fog post-process pass with custom configuration
    pub fn with_config(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        config: FogPostConfig,
    ) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Fog Post-Process Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../../../shaders/fog_post.wgsl").into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Fog Post-Process Uniform Buffer"),
            size: std::mem::size_of::<FogUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create sampler for scene texture
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Fog Post-Process Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout
        // Binding 0: FogParams uniform buffer
        // Binding 1: Scene color texture
        // Binding 2: Sampler
        // Binding 3: Depth texture
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Fog Post-Process Bind Group Layout"),
            entries: &[
                // Uniform buffer
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
                // Scene color texture
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Depth texture
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Fog Post-Process Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Fog Post-Process Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[], // Fullscreen triangle, no vertex buffer
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None, // No blending, fog overwrites
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for fullscreen triangle
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None, // Post-process doesn't write depth
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        println!("[FogPostPass] Initialized depth-based fog post-process");

        Self {
            pipeline,
            uniform_buffer,
            bind_group_layout,
            sampler,
            config,
        }
    }

    /// Get mutable access to the configuration
    pub fn config_mut(&mut self) -> &mut FogPostConfig {
        &mut self.config
    }

    /// Get the current configuration
    pub fn config(&self) -> &FogPostConfig {
        &self.config
    }

    /// Set a new configuration
    pub fn set_config(&mut self, config: FogPostConfig) {
        self.config = config;
    }

    /// Update uniform buffer with current camera matrices
    pub fn update(
        &self,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        camera_pos: Vec3,
    ) {
        let inv_view_proj = view_proj.inverse();

        let uniforms = FogUniforms {
            fog_color: self.config.fog_color.to_array(),
            density: self.config.density,
            height_fog_start: self.config.height_fog_start,
            height_fog_density: self.config.height_fog_density,
            _pad0: [0.0; 2],
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            camera_pos: camera_pos.to_array(),
            _pad1: 0.0,
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Create a bind group for the given scene and depth textures
    pub fn create_bind_group(
        &self,
        device: &wgpu::Device,
        scene_color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Fog Post-Process Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(scene_color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
            ],
        })
    }

    /// Record render commands into an existing render pass
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, bind_group: &'a wgpu::BindGroup) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        // Draw fullscreen triangle (3 vertices, no vertex buffer)
        render_pass.draw(0..3, 0..1);
    }

    /// Create a render pass and render the fog effect
    /// Convenience method that handles render pass creation
    pub fn render_to_view(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        scene_color_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        output_view: &wgpu::TextureView,
    ) {
        let bind_group = self.create_bind_group(device, scene_color_view, depth_view);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Fog Post-Process Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Preserve existing content (we're post-processing)
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Get the bind group layout for external bind group creation
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }
}
