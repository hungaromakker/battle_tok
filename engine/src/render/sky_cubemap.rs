//! Sky Cubemap - 6-face cubemap texture for sky rendering (US-0S01)

/// Holds a cubemap texture (6 faces), cube texture view, and linear sampler
/// for sky rendering. Each face is renderable as a render target.
pub struct SkyCubemap {
    pub texture: wgpu::Texture,
    pub cube_view: wgpu::TextureView,
    pub face_views: [wgpu::TextureView; 6],
    pub sampler: wgpu::Sampler,
    pub size: u32,
}

impl SkyCubemap {
    /// Create a new sky cubemap with the given face resolution (e.g. 512 or 1024).
    pub fn new(device: &wgpu::Device, size: u32) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sky_cubemap"),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let cube_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("sky_cubemap_cube_view"),
            dimension: Some(wgpu::TextureViewDimension::Cube),
            array_layer_count: Some(6),
            ..Default::default()
        });

        let face_views = std::array::from_fn(|i| {
            texture.create_view(&wgpu::TextureViewDescriptor {
                label: Some(&format!("sky_cubemap_face_{i}")),
                dimension: Some(wgpu::TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            })
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("sky_cubemap_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        Self {
            texture,
            cube_view,
            face_views,
            sampler,
            size,
        }
    }

    /// Returns bind group entries for the cubemap view (binding 0) and sampler (binding 1),
    /// starting at the given binding index.
    pub fn get_bind_group_entries(&self, base_binding: u32) -> [wgpu::BindGroupEntry<'_>; 2] {
        [
            wgpu::BindGroupEntry {
                binding: base_binding,
                resource: wgpu::BindingResource::TextureView(&self.cube_view),
            },
            wgpu::BindGroupEntry {
                binding: base_binding + 1,
                resource: wgpu::BindingResource::Sampler(&self.sampler),
            },
        ]
    }
}
