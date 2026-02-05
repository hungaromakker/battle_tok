//! GPU Context
//!
//! Unified GPU resource management for the engine.
//! Centralizes device, queue, and common buffers.

use std::sync::Arc;
use wgpu::util::DeviceExt;
use winit::window::Window;

/// Shared GPU resources
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
}

/// Configuration for GPU context creation
#[derive(Clone)]
pub struct GpuContextConfig {
    /// Use VSync (true = capped to monitor refresh, false = uncapped FPS)
    pub vsync: bool,
    /// Prefer high-performance GPU
    pub high_performance: bool,
    /// Enable debug validation
    pub debug: bool,
}

impl Default for GpuContextConfig {
    fn default() -> Self {
        Self {
            vsync: false, // Default to uncapped for performance
            high_performance: true,
            debug: cfg!(debug_assertions),
        }
    }
}

impl GpuContext {
    /// Create a new GPU context for a window
    pub fn new(window: Arc<Window>, config: GpuContextConfig) -> Self {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create surface
        let surface = instance
            .create_surface(Arc::clone(&window))
            .expect("Failed to create surface");

        // Request adapter
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: if config.high_performance {
                wgpu::PowerPreference::HighPerformance
            } else {
                wgpu::PowerPreference::LowPower
            },
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Failed to find GPU adapter");

        // Create device and queue
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("Battle Tök Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            ..Default::default()
        }))
        .expect("Failed to create GPU device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        // Select present mode based on vsync preference
        let present_mode = if config.vsync {
            wgpu::PresentMode::AutoVsync
        } else if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Immediate)
        {
            wgpu::PresentMode::Immediate
        } else if surface_caps
            .present_modes
            .contains(&wgpu::PresentMode::Mailbox)
        {
            wgpu::PresentMode::Mailbox
        } else {
            wgpu::PresentMode::AutoVsync
        };

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        // Create depth texture
        let (depth_texture, depth_view) =
            Self::create_depth_texture(&device, size.width, size.height);

        Self {
            device,
            queue,
            surface,
            surface_config,
            depth_texture,
            depth_view,
        }
    }

    /// Create depth texture with given dimensions
    fn create_depth_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size: wgpu::Extent3d {
                width: width.max(1),
                height: height.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }

    /// Handle window resize
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);

            // Recreate depth texture
            let (depth_texture, depth_view) =
                Self::create_depth_texture(&self.device, width, height);
            self.depth_texture = depth_texture;
            self.depth_view = depth_view;
        }
    }

    /// Get current surface dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.surface_config.width, self.surface_config.height)
    }

    /// Get surface format
    pub fn format(&self) -> wgpu::TextureFormat {
        self.surface_config.format
    }

    /// Get current surface texture for rendering
    pub fn get_current_texture(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.get_current_texture()
    }

    /// Create a uniform buffer with initial data
    pub fn create_uniform_buffer<T: bytemuck::Pod>(&self, label: &str, data: &T) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::bytes_of(data),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
    }

    /// Create an empty uniform buffer of given size
    pub fn create_empty_uniform_buffer(&self, label: &str, size: u64) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Create a vertex buffer with initial data
    pub fn create_vertex_buffer<T: bytemuck::Pod>(&self, label: &str, data: &[T]) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(data),
                usage: wgpu::BufferUsages::VERTEX,
            })
    }

    /// Create a dynamic vertex buffer (can be updated)
    pub fn create_dynamic_vertex_buffer(&self, label: &str, size: u64) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Create an index buffer with initial data
    pub fn create_index_buffer(&self, label: &str, data: &[u32]) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(data),
                usage: wgpu::BufferUsages::INDEX,
            })
    }

    /// Create a dynamic index buffer (can be updated)
    pub fn create_dynamic_index_buffer(&self, label: &str, size: u64) -> wgpu::Buffer {
        self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Write data to a buffer
    pub fn write_buffer<T: bytemuck::Pod>(&self, buffer: &wgpu::Buffer, data: &[T]) {
        self.queue
            .write_buffer(buffer, 0, bytemuck::cast_slice(data));
    }

    /// Create a standard mesh pipeline (position, normal, color)
    pub fn create_mesh_pipeline(
        &self,
        label: &str,
        shader_source: &str,
        bind_group_layout: &wgpu::BindGroupLayout,
        depth_enabled: bool,
    ) -> wgpu::RenderPipeline {
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(&format!("{} Shader", label)),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(&format!("{} Pipeline Layout", label)),
                bind_group_layouts: &[bind_group_layout],
                push_constant_ranges: &[],
            });

        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(&format!("{} Pipeline", label)),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 40, // 3 + 3 + 4 floats = 40 bytes
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
                        format: self.surface_config.format,
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
                depth_stencil: if depth_enabled {
                    Some(wgpu::DepthStencilState {
                        format: wgpu::TextureFormat::Depth32Float,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::Less,
                        stencil: wgpu::StencilState::default(),
                        bias: wgpu::DepthBiasState::default(),
                    })
                } else {
                    None
                },
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
    }
}
