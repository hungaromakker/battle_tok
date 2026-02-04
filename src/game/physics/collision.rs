//! Collision Detection
//!
//! Pure collision functions for player-world interactions.

use glam::Vec3;

/// Result of a collision check
#[derive(Debug, Clone, Default)]
pub struct CollisionResult {
    /// Position adjustment to push player out of collision
    pub push: Vec3,
    /// Velocity adjustment (for stopping velocity in collision direction)
    pub velocity_adjustment: Vec3,
    /// Whether player is grounded on this surface
    pub grounded: bool,
    /// Ground Y position if grounded
    pub ground_y: Option<f32>,
}

impl CollisionResult {
    /// Create a new empty collision result (no collision)
    pub fn none() -> Self {
        Self::default()
    }
    
    /// Check if there was any collision
    pub fn has_collision(&self) -> bool {
        self.push != Vec3::ZERO || self.grounded
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }
    
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
}

/// Check player capsule collision with an AABB (building block)
///
/// # Arguments
/// * `player_pos` - Player feet position
/// * `player_top` - Player capsule top Y
/// * `player_radius` - Player capsule horizontal radius
/// * `player_velocity` - Current player velocity (for velocity adjustment)
/// * `aabb` - The axis-aligned bounding box to check against
///
/// # Returns
/// CollisionResult with push direction and grounding info
pub fn check_capsule_aabb_collision(
    player_pos: Vec3,
    player_top: f32,
    player_radius: f32,
    player_velocity: Vec3,
    aabb: &AABB,
) -> CollisionResult {
    let mut result = CollisionResult::none();
    
    // Find closest point on AABB to player center (XZ only)
    let closest_x = player_pos.x.clamp(aabb.min.x, aabb.max.x);
    let closest_z = player_pos.z.clamp(aabb.min.z, aabb.max.z);
    
    let dx = player_pos.x - closest_x;
    let dz = player_pos.z - closest_z;
    let horizontal_dist = (dx * dx + dz * dz).sqrt();
    
    // Check horizontal overlap
    if horizontal_dist < player_radius {
        // Check vertical overlap
        let in_vertical_range = player_pos.y < aabb.max.y && player_top > aabb.min.y;
        
        if in_vertical_range {
            // Collision! Calculate push direction
            if horizontal_dist > 0.001 {
                let push_dir = Vec3::new(dx, 0.0, dz).normalize();
                let penetration = player_radius - horizontal_dist;
                result.push = push_dir * (penetration + 0.01);
                
                // Calculate velocity adjustment
                let vel_dot = player_velocity.dot(push_dir);
                if vel_dot < 0.0 {
                    result.velocity_adjustment = -push_dir * vel_dot;
                }
            } else {
                // Player is inside block, push toward center of AABB's nearest face
                let block_center = aabb.center();
                let to_player = player_pos - block_center;
                let push_dir = Vec3::new(to_player.x.signum(), 0.0, to_player.z.signum()).normalize_or_zero();
                result.push = push_dir * (player_radius + 0.1);
            }
        }
        
        // Check if player is standing on top of block
        if player_pos.y >= aabb.max.y - 0.1 && player_pos.y <= aabb.max.y + 0.5 {
            let on_top_xz = player_pos.x >= aabb.min.x - player_radius 
                && player_pos.x <= aabb.max.x + player_radius
                && player_pos.z >= aabb.min.z - player_radius 
                && player_pos.z <= aabb.max.z + player_radius;
            
            if on_top_xz {
                result.grounded = true;
                result.ground_y = Some(aabb.max.y);
            }
        }
    }
    
    result
}

/// Check player capsule collision with a hex prism
///
/// # Arguments
/// * `player_pos` - Player feet position
/// * `player_top` - Player capsule top Y
/// * `player_radius` - Player capsule horizontal radius
/// * `player_velocity` - Current player velocity
/// * `hex_center_x` - Hex prism center X in world space
/// * `hex_center_z` - Hex prism center Z in world space
/// * `hex_bottom` - Hex prism bottom Y
/// * `hex_top` - Hex prism top Y
/// * `hex_collision_radius` - Hex prism collision radius (inscribed circle)
///
/// # Returns
/// CollisionResult with push direction and grounding info
pub fn check_capsule_hex_collision(
    player_pos: Vec3,
    player_top: f32,
    player_radius: f32,
    player_velocity: Vec3,
    hex_center_x: f32,
    hex_center_z: f32,
    hex_bottom: f32,
    hex_top: f32,
    hex_collision_radius: f32,
) -> CollisionResult {
    let mut result = CollisionResult::none();
    
    let dx = player_pos.x - hex_center_x;
    let dz = player_pos.z - hex_center_z;
    let horizontal_dist = (dx * dx + dz * dz).sqrt();
    
    if horizontal_dist < hex_collision_radius + player_radius {
        // Check vertical overlap
        let in_vertical_range = player_pos.y < hex_top && player_top > hex_bottom;
        
        if in_vertical_range {
            // Push player out
            if horizontal_dist > 0.001 {
                let push_dir = Vec3::new(dx, 0.0, dz).normalize();
                let penetration = (hex_collision_radius + player_radius) - horizontal_dist;
                result.push = push_dir * (penetration + 0.01);
                
                // Stop velocity in push direction
                let vel_dot = player_velocity.dot(push_dir);
                if vel_dot < 0.0 {
                    result.velocity_adjustment = -push_dir * vel_dot;
                }
            }
        }
        
        // Check if player is standing on top of hex prism
        if player_pos.y >= hex_top - 0.1 && player_pos.y <= hex_top + 0.5 {
            if horizontal_dist < hex_collision_radius + player_radius {
                result.grounded = true;
                result.ground_y = Some(hex_top);
            }
        }
    }
    
    result
}

/// Convert hex grid coordinates to world position
///
/// # Arguments
/// * `q` - Axial Q coordinate
/// * `r` - Axial R coordinate
/// * `level` - Vertical level
/// * `hex_radius` - Hex prism radius
/// * `hex_height` - Hex prism height
///
/// # Returns
/// World position (x, y, z)
pub fn hex_to_world_position(q: i32, r: i32, level: i32, hex_radius: f32, hex_height: f32) -> Vec3 {
    let hex_x = (q as f32) * hex_radius * 1.5;
    let hex_z = (r as f32) * hex_radius * 3.0_f32.sqrt() 
        + (q as f32).abs() % 2.0 * hex_radius * 3.0_f32.sqrt() * 0.5;
    let hex_y = (level as f32) * hex_height;
    Vec3::new(hex_x, hex_y, hex_z)
}

/// Convert world position to approximate hex grid coordinates
///
/// # Arguments
/// * `world_pos` - World position
/// * `hex_radius` - Hex prism radius
/// * `hex_height` - Hex prism height
///
/// # Returns
/// Approximate (q, r, level) coordinates
pub fn world_to_hex_coords(world_pos: Vec3, hex_radius: f32, hex_height: f32) -> (i32, i32, i32) {
    let approx_q = (world_pos.x / (hex_radius * 1.732)).round() as i32;
    let approx_r = (world_pos.z / (hex_radius * 1.5)).round() as i32;
    let approx_level = (world_pos.y / hex_height).floor() as i32;
    (approx_q, approx_r, approx_level)
}
