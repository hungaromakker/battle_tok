//! Apocalyptic Sky Module
//!
//! High-fidelity volumetric skybox with:
//! - Dramatic stormy clouds with purple/orange lighting
//! - Nebula effects and stars
//! - Molten planet visible in sky
//! - Lightning strikes
//! - HDR output for bloom
//!
//! Designed to match the reference concept art with:
//! - Deep purple zenith
//! - Orange/red horizon from lava glow
//! - Volumetric clouds lit from below
//! - Apocalyptic planet in the sky

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// Configuration for the apocalyptic sky
#[derive(Clone, Copy, Debug)]
pub struct ApocalypticSkyConfig {
    // Cloud parameters
    pub cloud_speed: f32,
    pub cloud_density: f32,
    pub cloud_scale: f32,
    pub cloud_coverage: f32,

    // Sky colors
    pub zenith_color: Vec3,  // Deep purple
    pub horizon_color: Vec3, // Orange/red

    // Lava glow from below
    pub lava_glow_color: Vec3,
    pub lava_glow_strength: f32,

    // Sun direction (mainly for cloud lighting)
    pub sun_dir: Vec3,
    pub sun_intensity: f32,

    // Lightning
    pub lightning_intensity: f32,
    pub lightning_pos: (f32, f32),
}

impl Default for ApocalypticSkyConfig {
    fn default() -> Self {
        Self {
            cloud_speed: 0.1,
            cloud_density: 1.5,
            cloud_scale: 1.0,
            cloud_coverage: 0.6,

            // Deep purple zenith matching reference
            zenith_color: Vec3::new(0.08, 0.03, 0.15),
            // Orange-red horizon from lava
            horizon_color: Vec3::new(0.6, 0.25, 0.12),

            // Strong lava glow
            lava_glow_color: Vec3::new(1.2, 0.4, 0.1),
            lava_glow_strength: 2.0,

            // Low sun for dramatic rim lighting
            sun_dir: Vec3::new(0.2, 0.1, -0.9).normalize(),
            sun_intensity: 1.0,

            lightning_intensity: 0.0,
            lightning_pos: (0.5, 0.5),
        }
    }
}

impl ApocalypticSkyConfig {
    /// Create preset matching the reference image - dramatic storm over lava
    pub fn battle_arena() -> Self {
        Self {
            cloud_speed: 0.15,
            cloud_density: 2.5, // Denser, more dramatic clouds
            cloud_scale: 1.0,
            cloud_coverage: 0.75, // More cloud coverage

            // Deep purple zenith matching reference - more saturated
            zenith_color: Vec3::new(0.08, 0.02, 0.22),
            // Orange-red horizon with more intensity
            horizon_color: Vec3::new(0.85, 0.35, 0.12),

            // Intense lava glow from below
            lava_glow_color: Vec3::new(1.8, 0.55, 0.15),
            lava_glow_strength: 3.5,

            // Low sun for dramatic rim lighting on clouds
            sun_dir: Vec3::new(0.1, 0.05, -1.0).normalize(),
            sun_intensity: 1.0,

            lightning_intensity: 0.0,
            lightning_pos: (0.5, 0.5),
        }
    }

    /// Create preset matching the second reference (space/planet view)
    pub fn space_apocalypse() -> Self {
        Self {
            cloud_speed: 0.05,
            cloud_density: 0.8,
            cloud_scale: 0.8,
            cloud_coverage: 0.4,

            // Darker, more space-like
            zenith_color: Vec3::new(0.02, 0.01, 0.05),
            horizon_color: Vec3::new(0.5, 0.15, 0.08),

            // Intense planetary glow
            lava_glow_color: Vec3::new(2.0, 0.6, 0.15),
            lava_glow_strength: 3.0,

            sun_dir: Vec3::new(0.5, 0.3, -0.8).normalize(),
            sun_intensity: 1.2,

            lightning_intensity: 0.0,
            lightning_pos: (0.5, 0.5),
        }
    }

    /// Active lightning storm
    pub fn lightning_storm() -> Self {
        let mut config = Self::battle_arena();
        config.lightning_intensity = 0.8;
        config
    }
}

/// GPU uniform buffer layout
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct ApocalypticSkyUniforms {
    view_proj: [[f32; 4]; 4],
    inv_view_proj: [[f32; 4]; 4],
    camera_pos_x: f32,
    camera_pos_y: f32,
    camera_pos_z: f32,
    time: f32,
    resolution_x: f32,
    resolution_y: f32,
    cloud_speed: f32,
    cloud_density: f32,
    cloud_scale: f32,
    cloud_coverage: f32,
    zenith_r: f32,
    zenith_g: f32,
    zenith_b: f32,
    horizon_r: f32,
    horizon_g: f32,
    horizon_b: f32,
    lava_glow_r: f32,
    lava_glow_g: f32,
    lava_glow_b: f32,
    lava_glow_strength: f32,
    sun_dir_x: f32,
    sun_dir_y: f32,
    sun_dir_z: f32,
    sun_intensity: f32,
    lightning_intensity: f32,
    lightning_pos_x: f32,
    lightning_pos_z: f32,
    _pad: f32,
}

// Verify struct size at compile time (32 floats * 4 = 128 bytes + matrices)
const _: () = assert!(std::mem::size_of::<ApocalypticSkyUniforms>() == 240);

/// Apocalyptic Sky Renderer
pub struct ApocalypticSky {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    config: ApocalypticSkyConfig,
    lightning_timer: f32,
}

impl ApocalypticSky {
    /// Create a new apocalyptic sky renderer
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self::with_config(device, surface_format, ApocalypticSkyConfig::default())
    }

    /// Create with custom configuration
    pub fn with_config(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        config: ApocalypticSkyConfig,
    ) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Apocalyptic Sky Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/apocalyptic_sky.wgsl").into(),
            ),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Apocalyptic Sky Uniform Buffer"),
            size: std::mem::size_of::<ApocalypticSkyUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Apocalyptic Sky Bind Group Layout"),
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
            label: Some("Apocalyptic Sky Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Apocalyptic Sky Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Apocalyptic Sky Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
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
            multiview: None,
            cache: None,
        });

        println!("[ApocalypticSky] Initialized high-fidelity apocalyptic skybox");

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            config,
            lightning_timer: 0.0,
        }
    }

    /// Get mutable config
    pub fn config_mut(&mut self) -> &mut ApocalypticSkyConfig {
        &mut self.config
    }

    /// Get current config
    pub fn config(&self) -> &ApocalypticSkyConfig {
        &self.config
    }

    /// Set configuration
    pub fn set_config(&mut self, config: ApocalypticSkyConfig) {
        self.config = config;
    }

    /// Trigger a lightning flash
    pub fn trigger_lightning(&mut self) {
        self.config.lightning_intensity = 1.0;
        // Random position for lightning
        self.config.lightning_pos = (
            (self.lightning_timer * 7.3).sin() * 0.3 + 0.5,
            (self.lightning_timer * 11.7).cos() * 0.2 + 0.5,
        );
    }

    /// Update uniform buffer
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        camera_pos: Vec3,
        time: f32,
        resolution: (u32, u32),
        delta_time: f32,
    ) {
        // Decay lightning
        if self.config.lightning_intensity > 0.0 {
            self.config.lightning_intensity =
                (self.config.lightning_intensity - delta_time * 8.0).max(0.0);
        }

        // Random lightning triggers
        self.lightning_timer += delta_time;
        if self.lightning_timer > 5.0 + (time * 3.7).sin() * 3.0 {
            self.lightning_timer = 0.0;
            self.trigger_lightning();
        }

        let inv_view_proj = view_proj.inverse();

        let uniforms = ApocalypticSkyUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            camera_pos_x: camera_pos.x,
            camera_pos_y: camera_pos.y,
            camera_pos_z: camera_pos.z,
            time,
            resolution_x: resolution.0 as f32,
            resolution_y: resolution.1 as f32,
            cloud_speed: self.config.cloud_speed,
            cloud_density: self.config.cloud_density,
            cloud_scale: self.config.cloud_scale,
            cloud_coverage: self.config.cloud_coverage,
            zenith_r: self.config.zenith_color.x,
            zenith_g: self.config.zenith_color.y,
            zenith_b: self.config.zenith_color.z,
            horizon_r: self.config.horizon_color.x,
            horizon_g: self.config.horizon_color.y,
            horizon_b: self.config.horizon_color.z,
            lava_glow_r: self.config.lava_glow_color.x,
            lava_glow_g: self.config.lava_glow_color.y,
            lava_glow_b: self.config.lava_glow_color.z,
            lava_glow_strength: self.config.lava_glow_strength,
            sun_dir_x: self.config.sun_dir.x,
            sun_dir_y: self.config.sun_dir.y,
            sun_dir_z: self.config.sun_dir.z,
            sun_intensity: self.config.sun_intensity,
            lightning_intensity: self.config.lightning_intensity,
            lightning_pos_x: self.config.lightning_pos.0,
            lightning_pos_z: self.config.lightning_pos.1,
            _pad: 0.0,
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Render the sky
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1);
    }

    /// Create render pass and render
    pub fn render_to_view(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Apocalyptic Sky Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.02,
                        g: 0.01,
                        b: 0.05,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.render(&mut render_pass);
    }
}
