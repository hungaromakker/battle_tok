// ============================================================================
// Lava Shader (lava.wgsl) — OPTIMIZED
// ============================================================================
// Water-over-lava surface with visible glowing cracks underneath.
// Uses cheap cellular noise instead of expensive voronoi for performance.
// Target: 10000+ FPS on simple scenes.
// ============================================================================

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_dir: vec3<f32>,
    fog_density: f32,
    fog_color: vec3<f32>,
    ambient: f32,
}

struct LavaParams {
    time: f32,
    emissive_strength: f32,
    scale: f32,
    speed: f32,
    crack_sharpness: f32,
    normal_strength: f32,
    core_color: vec3<f32>,
    _pad0: f32,
    crust_color: vec3<f32>,
    _pad1: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<uniform> lava: LavaParams;

// ============================================================================
// CHEAP NOISE (fast, no loops)
// ============================================================================

fn hash1(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(q) * 43758.5453);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    return mix(
        mix(hash1(i), hash1(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash1(i + vec2<f32>(0.0, 1.0)), hash1(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

// Cheap 2-octave FBM (fast)
fn fbm2(p: vec2<f32>) -> f32 {
    return noise2d(p) * 0.6 + noise2d(p * 2.1 + vec2<f32>(17.0, 31.0)) * 0.4;
}

// ============================================================================
// CHEAP CELLULAR / CRACK PATTERN (replaces expensive voronoi)
// Uses grid-based nearest-point for crack-like edges — much faster
// ============================================================================

fn cellular_cracks(p: vec2<f32>, t: f32) -> f32 {
    let pi = floor(p);
    let pf = fract(p);

    var d1 = 8.0;  // Nearest distance
    var d2 = 8.0;  // Second nearest

    // 3x3 search (9 iterations, not 25+9=34 like voronoi)
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let neighbor = vec2<f32>(f32(x), f32(y));
            let point = hash2(pi + neighbor);
            // Animate the cell centers slowly
            let animated = 0.5 + 0.4 * sin(t * 0.3 + 6.283 * point);
            let diff = neighbor + animated - pf;
            let dist = dot(diff, diff);

            if (dist < d1) {
                d2 = d1;
                d1 = dist;
            } else if (dist < d2) {
                d2 = dist;
            }
        }
    }

    // Edge = where d1 and d2 are close (crack between cells)
    return sqrt(d2) - sqrt(d1);
}

// ============================================================================
// VERTEX SHADER
// ============================================================================

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) vertex_color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_position = in.position;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.world_normal = in.normal;
    out.vertex_color = in.color;
    return out;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position;
    let view_dir = normalize(uniforms.camera_pos - world_pos);
    let t = lava.time * lava.speed;

    // ========================================
    // 1. LAVA CRACKS (cheap cellular noise)
    // ========================================
    let crack_scale = 0.18;  // ~5.5m cells
    let cracks = cellular_cracks(world_pos.xz * crack_scale, t);

    // Crack mask: thin bright lines where cells meet
    let crack_bright = 1.0 - smoothstep(0.0, 0.15, cracks);

    // Flowing lava intensity in cracks (cheap 2-octave)
    let flow = fbm2(world_pos.xz * 0.25 + vec2<f32>(t * 0.12, t * 0.08));
    let pulse = 0.85 + 0.15 * sin(t * 1.2 + flow * 4.0);
    let heat = flow * pulse * lava.emissive_strength;

    // Lava color ramp (seen through cracks)
    let c_dark = vec3<f32>(0.6, 0.04, 0.0);
    let c_mid = vec3<f32>(1.0, 0.3, 0.0);
    let c_bright = vec3<f32>(1.0, 0.7, 0.1);
    var lava_col = mix(c_dark, c_mid, smoothstep(0.2, 0.5, heat));
    lava_col = mix(lava_col, c_bright, smoothstep(0.5, 0.8, heat));

    // Dark rock between cracks
    let rock_n = noise2d(world_pos.xz * 0.6 + vec2<f32>(100.0, 100.0));
    let rock = mix(vec3<f32>(0.08, 0.05, 0.03), vec3<f32>(0.18, 0.12, 0.08), rock_n);

    // Combine: rock plates with hot lava in cracks
    let lava_surface = mix(rock, lava_col, crack_bright);

    // Edge glow around cracks
    let edge_glow = (1.0 - smoothstep(0.0, 0.35, cracks));
    let glow = vec3<f32>(0.8, 0.15, 0.0) * edge_glow * edge_glow * 0.5 * heat;

    let lava_final = lava_surface + glow;

    // ========================================
    // 2. WATER LAYER (semi-transparent over lava)
    // ========================================
    let rd = -view_dir;
    let normal = normalize(in.world_normal);

    // Water normal with gentle waves (cheap)
    let w1 = sin(world_pos.x * 0.4 + lava.time * 1.2) * cos(world_pos.z * 0.35 + lava.time * 0.9) * 0.012;
    let w2 = sin(world_pos.x * 0.8 - lava.time * 1.8 + world_pos.z * 0.6) * 0.006;
    let water_n = normalize(vec3<f32>(-(w1 + w2) * 2.0, 1.0, -(w2) * 3.0));

    // Fresnel: more reflection at grazing, more lava visible looking down
    let fresnel = pow(1.0 - max(dot(-rd, water_n), 0.0), 3.0);
    let fres = mix(0.03, 0.8, fresnel);

    // Sky reflection
    let ref_dir = reflect(rd, water_n);
    let sky_up = max(ref_dir.y, 0.0);
    var sky_refl = mix(vec3<f32>(0.45, 0.55, 0.65), vec3<f32>(0.2, 0.3, 0.55), sky_up);
    let sun_dir = normalize(vec3<f32>(1.0, 0.5, 0.5));
    sky_refl += vec3<f32>(1.0, 0.9, 0.7) * pow(max(dot(ref_dir, sun_dir), 0.0), 32.0) * 0.5;

    // Lava visible through water (depth-attenuated but NOT fully hidden)
    let water_depth = 0.4;
    let depth_fade = exp(-water_depth * 2.5);
    let under_lava = lava_final * depth_fade;

    // Blue-teal water tint
    let water_tint = vec3<f32>(0.04, 0.14, 0.20);

    // Caustics
    let caustic = noise2d(world_pos.xz * 0.6 + t * 0.4) * noise2d(world_pos.xz * 0.9 - t * 0.25);
    let water_with_caustic = water_tint + vec3<f32>(0.04, 0.08, 0.10) * caustic;

    // Water base: lava underneath + tint, blended with sky via Fresnel
    var color = mix(under_lava + water_with_caustic, sky_refl, fres);

    // Specular sun highlight on water
    let spec = pow(max(dot(ref_dir, normalize(vec3<f32>(1.0, 0.8, 0.5))), 0.0), 48.0);
    color += vec3<f32>(1.0, 0.95, 0.85) * spec * 0.5;

    // Hot-spot glow where cracks are directly below water
    color += vec3<f32>(0.8, 0.35, 0.1) * crack_bright * 0.25 * lava.emissive_strength;

    // ========================================
    // 3. STEAM WISPS at hot cracks (cheap)
    // ========================================
    let steam_n = noise2d(world_pos.xz * 0.3 + vec2<f32>(t * 0.6, -t * 0.4));
    let steam = crack_bright * smoothstep(0.4, 0.7, steam_n) * 0.08;
    color = mix(color, vec3<f32>(0.82, 0.80, 0.76), steam);

    // ========================================
    // 4. MINIMAL FOG
    // ========================================
    let distance = length(uniforms.camera_pos - world_pos);
    let fog_amount = (1.0 - exp(-distance * uniforms.fog_density)) * 0.06;
    color = mix(color, uniforms.fog_color * 0.5, fog_amount);

    return vec4<f32>(color, 1.0);
}
