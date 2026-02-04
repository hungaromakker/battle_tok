// SDF Demo Shader - Interactive SDF Techniques
// Place objects and push them together to see smooth blending, CSG, and deformations!

// =============================================================================
// UNIFORMS - Must match Rust DemoUniforms exactly (80 bytes)
// =============================================================================

struct DemoUniforms {
    camera_pos: vec3<f32>,      // 0: 12 bytes
    time: f32,                   // 12: 4 bytes
    resolution: vec2<f32>,       // 16: 8 bytes
    demo_mode: u32,              // 24: 4 bytes (0=interactive, 1-7=demos)
    blend_mode: u32,             // 28: 4 bytes
    camera_target: vec3<f32>,    // 32: 12 bytes
    blend_k: f32,                // 44: 4 bytes
    twist_enabled: u32,          // 48: 4 bytes
    bend_enabled: u32,           // 52: 4 bytes
    twist_amount: f32,           // 56: 4 bytes
    bend_amount: f32,            // 60: 4 bytes
    show_steps: u32,             // 64: 4 bytes
    grid_size: f32,              // 68: 4 bytes
    show_hud: u32,               // 72: 4 bytes
    show_building: u32,          // 76: 4 bytes
    _pad0: u32,                  // 80: 4 bytes
    _pad1: u32,                  // 84: 4 bytes
    _pad2: u32,                  // 88: 4 bytes
    _pad3: u32,                  // 92: 4 bytes
}

// Placed entity - must match Rust PlacedEntity exactly (48 bytes)
struct PlacedEntity {
    position: vec3<f32>,
    _pad_after_pos: u32,
    entity_type: u32,
    _pad_before_scale_0: u32,
    _pad_before_scale_1: u32,
    _pad_before_scale_2: u32,
    scale: vec3<f32>,
    color_packed: u32,
}

// Entity buffer
struct EntityBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    entities: array<PlacedEntity, 64>,
}

@group(0) @binding(0)
var<uniform> uniforms: DemoUniforms;

@group(0) @binding(1)
var<storage, read> entity_buffer: EntityBuffer;

// =============================================================================
// CONSTANTS
// =============================================================================

const PI: f32 = 3.14159265359;
const MAX_STEPS: i32 = 150;       // More steps for terrain (needs smaller steps)
const MAX_DIST: f32 = 100.0;
const SURF_DIST: f32 = 0.001;     // Standard surface threshold

// Blend modes
const BLEND_UNION: u32 = 0u;
const BLEND_SMOOTH_UNION: u32 = 1u;
const BLEND_SUBTRACT: u32 = 2u;
const BLEND_INTERSECT: u32 = 3u;
const BLEND_SMOOTH_SUBTRACT: u32 = 4u;
const BLEND_SMOOTH_INTERSECT: u32 = 5u;

// =============================================================================
// VERTEX SHADER
// =============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    return out;
}

// =============================================================================
// SDF PRIMITIVES
// =============================================================================

fn sdf_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sdf_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

fn sdf_torus(p: vec3<f32>, t: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

fn sdf_cylinder(p: vec3<f32>, h: f32, r: f32) -> f32 {
    let d = abs(vec2<f32>(length(p.xz), p.y)) - vec2<f32>(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

fn sdf_capsule(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

fn sdf_plane(p: vec3<f32>) -> f32 {
    return p.y;
}

// =============================================================================
// SDF OPERATIONS
// =============================================================================

// Smooth minimum (metaball blending)
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// Smooth maximum
fn smax(a: f32, b: f32, k: f32) -> f32 {
    return -smin(-a, -b, k);
}

// =============================================================================
// DOMAIN OPERATIONS
// =============================================================================

// Twist around Y axis
fn op_twist(p: vec3<f32>, k: f32) -> vec3<f32> {
    let c = cos(k * p.y);
    let s = sin(k * p.y);
    return vec3<f32>(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
}

// Bend along X axis
fn op_bend(p: vec3<f32>, k: f32) -> vec3<f32> {
    let c = cos(k * p.x);
    let s = sin(k * p.x);
    return vec3<f32>(c * p.x - s * p.y, s * p.x + c * p.y, p.z);
}

// =============================================================================
// HASH FUNCTIONS FOR PROCEDURAL GENERATION
// =============================================================================

fn hash21(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

// Hash to color - generates vibrant colors based on position
fn hash_to_color(cell: vec2<f32>) -> vec3<f32> {
    let h = hash21(cell);
    // HSV to RGB with high saturation
    let hue = h * 6.0;
    let x = 1.0 - abs(hue % 2.0 - 1.0);

    var color: vec3<f32>;
    if hue < 1.0 {
        color = vec3<f32>(1.0, x, 0.0);
    } else if hue < 2.0 {
        color = vec3<f32>(x, 1.0, 0.0);
    } else if hue < 3.0 {
        color = vec3<f32>(0.0, 1.0, x);
    } else if hue < 4.0 {
        color = vec3<f32>(0.0, x, 1.0);
    } else if hue < 5.0 {
        color = vec3<f32>(x, 0.0, 1.0);
    } else {
        color = vec3<f32>(1.0, 0.0, x);
    }

    // Make colors slightly pastel for better visibility
    return mix(color, vec3<f32>(1.0), 0.2);
}

// =============================================================================
// PERLIN NOISE FOR TERRAIN
// =============================================================================

// Proper modulo that works with negative numbers
fn mod_positive(x: f32, y: f32) -> f32 {
    return x - y * floor(x / y);
}

// 2D noise (value noise with smooth interpolation)
fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    
    // Smooth interpolation curve
    let u = f * f * (3.0 - 2.0 * f);
    
    // Four corners
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    
    // Bilinear interpolation
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// FBM (Fractal Brownian Motion) - multiple octaves of noise
fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var total_amp = 0.0;
    var pos = p;
    
    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise2d(pos * frequency);
        total_amp += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
        // Rotate each octave to reduce axis-aligned artifacts
        pos = vec2<f32>(pos.x * 0.8 - pos.y * 0.6, pos.x * 0.6 + pos.y * 0.8);
    }
    
    return value / total_amp;
}

// =============================================================================
// 3D NOISE FOR CAVES
// =============================================================================

// 3D value noise with smooth interpolation
fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    
    // Smooth interpolation curve
    let u = f * f * (3.0 - 2.0 * f);
    
    // Eight corners of cube
    let n000 = hash31(i);
    let n100 = hash31(i + vec3<f32>(1.0, 0.0, 0.0));
    let n010 = hash31(i + vec3<f32>(0.0, 1.0, 0.0));
    let n110 = hash31(i + vec3<f32>(1.0, 1.0, 0.0));
    let n001 = hash31(i + vec3<f32>(0.0, 0.0, 1.0));
    let n101 = hash31(i + vec3<f32>(1.0, 0.0, 1.0));
    let n011 = hash31(i + vec3<f32>(0.0, 1.0, 1.0));
    let n111 = hash31(i + vec3<f32>(1.0, 1.0, 1.0));
    
    // Trilinear interpolation
    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);
    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);
    return mix(nxy0, nxy1, u.z);
}

// 3D FBM for organic cave shapes
fn fbm3d(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var total_amp = 0.0;
    var pos = p;
    
    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise3d(pos * frequency);
        total_amp += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    
    return value / total_amp;
}

// =============================================================================
// TERRAIN SDF WITH PERLIN NOISE MOUNTAINS
// =============================================================================

// Get terrain height at XZ position using FBM noise
// Single FBM call with 5 octaves (was 11 samples across 4 calls = 5x faster)
fn terrain_height(xz: vec2<f32>) -> f32 {
    return fbm(xz * 0.02, 5) * 30.0;
}

// Distance-based LOD terrain height (fewer octaves when far away)
fn terrain_height_lod(xz: vec2<f32>, dist: f32) -> f32 {
    // Close: 5 octaves, Medium: 3 octaves, Far: 2 octaves
    if dist < 20.0 {
        return fbm(xz * 0.02, 5) * 30.0;
    } else if dist < 50.0 {
        return fbm(xz * 0.02, 3) * 30.0;
    } else {
        return fbm(xz * 0.02, 2) * 30.0;
    }
}

// SDF for terrain surface only (no caves)
fn sdf_terrain_surface(p: vec3<f32>) -> f32 {
    let h = terrain_height(p.xz);
    let vertical_dist = p.y - h;
    
    // Very conservative factor - FBM gradient can be ~2.0
    return vertical_dist * 0.3;
}

// =============================================================================
// CAVE SYSTEM
// =============================================================================

// Water level constant - 70% water coverage (Earth-like)
const WATER_LEVEL: f32 = 18.0;

// Magma level (deep underground)
const MAGMA_LEVEL: f32 = -10.0;

// Cave SDF - returns negative inside caves
fn sdf_caves(p: vec3<f32>) -> f32 {
    // Use 3D noise to create organic cave tunnels
    let cave_noise = fbm3d(p * 0.08, 3);
    
    // Worm-like tunnels using sine waves
    let worm1 = sin(p.x * 0.15) * sin(p.z * 0.12) * 0.3;
    let worm2 = sin(p.y * 0.1 + p.x * 0.08) * 0.2;
    
    // Combine noise and worms
    let combined = cave_noise + worm1 + worm2;
    
    // Threshold determines cave density (higher = more caves)
    let threshold = 0.55;
    return (combined - threshold) * 8.0;
}

// Simple terrain without caves (caves disabled to fix artifacts)
fn sdf_terrain(p: vec3<f32>) -> f32 {
    return sdf_terrain_surface(p);
}

// Get terrain color based on height and slope
fn get_terrain_color(p: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let h = p.y;
    let slope = 1.0 - normal.y;  // 0 = flat, 1 = vertical
    
    // Height thresholds (adjusted for 70% water coverage)
    let beach_level = WATER_LEVEL + 1.0;   // 19.0 - narrow beach
    let grass_level = WATER_LEVEL + 4.0;   // 22.0 - grass zone
    let rock_level = WATER_LEVEL + 8.0;    // 26.0 - rocky highlands
    let snow_level = WATER_LEVEL + 10.0;   // 28.0 - snow peaks
    
    // Base colors - more vibrant
    let sand_color = vec3<f32>(0.82, 0.75, 0.55);
    let grass_color = vec3<f32>(0.25, 0.55, 0.18);
    let dark_grass = vec3<f32>(0.15, 0.35, 0.10);
    let rock_color = vec3<f32>(0.45, 0.42, 0.38);
    let snow_color = vec3<f32>(0.98, 0.98, 1.0);
    
    var color: vec3<f32>;
    
    // Height-based color selection with smooth transitions
    if h < beach_level {
        // Beach/sand near water
        color = sand_color;
    } else if h < grass_level {
        // Grass zone with variation
        let t = (h - beach_level) / (grass_level - beach_level);
        // Mix between light and dark grass based on position
        let grass_var = hash21(floor(p.xz * 3.0));
        let base_grass = mix(dark_grass, grass_color, grass_var);
        color = mix(sand_color, base_grass, smoothstep(0.0, 0.3, t));
    } else if h < rock_level {
        // Transition grass to rock
        let t = (h - grass_level) / (rock_level - grass_level);
        let grass_var = hash21(floor(p.xz * 2.0));
        let varied_grass = mix(dark_grass, grass_color, grass_var);
        color = mix(varied_grass, rock_color, smoothstep(0.0, 1.0, t));
    } else if h < snow_level {
        // Transition rock to snow
        let t = (h - rock_level) / (snow_level - rock_level);
        color = mix(rock_color, snow_color, smoothstep(0.0, 1.0, t));
    } else {
        // Pure snow at peaks
        color = snow_color;
    }
    
    // Steep slopes show rock regardless of height
    if slope > 0.5 && h > beach_level {
        let rock_blend = smoothstep(0.5, 0.75, slope);
        color = mix(color, rock_color, rock_blend);
    }
    
    // Snow accumulates on flat surfaces at high altitude
    if normal.y > 0.6 && h > rock_level - 3.0 {
        let snow_amount = smoothstep(rock_level - 3.0, snow_level, h) * (normal.y - 0.5) * 2.0;
        color = mix(color, snow_color, clamp(snow_amount, 0.0, 1.0));
    }
    
    return color;
}

// Get cave/underground color based on depth
fn get_cave_color(p: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let surface_h = terrain_height(p.xz);
    let depth = surface_h - p.y;  // Depth below surface
    
    // Underground colors
    let dirt_color = vec3<f32>(0.4, 0.3, 0.2);
    let stone_color = vec3<f32>(0.35, 0.35, 0.38);
    let dark_stone = vec3<f32>(0.2, 0.2, 0.22);
    let magma_color = vec3<f32>(1.0, 0.3, 0.05);
    
    var color: vec3<f32>;
    
    // Depth-based coloring
    if depth < 3.0 {
        // Near surface: dirt/soil
        color = dirt_color;
    } else if depth < 10.0 {
        // Mid depth: stone
        let t = (depth - 3.0) / 7.0;
        color = mix(dirt_color, stone_color, t);
    } else if depth < 20.0 {
        // Deep: dark stone
        let t = (depth - 10.0) / 10.0;
        color = mix(stone_color, dark_stone, t);
    } else {
        // Very deep: magma glow
        let magma_dist = p.y - MAGMA_LEVEL;
        let magma_t = smoothstep(5.0, 0.0, magma_dist);
        color = mix(dark_stone, magma_color, magma_t);
    }
    
    // Add some variation with noise
    let noise_var = hash31(floor(p * 2.0)) * 0.1;
    color += vec3<f32>(noise_var * 0.5, noise_var * 0.4, noise_var * 0.3);
    
    return color;
}

// Check if point is inside a cave (returns 0-1 blend factor for smooth transitions)
fn cave_blend(p: vec3<f32>) -> f32 {
    let surface_h = terrain_height(p.xz);
    let depth = surface_h - p.y;
    
    if depth < 0.1 || p.y < MAGMA_LEVEL {
        return 0.0;  // Not in cave zone
    }
    
    let caves = sdf_caves(p);
    // Smooth transition from surface to cave (-1 to 1 range mapped to 0-1)
    return smoothstep(1.0, -1.0, caves);
}

// Check if point is inside a cave (boolean for compatibility)
fn is_in_cave(p: vec3<f32>) -> bool {
    return cave_blend(p) > 0.5;
}

// Get tropical water color with caustics
fn get_water_color(p: vec3<f32>, view_dir: vec3<f32>) -> vec3<f32> {
    // Tropical turquoise palette
    let shallow = vec3<f32>(0.15, 0.85, 0.75);   // Bright turquoise
    let medium = vec3<f32>(0.0, 0.55, 0.65);     // Caribbean blue
    let deep = vec3<f32>(0.02, 0.25, 0.45);      // Deep ocean blue
    let foam_color = vec3<f32>(0.95, 0.98, 1.0); // White foam
    
    // Depth-based color gradient
    let terrain_h = terrain_height(p.xz);
    let depth = WATER_LEVEL - terrain_h;
    let depth_t = clamp(depth / 12.0, 0.0, 1.0);
    
    // Three-way gradient: shallow -> medium -> deep
    var color: vec3<f32>;
    if depth_t < 0.4 {
        color = mix(shallow, medium, depth_t / 0.4);
    } else {
        color = mix(medium, deep, (depth_t - 0.4) / 0.6);
    }
    
    // Animated caustics (light patterns dancing on shallow water)
    let caustic1 = sin(p.x * 3.0 + uniforms.time * 0.8) * sin(p.z * 2.5 + uniforms.time * 0.6);
    let caustic2 = sin(p.x * 2.0 - uniforms.time * 0.5) * sin(p.z * 3.5 + uniforms.time * 0.7);
    let caustics = (caustic1 + caustic2) * 0.5 * 0.15 * (1.0 - depth_t);
    color += vec3<f32>(caustics, caustics * 1.1, caustics * 0.9);
    
    // Fresnel reflection of sky
    let fresnel = pow(1.0 - max(dot(vec3<f32>(0.0, 1.0, 0.0), -view_dir), 0.0), 4.0);
    let sky_reflect = vec3<f32>(0.7, 0.85, 1.0);
    color = mix(color, sky_reflect, fresnel * 0.4);
    
    // Foam near shoreline
    if depth < 2.0 {
        let foam_amount = (1.0 - depth / 2.0);
        // Animated foam pattern
        let foam_noise = hash21(floor(p.xz * 5.0) + vec2<f32>(sin(uniforms.time * 0.3), cos(uniforms.time * 0.2)));
        let foam_wave = sin(p.x * 0.5 + p.z * 0.3 + uniforms.time * 2.0) * 0.5 + 0.5;
        color = mix(color, foam_color, foam_amount * foam_noise * foam_wave * 0.7);
    }
    
    return color;
}

// Calculate terrain normal from height samples
fn calc_terrain_normal(p: vec3<f32>) -> vec3<f32> {
    let e = 0.5;  // Sample distance
    let h = terrain_height(p.xz);
    let hx = terrain_height(p.xz + vec2<f32>(e, 0.0));
    let hz = terrain_height(p.xz + vec2<f32>(0.0, e));
    
    return normalize(vec3<f32>(h - hx, e, h - hz));
}

struct TerrainResult {
    distance: f32,
    color: vec3<f32>,
    is_water: bool,
}

// Water plane SDF
fn sdf_water(p: vec3<f32>) -> f32 {
    return p.y - WATER_LEVEL;
}

// Full terrain scene with coloring, caves, and water
fn terrain_scene(p: vec3<f32>) -> TerrainResult {
    var result: TerrainResult;
    result.is_water = false;
    
    // Get terrain distance (now includes caves with smooth blending)
    let terrain_dist = sdf_terrain(p);
    let terrain_normal = calc_terrain_normal(p);
    
    // SMOOTH color blending between surface and cave (no hard edges)
    let cave_factor = cave_blend(p);
    let surface_color = get_terrain_color(p, terrain_normal);
    let underground_color = get_cave_color(p, terrain_normal);
    let terrain_color = mix(surface_color, underground_color, cave_factor);
    
    // Get water distance
    let water_dist = sdf_water(p);
    
    // Pick closest surface (water doesn't fill caves underground)
    if water_dist < terrain_dist && cave_factor < 0.3 {
        result.distance = water_dist;
        // Water color will be computed in shading with view direction
        result.color = vec3<f32>(0.1, 0.3, 0.5);  // Placeholder
        result.is_water = true;
    } else {
        result.distance = terrain_dist;
        result.color = terrain_color;
    }
    
    return result;
}

// For entity placement - simple tree on terrain (optional)
fn sdf_tree_entity(p: vec3<f32>, scale: f32) -> f32 {
    // Simple sphere tree for entities
    let trunk = sdf_box(p - vec3<f32>(0.0, scale * 0.4, 0.0), vec3<f32>(scale * 0.1, scale * 0.4, scale * 0.1));
    let crown = sdf_sphere(p - vec3<f32>(0.0, scale * 1.2, 0.0), scale * 0.6);
    return min(trunk, crown);
}

// =============================================================================
// ENTITY SDF EVALUATION
// =============================================================================

// Unpack color from u32
fn unpack_color(packed: u32) -> vec3<f32> {
    let r = f32((packed >> 16u) & 0xFFu) / 255.0;
    let g = f32((packed >> 8u) & 0xFFu) / 255.0;
    let b = f32(packed & 0xFFu) / 255.0;
    return vec3<f32>(r, g, b);
}

// Evaluate a single entity's SDF
fn evaluate_entity(p: vec3<f32>, entity: PlacedEntity) -> f32 {
    var local_p = p - entity.position;

    // Apply domain deformations if enabled
    if uniforms.twist_enabled == 1u {
        local_p = op_twist(local_p, uniforms.twist_amount);
    }
    if uniforms.bend_enabled == 1u {
        local_p = op_bend(local_p, uniforms.bend_amount);
    }

    let scale = entity.scale.x; // Use uniform scale

    switch entity.entity_type {
        case 0u: {
            // Sphere
            return sdf_sphere(local_p, scale);
        }
        case 1u: {
            // Box
            return sdf_box(local_p, vec3<f32>(scale * 0.8));
        }
        case 2u: {
            // Capsule (vertical)
            return sdf_capsule(local_p, vec3<f32>(0.0, -scale * 0.5, 0.0), vec3<f32>(0.0, scale * 0.5, 0.0), scale * 0.3);
        }
        case 3u: {
            // Torus
            return sdf_torus(local_p, vec2<f32>(scale * 0.6, scale * 0.2));
        }
        case 5u: {
            // Tree - simple clean tree
            return sdf_tree_entity(local_p, scale);
        }
        case 4u: {
            // Cylinder
            return sdf_cylinder(local_p, scale * 0.5, scale * 0.4);
        }
        default: {
            return sdf_sphere(local_p, scale);
        }
    }
}

// =============================================================================
// DEMO SCENE FUNCTIONS
// Each demonstrates a key SDF technique from SimonDev's tutorial
// =============================================================================

// Demo 1: PERLIN NOISE TERRAIN - Mountains with water, grass, and snow
// Uses FBM (Fractal Brownian Motion) for natural terrain
fn demo_space_repetition(p: vec3<f32>) -> SceneResult {
    var result: SceneResult;
    
    // Get terrain with height-based coloring
    let terrain = terrain_scene(p);
    
    result.distance = terrain.distance;
    result.color = terrain.color;
    result.is_ground = !terrain.is_water;  // Water is not ground for grid rendering
    
    return result;
}

// Demo 2: METABALLS - Smooth minimum creates organic blending
// Formula: smin(a, b, k) = min(a,b) - h²k/4 where h = max(k-|a-b|, 0)/k
fn demo_metaballs(p: vec3<f32>) -> SceneResult {
    var result: SceneResult;
    let t = uniforms.time;
    var d = MAX_DIST;
    var col = vec3<f32>(0.8, 0.4, 0.3);

    // Orbiting spheres that blend together
    for (var i = 0; i < 5; i++) {
        let fi = f32(i);
        let angle = t * 0.6 + fi * 1.257;  // Golden angle spacing
        let radius = 1.8 + sin(t * 0.4 + fi * 0.7) * 0.6;
        let height = 2.0 + sin(t * 0.5 + fi * 1.1) * 0.8;
        let center = vec3<f32>(cos(angle) * radius, height, sin(angle) * radius);
        
        // Color varies with position
        let hue = fi / 5.0 + t * 0.05;
        let sphere_col = vec3<f32>(
            0.5 + 0.5 * cos(hue * 6.28),
            0.5 + 0.5 * cos((hue + 0.33) * 6.28),
            0.5 + 0.5 * cos((hue + 0.67) * 6.28)
        );
        
        let sphere_d = sdf_sphere(p - center, 0.7);
        let old_d = d;
        
        // THE KEY: smooth minimum creates metaball effect
        d = smin(d, sphere_d, 0.8);
        
        // Blend colors based on contribution
        if d < old_d {
            let blend = smoothstep(old_d, d, sphere_d);
            col = mix(sphere_col, col, blend);
        }
    }

    let ground = sdf_plane(p);
    result.distance = min(d, ground);
    result.is_ground = result.distance == ground;
    result.color = select(col, vec3<f32>(0.25, 0.25, 0.3), result.is_ground);
    return result;
}

// Demo 3: CSG OPERATIONS - Boolean carving with max(-a, b)
// Cathedral arch carved from wall using subtraction
fn demo_csg_operations(p: vec3<f32>) -> SceneResult {
    var result: SceneResult;
    
    // Main wall structure
    let wall = sdf_box(p - vec3<f32>(0.0, 3.0, 0.0), vec3<f32>(5.0, 3.0, 0.6));
    
    // Arch shape to subtract (cylinder + box for rounded arch)
    let arch_pos = p - vec3<f32>(0.0, 2.0, 0.0);
    let arch_top = sdf_cylinder(arch_pos.xzy, 0.8, 1.8);  // Rotated cylinder for arch top
    let arch_body = sdf_box(arch_pos - vec3<f32>(0.0, -1.0, 0.0), vec3<f32>(1.8, 1.0, 1.0));
    let arch = min(arch_top, arch_body);  // Union for arch shape
    
    // CSG SUBTRACTION: max(-carve_shape, base_shape)
    var d = max(-arch, wall);
    
    // Add decorative spheres on top (union)
    let ball1 = sdf_sphere(p - vec3<f32>(-3.0, 6.5, 0.0), 0.5);
    let ball2 = sdf_sphere(p - vec3<f32>(0.0, 6.5, 0.0), 0.5);
    let ball3 = sdf_sphere(p - vec3<f32>(3.0, 6.5, 0.0), 0.5);
    d = min(d, min(ball1, min(ball2, ball3)));
    
    // Side pillars
    let pillar1 = sdf_cylinder(p - vec3<f32>(-4.0, 1.5, 1.0), 1.5, 0.3);
    let pillar2 = sdf_cylinder(p - vec3<f32>(4.0, 1.5, 1.0), 1.5, 0.3);
    d = min(d, min(pillar1, pillar2));
    
    let ground = sdf_plane(p);
    result.distance = min(d, ground);
    result.is_ground = result.distance == ground;
    result.color = select(vec3<f32>(0.85, 0.78, 0.65), vec3<f32>(0.35, 0.3, 0.25), result.is_ground);
    return result;
}

// Demo 4: DOMAIN TWIST - Rotate coordinates based on position
// twist(p).xz = rotate(p.xz, k * p.y)
fn demo_domain_twist(p: vec3<f32>) -> SceneResult {
    var result: SceneResult;
    let t = uniforms.time;
    
    // Animated twist amount
    let twist_amt = sin(t * 0.4) * 0.8;
    
    // Tower position
    let tower_p = p - vec3<f32>(0.0, 3.0, 0.0);
    
    // Apply twist deformation
    let twisted = op_twist(tower_p, twist_amt);
    
    // Box with rounded edges (subtract small amount)
    let tower = sdf_box(twisted, vec3<f32>(1.0, 3.0, 1.0)) - 0.08;
    
    // Top dome blends smoothly
    let dome = sdf_sphere(tower_p - vec3<f32>(0.0, 3.5, 0.0), 1.2);
    let structure = smin(tower, dome, 0.4);
    
    // Second smaller twisted tower
    let tower2_p = p - vec3<f32>(4.0, 2.0, 0.0);
    let twisted2 = op_twist(tower2_p, -twist_amt * 1.5);
    let tower2 = sdf_box(twisted2, vec3<f32>(0.6, 2.0, 0.6)) - 0.05;
    
    let ground = sdf_plane(p);
    var d = min(structure, tower2);
    d = min(d, ground);
    
    result.distance = d;
    result.is_ground = d == ground;
    if d == ground {
        result.color = vec3<f32>(0.3, 0.35, 0.4);
    } else if d == tower2 {
        result.color = vec3<f32>(0.4, 0.7, 0.9);
    } else {
        result.color = vec3<f32>(0.95, 0.6, 0.25);
    }
    return result;
}

// Demo 5: SHAPE MORPHING - Interpolate between SDFs
// morph = mix(sdf_a, sdf_b, t)
fn demo_morphing(p: vec3<f32>) -> SceneResult {
    var result: SceneResult;
    let t = uniforms.time;
    let center = p - vec3<f32>(0.0, 2.0, 0.0);
    
    // Three primitive shapes
    let sphere = sdf_sphere(center, 1.5);
    let box_d = sdf_box(center, vec3<f32>(1.2));
    let torus = sdf_torus(center, vec2<f32>(1.3, 0.5));
    
    // Cycle through shapes smoothly
    let cycle = fract(t * 0.15) * 3.0;
    var d: f32;
    var col: vec3<f32>;
    
    // MORPHING: linear interpolation between distances
    if cycle < 1.0 {
        let blend = smoothstep(0.0, 1.0, cycle);
        d = mix(sphere, box_d, blend);
        col = mix(vec3<f32>(0.9, 0.3, 0.3), vec3<f32>(0.3, 0.9, 0.3), blend);
    } else if cycle < 2.0 {
        let blend = smoothstep(0.0, 1.0, cycle - 1.0);
        d = mix(box_d, torus, blend);
        col = mix(vec3<f32>(0.3, 0.9, 0.3), vec3<f32>(0.3, 0.3, 0.9), blend);
    } else {
        let blend = smoothstep(0.0, 1.0, cycle - 2.0);
        d = mix(torus, sphere, blend);
        col = mix(vec3<f32>(0.3, 0.3, 0.9), vec3<f32>(0.9, 0.3, 0.3), blend);
    }
    
    let ground = sdf_plane(p);
    result.distance = min(d, ground);
    result.is_ground = result.distance == ground;
    result.color = select(col, vec3<f32>(0.25), result.is_ground);
    return result;
}

// Demo 6: DISPLACEMENT - Add noise to surface
// displaced = base_sdf + noise(p) * amplitude
fn demo_displacement(p: vec3<f32>) -> SceneResult {
    var result: SceneResult;
    let t = uniforms.time;
    let center = p - vec3<f32>(0.0, 2.5, 0.0);
    
    // Slowly rotate the object
    let angle = t * 0.2;
    let rotated = vec3<f32>(
        center.x * cos(angle) + center.z * sin(angle),
        center.y,
        -center.x * sin(angle) + center.z * cos(angle)
    );
    
    // Base sphere
    let sphere = sdf_sphere(rotated, 2.0);
    
    // Multi-octave noise displacement
    let noise1 = hash31(rotated * 2.0 + vec3<f32>(t * 0.1, 0.0, 0.0));
    let noise2 = hash31(rotated * 4.0 + vec3<f32>(0.0, t * 0.15, 0.0)) * 0.5;
    let noise3 = hash31(rotated * 8.0 + vec3<f32>(0.0, 0.0, t * 0.2)) * 0.25;
    let noise_val = (noise1 + noise2 + noise3) / 1.75;
    
    // DISPLACEMENT: add noise to distance
    let displaced = sphere + (noise_val - 0.5) * 0.5;
    
    // Color based on displacement
    let col = mix(
        vec3<f32>(0.4, 0.3, 0.6),
        vec3<f32>(0.8, 0.5, 0.3),
        noise_val
    );
    
    let ground = sdf_plane(p);
    result.distance = min(displaced, ground);
    result.is_ground = result.distance == ground;
    result.color = select(col, vec3<f32>(0.2, 0.25, 0.3), result.is_ground);
    return result;
}

// Demo 7: SOFT SHADOWS - Secondary ray march toward light
fn demo_soft_shadows(p: vec3<f32>) -> SceneResult {
    var result: SceneResult;
    
    // Multiple objects to cast shadows
    let sphere = sdf_sphere(p - vec3<f32>(-2.5, 1.5, 0.0), 1.5);
    let box_d = sdf_box(p - vec3<f32>(2.5, 1.0, 0.0), vec3<f32>(1.0));
    let torus = sdf_torus(p - vec3<f32>(0.0, 0.5, -3.0), vec2<f32>(1.5, 0.4));
    let cylinder = sdf_cylinder(p - vec3<f32>(0.0, 2.0, 3.0), 2.0, 0.5);
    
    let ground = sdf_plane(p);
    
    var d = min(sphere, box_d);
    d = min(d, torus);
    d = min(d, cylinder);
    d = min(d, ground);
    
    result.distance = d;
    result.is_ground = d == ground;
    
    if d == ground {
        result.color = vec3<f32>(0.6, 0.6, 0.65);
    } else if d == sphere {
        result.color = vec3<f32>(0.95, 0.25, 0.35);
    } else if d == box_d {
        result.color = vec3<f32>(0.25, 0.7, 0.95);
    } else if d == torus {
        result.color = vec3<f32>(0.95, 0.8, 0.2);
    } else {
        result.color = vec3<f32>(0.3, 0.9, 0.4);
    }
    return result;
}

// Demo 8: COMBINED SCENE - All techniques together
fn demo_combined(p: vec3<f32>) -> SceneResult {
    var result: SceneResult;
    let t = uniforms.time;
    
    // Space-repeated trees in background
    // Terrain in outer areas
    let terrain_area = p.x > 15.0 || p.x < -15.0 || p.z > 15.0 || p.z < -15.0;
    var d = MAX_DIST;
    var col = vec3<f32>(0.5);
    
    if terrain_area {
        let terrain = terrain_scene(p);
        d = terrain.distance;
        col = terrain.color;
    }
    
    // Central twisted tower with CSG window
    let tower_p = p - vec3<f32>(0.0, 4.0, 0.0);
    let twist_amt = sin(t * 0.2) * 0.3;
    let twisted = op_twist(tower_p, twist_amt);
    var tower = sdf_box(twisted, vec3<f32>(1.5, 4.0, 1.5)) - 0.1;
    
    // Carve window (CSG subtraction)
    let window = sdf_sphere(tower_p - vec3<f32>(0.0, 1.0, 1.5), 0.8);
    tower = max(-window, tower);
    
    if tower < d {
        d = tower;
        col = vec3<f32>(0.8, 0.6, 0.4);
    }
    
    // Metaball fountain
    var fountain = MAX_DIST;
    for (var i = 0; i < 4; i++) {
        let fi = f32(i);
        let angle = t * 0.8 + fi * 1.57;
        let center = vec3<f32>(cos(angle) * 3.0, 1.0 + sin(t + fi) * 0.5, sin(angle) * 3.0);
        let ball = sdf_sphere(p - center, 0.5);
        fountain = smin(fountain, ball, 0.6);
    }
    
    if fountain < d {
        d = fountain;
        col = vec3<f32>(0.3, 0.6, 0.9);
    }
    
    // Morphing shape
    let morph_center = p - vec3<f32>(6.0, 1.5, 0.0);
    let morph_sphere = sdf_sphere(morph_center, 1.0);
    let morph_box = sdf_box(morph_center, vec3<f32>(0.8));
    let morph = mix(morph_sphere, morph_box, sin(t * 0.5) * 0.5 + 0.5);
    
    if morph < d {
        d = morph;
        col = vec3<f32>(0.9, 0.4, 0.6);
    }
    
    let ground = sdf_plane(p);
    if ground < d {
        d = ground;
        col = vec3<f32>(0.25, 0.35, 0.2);
    }
    
    result.distance = d;
    result.color = col;
    result.is_ground = d == ground;
    return result;
}

// =============================================================================
// SCENE SDF
// =============================================================================

struct SceneResult {
    distance: f32,
    color: vec3<f32>,
    is_ground: bool,
}

fn scene_sdf(p: vec3<f32>) -> SceneResult {
    var result: SceneResult;
    
    // First: Get base scene from demo mode
    switch uniforms.demo_mode {
        case 1u: { result = demo_space_repetition(p); }
        case 2u: { result = demo_metaballs(p); }
        case 3u: { result = demo_csg_operations(p); }
        case 4u: { result = demo_domain_twist(p); }
        case 5u: { result = demo_morphing(p); }
        case 6u: { result = demo_displacement(p); }
        case 7u: { result = demo_soft_shadows(p); }
        case 8u: { result = demo_combined(p); }
        default: {
            // Mode 0: Interactive mode - just ground
            result.distance = MAX_DIST;
            result.color = vec3<f32>(0.5);
            result.is_ground = false;
            
            // Ground plane
            let ground = sdf_plane(p);
            result.distance = ground;
            result.color = vec3<f32>(0.25, 0.4, 0.15);
            result.is_ground = true;
            
            // Terrain mountains if enabled (F key)
            if uniforms.show_building == 1u {
                let terrain = terrain_scene(p);
                if terrain.distance < result.distance {
                    result.distance = terrain.distance;
                    result.color = terrain.color;
                    result.is_ground = true;
                }
            }
        }
    }

    // Only evaluate placed entities in interactive mode (0) for performance!
    // Demo modes 1-8 should run at full speed without entity overhead
    if uniforms.demo_mode != 0u {
        return result;
    }
    
    let count = entity_buffer.count;
    
    // Build entity geometry SEPARATELY from ground, then combine
    // This prevents subtraction from eating the ground
    var entity_dist = MAX_DIST;
    var entity_color = vec3<f32>(0.5);
    var has_entities = false;

    for (var i: u32 = 0u; i < count; i = i + 1u) {
        let entity = entity_buffer.entities[i];
        let this_dist = evaluate_entity(p, entity);
        let this_color = unpack_color(entity.color_packed);

        // Apply blend mode BETWEEN ENTITIES ONLY
        switch uniforms.blend_mode {
            case 0u: {
                // Hard union
                if this_dist < entity_dist {
                    entity_dist = this_dist;
                    entity_color = this_color;
                    has_entities = true;
                }
            }
            case 1u: {
                // Smooth union (metaballs!)
                let old_dist = entity_dist;
                entity_dist = smin(entity_dist, this_dist, uniforms.blend_k);
                if entity_dist < old_dist {
                    let t = clamp((old_dist - entity_dist) / uniforms.blend_k, 0.0, 1.0);
                    entity_color = mix(entity_color, this_color, t);
                }
                has_entities = true;
            }
            case 2u: {
                // Subtraction (newer carves older ENTITIES)
                if i == 0u {
                    entity_dist = this_dist;
                    entity_color = this_color;
                    has_entities = true;
                } else {
                    // Carve this entity out of previous entities
                    entity_dist = max(entity_dist, -this_dist);
                }
            }
            case 3u: {
                // Intersection
                if i == 0u {
                    entity_dist = this_dist;
                    entity_color = this_color;
                    has_entities = true;
                } else {
                    entity_dist = max(entity_dist, this_dist);
                }
            }
            case 4u: {
                // Smooth subtraction
                if i == 0u {
                    entity_dist = this_dist;
                    entity_color = this_color;
                    has_entities = true;
                } else {
                    entity_dist = smax(entity_dist, -this_dist, uniforms.blend_k);
                }
            }
            case 5u: {
                // Smooth intersection
                if i == 0u {
                    entity_dist = this_dist;
                    entity_color = this_color;
                    has_entities = true;
                } else {
                    entity_dist = smax(entity_dist, this_dist, uniforms.blend_k);
                }
            }
            default: {
                if this_dist < entity_dist {
                    entity_dist = this_dist;
                    entity_color = this_color;
                    has_entities = true;
                }
            }
        }
    }

    // Combine entity geometry with scene (ground/terrain)
    // Entities always UNION with ground (so subtraction doesn't eat ground)
    if has_entities && entity_dist < result.distance {
        result.distance = entity_dist;
        result.color = entity_color;
        result.is_ground = false;
    }

    return result;
}

// Simple scene SDF for raymarching (just distance)
fn scene_dist(p: vec3<f32>) -> f32 {
    return scene_sdf(p).distance;
}

// =============================================================================
// RAYMARCHING
// =============================================================================

struct RayResult {
    hit: bool,
    distance: f32,
    position: vec3<f32>,
    steps: i32,
    color: vec3<f32>,
    is_ground: bool,
}

fn raymarch(ro: vec3<f32>, rd: vec3<f32>) -> RayResult {
    var result: RayResult;
    result.hit = false;
    result.distance = 0.0;
    result.steps = 0;
    result.color = vec3<f32>(0.5);
    result.is_ground = false;

    var t = 0.0;
    
    // Early exit: if looking straight up into space, no terrain to hit
    if ro.y > PLANET_RADIUS && rd.y > 0.5 {
        result.distance = MAX_DIST;
        return result;
    }

    // SPHERE TRACING with view-based optimizations
    for (var i = 0; i < MAX_STEPS; i++) {
        result.steps = i;
        let p = ro + rd * t;
        let scene = scene_sdf(p);
        let d = scene.distance;

        // Hit surface
        if d < SURF_DIST {
            result.hit = true;
            result.distance = t;
            result.position = p;
            result.color = scene.color;
            result.is_ground = scene.is_ground;
            return result;
        }

        // Too far - early exit
        if t > MAX_DIST {
            break;
        }
        
        // Early exit: ray going away from scene (optimization)
        if p.y > 50.0 && rd.y > 0.0 && d > 5.0 {
            break;  // Above terrain and going up, no hit possible
        }

        // FULL STEP - this is the core sphere tracing optimization!
        // SDF guarantees d is the safe distance to march
        t += d;
    }

    result.distance = t;
    result.position = ro + rd * t;
    return result;
}

// =============================================================================
// LIGHTING
// =============================================================================

fn calc_normal(p: vec3<f32>) -> vec3<f32> {
    // TETRAHEDRON METHOD from SimonDev - only 4 samples instead of 6!
    let e = 0.001;
    let k = vec2<f32>(1.0, -1.0);
    return normalize(
        k.xyy * scene_dist(p + k.xyy * e) +
        k.yyx * scene_dist(p + k.yyx * e) +
        k.yxy * scene_dist(p + k.yxy * e) +
        k.xxx * scene_dist(p + k.xxx * e)
    );
}

fn calc_ao(p: vec3<f32>, n: vec3<f32>) -> f32 {
    var occ = 0.0;
    var sca = 1.0;

    for (var i = 0; i < 5; i++) {
        let h = 0.01 + 0.12 * f32(i) / 4.0;
        let d = scene_dist(p + h * n);
        occ += (h - d) * sca;
        sca *= 0.95;
    }

    return clamp(1.0 - 3.0 * occ, 0.0, 1.0);
}

// =============================================================================
// GRID RENDERING
// =============================================================================

fn render_grid(p: vec3<f32>) -> vec3<f32> {
    let grid_size = uniforms.grid_size;
    let large_grid = grid_size * 10.0;

    // Small grid lines
    let small_x = abs(fract(p.x / grid_size + 0.5) - 0.5) * grid_size;
    let small_z = abs(fract(p.z / grid_size + 0.5) - 0.5) * grid_size;
    let small_line = min(small_x, small_z);

    // Large grid lines
    let large_x = abs(fract(p.x / large_grid + 0.5) - 0.5) * large_grid;
    let large_z = abs(fract(p.z / large_grid + 0.5) - 0.5) * large_grid;
    let large_line = min(large_x, large_z);

    // Axis lines
    let axis_x = abs(p.z);
    let axis_z = abs(p.x);

    // Base color (checkerboard)
    let check = (floor(p.x / grid_size) + floor(p.z / grid_size)) % 2.0;
    var color = mix(vec3<f32>(0.25, 0.25, 0.28), vec3<f32>(0.32, 0.32, 0.35), check);

    // Grid lines
    let line_width = 0.02;
    if small_line < line_width {
        color = mix(color, vec3<f32>(0.4, 0.4, 0.45), 0.5);
    }
    if large_line < line_width * 2.0 {
        color = mix(color, vec3<f32>(0.5, 0.5, 0.55), 0.7);
    }

    // Axis colors
    if axis_x < line_width * 3.0 {
        color = mix(color, vec3<f32>(0.8, 0.2, 0.2), 0.8); // X axis = red
    }
    if axis_z < line_width * 3.0 {
        color = mix(color, vec3<f32>(0.2, 0.2, 0.8), 0.8); // Z axis = blue
    }

    return color;
}

// =============================================================================
// SKY
// =============================================================================

fn get_sky_color(rd: vec3<f32>) -> vec3<f32> {
    let sky_top = vec3<f32>(0.3, 0.5, 0.85);
    let sky_horizon = vec3<f32>(0.6, 0.75, 0.9);
    let t = max(rd.y, 0.0);
    return mix(sky_horizon, sky_top, t);
}

// =============================================================================
// ATMOSPHERE SYSTEM
// =============================================================================

// Planet radius for atmosphere calculations (approximate based on terrain)
const PLANET_RADIUS: f32 = 50.0;
const ATMO_THICKNESS: f32 = 15.0;

// Atmosphere rim glow for space view
fn atmosphere_rim_glow(ro: vec3<f32>, rd: vec3<f32>) -> vec3<f32> {
    // Calculate how close the ray passes to the planet
    // Simplified: use camera height and view angle
    let cam_height = ro.y;
    
    // Looking down at planet from space
    if cam_height > PLANET_RADIUS + ATMO_THICKNESS {
        // Fresnel-like glow at horizon
        let horizon_angle = -rd.y;  // Looking down = positive
        let glow_strength = smoothstep(-0.1, 0.3, horizon_angle);
        
        // Blue atmosphere glow
        let atmo_color = vec3<f32>(0.4, 0.6, 1.0);
        return atmo_color * glow_strength * 0.5;
    }
    
    return vec3<f32>(0.0);
}

// Atmospheric fog for surface view
fn atmospheric_fog(color: vec3<f32>, dist: f32, rd: vec3<f32>, cam_height: f32) -> vec3<f32> {
    // Only apply fog when on/near surface
    if cam_height > PLANET_RADIUS {
        return color;  // In space, no fog
    }
    
    // Distance-based fog
    let fog_density = 0.008;
    let fog_amount = 1.0 - exp(-dist * fog_density);
    
    // Fog color based on view direction (bluer at horizon)
    let sky_blend = smoothstep(-0.2, 0.3, rd.y);
    let fog_color = mix(
        vec3<f32>(0.6, 0.7, 0.85),  // Horizon fog
        get_sky_color(rd),           // Sky fog
        sky_blend
    );
    
    return mix(color, fog_color, fog_amount);
}

// Get sky with atmosphere (space or surface)
fn get_sky_with_atmosphere(rd: vec3<f32>, ro: vec3<f32>) -> vec3<f32> {
    let cam_height = ro.y;
    
    if cam_height > PLANET_RADIUS + ATMO_THICKNESS {
        // In space: dark sky with stars and atmosphere glow
        let space_color = vec3<f32>(0.02, 0.02, 0.05);
        
        // Simple stars
        let star_pos = floor(rd * 100.0);
        let star = hash31(star_pos);
        var stars = vec3<f32>(0.0);
        if star > 0.98 {
            stars = vec3<f32>(star * 0.5);
        }
        
        // Add atmosphere rim when looking at planet
        let atmo_glow = atmosphere_rim_glow(ro, rd);
        
        return space_color + stars + atmo_glow;
    } else {
        // On surface: normal sky with atmospheric scattering
        return get_sky_color(rd);
    }
}

// =============================================================================
// STEP VISUALIZATION
// =============================================================================

fn visualize_steps(steps: i32) -> vec3<f32> {
    let t = f32(steps) / f32(MAX_STEPS);

    if t < 0.25 {
        return mix(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 1.0), t * 4.0);
    } else if t < 0.5 {
        return mix(vec3<f32>(0.0, 1.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), (t - 0.25) * 4.0);
    } else if t < 0.75 {
        return mix(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), (t - 0.5) * 4.0);
    } else {
        return mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), (t - 0.75) * 4.0);
    }
}

// =============================================================================
// MAIN SHADING
// =============================================================================

fn shade(result: RayResult, rd: vec3<f32>, ro: vec3<f32>) -> vec3<f32> {
    let cam_height = ro.y;
    
    if !result.hit {
        // Use atmosphere-aware sky
        return get_sky_with_atmosphere(rd, ro);
    }

    let p = result.position;
    let n = calc_normal(p);

    // Light direction
    let light_dir = normalize(vec3<f32>(0.5, 0.8, 0.3));
    
    // Check if this is water (flat surface at water level)
    let is_water = abs(p.y - WATER_LEVEL) < 0.1 && n.y > 0.99;
    
    // Check if in cave (for different lighting)
    let in_cave = is_in_cave(p);

    // Base color
    var base_color: vec3<f32>;
    if is_water {
        // Water gets special coloring with fresnel
        base_color = get_water_color(p, rd);
    } else if result.is_ground {
        base_color = render_grid(p);
    } else {
        base_color = result.color;
    }

    // Diffuse
    let diff = max(dot(n, light_dir), 0.0);

    // Shadows (skip in caves - they have their own lighting)
    var shadow = 1.0;
    if !in_cave {
        var shadow_t = 0.1;
        let shadow_start = p + n * 0.01;
        for (var i = 0; i < 16; i++) {
            let d = scene_dist(shadow_start + light_dir * shadow_t);
            if d < 0.001 {
                shadow = 0.2;
                break;
            }
            shadow = min(shadow, 8.0 * d / shadow_t);
            shadow_t += d;
            if shadow_t > 20.0 { break; }
        }
        shadow = clamp(shadow, 0.2, 1.0);
    } else {
        // Caves are darker with ambient occlusion only
        shadow = 0.3;
    }

    // AO
    let ao = calc_ao(p, n);

    // Combine lighting
    var ambient = 0.25;
    if in_cave {
        // Darker ambient in caves
        ambient = 0.1;
        
        // Add magma glow at depth
        if p.y < MAGMA_LEVEL + 5.0 {
            let magma_glow = smoothstep(MAGMA_LEVEL + 5.0, MAGMA_LEVEL, p.y);
            base_color += vec3<f32>(1.0, 0.3, 0.05) * magma_glow * 0.5;
        }
    }
    
    let lighting = ambient + diff * shadow * 0.75;
    var color = base_color * lighting * ao;

    // Fresnel rim (not in caves)
    if !in_cave {
        let fresnel = pow(1.0 - max(dot(n, -rd), 0.0), 3.0);
        color += vec3<f32>(0.2, 0.25, 0.3) * fresnel * 0.2;
    }

    // Apply atmospheric fog (on surface only)
    color = atmospheric_fog(color, result.distance, rd, cam_height);

    return color;
}

// =============================================================================
// FRAGMENT SHADER
// =============================================================================

@fragment
fn fs_main(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let aspect = uniforms.resolution.x / uniforms.resolution.y;
    let ndc = (uv * 2.0 - 1.0) * vec2<f32>(aspect, 1.0);

    let ro = uniforms.camera_pos;
    let cam_target = uniforms.camera_target;

    // Camera matrix
    let forward = normalize(cam_target - ro);
    let right = normalize(cross(vec3<f32>(0.0, 1.0, 0.0), forward));
    let up = cross(forward, right);

    // Ray direction
    let fov = 1.0;
    let rd = normalize(forward + right * ndc.x * fov + up * ndc.y * fov);

    // Raymarch
    let result = raymarch(ro, rd);

    // Shade
    var color: vec3<f32>;

    if uniforms.show_steps == 1u {
        // Step count visualization
        let normal_color = shade(result, rd, ro);
        let step_color = visualize_steps(result.steps);
        if result.hit {
            color = mix(normal_color, step_color, 0.5);
        } else {
            color = normal_color;
        }
    } else {
        color = shade(result, rd, ro);
    }

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(color, 1.0);
}
