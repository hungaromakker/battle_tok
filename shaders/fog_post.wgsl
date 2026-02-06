// ============================================================================
// Fog Post-Process Shader (fog_post.wgsl)
// ============================================================================
// Uses pre-baked 3D Perlin noise texture for smooth, natural steam.
// No per-pixel noise computation — just texture lookups.
// ============================================================================

struct FogParams {
    fog_color: vec3<f32>,
    density: f32,
    height_fog_start: f32,
    height_fog_density: f32,
    _pad0: vec2<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad1: f32,
    steam_color: vec3<f32>,
    steam_density: f32,
    island1_center: vec3<f32>,
    island_radius: f32,
    island2_center: vec3<f32>,
    lava_y: f32,
    steam_height: f32,
    wind_time: f32,
    wind_strength: f32,
    steam_edge_softness: f32,
}

@group(0) @binding(0) var<uniform> fog: FogParams;
@group(0) @binding(1) var scene_texture: texture_2d<f32>;
@group(0) @binding(2) var scene_sampler: sampler;
@group(0) @binding(3) var depth_texture: texture_depth_2d;
@group(0) @binding(4) var noise_3d: texture_3d<f32>;
@group(0) @binding(5) var noise_sampler: sampler;

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
    out.uv.y = 1.0 - out.uv.y;
    return out;
}

fn reconstruct_world_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0);
    let clip = vec4<f32>(ndc, depth, 1.0);
    let world = fog.inv_view_proj * clip;
    return world.xyz / world.w;
}

// ============================================================================
// SAMPLE 3D NOISE TEXTURE (smooth, pre-baked Perlin — no artifacts)
// The texture tiles seamlessly. We just scale world coords to UV space.
// ============================================================================

fn sample_noise(p: vec3<f32>) -> f32 {
    // Scale world position to noise UV — 0.015 means one tile = ~67m
    let uv = p * 0.015;
    return textureSample(noise_3d, noise_sampler, uv).r;
}

// Two-scale noise for richer detail (still just 2 texture lookups)
fn sample_noise2(p: vec3<f32>) -> f32 {
    let n1 = textureSample(noise_3d, noise_sampler, p * 0.012).r;
    let n2 = textureSample(noise_3d, noise_sampler, p * 0.035 + vec3<f32>(0.5, 0.3, 0.7)).r;
    return n1 * 0.6 + n2 * 0.4;
}

// ============================================================================
// STEAM DENSITY at a 3D point (uses texture lookups, not computation)
// ============================================================================

fn steam_density_at(p: vec3<f32>) -> f32 {
    // Distance from each island
    let d1 = length(p.xz - fog.island1_center.xz);
    let d2 = length(p.xz - fog.island2_center.xz);
    let min_dist = min(d1, d2);
    let edge_dist = min_dist - fog.island_radius;

    // Noise-distorted edge: wobble the boundary so it's not a perfect circle
    let edge_noise_uv = vec3<f32>(p.x * 0.02 + fog.wind_time * 0.03, 0.5, p.z * 0.02 + fog.wind_time * 0.02);
    let edge_wobble = (textureSample(noise_3d, noise_sampler, edge_noise_uv).r - 0.5) * 12.0;
    let noisy_edge = edge_dist + edge_wobble;

    // No steam well inside islands (with noise margin)
    if (noisy_edge < -4.0) { return 0.0; }

    // Also clear the bridge corridor between islands
    let bridge_dir = normalize(fog.island2_center.xz - fog.island1_center.xz);
    let to_p = p.xz - fog.island1_center.xz;
    let bridge_len = length(fog.island2_center.xz - fog.island1_center.xz);
    let along = dot(to_p, bridge_dir);
    if (along > 0.0 && along < bridge_len) {
        let perp = abs(dot(to_p, vec2<f32>(-bridge_dir.y, bridge_dir.x)));
        if (perp < 5.0 && p.y > fog.lava_y) { return 0.0; }
    }

    // Smooth ramp from noisy edge outward
    let edge_factor = smoothstep(-2.0, fog.steam_edge_softness, noisy_edge);

    // Height above lava
    let h = p.y - fog.lava_y;
    if (h < -1.0 || h > fog.steam_height) { return 0.0; }

    // Column height varies per XZ (slow-drifting noise)
    let wind_slow = vec3<f32>(fog.wind_time * 0.06, 0.0, fog.wind_time * 0.04);
    let column_n = sample_noise(vec3<f32>(p.x, 0.0, p.z) + wind_slow);
    let max_h = fog.steam_height * (0.2 + column_n * 0.8);
    if (h > max_h) { return 0.0; }

    // Vertical falloff (thick base, thin top)
    let h_ratio = h / max_h;
    let vert = (1.0 - h_ratio) * (1.0 - h_ratio * 0.7);

    // Surface boil
    let surface = exp(-max(h, 0.0) * 0.3) * 0.3;

    // 3D wisp turbulence (texture lookup with rising offset)
    let wisp_p = vec3<f32>(
        p.x + fog.wind_time * 0.8,
        p.y - fog.wind_time * 0.5,
        p.z + fog.wind_time * 0.4
    );
    let wisp = sample_noise2(wisp_p);
    let wisp_mask = smoothstep(0.2, 0.55, wisp);

    // Wind gust
    let gust = sin(fog.wind_time * 0.08 + p.x * 0.01) * 0.5 + 0.5;
    let wind_push = gust * fog.wind_strength * 8.0;
    let wind_edge = smoothstep(-wind_push, fog.steam_edge_softness * 0.6, edge_dist);
    let eff_edge = max(edge_factor, wind_edge * 0.5);

    return eff_edge * (vert + surface) * (0.3 + wisp_mask * 0.7) * fog.steam_density;
}

// ============================================================================
// RAY MARCH (12 steps through steam volume)
// ============================================================================

fn march_steam(ro: vec3<f32>, rd: vec3<f32>, max_dist: f32) -> f32 {
    let march_dist = min(max_dist, 120.0);
    let step_size = march_dist / 12.0;
    var accum = 0.0;

    var start_t = 0.0;
    if (ro.y > fog.lava_y + fog.steam_height && rd.y < -0.001) {
        start_t = (ro.y - fog.lava_y - fog.steam_height) / (-rd.y);
    }

    for (var i = 0; i < 12; i++) {
        let t = start_t + (f32(i) + 0.5) * step_size;
        if (t > march_dist) { break; }
        let p = ro + rd * t;
        let h = p.y - fog.lava_y;
        if (h >= -1.0 && h <= fog.steam_height) {
            accum += steam_density_at(p) * step_size * 0.03;
        }
        if (accum > 0.95) { break; }
    }
    return clamp(accum, 0.0, 0.95);
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let scene_color = textureSample(scene_texture, scene_sampler, uv).rgb;
    let depth = textureSample(depth_texture, scene_sampler, uv);

    let near_pos = reconstruct_world_position(uv, 0.1);
    let ray_dir = normalize(near_pos - fog.camera_pos);

    // Sky: ray march steam
    if (depth >= 0.9999) {
        if (fog.steam_density > 0.0) {
            let steam = march_steam(fog.camera_pos, ray_dir, 150.0);
            if (steam > 0.01) {
                let tint = mix(fog.steam_color, vec3<f32>(0.8, 0.78, 0.74), 0.3);
                return vec4<f32>(mix(scene_color, tint, steam), 1.0);
            }
        }
        return vec4<f32>(scene_color, 1.0);
    }

    // Geometry
    let world_pos = reconstruct_world_position(uv, depth);
    var final_color = scene_color;

    // Very light distance fog (only at extreme range)
    let distance = length(world_pos - fog.camera_pos);
    let fog_amount = clamp((1.0 - exp(-distance * fog.density)) * 0.25, 0.0, 0.3);
    final_color = mix(scene_color, fog.fog_color, fog_amount);

    // Steam volume
    if (fog.steam_density > 0.0) {
        let steam = march_steam(fog.camera_pos, ray_dir, distance);
        if (steam > 0.01) {
            let h_approx = mix(fog.camera_pos.y, world_pos.y, 0.5);
            let h_ratio = clamp((h_approx - fog.lava_y) / fog.steam_height, 0.0, 1.0);
            let tint = mix(fog.steam_color, vec3<f32>(0.8, 0.78, 0.74), h_ratio * 0.4);
            let glow = vec3<f32>(0.5, 0.18, 0.04) * exp(-max(h_approx - fog.lava_y, 0.0) * 0.12) * 0.06;
            final_color = mix(final_color, tint + glow, steam);
        }
    }

    return vec4<f32>(final_color, 1.0);
}
