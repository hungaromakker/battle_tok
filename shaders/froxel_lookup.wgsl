// Froxel Lookup Functions (US-036)
//
// This shader module provides WGSL functions for froxel coordinate calculation.
// Froxels (frustum + voxels) partition the view frustum into a 3D grid for
// efficient SDF culling during raymarching.
//
// Grid Configuration:
// - X/Y: 16×16 screen-space tiles
// - Z: 24 exponentially distributed depth slices
// - Total: 6,144 froxels
//
// Exponential Depth Distribution:
// depth = near * (far/near)^(slice/total_slices)
// This places more slices near the camera where detail matters most.

// ============================================================================
// CONSTANTS
// ============================================================================

/// Number of froxel tiles in the X (horizontal) direction
const FROXEL_TILES_X: u32 = 16u;

/// Number of froxel tiles in the Y (vertical) direction
const FROXEL_TILES_Y: u32 = 16u;

/// Number of depth slices for froxel partitioning
const FROXEL_DEPTH_SLICES: u32 = 24u;

/// Total number of froxels in the grid
const TOTAL_FROXELS: u32 = FROXEL_TILES_X * FROXEL_TILES_Y * FROXEL_DEPTH_SLICES; // 6144

/// Invalid froxel index (returned when position is outside frustum)
const FROXEL_INVALID: u32 = 0xFFFFFFFFu;

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Camera parameters needed for froxel calculations.
/// This should match the data available in the main shader's uniform buffer.
struct FroxelCamera {
    /// Camera position in world space (meters)
    position: vec3<f32>,
    /// Forward direction (normalized, camera looks this way)
    forward: vec3<f32>,
    /// Up direction (normalized)
    up: vec3<f32>,
    /// Right direction (normalized)
    right: vec3<f32>,
    /// Vertical field of view in radians
    fov_y: f32,
    /// Aspect ratio (width / height)
    aspect_ratio: f32,
    /// Near plane distance in meters
    near: f32,
    /// Far plane distance in meters
    far: f32,
}

/// Ray structure for froxel traversal
struct FroxelRay {
    /// Ray origin in world space
    origin: vec3<f32>,
    /// Ray direction (normalized)
    direction: vec3<f32>,
}

// ============================================================================
// DEPTH SLICE FUNCTIONS
// ============================================================================

/// Calculate the near depth for a given slice index.
///
/// Uses exponential distribution: depth = near * (far/near)^(slice/total_slices)
/// This matches the CPU-side depth_slice_bounds() function in froxel_config.rs.
///
/// # Arguments
/// * `slice` - Depth slice index (0 to FROXEL_DEPTH_SLICES - 1)
/// * `near` - Near plane distance
/// * `far` - Far plane distance
///
/// # Returns
/// The near depth boundary of the slice in world units (meters)
fn froxel_slice_near_depth(slice: u32, near: f32, far: f32) -> f32 {
    let total_slices = f32(FROXEL_DEPTH_SLICES);
    let t = f32(slice) / total_slices;
    let ratio = far / near;
    return near * pow(ratio, t);
}

/// Calculate the far depth for a given slice index.
///
/// # Arguments
/// * `slice` - Depth slice index (0 to FROXEL_DEPTH_SLICES - 1)
/// * `near` - Near plane distance
/// * `far` - Far plane distance
///
/// # Returns
/// The far depth boundary of the slice in world units (meters)
fn froxel_slice_far_depth(slice: u32, near: f32, far: f32) -> f32 {
    let total_slices = f32(FROXEL_DEPTH_SLICES);
    let t = f32(slice + 1u) / total_slices;
    let ratio = far / near;
    return near * pow(ratio, t);
}

/// Convert a linear depth value to a depth slice index.
///
/// Inverse of exponential distribution: slice = total_slices * log(depth/near) / log(far/near)
///
/// # Arguments
/// * `depth` - Linear depth in world units (meters), must be positive
/// * `near` - Near plane distance
/// * `far` - Far plane distance
///
/// # Returns
/// The depth slice index (0 to FROXEL_DEPTH_SLICES - 1), or FROXEL_DEPTH_SLICES if outside range
fn froxel_depth_to_slice(depth: f32, near: f32, far: f32) -> u32 {
    // Handle edge cases
    if depth < near {
        return FROXEL_DEPTH_SLICES; // Invalid - before near plane
    }
    if depth >= far {
        return FROXEL_DEPTH_SLICES; // Invalid - beyond far plane
    }

    // Inverse of exponential: t = log(depth/near) / log(far/near)
    let ratio = far / near;
    let t = log(depth / near) / log(ratio);

    // Convert to slice index (floor to get containing slice)
    let slice = u32(t * f32(FROXEL_DEPTH_SLICES));

    // Clamp to valid range (should already be valid, but be safe)
    return min(slice, FROXEL_DEPTH_SLICES - 1u);
}

// ============================================================================
// COORDINATE CONVERSION FUNCTIONS
// ============================================================================

/// Convert world position to view-space coordinates relative to camera.
///
/// Returns (right_offset, up_offset, forward_depth) where:
/// - right_offset: offset along camera right vector
/// - up_offset: offset along camera up vector
/// - forward_depth: depth along camera forward vector (distance from camera)
fn world_to_view_coords(world_pos: vec3<f32>, camera: FroxelCamera) -> vec3<f32> {
    let offset = world_pos - camera.position;

    // Project onto camera axes
    let right_offset = dot(offset, camera.right);
    let up_offset = dot(offset, camera.up);
    let forward_depth = dot(offset, camera.forward);

    return vec3<f32>(right_offset, up_offset, forward_depth);
}

/// Convert view-space coordinates to NDC (normalized device coordinates).
///
/// NDC range: -1 to +1 for both X and Y (left/bottom to right/top)
///
/// # Arguments
/// * `view_coords` - (right_offset, up_offset, forward_depth) from world_to_view_coords
/// * `camera` - Camera parameters
///
/// # Returns
/// * NDC coordinates (x, y), or values outside [-1, 1] if outside frustum
fn view_to_ndc(view_coords: vec3<f32>, camera: FroxelCamera) -> vec2<f32> {
    let depth = view_coords.z;

    // Avoid division by zero/negative
    if depth <= 0.0 {
        return vec2<f32>(1000.0, 1000.0); // Way outside NDC range
    }

    // Calculate half-extents at this depth
    let half_fov_y = camera.fov_y * 0.5;
    let half_height = depth * tan(half_fov_y);
    let half_width = half_height * camera.aspect_ratio;

    // Convert to NDC (-1 to +1)
    let ndc_x = view_coords.x / half_width;
    let ndc_y = view_coords.y / half_height;

    return vec2<f32>(ndc_x, ndc_y);
}

/// Convert NDC coordinates to tile indices.
///
/// # Arguments
/// * `ndc` - NDC coordinates in range [-1, +1]
///
/// # Returns
/// * Tile indices (x, y), or values >= FROXEL_TILES if outside frustum
fn ndc_to_tile(ndc: vec2<f32>) -> vec2<u32> {
    // Check if within NDC bounds
    if ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 {
        return vec2<u32>(FROXEL_TILES_X, FROXEL_TILES_Y); // Invalid
    }

    // Map NDC [-1, +1] to tile index [0, TILES-1]
    // ndc=-1 -> tile=0, ndc=+1 -> tile=TILES-1
    let tile_x = u32((ndc.x + 1.0) * 0.5 * f32(FROXEL_TILES_X));
    let tile_y = u32((ndc.y + 1.0) * 0.5 * f32(FROXEL_TILES_Y));

    // Clamp to valid range (handle ndc=+1.0 edge case)
    return vec2<u32>(
        min(tile_x, FROXEL_TILES_X - 1u),
        min(tile_y, FROXEL_TILES_Y - 1u)
    );
}

// ============================================================================
// MAIN FROXEL LOOKUP FUNCTIONS
// ============================================================================

/// Calculate the linear froxel index from 3D grid coordinates.
///
/// Grid layout: X varies fastest, then Y, then Z (row-major order)
/// index = z * (TILES_X * TILES_Y) + y * TILES_X + x
///
/// This matches the CPU-side FroxelBoundsBuffer::froxel_index() function.
fn froxel_coords_to_index(x: u32, y: u32, z: u32) -> u32 {
    return z * (FROXEL_TILES_X * FROXEL_TILES_Y) + y * FROXEL_TILES_X + x;
}

/// Convert a linear froxel index back to 3D grid coordinates.
///
/// # Returns
/// (x, y, z) grid coordinates
fn froxel_index_to_coords(index: u32) -> vec3<u32> {
    let tiles_per_slice = FROXEL_TILES_X * FROXEL_TILES_Y;
    let z = index / tiles_per_slice;
    let remaining = index % tiles_per_slice;
    let y = remaining / FROXEL_TILES_X;
    let x = remaining % FROXEL_TILES_X;
    return vec3<u32>(x, y, z);
}

/// Get the froxel index for a world-space position.
///
/// This is the main function for determining which froxel contains a given point.
///
/// # Arguments
/// * `world_pos` - Position in world space (meters)
/// * `camera` - Camera parameters
///
/// # Returns
/// * Linear froxel index (0 to TOTAL_FROXELS-1), or FROXEL_INVALID if outside frustum
///
/// # Algorithm
/// 1. Transform world position to view space
/// 2. Check if depth is within [near, far]
/// 3. Convert to NDC and check if within [-1, +1]
/// 4. Map to tile (x, y) indices
/// 5. Map depth to slice (z) index
/// 6. Compute linear index
fn get_froxel_index(world_pos: vec3<f32>, camera: FroxelCamera) -> u32 {
    // Step 1: Transform to view space
    let view_coords = world_to_view_coords(world_pos, camera);
    let depth = view_coords.z;

    // Step 2: Check depth bounds
    if depth < camera.near || depth >= camera.far {
        return FROXEL_INVALID;
    }

    // Step 3: Convert to NDC
    let ndc = view_to_ndc(view_coords, camera);

    // Step 4: Check NDC bounds and get tile indices
    if ndc.x < -1.0 || ndc.x > 1.0 || ndc.y < -1.0 || ndc.y > 1.0 {
        return FROXEL_INVALID;
    }

    let tile = ndc_to_tile(ndc);

    // Extra check (should be redundant after NDC check)
    if tile.x >= FROXEL_TILES_X || tile.y >= FROXEL_TILES_Y {
        return FROXEL_INVALID;
    }

    // Step 5: Get depth slice
    let slice = froxel_depth_to_slice(depth, camera.near, camera.far);
    if slice >= FROXEL_DEPTH_SLICES {
        return FROXEL_INVALID;
    }

    // Step 6: Compute linear index
    return froxel_coords_to_index(tile.x, tile.y, slice);
}

// ============================================================================
// FROXEL EXIT DISTANCE FUNCTIONS
// ============================================================================

/// Get the NDC bounds for a tile.
///
/// # Returns
/// (ndc_left, ndc_right, ndc_bottom, ndc_top)
fn get_tile_ndc_bounds(tile_x: u32, tile_y: u32) -> vec4<f32> {
    let tile_width_ndc = 2.0 / f32(FROXEL_TILES_X);
    let tile_height_ndc = 2.0 / f32(FROXEL_TILES_Y);

    let ndc_left = -1.0 + f32(tile_x) * tile_width_ndc;
    let ndc_right = -1.0 + f32(tile_x + 1u) * tile_width_ndc;
    let ndc_bottom = -1.0 + f32(tile_y) * tile_height_ndc;
    let ndc_top = -1.0 + f32(tile_y + 1u) * tile_height_ndc;

    return vec4<f32>(ndc_left, ndc_right, ndc_bottom, ndc_top);
}

/// Calculate the distance along a ray to exit a froxel.
///
/// This function computes how far a ray travels before leaving the current froxel,
/// which is useful for froxel-based raymarching where we can skip to the next
/// froxel boundary.
///
/// # Arguments
/// * `ray` - Ray origin and direction (in world space)
/// * `froxel_idx` - Linear froxel index
/// * `camera` - Camera parameters (needed to reconstruct froxel bounds)
///
/// # Returns
/// * Distance to exit the froxel along the ray direction, or a large value if invalid
///
/// # Algorithm
/// The froxel is bounded by:
/// - 4 planes from the frustum (left, right, top, bottom)
/// - 2 depth boundaries (near_depth, far_depth of the slice)
///
/// We find the intersection distance with each boundary and return the minimum
/// positive distance.
fn get_froxel_exit_distance(ray: FroxelRay, froxel_idx: u32, camera: FroxelCamera) -> f32 {
    // Check for invalid froxel index
    if froxel_idx >= TOTAL_FROXELS {
        return 10000.0; // Large distance for invalid froxels
    }

    // Get froxel coordinates
    let coords = froxel_index_to_coords(froxel_idx);
    let tile_x = coords.x;
    let tile_y = coords.y;
    let slice_z = coords.z;

    // Get depth bounds for this slice
    let depth_near = froxel_slice_near_depth(slice_z, camera.near, camera.far);
    let depth_far = froxel_slice_far_depth(slice_z, camera.near, camera.far);

    // Get NDC bounds for this tile
    let ndc_bounds = get_tile_ndc_bounds(tile_x, tile_y);
    let ndc_left = ndc_bounds.x;
    let ndc_right = ndc_bounds.y;
    let ndc_bottom = ndc_bounds.z;
    let ndc_top = ndc_bounds.w;

    // Calculate half-angle tangents
    let half_fov_y = camera.fov_y * 0.5;
    let tan_half_fov_y = tan(half_fov_y);
    let tan_half_fov_x = tan_half_fov_y * camera.aspect_ratio;

    // Track minimum positive exit distance
    var min_exit_dist = 10000.0;

    // Transform ray to view space for calculations
    let ray_offset = ray.origin - camera.position;
    let ray_view_x = dot(ray_offset, camera.right);
    let ray_view_y = dot(ray_offset, camera.up);
    let ray_view_z = dot(ray_offset, camera.forward);

    let dir_view_x = dot(ray.direction, camera.right);
    let dir_view_y = dot(ray.direction, camera.up);
    let dir_view_z = dot(ray.direction, camera.forward);

    // === Check depth plane intersections ===

    // Near depth plane (z = depth_near)
    if abs(dir_view_z) > 0.0001 {
        let t_near = (depth_near - ray_view_z) / dir_view_z;
        if t_near > 0.001 {
            min_exit_dist = min(min_exit_dist, t_near);
        }

        // Far depth plane (z = depth_far)
        let t_far = (depth_far - ray_view_z) / dir_view_z;
        if t_far > 0.001 {
            min_exit_dist = min(min_exit_dist, t_far);
        }
    }

    // === Check frustum plane intersections ===
    // Each frustum plane passes through the camera origin and is defined by NDC bounds

    // Left plane: x/z = ndc_left * tan_half_fov_x
    // Plane equation: x - z * ndc_left * tan_half_fov_x = 0
    {
        let plane_coeff = ndc_left * tan_half_fov_x;
        let denom = dir_view_x - dir_view_z * plane_coeff;
        if abs(denom) > 0.0001 {
            let t = -(ray_view_x - ray_view_z * plane_coeff) / denom;
            if t > 0.001 {
                min_exit_dist = min(min_exit_dist, t);
            }
        }
    }

    // Right plane: x/z = ndc_right * tan_half_fov_x
    {
        let plane_coeff = ndc_right * tan_half_fov_x;
        let denom = dir_view_x - dir_view_z * plane_coeff;
        if abs(denom) > 0.0001 {
            let t = -(ray_view_x - ray_view_z * plane_coeff) / denom;
            if t > 0.001 {
                min_exit_dist = min(min_exit_dist, t);
            }
        }
    }

    // Bottom plane: y/z = ndc_bottom * tan_half_fov_y
    {
        let plane_coeff = ndc_bottom * tan_half_fov_y;
        let denom = dir_view_y - dir_view_z * plane_coeff;
        if abs(denom) > 0.0001 {
            let t = -(ray_view_y - ray_view_z * plane_coeff) / denom;
            if t > 0.001 {
                min_exit_dist = min(min_exit_dist, t);
            }
        }
    }

    // Top plane: y/z = ndc_top * tan_half_fov_y
    {
        let plane_coeff = ndc_top * tan_half_fov_y;
        let denom = dir_view_y - dir_view_z * plane_coeff;
        if abs(denom) > 0.0001 {
            let t = -(ray_view_y - ray_view_z * plane_coeff) / denom;
            if t > 0.001 {
                min_exit_dist = min(min_exit_dist, t);
            }
        }
    }

    return min_exit_dist;
}

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

/// Check if a froxel index is valid.
fn is_froxel_valid(froxel_idx: u32) -> bool {
    return froxel_idx != FROXEL_INVALID && froxel_idx < TOTAL_FROXELS;
}

/// Get the next froxel along a ray direction from current position.
///
/// # Arguments
/// * `current_idx` - Current froxel index
/// * `ray_dir` - Ray direction in world space
/// * `camera` - Camera parameters
///
/// # Returns
/// * Next froxel index, or FROXEL_INVALID if exiting the frustum
fn get_next_froxel(current_idx: u32, ray_dir: vec3<f32>, camera: FroxelCamera) -> u32 {
    if current_idx >= TOTAL_FROXELS {
        return FROXEL_INVALID;
    }

    let coords = froxel_index_to_coords(current_idx);
    var next_x = i32(coords.x);
    var next_y = i32(coords.y);
    var next_z = i32(coords.z);

    // Determine direction of ray in view space
    let dir_view_x = dot(ray_dir, camera.right);
    let dir_view_y = dot(ray_dir, camera.up);
    let dir_view_z = dot(ray_dir, camera.forward);

    // Simple heuristic: move in the dominant direction
    // A more accurate implementation would compute actual intersection distances
    let abs_x = abs(dir_view_x);
    let abs_y = abs(dir_view_y);
    let abs_z = abs(dir_view_z);

    if abs_z >= abs_x && abs_z >= abs_y {
        // Moving primarily in depth direction
        if dir_view_z > 0.0 {
            next_z = next_z + 1;
        } else {
            next_z = next_z - 1;
        }
    } else if abs_x >= abs_y {
        // Moving primarily in horizontal direction
        if dir_view_x > 0.0 {
            next_x = next_x + 1;
        } else {
            next_x = next_x - 1;
        }
    } else {
        // Moving primarily in vertical direction
        if dir_view_y > 0.0 {
            next_y = next_y + 1;
        } else {
            next_y = next_y - 1;
        }
    }

    // Check bounds
    if next_x < 0 || next_x >= i32(FROXEL_TILES_X) ||
       next_y < 0 || next_y >= i32(FROXEL_TILES_Y) ||
       next_z < 0 || next_z >= i32(FROXEL_DEPTH_SLICES) {
        return FROXEL_INVALID;
    }

    return froxel_coords_to_index(u32(next_x), u32(next_y), u32(next_z));
}
