//! Background Image Tracing for Asset Editor
//!
//! Loads a reference image (PNG or JPG) and renders it as a semi-transparent
//! background layer on the 2D drawing canvas. Artists can trace over this
//! image to create outlines for asset creation.
//!
//! GPU resources are created on load and released on drop. The image is
//! rendered as a textured quad with alpha blending, positioned behind
//! the canvas grid and outlines.

use std::path::Path;

// ============================================================================
// VERTEX & UNIFORM TYPES
// ============================================================================

/// Vertex for the textured quad (position + UV).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Canvas2DVertex {
    position: [f32; 3],
    uv: [f32; 2],
}

/// Uniform buffer layout for the canvas 2D shader (16-byte aligned).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Canvas2DUniforms {
    view_projection: [[f32; 4]; 4],  // 64 bytes
    opacity: f32,                     // 4 bytes
    _pad0: f32,                       // 4 bytes
    _pad1: f32,                       // 4 bytes
    _pad2: f32,                       // 4 bytes — total 80 bytes
}

// ============================================================================
// CANVAS VIEWPORT
// ============================================================================

/// Describes the current 2D canvas viewport state, needed to build the
/// orthographic view-projection matrix that matches the Canvas2D coordinate
/// system.
pub struct CanvasViewport {
    /// Zoom level (1.0 = default, 20x20 world units visible).
    pub zoom: f32,
    /// Camera pan offset in canvas (world) coordinates.
    pub pan: [f32; 2],
    /// Viewport width in pixels.
    pub width: f32,
    /// Viewport height in pixels.
    pub height: f32,
}

impl CanvasViewport {
    /// Half-extent of the visible canvas range (same as Canvas2D::DEFAULT_HALF_EXTENT).
    const DEFAULT_HALF_EXTENT: f32 = 10.0;

    /// Build an orthographic view-projection matrix that matches Canvas2D's
    /// coordinate system. Returns a column-major 4x4 matrix.
    pub fn view_projection(&self) -> [[f32; 4]; 4] {
        let aspect = self.width / self.height;
        let half_x = Self::DEFAULT_HALF_EXTENT / self.zoom * aspect;
        let half_y = Self::DEFAULT_HALF_EXTENT / self.zoom;

        let left = self.pan[0] - half_x;
        let right = self.pan[0] + half_x;
        let bottom = self.pan[1] - half_y;
        let top = self.pan[1] + half_y;

        // Orthographic projection, depth 0..1 (wgpu convention, right-handed)
        let near = -1.0_f32;
        let far = 1.0_f32;

        let sx = 2.0 / (right - left);
        let sy = 2.0 / (top - bottom);
        let sz = 1.0 / (far - near);
        let tx = -(right + left) / (right - left);
        let ty = -(top + bottom) / (top - bottom);
        let tz = -near / (far - near);

        // Column-major layout
        [
            [sx,  0.0, 0.0, 0.0],
            [0.0, sy,  0.0, 0.0],
            [0.0, 0.0, sz,  0.0],
            [tx,  ty,  tz,  1.0],
        ]
    }
}

// ============================================================================
// IMAGE TRACE
// ============================================================================

/// Background reference image for tracing on the 2D canvas.
///
/// Manages GPU resources (texture, pipeline, buffers) for rendering a
/// loaded image as a semi-transparent quad behind the canvas drawing.
pub struct ImageTrace {
    // GPU resources
    _texture: wgpu::Texture,
    _texture_view: wgpu::TextureView,
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    pipeline: wgpu::RenderPipeline,

    // State
    /// Opacity of the background image (0.0 = invisible, 1.0 = fully opaque).
    pub opacity: f32,
    /// Scale factor for the image quad in canvas units.
    pub scale: f32,
    /// Position offset of the image center in canvas coordinates.
    pub position: [f32; 2],
    /// Whether the image is visible.
    pub visible: bool,

    // Metadata
    /// Width of the loaded image in pixels.
    pub image_width: u32,
    /// Height of the loaded image in pixels.
    pub image_height: u32,
}

impl ImageTrace {
    /// Default opacity for the reference image.
    const DEFAULT_OPACITY: f32 = 0.3;
    /// Default scale (1.0 = 1 pixel = 1 canvas unit for the image's longer dimension).
    const DEFAULT_SCALE: f32 = 1.0;
    /// Minimum allowed scale.
    pub const MIN_SCALE: f32 = 0.1;
    /// Maximum allowed scale.
    pub const MAX_SCALE: f32 = 10.0;

    /// Load a reference image from disk and create all GPU resources.
    ///
    /// The image is loaded as RGBA8 and uploaded to a GPU texture. A render
    /// pipeline is created for rendering the image as a textured quad with
    /// alpha blending.
    ///
    /// # Arguments
    /// * `path` - Path to the image file (PNG or JPG)
    /// * `device` - wgpu device for creating GPU resources
    /// * `queue` - wgpu queue for uploading texture data
    /// * `surface_format` - The surface texture format for pipeline compatibility
    pub fn load(
        path: &Path,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, String> {
        // Load and decode the image
        let img = image::open(path).map_err(|e| format!("Failed to load image: {e}"))?;
        let rgba = img.to_rgba8();
        let (width, height) = (rgba.width(), rgba.height());

        println!(
            "ImageTrace: loaded {}x{} image from {:?}",
            width,
            height,
            path.file_name().unwrap_or_default()
        );

        // Create GPU texture
        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("ImageTrace Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload pixel data (wgpu 27 API)
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ImageTrace Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Uniform buffer
        let uniforms = Canvas2DUniforms {
            view_projection: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            opacity: Self::DEFAULT_OPACITY,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ImageTrace Uniform Buffer"),
            size: std::mem::size_of::<Canvas2DUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        queue.write_buffer(&uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Vertex buffer — quad sized in canvas units
        // The image is scaled so its longer dimension = 10.0 canvas units (half the default extent)
        let aspect_ratio = width as f32 / height as f32;
        let (half_w, half_h) = if aspect_ratio >= 1.0 {
            (5.0, 5.0 / aspect_ratio)
        } else {
            (5.0 * aspect_ratio, 5.0)
        };

        let vertices: [Canvas2DVertex; 4] = [
            Canvas2DVertex { position: [-half_w, -half_h, 0.0], uv: [0.0, 1.0] },  // bottom-left
            Canvas2DVertex { position: [ half_w, -half_h, 0.0], uv: [1.0, 1.0] },  // bottom-right
            Canvas2DVertex { position: [ half_w,  half_h, 0.0], uv: [1.0, 0.0] },  // top-right
            Canvas2DVertex { position: [-half_w,  half_h, 0.0], uv: [0.0, 0.0] },  // top-left
        ];

        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ImageTrace Vertex Buffer"),
            size: std::mem::size_of_val(&vertices) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        let quad_indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ImageTrace Index Buffer"),
            size: std::mem::size_of_val(&quad_indices) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&quad_indices));

        // Bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("ImageTrace Bind Group Layout"),
            entries: &[
                // @binding(0): uniform buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // @binding(1): texture
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
                // @binding(2): sampler
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("ImageTrace Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ImageTrace Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Shader module
        let shader_source = include_str!("../../../shaders/canvas_2d.wgsl");
        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ImageTrace Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Vertex buffer layout: 20 bytes stride, 2 attributes
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Canvas2DVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // position: Float32x3 @ offset 0
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // uv: Float32x2 @ offset 12
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
            ],
        };

        // Render pipeline with alpha blending
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ImageTrace Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout],
                compilation_options: Default::default(),
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            _texture: texture,
            _texture_view: texture_view,
            _sampler: sampler,
            bind_group,
            uniform_buffer,
            vertex_buffer,
            index_buffer,
            pipeline,
            opacity: Self::DEFAULT_OPACITY,
            scale: Self::DEFAULT_SCALE,
            position: [0.0, 0.0],
            visible: true,
            image_width: width,
            image_height: height,
        })
    }

    /// Render the reference image onto the given texture view.
    ///
    /// Uses `LoadOp::Load` so it composites on top of the existing
    /// framebuffer content (the clear color). Call this AFTER the clear
    /// pass but BEFORE rendering canvas outlines.
    ///
    /// # Arguments
    /// * `encoder` - Command encoder to record the render pass
    /// * `view` - Target texture view (the current surface texture)
    /// * `queue` - wgpu queue for updating uniform buffer
    /// * `canvas_vp` - Current canvas viewport state for the view-projection matrix
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        queue: &wgpu::Queue,
        canvas_vp: &CanvasViewport,
    ) {
        if !self.visible {
            return;
        }

        // Build the view-projection matrix from the canvas viewport,
        // then apply scale and position offset for the image quad.
        let vp = canvas_vp.view_projection();

        // Update uniform buffer with current VP and opacity
        let uniforms = Canvas2DUniforms {
            view_projection: vp,
            opacity: self.opacity,
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Update vertex buffer if scale or position changed
        let aspect_ratio = self.image_width as f32 / self.image_height as f32;
        let (half_w, half_h) = if aspect_ratio >= 1.0 {
            (5.0 * self.scale, 5.0 * self.scale / aspect_ratio)
        } else {
            (5.0 * self.scale * aspect_ratio, 5.0 * self.scale)
        };

        let px = self.position[0];
        let py = self.position[1];

        let vertices: [Canvas2DVertex; 4] = [
            Canvas2DVertex { position: [px - half_w, py - half_h, 0.0], uv: [0.0, 1.0] },
            Canvas2DVertex { position: [px + half_w, py - half_h, 0.0], uv: [1.0, 1.0] },
            Canvas2DVertex { position: [px + half_w, py + half_h, 0.0], uv: [1.0, 0.0] },
            Canvas2DVertex { position: [px - half_w, py + half_h, 0.0], uv: [0.0, 0.0] },
        ];
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        // Render pass — loads existing content, draws the textured quad
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ImageTrace Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..6, 0, 0..1);
        }
    }

    /// Toggle visibility of the reference image.
    pub fn toggle_visible(&mut self) {
        self.visible = !self.visible;
        println!(
            "ImageTrace: visibility {}",
            if self.visible { "on" } else { "off" }
        );
    }

    /// Adjust the scale by a multiplicative factor, clamped to valid range.
    pub fn adjust_scale(&mut self, factor: f32) {
        self.scale = (self.scale * factor).clamp(Self::MIN_SCALE, Self::MAX_SCALE);
        println!("ImageTrace: scale = {:.2}", self.scale);
    }
}
