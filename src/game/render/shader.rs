//! Shader Source
//!
//! WGSL shader source code for the arena rendering pipeline.

/// Main shader source with PBR lighting and post-processing effects
pub const SHADER_SOURCE: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_dir: vec3<f32>,
    fog_density: f32,
    fog_color: vec3<f32>,
    ambient: f32,
    projectile_count: u32,
    _padding1: vec3<f32>,
    projectile_positions: array<vec4<f32>, 32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var structure_rock_tex: texture_2d<f32>;
@group(0) @binding(2) var structure_rock_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) screen_uv: vec2<f32>,
}

// ============================================================================
// ACES FILMIC TONEMAPPING (Unreal Engine 5 style)
// ============================================================================
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Gamma correction (linear to sRGB)
fn gamma_correct(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(1.0 / 2.2));
}

// Vignette effect
fn apply_vignette(color: vec3<f32>, uv: vec2<f32>, intensity: f32) -> vec3<f32> {
    let center = vec2<f32>(0.5, 0.5);
    let dist = distance(uv, center);
    let vignette = 1.0 - smoothstep(0.3, 0.9, dist * intensity);
    return color * vignette;
}

// Film grain for cinematic feel
fn film_grain(color: vec3<f32>, uv: vec2<f32>, time: f32, intensity: f32) -> vec3<f32> {
    let noise = fract(sin(dot(uv + time * 0.1, vec2<f32>(12.9898, 78.233))) * 43758.5453);
    return color + (noise - 0.5) * intensity;
}

// ============================================================================
// PBR-LIKE LIGHTING (UE5 inspired)
// ============================================================================

// GGX Normal Distribution Function
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let denom = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / (3.14159265 * denom * denom);
}

// Fresnel-Schlick approximation
fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3<f32>(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// Geometry Smith
fn geometry_schlick_ggx(n_dot_v: f32, roughness: f32) -> f32 {
    let r = roughness + 1.0;
    let k = (r * r) / 8.0;
    return n_dot_v / (n_dot_v * (1.0 - k) + k);
}

fn geometry_smith(n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, roughness: f32) -> f32 {
    let n_dot_v = max(dot(n, v), 0.0);
    let n_dot_l = max(dot(n, l), 0.0);
    let ggx1 = geometry_schlick_ggx(n_dot_v, roughness);
    let ggx2 = geometry_schlick_ggx(n_dot_l, roughness);
    return ggx1 * ggx2;
}

// Subsurface scattering approximation for vegetation
fn sss_approx(view_dir: vec3<f32>, light_dir: vec3<f32>, normal: vec3<f32>, thickness: f32) -> f32 {
    // Light passing through from behind
    let sss_dot = clamp(dot(-view_dir, light_dir), 0.0, 1.0);
    let sss = pow(sss_dot, 3.0) * thickness;
    // Wrap lighting contribution
    let wrap = clamp(dot(normal, light_dir) * 0.5 + 0.5, 0.0, 1.0);
    return sss * wrap;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.world_pos = in.position;
    out.normal = in.normal;
    out.color = in.color;
    // Calculate screen UV for post-processing
    out.screen_uv = (out.clip_position.xy / out.clip_position.w) * 0.5 + 0.5;
    out.screen_uv.y = 1.0 - out.screen_uv.y;
    return out;
}

// ============================================================================
// ENHANCED TERRAIN MATERIALS (apocalyptic battle arena style)
// ============================================================================

// Material colors (natural, vibrant)
const GRASS_COLOR: vec3<f32> = vec3<f32>(0.22, 0.38, 0.12);  // Rich green grass
const DIRT_COLOR: vec3<f32> = vec3<f32>(0.40, 0.30, 0.20);   // Warm earthy brown
const ROCK_COLOR: vec3<f32> = vec3<f32>(0.42, 0.38, 0.34);   // Natural warm stone
const SNOW_COLOR: vec3<f32> = vec3<f32>(0.62, 0.60, 0.56);   // Bright stone peaks

// Height band thresholds (world units)
const DIRT_START: f32 = 0.5;
const DIRT_END: f32 = 1.8;
const ROCK_START: f32 = 1.2;
const ROCK_END: f32 = 3.5;
const SNOW_START: f32 = 3.0;
const SNOW_END: f32 = 5.0;

fn terrain_noise(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn terrain_fbm(p: vec2<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    for (var i = 0; i < 3; i++) {
        let i_p = floor(pos);
        let f_p = fract(pos);
        let a = terrain_noise(i_p);
        let b = terrain_noise(i_p + vec2<f32>(1.0, 0.0));
        let c = terrain_noise(i_p + vec2<f32>(0.0, 1.0));
        let d = terrain_noise(i_p + vec2<f32>(1.0, 1.0));
        let u = f_p * f_p * (3.0 - 2.0 * f_p);
        value += amplitude * mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
        pos *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

fn get_terrain_material(world_pos: vec3<f32>, normal: vec3<f32>, vertex_color: vec3<f32>) -> vec3<f32> {
    let h = world_pos.y;
    let up = vec3<f32>(0.0, 1.0, 0.0);
    
    // Slope detection (0 = flat, 1 = cliff)
    let slope = 1.0 - clamp(dot(normalize(normal), up), 0.0, 1.0);
    
    // Height-based material transitions
    let dirt_t = smoothstep(DIRT_START, DIRT_END, h);
    let rock_t = smoothstep(ROCK_START, ROCK_END, h);
    let snow_t = smoothstep(SNOW_START, SNOW_END, h);
    
    // Blend by height
    var col = mix(GRASS_COLOR, DIRT_COLOR, dirt_t);
    col = mix(col, ROCK_COLOR, rock_t);
    col = mix(col, SNOW_COLOR, snow_t);
    
    // Steep slopes become rocky
    col = mix(col, ROCK_COLOR, slope * 0.85);
    
    // Add noise variation
    let noise = terrain_fbm(world_pos.xz * 0.25) * 0.12 - 0.06;
    col = col + vec3<f32>(noise, noise * 0.5, noise * 0.3);
    
    // Blend with vertex color (60% height-based, 40% vertex color for artist control)
    col = mix(col, vertex_color, 0.4);
    
    return col;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.normal);
    let sun_dir = normalize(uniforms.sun_dir);
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let half_dir = normalize(view_dir + sun_dir);

    // Enhanced terrain color with height bands + slope detection
    let albedo = get_terrain_material(in.world_pos, normal, in.color.rgb);

    // Stone texture modulation for structure blocks (world-space tiled)
    let uv = in.world_pos.xz * 0.18;
    let rock_sample = textureSample(structure_rock_tex, structure_rock_sampler, uv).rgb;
    let neutral_delta =
        abs(in.color.r - in.color.g) + abs(in.color.g - in.color.b) + abs(in.color.r - in.color.b);
    let stone_mask = clamp(1.0 - neutral_delta * 3.5, 0.0, 1.0);
    let textured_albedo = mix(albedo, albedo * (0.7 + rock_sample * 0.6), stone_mask * 0.7);
    
    // Detect grass/vegetation by green channel dominance
    let is_grass = albedo.g > albedo.r * 1.3 && albedo.g > albedo.b * 1.5;
    
    // Material properties (vary by surface type)
    let roughness = select(0.6, 0.85, is_grass);  // Grass is rougher
    let metallic = 0.0;  // Terrain is non-metallic
    let f0 = vec3<f32>(0.04);  // Dielectric base reflectivity
    
    // ============================================================
    // PBR LIGHTING
    // ============================================================
    let n_dot_l = max(dot(normal, sun_dir), 0.0);
    let n_dot_v = max(dot(normal, view_dir), 0.0);
    let n_dot_h = max(dot(normal, half_dir), 0.0);
    let h_dot_v = max(dot(half_dir, view_dir), 0.0);
    
    // Cook-Torrance BRDF
    let ndf = distribution_ggx(n_dot_h, roughness);
    let g = geometry_smith(normal, view_dir, sun_dir, roughness);
    let f = fresnel_schlick(h_dot_v, f0);
    
    let numerator = ndf * g * f;
    let denominator = 4.0 * n_dot_v * n_dot_l + 0.0001;
    let specular = numerator / denominator;
    
    // Energy conservation
    let ks = f;
    let kd = (vec3<f32>(1.0) - ks) * (1.0 - metallic);
    
    // Sun light (natural warm daylight)
    let sun_color = vec3<f32>(1.4, 1.3, 1.1);  // Warm white sunlight
    let sun_intensity = 2.0;
    let radiance = sun_color * sun_intensity;
    
    // Direct lighting contribution
    var direct_light = (kd * textured_albedo / 3.14159265 + specular) * radiance * n_dot_l;
    
    // ============================================================
    // SUBSURFACE SCATTERING (grass/vegetation)
    // ============================================================
    if (is_grass) {
        let sss = sss_approx(view_dir, sun_dir, normal, 0.4);
        let sss_color = vec3<f32>(0.4, 0.55, 0.15) * 2.0;  // Bright yellow-green
        direct_light = direct_light + sss_color * sss * textured_albedo;
    }
    
    // ============================================================
    // AMBIENT / HEMISPHERE LIGHTING (natural sky + ground bounce)
    // ============================================================
    let sky_color = vec3<f32>(0.55, 0.65, 0.80);   // Blue sky ambient
    let ground_color = vec3<f32>(0.30, 0.25, 0.18); // Warm ground bounce
    let sky_blend = normal.y * 0.5 + 0.5;
    let ambient_color = mix(ground_color, sky_color, sky_blend);

    // Simple AO based on normal (facing down = more occluded)
    let ao = normal.y * 0.25 + 0.75;

    let ambient_light = textured_albedo * ambient_color * 0.5 * ao;
    
    // ============================================================
    // RIM LIGHT (subtle sky-colored edge highlight)
    // ============================================================
    let rim = pow(1.0 - n_dot_v, 4.0);
    let rim_color = vec3<f32>(0.5, 0.6, 0.7) * rim * 0.15;  // Subtle sky-colored rim
    
    // ============================================================
    // COMBINE LIGHTING
    // ============================================================
    var color = direct_light + ambient_light + rim_color;
    
    // ============================================================
    // ATMOSPHERIC FOG (UE5-style height fog)
    // ============================================================
    let dist = length(in.world_pos - uniforms.camera_pos);
    let height = in.world_pos.y;
    
    // Height-based fog density (denser at low altitudes)
    let height_falloff = 0.02;
    let height_factor = exp(-max(height, 0.0) * height_falloff);
    
    // Distance fog with height modulation (very light — let skybox show through)
    let fog_amount = (1.0 - exp(-dist * uniforms.fog_density * height_factor)) * 0.5;
    
    // Natural atmospheric haze (blue-gray, not orange)
    let fog_near = vec3<f32>(0.55, 0.60, 0.65);  // Light blue-gray haze
    let fog_far = vec3<f32>(0.45, 0.50, 0.58);   // Slightly deeper blue-gray
    let fog_blend = clamp(dist / 120.0, 0.0, 1.0);
    let final_fog_color = mix(fog_near, fog_far, fog_blend);
    
    color = mix(color, final_fog_color, fog_amount);
    
    // Keep scene output in linear HDR; tonemap is handled in the final post pass.
    return vec4<f32>(color, in.color.a);
}
"#;
