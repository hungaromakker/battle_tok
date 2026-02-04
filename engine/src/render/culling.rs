//! Tile-Based Culling Data Structures
//!
//! US-003: Create Tile Data Structure for Culling
//!
//! This module contains GPU-compatible data structures for tile-based culling.
//! The screen is divided into 16×16 pixel tiles, and each tile stores up to 32
//! entity indices that potentially overlap that tile.
//!
//! ## Memory Layout
//!
//! At 1920×1080 (1080p):
//! - Horizontal tiles: ceil(1920 / 16) = 120 tiles
//! - Vertical tiles: ceil(1080 / 16) = 68 tiles
//! - Total tiles: 120 × 68 = 8,160 tiles
//!
//! Each tile (TileData) is 136 bytes:
//! - entity_count: 4 bytes
//! - _padding: 4 bytes
//! - entity_indices: 32 × 4 = 128 bytes
//!
//! Total buffer size at 1080p:
//! - Header (TileBufferHeader): 16 bytes
//! - Tile data: 8,160 × 136 = 1,109,760 bytes (~1.06 MB)
//! - Total: ~1.06 MB (within ~1 MB target)

/// Tile size in pixels (16×16 pixel tiles)
pub const TILE_SIZE: u32 = 16;

/// Maximum entities per tile
pub const MAX_ENTITIES_PER_TILE: usize = 32;

/// Grid dimensions at 1080p resolution (1920×1080)
pub const TILES_X_1080P: u32 = 120;  // ceil(1920 / 16)
pub const TILES_Y_1080P: u32 = 68;   // ceil(1080 / 16)
pub const TOTAL_TILES_1080P: u32 = TILES_X_1080P * TILES_Y_1080P;  // 8,160

/// Per-tile culling data - stores entity indices overlapping this tile.
///
/// WGSL Layout (136 bytes total):
/// - entity_count: u32 (4 bytes) - Number of valid entity indices
/// - _padding: u32 (4 bytes) - Alignment padding
/// - entity_indices: array<u32, 32> (128 bytes) - Entity indices
///
/// This struct is GPU-compatible and matches the WGSL TileData struct
/// defined in shaders/culling.wgsl.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileData {
    /// Number of entities that overlap this tile (0-32)
    pub entity_count: u32,
    /// Padding for 8-byte alignment
    pub _padding: u32,
    /// Entity indices that overlap this tile (only first entity_count are valid)
    pub entity_indices: [u32; MAX_ENTITIES_PER_TILE],
}

impl Default for TileData {
    fn default() -> Self {
        Self {
            entity_count: 0,
            _padding: 0,
            entity_indices: [0; MAX_ENTITIES_PER_TILE],
        }
    }
}

impl TileData {
    /// Create an empty tile with no entities
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the tile (reset entity count to 0)
    pub fn clear(&mut self) {
        self.entity_count = 0;
    }

    /// Add an entity to this tile
    /// Returns true if added successfully, false if tile is full
    pub fn add_entity(&mut self, entity_index: u32) -> bool {
        if (self.entity_count as usize) >= MAX_ENTITIES_PER_TILE {
            return false;
        }
        self.entity_indices[self.entity_count as usize] = entity_index;
        self.entity_count += 1;
        true
    }

    /// Check if this tile contains any entities
    pub fn is_empty(&self) -> bool {
        self.entity_count == 0
    }

    /// Get the number of entities in this tile
    pub fn len(&self) -> usize {
        self.entity_count as usize
    }

    /// Iterate over the valid entity indices in this tile
    pub fn entity_iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.entity_indices[..self.entity_count as usize].iter().copied()
    }
}

/// Tile buffer header - metadata for the tile grid.
///
/// WGSL Layout (16 bytes):
/// - tiles_x: u32 (4 bytes) - Number of horizontal tiles
/// - tiles_y: u32 (4 bytes) - Number of vertical tiles
/// - tile_size: u32 (4 bytes) - Tile size in pixels
/// - total_tiles: u32 (4 bytes) - Total tile count
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TileBufferHeader {
    /// Number of tiles in X direction
    pub tiles_x: u32,
    /// Number of tiles in Y direction
    pub tiles_y: u32,
    /// Size of each tile in pixels (always 16)
    pub tile_size: u32,
    /// Total number of tiles (tiles_x × tiles_y)
    pub total_tiles: u32,
}

impl TileBufferHeader {
    /// Create a header for the given resolution
    pub fn for_resolution(width: u32, height: u32) -> Self {
        let tiles_x = (width + TILE_SIZE - 1) / TILE_SIZE;
        let tiles_y = (height + TILE_SIZE - 1) / TILE_SIZE;
        Self {
            tiles_x,
            tiles_y,
            tile_size: TILE_SIZE,
            total_tiles: tiles_x * tiles_y,
        }
    }

    /// Create a header for 1080p resolution (1920×1080)
    pub fn for_1080p() -> Self {
        Self::for_resolution(1920, 1080)
    }
}

impl Default for TileBufferHeader {
    fn default() -> Self {
        Self::for_1080p()
    }
}

/// Complete tile buffer with header and tile data.
///
/// This struct is used for CPU-side management of the tile buffer.
/// The GPU buffer is created separately with the appropriate size.
///
/// Total size at 1080p:
/// - Header: 16 bytes
/// - Tiles: 8,160 × 136 = 1,109,760 bytes
/// - Total: 1,109,776 bytes (~1.06 MB)
pub struct TileBuffer {
    /// Header containing grid dimensions
    pub header: TileBufferHeader,
    /// Tile data array
    pub tiles: Vec<TileData>,
}

impl TileBuffer {
    /// Create a new tile buffer for the given resolution
    pub fn for_resolution(width: u32, height: u32) -> Self {
        let header = TileBufferHeader::for_resolution(width, height);
        let tiles = vec![TileData::default(); header.total_tiles as usize];
        Self { header, tiles }
    }

    /// Create a new tile buffer for 1080p resolution
    pub fn for_1080p() -> Self {
        Self::for_resolution(1920, 1080)
    }

    /// Clear all tiles (reset entity counts to 0)
    pub fn clear(&mut self) {
        for tile in &mut self.tiles {
            tile.clear();
        }
    }

    /// Get the tile index for a pixel coordinate
    pub fn get_tile_index(&self, pixel_x: u32, pixel_y: u32) -> Option<usize> {
        let tile_x = pixel_x / TILE_SIZE;
        let tile_y = pixel_y / TILE_SIZE;
        if tile_x >= self.header.tiles_x || tile_y >= self.header.tiles_y {
            return None;
        }
        Some((tile_y * self.header.tiles_x + tile_x) as usize)
    }

    /// Get a reference to a tile at the given pixel coordinate
    pub fn get_tile(&self, pixel_x: u32, pixel_y: u32) -> Option<&TileData> {
        self.get_tile_index(pixel_x, pixel_y)
            .map(|idx| &self.tiles[idx])
    }

    /// Get a mutable reference to a tile at the given pixel coordinate
    pub fn get_tile_mut(&mut self, pixel_x: u32, pixel_y: u32) -> Option<&mut TileData> {
        self.get_tile_index(pixel_x, pixel_y)
            .map(|idx| &mut self.tiles[idx])
    }

    /// Add an entity to all tiles it overlaps
    /// Returns the number of tiles the entity was added to
    pub fn add_entity_to_tiles(
        &mut self,
        entity_index: u32,
        screen_min_x: u32,
        screen_min_y: u32,
        screen_max_x: u32,
        screen_max_y: u32,
    ) -> u32 {
        let tile_min_x = screen_min_x / TILE_SIZE;
        let tile_min_y = screen_min_y / TILE_SIZE;
        let tile_max_x = (screen_max_x + TILE_SIZE - 1) / TILE_SIZE;
        let tile_max_y = (screen_max_y + TILE_SIZE - 1) / TILE_SIZE;

        let mut count = 0;
        for tile_y in tile_min_y..tile_max_y.min(self.header.tiles_y) {
            for tile_x in tile_min_x..tile_max_x.min(self.header.tiles_x) {
                let idx = (tile_y * self.header.tiles_x + tile_x) as usize;
                if self.tiles[idx].add_entity(entity_index) {
                    count += 1;
                }
            }
        }
        count
    }

    /// Calculate the total GPU buffer size needed
    pub fn gpu_buffer_size(&self) -> usize {
        std::mem::size_of::<TileBufferHeader>()
            + self.tiles.len() * std::mem::size_of::<TileData>()
    }

    /// Get the header as bytes for GPU upload
    pub fn header_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(&self.header)
    }

    /// Get the tiles as bytes for GPU upload
    pub fn tiles_bytes(&self) -> &[u8] {
        bytemuck::cast_slice(&self.tiles)
    }
}

impl Default for TileBuffer {
    fn default() -> Self {
        Self::for_1080p()
    }
}

// Compile-time assertions to verify struct sizes match WGSL layout
const _: () = {
    // TileData: 4 (count) + 4 (padding) + 128 (indices) = 136 bytes
    assert!(
        std::mem::size_of::<TileData>() == 136,
        "TileData must be 136 bytes to match WGSL"
    );
    // TileBufferHeader: 4 × 4 = 16 bytes
    assert!(
        std::mem::size_of::<TileBufferHeader>() == 16,
        "TileBufferHeader must be 16 bytes to match WGSL"
    );
    // Verify MAX_ENTITIES_PER_TILE × 4 = 128 bytes
    assert!(
        MAX_ENTITIES_PER_TILE * 4 == 128,
        "Entity indices array must be 128 bytes"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tile_data_size() {
        assert_eq!(std::mem::size_of::<TileData>(), 136);
    }

    #[test]
    fn test_tile_buffer_header_size() {
        assert_eq!(std::mem::size_of::<TileBufferHeader>(), 16);
    }

    #[test]
    fn test_header_for_1080p() {
        let header = TileBufferHeader::for_1080p();
        assert_eq!(header.tiles_x, 120);
        assert_eq!(header.tiles_y, 68);
        assert_eq!(header.tile_size, 16);
        assert_eq!(header.total_tiles, 8160);
    }

    #[test]
    fn test_tile_buffer_size_1080p() {
        let buffer = TileBuffer::for_1080p();
        // Header: 16 bytes
        // Tiles: 8160 × 136 = 1,109,760 bytes
        // Total: 1,109,776 bytes (~1.06 MB)
        let expected_size = 16 + 8160 * 136;
        assert_eq!(buffer.gpu_buffer_size(), expected_size);
        // Verify it's approximately 1 MB
        assert!(buffer.gpu_buffer_size() >= 1_000_000);
        assert!(buffer.gpu_buffer_size() <= 1_200_000);
    }

    #[test]
    fn test_tile_add_entity() {
        let mut tile = TileData::new();
        assert!(tile.is_empty());

        // Add entities up to capacity
        for i in 0..MAX_ENTITIES_PER_TILE {
            assert!(tile.add_entity(i as u32));
        }
        assert_eq!(tile.len(), MAX_ENTITIES_PER_TILE);

        // Should fail to add more
        assert!(!tile.add_entity(100));
    }

    #[test]
    fn test_tile_index_calculation() {
        let buffer = TileBuffer::for_1080p();

        // Top-left corner
        assert_eq!(buffer.get_tile_index(0, 0), Some(0));

        // Second tile in first row
        assert_eq!(buffer.get_tile_index(16, 0), Some(1));

        // First tile in second row
        assert_eq!(buffer.get_tile_index(0, 16), Some(120));

        // Last valid pixel in screen (1919, 1079)
        // tile_x = 1919 / 16 = 119, tile_y = 1079 / 16 = 67
        // index = 67 * 120 + 119 = 8040 + 119 = 8159
        assert_eq!(buffer.get_tile_index(1919, 1079), Some(8159));

        // Out of bounds - tiles beyond the grid
        // tiles_x = 120, so tile_x = 120 would be out of bounds
        // 120 * 16 = 1920, so pixel 1920 maps to tile_x = 120 (out of bounds)
        assert_eq!(buffer.get_tile_index(1920, 0), None);
        // tiles_y = 68, so tile_y = 68 would be out of bounds
        // 68 * 16 = 1088, so pixel 1088 maps to tile_y = 68 (out of bounds)
        assert_eq!(buffer.get_tile_index(0, 1088), None);
    }

    #[test]
    fn test_entity_indices_128_bytes() {
        // 32 entities × 4 bytes per u32 = 128 bytes
        assert_eq!(MAX_ENTITIES_PER_TILE * std::mem::size_of::<u32>(), 128);
    }
}
