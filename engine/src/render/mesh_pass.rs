//! Mesh Render Pass
//!
//! Handles rendering of 3D meshes with lighting and depth testing.
//! This includes terrain, walls, projectiles, trees, and building blocks.

use wgpu::util::DeviceExt;
use glam::Mat4;

use super::render_pass::{RenderPass, RenderPassPriority, RenderContext, FrameContext};

/// Vertex for mesh rendering (position, normal, color)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

/// Uniform data for mesh rendering
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub time: f32,
    pub sun_dir: [f32; 3],
    pub fog_density: f32,
    pub fog_color: [f32; 3],
    pub ambient: f32,
    pub projectile_count: u32,
    pub _pad1: [f32; 3],
    pub _pad2: [f32; 3],
    pub _pad3: f32,
    pub projectile_positions: [[f32; 4]; 32],
}

impl Default for MeshUniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            time: 0.0,
            sun_dir: [0.5, 0.8, 0.3],
            fog_density: 0.003,
            fog_color: [0.3, 0.2, 0.25],
            ambient: 0.2,
            projectile_count: 0,
            _pad1: [0.0; 3],
            _pad2: [0.0; 3],
            _pad3: 0.0,
            projectile_positions: [[0.0; 4]; 32],
        }
    }
}

/// A mesh buffer that can be drawn
pub struct MeshBuffer {
    pub label: &'static str,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

/// Mesh render pass that renders all registered mesh buffers
pub struct MeshRenderPass {
    enabled: bool,
    initialized: bool,
    pipeline: Option<wgpu::RenderPipeline>,
    bind_group: Option<wgpu::BindGroup>,
    bind_group_layout: Option<wgpu::BindGroupLayout>,
    uniform_buffer: Option<wgpu::Buffer>,
    // Uniforms to update each frame
    uniforms: MeshUniforms,
}

impl MeshRenderPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            initialized: false,
            pipeline: None,
            bind_group: None,
            bind_group_layout: None,
            uniform_buffer: None,
            uniforms: MeshUniforms::default(),
        }
    }

    /// Update uniforms (call before render)
    pub fn update_uniforms(
        &mut self,
        view_proj: [[f32; 4]; 4],
        camera_pos: [f32; 3],
        time: f32,
        sun_dir: [f32; 3],
        fog_density: f32,
        fog_color: [f32; 3],
        ambient: f32,
    ) {
        self.uniforms.view_proj = view_proj;
        self.uniforms.camera_pos = camera_pos;
        self.uniforms.time = time;
        self.uniforms.sun_dir = sun_dir;
        self.uniforms.fog_density = fog_density;
        self.uniforms.fog_color = fog_color;
        self.uniforms.ambient = ambient;
    }

    /// Set projectile positions for lighting effects
    pub fn set_projectiles(&mut self, positions: &[[f32; 4]], count: u32) {
        self.uniforms.projectile_count = count;
        let copy_count = positions.len().min(32);
        self.uniforms.projectile_positions[..copy_count].copy_from_slice(&positions[..copy_count]);
    }

    /// Get the bind group layout for creating mesh buffers
    pub fn bind_group_layout(&self) -> Option<&wgpu::BindGroupLayout> {
        self.bind_group_layout.as_ref()
    }

    /// Get the pipeline for external use
    pub fn pipeline(&self) -> Option<&wgpu::RenderPipeline> {
        self.pipeline.as_ref()
    }

    /// Get the bind group for external use
    pub fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.bind_group.as_ref()
    }

    /// Upload uniforms to GPU
    pub fn upload_uniforms(&self, queue: &wgpu::Queue) {
        if let Some(buffer) = &self.uniform_buffer {
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&self.uniforms));
        }
    }

    /// Create a mesh buffer from vertex/index data
    pub fn create_mesh_buffer(
        &self,
        device: &wgpu::Device,
        label: &'static str,
        vertices: &[MeshVertex],
        indices: &[u32],
    ) -> MeshBuffer {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Vertex Buffer", label)),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{} Index Buffer", label)),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        MeshBuffer {
            label,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    /// Create a dynamic mesh buffer that can be updated
    pub fn create_dynamic_mesh_buffer(
        &self,
        device: &wgpu::Device,
        label: &'static str,
        max_vertices: usize,
        max_indices: usize,
    ) -> MeshBuffer {
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{} Vertex Buffer", label)),
            size: (max_vertices * std::mem::size_of::<MeshVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(&format!("{} Index Buffer", label)),
            size: (max_indices * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        MeshBuffer {
            label,
            vertex_buffer,
            index_buffer,
            index_count: 0,
        }
    }
}

impl Default for MeshRenderPass {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderPass for MeshRenderPass {
    fn name(&self) -> &'static str {
        "Mesh"
    }

    fn priority(&self) -> RenderPassPriority {
        RenderPassPriority::Geometry
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn initialize(&mut self, ctx: &RenderContext) {
        if self.initialized {
            return;
        }

        // Load mesh shader
        let shader_source = include_str!("../../../shaders/mesh.wgsl");
        let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Mesh Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create uniform buffer
        let uniform_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Mesh Uniform Buffer"),
            size: std::mem::size_of::<MeshUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = ctx.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Mesh Bind Group Layout"),
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
        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Mesh Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Mesh Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create pipeline (with depth testing)
        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Mesh Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<MeshVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 12,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x4,
                            offset: 24,
                            shader_location: 2,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: ctx.surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
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

        self.pipeline = Some(pipeline);
        self.bind_group = Some(bind_group);
        self.bind_group_layout = Some(bind_group_layout);
        self.uniform_buffer = Some(uniform_buffer);
        self.initialized = true;
    }

    fn render(&self, _ctx: &RenderContext, _frame: &mut FrameContext) {
        // This pass doesn't render directly - it provides pipeline and bind group
        // for external code to use with its own mesh buffers
        // The actual rendering happens in the game code
    }
}

/// Helper to draw a mesh buffer using the mesh pass
pub fn draw_mesh_buffer(
    render_pass: &mut wgpu::RenderPass<'_>,
    mesh_buffer: &MeshBuffer,
) {
    if mesh_buffer.index_count > 0 {
        render_pass.set_vertex_buffer(0, mesh_buffer.vertex_buffer.slice(..));
        render_pass.set_index_buffer(mesh_buffer.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..mesh_buffer.index_count, 0, 0..1);
    }
}
