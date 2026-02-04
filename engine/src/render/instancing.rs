//! GPU Instance Buffer System for Creature Rendering
//!
//! This module provides GPU-compatible instance data structures for rendering
//! up to 2000 creature entities using instanced rendering.

use wgpu::util::DeviceExt;

/// Maximum number of creature instances supported (96 KB / 48 bytes = 2000)
pub const MAX_CREATURE_INSTANCES: usize = 2000;

/// Total buffer size in bytes (2000 * 48 = 96,000 bytes = 96 KB)
pub const INSTANCE_BUFFER_SIZE: usize = MAX_CREATURE_INSTANCES * std::mem::size_of::<CreatureInstance>();

/// GPU instance data for a single creature entity.
///
/// Layout (48 bytes total, 16-byte aligned for GPU compatibility):
/// - position:        vec3<f32> (12 bytes) - World position
/// - _pad0:           u32 (4 bytes) - Padding for alignment
/// - rotation:        vec4<f32> (16 bytes) - Quaternion rotation
/// - scale:           f32 (4 bytes) - Uniform scale factor
/// - baked_sdf_id:    u32 (4 bytes) - ID of the baked SDF model
/// - animation_state: u32 (4 bytes) - Current animation state/frame
/// - tint_color:      u32 (4 bytes) - Packed RGBA color (8 bits per channel)
///
/// Total: 12 + 4 + 16 + 4 + 4 + 4 + 4 = 48 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CreatureInstance {
    /// World position (x, y, z) - 12 bytes
    pub position: [f32; 3],
    /// Padding to align rotation quaternion to 16-byte boundary - 4 bytes
    pub _pad0: u32,
    /// Rotation quaternion (x, y, z, w) - 16 bytes
    pub rotation: [f32; 4],
    /// Uniform scale factor - 4 bytes
    pub scale: f32,
    /// ID referencing a baked SDF model - 4 bytes
    pub baked_sdf_id: u32,
    /// Animation state/frame identifier - 4 bytes
    pub animation_state: u32,
    /// Packed RGBA tint color (0xRRGGBBAA format) - 4 bytes
    pub tint_color: u32,
}

// Compile-time assertion to verify struct size is exactly 48 bytes
const _: () = {
    assert!(
        std::mem::size_of::<CreatureInstance>() == 48,
        "CreatureInstance must be exactly 48 bytes for GPU instancing"
    );
};

impl Default for CreatureInstance {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            _pad0: 0,
            rotation: [0.0, 0.0, 0.0, 1.0], // Identity quaternion
            scale: 1.0,
            baked_sdf_id: 0,
            animation_state: 0,
            tint_color: 0xFFFFFFFF, // White, fully opaque
        }
    }
}

impl CreatureInstance {
    /// Create a new creature instance with the given parameters.
    pub fn new(
        position: [f32; 3],
        rotation: [f32; 4],
        scale: f32,
        baked_sdf_id: u32,
        animation_state: u32,
        tint_color: u32,
    ) -> Self {
        Self {
            position,
            _pad0: 0,
            rotation,
            scale,
            baked_sdf_id,
            animation_state,
            tint_color,
        }
    }

    /// Create a simple instance at a position with default rotation and scale.
    pub fn at_position(position: [f32; 3], baked_sdf_id: u32) -> Self {
        Self {
            position,
            baked_sdf_id,
            ..Default::default()
        }
    }

    /// Set position and return self for chaining.
    pub fn with_position(mut self, position: [f32; 3]) -> Self {
        self.position = position;
        self
    }

    /// Set rotation quaternion and return self for chaining.
    pub fn with_rotation(mut self, rotation: [f32; 4]) -> Self {
        self.rotation = rotation;
        self
    }

    /// Set scale and return self for chaining.
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    /// Set baked SDF ID and return self for chaining.
    pub fn with_sdf_id(mut self, baked_sdf_id: u32) -> Self {
        self.baked_sdf_id = baked_sdf_id;
        self
    }

    /// Set animation state and return self for chaining.
    pub fn with_animation_state(mut self, animation_state: u32) -> Self {
        self.animation_state = animation_state;
        self
    }

    /// Set tint color and return self for chaining.
    pub fn with_tint_color(mut self, tint_color: u32) -> Self {
        self.tint_color = tint_color;
        self
    }
}

/// Pack RGBA color components into a single u32 value.
/// Format: 0xRRGGBBAA
#[inline]
pub fn pack_rgba(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((r as u32) << 24) | ((g as u32) << 16) | ((b as u32) << 8) | (a as u32)
}

/// Unpack a u32 color value into RGBA components.
#[inline]
pub fn unpack_rgba(packed: u32) -> (u8, u8, u8, u8) {
    let r = ((packed >> 24) & 0xFF) as u8;
    let g = ((packed >> 16) & 0xFF) as u8;
    let b = ((packed >> 8) & 0xFF) as u8;
    let a = (packed & 0xFF) as u8;
    (r, g, b, a)
}

/// Create a GPU instance buffer that can hold up to MAX_CREATURE_INSTANCES entities.
///
/// # Arguments
/// * `device` - The wgpu device to create the buffer on
/// * `label` - Optional label for debugging
///
/// # Returns
/// A wgpu::Buffer configured for vertex instance data with COPY_DST usage.
pub fn create_instance_buffer(device: &wgpu::Device, label: Option<&str>) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label,
        size: INSTANCE_BUFFER_SIZE as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

/// Create a GPU instance buffer initialized with the given instances.
///
/// # Arguments
/// * `device` - The wgpu device to create the buffer on
/// * `instances` - Slice of creature instances to initialize the buffer with
/// * `label` - Optional label for debugging
///
/// # Returns
/// A wgpu::Buffer initialized with the provided instance data.
///
/// # Panics
/// Panics if instances.len() > MAX_CREATURE_INSTANCES
pub fn create_instance_buffer_init(
    device: &wgpu::Device,
    instances: &[CreatureInstance],
    label: Option<&str>,
) -> wgpu::Buffer {
    assert!(
        instances.len() <= MAX_CREATURE_INSTANCES,
        "Cannot create instance buffer with {} instances, max is {}",
        instances.len(),
        MAX_CREATURE_INSTANCES
    );

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label,
        contents: bytemuck::cast_slice(instances),
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    })
}

/// Update an existing instance buffer with new data.
///
/// # Arguments
/// * `queue` - The wgpu queue for submitting the write operation
/// * `buffer` - The instance buffer to update
/// * `instances` - Slice of creature instances to write
/// * `offset_instances` - Instance offset (in number of instances, not bytes)
///
/// # Panics
/// Panics if the write would exceed MAX_CREATURE_INSTANCES
pub fn update_instance_buffer(
    queue: &wgpu::Queue,
    buffer: &wgpu::Buffer,
    instances: &[CreatureInstance],
    offset_instances: usize,
) {
    assert!(
        offset_instances + instances.len() <= MAX_CREATURE_INSTANCES,
        "Instance buffer write would exceed max capacity"
    );

    let offset_bytes = (offset_instances * std::mem::size_of::<CreatureInstance>()) as u64;
    queue.write_buffer(buffer, offset_bytes, bytemuck::cast_slice(instances));
}

/// Describes the vertex buffer layout for CreatureInstance.
/// Use this when creating render pipelines that use instanced rendering.
pub fn instance_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<CreatureInstance>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            // position: vec3<f32> at offset 0
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            // _pad0 is skipped (offset 12)
            // rotation: vec4<f32> at offset 16
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 16,
                shader_location: 1,
            },
            // scale: f32 at offset 32
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32,
                offset: 32,
                shader_location: 2,
            },
            // baked_sdf_id: u32 at offset 36
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 36,
                shader_location: 3,
            },
            // animation_state: u32 at offset 40
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 40,
                shader_location: 4,
            },
            // tint_color: u32 at offset 44
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 44,
                shader_location: 5,
            },
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creature_instance_size() {
        assert_eq!(std::mem::size_of::<CreatureInstance>(), 48);
    }

    #[test]
    fn test_instance_buffer_size() {
        assert_eq!(INSTANCE_BUFFER_SIZE, 96_000); // 96 KB
    }

    #[test]
    fn test_max_instances() {
        assert_eq!(MAX_CREATURE_INSTANCES, 2000);
    }

    #[test]
    fn test_default_instance() {
        let instance = CreatureInstance::default();
        assert_eq!(instance.position, [0.0, 0.0, 0.0]);
        assert_eq!(instance.rotation, [0.0, 0.0, 0.0, 1.0]); // Identity quaternion
        assert_eq!(instance.scale, 1.0);
        assert_eq!(instance.baked_sdf_id, 0);
        assert_eq!(instance.animation_state, 0);
        assert_eq!(instance.tint_color, 0xFFFFFFFF);
    }

    #[test]
    fn test_pack_unpack_rgba() {
        let (r, g, b, a) = (255, 128, 64, 200);
        let packed = pack_rgba(r, g, b, a);
        let (ur, ug, ub, ua) = unpack_rgba(packed);
        assert_eq!((ur, ug, ub, ua), (r, g, b, a));
    }

    #[test]
    fn test_builder_pattern() {
        let instance = CreatureInstance::default()
            .with_position([1.0, 2.0, 3.0])
            .with_rotation([0.0, 0.707, 0.0, 0.707])
            .with_scale(2.0)
            .with_sdf_id(5)
            .with_animation_state(3)
            .with_tint_color(0xFF0000FF);

        assert_eq!(instance.position, [1.0, 2.0, 3.0]);
        assert_eq!(instance.rotation, [0.0, 0.707, 0.0, 0.707]);
        assert_eq!(instance.scale, 2.0);
        assert_eq!(instance.baked_sdf_id, 5);
        assert_eq!(instance.animation_state, 3);
        assert_eq!(instance.tint_color, 0xFF0000FF);
    }

    #[test]
    fn test_at_position() {
        let instance = CreatureInstance::at_position([5.0, 10.0, 15.0], 42);
        assert_eq!(instance.position, [5.0, 10.0, 15.0]);
        assert_eq!(instance.baked_sdf_id, 42);
        assert_eq!(instance.scale, 1.0); // Default
    }
}
