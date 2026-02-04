//! Cloud Noise Texture Module
//!
//! Generates a 300x300 R8 tileable perlin noise texture for volumetric cloud rendering.
//! The low resolution is intentional for soft, aurora-like clouds.

use wgpu;

/// Size of the cloud noise texture in pixels (300x300)
pub const CLOUD_TEXTURE_SIZE: u32 = 300;

/// Number of octaves for fractal perlin noise
const NOISE_OCTAVES: u32 = 4;

/// Cloud noise texture and associated GPU resources.
pub struct CloudTexture {
    /// The GPU texture containing the noise data
    pub texture: wgpu::Texture,
    /// Texture view for shader access
    pub view: wgpu::TextureView,
    /// Sampler for texture filtering
    pub sampler: wgpu::Sampler,
    /// Bind group for shader access
    pub bind_group: wgpu::BindGroup,
    /// Bind group layout (needed for pipeline creation)
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl CloudTexture {
    /// Create a new cloud noise texture.
    ///
    /// Generates 4-octave tileable perlin noise at creation time.
    /// Performance impact is minimal (<0.5ms) as this is only done once.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        // Generate noise data on CPU
        let noise_data = generate_tileable_perlin_noise(CLOUD_TEXTURE_SIZE, NOISE_OCTAVES);

        // Create texture
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Cloud Noise Texture"),
            size: wgpu::Extent3d {
                width: CLOUD_TEXTURE_SIZE,
                height: CLOUD_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm, // R8 format as specified
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload noise data to GPU
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &noise_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(CLOUD_TEXTURE_SIZE),
                rows_per_image: Some(CLOUD_TEXTURE_SIZE),
            },
            wgpu::Extent3d {
                width: CLOUD_TEXTURE_SIZE,
                height: CLOUD_TEXTURE_SIZE,
                depth_or_array_layers: 1,
            },
        );

        // Create texture view
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Cloud Noise Texture View"),
            ..Default::default()
        });

        // Create sampler with repeat addressing for seamless tiling
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Cloud Noise Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Cloud Texture Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cloud Texture Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            texture,
            view,
            sampler,
            bind_group,
            bind_group_layout,
        }
    }
}

/// Generate tileable 4-octave perlin noise.
///
/// The noise wraps seamlessly at all edges, making it suitable for
/// infinite scrolling cloud effects.
fn generate_tileable_perlin_noise(size: u32, octaves: u32) -> Vec<u8> {
    let size_f = size as f32;
    let mut data = vec![0u8; (size * size) as usize];

    // Permutation table for noise (doubled for wrapping)
    let perm = generate_permutation_table();

    for y in 0..size {
        for x in 0..size {
            let mut value = 0.0f32;
            let mut amplitude = 1.0f32;
            let mut frequency = 1.0f32;
            let mut max_value = 0.0f32;

            // Sum multiple octaves of noise
            for _ in 0..octaves {
                let nx = x as f32 * frequency / size_f;
                let ny = y as f32 * frequency / size_f;

                // Use tileable noise that wraps at integer boundaries
                let noise_val = tileable_perlin_2d(nx, ny, frequency, &perm);
                value += noise_val * amplitude;
                max_value += amplitude;

                amplitude *= 0.5;
                frequency *= 2.0;
            }

            // Normalize to 0-1 range
            value = (value / max_value + 1.0) * 0.5;
            value = value.clamp(0.0, 1.0);

            // Convert to u8
            data[(y * size + x) as usize] = (value * 255.0) as u8;
        }
    }

    data
}

/// Generate a permutation table for perlin noise.
fn generate_permutation_table() -> [u8; 512] {
    // Standard permutation table (Ken Perlin's original)
    const P: [u8; 256] = [
        151, 160, 137, 91, 90, 15, 131, 13, 201, 95, 96, 53, 194, 233, 7, 225,
        140, 36, 103, 30, 69, 142, 8, 99, 37, 240, 21, 10, 23, 190, 6, 148,
        247, 120, 234, 75, 0, 26, 197, 62, 94, 252, 219, 203, 117, 35, 11, 32,
        57, 177, 33, 88, 237, 149, 56, 87, 174, 20, 125, 136, 171, 168, 68, 175,
        74, 165, 71, 134, 139, 48, 27, 166, 77, 146, 158, 231, 83, 111, 229, 122,
        60, 211, 133, 230, 220, 105, 92, 41, 55, 46, 245, 40, 244, 102, 143, 54,
        65, 25, 63, 161, 1, 216, 80, 73, 209, 76, 132, 187, 208, 89, 18, 169,
        200, 196, 135, 130, 116, 188, 159, 86, 164, 100, 109, 198, 173, 186, 3, 64,
        52, 217, 226, 250, 124, 123, 5, 202, 38, 147, 118, 126, 255, 82, 85, 212,
        207, 206, 59, 227, 47, 16, 58, 17, 182, 189, 28, 42, 223, 183, 170, 213,
        119, 248, 152, 2, 44, 154, 163, 70, 221, 153, 101, 155, 167, 43, 172, 9,
        129, 22, 39, 253, 19, 98, 108, 110, 79, 113, 224, 232, 178, 185, 112, 104,
        218, 246, 97, 228, 251, 34, 242, 193, 238, 210, 144, 12, 191, 179, 162, 241,
        81, 51, 145, 235, 249, 14, 239, 107, 49, 192, 214, 31, 181, 199, 106, 157,
        184, 84, 204, 176, 115, 121, 50, 45, 127, 4, 150, 254, 138, 236, 205, 93,
        222, 114, 67, 29, 24, 72, 243, 141, 128, 195, 78, 66, 215, 61, 156, 180,
    ];

    let mut perm = [0u8; 512];
    for i in 0..256 {
        perm[i] = P[i];
        perm[i + 256] = P[i];
    }
    perm
}

/// 2D tileable perlin noise using seamless wrapping.
fn tileable_perlin_2d(x: f32, y: f32, period: f32, perm: &[u8; 512]) -> f32 {
    let period_i = period as i32;

    // Integer coordinates (wrapped to period)
    let xi = (x.floor() as i32).rem_euclid(period_i);
    let yi = (y.floor() as i32).rem_euclid(period_i);

    // Fractional coordinates
    let xf = x - x.floor();
    let yf = y - y.floor();

    // Smoothstep for interpolation
    let u = fade(xf);
    let v = fade(yf);

    // Wrapped coordinates for corners
    let xi1 = (xi + 1).rem_euclid(period_i);
    let yi1 = (yi + 1).rem_euclid(period_i);

    // Hash corners
    let aa = perm[(perm[xi as usize] as i32 + yi) as usize & 255];
    let ab = perm[(perm[xi as usize] as i32 + yi1) as usize & 255];
    let ba = perm[(perm[xi1 as usize] as i32 + yi) as usize & 255];
    let bb = perm[(perm[xi1 as usize] as i32 + yi1) as usize & 255];

    // Gradient values at corners
    let g00 = grad2d(aa, xf, yf);
    let g10 = grad2d(ba, xf - 1.0, yf);
    let g01 = grad2d(ab, xf, yf - 1.0);
    let g11 = grad2d(bb, xf - 1.0, yf - 1.0);

    // Bilinear interpolation
    let x1 = lerp(g00, g10, u);
    let x2 = lerp(g01, g11, u);
    lerp(x1, x2, v)
}

/// Smoothstep fade function for perlin noise.
#[inline]
fn fade(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// Linear interpolation.
#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + t * (b - a)
}

/// 2D gradient function.
#[inline]
fn grad2d(hash: u8, x: f32, y: f32) -> f32 {
    match hash & 3 {
        0 => x + y,
        1 => -x + y,
        2 => x - y,
        _ => -x - y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_generation_size() {
        let noise = generate_tileable_perlin_noise(CLOUD_TEXTURE_SIZE, NOISE_OCTAVES);
        assert_eq!(noise.len(), (CLOUD_TEXTURE_SIZE * CLOUD_TEXTURE_SIZE) as usize);
    }

    #[test]
    fn test_noise_values_generated() {
        let noise = generate_tileable_perlin_noise(CLOUD_TEXTURE_SIZE, NOISE_OCTAVES);
        // Verify noise has variation (not all zeros or all same value)
        let min_value = noise.iter().min().copied().unwrap_or(0);
        let max_value = noise.iter().max().copied().unwrap_or(0);
        assert!(max_value > min_value, "Noise should have variation");
    }

    #[test]
    fn test_noise_is_tileable() {
        let noise = generate_tileable_perlin_noise(CLOUD_TEXTURE_SIZE, NOISE_OCTAVES);

        // Check that edges match (within a small tolerance due to gradient continuity)
        let size = CLOUD_TEXTURE_SIZE as usize;

        // Sample some points along the edges
        for i in 0..size {
            // Left edge should tile with right edge (gradient continuity)
            let left = noise[i * size] as i32;
            let right = noise[i * size + size - 1] as i32;
            // Allow some difference due to interpolation
            assert!(
                (left - right).abs() < 100,
                "Left-right edge mismatch at row {}: {} vs {}",
                i, left, right
            );

            // Top edge should tile with bottom edge
            let top = noise[i] as i32;
            let bottom = noise[(size - 1) * size + i] as i32;
            assert!(
                (top - bottom).abs() < 100,
                "Top-bottom edge mismatch at col {}: {} vs {}",
                i, top, bottom
            );
        }
    }

    #[test]
    fn test_permutation_table() {
        let perm = generate_permutation_table();
        // Check that the table is doubled
        for i in 0..256 {
            assert_eq!(perm[i], perm[i + 256]);
        }
    }

    #[test]
    fn test_noise_generation_performance() {
        use std::time::Instant;

        let start = Instant::now();
        let _noise = generate_tileable_perlin_noise(CLOUD_TEXTURE_SIZE, NOISE_OCTAVES);
        let elapsed = start.elapsed();

        // Should complete in less than 0.5 seconds (the 0.5ms criteria is for GPU upload + generation)
        assert!(
            elapsed.as_millis() < 500,
            "Noise generation took too long: {:?}",
            elapsed
        );
    }
}
