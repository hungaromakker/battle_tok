// ============================================================================
// Wood Plank Material Shader (wood_plank.wgsl)
// ============================================================================
// Procedural wood plank shader for bridge walkway with:
// - Natural brown wood base color
// - Noise-based grain variation
// - Lambert diffuse lighting
// - Fog integration

// Uniforms - scalar fields to match Rust alignment
// Total size: 144 bytes (aligned to 16)
struct WoodPlankUniforms {
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
    wood_color_r: f32,                // 4 bytes (offset 116)
    wood_color_g: f32,                // 4 bytes (offset 120)
    wood_color_b: f32,                // 4 bytes (offset 124)
    grain_scale: f32,                 // 4 bytes (offset 128)
    grain_strength: f32,              // 4 bytes (offset 132)
    _pad1: f32,                       // 4 bytes (offset 136) - padding to 144
    _pad2: f32,                       // 4 bytes (offset 140) - padding to 144
}

@group(0) @binding(0)
var<uniform> uniforms: WoodPlankUniforms;

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

fn get_wood_color() -> vec3<f32> {
    return vec3<f32>(uniforms.wood_color_r, uniforms.wood_color_g, uniforms.wood_color_b);
}

// ============================================================================
// NOISE FUNCTIONS FOR WOOD GRAIN
// ============================================================================

// Simple hash function for pseudo-random values
fn hash2d(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash3d(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

// 2D value noise
fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(hash2d(i + vec2<f32>(0.0, 0.0)), hash2d(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash2d(i + vec2<f32>(0.0, 1.0)), hash2d(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

// Wood grain pattern - elongated noise along one axis
fn wood_grain(p: vec2<f32>, scale: f32) -> f32 {
    // Stretch noise more along one axis for grain direction
    let stretched = vec2<f32>(p.x * scale, p.y * scale * 0.15);

    // Layer multiple noise frequencies for realistic grain
    var grain = noise2d(stretched) * 0.6;
    grain += noise2d(stretched * 2.3) * 0.25;
    grain += noise2d(stretched * 5.1) * 0.15;

    return grain;
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

    // Pass world space position for procedural pattern
    out.world_pos = in.position;

    // Pass world space normal for lighting
    out.world_normal = normalize(in.normal);

    // Calculate view direction
    let camera_pos = get_camera_pos();
    out.view_dir = normalize(camera_pos - in.position);

    return out;
}

// ============================================================================
// FRAGMENT SHADER - WOOD PLANK WITH GRAIN
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_pos;
    let normal = normalize(in.world_normal);
    let view_dir = normalize(in.view_dir);

    // ========================================================================
    // BASE WOOD COLOR
    // ========================================================================

    // Base brown wood color from uniforms
    var base_color = get_wood_color();

    // ========================================================================
    // WOOD GRAIN PATTERN
    // ========================================================================

    let grain_scale = uniforms.grain_scale;
    let grain_strength = uniforms.grain_strength;

    // Choose UV based on surface orientation for consistent grain direction
    var grain_uv: vec2<f32>;
    let abs_normal = abs(normal);

    if (abs_normal.y > abs_normal.x && abs_normal.y > abs_normal.z) {
        // Horizontal surface (floor) - grain runs along X
        grain_uv = world_pos.xz;
    } else if (abs_normal.x > abs_normal.z) {
        // Vertical surface facing X - grain runs along Y (vertical boards)
        grain_uv = world_pos.yz;
    } else {
        // Vertical surface facing Z - grain runs along X
        grain_uv = world_pos.xy;
    }

    // Calculate wood grain
    let grain = wood_grain(grain_uv, grain_scale);

    // Apply grain variation: darken/lighten wood based on grain
    // Grain adds reddish-brown variation (more red, less blue)
    let grain_offset = (grain - 0.5) * grain_strength;
    base_color = base_color + vec3<f32>(grain_offset, grain_offset * 0.5, 0.0);

    // Add subtle plank-to-plank color variation
    let plank_id = floor(world_pos.x * 2.0); // Assume planks run along X
    let plank_variation = hash2d(vec2<f32>(plank_id, 0.0)) * 0.15 - 0.075;
    base_color = base_color * (1.0 + plank_variation);

    // Clamp to valid color range
    base_color = clamp(base_color, vec3<f32>(0.0), vec3<f32>(1.0));

    // ========================================================================
    // LIGHTING - LAMBERT DIFFUSE
    // ========================================================================

    let sun_dir = get_sun_dir();

    // Lambert diffuse lighting from sun
    let sun_intensity = uniforms.sun_intensity;
    let n_dot_l = max(dot(normal, sun_dir), 0.0);
    let sun_light = vec3<f32>(1.0, 0.95, 0.9) * n_dot_l * sun_intensity;

    // Ambient light
    let ambient = vec3<f32>(0.2, 0.18, 0.15) * uniforms.ambient_strength;

    // Combine lighting (no specular - wood is matte)
    let total_light = sun_light + ambient;
    var final_color = base_color * (0.3 + total_light * 0.9);

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
