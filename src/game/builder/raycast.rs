//! Raycast Utilities
//!
//! Pure raycast functions for builder mode and block selection.

use glam::Vec3;
use crate::game::physics::AABB;

/// Convert screen coordinates to a world-space ray
///
/// # Arguments
/// * `screen_x` - Screen X coordinate
/// * `screen_y` - Screen Y coordinate  
/// * `screen_width` - Screen width in pixels
/// * `screen_height` - Screen height in pixels
/// * `camera_position` - Camera position in world space
/// * `camera_forward` - Camera forward direction (normalized)
/// * `camera_right` - Camera right direction (normalized)
/// * `camera_fov` - Camera field of view in radians
///
/// # Returns
/// Tuple of (ray_origin, ray_direction)
pub fn screen_to_ray(
    screen_x: f32,
    screen_y: f32,
    screen_width: f32,
    screen_height: f32,
    camera_position: Vec3,
    camera_forward: Vec3,
    camera_right: Vec3,
    camera_fov: f32,
) -> (Vec3, Vec3) {
    // Convert to normalized device coordinates (-1 to 1)
    let ndc_x = (2.0 * screen_x / screen_width) - 1.0;
    let ndc_y = 1.0 - (2.0 * screen_y / screen_height); // Flip Y
    
    // Calculate up vector from forward and right
    let up = camera_right.cross(camera_forward).normalize();
    
    // Calculate ray direction based on FOV and aspect ratio
    let aspect = screen_width / screen_height;
    let half_fov_tan = (camera_fov / 2.0).tan();
    
    let ray_dir = (camera_forward + camera_right * ndc_x * half_fov_tan * aspect + up * ndc_y * half_fov_tan).normalize();
    
    (camera_position, ray_dir)
}

/// Determine which face of an AABB was hit based on a point inside/near it
///
/// # Arguments
/// * `point` - The hit point
/// * `aabb` - The axis-aligned bounding box
///
/// # Returns
/// Tuple of (face_normal, face_center_position, face_size_wh)
pub fn determine_hit_face(point: Vec3, aabb: &AABB) -> (Vec3, Vec3, (f32, f32)) {
    let center = aabb.center();
    let half_size = (aabb.max - aabb.min) * 0.5;
    let offset = point - center;
    let abs_offset = Vec3::new(offset.x.abs(), offset.y.abs(), offset.z.abs());
    
    if abs_offset.x > abs_offset.y && abs_offset.x > abs_offset.z {
        // X face (left/right)
        let normal = Vec3::new(offset.x.signum(), 0.0, 0.0);
        let face_pos = center + normal * half_size.x;
        (normal, face_pos, (half_size.z * 2.0, half_size.y * 2.0))
    } else if abs_offset.y > abs_offset.z {
        // Y face (top/bottom)
        let normal = Vec3::new(0.0, offset.y.signum(), 0.0);
        let face_pos = center + normal * half_size.y;
        (normal, face_pos, (half_size.x * 2.0, half_size.z * 2.0))
    } else {
        // Z face (front/back)
        let normal = Vec3::new(0.0, 0.0, offset.z.signum());
        let face_pos = center + normal * half_size.z;
        (normal, face_pos, (half_size.x * 2.0, half_size.y * 2.0))
    }
}

/// Calculate the adjacent block position based on hit point and AABB
///
/// # Arguments
/// * `hit_point` - The point where the ray hit
/// * `aabb` - The AABB of the block that was hit
/// * `grid_size` - The grid snap size
///
/// # Returns
/// Position where a new block should be placed
pub fn calculate_adjacent_block_position(hit_point: Vec3, aabb: &AABB, grid_size: f32) -> Vec3 {
    let block_center = aabb.center();
    let hit_offset = hit_point - block_center;
    
    // Find dominant axis
    let abs_offset = Vec3::new(hit_offset.x.abs(), hit_offset.y.abs(), hit_offset.z.abs());
    
    let placement_pos = if abs_offset.y > abs_offset.x && abs_offset.y > abs_offset.z {
        // Top or bottom
        if hit_offset.y > 0.0 {
            Vec3::new(block_center.x, aabb.max.y + 0.5, block_center.z)
        } else {
            Vec3::new(block_center.x, aabb.min.y - 0.5, block_center.z)
        }
    } else if abs_offset.x > abs_offset.z {
        // Left or right
        if hit_offset.x > 0.0 {
            Vec3::new(aabb.max.x + 0.5, block_center.y, block_center.z)
        } else {
            Vec3::new(aabb.min.x - 0.5, block_center.y, block_center.z)
        }
    } else {
        // Front or back
        if hit_offset.z > 0.0 {
            Vec3::new(block_center.x, block_center.y, aabb.max.z + 0.5)
        } else {
            Vec3::new(block_center.x, block_center.y, aabb.min.z - 0.5)
        }
    };
    
    // Snap to grid
    Vec3::new(
        (placement_pos.x / grid_size).round() * grid_size,
        placement_pos.y,
        (placement_pos.z / grid_size).round() * grid_size,
    )
}

/// Snap a position to the nearest grid point
///
/// # Arguments
/// * `position` - The position to snap
/// * `grid_size` - The grid size
///
/// # Returns
/// Snapped position (XZ only, Y unchanged)
pub fn snap_to_grid(position: Vec3, grid_size: f32) -> Vec3 {
    Vec3::new(
        (position.x / grid_size).round() * grid_size,
        position.y,
        (position.z / grid_size).round() * grid_size,
    )
}

/// Find the best snap position from nearby block positions
///
/// # Arguments
/// * `position` - Current position
/// * `block_centers` - Iterator of (block_center, aabb_max_y) tuples
/// * `grid_size` - Grid size for snapping
/// * `snap_distance` - Maximum distance to snap
///
/// # Returns
/// Best snapped position
pub fn find_snap_position<'a>(
    position: Vec3,
    block_centers: impl Iterator<Item = (Vec3, f32)>,
    grid_size: f32,
    snap_distance: f32,
) -> Vec3 {
    let mut best_pos = position;
    let mut best_dist = snap_distance;
    
    for (block_center, aabb_max_y) in block_centers {
        // Possible snap positions (adjacent to this block)
        let snap_positions = [
            Vec3::new(block_center.x + grid_size, position.y, block_center.z), // Right
            Vec3::new(block_center.x - grid_size, position.y, block_center.z), // Left
            Vec3::new(block_center.x, position.y, block_center.z + grid_size), // Front
            Vec3::new(block_center.x, position.y, block_center.z - grid_size), // Back
            Vec3::new(block_center.x, aabb_max_y + 0.5, block_center.z), // Top
        ];
        
        for snap_pos in snap_positions {
            let dist = (snap_pos - position).length();
            if dist < best_dist {
                best_dist = dist;
                best_pos = snap_pos;
            }
        }
    }
    
    best_pos
}

/// Ray-march along a ray to find terrain intersection
///
/// # Arguments
/// * `ray_origin` - Start of the ray
/// * `ray_dir` - Direction of the ray (normalized)
/// * `terrain_height_fn` - Function that returns terrain height at (x, z)
/// * `max_distance` - Maximum ray distance
/// * `step_size` - Step size for marching
///
/// # Returns
/// Optional hit position on terrain
pub fn ray_terrain_intersection<F>(
    ray_origin: Vec3,
    ray_dir: Vec3,
    terrain_height_fn: F,
    max_distance: f32,
    step_size: f32,
) -> Option<Vec3>
where
    F: Fn(f32, f32) -> f32,
{
    let mut t = 1.0;
    
    while t < max_distance {
        let p = ray_origin + ray_dir * t;
        let ground_height = terrain_height_fn(p.x, p.z);
        
        if p.y <= ground_height {
            return Some(p);
        }
        
        t += step_size;
    }
    
    None
}
