//! Fog Post-Process Pass Module
//!
//! Fullscreen post-process fog that applies depth-based atmospheric fog:
//! - Distance fog (exponential falloff with distance from camera)
//! - Height fog (thicker near ground level, thinner at elevation)
//! - Lava steam wall (dense steam/fog ring around island boundaries)
//! - Wind-driven steam gusts that push fog onto island edges
//! - World position reconstruction from depth buffer
//!
//! # Usage
//!
//! ```rust,ignore
//! // Initialize
//! let fog_pass = FogPostPass::new(&device, surface_format);
//!
//! // Configure lava steam boundary
//! fog_pass.set_steam_config(LavaSteamConfig::battle_arena(
//!     Vec3::new(0.0, 0.0, 45.0),   // island 1 center
//!     Vec3::new(0.0, 0.0, -45.0),  // island 2 center
//!     30.0, -18.0,                   // island radius, lava Y
//! ));
//!
//! // Each frame - update with camera matrices and time
//! fog_pass.update(&queue, view_proj, camera_pos, time);
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

/// Configuration for lava steam boundary around islands.
///
/// Creates a dense wall of animated steam/fog at the island edges,
/// rising from the lava surface to hide the outer world and focus
/// gameplay on the island arena. Wind periodically pushes steam
/// wisps onto the island edges.
#[derive(Clone, Copy, Debug)]
pub struct LavaSteamConfig {
    /// Steam/fog color (warm white-orange from lava heat)
    pub steam_color: Vec3,
    /// Steam density multiplier (0 = disabled, 1..4 = light..heavy)
    pub steam_density: f32,
    /// How high steam rises above the lava surface (meters)
    pub steam_height: f32,
    /// Wind gust strength (0 = calm, 0.5..1.5 = light..strong gusts)
    pub wind_strength: f32,
    /// How soft the steam edge transition is (meters, 5..25)
    pub steam_edge_softness: f32,
    /// Center of island 1 (XZ plane position)
    pub island1_center: Vec3,
    /// Center of island 2 (XZ plane position)
    pub island2_center: Vec3,
    /// Island hexagonal radius (meters)
    pub island_radius: f32,
    /// Y level of the lava ocean surface
    pub lava_y: f32,
}

impl LavaSteamConfig {
    /// Create a battle arena preset with the given island geometry.
    ///
    /// Steam rises from lava surface (at terrain level) creating a dense
    /// wall around the islands that hides the outer world.
    pub fn battle_arena(
        island1_center: Vec3,
        island2_center: Vec3,
        island_radius: f32,
        lava_y: f32,
    ) -> Self {
        Self {
            steam_color: Vec3::new(0.72, 0.68, 0.62), // Warm white-gray steam
            steam_density: 4.0,                       // Thick wall — blocks view
            steam_height: 30.0,                       // Tall enough to block sky at edge
            wind_strength: 0.6,                       // Moderate gusts push wisps inward
            steam_edge_softness: 10.0, // Tight edge — wall starts right at island rim
            island1_center,
            island2_center,
            island_radius,
            lava_y,
        }
    }
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
            fog_color: Vec3::new(0.45, 0.35, 0.40), // Warm haze
            density: 0.003,                         // Very light distance fog
            height_fog_start: -2.0,                 // Only below terrain
            height_fog_density: 0.015,              // Minimal height fog
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
/// Total size: 176 bytes
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct FogUniforms {
    fog_color: [f32; 3],          // 12 bytes (offset 0)
    density: f32,                 // 4 bytes (offset 12) - total 16
    height_fog_start: f32,        // 4 bytes (offset 16)
    height_fog_density: f32,      // 4 bytes (offset 20)
    _pad0: [f32; 2],              // 8 bytes (offset 24) - align to 32
    inv_view_proj: [[f32; 4]; 4], // 64 bytes (offset 32) - total 96
    camera_pos: [f32; 3],         // 12 bytes (offset 96)
    _pad1: f32,                   // 4 bytes (offset 108) - total 112
    // Lava steam parameters
    steam_color: [f32; 3],    // 12 bytes (offset 112)
    steam_density: f32,       // 4 bytes (offset 124) - total 128
    island1_center: [f32; 3], // 12 bytes (offset 128)
    island_radius: f32,       // 4 bytes (offset 140) - total 144
    island2_center: [f32; 3], // 12 bytes (offset 144)
    lava_y: f32,              // 4 bytes (offset 156) - total 160
    steam_height: f32,        // 4 bytes (offset 160)
    wind_time: f32,           // 4 bytes (offset 164)
    wind_strength: f32,       // 4 bytes (offset 168)
    steam_edge_softness: f32, // 4 bytes (offset 172) - total 176
}

// Verify struct size at compile time
const _: () = assert!(std::mem::size_of::<FogUniforms>() == 176);

/// Size of the 3D noise texture (64^3 = 262144 texels, ~256KB)
const NOISE_3D_SIZE: u32 = 64;

/// Fog Post-Process Pass renderer
/// Applies depth-based distance and height fog to the scene,
/// plus lava steam boundary around islands using pre-baked 3D Perlin noise.
pub struct FogPostPass {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    config: FogPostConfig,
    steam_config: Option<LavaSteamConfig>,
    #[allow(dead_code)]
    noise_3d_texture: wgpu::Texture,
    noise_3d_view: wgpu::TextureView,
    noise_3d_sampler: wgpu::Sampler,
}

impl FogPostPass {
    /// Create a new fog post-process pass
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
    ) -> Self {
        Self::with_config(device, queue, surface_format, FogPostConfig::default())
    }

    /// Create a new fog post-process pass with custom configuration
    pub fn with_config(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        config: FogPostConfig,
    ) -> Self {
        // ============================================
        // Pre-bake 3D tileable Perlin noise texture
        // ============================================
        let noise_data = generate_tileable_perlin_3d(NOISE_3D_SIZE, 4);
        let noise_3d_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Steam 3D Noise Texture"),
            size: wgpu::Extent3d {
                width: NOISE_3D_SIZE,
                height: NOISE_3D_SIZE,
                depth_or_array_layers: NOISE_3D_SIZE,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &noise_3d_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &noise_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(NOISE_3D_SIZE),
                rows_per_image: Some(NOISE_3D_SIZE),
            },
            wgpu::Extent3d {
                width: NOISE_3D_SIZE,
                height: NOISE_3D_SIZE,
                depth_or_array_layers: NOISE_3D_SIZE,
            },
        );
        let noise_3d_view = noise_3d_texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Steam 3D Noise View"),
            dimension: Some(wgpu::TextureViewDimension::D3),
            ..Default::default()
        });
        let noise_3d_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Steam 3D Noise Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        println!(
            "[FogPostPass] Pre-baked 3D Perlin noise: {}x{}x{} ({} bytes)",
            NOISE_3D_SIZE,
            NOISE_3D_SIZE,
            NOISE_3D_SIZE,
            noise_data.len()
        );

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
        // Binding 2: Sampler (scene)
        // Binding 3: Depth texture
        // Binding 4: 3D noise texture
        // Binding 5: Noise sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Fog Post-Process Bind Group Layout"),
            entries: &[
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
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
                // 3D noise texture
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
                        multisampled: false,
                    },
                    count: None,
                },
                // Noise sampler (repeat)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
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

        println!("[FogPostPass] Initialized with 3D Perlin noise steam");

        Self {
            pipeline,
            uniform_buffer,
            bind_group_layout,
            sampler,
            config,
            steam_config: None,
            noise_3d_texture,
            noise_3d_view,
            noise_3d_sampler,
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

    /// Configure lava steam boundary around islands.
    ///
    /// When set, creates a dense wall of animated steam/fog at the island
    /// edges that hides the outer world and focuses gameplay on the arena.
    pub fn set_steam_config(&mut self, config: LavaSteamConfig) {
        println!(
            "[FogPostPass] Lava steam enabled: density={:.1}, height={:.0}m, wind={:.1}",
            config.steam_density, config.steam_height, config.wind_strength
        );
        self.steam_config = Some(config);
    }

    /// Update uniform buffer with current camera matrices and time.
    ///
    /// `time` drives wind animation for the lava steam boundary.
    pub fn update(&self, queue: &wgpu::Queue, view_proj: Mat4, camera_pos: Vec3, time: f32) {
        let inv_view_proj = view_proj.inverse();

        // Steam defaults (zeroed = disabled)
        let (
            steam_color,
            steam_density,
            island1_center,
            island_radius,
            island2_center,
            lava_y,
            steam_height,
            wind_strength,
            steam_edge_softness,
        ) = if let Some(ref s) = self.steam_config {
            (
                s.steam_color.to_array(),
                s.steam_density,
                s.island1_center.to_array(),
                s.island_radius,
                s.island2_center.to_array(),
                s.lava_y,
                s.steam_height,
                s.wind_strength,
                s.steam_edge_softness,
            )
        } else {
            ([0.0; 3], 0.0, [0.0; 3], 0.0, [0.0; 3], 0.0, 0.0, 0.0, 0.0)
        };

        let uniforms = FogUniforms {
            fog_color: self.config.fog_color.to_array(),
            density: self.config.density,
            height_fog_start: self.config.height_fog_start,
            height_fog_density: self.config.height_fog_density,
            _pad0: [0.0; 2],
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            camera_pos: camera_pos.to_array(),
            _pad1: 0.0,
            // Lava steam
            steam_color,
            steam_density,
            island1_center,
            island_radius,
            island2_center,
            lava_y,
            steam_height,
            wind_time: time,
            wind_strength,
            steam_edge_softness,
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
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&self.noise_3d_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&self.noise_3d_sampler),
                },
            ],
        })
    }

    /// Record render commands into an existing render pass
    pub fn render<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        bind_group: &'a wgpu::BindGroup,
    ) {
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

// ============================================================================
// 3D TILEABLE PERLIN NOISE GENERATION
// ============================================================================

/// Perlin fade function: 6t^5 - 15t^4 + 10t^3
#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

/// 3D gradient from hash
#[inline]
fn grad3d(hash: u8, x: f32, y: f32, z: f32) -> f32 {
    let h = hash & 15;
    let u = if h < 8 { x } else { y };
    let v = if h < 4 {
        y
    } else if h == 12 || h == 14 {
        x
    } else {
        z
    };
    (if h & 1 == 0 { u } else { -u }) + (if h & 2 == 0 { v } else { -v })
}

/// Standard Perlin permutation table (doubled for wrapping)
fn perm_table() -> [u8; 512] {
    const P: [u8; 256] = [
        151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225, 140, 36, 103, 30,
        69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148, 247, 120, 234, 75, 0, 26, 197, 62, 94,
        252, 219, 203, 117, 35, 11, 32, 57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171,
        168, 68, 175, 74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122, 60,
        211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54, 65, 25, 63, 161, 1,
        216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169, 200, 196, 135, 130, 116, 188, 159, 86,
        164, 100, 109, 198, 173, 186, 3, 64, 52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118,
        126, 255, 82, 85, 212, 207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170,
        213, 119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9, 129, 22, 39,
        253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104, 218, 246, 97, 228, 251, 34,
        242, 193, 238, 210, 144, 12, 191, 179, 162, 241, 81, 51, 145, 235, 249, 14, 239, 107, 49,
        192, 214, 31, 181, 199, 106, 157, 184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254,
        138, 236, 205, 93, 222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
    ];
    let mut perm = [0u8; 512];
    for i in 0..256 {
        perm[i] = P[i];
        perm[i + 256] = P[i];
    }
    perm
}

/// Tileable 3D Perlin noise at a single point.
/// Wraps seamlessly at `period` boundaries.
fn tileable_perlin_3d(x: f32, y: f32, z: f32, period: f32, perm: &[u8; 512]) -> f32 {
    let pi = period as i32;

    let xi = (x.floor() as i32).rem_euclid(pi) as usize;
    let yi = (y.floor() as i32).rem_euclid(pi) as usize;
    let zi = (z.floor() as i32).rem_euclid(pi) as usize;
    let xi1 = ((xi as i32 + 1).rem_euclid(pi)) as usize;
    let yi1 = ((yi as i32 + 1).rem_euclid(pi)) as usize;
    let zi1 = ((zi as i32 + 1).rem_euclid(pi)) as usize;

    let xf = x - x.floor();
    let yf = y - y.floor();
    let zf = z - z.floor();

    let u = fade(xf);
    let v = fade(yf);
    let w = fade(zf);

    let aaa = perm[perm[perm[xi] as usize + yi] as usize + zi];
    let aba = perm[perm[perm[xi] as usize + yi1] as usize + zi];
    let aab = perm[perm[perm[xi] as usize + yi] as usize + zi1];
    let abb = perm[perm[perm[xi] as usize + yi1] as usize + zi1];
    let baa = perm[perm[perm[xi1] as usize + yi] as usize + zi];
    let bba = perm[perm[perm[xi1] as usize + yi1] as usize + zi];
    let bab = perm[perm[perm[xi1] as usize + yi] as usize + zi1];
    let bbb = perm[perm[perm[xi1] as usize + yi1] as usize + zi1];

    let x1 = lerp(
        lerp(grad3d(aaa, xf, yf, zf), grad3d(baa, xf - 1.0, yf, zf), u),
        lerp(
            grad3d(aba, xf, yf - 1.0, zf),
            grad3d(bba, xf - 1.0, yf - 1.0, zf),
            u,
        ),
        v,
    );
    let x2 = lerp(
        lerp(
            grad3d(aab, xf, yf, zf - 1.0),
            grad3d(bab, xf - 1.0, yf, zf - 1.0),
            u,
        ),
        lerp(
            grad3d(abb, xf, yf - 1.0, zf - 1.0),
            grad3d(bbb, xf - 1.0, yf - 1.0, zf - 1.0),
            u,
        ),
        v,
    );
    lerp(x1, x2, w)
}

/// Generate a 3D tileable Perlin noise texture (R8 format).
/// Multi-octave FBM for natural-looking volumetric fog.
fn generate_tileable_perlin_3d(size: u32, octaves: u32) -> Vec<u8> {
    let perm = perm_table();
    let s = size as usize;
    let mut data = vec![0u8; s * s * s];

    for z in 0..size {
        for y in 0..size {
            for x in 0..size {
                let mut value = 0.0f32;
                let mut amplitude = 1.0f32;
                let mut frequency = 1.0f32;
                let mut max_val = 0.0f32;

                for _ in 0..octaves {
                    let nx = x as f32 * frequency / size as f32;
                    let ny = y as f32 * frequency / size as f32;
                    let nz = z as f32 * frequency / size as f32;
                    value += tileable_perlin_3d(nx, ny, nz, frequency, &perm) * amplitude;
                    max_val += amplitude;
                    amplitude *= 0.5;
                    frequency *= 2.0;
                }

                let normalized = ((value / max_val) + 1.0) * 0.5;
                let idx = (z as usize * s + y as usize) * s + x as usize;
                data[idx] = (normalized.clamp(0.0, 1.0) * 255.0) as u8;
            }
        }
    }

    data
}
