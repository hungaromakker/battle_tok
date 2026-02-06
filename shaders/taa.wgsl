// ============================================================================
// Temporal AA (taa.wgsl)
// ============================================================================

struct TaaParams {
    inv_curr_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
    inv_resolution: vec2<f32>,
    jitter: vec2<f32>,
    history_weight: f32,
    new_weight: f32,
    reject_threshold: f32,
    enabled: u32,
};

@group(0) @binding(0) var<uniform> taa: TaaParams;
@group(0) @binding(1) var current_tex: texture_2d<f32>;
@group(0) @binding(2) var history_tex: texture_2d<f32>;
@group(0) @binding(3) var depth_tex: texture_depth_2d;
@group(0) @binding(4) var linear_sampler: sampler;

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

fn luminance(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn reconstruct_world_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0);
    let clip = vec4<f32>(ndc, depth, 1.0);
    let world = taa.inv_curr_view_proj * clip;
    return world.xyz / world.w;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let current = textureSample(current_tex, linear_sampler, uv).rgb;

    if (taa.enabled == 0u) {
        return vec4<f32>(current, 1.0);
    }

    let depth = textureSample(depth_tex, linear_sampler, uv);
    if (depth >= 0.9999) {
        return vec4<f32>(current, 1.0);
    }

    let world_pos = reconstruct_world_position(uv, depth);
    let prev_clip = taa.prev_view_proj * vec4<f32>(world_pos, 1.0);

    if (abs(prev_clip.w) < 0.0001) {
        return vec4<f32>(current, 1.0);
    }

    let prev_ndc = prev_clip.xy / prev_clip.w;
    let prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 1.0 - (prev_ndc.y * 0.5 + 0.5));

    if (any(prev_uv < vec2<f32>(0.001, 0.001)) || any(prev_uv > vec2<f32>(0.999, 0.999))) {
        return vec4<f32>(current, 1.0);
    }

    var min_col = vec3<f32>(1e9);
    var max_col = vec3<f32>(-1e9);
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let o = vec2<f32>(f32(x), f32(y)) * taa.inv_resolution;
            let s = textureSample(current_tex, linear_sampler, uv + o).rgb;
            min_col = min(min_col, s);
            max_col = max(max_col, s);
        }
    }

    var history = textureSample(history_tex, linear_sampler, prev_uv).rgb;
    history = clamp(history, min_col, max_col);

    let l_diff = abs(luminance(history) - luminance(current));
    let reject = smoothstep(taa.reject_threshold, taa.reject_threshold * 4.0, l_diff);
    let hist_w = taa.history_weight * (1.0 - reject);
    let cur_w = taa.new_weight;

    let accum = history * hist_w + current * cur_w;
    let norm = max(hist_w + cur_w, 1e-5);
    return vec4<f32>(accum / norm, 1.0);
}
