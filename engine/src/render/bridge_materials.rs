//! Bridge Materials Module
//!
//! Material shaders for the chain bridge connecting hex islands:
//! - WoodPlankMaterial: Brown wood with grain variation for walkway planks
//! - ChainMetalMaterial: Metallic steel with Fresnel rim highlights for chain supports
//!
//! # Usage
//!
//! ```rust,ignore
//! // Initialize materials
//! let wood = WoodPlankMaterial::new(&device, surface_format);
//! let chain = ChainMetalMaterial::new(&device, surface_format);
//!
//! // Each frame
//! wood.update(&queue, view_proj, camera_pos, time);
//! chain.update(&queue, view_proj, camera_pos, time);
//!
//! // Render planks
//! wood.bind(&mut render_pass);
//! render_pass.draw(...);
//!
//! // Render chains
//! chain.bind(&mut render_pass);
//! render_pass.draw(...);
//! ```

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

// ============================================================================
// WOOD PLANK MATERIAL
// ============================================================================

/// Wood plank material configuration
#[derive(Clone, Copy, Debug)]
pub struct WoodPlankConfig {
    /// Sun direction (normalized)
    pub sun_dir: Vec3,
    /// Sun light intensity (default: 1.0)
    pub sun_intensity: f32,
    /// Ambient light strength (default: 0.35)
    pub ambient_strength: f32,
    /// Fog density (default: 0.5)
    pub fog_density: f32,
    /// Fog color
    pub fog_color: Vec3,
    /// Base wood color (default: brown)
    pub wood_color: Vec3,
    /// Grain pattern scale (default: 5.0)
    pub grain_scale: f32,
    /// Grain color strength (default: 0.15)
    pub grain_strength: f32,
}

impl Default for WoodPlankConfig {
    fn default() -> Self {
        Self {
            sun_dir: Vec3::new(0.5, 0.7, 0.3).normalize(),
            sun_intensity: 1.0,
            ambient_strength: 0.35,
            fog_density: 0.5,
            fog_color: Vec3::new(0.12, 0.1, 0.08),
            wood_color: Vec3::new(0.38, 0.26, 0.16), // Natural brown wood
            grain_scale: 5.0,
            grain_strength: 0.15,
        }
    }
}

impl WoodPlankConfig {
    /// Create a preset for apocalyptic battle arena (lava-lit)
    pub fn battle_arena() -> Self {
        Self {
            sun_dir: Vec3::new(0.0, 0.15, -1.0).normalize(),
            sun_intensity: 0.4,
            ambient_strength: 0.25,
            fog_density: 0.6,
            fog_color: Vec3::new(0.35, 0.15, 0.1), // Orange-brown fog
            wood_color: Vec3::new(0.35, 0.22, 0.12), // Slightly darker/charred
            grain_scale: 5.0,
            grain_strength: 0.12,
        }
    }

    /// Create a preset for well-lit exterior
    pub fn exterior() -> Self {
        Self {
            sun_dir: Vec3::new(0.4, 0.8, 0.2).normalize(),
            sun_intensity: 1.2,
            ambient_strength: 0.4,
            fog_density: 0.3,
            fog_color: Vec3::new(0.5, 0.55, 0.6),
            wood_color: Vec3::new(0.42, 0.3, 0.18), // Warm brown
            grain_scale: 5.0,
            grain_strength: 0.18,
        }
    }
}

/// GPU uniform buffer layout for wood plank shader
/// Total size: 144 bytes (aligned to 16)
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct WoodPlankUniforms {
    view_proj: [[f32; 4]; 4],         // 64 bytes (offset 0)
    camera_pos_x: f32,                // 4 bytes (offset 64)
    camera_pos_y: f32,                // 4 bytes (offset 68)
    camera_pos_z: f32,                // 4 bytes (offset 72)
    time: f32,                        // 4 bytes (offset 76)
    sun_dir_x: f32,                   // 4 bytes (offset 80)
    sun_dir_y: f32,                   // 4 bytes (offset 84)
    sun_dir_z: f32,                   // 4 bytes (offset 88)
    sun_intensity: f32,               // 4 bytes (offset 92)
    ambient_strength: f32,            // 4 bytes (offset 96)
    fog_density: f32,                 // 4 bytes (offset 100)
    fog_color_r: f32,                 // 4 bytes (offset 104)
    fog_color_g: f32,                 // 4 bytes (offset 108)
    fog_color_b: f32,                 // 4 bytes (offset 112)
    wood_color_r: f32,                // 4 bytes (offset 116)
    wood_color_g: f32,                // 4 bytes (offset 120)
    wood_color_b: f32,                // 4 bytes (offset 124)
    grain_scale: f32,                 // 4 bytes (offset 128)
    grain_strength: f32,              // 4 bytes (offset 132)
    _pad1: f32,                       // 4 bytes (offset 136)
    _pad2: f32,                       // 4 bytes (offset 140)
    _pad3: f32,                       // 4 bytes (offset 144) - align to 16
}

// Verify struct size at compile time (must be multiple of 16)
const _: () = assert!(std::mem::size_of::<WoodPlankUniforms>() == 152);

/// Wood plank material renderer for bridge walkway
pub struct WoodPlankMaterial {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    config: WoodPlankConfig,
}

impl WoodPlankMaterial {
    /// Create a new wood plank material with default configuration
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self::with_config(device, surface_format, WoodPlankConfig::default())
    }

    /// Create a new wood plank material with custom configuration
    pub fn with_config(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        config: WoodPlankConfig,
    ) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Wood Plank Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/wood_plank.wgsl").into(),
            ),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Wood Plank Uniform Buffer"),
            size: std::mem::size_of::<WoodPlankUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Wood Plank Bind Group Layout"),
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
            label: Some("Wood Plank Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Wood Plank Pipeline Layout"),
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
            label: Some("Wood Plank Pipeline"),
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

        println!("[WoodPlankMaterial] Initialized wood plank material");

        Self {
            pipeline,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            config,
        }
    }

    /// Get mutable access to the configuration
    pub fn config_mut(&mut self) -> &mut WoodPlankConfig {
        &mut self.config
    }

    /// Get the current configuration
    pub fn config(&self) -> &WoodPlankConfig {
        &self.config
    }

    /// Set a new configuration
    pub fn set_config(&mut self, config: WoodPlankConfig) {
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
        let uniforms = WoodPlankUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos_x: camera_pos.x,
            camera_pos_y: camera_pos.y,
            camera_pos_z: camera_pos.z,
            time,
            sun_dir_x: self.config.sun_dir.x,
            sun_dir_y: self.config.sun_dir.y,
            sun_dir_z: self.config.sun_dir.z,
            sun_intensity: self.config.sun_intensity,
            ambient_strength: self.config.ambient_strength,
            fog_density: self.config.fog_density,
            fog_color_r: self.config.fog_color.x,
            fog_color_g: self.config.fog_color.y,
            fog_color_b: self.config.fog_color.z,
            wood_color_r: self.config.wood_color.x,
            wood_color_g: self.config.wood_color.y,
            wood_color_b: self.config.wood_color.z,
            grain_scale: self.config.grain_scale,
            grain_strength: self.config.grain_strength,
            _pad1: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Bind the material for rendering
    pub fn bind<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
    }

    /// Get the bind group for external use
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

// ============================================================================
// CHAIN METAL MATERIAL
// ============================================================================

/// Chain metal material configuration
#[derive(Clone, Copy, Debug)]
pub struct ChainMetalConfig {
    /// Sun direction (normalized)
    pub sun_dir: Vec3,
    /// Sun light intensity (default: 1.0)
    pub sun_intensity: f32,
    /// Ambient light strength (default: 0.3)
    pub ambient_strength: f32,
    /// Fog density (default: 0.5)
    pub fog_density: f32,
    /// Fog color
    pub fog_color: Vec3,
    /// Base steel color
    pub steel_color: Vec3,
    /// Fresnel rim shine intensity (default: 0.8)
    pub shine: f32,
    /// Surface roughness 0-1 (default: 0.3)
    pub roughness: f32,
    /// Metallic factor 0-1 (default: 0.9)
    pub metallic: f32,
}

impl Default for ChainMetalConfig {
    fn default() -> Self {
        Self {
            sun_dir: Vec3::new(0.5, 0.7, 0.3).normalize(),
            sun_intensity: 1.0,
            ambient_strength: 0.3,
            fog_density: 0.5,
            fog_color: Vec3::new(0.12, 0.1, 0.08),
            steel_color: Vec3::new(0.55, 0.58, 0.62), // Steel gray
            shine: 0.8, // Strong Fresnel rim
            roughness: 0.3,
            metallic: 0.9,
        }
    }
}

impl ChainMetalConfig {
    /// Create a preset for apocalyptic battle arena
    pub fn battle_arena() -> Self {
        Self {
            sun_dir: Vec3::new(0.0, 0.15, -1.0).normalize(),
            sun_intensity: 0.4,
            ambient_strength: 0.2,
            fog_density: 0.6,
            fog_color: Vec3::new(0.35, 0.15, 0.1), // Orange-brown fog
            steel_color: Vec3::new(0.45, 0.42, 0.4), // Darker, weathered steel
            shine: 1.0, // Strong rim from lava glow
            roughness: 0.4,
            metallic: 0.85,
        }
    }

    /// Create a preset for polished new chains
    pub fn polished() -> Self {
        Self {
            sun_dir: Vec3::new(0.4, 0.8, 0.2).normalize(),
            sun_intensity: 1.2,
            ambient_strength: 0.4,
            fog_density: 0.3,
            fog_color: Vec3::new(0.5, 0.55, 0.6),
            steel_color: Vec3::new(0.65, 0.68, 0.72), // Brighter steel
            shine: 1.2, // Very shiny
            roughness: 0.15,
            metallic: 0.95,
        }
    }

    /// Create a preset for rusted old chains
    pub fn rusted() -> Self {
        Self {
            sun_dir: Vec3::new(0.5, 0.7, 0.3).normalize(),
            sun_intensity: 1.0,
            ambient_strength: 0.35,
            fog_density: 0.5,
            fog_color: Vec3::new(0.2, 0.18, 0.15),
            steel_color: Vec3::new(0.45, 0.35, 0.28), // Rusty brown-gray
            shine: 0.4, // Less shiny due to rust
            roughness: 0.6,
            metallic: 0.6,
        }
    }
}

/// GPU uniform buffer layout for chain metal shader
/// Total size: 144 bytes (aligned to 16)
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ChainMetalUniforms {
    view_proj: [[f32; 4]; 4],         // 64 bytes (offset 0)
    camera_pos_x: f32,                // 4 bytes (offset 64)
    camera_pos_y: f32,                // 4 bytes (offset 68)
    camera_pos_z: f32,                // 4 bytes (offset 72)
    time: f32,                        // 4 bytes (offset 76)
    sun_dir_x: f32,                   // 4 bytes (offset 80)
    sun_dir_y: f32,                   // 4 bytes (offset 84)
    sun_dir_z: f32,                   // 4 bytes (offset 88)
    sun_intensity: f32,               // 4 bytes (offset 92)
    ambient_strength: f32,            // 4 bytes (offset 96)
    fog_density: f32,                 // 4 bytes (offset 100)
    fog_color_r: f32,                 // 4 bytes (offset 104)
    fog_color_g: f32,                 // 4 bytes (offset 108)
    fog_color_b: f32,                 // 4 bytes (offset 112)
    steel_color_r: f32,               // 4 bytes (offset 116)
    steel_color_g: f32,               // 4 bytes (offset 120)
    steel_color_b: f32,               // 4 bytes (offset 124)
    shine: f32,                       // 4 bytes (offset 128)
    roughness: f32,                   // 4 bytes (offset 132)
    metallic: f32,                    // 4 bytes (offset 136)
    _pad1: f32,                       // 4 bytes (offset 140) - align to 16
}

// Verify struct size at compile time (must be multiple of 16)
const _: () = assert!(std::mem::size_of::<ChainMetalUniforms>() == 144);

/// Chain metal material renderer for bridge supports
pub struct ChainMetalMaterial {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    config: ChainMetalConfig,
}

impl ChainMetalMaterial {
    /// Create a new chain metal material with default configuration
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self::with_config(device, surface_format, ChainMetalConfig::default())
    }

    /// Create a new chain metal material with custom configuration
    pub fn with_config(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        config: ChainMetalConfig,
    ) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Chain Metal Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/chain_metal.wgsl").into(),
            ),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Chain Metal Uniform Buffer"),
            size: std::mem::size_of::<ChainMetalUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Chain Metal Bind Group Layout"),
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
            label: Some("Chain Metal Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Chain Metal Pipeline Layout"),
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
            label: Some("Chain Metal Pipeline"),
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

        println!("[ChainMetalMaterial] Initialized chain metal material");

        Self {
            pipeline,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            config,
        }
    }

    /// Get mutable access to the configuration
    pub fn config_mut(&mut self) -> &mut ChainMetalConfig {
        &mut self.config
    }

    /// Get the current configuration
    pub fn config(&self) -> &ChainMetalConfig {
        &self.config
    }

    /// Set a new configuration
    pub fn set_config(&mut self, config: ChainMetalConfig) {
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
        let uniforms = ChainMetalUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos_x: camera_pos.x,
            camera_pos_y: camera_pos.y,
            camera_pos_z: camera_pos.z,
            time,
            sun_dir_x: self.config.sun_dir.x,
            sun_dir_y: self.config.sun_dir.y,
            sun_dir_z: self.config.sun_dir.z,
            sun_intensity: self.config.sun_intensity,
            ambient_strength: self.config.ambient_strength,
            fog_density: self.config.fog_density,
            fog_color_r: self.config.fog_color.x,
            fog_color_g: self.config.fog_color.y,
            fog_color_b: self.config.fog_color.z,
            steel_color_r: self.config.steel_color.x,
            steel_color_g: self.config.steel_color.y,
            steel_color_b: self.config.steel_color.z,
            shine: self.config.shine,
            roughness: self.config.roughness,
            metallic: self.config.metallic,
            _pad1: 0.0,
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Bind the material for rendering
    pub fn bind<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
    }

    /// Get the bind group for external use
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
