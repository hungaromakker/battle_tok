// ============================================================================
// Skybox Shader (skybox.wgsl)
// ============================================================================
// Dual-cubemap skybox with day/night crossfade.
// Fullscreen triangle reconstructs world-space ray direction from inv_view_proj,
// then samples two cubemap textures and blends between them.
// ============================================================================

struct SkyboxUniforms {
    inv_view_proj: mat4x4<f32>,  // Inverse view-projection matrix
    blend_factor: f32,            // 0.0 = day, 1.0 = night
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
}

@group(0) @binding(0)
var<uniform> sky: SkyboxUniforms;

@group(0) @binding(1)
var day_cubemap: texture_cube<f32>;

@group(0) @binding(2)
var sky_sampler: sampler;

@group(0) @binding(3)
var night_cubemap: texture_cube<f32>;

// ============================================================================
// VERTEX SHADER (fullscreen triangle)
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle covering the entire screen
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );

    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    out.uv.y = 1.0 - out.uv.y; // Flip Y
    return out;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Reconstruct clip-space position from UV
    let ndc = vec2<f32>(in.uv.x * 2.0 - 1.0, (1.0 - in.uv.y) * 2.0 - 1.0);

    // Two points along the ray: near and far plane
    let near_clip = vec4<f32>(ndc, 0.0, 1.0);
    let far_clip = vec4<f32>(ndc, 1.0, 1.0);

    // Transform to world space
    let near_world = sky.inv_view_proj * near_clip;
    let far_world = sky.inv_view_proj * far_clip;

    let near_pos = near_world.xyz / near_world.w;
    let far_pos = far_world.xyz / far_world.w;

    // Ray direction in world space (cubemap lookup direction)
    let ray_dir = normalize(far_pos - near_pos);

    // Sample both cubemaps
    let day_color = textureSample(day_cubemap, sky_sampler, ray_dir).rgb;
    let night_color = textureSample(night_cubemap, sky_sampler, ray_dir).rgb;

    // Crossfade between day and night
    let color = mix(day_color, night_color, sky.blend_factor);

    return vec4<f32>(color, 1.0);
}
