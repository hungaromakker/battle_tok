// ============================================================================
// Tonemap + Bloom Composite (tonemap_composite.wgsl)
// ============================================================================

struct TonemapCompositeParams {
    exposure: f32,
    saturation: f32,
    contrast: f32,
    bloom_intensity: f32,
}

@group(0) @binding(0) var<uniform> params: TonemapCompositeParams;
@group(0) @binding(1) var hdr_scene: texture_2d<f32>;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;

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

fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn adjust_saturation(color: vec3<f32>, saturation: f32) -> vec3<f32> {
    let luma = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    return mix(vec3<f32>(luma), color, saturation);
}

fn adjust_contrast(color: vec3<f32>, contrast: f32) -> vec3<f32> {
    return (color - 0.5) * contrast + 0.5;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let scene = textureSample(hdr_scene, linear_sampler, in.uv).rgb;
    let bloom = textureSample(bloom_tex, linear_sampler, in.uv).rgb * params.bloom_intensity;
    var hdr = (scene + bloom) * params.exposure;

    var ldr = aces_tonemap(hdr);
    ldr = adjust_saturation(ldr, params.saturation);
    ldr = adjust_contrast(ldr, params.contrast);

    return vec4<f32>(ldr, 1.0);
}
