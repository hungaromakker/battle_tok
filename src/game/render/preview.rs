//! Preview Mesh Generation
//!
//! Mesh generation for builder mode previews and grid overlays.

use crate::game::types::{Mesh, Vertex};
use glam::Vec3;

/// Generate a hex grid overlay mesh for builder mode
///
/// Shows hex grid cells around a center coordinate position.
///
/// # Arguments
/// * `center_world_pos` - World position of the center cell
/// * `center_coord` - Axial coordinate (q, r, level) of the center
/// * `grid_radius` - Number of cells to show around center
/// * `hex_radius` - Radius of each hex cell
/// * `line_thickness` - Thickness of grid lines
/// * `terrain_height_fn` - Function to get terrain height at (x, z)
/// * `axial_to_world_fn` - Function to convert (q, r, level) to world position
///
/// # Returns
/// Mesh containing the grid overlay
pub fn generate_hex_grid_overlay<F, G>(
    center_coord: (i32, i32, i32),
    grid_radius: i32,
    hex_radius: f32,
    line_thickness: f32,
    terrain_height_fn: F,
    axial_to_world_fn: G,
) -> Option<Mesh>
where
    F: Fn(f32, f32) -> f32,
    G: Fn(i32, i32, i32) -> Vec3,
{
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Generate hex outlines around cursor
    for dq in -grid_radius..=grid_radius {
        for dr in -grid_radius..=grid_radius {
            // Hex distance check (axial coordinates)
            let ds = -dq - dr;
            let hex_dist = (dq.abs() + dr.abs() + ds.abs()) / 2;
            if hex_dist > grid_radius {
                continue;
            }

            let q = center_coord.0 + dq;
            let r = center_coord.1 + dr;
            let world_pos = axial_to_world_fn(q, r, center_coord.2);

            // Color - highlight cursor cell differently
            let is_cursor = dq == 0 && dr == 0;
            let cell_color = if is_cursor {
                [0.2, 1.0, 0.4, 0.9] // Bright green for cursor
            } else {
                [0.3, 0.7, 1.0, 0.4] // Cyan for others
            };

            // Generate 6 edge quads for the hex outline
            for i in 0..6 {
                let angle1 = (i as f32) * std::f32::consts::PI / 3.0;
                let angle2 = ((i + 1) % 6) as f32 * std::f32::consts::PI / 3.0;

                // Hex vertices (pointy-top orientation)
                let x1 = world_pos.x + hex_radius * angle1.sin();
                let z1 = world_pos.z + hex_radius * angle1.cos();
                let x2 = world_pos.x + hex_radius * angle2.sin();
                let z2 = world_pos.z + hex_radius * angle2.cos();

                // Sample terrain height + small offset above terrain
                let y1 = terrain_height_fn(x1, z1) + 0.1;
                let y2 = terrain_height_fn(x2, z2) + 0.1;

                // Calculate perpendicular offset for line thickness
                let edge_dx = x2 - x1;
                let edge_dz = z2 - z1;
                let edge_len = (edge_dx * edge_dx + edge_dz * edge_dz).sqrt();
                if edge_len < 0.001 {
                    continue;
                }

                let perp_x = -edge_dz / edge_len * line_thickness;
                let perp_z = edge_dx / edge_len * line_thickness;

                // Create quad for this edge (4 vertices)
                let base_idx = vertices.len() as u32;

                vertices.push(Vertex {
                    position: [x1 - perp_x, y1, z1 - perp_z],
                    normal: [0.0, 1.0, 0.0],
                    color: cell_color,
                });
                vertices.push(Vertex {
                    position: [x1 + perp_x, y1, z1 + perp_z],
                    normal: [0.0, 1.0, 0.0],
                    color: cell_color,
                });
                vertices.push(Vertex {
                    position: [x2 + perp_x, y2, z2 + perp_z],
                    normal: [0.0, 1.0, 0.0],
                    color: cell_color,
                });
                vertices.push(Vertex {
                    position: [x2 - perp_x, y2, z2 - perp_z],
                    normal: [0.0, 1.0, 0.0],
                    color: cell_color,
                });

                // Two triangles per quad
                indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2]);
                indices.extend_from_slice(&[base_idx, base_idx + 2, base_idx + 3]);
            }
        }
    }

    if vertices.is_empty() {
        None
    } else {
        Some(Mesh { vertices, indices })
    }
}

/// Generate a pulsing ghost color for preview meshes
///
/// # Arguments
/// * `time` - Current time in seconds
/// * `base_color` - Base RGB color (without alpha)
///
/// # Returns
/// RGBA color array with pulsing alpha
pub fn calculate_ghost_color(time: f32, base_color: [f32; 3]) -> [f32; 4] {
    let pulse = 0.5 + (time * 4.0).sin() * 0.3;
    [base_color[0], base_color[1], base_color[2], pulse]
}

/// Default ghost preview color (green)
pub const GHOST_PREVIEW_COLOR: [f32; 3] = [0.3, 0.9, 0.4];

/// Convert hex prism vertices to our Vertex format with a custom color
///
/// # Arguments
/// * `hex_vertices` - Input vertices from hex prism mesh generation
/// * `color` - Color to apply to all vertices
///
/// # Returns
/// Vec of Vertex with the specified color
pub fn convert_hex_vertices_with_color(
    hex_vertices: &[(Vec3, Vec3)], // (position, normal)
    color: [f32; 4],
) -> Vec<Vertex> {
    hex_vertices
        .iter()
        .map(|(pos, norm)| Vertex {
            position: [pos.x, pos.y, pos.z],
            normal: [norm.x, norm.y, norm.z],
            color,
        })
        .collect()
}

/// Generate block preview mesh for the build toolbar
///
/// # Arguments
/// * `position` - World position for the preview
/// * `shape_index` - Index of the selected shape
/// * `time` - Current time for animation
///
/// # Returns
/// Optional preview mesh
pub fn generate_block_preview_mesh(position: Vec3, time: f32) -> Mesh {
    let ghost_color = calculate_ghost_color(time, GHOST_PREVIEW_COLOR);

    // Generate a simple cube preview
    let half_size = 0.5;
    let min = position - Vec3::splat(half_size);
    let max = position + Vec3::splat(half_size);

    // Generate cube vertices (8 corners, but we need separate vertices for each face for normals)
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Front face (+Z)
    let base = vertices.len() as u32;
    vertices.push(Vertex {
        position: [min.x, min.y, max.z],
        normal: [0.0, 0.0, 1.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, min.y, max.z],
        normal: [0.0, 0.0, 1.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, max.y, max.z],
        normal: [0.0, 0.0, 1.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [min.x, max.y, max.z],
        normal: [0.0, 0.0, 1.0],
        color: ghost_color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

    // Back face (-Z)
    let base = vertices.len() as u32;
    vertices.push(Vertex {
        position: [max.x, min.y, min.z],
        normal: [0.0, 0.0, -1.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [min.x, min.y, min.z],
        normal: [0.0, 0.0, -1.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [min.x, max.y, min.z],
        normal: [0.0, 0.0, -1.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, max.y, min.z],
        normal: [0.0, 0.0, -1.0],
        color: ghost_color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

    // Top face (+Y)
    let base = vertices.len() as u32;
    vertices.push(Vertex {
        position: [min.x, max.y, min.z],
        normal: [0.0, 1.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [min.x, max.y, max.z],
        normal: [0.0, 1.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, max.y, max.z],
        normal: [0.0, 1.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, max.y, min.z],
        normal: [0.0, 1.0, 0.0],
        color: ghost_color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

    // Bottom face (-Y)
    let base = vertices.len() as u32;
    vertices.push(Vertex {
        position: [min.x, min.y, max.z],
        normal: [0.0, -1.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [min.x, min.y, min.z],
        normal: [0.0, -1.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, min.y, min.z],
        normal: [0.0, -1.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, min.y, max.z],
        normal: [0.0, -1.0, 0.0],
        color: ghost_color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

    // Right face (+X)
    let base = vertices.len() as u32;
    vertices.push(Vertex {
        position: [max.x, min.y, max.z],
        normal: [1.0, 0.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, min.y, min.z],
        normal: [1.0, 0.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, max.y, min.z],
        normal: [1.0, 0.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [max.x, max.y, max.z],
        normal: [1.0, 0.0, 0.0],
        color: ghost_color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

    // Left face (-X)
    let base = vertices.len() as u32;
    vertices.push(Vertex {
        position: [min.x, min.y, min.z],
        normal: [-1.0, 0.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [min.x, min.y, max.z],
        normal: [-1.0, 0.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [min.x, max.y, max.z],
        normal: [-1.0, 0.0, 0.0],
        color: ghost_color,
    });
    vertices.push(Vertex {
        position: [min.x, max.y, min.z],
        normal: [-1.0, 0.0, 0.0],
        color: ghost_color,
    });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);

    Mesh { vertices, indices }
}
