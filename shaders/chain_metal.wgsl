// ============================================================================
// Chain Metal Material Shader (chain_metal.wgsl)
// ============================================================================
// Metallic shader for bridge chain supports with:
// - Steel gray base color
// - Fresnel rim highlights (bright edges at grazing angles)
// - Specular reflections
// - Fog integration

// Uniforms - scalar fields to match Rust alignment
struct ChainMetalUniforms {
    view_proj: mat4x4<f32>,           // 64 bytes (offset 0)
    camera_pos_x: f32,                // 4 bytes (offset 64)
    camera_pos_y: f32,                // 4 bytes (offset 68)
    camera_pos_z: f32,                // 4 bytes (offset 72)
    time: f32,                        // 4 bytes (offset 76)
    sun_dir_x: f32,                   // 4 bytes (offset 80)
    sun_dir_y: f32,                   // 4 bytes (offset 84)
    sun_dir_z: f32,                   // 4 bytes (offset 88)
    sun_intensity: f32,               // 4 bytes (offset 92)
    ambient_strength: f32,            // 4 bytes (offset 96)
    fog_density: f32,                 // 4 bytes (offset 100)
    fog_color_r: f32,                 // 4 bytes (offset 104)
    fog_color_g: f32,                 // 4 bytes (offset 108)
    fog_color_b: f32,                 // 4 bytes (offset 112)
    steel_color_r: f32,               // 4 bytes (offset 116)
    steel_color_g: f32,               // 4 bytes (offset 120)
    steel_color_b: f32,               // 4 bytes (offset 124)
    shine: f32,                       // 4 bytes (offset 128) - Fresnel rim intensity
    roughness: f32,                   // 4 bytes (offset 132) - surface roughness
    metallic: f32,                    // 4 bytes (offset 136) - metallic factor
    _pad1: f32,                       // 4 bytes (offset 140) - padding to 16-byte align
}

@group(0) @binding(0)
var<uniform> uniforms: ChainMetalUniforms;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn get_camera_pos() -> vec3<f32> {
    return vec3<f32>(uniforms.camera_pos_x, uniforms.camera_pos_y, uniforms.camera_pos_z);
}

fn get_sun_dir() -> vec3<f32> {
    return normalize(vec3<f32>(uniforms.sun_dir_x, uniforms.sun_dir_y, uniforms.sun_dir_z));
}

fn get_fog_color() -> vec3<f32> {
    return vec3<f32>(uniforms.fog_color_r, uniforms.fog_color_g, uniforms.fog_color_b);
}

fn get_steel_color() -> vec3<f32> {
    return vec3<f32>(uniforms.steel_color_r, uniforms.steel_color_g, uniforms.steel_color_b);
}

// ============================================================================
// NOISE FOR SURFACE VARIATION
// ============================================================================

fn hash3d(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

// ============================================================================
// VERTEX SHADER
// ============================================================================

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) view_dir: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform to clip space
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);

    // Pass world space position
    out.world_pos = in.position;

    // Pass world space normal for lighting
    out.world_normal = normalize(in.normal);

    // Calculate view direction
    let camera_pos = get_camera_pos();
    out.view_dir = normalize(camera_pos - in.position);

    return out;
}

// ============================================================================
// FRAGMENT SHADER - METALLIC CHAIN WITH FRESNEL RIM
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_pos;
    let n = normalize(in.world_normal);
    let v = normalize(in.view_dir);
    let l = get_sun_dir();

    // ========================================================================
    // BASE STEEL COLOR
    // ========================================================================

    // Steel gray base color
    var steel = get_steel_color();

    // Add subtle surface variation for worn metal look
    let variation = hash3d(world_pos * 10.0) * 0.1 - 0.05;
    steel = steel * (1.0 + variation);

    // ========================================================================
    // FRESNEL RIM EFFECT
    // ========================================================================
    // Fresnel effect: surfaces at grazing angles reflect more light
    // This creates bright rim highlights on metal edges

    let n_dot_v = clamp(dot(n, v), 0.0, 1.0);

    // Fresnel approximation: bright at grazing angles (n_dot_v near 0)
    // Power of 4 gives a sharp rim effect
    let fresnel = pow(1.0 - n_dot_v, 4.0);

    // Rim highlight color (bright white/blue-ish for steel)
    let rim_color = vec3<f32>(1.0, 1.0, 1.1);
    let rim_contribution = rim_color * fresnel * uniforms.shine;

    // ========================================================================
    // DIFFUSE LIGHTING (LAMBERT)
    // ========================================================================

    let sun_intensity = uniforms.sun_intensity;
    let n_dot_l = max(dot(n, l), 0.0);

    // Diffuse lighting - metals have less diffuse, more specular
    let diffuse_factor = 1.0 - uniforms.metallic * 0.7;
    let diffuse = steel * n_dot_l * sun_intensity * diffuse_factor;

    // ========================================================================
    // SPECULAR LIGHTING (BLINN-PHONG)
    // ========================================================================

    // Half vector for Blinn-Phong
    let h = normalize(l + v);
    let n_dot_h = max(dot(n, h), 0.0);

    // Specular exponent based on roughness (lower roughness = sharper specular)
    let specular_power = mix(8.0, 128.0, 1.0 - uniforms.roughness);
    let specular = pow(n_dot_h, specular_power);

    // Specular color for metals tinted by base color
    let specular_color = mix(vec3<f32>(1.0), steel, uniforms.metallic);
    let specular_contribution = specular_color * specular * sun_intensity * 0.8;

    // ========================================================================
    // AMBIENT LIGHTING
    // ========================================================================

    // Ambient light with slight blue tint for sky
    let ambient = vec3<f32>(0.18, 0.2, 0.25) * uniforms.ambient_strength;
    let ambient_contribution = steel * ambient;

    // ========================================================================
    // FINAL COLOR COMPOSITION
    // ========================================================================

    // Combine all lighting contributions
    var final_color = ambient_contribution + diffuse + specular_contribution;

    // Add Fresnel rim highlight
    final_color = final_color + rim_contribution;

    // ========================================================================
    // FOG
    // ========================================================================

    let camera_pos = get_camera_pos();
    let dist = length(world_pos - camera_pos);
    let fog_factor = 1.0 - exp(-dist * uniforms.fog_density * 0.01);
    let fog_color = get_fog_color();
    final_color = mix(final_color, fog_color, clamp(fog_factor, 0.0, 1.0));

    // ========================================================================
    // TONE MAPPING AND GAMMA
    // ========================================================================

    // Reinhard tone mapping
    final_color = final_color / (final_color + vec3<f32>(1.0));

    // Gamma correction
    final_color = pow(final_color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(final_color, 1.0);
}
