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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.normal);
    let sun_dir = normalize(uniforms.sun_dir);
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let half_dir = normalize(view_dir + sun_dir);

    // Base color from vertex (terrain colors already in linear space)
    let albedo = in.color.rgb;
    
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
    
    // Sun light
    let sun_color = vec3<f32>(1.0, 0.95, 0.85);  // Warm sunlight
    let sun_intensity = 2.5;  // HDR intensity
    let radiance = sun_color * sun_intensity;
    
    // Direct lighting contribution
    var direct_light = (kd * albedo / 3.14159265 + specular) * radiance * n_dot_l;
    
    // ============================================================
    // SUBSURFACE SCATTERING (grass/vegetation)
    // ============================================================
    if (is_grass) {
        let sss = sss_approx(view_dir, sun_dir, normal, 0.4);
        let sss_color = vec3<f32>(0.4, 0.55, 0.15) * 2.0;  // Bright yellow-green
        direct_light = direct_light + sss_color * sss * albedo;
    }
    
    // ============================================================
    // AMBIENT / HEMISPHERE LIGHTING
    // ============================================================
    let sky_color = vec3<f32>(0.45, 0.55, 0.75);
    let ground_color = vec3<f32>(0.25, 0.20, 0.15);
    let sky_blend = normal.y * 0.5 + 0.5;
    let ambient_color = mix(ground_color, sky_color, sky_blend);
    
    // Simple AO based on normal (facing down = more occluded)
    let ao = normal.y * 0.3 + 0.7;
    
    let ambient_light = albedo * ambient_color * 0.35 * ao;
    
    // ============================================================
    // RIM LIGHT (edge highlighting)
    // ============================================================
    let rim = pow(1.0 - n_dot_v, 4.0);
    let rim_color = vec3<f32>(0.6, 0.7, 0.85) * rim * 0.15;
    
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
    let height_falloff = 0.04;
    let height_factor = exp(-max(height, 0.0) * height_falloff);
    
    // Distance fog with height modulation
    let fog_amount = (1.0 - exp(-dist * uniforms.fog_density * height_factor)) * 0.85;
    
    // Fog color varies with distance (blue haze)
    let fog_near = vec3<f32>(0.55, 0.65, 0.80);
    let fog_far = vec3<f32>(0.45, 0.50, 0.60);
    let fog_blend = clamp(dist / 100.0, 0.0, 1.0);
    let final_fog_color = mix(fog_near, fog_far, fog_blend);
    
    color = mix(color, final_fog_color, fog_amount);
    
    // ============================================================
    // POST-PROCESSING (Unreal-style cinematic look)
    // ============================================================
    
    // ACES Tonemapping
    color = aces_tonemap(color);
    
    // Subtle color grading (warm shadows, cool highlights)
    let lift = vec3<f32>(0.015, 0.01, 0.02);
    let gain = vec3<f32>(1.03, 1.01, 0.98);
    color = color * gain + lift;
    
    // Vignette
    color = apply_vignette(color, in.screen_uv, 1.0);
    
    // Very subtle film grain
    color = film_grain(color, in.screen_uv, uniforms.time, 0.015);
    
    // Gamma correction
    color = gamma_correct(color);
    
    return vec4<f32>(color, in.color.a);
}
"#;
