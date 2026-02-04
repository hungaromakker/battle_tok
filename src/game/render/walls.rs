//! Hex-Prism Walls
//!
//! Wall creation utilities for the battle arena.

use crate::render::HexPrismGrid;

/// Merged mesh buffers for efficient rendering
pub struct MergedMeshBuffers {
    pub vertex_count: u32,
    pub index_count: u32,
}

impl Default for MergedMeshBuffers {
    fn default() -> Self {
        Self {
            vertex_count: 0,
            index_count: 0,
        }
    }
}

/// Create test hex-prism walls for the battle arena
/// Creates a simple wall: 5 prisms in a row, 3 layers high on the defender hex
#[allow(dead_code)]  // Will be used when hex-prism rendering is integrated
pub fn create_test_walls() -> HexPrismGrid {
    let mut grid = HexPrismGrid::new();
    // Build a wall: 5 prisms wide, 3 layers tall, material 0 = stone gray
    grid.create_wall(0, 0, 5, 3, 0);
    // Add variety wall with material 2 = stone dark
    grid.create_wall(-2, 2, 3, 2, 2);
    println!("[Battle Arena] Created hex-prism walls: {} prisms", grid.len());
    grid
}
