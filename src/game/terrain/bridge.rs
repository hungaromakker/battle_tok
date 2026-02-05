//! Bridge Mesh Generation
//!
//! Generates procedural bridge meshes with wood planks and chain supports
//! connecting two hex islands.

use crate::game::types::{Mesh, Vertex};
use glam::Vec3;

// ============================================================================
// BRIDGE CONFIGURATION
// ============================================================================

/// Bridge configuration parameters
#[derive(Clone, Debug)]
pub struct BridgeConfig {
    /// Width of the bridge walkway (meters)
    pub width: f32,
    /// Thickness of wood planks (meters)
    pub plank_thickness: f32,
    /// Gap between planks (meters)
    pub plank_gap: f32,
    /// Height of chain rails above planks (meters)
    pub rail_height: f32,
    /// Chain sag amount (fraction of span, 0.1 = 10% sag)
    pub chain_sag: f32,
    /// Number of chain links per meter
    pub chain_density: f32,
    /// Chain link thickness (meters)
    pub chain_thickness: f32,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            width: 2.5,
            plank_thickness: 0.15,
            plank_gap: 0.08,
            rail_height: 1.0,
            chain_sag: 0.12,
            chain_density: 4.0,
            chain_thickness: 0.08,
        }
    }
}

// ============================================================================
// BRIDGE COLORS
// ============================================================================

/// Colors for bridge materials
pub mod colors {
    /// Dark wood plank color
    pub const WOOD_DARK: [f32; 4] = [0.28, 0.18, 0.10, 1.0];
    /// Light wood plank color (for variation)
    pub const WOOD_LIGHT: [f32; 4] = [0.38, 0.26, 0.16, 1.0];
    /// Weathered wood color
    pub const WOOD_WEATHERED: [f32; 4] = [0.32, 0.28, 0.22, 1.0];
    /// Chain metal color
    pub const CHAIN_METAL: [f32; 4] = [0.45, 0.48, 0.52, 1.0];
    /// Rusty chain color
    pub const CHAIN_RUSTY: [f32; 4] = [0.50, 0.35, 0.25, 1.0];
    /// Support post color (darker wood)
    pub const POST_WOOD: [f32; 4] = [0.22, 0.14, 0.08, 1.0];
}

// ============================================================================
// BRIDGE GENERATION
// ============================================================================

/// Generate a complete bridge mesh between two points
pub fn generate_bridge(start: Vec3, end: Vec3, config: &BridgeConfig) -> Mesh {
    let mut mesh = Mesh::new();

    // Calculate bridge direction and length
    let span = end - start;
    let length = span.length();
    let direction = span.normalize_or_zero();
    let right = direction.cross(Vec3::Y).normalize_or_zero();

    // If direction is nearly vertical, use a different right vector
    let right = if right.length() < 0.1 { Vec3::X } else { right };

    // Generate components
    let planks = generate_planks(start, end, direction, right, config);
    let left_chain = generate_chain(
        start + right * (config.width / 2.0) + Vec3::Y * config.rail_height,
        end + right * (config.width / 2.0) + Vec3::Y * config.rail_height,
        config,
    );
    let right_chain = generate_chain(
        start - right * (config.width / 2.0) + Vec3::Y * config.rail_height,
        end - right * (config.width / 2.0) + Vec3::Y * config.rail_height,
        config,
    );
    let start_posts = generate_posts(start, direction, right, config);
    let end_posts = generate_posts(end, -direction, right, config);

    // Merge all components
    mesh.merge(&planks);
    mesh.merge(&left_chain);
    mesh.merge(&right_chain);
    mesh.merge(&start_posts);
    mesh.merge(&end_posts);

    mesh
}

/// Generate wood planks for the bridge walkway
fn generate_planks(
    start: Vec3,
    end: Vec3,
    direction: Vec3,
    right: Vec3,
    config: &BridgeConfig,
) -> Mesh {
    let mut mesh = Mesh::new();

    let span = end - start;
    let length = span.length();
    let plank_width = config.width;
    let plank_length = 0.25; // Length of each plank along bridge direction
    let plank_height = config.plank_thickness;

    let num_planks = (length / (plank_length + config.plank_gap)).floor() as u32;

    for i in 0..num_planks {
        let t = (i as f32 + 0.5) / num_planks as f32;
        let center = start + span * t;

        // Slight sag in the middle following catenary curve
        let sag_t = (t - 0.5).abs() * 2.0; // 0 at middle, 1 at ends
        let sag = (1.0 - sag_t * sag_t) * config.chain_sag * length * 0.3;
        let plank_center = center - Vec3::Y * sag;

        // Vary wood color for visual interest
        let color_seed = (i as f32 * 7.3).sin() * 0.5 + 0.5;
        let color = if color_seed < 0.33 {
            colors::WOOD_DARK
        } else if color_seed < 0.66 {
            colors::WOOD_LIGHT
        } else {
            colors::WOOD_WEATHERED
        };

        let plank = generate_oriented_plank(
            plank_center,
            Vec3::new(plank_width, plank_height, plank_length),
            direction,
            right,
            color,
        );
        mesh.merge(&plank);
    }

    mesh
}

/// Generate an oriented plank (box aligned to bridge direction)
fn generate_oriented_plank(
    center: Vec3,
    size: Vec3,
    forward: Vec3,
    right: Vec3,
    color: [f32; 4],
) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let up = right.cross(forward).normalize();
    let (hw, hh, hl) = (size.x / 2.0, size.y / 2.0, size.z / 2.0);

    // Local space corners
    let corners = [
        Vec3::new(-hw, -hh, -hl),
        Vec3::new(hw, -hh, -hl),
        Vec3::new(hw, hh, -hl),
        Vec3::new(-hw, hh, -hl),
        Vec3::new(-hw, -hh, hl),
        Vec3::new(hw, -hh, hl),
        Vec3::new(hw, hh, hl),
        Vec3::new(-hw, hh, hl),
    ];

    // Transform to world space
    let transform =
        |local: Vec3| -> Vec3 { center + right * local.x + up * local.y + forward * local.z };

    let faces: [([usize; 4], Vec3); 6] = [
        ([0, 3, 2, 1], -forward), // Back
        ([4, 5, 6, 7], forward),  // Front
        ([0, 4, 7, 3], -right),   // Left
        ([1, 2, 6, 5], right),    // Right
        ([3, 7, 6, 2], up),       // Top
        ([0, 1, 5, 4], -up),      // Bottom
    ];

    for (face_indices, local_normal) in &faces {
        let base = vertices.len() as u32;
        let world_normal = right * local_normal.x + up * local_normal.y + forward * local_normal.z;

        for &i in face_indices {
            let pos = transform(corners[i]);
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [world_normal.x, world_normal.y, world_normal.z],
                color,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh { vertices, indices }
}

/// Generate a sagging chain between two points
fn generate_chain(start: Vec3, end: Vec3, config: &BridgeConfig) -> Mesh {
    let mut mesh = Mesh::new();

    let span = end - start;
    let length = span.length();
    let direction = span.normalize_or_zero();

    let num_links = (length * config.chain_density).ceil() as u32;
    let num_links = num_links.max(4);

    for i in 0..num_links {
        let t = i as f32 / (num_links - 1) as f32;

        // Catenary sag curve: lower in the middle
        let sag_t = (t - 0.5) * 2.0; // -1 to 1
        let sag = (1.0 - sag_t * sag_t) * config.chain_sag * length;

        let pos = start + span * t - Vec3::Y * sag;

        // Alternate between rusty and normal metal for variation
        let color = if i % 3 == 0 {
            colors::CHAIN_RUSTY
        } else {
            colors::CHAIN_METAL
        };

        let link = generate_chain_link(pos, config.chain_thickness, direction, color);
        mesh.merge(&link);
    }

    mesh
}

/// Generate a single chain link (simplified as a small box for performance)
fn generate_chain_link(center: Vec3, thickness: f32, direction: Vec3, color: [f32; 4]) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Simple box link for performance (could be torus for more detail)
    let half = thickness / 2.0;
    let link_length = thickness * 1.5;

    let up = Vec3::Y;
    let right = direction.cross(up).normalize_or_zero();
    let right = if right.length() < 0.1 { Vec3::X } else { right };

    let corners = [
        Vec3::new(-half, -link_length, -half),
        Vec3::new(half, -link_length, -half),
        Vec3::new(half, link_length, -half),
        Vec3::new(-half, link_length, -half),
        Vec3::new(-half, -link_length, half),
        Vec3::new(half, -link_length, half),
        Vec3::new(half, link_length, half),
        Vec3::new(-half, link_length, half),
    ];

    let faces: [([usize; 4], Vec3); 6] = [
        ([0, 3, 2, 1], Vec3::new(0.0, 0.0, -1.0)),
        ([4, 5, 6, 7], Vec3::new(0.0, 0.0, 1.0)),
        ([0, 4, 7, 3], Vec3::new(-1.0, 0.0, 0.0)),
        ([1, 2, 6, 5], Vec3::new(1.0, 0.0, 0.0)),
        ([3, 7, 6, 2], Vec3::new(0.0, 1.0, 0.0)),
        ([0, 1, 5, 4], Vec3::new(0.0, -1.0, 0.0)),
    ];

    for (face_indices, normal) in &faces {
        let base = vertices.len() as u32;
        for &i in face_indices {
            let pos = center + corners[i];
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [normal.x, normal.y, normal.z],
                color,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh { vertices, indices }
}

/// Generate support posts at bridge ends
fn generate_posts(position: Vec3, forward: Vec3, right: Vec3, config: &BridgeConfig) -> Mesh {
    let mut mesh = Mesh::new();

    let post_width = 0.25;
    let post_height = config.rail_height + 0.5;
    let post_depth = 0.25;

    // Left post
    let left_pos = position + right * (config.width / 2.0) + Vec3::Y * (post_height / 2.0);
    let left_post = generate_post(left_pos, post_width, post_height, post_depth);
    mesh.merge(&left_post);

    // Right post
    let right_pos = position - right * (config.width / 2.0) + Vec3::Y * (post_height / 2.0);
    let right_post = generate_post(right_pos, post_width, post_height, post_depth);
    mesh.merge(&right_post);

    mesh
}

/// Generate a single support post
fn generate_post(center: Vec3, width: f32, height: f32, depth: f32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let (hw, hh, hd) = (width / 2.0, height / 2.0, depth / 2.0);

    let corners = [
        Vec3::new(-hw, -hh, -hd),
        Vec3::new(hw, -hh, -hd),
        Vec3::new(hw, hh, -hd),
        Vec3::new(-hw, hh, -hd),
        Vec3::new(-hw, -hh, hd),
        Vec3::new(hw, -hh, hd),
        Vec3::new(hw, hh, hd),
        Vec3::new(-hw, hh, hd),
    ];

    let faces: [([usize; 4], Vec3); 6] = [
        ([0, 3, 2, 1], Vec3::new(0.0, 0.0, -1.0)),
        ([4, 5, 6, 7], Vec3::new(0.0, 0.0, 1.0)),
        ([0, 4, 7, 3], Vec3::new(-1.0, 0.0, 0.0)),
        ([1, 2, 6, 5], Vec3::new(1.0, 0.0, 0.0)),
        ([3, 7, 6, 2], Vec3::new(0.0, 1.0, 0.0)),
        ([0, 1, 5, 4], Vec3::new(0.0, -1.0, 0.0)),
    ];

    let color = colors::POST_WOOD;

    for (face_indices, normal) in &faces {
        let base = vertices.len() as u32;
        for &i in face_indices {
            let pos = center + corners[i];
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [normal.x, normal.y, normal.z],
                color,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh { vertices, indices }
}

// ============================================================================
// BRIDGE COLLISION
// ============================================================================

/// Axis-aligned bounding box for collision
#[derive(Clone, Copy, Debug)]
pub struct BridgeAABB {
    pub min: Vec3,
    pub max: Vec3,
}

/// Generate collision AABBs for a bridge (for unit pathfinding)
pub fn generate_bridge_collision(start: Vec3, end: Vec3, config: &BridgeConfig) -> Vec<BridgeAABB> {
    let mut colliders = Vec::new();

    let span = end - start;
    let length = span.length();
    let direction = span.normalize_or_zero();
    let right = direction.cross(Vec3::Y).normalize_or_zero();
    let right = if right.length() < 0.1 { Vec3::X } else { right };

    // Divide bridge into segments for collision
    let num_segments = (length / 2.0).ceil() as u32; // ~2 meter segments
    let num_segments = num_segments.max(2);

    for i in 0..num_segments {
        let t0 = i as f32 / num_segments as f32;
        let t1 = (i + 1) as f32 / num_segments as f32;

        let p0 = start + span * t0;
        let p1 = start + span * t1;

        // Calculate sag at this segment
        let mid_t = (t0 + t1) / 2.0;
        let sag_t = (mid_t - 0.5).abs() * 2.0;
        let sag = (1.0 - sag_t * sag_t) * config.chain_sag * length * 0.3;

        // AABB for this segment
        let half_width = config.width / 2.0;
        let segment_min = Vec3::new(
            p0.x.min(p1.x) - half_width,
            p0.y.min(p1.y) - sag - config.plank_thickness,
            p0.z.min(p1.z) - half_width,
        );
        let segment_max = Vec3::new(
            p0.x.max(p1.x) + half_width,
            p0.y.max(p1.y) - sag + 0.1, // Small height for walking on
            p0.z.max(p1.z) + half_width,
        );

        colliders.push(BridgeAABB {
            min: segment_min,
            max: segment_max,
        });
    }

    colliders
}

/// Check if a point is on the bridge walkway
pub fn is_point_on_bridge(point: Vec3, start: Vec3, end: Vec3, config: &BridgeConfig) -> bool {
    let span = end - start;
    let length = span.length();
    let direction = span.normalize_or_zero();

    // Project point onto bridge line
    let to_point = point - start;
    let along = to_point.dot(direction);

    // Check if within bridge length
    if along < 0.0 || along > length {
        return false;
    }

    // Calculate sag at this position
    let t = along / length;
    let sag_t = (t - 0.5).abs() * 2.0;
    let sag = (1.0 - sag_t * sag_t) * config.chain_sag * length * 0.3;

    // Expected Y position on bridge
    let expected_y = start.y + (end.y - start.y) * t - sag;

    // Check height (allow some tolerance)
    if (point.y - expected_y).abs() > 1.0 {
        return false;
    }

    // Check perpendicular distance (width)
    let on_line = start + direction * along;
    let perp_dist = (point - on_line).length();

    perp_dist <= config.width / 2.0
}

/// Get the height of the bridge walkway at a given XZ position
pub fn get_bridge_height(
    x: f32,
    z: f32,
    start: Vec3,
    end: Vec3,
    config: &BridgeConfig,
) -> Option<f32> {
    let point = Vec3::new(x, 0.0, z);
    let span = end - start;
    let length = span.length();
    let direction = span.normalize_or_zero();
    let right = direction.cross(Vec3::Y).normalize_or_zero();
    let right = if right.length() < 0.1 { Vec3::X } else { right };

    // Project point onto bridge line (XZ plane)
    let start_xz = Vec3::new(start.x, 0.0, start.z);
    let dir_xz = Vec3::new(direction.x, 0.0, direction.z).normalize_or_zero();

    let to_point = point - start_xz;
    let along = to_point.dot(dir_xz);

    // Check if within bridge length
    let length_xz = Vec3::new(span.x, 0.0, span.z).length();
    if along < 0.0 || along > length_xz {
        return None;
    }

    // Check perpendicular distance
    let on_line = start_xz + dir_xz * along;
    let perp_dist = (point - on_line).length();

    if perp_dist > config.width / 2.0 {
        return None;
    }

    // Calculate bridge height at this position
    let t = along / length_xz;
    let sag_t = (t - 0.5).abs() * 2.0;
    let sag = (1.0 - sag_t * sag_t) * config.chain_sag * length * 0.3;

    let base_height = start.y + (end.y - start.y) * t;
    Some(base_height - sag)
}
