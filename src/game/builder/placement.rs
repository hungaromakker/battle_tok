//! Block Placement
//!
//! Block placement logic and support checking for building blocks.

use crate::game::physics::AABB;
use glam::Vec3;

/// Check if a block has support from below (either ground or another block)
///
/// # Arguments
/// * `block_position` - Block center position
/// * `block_aabb` - Block AABB
/// * `ground_height` - Ground height at block position
/// * `ground_threshold` - Tolerance for ground check
/// * `other_blocks` - Iterator of (id, AABB) for other blocks
///
/// # Returns
/// true if block has support
pub fn check_block_support<'a>(
    _block_position: Vec3,
    block_aabb: &AABB,
    ground_height: f32,
    ground_threshold: f32,
    other_blocks: impl Iterator<Item = (u32, &'a AABB)>,
    block_id: u32,
) -> bool {
    let bottom_y = block_aabb.min.y;

    // Check if block is on ground
    if bottom_y <= ground_height + ground_threshold {
        return true;
    }

    // Check if block is supported by another block
    for (other_id, other_aabb) in other_blocks {
        if other_id == block_id {
            continue;
        }

        // Check if other block is below and overlapping
        if other_aabb.max.y >= block_aabb.min.y - 0.2 && other_aabb.max.y < block_aabb.min.y + 0.1 {
            // Check XZ overlap
            if block_aabb.min.x < other_aabb.max.x
                && block_aabb.max.x > other_aabb.min.x
                && block_aabb.min.z < other_aabb.max.z
                && block_aabb.max.z > other_aabb.min.z
            {
                return true;
            }
        }
    }

    false
}

/// Calculate bridge segment positions between two faces
///
/// # Arguments
/// * `start_position` - Start face center position
/// * `end_position` - End face center position
/// * `start_size` - Start face size (width, height)
/// * `end_size` - End face size (width, height)
/// * `grid_size` - Grid size for segment spacing
///
/// # Returns
/// Vec of (position, half_extents) for each bridge segment
pub fn calculate_bridge_segments(
    start_position: Vec3,
    end_position: Vec3,
    start_size: (f32, f32),
    end_size: (f32, f32),
    grid_size: f32,
) -> Vec<(Vec3, Vec3)> {
    let direction = end_position - start_position;
    let length = direction.length();

    if length < 0.1 {
        return Vec::new();
    }

    let num_segments = (length / grid_size).ceil() as i32;
    let segment_length = length / num_segments as f32;

    let mut segments = Vec::with_capacity((num_segments + 1) as usize);

    for i in 0..=num_segments {
        let t = i as f32 / num_segments as f32;
        let pos = start_position + direction * t;

        // Interpolate size from start to end face
        let w = start_size.0 * (1.0 - t) + end_size.0 * t;
        let h = start_size.1 * (1.0 - t) + end_size.1 * t;

        let half_extents = Vec3::new(segment_length * 0.6, h * 0.5, w * 0.5);

        segments.push((pos, half_extents));
    }

    segments
}

/// Result of a block placement calculation
#[derive(Debug, Clone)]
pub struct PlacementResult {
    /// The calculated placement position
    pub position: Vec3,
    /// Whether this position snapped to an existing block
    pub snapped: bool,
    /// Block ID if snapped to a block (for stacking)
    pub adjacent_to: Option<u32>,
}

impl PlacementResult {
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            snapped: false,
            adjacent_to: None,
        }
    }

    pub fn with_snap(position: Vec3, adjacent_to: u32) -> Self {
        Self {
            position,
            snapped: true,
            adjacent_to: Some(adjacent_to),
        }
    }
}
