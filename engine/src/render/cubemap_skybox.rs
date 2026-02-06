//! Cubemap Skybox with Day/Night Crossfade
//!
//! Loads two PNG cubemap sets (day + night) and renders a fullscreen skybox
//! that crossfades between them based on a blend factor driven by the
//! game's DayCycle system.
//!
//! # Usage
//!
//! ```rust,ignore
//! let skybox = CubemapSkybox::new(
//!     &device, &queue, surface_format,
//!     "Assets/Skybox/sky_26_cubemap_2k", // day
//!     "Assets/Skybox/sky_16_cubemap_2k", // night
//! );
//!
//! // Each frame:
//! skybox.update(&queue, view_proj, blend_factor); // 0.0 = day, 1.0 = night
//! skybox.render_to_view(&mut encoder, &view);
//! ```

use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use std::path::Path;

use super::sky_cubemap::SkyCubemap;

/// GPU uniform buffer layout (must match skybox.wgsl SkyboxUniforms)
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct SkyboxUniforms {
    inv_view_proj: [[f32; 4]; 4], // 64 bytes
    blend_factor: f32,            // 4 bytes — 0.0 = day, 1.0 = night
    _pad0: f32,                   // 4 bytes
    _pad1: f32,                   // 4 bytes
    _pad2: f32,                   // 4 bytes — total 80
}

const _: () = assert!(std::mem::size_of::<SkyboxUniforms>() == 80);

/// Cubemap skybox renderer with day/night crossfade.
pub struct CubemapSkybox {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    day_cubemap: SkyCubemap,
    #[allow(dead_code)]
    night_cubemap: SkyCubemap,
}

/// Load a cubemap from 6 PNG face files in a directory.
///
/// Expects files named: px.png, nx.png, py.png, ny.png, pz.png, nz.png
/// Returns a `SkyCubemap` with face data uploaded to the GPU.
fn load_cubemap_from_files(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    folder: &str,
) -> SkyCubemap {
    let face_names = ["px.png", "nx.png", "py.png", "ny.png", "pz.png", "nz.png"];

    // Load and decode first face to get dimensions
    let first_path = Path::new(folder).join(face_names[0]);
    let first_img = image::open(&first_path)
        .unwrap_or_else(|e| panic!("Failed to load cubemap face {}: {}", first_path.display(), e))
        .to_rgba8();
    let size = first_img.width();
    assert_eq!(
        first_img.width(),
        first_img.height(),
        "Cubemap faces must be square, got {}x{}",
        first_img.width(),
        first_img.height()
    );

    println!(
        "[CubemapSkybox] Loading cubemap from {} ({}x{} per face)",
        folder, size, size
    );

    // Create the cubemap texture with COPY_DST for uploading
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some(&format!("cubemap_{folder}")),
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 6,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    // Upload each face
    for (i, name) in face_names.iter().enumerate() {
        let img = if i == 0 {
            first_img.clone()
        } else {
            let path = Path::new(folder).join(name);
            image::open(&path)
                .unwrap_or_else(|e| {
                    panic!("Failed to load cubemap face {}: {}", path.display(), e)
                })
                .to_rgba8()
        };

        assert_eq!(
            img.width(),
            size,
            "All cubemap faces must be the same size"
        );

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: i as u32, // Array layer = cubemap face
                },
                aspect: wgpu::TextureAspect::All,
            },
            &img,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * size),
                rows_per_image: Some(size),
            },
            wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
        );
    }

    // Create cube view and sampler
    let cube_view = texture.create_view(&wgpu::TextureViewDescriptor {
        label: Some(&format!("cubemap_{folder}_cube_view")),
        dimension: Some(wgpu::TextureViewDimension::Cube),
        array_layer_count: Some(6),
        ..Default::default()
    });

    let face_views = std::array::from_fn(|i| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some(&format!("cubemap_{folder}_face_{i}")),
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: i as u32,
            array_layer_count: Some(1),
            ..Default::default()
        })
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some(&format!("cubemap_{folder}_sampler")),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        ..Default::default()
    });

    println!("[CubemapSkybox] Loaded {} (6 faces, {}x{})", folder, size, size);

    SkyCubemap {
        texture,
        cube_view,
        face_views,
        sampler,
        size,
    }
}

impl CubemapSkybox {
    /// Create a new cubemap skybox with day and night cubemap folders.
    ///
    /// Each folder must contain: px.png, nx.png, py.png, ny.png, pz.png, nz.png
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        surface_format: wgpu::TextureFormat,
        day_folder: &str,
        night_folder: &str,
    ) -> Self {
        // Load cubemap textures from PNG files
        let day_cubemap = load_cubemap_from_files(device, queue, day_folder);
        let night_cubemap = load_cubemap_from_files(device, queue, night_folder);

        // Load shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Skybox Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../shaders/skybox.wgsl").into(),
            ),
        });

        // Uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Skybox Uniform Buffer"),
            size: std::mem::size_of::<SkyboxUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bind group layout:
        //   0: uniforms
        //   1: day cubemap texture
        //   2: sampler
        //   3: night cubemap texture
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Skybox Bind Group Layout"),
            entries: &[
                // Uniforms
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
                // Day cubemap
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler (shared)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                // Night cubemap
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::Cube,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        // Bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Skybox Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&day_cubemap.cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&day_cubemap.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&night_cubemap.cube_view),
                },
            ],
        });

        // Pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Skybox Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Render pipeline (fullscreen triangle, no depth, no vertex buffers)
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Skybox Pipeline"),
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
            depth_stencil: None, // Sky has no depth
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        println!(
            "[CubemapSkybox] Initialized with day/night crossfade (day: {}, night: {})",
            day_folder, night_folder
        );

        Self {
            pipeline,
            uniform_buffer,
            bind_group,
            day_cubemap,
            night_cubemap,
        }
    }

    /// Update uniforms for the current frame.
    ///
    /// `blend_factor`: 0.0 = pure day sky, 1.0 = pure night sky
    pub fn update(&self, queue: &wgpu::Queue, view_proj: Mat4, blend_factor: f32) {
        let inv_view_proj = view_proj.inverse();

        let uniforms = SkyboxUniforms {
            inv_view_proj: inv_view_proj.to_cols_array_2d(),
            blend_factor: blend_factor.clamp(0.0, 1.0),
            _pad0: 0.0,
            _pad1: 0.0,
            _pad2: 0.0,
        };

        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Render the skybox to the given texture view.
    ///
    /// Clears the view and draws the cubemap fullscreen.
    pub fn render_to_view(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Skybox Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
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

    /// Record skybox draw commands into an existing render pass.
    pub fn render<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Fullscreen triangle
    }
}
