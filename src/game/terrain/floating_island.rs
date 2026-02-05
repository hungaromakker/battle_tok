//! Floating Island Generation
//!
//! Generates floating island terrain meshes with visible layered earth crust:
//! - Top layer: Grass/scorched terrain
//! - Middle layers: Earth (föld), rocky sections (köves rész) with rare items
//! - Bottom: Molten lava underneath showing through cracks
//!
//! Creates the apocalyptic battle arena setting with islands floating over a lava sea.

use glam::Vec3;

use super::generation::{
    is_inside_hexagon, terrain_color_at, terrain_height_at_island, terrain_normal_at_island,
};
use crate::game::types::{Mesh, Vertex};

/// Layer types for the island crust
#[derive(Clone, Copy, Debug)]
pub enum IslandLayer {
    /// Top terrain (grass/rock)
    Surface,
    /// Earth/soil layer (föld)
    Earth,
    /// Rocky layer with stones (köves rész)
    Rock,
    /// Rare ore layer (ritka tárgyak)
    Ore,
    /// Molten core (láva)
    MoltenCore,
}

/// Configuration for floating island generation
#[derive(Clone, Copy, Debug)]
pub struct FloatingIslandConfig {
    /// Radius of the island
    pub radius: f32,
    /// Height of the top surface above center
    pub surface_height: f32,
    /// Total depth below surface (how thick the island is)
    pub island_thickness: f32,
    /// How much the bottom tapers inward (0.0 = cylinder, 1.0 = cone point)
    pub taper_amount: f32,
    /// Number of layer bands visible on the edge
    pub num_layers: u32,
    /// Noise amplitude for organic edge shape
    pub edge_noise: f32,
}

impl Default for FloatingIslandConfig {
    fn default() -> Self {
        Self {
            radius: 40.0,
            surface_height: 5.0,
            island_thickness: 25.0,
            taper_amount: 0.6,
            num_layers: 5,
            edge_noise: 3.0,
        }
    }
}

/// Colors for each layer - rich earth tones with visible layers (föld)
fn layer_color(layer: IslandLayer) -> [f32; 4] {
    match layer {
        IslandLayer::Surface => [0.22, 0.25, 0.15, 1.0], // Grass/topsoil
        IslandLayer::Earth => [0.35, 0.25, 0.18, 1.0],   // Brown earth (föld)
        IslandLayer::Rock => [0.42, 0.38, 0.35, 1.0],    // Gray-brown rock (köves rész)
        IslandLayer::Ore => [0.50, 0.42, 0.22, 1.0],     // Golden ore hints (ritka tárgyak)
        IslandLayer::MoltenCore => [2.5, 0.7, 0.12, 1.0], // Molten lava (láva)
    }
}

/// Simple 3D noise for edge variation
fn noise3d(p: Vec3) -> f32 {
    let p = p * 0.5;
    let i = p.floor();
    let f = p - i;

    // Smooth interpolation
    let u = f * f * (Vec3::splat(3.0) - f * 2.0);

    // Hash corners
    fn hash(p: Vec3) -> f32 {
        let p = Vec3::new(
            p.x.sin() * 43758.5453,
            p.y.sin() * 12345.6789,
            p.z.sin() * 98765.4321,
        );
        (p.x + p.y + p.z).fract()
    }

    // Trilinear interpolation
    let n000 = hash(i);
    let n100 = hash(i + Vec3::new(1.0, 0.0, 0.0));
    let n010 = hash(i + Vec3::new(0.0, 1.0, 0.0));
    let n110 = hash(i + Vec3::new(1.0, 1.0, 0.0));
    let n001 = hash(i + Vec3::new(0.0, 0.0, 1.0));
    let n101 = hash(i + Vec3::new(1.0, 0.0, 1.0));
    let n011 = hash(i + Vec3::new(0.0, 1.0, 1.0));
    let n111 = hash(i + Vec3::ONE);

    let nx00 = n000 * (1.0 - u.x) + n100 * u.x;
    let nx10 = n010 * (1.0 - u.x) + n110 * u.x;
    let nx01 = n001 * (1.0 - u.x) + n101 * u.x;
    let nx11 = n011 * (1.0 - u.x) + n111 * u.x;

    let nxy0 = nx00 * (1.0 - u.y) + nx10 * u.y;
    let nxy1 = nx01 * (1.0 - u.y) + nx11 * u.y;

    nxy0 * (1.0 - u.z) + nxy1 * u.z
}

/// FBM noise for organic shapes
fn fbm(p: Vec3, octaves: i32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 0.5;
    let mut pos = p;

    for _ in 0..octaves {
        value += amplitude * noise3d(pos);
        pos = pos * 2.0;
        amplitude *= 0.5;
    }

    value
}

/// Generate a complete floating island with layered crust visible from below
pub fn generate_floating_island(center: Vec3, config: FloatingIslandConfig) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // ========================================
    // TOP SURFACE (using existing terrain generation)
    // ========================================
    let top_mesh = generate_island_surface(center, config);
    let surface_vertex_offset = vertices.len() as u32;
    vertices.extend(top_mesh.vertices);
    for idx in top_mesh.indices {
        indices.push(idx + surface_vertex_offset);
    }

    // ========================================
    // SIDE WALLS (layered crust)
    // ========================================
    let wall_mesh = generate_island_walls(center, config);
    let wall_vertex_offset = vertices.len() as u32;
    vertices.extend(wall_mesh.vertices);
    for idx in wall_mesh.indices {
        indices.push(idx + wall_vertex_offset);
    }

    // ========================================
    // BOTTOM (molten core visible)
    // ========================================
    let bottom_mesh = generate_island_bottom(center, config);
    let bottom_vertex_offset = vertices.len() as u32;
    vertices.extend(bottom_mesh.vertices);
    for idx in bottom_mesh.indices {
        indices.push(idx + bottom_vertex_offset);
    }

    Mesh { vertices, indices }
}

/// Generate the top surface with terrain features
fn generate_island_surface(center: Vec3, config: FloatingIslandConfig) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let subdivisions = 64u32;
    let grid_count = subdivisions + 1;
    let cell_size = (config.radius * 2.0) / (subdivisions as f32);
    let half_size = config.radius;
    let base_y = center.y + config.surface_height;

    for gz in 0..grid_count {
        for gx in 0..grid_count {
            let local_x = (gx as f32) * cell_size - half_size;
            let local_z = (gz as f32) * cell_size - half_size;
            let x = center.x + local_x;
            let z = center.z + local_z;

            let y = terrain_height_at_island(x, z, base_y, center.x, center.z, config.radius);
            let normal = terrain_normal_at_island(x, z, base_y, center.x, center.z, config.radius);
            let color = terrain_color_at(y, normal, base_y);

            vertices.push(Vertex {
                position: [x, y, z],
                normal: normal.to_array(),
                color,
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

            if is_inside_hexagon(dx, dz, config.radius) {
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

/// Generate the layered side walls of the floating island with jagged rocky cliffs
fn generate_island_walls(center: Vec3, config: FloatingIslandConfig) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let segments = 64u32; // More segments for jagged detail
    let layers = (config.num_layers * 3).max(12); // More layers for rocky detail
    let top_y = center.y + config.surface_height;
    let _bottom_y = top_y - config.island_thickness;
    let _layer_height = config.island_thickness / (layers as f32);

    // Generate vertices for each layer ring - with jagged rocky edges
    for layer in 0..=layers {
        let t = layer as f32 / layers as f32; // 0 = top, 1 = bottom
        let base_y = top_y - t * config.island_thickness;

        // Taper radius toward bottom
        let taper = 1.0 - t * config.taper_amount;
        let layer_radius = config.radius * taper;

        // Determine layer type and color with more variation
        let layer_type = if t < 0.08 {
            IslandLayer::Surface
        } else if t < 0.25 {
            IslandLayer::Earth
        } else if t < 0.55 {
            IslandLayer::Rock
        } else if t < 0.80 {
            IslandLayer::Ore
        } else {
            IslandLayer::MoltenCore
        };
        let base_color = layer_color(layer_type);

        for seg in 0..segments {
            let angle = (seg as f32) * std::f32::consts::TAU / (segments as f32);

            // Hexagonal shape with AGGRESSIVE noise variation for jagged look
            let hex_factor = hexagon_radius_factor(angle);

            // Multiple noise octaves for jagged cliff appearance
            let noise1 =
                fbm(Vec3::new(angle * 5.0, base_y * 0.3, t * 8.0), 4) * config.edge_noise * 1.5;
            let noise2 = fbm(Vec3::new(angle * 12.0 + 100.0, base_y * 0.5, t * 15.0), 3)
                * config.edge_noise
                * 0.8;
            let noise3 = noise3d(Vec3::new(angle * 25.0, base_y * 0.8, seg as f32 * 0.3))
                * config.edge_noise
                * 0.5;

            // Sharp cliff edges - random outcrops
            let outcrop = if (seg as f32 * 7.3 + t * 11.0).sin() > 0.7 {
                config.edge_noise * 2.0 * fbm(Vec3::new(angle * 8.0, t * 12.0, 0.0), 2)
            } else {
                0.0
            };

            let total_noise = noise1 + noise2 + noise3 + outcrop;
            let r = layer_radius * hex_factor + total_noise;

            // Add vertical jaggedness - each vertex can be offset up/down
            let y_jag = fbm(Vec3::new(angle * 8.0, t * 10.0, seg as f32 * 0.2), 2) * 1.5;
            let y = base_y + y_jag;

            let x = center.x + r * angle.cos();
            let z = center.z + r * angle.sin();

            // Normal points outward with variation
            let nx = angle.cos() + fbm(Vec3::new(x * 0.2, y * 0.2, z * 0.2), 2) * 0.3;
            let nz = angle.sin() + fbm(Vec3::new(x * 0.2 + 50.0, y * 0.2, z * 0.2), 2) * 0.3;
            let ny = config.taper_amount * 0.3 + fbm(Vec3::new(x * 0.1, y * 0.3, z * 0.1), 2) * 0.2;
            let normal = Vec3::new(nx, ny, nz).normalize();

            // Add variation to color based on noise - more dramatic
            let color_var = fbm(Vec3::new(x * 0.15, y * 0.15, z * 0.15), 3) * 0.3;
            let dark_var = noise3d(Vec3::new(x * 0.4, y * 0.4, z * 0.4)) * 0.15;

            // Darker recesses, lighter outcrops
            let outcrop_light = if total_noise > config.edge_noise {
                0.1
            } else {
                -0.05
            };

            let color = [
                (base_color[0] + color_var - dark_var + outcrop_light).clamp(0.0, 3.0),
                (base_color[1] + color_var * 0.5 - dark_var * 0.5 + outcrop_light * 0.5)
                    .clamp(0.0, 2.0),
                (base_color[2] + color_var * 0.3 - dark_var * 0.3 + outcrop_light * 0.3)
                    .clamp(0.0, 1.5),
                base_color[3],
            ];

            vertices.push(Vertex {
                position: [x, y, z],
                normal: normal.to_array(),
                color,
            });
        }
    }

    // Generate indices for wall quads
    for layer in 0..layers {
        for seg in 0..segments {
            let next_seg = (seg + 1) % segments;

            let i00 = layer * segments + seg;
            let i10 = layer * segments + next_seg;
            let i01 = (layer + 1) * segments + seg;
            let i11 = (layer + 1) * segments + next_seg;

            // Two triangles per quad
            indices.push(i00);
            indices.push(i01);
            indices.push(i10);

            indices.push(i10);
            indices.push(i01);
            indices.push(i11);
        }
    }

    Mesh { vertices, indices }
}

/// Generate the bottom of the island (molten core visible)
fn generate_island_bottom(center: Vec3, config: FloatingIslandConfig) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let bottom_y = center.y + config.surface_height - config.island_thickness;
    let bottom_radius = config.radius * (1.0 - config.taper_amount);

    // Molten core colors (HDR emissive)
    let core_color = [3.0, 0.8, 0.15, 1.0];
    let dark_color = [0.8, 0.2, 0.05, 1.0];

    // Center vertex
    vertices.push(Vertex {
        position: [center.x, bottom_y, center.z],
        normal: [0.0, -1.0, 0.0],
        color: core_color,
    });

    // Ring vertices with crack pattern
    let segments = 32u32;
    for seg in 0..segments {
        let angle = (seg as f32) * std::f32::consts::TAU / (segments as f32);
        let hex_factor = hexagon_radius_factor(angle);

        // Add noise for organic shape
        let noise = fbm(Vec3::new(angle * 5.0, bottom_y * 0.1, 0.0), 2) * 2.0;
        let r = bottom_radius * hex_factor + noise;

        let x = center.x + r * angle.cos();
        let z = center.z + r * angle.sin();

        // Alternate colors for crack effect
        let color = if seg % 3 == 0 { core_color } else { dark_color };

        vertices.push(Vertex {
            position: [x, bottom_y, z],
            normal: [0.0, -1.0, 0.0],
            color,
        });
    }

    // Generate fan triangles
    for seg in 0..segments {
        let next = (seg + 1) % segments;
        indices.push(0); // Center
        indices.push(next + 1); // Next outer
        indices.push(seg + 1); // Current outer (reversed winding for bottom)
    }

    Mesh { vertices, indices }
}

/// Calculate radius factor for hexagonal shape (1.0 at flat edge, slightly more at corners)
fn hexagon_radius_factor(angle: f32) -> f32 {
    // Hexagon has 6-fold symmetry
    let angle_in_sector = (angle * 3.0 / std::f32::consts::PI).rem_euclid(1.0);
    let corner_dist = (angle_in_sector - 0.5).abs();

    // Flat edges at 1.0, corners push out slightly
    1.0 + corner_dist * 0.15
}

/// Generate a large lava ocean plane that surrounds all islands
pub fn generate_lava_ocean(world_size: f32, ocean_level: f32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Lava colors (HDR emissive) - glowing but not blinding
    let lava_bright = [2.5, 0.7, 0.12, 1.0]; // Bright lava glow
    let lava_dark = [0.5, 0.10, 0.03, 1.0]; // Dark crust areas
    let normal = [0.0, 1.0, 0.0];

    let subdivisions = 64u32;
    let grid_count = subdivisions + 1;
    let cell_size = (world_size * 2.0) / (subdivisions as f32);
    let half_size = world_size;

    for gz in 0..grid_count {
        for gx in 0..grid_count {
            let x = (gx as f32) * cell_size - half_size;
            let z = (gz as f32) * cell_size - half_size;

            // Vary color with noise for flow patterns
            let noise_val = fbm(Vec3::new(x * 0.02, 0.0, z * 0.02), 3);
            let color = if noise_val > 0.5 {
                lava_bright
            } else {
                let t = noise_val * 2.0;
                [
                    lava_dark[0] + t * (lava_bright[0] - lava_dark[0]),
                    lava_dark[1] + t * (lava_bright[1] - lava_dark[1]),
                    lava_dark[2] + t * (lava_bright[2] - lava_dark[2]),
                    1.0,
                ]
            };

            vertices.push(Vertex {
                position: [x, ocean_level, z],
                normal,
                color,
            });
        }
    }

    for gz in 0..subdivisions {
        for gx in 0..subdivisions {
            let i00 = gz * grid_count + gx;
            let i10 = gz * grid_count + (gx + 1);
            let i01 = (gz + 1) * grid_count + gx;
            let i11 = (gz + 1) * grid_count + (gx + 1);

            indices.push(i00);
            indices.push(i01);
            indices.push(i10);

            indices.push(i10);
            indices.push(i01);
            indices.push(i11);
        }
    }

    Mesh { vertices, indices }
}
