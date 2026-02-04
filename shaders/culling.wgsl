// Tile-Based Culling Shader
// US-003: Create Tile Data Structure for Culling
//
// This shader defines data structures for tile-based culling, where the screen
// is divided into 16×16 pixel tiles for efficient entity visibility testing.
//
// At 1920×1080 resolution:
// - Horizontal tiles: ceil(1920 / 16) = 120 tiles
// - Vertical tiles: ceil(1080 / 16) = 68 tiles
// - Total tiles: 120 × 68 = 8,160 tiles
//
// Each tile stores up to 32 entity indices that potentially intersect that tile.
// This allows the ray marcher to only evaluate entities relevant to each pixel's tile.

// ============================================================================
// TILE CONSTANTS
// ============================================================================

// Tile dimensions in pixels
const TILE_SIZE: u32 = 16u;

// Maximum entities per tile (32 indices × 4 bytes = 128 bytes per tile)
const MAX_ENTITIES_PER_TILE: u32 = 32u;

// Grid dimensions at 1080p (1920×1080)
// These are calculated as: ceil(resolution / TILE_SIZE)
const TILES_X_1080P: u32 = 120u;  // ceil(1920 / 16) = 120
const TILES_Y_1080P: u32 = 68u;   // ceil(1080 / 16) = 68
const TOTAL_TILES_1080P: u32 = 8160u;  // 120 × 68 = 8,160

// ============================================================================
// TILE DATA STRUCTURES
// ============================================================================

// TileData: Per-tile culling information
// Stores the count and indices of entities that overlap this tile.
//
// Memory layout (136 bytes total per tile):
// - entity_count: atomic<u32> (4 bytes) - Number of valid entity indices (atomic for concurrent writes)
// - _padding: u32 (4 bytes) - Alignment padding
// - entity_indices: array<u32, 32> (128 bytes) - Entity indices that overlap this tile
//
// Note: Using array<u32, 32> instead of a runtime-sized array because WGSL
// storage buffers with runtime arrays have specific constraints. Fixed-size
// arrays allow for simpler memory layout and GPU access patterns.
// entity_count is atomic to support concurrent writes from compute shader threads.
struct TileData {
    // Number of entities that overlap this tile (0-32)
    // Atomic for thread-safe increment/decrement in compute shaders
    entity_count: atomic<u32>,
    // Padding for 8-byte alignment of the array
    _padding: u32,
    // Indices into the entity buffer for entities overlapping this tile
    // Only the first entity_count entries are valid
    entity_indices: array<u32, 32>,
}

// TileBuffer: Complete tile buffer for the entire screen
// Contains metadata and the array of all tile data.
//
// Memory layout at 1080p:
// - tiles_x: u32 (4 bytes) - Number of horizontal tiles
// - tiles_y: u32 (4 bytes) - Number of vertical tiles
// - tile_size: u32 (4 bytes) - Tile size in pixels (16)
// - total_tiles: u32 (4 bytes) - Total number of tiles
// - tiles: array<TileData> - Per-tile culling data
//
// Total buffer size at 1080p:
// - Header: 16 bytes
// - Tile data: 8,160 tiles × 136 bytes = ~1.06 MB
// - Total: ~1.06 MB (within the ~1 MB target)
struct TileBuffer {
    // Number of tiles in X direction
    tiles_x: u32,
    // Number of tiles in Y direction
    tiles_y: u32,
    // Size of each tile in pixels (16)
    tile_size: u32,
    // Total number of tiles (tiles_x × tiles_y)
    total_tiles: u32,
    // Array of tile data (runtime-sized for flexibility)
    tiles: array<TileData>,
}

// ============================================================================
// TILE BUFFER BINDING
// ============================================================================

// The tile buffer is bound as a storage buffer for read/write access.
// The culling compute shader writes to it, and the ray marcher reads from it.
//
// Usage:
// - Culling pass (compute): Writes entity indices to tiles
// - Ray march pass (fragment): Reads tile data to limit entity iteration
//
// Binding group 1 is used to separate culling data from main render bindings.
// This allows the culling system to be enabled/disabled without affecting
// the main render pipeline bindings.
@group(1) @binding(0)
var<storage, read_write> tile_buffer: TileBuffer;

// ============================================================================
// TILE UTILITY FUNCTIONS
// ============================================================================

// Get the tile index for a given pixel coordinate
// Returns a 1D index into the tiles array
fn get_tile_index(pixel_x: u32, pixel_y: u32) -> u32 {
    let tile_x = pixel_x / TILE_SIZE;
    let tile_y = pixel_y / TILE_SIZE;
    return tile_y * tile_buffer.tiles_x + tile_x;
}

// Get the tile coordinates for a given pixel coordinate
// Returns (tile_x, tile_y)
fn get_tile_coords(pixel_x: u32, pixel_y: u32) -> vec2<u32> {
    return vec2<u32>(pixel_x / TILE_SIZE, pixel_y / TILE_SIZE);
}

// Check if a tile index is valid
fn is_valid_tile(tile_index: u32) -> bool {
    return tile_index < tile_buffer.total_tiles;
}

// Get the number of entities in a tile
fn get_tile_entity_count(tile_index: u32) -> u32 {
    if (!is_valid_tile(tile_index)) {
        return 0u;
    }
    return min(atomicLoad(&tile_buffer.tiles[tile_index].entity_count), MAX_ENTITIES_PER_TILE);
}

// Get an entity index from a tile
// Returns the entity index, or 0xFFFFFFFF if the index is invalid
fn get_tile_entity(tile_index: u32, entity_slot: u32) -> u32 {
    if (!is_valid_tile(tile_index) || entity_slot >= MAX_ENTITIES_PER_TILE) {
        return 0xFFFFFFFFu;  // Invalid sentinel value
    }
    let count = atomicLoad(&tile_buffer.tiles[tile_index].entity_count);
    if (entity_slot >= count) {
        return 0xFFFFFFFFu;
    }
    return tile_buffer.tiles[tile_index].entity_indices[entity_slot];
}

// ============================================================================
// TILE CLEAR FUNCTION (for compute shader use)
// ============================================================================

// Clear a tile's entity count (called at the start of culling)
fn clear_tile(tile_index: u32) {
    if (is_valid_tile(tile_index)) {
        atomicStore(&tile_buffer.tiles[tile_index].entity_count, 0u);
    }
}

// ============================================================================
// TILE ADD ENTITY FUNCTION (for compute shader use)
// ============================================================================

// Add an entity to a tile (atomic to handle concurrent writes)
// Returns true if the entity was added, false if the tile is full
fn add_entity_to_tile(tile_index: u32, entity_index: u32) -> bool {
    if (!is_valid_tile(tile_index)) {
        return false;
    }

    // Atomically increment the entity count and get the previous value
    // This is the slot where we'll store the entity index
    let slot = atomicAdd(&tile_buffer.tiles[tile_index].entity_count, 1u);

    // Check if we have space
    if (slot >= MAX_ENTITIES_PER_TILE) {
        // No space - decrement the count back (atomic to be safe)
        atomicSub(&tile_buffer.tiles[tile_index].entity_count, 1u);
        return false;
    }

    // Store the entity index
    tile_buffer.tiles[tile_index].entity_indices[slot] = entity_index;
    return true;
}

// ============================================================================
// SCREEN TO TILE MAPPING
// ============================================================================

// Calculate tile grid dimensions for a given resolution
fn calculate_tile_dimensions(resolution: vec2<u32>) -> vec2<u32> {
    // Ceiling division: (resolution + TILE_SIZE - 1) / TILE_SIZE
    let tiles_x = (resolution.x + TILE_SIZE - 1u) / TILE_SIZE;
    let tiles_y = (resolution.y + TILE_SIZE - 1u) / TILE_SIZE;
    return vec2<u32>(tiles_x, tiles_y);
}

// Calculate total tiles for a given resolution
fn calculate_total_tiles(resolution: vec2<u32>) -> u32 {
    let dims = calculate_tile_dimensions(resolution);
    return dims.x * dims.y;
}
