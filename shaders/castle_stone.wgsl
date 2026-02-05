// ============================================================================
// Castle Stone Material Shader (castle_stone.wgsl)
// ============================================================================
// Procedural medieval stone shader with:
// - Brick/block pattern with mortar lines
// - Grime darkening near ground level
// - Warm torch bounce lighting
// - Lambert diffuse lighting from sun

// Uniforms - scalar fields to match Rust alignment
struct CastleUniforms {
    view_proj: mat4x4<f32>,           // 64 bytes (offset 0)
    camera_pos_x: f32,                // 4 bytes (offset 64)
    camera_pos_y: f32,                // 4 bytes (offset 68)
    camera_pos_z: f32,                // 4 bytes (offset 72)
    time: f32,                        // 4 bytes (offset 76)
    sun_dir_x: f32,                   // 4 bytes (offset 80)
    sun_dir_y: f32,                   // 4 bytes (offset 84)
    sun_dir_z: f32,                   // 4 bytes (offset 88)
    sun_intensity: f32,               // 4 bytes (offset 92)
    torch_color_r: f32,               // 4 bytes (offset 96)
    torch_color_g: f32,               // 4 bytes (offset 100)
    torch_color_b: f32,               // 4 bytes (offset 104)
    torch_strength: f32,              // 4 bytes (offset 108)
    ambient_strength: f32,            // 4 bytes (offset 112)
    fog_density: f32,                 // 4 bytes (offset 116)
    fog_color_r: f32,                 // 4 bytes (offset 120)
    fog_color_g: f32,                 // 4 bytes (offset 124)
    fog_color_b: f32,                 // 4 bytes (offset 128)
    stone_color_r: f32,               // 4 bytes (offset 132)
    stone_color_g: f32,               // 4 bytes (offset 136)
    stone_color_b: f32,               // 4 bytes (offset 140)
    mortar_color_r: f32,              // 4 bytes (offset 144)
    mortar_color_g: f32,              // 4 bytes (offset 148)
    mortar_color_b: f32,              // 4 bytes (offset 152)
    grime_strength: f32,              // 4 bytes (offset 156)
    brick_scale: f32,                 // 4 bytes (offset 160)
    _pad1: f32,                       // 4 bytes (offset 164) - padding
    _pad2: f32,                       // 4 bytes (offset 168) - padding
    _pad3: f32,                       // 4 bytes (offset 172) - align to 176
}

@group(0) @binding(0)
var<uniform> uniforms: CastleUniforms;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn get_camera_pos() -> vec3<f32> {
    return vec3<f32>(uniforms.camera_pos_x, uniforms.camera_pos_y, uniforms.camera_pos_z);
}

fn get_sun_dir() -> vec3<f32> {
    return normalize(vec3<f32>(uniforms.sun_dir_x, uniforms.sun_dir_y, uniforms.sun_dir_z));
}

fn get_torch_color() -> vec3<f32> {
    return vec3<f32>(uniforms.torch_color_r, uniforms.torch_color_g, uniforms.torch_color_b);
}

fn get_fog_color() -> vec3<f32> {
    return vec3<f32>(uniforms.fog_color_r, uniforms.fog_color_g, uniforms.fog_color_b);
}

fn get_stone_color() -> vec3<f32> {
    return vec3<f32>(uniforms.stone_color_r, uniforms.stone_color_g, uniforms.stone_color_b);
}

fn get_mortar_color() -> vec3<f32> {
    return vec3<f32>(uniforms.mortar_color_r, uniforms.mortar_color_g, uniforms.mortar_color_b);
}

// ============================================================================
// NOISE FUNCTIONS FOR PROCEDURAL PATTERN
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
// FRAGMENT SHADER - PROCEDURAL BRICK PATTERN WITH LIGHTING
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_pos;
    let normal = normalize(in.world_normal);
    let view_dir = normalize(in.view_dir);

    // ========================================================================
    // PROCEDURAL BRICK PATTERN
    // ========================================================================

    let brick_scale = uniforms.brick_scale;

    // Create brick UV coordinates based on world position
    // Use different planes based on surface normal
    var brick_uv: vec2<f32>;
    let abs_normal = abs(normal);

    if (abs_normal.y > abs_normal.x && abs_normal.y > abs_normal.z) {
        // Horizontal surface (floor/ceiling) - use XZ plane
        brick_uv = world_pos.xz * brick_scale;
    } else if (abs_normal.x > abs_normal.z) {
        // Vertical surface facing X - use YZ plane
        brick_uv = world_pos.yz * brick_scale;
    } else {
        // Vertical surface facing Z - use XY plane
        brick_uv = world_pos.xy * brick_scale;
    }

    // Offset every other row for brick stagger pattern
    let row = floor(brick_uv.y);
    let row_offset = (row % 2.0) * 0.5;
    let offset_uv = vec2<f32>(brick_uv.x + row_offset, brick_uv.y);

    // Get brick cell coordinates
    let brick_cell = floor(offset_uv);
    let brick_local = fract(offset_uv);

    // Mortar lines between bricks (at edges of each brick cell)
    let mortar_width = 0.08; // Width of mortar lines
    let mortar_x = smoothstep(0.0, mortar_width, brick_local.x) *
                   smoothstep(0.0, mortar_width, 1.0 - brick_local.x);
    let mortar_y = smoothstep(0.0, mortar_width, brick_local.y) *
                   smoothstep(0.0, mortar_width, 1.0 - brick_local.y);
    let is_brick = mortar_x * mortar_y; // 1.0 = brick, 0.0 = mortar

    // Add variation to individual bricks
    let brick_hash = hash2d(brick_cell);
    let brick_variation = 0.85 + brick_hash * 0.3; // 0.85 to 1.15 variation

    // Add subtle noise to break up flat surfaces
    let surface_noise = noise2d(brick_uv * 8.0) * 0.15 + 0.85;

    // Get base colors
    let stone_color = get_stone_color() * brick_variation * surface_noise;
    let mortar_color = get_mortar_color() * (0.9 + hash2d(brick_cell * 0.37) * 0.2);

    // Mix stone and mortar based on position
    var base_color = mix(mortar_color, stone_color, is_brick);

    // ========================================================================
    // GRIME DARKENING NEAR GROUND
    // ========================================================================

    // Grime accumulates at the bottom of walls
    // Height-based darkening: more grime at lower y values
    let grime_height = clamp(1.0 - world_pos.y * uniforms.grime_strength, 0.0, 1.0);

    // Add some noise to grime for organic look
    let grime_noise = noise2d(world_pos.xz * 2.0) * 0.3 + 0.7;
    let grime_factor = grime_height * grime_noise;

    // Grime is dark greenish-brown
    let grime_color = vec3<f32>(0.08, 0.1, 0.06);
    base_color = mix(base_color, grime_color, grime_factor * 0.6);

    // ========================================================================
    // LIGHTING
    // ========================================================================

    let sun_dir = get_sun_dir();

    // Lambert diffuse lighting from sun
    let sun_intensity = uniforms.sun_intensity;
    let n_dot_l = max(dot(normal, sun_dir), 0.0);
    let sun_light = vec3<f32>(1.0, 0.95, 0.9) * n_dot_l * sun_intensity;

    // Ambient light (sky contribution)
    let ambient = vec3<f32>(0.15, 0.18, 0.22) * uniforms.ambient_strength;

    // ========================================================================
    // TORCH BOUNCE LIGHTING
    // ========================================================================

    // Simulated warm torch light bouncing from below
    // Torches illuminate lower portions of walls with warm orange glow
    let torch_color = get_torch_color();
    let torch_strength = uniforms.torch_strength;

    // Torch light comes from below (upward facing surfaces catch more)
    let torch_dir = vec3<f32>(0.0, 1.0, 0.0);
    let torch_dot = max(dot(normal, torch_dir), 0.0);

    // Torch light is stronger near the ground and fades with height
    let torch_height_falloff = clamp(1.0 - world_pos.y * 0.08, 0.0, 1.0);

    // Add flicker to torch light
    let time = uniforms.time;
    let flicker = 0.85 + 0.15 * sin(time * 12.0 + world_pos.x * 0.3)
                      + 0.08 * sin(time * 23.0 + world_pos.z * 0.5);

    let torch_light = torch_color * torch_dot * torch_height_falloff * torch_strength * flicker;

    // ========================================================================
    // FINAL COLOR COMPOSITION
    // ========================================================================

    // Combine all lighting
    let total_light = sun_light + ambient + torch_light;
    var final_color = base_color * total_light;

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
