//! Sky Bake Dispatch (US-0S04)
//!
//! Dispatches the sky_bake.wgsl render shader across all 6 cubemap faces.
//! Each face is rendered in a separate render pass targeting the corresponding
//! face view of the SkyCubemap texture.

use wgpu::util::DeviceExt;

use super::sky_cubemap::SkyCubemap;

/// Size of the BakeParams uniform: face_index(u32) + _pad(vec3<u32>) = 16 bytes.
const BAKE_PARAMS_SIZE: usize = 16;

/// Holds the render pipeline and bind group layout for sky cubemap baking.
pub struct SkyBakePipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
}

impl SkyBakePipeline {
    /// Create the sky bake render pipeline from sky_bake.wgsl.
    pub fn new(device: &wgpu::Device, sky_bake_source: &str) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky_bake_shader"),
            source: wgpu::ShaderSource::Wgsl(sky_bake_source.into()),
        });

        // @group(0) @binding(0): uniform SkySettings (352 bytes)
        // @group(0) @binding(1): uniform BakeParams (16 bytes)
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("sky_bake_bind_group_layout"),
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
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("sky_bake_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky_bake_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group_layout,
        }
    }
}

/// GPU-side face orientation parameters matching `BakeParams` in sky_bake.wgsl.
///
/// Layout (16 bytes):
/// - face_index: u32
/// - _pad: vec3<u32>
#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuBakeParams {
    face_index: u32,
    _pad: [u32; 3],
}

const _: () = assert!(std::mem::size_of::<GpuBakeParams>() == BAKE_PARAMS_SIZE);

/// Dispatch the sky bake shader across all 6 cubemap faces.
///
/// This issues 6 render passes, one per face, each rendering a fullscreen triangle
/// with the appropriate `face_index` uniform. The face orientation matrices are
/// encoded inside the shader via `cubemap_ray_direction()`.
///
/// # Arguments
/// * `encoder` - Command encoder to record render passes into
/// * `device` - GPU device for creating temporary buffers
/// * `pipeline` - The sky bake render pipeline
/// * `cubemap` - Target cubemap with per-face texture views
/// * `sky_uniforms_buffer` - Buffer containing the 352-byte SkySettings uniform
pub fn dispatch_sky_bake(
    encoder: &mut wgpu::CommandEncoder,
    device: &wgpu::Device,
    pipeline: &SkyBakePipeline,
    cubemap: &SkyCubemap,
    sky_uniforms_buffer: &wgpu::Buffer,
) {
    // Pre-create all 6 face param buffers to minimize per-face overhead.
    let face_buffers: Vec<wgpu::Buffer> = (0..6u32)
        .map(|face| {
            let params = GpuBakeParams {
                face_index: face,
                _pad: [0; 3],
            };
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("sky_bake_face_{face}_params")),
                contents: bytemuck::bytes_of(&params),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        })
        .collect();

    for face in 0..6u32 {
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&format!("sky_bake_face_{face}_bind_group")),
            layout: &pipeline.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: sky_uniforms_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: face_buffers[face as usize].as_entire_binding(),
                },
            ],
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&format!("sky_bake_face_{face}_pass")),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &cubemap.face_views[face as usize],
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_pipeline(&pipeline.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1); // Fullscreen triangle (3 vertices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bake_params_size() {
        assert_eq!(std::mem::size_of::<GpuBakeParams>(), 16);
    }
}
