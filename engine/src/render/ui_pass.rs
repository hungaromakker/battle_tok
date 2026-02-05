//! UI Render Pass
//!
//! Handles rendering of 2D UI elements on top of the scene.
//! No depth testing, uses alpha blending.

use glam::Mat4;
use wgpu::util::DeviceExt;

use super::render_pass::{RenderPass, RenderPassPriority, RenderContext, FrameContext};

/// Vertex for UI rendering (position, normal, color)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

/// UI uniform data (identity matrix for screen-space rendering)
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct UiUniforms {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    time: f32,
    sun_dir: [f32; 3],
    fog_density: f32,
    fog_color: [f32; 3],
    ambient: f32,
    projectile_count: u32,
    _padding1: [f32; 3],
    _padding2: [f32; 3],
    _padding3: f32,
    projectile_positions: [[f32; 4]; 32],
}

impl Default for UiUniforms {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0],
            time: 0.0,
            sun_dir: [0.0, 1.0, 0.0],
            fog_density: 0.0, // No fog for UI
            fog_color: [0.0, 0.0, 0.0],
            ambient: 1.0, // Full brightness
            projectile_count: 0,
            _padding1: [0.0; 3],
            _padding2: [0.0; 3],
            _padding3: 0.0,
            projectile_positions: [[0.0; 4]; 32],
        }
    }
}

/// A UI mesh to be rendered
pub struct UiMesh {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
}

impl UiMesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Add a quad to the mesh
    pub fn add_quad(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: [f32; 4]) {
        let base = self.vertices.len() as u32;
        let normal = [0.0, 0.0, 1.0];

        self.vertices.push(UiVertex { position: [x1, y1, 0.0], normal, color });
        self.vertices.push(UiVertex { position: [x2, y1, 0.0], normal, color });
        self.vertices.push(UiVertex { position: [x2, y2, 0.0], normal, color });
        self.vertices.push(UiVertex { position: [x1, y2, 0.0], normal, color });

        self.indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }

    /// Convert screen coordinates to NDC
    pub fn screen_to_ndc(x: f32, y: f32, width: f32, height: f32) -> [f32; 3] {
        [(x / width) * 2.0 - 1.0, 1.0 - (y / height) * 2.0, 0.0]
    }
}

impl Default for UiMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Trait for UI components that can generate meshes
pub trait UiComponent {
    fn generate_mesh(&self, width: f32, height: f32) -> UiMesh;
    fn is_visible(&self) -> bool { true }
}

/// UI render pass that renders all registered UI components
pub struct UiRenderPass {
    enabled: bool,
    initialized: bool,
    pipeline: Option<wgpu::RenderPipeline>,
    bind_group: Option<wgpu::BindGroup>,
    uniform_buffer: Option<wgpu::Buffer>,
    // Dynamic buffers for UI meshes
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    // Current mesh data (set before render)
    current_vertices: Vec<UiVertex>,
    current_indices: Vec<u32>,
}

impl UiRenderPass {
    pub fn new() -> Self {
        Self {
            enabled: true,
            initialized: false,
            pipeline: None,
            bind_group: None,
            uniform_buffer: None,
            vertex_buffer: None,
            index_buffer: None,
            current_vertices: Vec::new(),
            current_indices: Vec::new(),
        }
    }

    /// Set the mesh data to render this frame
    pub fn set_mesh(&mut self, vertices: Vec<UiVertex>, indices: Vec<u32>) {
        self.current_vertices = vertices;
        self.current_indices = indices;
    }

    /// Add a UI mesh to render
    pub fn add_mesh(&mut self, mesh: &UiMesh) {
        let base = self.current_vertices.len() as u32;
        self.current_vertices.extend_from_slice(&mesh.vertices);
        for idx in &mesh.indices {
            self.current_indices.push(base + idx);
        }
    }

    /// Clear current mesh data
    pub fn clear(&mut self) {
        self.current_vertices.clear();
        self.current_indices.clear();
    }
}

impl Default for UiRenderPass {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderPass for UiRenderPass {
    fn name(&self) -> &'static str {
        "UI"
    }

    fn priority(&self) -> RenderPassPriority {
        RenderPassPriority::UI
    }

    fn is_enabled(&self) -> bool {
        self.enabled && !self.current_vertices.is_empty()
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn initialize(&mut self, ctx: &RenderContext) {
        if self.initialized {
            return;
        }

        // UI shader (same as mesh shader but used differently)
        let shader_source = include_str!("../../../shaders/ui.wgsl");
        let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create uniform buffer
        let uniforms = UiUniforms::default();
        let uniform_buffer = ctx.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Uniform Buffer"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        // Create bind group layout
        let bind_group_layout = ctx.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("UI Bind Group Layout"),
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
            label: Some("UI Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create pipeline (no depth testing, alpha blending)
        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // No culling for UI
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None, // No depth testing for UI
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create dynamic buffers for UI meshes
        let vertex_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Vertex Buffer"),
            size: 1024 * 1024, // 1MB for UI vertices
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI Index Buffer"),
            size: 256 * 1024, // 256KB for UI indices
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.pipeline = Some(pipeline);
        self.bind_group = Some(bind_group);
        self.uniform_buffer = Some(uniform_buffer);
        self.vertex_buffer = Some(vertex_buffer);
        self.index_buffer = Some(index_buffer);
        self.initialized = true;
    }

    fn render(&self, ctx: &RenderContext, frame: &mut FrameContext) {
        if !self.initialized || self.current_vertices.is_empty() {
            return;
        }

        let pipeline = self.pipeline.as_ref().unwrap();
        let bind_group = self.bind_group.as_ref().unwrap();
        let vertex_buffer = self.vertex_buffer.as_ref().unwrap();
        let index_buffer = self.index_buffer.as_ref().unwrap();

        // Upload mesh data
        ctx.queue.write_buffer(vertex_buffer, 0, bytemuck::cast_slice(&self.current_vertices));
        ctx.queue.write_buffer(index_buffer, 0, bytemuck::cast_slice(&self.current_indices));

        // Begin render pass
        let mut render_pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("UI Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Preserve previous passes
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None, // No depth for UI
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.current_indices.len() as u32, 0, 0..1);
    }
}
