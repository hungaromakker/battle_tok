//! Terrain Generation Functions
//!
//! Height, color, and normal sampling for procedural terrain.

use glam::Vec3;

use super::params::get_terrain_params;
use crate::game::types::{fbm_noise, ridged_noise, turbulent_noise};

/// Sample terrain height using UI-adjustable parameters.
///
/// `center_x` / `center_z` define the island center so that the
/// edge-falloff factor is computed relative to the island, not the
/// world origin.  Legacy callers that pass `(0.0, 0.0)` get the
/// old behaviour.
pub fn terrain_height_at_island(
    x: f32,
    z: f32,
    base_y: f32,
    center_x: f32,
    center_z: f32,
    island_radius: f32,
) -> f32 {
    let params = get_terrain_params();

    const MAX_MOUNTAIN: f32 = 8.0;
    const MAX_ROCK: f32 = 3.0;
    const MAX_HILL: f32 = 2.0;
    const MAX_DETAIL: f32 = 0.5;

    let dx = x - center_x;
    let dz = z - center_z;
    let dist_from_center = (dx * dx + dz * dz).sqrt() / island_radius;
    let edge_factor = (dist_from_center * 1.2).min(1.0);

    let mut height = 0.0;

    if params.mountains > 0.01 {
        let mountain_noise = ridged_noise(x * 0.04 + 100.0, z * 0.04 + 100.0, 4);
        height += mountain_noise * MAX_MOUNTAIN * params.mountains * edge_factor;
    }

    if params.rocks > 0.01 {
        let rock_noise = turbulent_noise(x * 0.1 + 50.0, z * 0.1 + 50.0, 3);
        height += rock_noise * MAX_ROCK * params.rocks;
    }

    if params.hills > 0.01 {
        let hill_noise = fbm_noise(x * 0.08, z * 0.08, 3);
        height += hill_noise * MAX_HILL * params.hills;
    }

    if params.detail > 0.01 {
        let detail_noise = fbm_noise(x * 0.3 + 200.0, z * 0.3 + 200.0, 2);
        height += detail_noise * MAX_DETAIL * params.detail;
    }

    height *= params.height_scale;

    base_y + height
}

/// Legacy wrapper — uses world origin as island center with radius 30.
pub fn terrain_height_at(x: f32, z: f32, base_y: f32) -> f32 {
    terrain_height_at_island(x, z, base_y, 0.0, 0.0, 30.0)
}

/// Apocalyptic terrain color with scorched/burnt palette
pub fn terrain_color_at(height: f32, normal: Vec3, base_y: f32) -> [f32; 4] {
    let params = get_terrain_params();
    let relative_height = height - base_y;
    let slope = 1.0 - normal.y.abs();

    let water_level = params.water * 2.0;

    // Earth/ground palette - solid, natural-looking terrain
    let grass_dark = [0.15, 0.18, 0.10, 1.0]; // Dark grass
    let grass_mid = [0.22, 0.25, 0.14, 1.0]; // Mid grass
    let grass_light = [0.28, 0.30, 0.18, 1.0]; // Lighter grass
    let grass_dry = [0.30, 0.25, 0.16, 1.0]; // Dry grass

    // Rock palette - solid earth tones
    let rock_dark = [0.18, 0.15, 0.13, 1.0]; // Dark rock
    let rock_mid = [0.32, 0.28, 0.24, 1.0]; // Mid rock
    let rock_light = [0.42, 0.38, 0.34, 1.0]; // Lighter rock
    let rock_moss = [0.20, 0.24, 0.16, 1.0]; // Moss on rock

    // Dirt and earth palette
    let sand_wet = [0.28, 0.22, 0.16, 1.0]; // Wet earth
    let sand_dry = [0.40, 0.35, 0.28, 1.0]; // Dry earth
    let dirt_base = [0.25, 0.20, 0.15, 1.0]; // Base earth (föld)
    let mud_wet = [0.18, 0.14, 0.10, 1.0]; // Dark mud

    // Lava pools (only visible if water param > 0)
    let water_shallow = [1.8, 0.5, 0.1, 0.90]; // Bright lava surface (HDR)
    let water_deep = [0.8, 0.2, 0.05, 0.95]; // Cooler lava crust

    let px = (height * 7.3 + relative_height * 13.7).sin() * 0.5 + 0.5;
    let py = (relative_height * 11.1 + slope * 17.3).cos() * 0.5 + 0.5;
    let noise = (px + py) * 0.5;

    if relative_height < water_level && params.water > 0.01 {
        let depth = (water_level - relative_height) / 2.0;
        let water_blend = depth.clamp(0.0, 1.0);
        return blend_colors(&water_shallow, &water_deep, water_blend);
    }

    let beach_width = 0.8;
    let beach_zone = ((relative_height - water_level) / beach_width).clamp(0.0, 1.0);
    if beach_zone < 1.0 && params.water > 0.01 {
        let sand = blend_colors(&sand_wet, &sand_dry, beach_zone);
        let mud_factor = (1.0 - beach_zone) * 0.4;
        return blend_colors(&sand, &mud_wet, mud_factor);
    }

    let height_factor = ((relative_height - water_level) / 8.0).clamp(0.0, 1.0);
    let slope_sharp = smooth_step(0.35, 0.65, slope);

    let grass_variation = noise;
    let mut grass = blend_colors(&grass_dark, &grass_mid, grass_variation);
    grass = blend_colors(&grass, &grass_light, (noise * 0.7).clamp(0.0, 1.0));

    let dry_factor =
        (height_factor * 0.6 + slope * 0.3 + (noise - 0.5).abs() * 0.4).clamp(0.0, 1.0);
    grass = blend_colors(&grass, &grass_dry, dry_factor * 0.5);

    let mut rock = blend_colors(&rock_dark, &rock_mid, noise);
    rock = blend_colors(&rock, &rock_light, ((noise - 0.3) * 2.0).clamp(0.0, 1.0));

    let north_facing = (-normal.z * 0.5 + 0.5).clamp(0.0, 1.0);
    let moisture = (1.0 - height_factor) * (1.0 - slope);
    let moss_factor = north_facing * moisture * 0.6;
    rock = blend_colors(&rock, &rock_moss, moss_factor);

    let dirt = blend_colors(&dirt_base, &sand_dry, noise * 0.3);

    let mut result = blend_colors(&grass, &rock, slope_sharp);

    let altitude_rock = (height_factor * 1.5).clamp(0.0, 1.0);
    result = blend_colors(&result, &rock, altitude_rock * (1.0 - slope_sharp) * 0.5);

    let altitude_dirt = ((height_factor - 0.6) * 2.5).clamp(0.0, 1.0);
    result = blend_colors(&result, &dirt, altitude_dirt * 0.4);

    result
}

/// Smooth step function for natural transitions
pub fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Helper to blend two colors
pub fn blend_colors(a: &[f32; 4], b: &[f32; 4], t: f32) -> [f32; 4] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
        a[3] + (b[3] - a[3]) * t,
    ]
}

/// Compute normal from terrain height gradient (island-aware).
pub fn terrain_normal_at_island(
    x: f32,
    z: f32,
    base_y: f32,
    cx: f32,
    cz: f32,
    radius: f32,
) -> Vec3 {
    let epsilon = 0.2;
    let h_center = terrain_height_at_island(x, z, base_y, cx, cz, radius);
    let h_dx = terrain_height_at_island(x + epsilon, z, base_y, cx, cz, radius);
    let h_dz = terrain_height_at_island(x, z + epsilon, base_y, cx, cz, radius);

    let tangent_x = Vec3::new(epsilon, h_dx - h_center, 0.0);
    let tangent_z = Vec3::new(0.0, h_dz - h_center, epsilon);

    tangent_z.cross(tangent_x).normalize()
}

/// Compute normal from terrain height gradient (legacy wrapper).
pub fn terrain_normal_at(x: f32, z: f32, base_y: f32) -> Vec3 {
    let epsilon = 0.2;
    let h_center = terrain_height_at(x, z, base_y);
    let h_dx = terrain_height_at(x + epsilon, z, base_y);
    let h_dz = terrain_height_at(x, z + epsilon, base_y);

    let tangent_x = Vec3::new(epsilon, h_dx - h_center, 0.0);
    let tangent_z = Vec3::new(0.0, h_dz - h_center, epsilon);

    tangent_z.cross(tangent_x).normalize()
}

/// Check if point (dx, dz) is inside a regular pointy-top hexagon
///
/// For a pointy-top hexagon with circumradius (center-to-vertex distance) = radius:
/// - Vertices are at angles 0°, 60°, 120°, 180°, 240°, 300° from center
/// - The apothem (center-to-edge) = radius * cos(30°) = radius * sqrt(3)/2
///
/// Uses 6-fold symmetry: work with absolute values and check 2 constraints:
/// 1. |z| <= radius (top/bottom vertex constraint)
/// 2. sqrt(3)*|x| + |z| <= sqrt(3)*radius (diagonal edge constraint)
pub fn is_inside_hexagon(dx: f32, dz: f32, radius: f32) -> bool {
    let ax = dx.abs();
    let az = dz.abs();
    const SQRT3: f32 = 1.732050808;

    // Pointy-top hexagon boundary test
    az <= radius && (SQRT3 * ax + az) <= SQRT3 * radius
}
