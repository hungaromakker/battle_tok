//! Dark and Stormy Skybox Module
//!
//! Volumetric procedural cloud shader with:
//! - Flow mapping for organic cloud movement
//! - Steep parallax mapping for volumetric depth
//! - Wave distortion for ocean-like motion
//! - Front-to-back alpha blending for layered clouds
//! - Lightning flash support
//!
//! # Usage
//!
//! ```rust,ignore
//! // Initialize
//! let skybox = StormySky::new(&device, surface_format);
//!
//! // Each frame
//! skybox.update(&queue, &camera, time, &config);
//!
//! // Render (as first pass before mesh rendering)
//! skybox.render(&mut encoder, &view);
//! ```

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// Stormy sky configuration
#[derive(Clone, Copy, Debug)]
pub struct StormySkyConfig {
    /// Overall cloud animation speed (default: 0.1)
    pub cloud_speed: f32,
    /// Flow map animation speed (default: 0.02)
    pub flow_speed: f32,
    /// Flow distortion strength (default: 0.3)
    pub flow_amount: f32,
    /// Wave distortion strength (default: 0.15)
    pub wave_amount: f32,
    /// Wave UV distortion (default: 0.05)
    pub wave_distort: f32,
    /// Cloud thickness/opacity (default: 5.0)
    pub cloud_density: f32,
    /// Cloud texture scale (default: 1.0)
    pub cloud_scale: f32,
    /// Cloud height bias (default: -0.1)
    pub cloud_bias: f32,
    /// Parallax depth amount (default: 0.1)
    pub bump_offset: f32,
    /// Number of parallax layers (default: 10.0)
    pub parallax_steps: f32,
    /// World-space cloud height (default: 100.0)
    pub cloud_height: f32,
    /// World-space UV scale (default: 10.0)
    pub world_scale: f32,
    /// Sun direction (normalized)
    pub sun_dir: Vec3,
    /// Lightning flash intensity (0-1)
    pub lightning_intensity: f32,
    /// Cloud highlight color (thin areas showing sky)
    pub cloud_color1: Vec3,
    /// Cloud shadow color (thick dark areas)
    pub cloud_color2: Vec3,
    /// Sky color showing through clouds
    pub upper_color: Vec3,
    /// Horizon fog color
    pub fog_color: Vec3,
    /// Fog density at horizon
    pub fog_density: f32,
}

impl Default for StormySkyConfig {
    fn default() -> Self {
        Self {
            cloud_speed: 0.05, // Slow dramatic movement
            flow_speed: 0.02,
            flow_amount: 0.3,
            wave_amount: 0.15,
            wave_distort: 0.05,
            cloud_density: 1.0, // Used for blending
            cloud_scale: 1.0,
            cloud_bias: 0.0,
            bump_offset: 0.1,
            parallax_steps: 6.0, // Optimized - only 6 ray march steps
            cloud_height: 100.0,
            world_scale: 1.0,
            // Sun position - low on horizon for dramatic rim lighting
            sun_dir: Vec3::new(0.3, 0.15, -0.9).normalize(),
            lightning_intensity: 0.0,
            // Colors (used as fallback, main colors are in shader)
            cloud_color1: Vec3::new(0.4, 0.35, 0.3), // Lit cloud edges
            cloud_color2: Vec3::new(0.05, 0.05, 0.07), // Dark cloud core
            upper_color: Vec3::new(0.02, 0.02, 0.04), // Near-black zenith
            fog_color: Vec3::new(0.12, 0.1, 0.08),   // Brown horizon fog
            fog_density: 0.6,
        }
    }
}

impl StormySkyConfig {
    /// Create a preset for a very dark and ominous storm
    pub fn very_dark() -> Self {
        Self {
            cloud_speed: 0.15,
            flow_speed: 0.03,
            flow_amount: 0.4,
            wave_amount: 0.2,
            cloud_density: 7.0,
            cloud_scale: 1.2,
            cloud_bias: -0.2,
            parallax_steps: 12.0,
            cloud_color1: Vec3::new(0.35, 0.38, 0.42),
            cloud_color2: Vec3::new(0.08, 0.08, 0.1),
            upper_color: Vec3::new(0.15, 0.18, 0.25),
            fog_color: Vec3::new(0.1, 0.12, 0.15),
            fog_density: 0.9,
            ..Default::default()
        }
    }

    /// Create a preset for an active lightning storm
    pub fn lightning_storm() -> Self {
        Self {
            cloud_speed: 0.2,
            flow_speed: 0.04,
            flow_amount: 0.5,
            wave_amount: 0.25,
            cloud_density: 6.0,
            cloud_color1: Vec3::new(0.6, 0.65, 0.75),
            cloud_color2: Vec3::new(0.12, 0.12, 0.15),
            upper_color: Vec3::new(0.25, 0.3, 0.4),
            ..Default::default()
        }
    }

    /// Create a preset for twilight/dusk storm
    pub fn twilight_storm() -> Self {
        Self {
            cloud_speed: 0.08,
            cloud_color1: Vec3::new(0.6, 0.45, 0.5), // Purple-ish highlights
            cloud_color2: Vec3::new(0.15, 0.1, 0.15), // Dark purple shadows
            upper_color: Vec3::new(0.4, 0.25, 0.35), // Twilight purple
            fog_color: Vec3::new(0.25, 0.15, 0.2),   // Purple fog
            sun_dir: Vec3::new(-0.8, 0.1, -0.5).normalize(),
            ..Default::default()
        }
    }

    /// Create a preset for an apocalyptic battle arena with lava and meteors
    /// Matching the dramatic concept art: purple sky, orange horizon, fiery atmosphere
    pub fn battle_arena() -> Self {
        Self {
            cloud_speed: 0.12, // Faster, more dramatic
            flow_speed: 0.04,
            flow_amount: 0.5,
            wave_amount: 0.3,
            wave_distort: 0.08,
            cloud_density: 2.0, // Thicker, heavier clouds
            cloud_scale: 1.5,
            cloud_bias: -0.15,
            bump_offset: 0.15,
            parallax_steps: 8.0,
            cloud_height: 80.0,
            world_scale: 1.0,
            // Low sun for dramatic rim lighting from horizon (lava glow)
            sun_dir: Vec3::new(0.0, 0.08, -1.0).normalize(),
            lightning_intensity: 0.8, // Active lightning
            // Cloud colors: vivid purple with fiery orange underlit
            cloud_color1: Vec3::new(0.95, 0.55, 0.3), // Brighter orange-lit cloud edges
            cloud_color2: Vec3::new(0.18, 0.1, 0.25), // Richer purple shadows
            // Sky gradient
            upper_color: Vec3::new(0.2, 0.1, 0.35), // Deeper purple zenith
            // Horizon fog: warm orange glow
            fog_color: Vec3::new(0.7, 0.35, 0.2), // More saturated orange-red
            fog_density: 0.5,                     // Less dense fog in sky
        }
    }
}

/// GPU uniform buffer layout (must match WGSL struct)
/// Using scalar fields for all vec3 to ensure proper alignment
/// Total size: 288 bytes
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SkyUniforms {
    view_proj: [[f32; 4]; 4],     // 64 bytes (offset 0)
    inv_view_proj: [[f32; 4]; 4], // 64 bytes (offset 64)
    camera_pos_x: f32,            // 4 bytes (offset 128)
    camera_pos_y: f32,            // 4 bytes (offset 132)
    camera_pos_z: f32,            // 4 bytes (offset 136)
    time: f32,                    // 4 bytes (offset 140)
    resolution_x: f32,            // 4 bytes (offset 144)
    resolution_y: f32,            // 4 bytes (offset 148)
    cloud_speed: f32,             // 4 bytes (offset 152)
    flow_speed: f32,              // 4 bytes (offset 156)
    flow_amount: f32,             // 4 bytes (offset 160)
    wave_amount: f32,             // 4 bytes (offset 164)
    wave_distort: f32,            // 4 bytes (offset 168)
    cloud_density: f32,           // 4 bytes (offset 172)
    cloud_scale: f32,             // 4 bytes (offset 176)
    cloud_bias: f32,              // 4 bytes (offset 180)
    bump_offset: f32,             // 4 bytes (offset 184)
    parallax_steps: f32,          // 4 bytes (offset 188)
    cloud_height: f32,            // 4 bytes (offset 192)
    world_scale: f32,             // 4 bytes (offset 196)
    light_spread_power1: f32,     // 4 bytes (offset 200)
    light_spread_factor1: f32,    // 4 bytes (offset 204)
    light_spread_power2: f32,     // 4 bytes (offset 208)
    light_spread_factor2: f32,    // 4 bytes (offset 212)
    sun_dir_x: f32,               // 4 bytes (offset 216)
    sun_dir_y: f32,               // 4 bytes (offset 220)
    sun_dir_z: f32,               // 4 bytes (offset 224)
    lightning_intensity: f32,     // 4 bytes (offset 228)
    cloud_color1_r: f32,          // 4 bytes (offset 232)
    cloud_color1_g: f32,          // 4 bytes (offset 236)
    cloud_color1_b: f32,          // 4 bytes (offset 240)
    cloud_color2_r: f32,          // 4 bytes (offset 244)
    cloud_color2_g: f32,          // 4 bytes (offset 248)
    cloud_color2_b: f32,          // 4 bytes (offset 252)
    upper_color_r: f32,           // 4 bytes (offset 256)
    upper_color_g: f32,           // 4 bytes (offset 260)
    upper_color_b: f32,           // 4 bytes (offset 264)
    fog_color_r: f32,             // 4 bytes (offset 268)
    fog_color_g: f32,             // 4 bytes (offset 272)
    fog_color_b: f32,             // 4 bytes (offset 276)
    fog_density: f32,             // 4 bytes (offset 280)
    _pad: f32,                    // 4 bytes (offset 284) - align to 288
}

// Verify struct size at compile time
const _: () = assert!(std::mem::size_of::<SkyUniforms>() == 288);

/// Dark and Stormy Skybox renderer
pub struct StormySky {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    config: StormySkyConfig,
}

impl StormySky {
    /// Create a new stormy sky renderer
    pub fn new(device: &wgpu::Device, surface_format: wgpu::TextureFormat) -> Self {
        Self::with_config(device, surface_format, StormySkyConfig::default())
    }

    /// Create a new stormy sky renderer with custom configuration
    pub fn with_config(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        config: StormySkyConfig,
    ) -> Self {
        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Stormy Sky Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/stormy_sky.wgsl").into(),
            ),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Stormy Sky Uniform Buffer"),
            size: std::mem::size_of::<SkyUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Stormy Sky Bind Group Layout"),
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
            label: Some("Stormy Sky Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Stormy Sky Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create render pipeline (no depth testing - sky is infinitely far)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Stormy Sky Pipeline"),
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
                    blend: None, // No blending, sky writes directly
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
            depth_stencil: None, // Sky doesn't use depth
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        println!("[StormySky] Initialized dark and stormy skybox");

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            config,
        }
    }

    /// Get mutable access to the configuration
    pub fn config_mut(&mut self) -> &mut StormySkyConfig {
        &mut self.config
    }

    /// Get the current configuration
    pub fn config(&self) -> &StormySkyConfig {
        &self.config
    }

    /// Set a new configuration
    pub fn set_config(&mut self, config: StormySkyConfig) {
        self.config = config;
    }

    /// Trigger a lightning flash (will decay automatically)
    pub fn trigger_lightning(&mut self) {
        self.config.lightning_intensity = 1.0;
    }

    /// Update uniform buffer with current camera and time
    pub fn update(
        &mut self,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        camera_pos: Vec3,
        time: f32,
        resolution: (u32, u32),
    ) {
        // Decay lightning
        if self.config.lightning_intensity > 0.0 {
            self.config.lightning_intensity = (self.config.lightning_intensity - 0.05).max(0.0);
        }

        let inv_view_proj = view_proj.inverse();

        let uniforms = SkyUniforms {
            view_proj: view_proj.to_cols_array_2d(),
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            camera_pos_x: camera_pos.x,
            camera_pos_y: camera_pos.y,
            camera_pos_z: camera_pos.z,
            time,
            resolution_x: resolution.0 as f32,
            resolution_y: resolution.1 as f32,
            cloud_speed: self.config.cloud_speed,
            flow_speed: self.config.flow_speed,
            flow_amount: self.config.flow_amount,
            wave_amount: self.config.wave_amount,
            wave_distort: self.config.wave_distort,
            cloud_density: self.config.cloud_density,
            cloud_scale: self.config.cloud_scale,
            cloud_bias: self.config.cloud_bias,
            bump_offset: self.config.bump_offset,
            parallax_steps: self.config.parallax_steps,
            cloud_height: self.config.cloud_height,
            world_scale: self.config.world_scale,
            light_spread_power1: 2.0,  // Inner glow tightness
            light_spread_factor1: 1.0, // Inner glow strength
            light_spread_power2: 50.0, // Outer glow tightness
            light_spread_factor2: 3.0, // Outer glow strength
            sun_dir_x: self.config.sun_dir.x,
            sun_dir_y: self.config.sun_dir.y,
            sun_dir_z: self.config.sun_dir.z,
            lightning_intensity: self.config.lightning_intensity,
            cloud_color1_r: self.config.cloud_color1.x,
            cloud_color1_g: self.config.cloud_color1.y,
            cloud_color1_b: self.config.cloud_color1.z,
            cloud_color2_r: self.config.cloud_color2.x,
            cloud_color2_g: self.config.cloud_color2.y,
            cloud_color2_b: self.config.cloud_color2.z,
            upper_color_r: self.config.upper_color.x,
            upper_color_g: self.config.upper_color.y,
            upper_color_b: self.config.upper_color.z,
            fog_color_r: self.config.fog_color.x,
            fog_color_g: self.config.fog_color.y,
            fog_color_b: self.config.fog_color.z,
            fog_density: self.config.fog_density,
            _pad: 0.0,
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Record render commands into encoder
    /// Should be called BEFORE mesh rendering (sky is background)
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        // Draw fullscreen triangle (3 vertices, no vertex buffer)
        render_pass.draw(0..3, 0..1);
    }

    /// Create a render pass and render the sky
    /// Convenience method that handles render pass creation
    pub fn render_to_view(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Stormy Sky Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.05,
                        g: 0.05,
                        b: 0.08,
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
