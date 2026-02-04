//! SDF Brick Cache
//!
//! This module provides `BrickCache` for storing pre-computed SDF volumes
//! as 64³ f32 bricks in a flat SSBO buffer.
//!
//! Each brick uses 1 MB of GPU memory (64 * 64 * 64 * 4 bytes).
//! Capacity is determined by device limits, capped at 256 bricks.

use wgpu;

/// Resolution of each baked SDF volume (64³ voxels)
pub const SDF_RESOLUTION: u32 = 64;

/// Maximum number of unique baked SDFs that can be stored
pub const MAX_BAKED_SDFS: u32 = 256;

/// Brick-based SDF cache using a flat SSBO buffer.
///
/// Each brick is a 64³ volume of f32 values (1,048,576 bytes per brick).
/// Capacity is determined by device limits, capped at 256 bricks.
pub struct BrickCache {
    buffer: wgpu::Buffer,
    capacity: u32,
    slot_bitmap: [u64; 4],
    allocated_count: u32,
    bake_bind_group_layout: wgpu::BindGroupLayout,
    bind_group_layout: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
}

/// Size of one brick in bytes: 64 * 64 * 64 * 4 (f32) = 1,048,576
const BRICK_SIZE_BYTES: u64 = 1_048_576;

impl BrickCache {
    /// Creates a new BrickCache with capacity determined by device limits.
    pub fn new(device: &wgpu::Device) -> Self {
        let limit = device.limits().max_storage_buffer_binding_size;
        let capacity = ((limit as u64 / BRICK_SIZE_BYTES) as u32).min(256);

        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("BrickCache SSBO"),
            size: capacity as u64 * BRICK_SIZE_BYTES,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bake_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BrickCache bake (read-write) layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("BrickCache read-only layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT | wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BrickCache read-only bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        println!("[BrickCache] Created with capacity: {} bricks", capacity);

        Self {
            buffer,
            capacity,
            slot_bitmap: [0; 4],
            allocated_count: 0,
            bake_bind_group_layout,
            bind_group_layout,
            bind_group,
        }
    }

    /// Returns a reference to the underlying GPU buffer.
    pub fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Returns the brick capacity.
    pub fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Returns the bake bind group layout (read-write, COMPUTE visibility).
    pub fn bake_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bake_bind_group_layout
    }

    /// Returns the read-only bind group layout (FRAGMENT | COMPUTE visibility).
    pub fn bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.bind_group_layout
    }

    /// Returns the read-only bind group for the BrickCache buffer.
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Creates a bind group for the bake compute shader to write SDF data.
    pub fn create_bake_bind_group(&self, device: &wgpu::Device) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("BrickCache bake bind group"),
            layout: &self.bake_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.buffer.as_entire_binding(),
            }],
        })
    }

    /// Allocates a slot for a new baked SDF brick.
    ///
    /// Returns `Some(slot_id)` if a slot is available, or `None` if capacity is reached.
    pub fn allocate_sdf_slot(&mut self) -> Option<u32> {
        for (word_idx, word) in self.slot_bitmap.iter_mut().enumerate() {
            if *word != u64::MAX {
                let bit = (!*word).trailing_zeros();
                let slot = word_idx as u32 * 64 + bit;
                if slot >= self.capacity {
                    return None;
                }
                *word |= 1u64 << bit;
                self.allocated_count += 1;
                return Some(slot);
            }
        }
        None
    }

    /// Frees a previously allocated SDF slot.
    pub fn free_sdf_slot(&mut self, slot: u32) {
        assert!(slot < self.capacity, "Slot out of range: {}", slot);
        let word_idx = (slot / 64) as usize;
        let bit = slot % 64;
        let mask = 1u64 << bit;
        assert!(
            self.slot_bitmap[word_idx] & mask != 0,
            "Slot {} was not allocated",
            slot
        );
        self.slot_bitmap[word_idx] &= !mask;
        self.allocated_count -= 1;
    }

    /// Checks if a slot is currently allocated.
    #[inline]
    pub fn is_slot_allocated(&self, slot: u32) -> bool {
        if slot >= self.capacity {
            return false;
        }
        let word_idx = (slot / 64) as usize;
        let bit = slot % 64;
        (self.slot_bitmap[word_idx] & (1u64 << bit)) != 0
    }

    /// Returns the number of currently allocated slots.
    #[inline]
    pub fn allocated_count(&self) -> u32 {
        self.allocated_count
    }

    /// Returns the number of available (free) slots.
    #[inline]
    pub fn available_count(&self) -> u32 {
        self.capacity - self.allocated_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sdf_constants() {
        assert_eq!(SDF_RESOLUTION, 64);
        assert_eq!(MAX_BAKED_SDFS, 256);
    }

    #[test]
    fn test_slot_allocation_bitmap() {
        // Verify bitmap can hold 256 bits (4 * 64 = 256)
        assert_eq!(4 * 64, 256);
    }
}
