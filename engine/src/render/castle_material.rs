//! Castle Stone Material Module
//!
//! Procedural medieval castle stone shader with:
//! - Brick/block pattern with mortar lines
//! - Grime darkening near ground level
//! - Warm torch bounce lighting with flicker
//! - Lambert diffuse lighting from sun
//!
//! # Usage
//!
//! ```rust,ignore
//! // Initialize
//! let material = CastleMaterial::new(&device, surface_format);
//!
//! // Each frame
//! material.update(&queue, &camera, time, &config);
//!
//! // Render (bind during mesh rendering)
//! material.bind(&mut render_pass);
//! ```

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// Castle stone material configuration
#[derive(Clone, Copy, Debug)]
pub struct CastleMaterialConfig {
    /// Sun direction (normalized)
    pub sun_dir: Vec3,
    /// Sun light intensity (default: 1.0)
    pub sun_intensity: f32,
    /// Torch color (default: warm orange)
    pub torch_color: Vec3,
    /// Torch light strength (default: 0.6)
    pub torch_strength: f32,
    /// Ambient light strength (default: 0.3)
    pub ambient_strength: f32,
    /// Fog density (default: 0.5)
    pub fog_density: f32,
    /// Fog color
    pub fog_color: Vec3,
    /// Base stone color
    pub stone_color: Vec3,
    /// Mortar color between bricks
    pub mortar_color: Vec3,
    /// Grime strength - how much darkening near ground (default: 0.12)
    pub grime_strength: f32,
    /// Brick pattern scale (default: 1.2)
    pub brick_scale: f32,
}

impl Default for CastleMaterialConfig {
    fn default() -> Self {
        Self {
            sun_dir: Vec3::new(0.5, 0.7, 0.3).normalize(),
            sun_intensity: 1.0,
            torch_color: Vec3::new(1.0, 0.6, 0.2), // Warm orange
            torch_strength: 0.6,
            ambient_strength: 0.3,
            fog_density: 0.5,
            fog_color: Vec3::new(0.12, 0.1, 0.08),
            stone_color: Vec3::new(0.45, 0.42, 0.38), // Gray-brown stone
            mortar_color: Vec3::new(0.25, 0.23, 0.2), // Darker mortar
            grime_strength: 0.12,
            brick_scale: 1.2,
        }
    }
}

impl CastleMaterialConfig {
    /// Create a preset for dark dungeon walls
    pub fn dungeon() -> Self {
        Self {
            sun_dir: Vec3::new(0.2, 0.3, -0.9).normalize(),
            sun_intensity: 0.3,
            torch_color: Vec3::new(1.0, 0.5, 0.15),
            torch_strength: 1.2,
            ambient_strength: 0.15,
            fog_density: 0.8,
            fog_color: Vec3::new(0.05, 0.05, 0.07),
            stone_color: Vec3::new(0.35, 0.33, 0.3),
            mortar_color: Vec3::new(0.18, 0.16, 0.14),
            grime_strength: 0.2,
            brick_scale: 1.0,
        }
    }

    /// Create a preset for sunlit castle exterior
    pub fn castle_exterior() -> Self {
        Self {
            sun_dir: Vec3::new(0.4, 0.8, 0.2).normalize(),
            sun_intensity: 1.2,
            torch_color: Vec3::new(1.0, 0.7, 0.3),
            torch_strength: 0.2,
            ambient_strength: 0.4,
            fog_density: 0.3,
            fog_color: Vec3::new(0.5, 0.55, 0.6),
            stone_color: Vec3::new(0.55, 0.52, 0.45),
            mortar_color: Vec3::new(0.3, 0.28, 0.25),
            grime_strength: 0.08,
            brick_scale: 1.5,
        }
    }

    /// Create a preset for apocalyptic arena (matching battle arena atmosphere)
    pub fn battle_arena() -> Self {
        Self {
            sun_dir: Vec3::new(0.0, 0.15, -1.0).normalize(),
            sun_intensity: 0.4,
            torch_color: Vec3::new(1.0, 0.4, 0.1), // Lava-like orange
            torch_strength: 1.0,
            ambient_strength: 0.2,
            fog_density: 0.6,
            fog_color: Vec3::new(0.35, 0.15, 0.1), // Orange-brown fog
            stone_color: Vec3::new(0.4, 0.35, 0.32),
            mortar_color: Vec3::new(0.2, 0.15, 0.12),
            grime_strength: 0.15,
            brick_scale: 1.2,
        }
    }
}

/// GPU uniform buffer layout (must match WGSL struct)
/// Using scalar fields for all vec3 to ensure proper alignment
/// Total size: 176 bytes (aligned to 16)
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CastleUniforms {
    view_proj: [[f32; 4]; 4],         // 64 bytes (offset 0)
    camera_pos_x: f32,                // 4 bytes (offset 64)
    camera_pos_y: f32,                // 4 bytes (offset 68)
    camera_pos_z: f32,                // 4 bytes (offset 72)
    time: f32,                        // 4 bytes (offset 76)
    sun_dir_x: f32,                   // 4 bytes (offset 80)
    sun_dir_y: f32,                   // 4 bytes (offset 84)
    sun_dir_z: f32,                   // 4 bytes (offset 88)
    sun_intensity: f32,               // 4 bytes (offset 92)
    torch_color_r: f32,               // 4 bytes (offset 96)
    torch_color_g: f32,               // 4 bytes (offset 100)
    torch_color_b: f32,               // 4 bytes (offset 104)
    torch_strength: f32,              // 4 bytes (offset 108)
    ambient_strength: f32,            // 4 bytes (offset 112)
    fog_density: f32,                 // 4 bytes (offset 116)
    fog_color_r: f32,                 // 4 bytes (offset 120)
    fog_color_g: f32,                 // 4 bytes (offset 124)
    fog_color_b: f32,                 // 4 bytes (offset 128)
    stone_color_r: f32,               // 4 bytes (offset 132)
    stone_color_g: f32,               // 4 bytes (offset 136)
    stone_color_b: f32,               // 4 bytes (offset 140)
    mortar_color_r: f32,              // 4 bytes (offset 144)
    mortar_color_g: f32,              // 4 bytes (offset 148)
    mortar_color_b: f32,              // 4 bytes (offset 152)
    grime_strength: f32,              // 4 bytes (offset 156)
    brick_scale: f32,                 // 4 bytes (offset 160)
    _pad1: f32,                       // 4 bytes (offset 164) - padding
    _pad2: f32,                       // 4 bytes (offset 168) - padding
    _pad3: f32,                       // 4 bytes (offset 172) - align to 176
}

// Verify struct size at compile time
const _: () = assert!(std::mem::size_of::<CastleUniforms>() == 176);

/// Castle stone material renderer
pub struct CastleMaterial {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    config: CastleMaterialConfig,
}

impl CastleMaterial {
    /// Create a new castle material with default configuration
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self::with_config(device, surface_format, CastleMaterialConfig::default())
    }

    /// Create a new castle material with custom configuration
    pub fn with_config(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        config: CastleMaterialConfig,
    ) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Castle Stone Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/castle_stone.wgsl").into(),
            ),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Castle Stone Uniform Buffer"),
            size: std::mem::size_of::<CastleUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Castle Stone Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Castle Stone Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Castle Stone Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Vertex buffer layout for mesh with position and normal
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: 24, // 6 floats * 4 bytes = 24 bytes per vertex
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // Normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 12,
                    shader_location: 1,
                },
            ],
        };

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Castle Stone Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        println!("[CastleMaterial] Initialized castle stone material");

        Self {
            pipeline,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            config,
        }
    }

    /// Get mutable access to the configuration
    pub fn config_mut(&mut self) -> &mut CastleMaterialConfig {
        &mut self.config
    }

    /// Get the current configuration
    pub fn config(&self) -> &CastleMaterialConfig {
        &self.config
    }

    /// Set a new configuration
    pub fn set_config(&mut self, config: CastleMaterialConfig) {
        self.config = config;
    }

    /// Get the bind group layout for external use
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Get the render pipeline
    pub fn pipeline(&self) -> &wgpu::RenderPipeline {
        &self.pipeline
    }

    /// Update uniform buffer with current camera and time
    pub fn update(
        &self,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        camera_pos: Vec3,
        time: f32,
    ) {
        let uniforms = CastleUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos_x: camera_pos.x,
            camera_pos_y: camera_pos.y,
            camera_pos_z: camera_pos.z,
            time,
            sun_dir_x: self.config.sun_dir.x,
            sun_dir_y: self.config.sun_dir.y,
            sun_dir_z: self.config.sun_dir.z,
            sun_intensity: self.config.sun_intensity,
            torch_color_r: self.config.torch_color.x,
            torch_color_g: self.config.torch_color.y,
            torch_color_b: self.config.torch_color.z,
            torch_strength: self.config.torch_strength,
            ambient_strength: self.config.ambient_strength,
            fog_density: self.config.fog_density,
            fog_color_r: self.config.fog_color.x,
            fog_color_g: self.config.fog_color.y,
            fog_color_b: self.config.fog_color.z,
            stone_color_r: self.config.stone_color.x,
            stone_color_g: self.config.stone_color.y,
            stone_color_b: self.config.stone_color.z,
            mortar_color_r: self.config.mortar_color.x,
            mortar_color_g: self.config.mortar_color.y,
            mortar_color_b: self.config.mortar_color.z,
            grime_strength: self.config.grime_strength,
            brick_scale: self.config.brick_scale,
            _pad1: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Bind the material for rendering
    /// Call this before drawing meshes that use this material
    pub fn bind<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
    }

    /// Get the bind group for external use
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
