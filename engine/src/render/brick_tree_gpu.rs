use crate::game::systems::voxel_building::{BrickLeaf64, BrickNode, RaymarchQualityState};
use wgpu::util::DeviceExt;

/// GPU buffers for BrickTree traversal payloads in raymarch passes.
pub struct BrickTreeGpuBuffers {
    pub node_buffer: wgpu::Buffer,
    pub leaf_buffer: wgpu::Buffer,
    pub node_count: u32,
    pub leaf_count: u32,
}

impl BrickTreeGpuBuffers {
    pub fn create_empty(device: &wgpu::Device) -> Self {
        let node_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BrickTree Node Buffer"),
            size: std::mem::size_of::<BrickNode>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let leaf_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BrickTree Leaf Buffer"),
            size: std::mem::size_of::<BrickLeaf64>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            node_buffer,
            leaf_buffer,
            node_count: 0,
            leaf_count: 0,
        }
    }

    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, nodes: &[BrickNode], leaves: &[BrickLeaf64]) {
        let node_bytes = bytemuck::cast_slice(nodes);
        let leaf_bytes = bytemuck::cast_slice(leaves);

        if !node_bytes.is_empty() {
            self.node_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("BrickTree Node Buffer"),
                contents: node_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });
        } else {
            queue.write_buffer(&self.node_buffer, 0, &[0u8; 4]);
        }

        if !leaf_bytes.is_empty() {
            self.leaf_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("BrickTree Leaf Buffer"),
                contents: leaf_bytes,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            });
        } else {
            queue.write_buffer(&self.leaf_buffer, 0, &[0u8; 4]);
        }

        self.node_count = nodes.len() as u32;
        self.leaf_count = leaves.len() as u32;
    }
}

/// Runtime raymarch quality controls (dynamic resolution + step multiplier).
#[derive(Debug, Clone, Copy)]
pub struct RaymarchShellQuality {
    pub state: RaymarchQualityState,
}

impl Default for RaymarchShellQuality {
    fn default() -> Self {
        Self {
            state: RaymarchQualityState::default(),
        }
    }
}
