// ============================================================================
// Bloom Extract (bloom_extract.wgsl)
// ============================================================================

struct BloomExtractParams {
    threshold: f32,
    knee: f32,
    _pad0: vec2<f32>,
}

@group(0) @binding(0) var<uniform> params: BloomExtractParams;
@group(0) @binding(1) var scene_tex: texture_2d<f32>;
@group(0) @binding(2) var linear_sampler: sampler;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(scene_tex, linear_sampler, in.uv).rgb;
    let b = max(max(c.r, c.g), c.b);
    let soft = clamp((b - params.threshold + params.knee) / max(2.0 * params.knee, 1e-4), 0.0, 1.0);
    let hard = max(b - params.threshold, 0.0);
    let contrib = max(hard, soft * soft * params.knee);
    let scale = contrib / max(b, 1e-4);
    return vec4<f32>(c * scale, 1.0);
}
