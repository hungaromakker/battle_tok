//! Procedural Tree Generation
//!
//! Generates trees on terrain using noise-based distribution.

use glam::Vec3;

use super::types::{Mesh, Vertex, fbm_noise};
use super::terrain::{terrain_height_at, terrain_normal_at, is_inside_hexagon, WATER_LEVEL};

/// Simple tree structure for harvesting
#[derive(Clone)]
pub struct PlacedTree {
    pub position: Vec3,
    pub height: f32,
    pub trunk_radius: f32,
    pub foliage_radius: f32,
    pub harvested: bool,
}

/// Generate procedural trees on terrain using noise-based distribution
pub fn generate_trees_on_terrain(center: Vec3, radius: f32, density: f32, seed_offset: f32) -> Vec<PlacedTree> {
    let mut trees = Vec::new();
    
    let spacing = 3.0;
    let grid_size = (radius * 2.0 / spacing) as i32;
    
    for gz in -grid_size..=grid_size {
        for gx in -grid_size..=grid_size {
            let base_x = center.x + (gx as f32) * spacing;
            let base_z = center.z + (gz as f32) * spacing;
            
            let jitter_x = fbm_noise((base_x + seed_offset) * 0.5, base_z * 0.5, 2) * spacing * 0.4;
            let jitter_z = fbm_noise(base_x * 0.5, (base_z + seed_offset) * 0.5, 2) * spacing * 0.4;
            let x = base_x + jitter_x;
            let z = base_z + jitter_z;
            
            let dx = x - center.x;
            let dz = z - center.z;
            if !is_inside_hexagon(dx, dz, radius * 0.85) {
                continue;
            }
            
            let tree_noise = fbm_noise((x + seed_offset * 2.0) * 0.1, z * 0.1, 3);
            if tree_noise < density {
                continue;
            }
            
            let terrain_y = terrain_height_at(x, z, center.y);
            let relative_height = terrain_y - center.y;
            if relative_height < WATER_LEVEL + 0.5 {
                continue;
            }
            
            let normal = terrain_normal_at(x, z, center.y);
            let slope = 1.0 - normal.y.abs();
            if slope > 0.5 {
                continue;
            }
            
            let size_noise = fbm_noise(x * 0.2, z * 0.2, 2);
            let height = 2.0 + size_noise * 3.0;
            let trunk_radius = 0.15 + size_noise * 0.1;
            let foliage_radius = 0.8 + size_noise * 0.6;
            
            trees.push(PlacedTree {
                position: Vec3::new(x, terrain_y, z),
                height,
                trunk_radius,
                foliage_radius,
                harvested: false,
            });
        }
    }
    
    trees
}

/// Generate mesh for a single tree (trunk + foliage cone)
pub fn generate_tree_mesh(tree: &PlacedTree) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    let trunk_color = [0.4, 0.25, 0.15, 1.0];
    let foliage_color = [0.2, 0.5, 0.2, 1.0];
    
    let pos = tree.position;
    let segments = 6;
    
    // Trunk
    let trunk_height = tree.height * 0.4;
    let trunk_base_idx = vertices.len() as u32;
    
    for i in 0..segments {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let nx = angle.cos();
        let nz = angle.sin();
        let x = pos.x + nx * tree.trunk_radius;
        let z = pos.z + nz * tree.trunk_radius;
        vertices.push(Vertex {
            position: [x, pos.y, z],
            normal: [nx, 0.0, nz],
            color: trunk_color,
        });
    }
    
    for i in 0..segments {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let nx = angle.cos();
        let nz = angle.sin();
        let x = pos.x + nx * tree.trunk_radius;
        let z = pos.z + nz * tree.trunk_radius;
        vertices.push(Vertex {
            position: [x, pos.y + trunk_height, z],
            normal: [nx, 0.0, nz],
            color: trunk_color,
        });
    }
    
    for i in 0..segments {
        let i0 = trunk_base_idx + i as u32;
        let i1 = trunk_base_idx + ((i + 1) % segments) as u32;
        let i2 = trunk_base_idx + segments as u32 + i as u32;
        let i3 = trunk_base_idx + segments as u32 + ((i + 1) % segments) as u32;
        
        indices.extend_from_slice(&[i0, i2, i1]);
        indices.extend_from_slice(&[i1, i2, i3]);
    }
    
    // Foliage
    let foliage_base_y = pos.y + trunk_height;
    let foliage_top_y = pos.y + tree.height;
    let foliage_base_idx = vertices.len() as u32;
    
    for i in 0..segments {
        let angle = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let nx = angle.cos();
        let nz = angle.sin();
        let x = pos.x + nx * tree.foliage_radius;
        let z = pos.z + nz * tree.foliage_radius;
        let normal = Vec3::new(nx, 0.5, nz).normalize();
        vertices.push(Vertex {
            position: [x, foliage_base_y, z],
            normal: [normal.x, normal.y, normal.z],
            color: foliage_color,
        });
    }
    
    let apex_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [pos.x, foliage_top_y, pos.z],
        normal: [0.0, 1.0, 0.0],
        color: foliage_color,
    });
    
    for i in 0..segments {
        let i0 = foliage_base_idx + i as u32;
        let i1 = foliage_base_idx + ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[i0, apex_idx, i1]);
    }
    
    let base_cap_center_idx = vertices.len() as u32;
    vertices.push(Vertex {
        position: [pos.x, foliage_base_y, pos.z],
        normal: [0.0, -1.0, 0.0],
        color: foliage_color,
    });
    
    for i in 0..segments {
        let i0 = foliage_base_idx + i as u32;
        let i1 = foliage_base_idx + ((i + 1) % segments) as u32;
        indices.extend_from_slice(&[i0, i1, base_cap_center_idx]);
    }
    
    Mesh { vertices, indices }
}

/// Generate combined mesh for all trees
pub fn generate_all_trees_mesh(trees: &[PlacedTree]) -> Mesh {
    let mut combined = Mesh::new();
    for tree in trees {
        if !tree.harvested {
            let tree_mesh = generate_tree_mesh(tree);
            combined.merge(&tree_mesh);
        }
    }
    combined
}
