// Froxel Clear Compute Shader
// US-035: Clear froxel lists each frame
//
// This compute shader clears all froxel SDF counts to 0 before the assignment
// shader runs. This must be executed before the assignment shader to ensure
// stale data from the previous frame is cleared.
//
// Workgroup configuration:
// - Workgroup size: 64 threads
// - Each thread clears 96 froxels (6144 / 64 = 96)
// - One dispatch (1, 1, 1) is sufficient to clear all froxels

// ============================================================================
// FROXEL CONSTANTS (must match froxel_config.rs and froxel_buffers.rs)
// ============================================================================

// Froxel grid dimensions
const FROXEL_TILES_X: u32 = 16u;
const FROXEL_TILES_Y: u32 = 16u;
const FROXEL_DEPTH_SLICES: u32 = 24u;

// Total number of froxels in the grid
const TOTAL_FROXELS: u32 = 6144u; // 16 * 16 * 24

// Maximum SDFs per froxel
const MAX_SDFS_PER_FROXEL: u32 = 64u;

// Number of froxels each thread clears
const FROXELS_PER_THREAD: u32 = 96u; // 6144 / 64 = 96

// ============================================================================
// FROXEL SDF LIST STRUCTURE (must match froxel_buffers.rs)
// ============================================================================

// Per-froxel list of SDF indices
// WGSL Layout (272 bytes):
//   offset  0-3:   count (u32) - Number of valid SDF indices
//   offset  4-7:   _pad0 (u32) - Padding for 16-byte alignment
//   offset  8-11:  _pad1 (u32) - Padding
//   offset 12-15:  _pad2 (u32) - Padding
//   offset 16-271: sdf_indices (array<u32, 64>) - SDF indices (256 bytes)
struct FroxelSDFList {
    count: atomic<u32>,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    sdf_indices: array<u32, 64>,
}

// Buffer containing SDF lists for all froxels
// Layout:
//   offset  0-3:   count (u32) - Always equals TOTAL_FROXELS
//   offset  4-15:  padding (3 x u32 for 16-byte alignment)
//   offset 16+:    lists (array<FroxelSDFList>) - 6144 froxel lists
struct FroxelSDFListBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    lists: array<FroxelSDFList>,
}

// ============================================================================
// BINDINGS
// ============================================================================

// The froxel SDF list buffer to clear
// This is binding 2 in the assignment bind group (matching froxel_assignment.rs)
@group(0) @binding(0)
var<storage, read_write> froxel_sdf_lists: FroxelSDFListBuffer;

// ============================================================================
// CLEAR SHADER ENTRY POINT
// ============================================================================

// Workgroup size 64: one thread per 96 froxels
// Dispatch with (1, 1, 1) to cover all 6144 froxels
@compute @workgroup_size(64, 1, 1)
fn cs_clear_froxels(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Each thread clears FROXELS_PER_THREAD froxels
    let start_froxel = thread_id * FROXELS_PER_THREAD;

    // Clear all froxels assigned to this thread
    for (var i: u32 = 0u; i < FROXELS_PER_THREAD; i = i + 1u) {
        let froxel_index = start_froxel + i;

        // Bounds check (should always pass with correct dispatch)
        if (froxel_index < TOTAL_FROXELS) {
            // Reset the SDF count to 0
            // We use atomicStore for consistency with how the assignment shader
            // uses atomicAdd to increment counts
            atomicStore(&froxel_sdf_lists.lists[froxel_index].count, 0u);
        }
    }
}
