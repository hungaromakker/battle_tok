//! Hex Terrain Generation
//!
//! Generates hexagonal terrain meshes with procedural features.

use glam::Vec3;

use super::generation::{
    is_inside_hexagon, terrain_color_at, terrain_height_at, terrain_normal_at,
};
use super::params::WATER_LEVEL;
use crate::game::types::{Mesh, Vertex};

/// Generate an elevated hexagonal terrain with procedural mountains, rocks, and water
pub fn generate_elevated_hex_terrain(
    center: Vec3,
    radius: f32,
    _color: [f32; 4],
    subdivisions: u32,
) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let actual_subdivisions = subdivisions.max(64);
    let grid_count = actual_subdivisions + 1;
    let cell_size = (radius * 2.0) / (actual_subdivisions as f32);
    let half_size = radius;

    for gz in 0..grid_count {
        for gx in 0..grid_count {
            let local_x = (gx as f32) * cell_size - half_size;
            let local_z = (gz as f32) * cell_size - half_size;
            let x = center.x + local_x;
            let z = center.z + local_z;

            let y = terrain_height_at(x, z, center.y);
            let normal = terrain_normal_at(x, z, center.y);
            let color = terrain_color_at(y, normal, center.y);

            vertices.push(Vertex {
                position: [x, y, z],
                normal: normal.to_array(),
                color,
            });
        }
    }

    for gz in 0..actual_subdivisions {
        for gx in 0..actual_subdivisions {
            let i00 = gz * grid_count + gx;
            let i10 = gz * grid_count + (gx + 1);
            let i01 = (gz + 1) * grid_count + gx;
            let i11 = (gz + 1) * grid_count + (gx + 1);

            let cx = center.x + ((gx as f32 + 0.5) * cell_size - half_size);
            let cz = center.z + ((gz as f32 + 0.5) * cell_size - half_size);
            let dx = cx - center.x;
            let dz = cz - center.z;

            if is_inside_hexagon(dx, dz, radius) {
                indices.push(i00);
                indices.push(i01);
                indices.push(i10);

                indices.push(i10);
                indices.push(i01);
                indices.push(i11);
            }
        }
    }

    Mesh { vertices, indices }
}

/// Generate a flat lava plane (replaces water for battle arena)
/// Uses emissive orange-red colors that work with the HDR lava shader
pub fn generate_lava_plane(center: Vec3, radius: f32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Bright molten lava color (HDR emissive values)
    let lava_color = [1.8, 0.5, 0.1, 1.0]; // HDR orange-red
    let lava_y = center.y + WATER_LEVEL - 0.3; // Slightly below water level for depth
    let normal = [0.0, 1.0, 0.0];

    let subdivisions = 32u32;
    let grid_count = subdivisions + 1;
    let cell_size = (radius * 2.0) / (subdivisions as f32);
    let half_size = radius;

    for gz in 0..grid_count {
        for gx in 0..grid_count {
            let local_x = (gx as f32) * cell_size - half_size;
            let local_z = (gz as f32) * cell_size - half_size;
            let x = center.x + local_x;
            let z = center.z + local_z;

            vertices.push(Vertex {
                position: [x, lava_y, z],
                normal,
                color: lava_color,
            });
        }
    }

    for gz in 0..subdivisions {
        for gx in 0..subdivisions {
            let i00 = gz * grid_count + gx;
            let i10 = gz * grid_count + (gx + 1);
            let i01 = (gz + 1) * grid_count + gx;
            let i11 = (gz + 1) * grid_count + (gx + 1);

            let cx = center.x + ((gx as f32 + 0.5) * cell_size - half_size);
            let cz = center.z + ((gz as f32 + 0.5) * cell_size - half_size);
            let dx = cx - center.x;
            let dz = cz - center.z;

            if is_inside_hexagon(dx, dz, radius * 0.95) {
                indices.push(i00);
                indices.push(i01);
                indices.push(i10);
                indices.push(i10);
                indices.push(i01);
                indices.push(i11);
            }
        }
    }

    Mesh { vertices, indices }
}

/// Generate a flat water plane at the water level
pub fn generate_water_plane(center: Vec3, radius: f32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let water_color = [0.3, 0.65, 0.6, 0.85];
    let water_y = center.y + WATER_LEVEL;
    let normal = [0.0, 1.0, 0.0];

    let subdivisions = 32u32;
    let grid_count = subdivisions + 1;
    let cell_size = (radius * 2.0) / (subdivisions as f32);
    let half_size = radius;

    for gz in 0..grid_count {
        for gx in 0..grid_count {
            let local_x = (gx as f32) * cell_size - half_size;
            let local_z = (gz as f32) * cell_size - half_size;
            let x = center.x + local_x;
            let z = center.z + local_z;

            vertices.push(Vertex {
                position: [x, water_y, z],
                normal,
                color: water_color,
            });
        }
    }

    for gz in 0..subdivisions {
        for gx in 0..subdivisions {
            let i00 = gz * grid_count + gx;
            let i10 = gz * grid_count + (gx + 1);
            let i01 = (gz + 1) * grid_count + gx;
            let i11 = (gz + 1) * grid_count + (gx + 1);

            let cx = center.x + ((gx as f32 + 0.5) * cell_size - half_size);
            let cz = center.z + ((gz as f32 + 0.5) * cell_size - half_size);
            let dx = cx - center.x;
            let dz = cz - center.z;

            if is_inside_hexagon(dx, dz, radius * 0.95) {
                indices.push(i00);
                indices.push(i01);
                indices.push(i10);
                indices.push(i10);
                indices.push(i01);
                indices.push(i11);
            }
        }
    }

    Mesh { vertices, indices }
}

/// Generate a flat hexagonal platform mesh
pub fn generate_hex_platform(center: Vec3, radius: f32, color: [f32; 4]) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    vertices.push(Vertex {
        position: [center.x, center.y, center.z],
        normal: [0.0, 1.0, 0.0],
        color,
    });

    for i in 0..6 {
        let angle = (i as f32) * std::f32::consts::PI / 3.0;
        let x = center.x + radius * angle.cos();
        let z = center.z + radius * angle.sin();
        vertices.push(Vertex {
            position: [x, center.y, z],
            normal: [0.0, 1.0, 0.0],
            color,
        });
    }

    for i in 0..6 {
        let next = (i + 1) % 6;
        indices.push(0);
        indices.push(i + 1);
        indices.push(next + 1);
    }

    Mesh { vertices, indices }
}
