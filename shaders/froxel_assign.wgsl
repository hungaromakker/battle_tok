// Froxel SDF Assignment Compute Shader (US-034)
//
// This compute shader assigns SDFs to froxels by testing each SDF's bounding
// sphere against all froxel bounds. Each thread processes one SDF and adds
// its index to all froxels it potentially intersects.
//
// Algorithm:
// 1. Each thread handles one SDF (parallelized across 256 threads per workgroup)
// 2. For each SDF, iterate through all froxels
// 3. Test SDF bounding sphere against froxel AABB
// 4. Atomically add SDF index to intersecting froxels' lists
//
// Performance target: <0.5ms for 1024 SDFs

// ============================================================================
// FROXEL CONSTANTS (must match engine/src/render/froxel_config.rs)
// ============================================================================

const FROXEL_TILES_X: u32 = 16u;
const FROXEL_TILES_Y: u32 = 16u;
const FROXEL_DEPTH_SLICES: u32 = 24u;
const TOTAL_FROXELS: u32 = 6144u;  // 16 * 16 * 24
const MAX_SDFS_PER_FROXEL: u32 = 64u;
const MAX_SDF_COUNT: u32 = 1024u;

// Cap SDF additions at 63 to handle overflow gracefully
// This ensures we never exceed the array bounds even under high contention
const SDF_ADD_CAP: u32 = 63u;

// ============================================================================
// SDF BOUNDS STRUCTURES (must match engine/src/render/froxel_assignment.rs)
// ============================================================================

// SdfBounds: World-space AABB for a single SDF/creature (32 bytes)
// Memory layout:
//   offset  0: min_x, min_y, min_z, _pad0 (16 bytes)
//   offset 16: max_x, max_y, max_z, _pad1 (16 bytes)
struct SdfBounds {
    min_x: f32,
    min_y: f32,
    min_z: f32,
    _pad0: u32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
    _pad1: u32,
}

// SdfBoundsBuffer: Contains bounds for all SDFs
// Memory layout:
//   offset  0: count, _pad0, _pad1, _pad2 (16 bytes)
//   offset 16: bounds array (32 bytes * 1024 = 32,768 bytes)
struct SdfBoundsBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    bounds: array<SdfBounds, 1024>,
}

// ============================================================================
// FROXEL BOUNDS STRUCTURES (must match engine/src/render/froxel_buffers.rs)
// ============================================================================

// FroxelBounds: World-space AABB for a single froxel (32 bytes)
// Memory layout:
//   offset  0: min_x, min_y, min_z, _pad0 (16 bytes)
//   offset 16: max_x, max_y, max_z, _pad1 (16 bytes)
struct FroxelBounds {
    min_x: f32,
    min_y: f32,
    min_z: f32,
    _pad0: u32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
    _pad1: u32,
}

// FroxelBoundsBuffer: Contains bounds for all froxels
// Memory layout:
//   offset  0: count, _pad0, _pad1, _pad2 (16 bytes)
//   offset 16: bounds array (32 bytes * 6144 = 196,608 bytes)
struct FroxelBoundsBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    bounds: array<FroxelBounds, 6144>,
}

// ============================================================================
// FROXEL SDF LIST STRUCTURES (must match engine/src/render/froxel_buffers.rs)
// ============================================================================

// FroxelSDFList: Per-froxel list of SDF indices (272 bytes)
// Memory layout:
//   offset  0: count, _pad0, _pad1, _pad2 (16 bytes)
//   offset 16: sdf_indices array (4 bytes * 64 = 256 bytes)
struct FroxelSDFList {
    count: atomic<u32>,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    sdf_indices: array<u32, 64>,
}

// FroxelSDFListBuffer: Contains SDF lists for all froxels
// Memory layout:
//   offset  0: count, _pad0, _pad1, _pad2 (16 bytes)
//   offset 16: lists array (272 bytes * 6144 = 1,671,168 bytes)
struct FroxelSDFListBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    lists: array<FroxelSDFList, 6144>,
}

// ============================================================================
// ASSIGNMENT UNIFORMS (must match engine/src/render/froxel_assignment.rs)
// ============================================================================

// AssignmentUniforms: Parameters for the assignment compute shader (16 bytes)
struct AssignmentUniforms {
    creature_count: u32,   // Number of SDFs to process
    froxel_count: u32,     // Number of froxels (always 6144)
    _pad0: u32,
    _pad1: u32,
}

// ============================================================================
// BINDINGS (must match create_assignment_bind_group_layout)
// ============================================================================

// Binding 0: SDF bounds buffer (read-only)
@group(0) @binding(0)
var<storage, read> sdf_bounds: SdfBoundsBuffer;

// Binding 1: Froxel bounds buffer (read-only)
@group(0) @binding(1)
var<storage, read> froxel_bounds: FroxelBoundsBuffer;

// Binding 2: Froxel SDF lists buffer (read-write)
@group(0) @binding(2)
var<storage, read_write> froxel_sdf_lists: FroxelSDFListBuffer;

// Binding 3: Assignment uniforms (read-only)
@group(0) @binding(3)
var<uniform> uniforms: AssignmentUniforms;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

// Get SDF bounds as min/max vec3 pair
fn get_sdf_min(bounds: SdfBounds) -> vec3<f32> {
    return vec3<f32>(bounds.min_x, bounds.min_y, bounds.min_z);
}

fn get_sdf_max(bounds: SdfBounds) -> vec3<f32> {
    return vec3<f32>(bounds.max_x, bounds.max_y, bounds.max_z);
}

// Get froxel bounds as min/max vec3 pair
fn get_froxel_min(bounds: FroxelBounds) -> vec3<f32> {
    return vec3<f32>(bounds.min_x, bounds.min_y, bounds.min_z);
}

fn get_froxel_max(bounds: FroxelBounds) -> vec3<f32> {
    return vec3<f32>(bounds.max_x, bounds.max_y, bounds.max_z);
}

// Calculate bounding sphere from AABB
// Returns (center, radius)
fn aabb_to_bounding_sphere(aabb_min: vec3<f32>, aabb_max: vec3<f32>) -> vec4<f32> {
    let center = (aabb_min + aabb_max) * 0.5;
    let half_extents = (aabb_max - aabb_min) * 0.5;
    // Sphere radius is the length of the half-diagonal
    let radius = length(half_extents);
    return vec4<f32>(center, radius);
}

// Test if a bounding sphere intersects an AABB
// Uses the closest point on AABB to sphere center technique
fn sphere_intersects_aabb(
    sphere_center: vec3<f32>,
    sphere_radius: f32,
    aabb_min: vec3<f32>,
    aabb_max: vec3<f32>
) -> bool {
    // Find the closest point on the AABB to the sphere center
    let closest = clamp(sphere_center, aabb_min, aabb_max);

    // Check if the closest point is within the sphere radius
    let distance_sq = dot(closest - sphere_center, closest - sphere_center);
    return distance_sq <= (sphere_radius * sphere_radius);
}

// Test if two AABBs intersect
fn aabb_intersects_aabb(
    a_min: vec3<f32>,
    a_max: vec3<f32>,
    b_min: vec3<f32>,
    b_max: vec3<f32>
) -> bool {
    return a_min.x <= b_max.x && a_max.x >= b_min.x &&
           a_min.y <= b_max.y && a_max.y >= b_min.y &&
           a_min.z <= b_max.z && a_max.z >= b_min.z;
}

// Add an SDF index to a froxel's list
// Returns true if the SDF was added, false if the froxel is full
fn add_sdf_to_froxel(froxel_index: u32, sdf_index: u32) -> bool {
    // Bounds check
    if (froxel_index >= TOTAL_FROXELS) {
        return false;
    }

    // Atomically increment the SDF count
    let slot = atomicAdd(&froxel_sdf_lists.lists[froxel_index].count, 1u);

    // Check if we have space (cap at 63 to handle overflow)
    if (slot >= SDF_ADD_CAP) {
        // No space - decrement the count
        atomicSub(&froxel_sdf_lists.lists[froxel_index].count, 1u);
        return false;
    }

    // Store the SDF index
    froxel_sdf_lists.lists[froxel_index].sdf_indices[slot] = sdf_index;
    return true;
}

// Clear a froxel's SDF count (called in the clear pass)
fn clear_froxel(froxel_index: u32) {
    if (froxel_index < TOTAL_FROXELS) {
        atomicStore(&froxel_sdf_lists.lists[froxel_index].count, 0u);
    }
}

// ============================================================================
// COMPUTE SHADER ENTRY POINTS
// ============================================================================

// Clear pass: Reset all froxel SDF counts to zero
// Workgroup size: 256 threads (16x16)
// Dispatch: ceil(TOTAL_FROXELS / 256) = ceil(6144 / 256) = 24 workgroups
@compute @workgroup_size(256, 1, 1)
fn cs_clear_froxels(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let froxel_index = global_id.x;

    if (froxel_index >= TOTAL_FROXELS) {
        return;
    }

    clear_froxel(froxel_index);
}

// Assignment pass: Each thread processes one SDF and assigns it to all intersecting froxels
// Workgroup size: 256 threads (one thread per SDF)
// Dispatch: ceil(creature_count / 256) workgroups
//
// Algorithm for each SDF:
// 1. Load SDF bounding box
// 2. Convert to bounding sphere for fast rejection testing
// 3. Iterate through all froxels
// 4. Test sphere-AABB intersection (or AABB-AABB for precision)
// 5. Add SDF index to intersecting froxels via atomicAdd
@compute @workgroup_size(256, 1, 1)
fn cs_assign_sdfs(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let sdf_index = global_id.x;

    // Early out if we're past the SDF count
    if (sdf_index >= uniforms.creature_count || sdf_index >= MAX_SDF_COUNT) {
        return;
    }

    // Get SDF bounds
    let bounds = sdf_bounds.bounds[sdf_index];
    let sdf_min = get_sdf_min(bounds);
    let sdf_max = get_sdf_max(bounds);

    // Convert to bounding sphere for intersection tests
    let sphere = aabb_to_bounding_sphere(sdf_min, sdf_max);
    let sphere_center = sphere.xyz;
    let sphere_radius = sphere.w;

    // Test against all froxels
    // Note: This is O(SDFs * froxels) but each test is very cheap
    // For 1024 SDFs and 6144 froxels, that's ~6.3M tests per frame
    // Each test is just a few ALU ops, so this is GPU-friendly
    for (var i: u32 = 0u; i < TOTAL_FROXELS; i = i + 1u) {
        let froxel = froxel_bounds.bounds[i];
        let froxel_min = get_froxel_min(froxel);
        let froxel_max = get_froxel_max(froxel);

        // Use sphere-AABB test for fast rejection
        if (sphere_intersects_aabb(sphere_center, sphere_radius, froxel_min, froxel_max)) {
            // Optionally: could add AABB-AABB test here for tighter culling
            // but sphere test is usually sufficient and faster
            add_sdf_to_froxel(i, sdf_index);
        }
    }
}

// Alternative: Tighter assignment using AABB-AABB intersection
// This is more precise but slightly slower due to more comparisons
@compute @workgroup_size(256, 1, 1)
fn cs_assign_sdfs_tight(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let sdf_index = global_id.x;

    // Early out if we're past the SDF count
    if (sdf_index >= uniforms.creature_count || sdf_index >= MAX_SDF_COUNT) {
        return;
    }

    // Get SDF bounds
    let bounds = sdf_bounds.bounds[sdf_index];
    let sdf_min = get_sdf_min(bounds);
    let sdf_max = get_sdf_max(bounds);

    // Test against all froxels using AABB-AABB intersection
    for (var i: u32 = 0u; i < TOTAL_FROXELS; i = i + 1u) {
        let froxel = froxel_bounds.bounds[i];
        let froxel_min = get_froxel_min(froxel);
        let froxel_max = get_froxel_max(froxel);

        if (aabb_intersects_aabb(sdf_min, sdf_max, froxel_min, froxel_max)) {
            add_sdf_to_froxel(i, sdf_index);
        }
    }
}

// Combined clear and assign in a single dispatch
// This version has thread 0 clear all froxels first (with barrier), then all threads assign
// Note: This requires workgroup barriers and may not be more efficient than separate passes
@compute @workgroup_size(256, 1, 1)
fn cs_clear_and_assign(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_index) local_index: u32
) {
    // Phase 1: Clear froxels (all threads participate)
    // Each thread clears multiple froxels to cover all 6144
    let froxels_per_thread = (TOTAL_FROXELS + 255u) / 256u;  // = 24
    let start_froxel = local_index * froxels_per_thread;

    for (var f: u32 = 0u; f < froxels_per_thread; f = f + 1u) {
        let froxel_idx = start_froxel + f;
        if (froxel_idx < TOTAL_FROXELS) {
            clear_froxel(froxel_idx);
        }
    }

    // Workgroup barrier to ensure all froxels are cleared before assignment
    workgroupBarrier();

    // Phase 2: Assign SDFs to froxels
    let sdf_index = global_id.x;

    if (sdf_index >= uniforms.creature_count || sdf_index >= MAX_SDF_COUNT) {
        return;
    }

    // Get SDF bounds
    let bounds = sdf_bounds.bounds[sdf_index];
    let sdf_min = get_sdf_min(bounds);
    let sdf_max = get_sdf_max(bounds);

    // Convert to bounding sphere
    let sphere = aabb_to_bounding_sphere(sdf_min, sdf_max);
    let sphere_center = sphere.xyz;
    let sphere_radius = sphere.w;

    // Test against all froxels
    for (var i: u32 = 0u; i < TOTAL_FROXELS; i = i + 1u) {
        let froxel = froxel_bounds.bounds[i];
        let froxel_min = get_froxel_min(froxel);
        let froxel_max = get_froxel_max(froxel);

        if (sphere_intersects_aabb(sphere_center, sphere_radius, froxel_min, froxel_max)) {
            add_sdf_to_froxel(i, sdf_index);
        }
    }
}
