// Mesh Shader
// Standard lit shader for 3D meshes
// Supports sun lighting, fog, and projectile glow

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_dir: vec3<f32>,
    fog_density: f32,
    fog_color: vec3<f32>,
    ambient: f32,
    projectile_count: u32,
    _pad1: vec3<f32>,
    _pad2: vec3<f32>,
    _pad3: f32,
    projectile_positions: array<vec4<f32>, 32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

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
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.world_pos = in.position;
    out.normal = normalize(in.normal);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.normal);
    let sun_dir = normalize(uniforms.sun_dir);

    // Basic sun lighting
    let sun_diffuse = max(dot(normal, sun_dir), 0.0);
    let sun_color = vec3<f32>(1.2, 0.9, 0.8); // Warm sun

    // Ambient lighting (apocalyptic purple tint)
    let ambient_color = vec3<f32>(0.35, 0.25, 0.45);

    // Lava glow from below (orange)
    let up_dot = max(dot(normal, vec3<f32>(0.0, -1.0, 0.0)), 0.0);
    let lava_glow = vec3<f32>(0.45, 0.25, 0.15) * up_dot;

    // Rim lighting (fiery orange)
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let rim = 1.0 - max(dot(view_dir, normal), 0.0);
    let rim_power = pow(rim, 3.0);
    let rim_color = vec3<f32>(0.9, 0.5, 0.25) * rim_power * 0.5;

    // Combine lighting
    var lighting = ambient_color * uniforms.ambient
                 + sun_color * sun_diffuse * 0.7
                 + lava_glow
                 + rim_color;

    // Add projectile glow (for nearby projectiles)
    for (var i = 0u; i < uniforms.projectile_count; i = i + 1u) {
        let proj_pos = uniforms.projectile_positions[i].xyz;
        let dist = distance(in.world_pos, proj_pos);
        if dist < 5.0 {
            let glow = (5.0 - dist) / 5.0;
            lighting = lighting + vec3<f32>(1.0, 0.6, 0.2) * glow * 0.3;
        }
    }

    // Apply lighting to base color
    var final_color = in.color.rgb * lighting;

    // Distance fog
    let camera_dist = distance(in.world_pos, uniforms.camera_pos);
    let fog_factor = 1.0 - exp(-camera_dist * uniforms.fog_density);
    final_color = mix(final_color, uniforms.fog_color, clamp(fog_factor, 0.0, 0.9));

    return vec4<f32>(final_color, in.color.a);
}
