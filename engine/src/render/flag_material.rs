//! Team Flag Material Module
//!
//! Flag shader with vertex-based wind animation and team colors.
//! Features:
//! - Wave displacement that increases toward flag edge
//! - Configurable team color (red or blue)
//! - Horizontal stripe band for visual interest
//!
//! # Usage
//!
//! ```rust,ignore
//! // Initialize
//! let material = FlagMaterial::new(&device, surface_format);
//!
//! // Set team (red or blue)
//! material.set_team(FlagTeam::Red);
//!
//! // Each frame
//! material.update(&queue, &camera, time);
//!
//! // Render (bind during mesh rendering)
//! material.bind(&mut render_pass);
//! ```

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// Team color enumeration
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlagTeam {
    /// Red team - warm red color with darker stripe
    Red,
    /// Blue team - bright blue color with darker stripe
    Blue,
}

impl Default for FlagTeam {
    fn default() -> Self {
        FlagTeam::Red
    }
}

/// Flag material configuration
#[derive(Clone, Copy, Debug)]
pub struct FlagMaterialConfig {
    /// Team color (RGB)
    pub team_color: Vec3,
    /// Stripe color (darker variant of team color)
    pub stripe_color: Vec3,
    /// Wind strength (0.1 to 0.3 typical)
    pub wind_strength: f32,
    /// Ambient light strength (default: 0.4)
    pub ambient_strength: f32,
}

impl Default for FlagMaterialConfig {
    fn default() -> Self {
        // Default to red team
        Self::for_team(FlagTeam::Red)
    }
}

impl FlagMaterialConfig {
    /// Create configuration for a specific team
    pub fn for_team(team: FlagTeam) -> Self {
        match team {
            FlagTeam::Red => Self {
                team_color: Vec3::new(0.8, 0.1, 0.1),       // Bright red
                stripe_color: Vec3::new(0.4, 0.05, 0.05),  // Dark red stripe
                wind_strength: 0.2,
                ambient_strength: 0.4,
            },
            FlagTeam::Blue => Self {
                team_color: Vec3::new(0.1, 0.4, 0.9),       // Bright blue
                stripe_color: Vec3::new(0.05, 0.2, 0.45),  // Dark blue stripe
                wind_strength: 0.2,
                ambient_strength: 0.4,
            },
        }
    }

    /// Create a preset for apocalyptic arena (darker, more dramatic)
    pub fn battle_arena(team: FlagTeam) -> Self {
        match team {
            FlagTeam::Red => Self {
                team_color: Vec3::new(0.9, 0.15, 0.1),      // Fiery red
                stripe_color: Vec3::new(0.35, 0.05, 0.02), // Dark ember stripe
                wind_strength: 0.25,                        // Stronger wind
                ambient_strength: 0.3,                      // Darker ambient
            },
            FlagTeam::Blue => Self {
                team_color: Vec3::new(0.15, 0.5, 0.95),     // Electric blue
                stripe_color: Vec3::new(0.05, 0.2, 0.4),   // Deep blue stripe
                wind_strength: 0.25,
                ambient_strength: 0.3,
            },
        }
    }
}

/// GPU uniform buffer layout (must match WGSL struct)
/// Total size: 128 bytes (aligned to 16)
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FlagUniforms {
    view_proj: [[f32; 4]; 4],         // 64 bytes (offset 0)
    time: f32,                        // 4 bytes (offset 64)
    team_color_r: f32,                // 4 bytes (offset 68)
    team_color_g: f32,                // 4 bytes (offset 72)
    team_color_b: f32,                // 4 bytes (offset 76)
    stripe_color_r: f32,              // 4 bytes (offset 80)
    stripe_color_g: f32,              // 4 bytes (offset 84)
    stripe_color_b: f32,              // 4 bytes (offset 88)
    wind_strength: f32,               // 4 bytes (offset 92)
    camera_pos_x: f32,                // 4 bytes (offset 96)
    camera_pos_y: f32,                // 4 bytes (offset 100)
    camera_pos_z: f32,                // 4 bytes (offset 104)
    ambient_strength: f32,            // 4 bytes (offset 108)
    _pad1: f32,                       // 4 bytes (offset 112)
    _pad2: f32,                       // 4 bytes (offset 116)
    _pad3: f32,                       // 4 bytes (offset 120)
    _pad4: f32,                       // 4 bytes (offset 124) - align to 128
}

// Verify struct size at compile time
const _: () = assert!(std::mem::size_of::<FlagUniforms>() == 128);

/// Team flag material renderer
pub struct FlagMaterial {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    config: FlagMaterialConfig,
    team: FlagTeam,
}

impl FlagMaterial {
    /// Create a new flag material with default configuration (Red team)
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self::with_team(device, surface_format, FlagTeam::Red)
    }

    /// Create a new flag material for a specific team
    pub fn with_team(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        team: FlagTeam,
    ) -> Self {
        Self::with_config(device, surface_format, FlagMaterialConfig::for_team(team), team)
    }

    /// Create a new flag material with custom configuration
    pub fn with_config(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        config: FlagMaterialConfig,
        team: FlagTeam,
    ) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Flag Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/flag.wgsl").into(),
            ),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Flag Uniform Buffer"),
            size: std::mem::size_of::<FlagUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Flag Bind Group Layout"),
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
            label: Some("Flag Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Flag Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Vertex buffer layout for flag mesh with position and UV
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: 20, // 5 floats * 4 bytes = 20 bytes per vertex (pos3 + uv2)
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // UV
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
            ],
        };

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Flag Pipeline"),
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
                cull_mode: None, // Don't cull - flags are visible from both sides
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

        println!("[FlagMaterial] Initialized flag material for {:?} team", team);

        Self {
            pipeline,
            uniform_buffer,
            bind_group_layout,
            bind_group,
            config,
            team,
        }
    }

    /// Get mutable access to the configuration
    pub fn config_mut(&mut self) -> &mut FlagMaterialConfig {
        &mut self.config
    }

    /// Get the current configuration
    pub fn config(&self) -> &FlagMaterialConfig {
        &self.config
    }

    /// Set a new configuration
    pub fn set_config(&mut self, config: FlagMaterialConfig) {
        self.config = config;
    }

    /// Get the current team
    pub fn team(&self) -> FlagTeam {
        self.team
    }

    /// Set the team (updates colors automatically)
    pub fn set_team(&mut self, team: FlagTeam) {
        self.team = team;
        self.config = FlagMaterialConfig::for_team(team);
    }

    /// Set the team with battle arena preset
    pub fn set_team_battle_arena(&mut self, team: FlagTeam) {
        self.team = team;
        self.config = FlagMaterialConfig::battle_arena(team);
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
        let uniforms = FlagUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            time,
            team_color_r: self.config.team_color.x,
            team_color_g: self.config.team_color.y,
            team_color_b: self.config.team_color.z,
            stripe_color_r: self.config.stripe_color.x,
            stripe_color_g: self.config.stripe_color.y,
            stripe_color_b: self.config.stripe_color.z,
            wind_strength: self.config.wind_strength,
            camera_pos_x: camera_pos.x,
            camera_pos_y: camera_pos.y,
            camera_pos_z: camera_pos.z,
            ambient_strength: self.config.ambient_strength,
            _pad1: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
            _pad4: 0.0,
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Bind the material for rendering
    /// Call this before drawing flag meshes
    pub fn bind<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
    }

    /// Get the bind group for external use
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

/// Flag vertex structure for mesh creation
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct FlagVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
}

impl FlagVertex {
    /// Create a simple quad flag mesh (4 vertices, 6 indices)
    /// The flag extends from the pole (x=0) to the free edge (x=width)
    /// UV coordinates: x=0 at pole, x=1 at edge; y=0 at bottom, y=1 at top
    pub fn create_flag_quad(width: f32, height: f32) -> (Vec<FlagVertex>, Vec<u16>) {
        let vertices = vec![
            // Bottom-left (at pole)
            FlagVertex {
                position: [0.0, 0.0, 0.0],
                uv: [0.0, 0.0],
            },
            // Bottom-right (free edge)
            FlagVertex {
                position: [width, 0.0, 0.0],
                uv: [1.0, 0.0],
            },
            // Top-right (free edge)
            FlagVertex {
                position: [width, height, 0.0],
                uv: [1.0, 1.0],
            },
            // Top-left (at pole)
            FlagVertex {
                position: [0.0, height, 0.0],
                uv: [0.0, 1.0],
            },
        ];

        let indices = vec![
            0, 1, 2, // First triangle
            0, 2, 3, // Second triangle
        ];

        (vertices, indices)
    }

    /// Create a subdivided flag mesh for smoother wave animation
    /// subdivisions_x: number of divisions along flag width
    /// subdivisions_y: number of divisions along flag height
    pub fn create_subdivided_flag(
        width: f32,
        height: f32,
        subdivisions_x: u32,
        subdivisions_y: u32,
    ) -> (Vec<FlagVertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let step_x = width / subdivisions_x as f32;
        let step_y = height / subdivisions_y as f32;
        let uv_step_x = 1.0 / subdivisions_x as f32;
        let uv_step_y = 1.0 / subdivisions_y as f32;

        // Generate vertices
        for j in 0..=subdivisions_y {
            for i in 0..=subdivisions_x {
                let x = i as f32 * step_x;
                let y = j as f32 * step_y;
                let u = i as f32 * uv_step_x;
                let v = j as f32 * uv_step_y;

                vertices.push(FlagVertex {
                    position: [x, y, 0.0],
                    uv: [u, v],
                });
            }
        }

        // Generate indices (two triangles per quad)
        let cols = subdivisions_x + 1;
        for j in 0..subdivisions_y {
            for i in 0..subdivisions_x {
                let base = j * cols + i;
                // First triangle
                indices.push(base as u16);
                indices.push((base + 1) as u16);
                indices.push((base + cols + 1) as u16);
                // Second triangle
                indices.push(base as u16);
                indices.push((base + cols + 1) as u16);
                indices.push((base + cols) as u16);
            }
        }

        (vertices, indices)
    }
}
