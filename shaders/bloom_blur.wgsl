// ============================================================================
// Bloom Blur / Downsample / Upsample (bloom_blur.wgsl)
// ============================================================================

struct BloomBlurParams {
    texel_size: vec2<f32>,
    direction: vec2<f32>,
    intensity: f32,
    mode: u32, // 0: downsample, 1: blur, 2: upsample
}

@group(0) @binding(0) var<uniform> params: BloomBlurParams;
@group(0) @binding(1) var source_tex: texture_2d<f32>;
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
    let uv = in.uv;

    if (params.mode == 0u) {
        // 4-tap downsample
        let o = params.texel_size;
        let c0 = textureSample(source_tex, linear_sampler, uv + vec2<f32>(-o.x, -o.y)).rgb;
        let c1 = textureSample(source_tex, linear_sampler, uv + vec2<f32>( o.x, -o.y)).rgb;
        let c2 = textureSample(source_tex, linear_sampler, uv + vec2<f32>(-o.x,  o.y)).rgb;
        let c3 = textureSample(source_tex, linear_sampler, uv + vec2<f32>( o.x,  o.y)).rgb;
        return vec4<f32>((c0 + c1 + c2 + c3) * 0.25, 1.0);
    }

    let d = params.direction * params.texel_size;
    let w0 = 0.227027;
    let w1 = 0.316216;
    let w2 = 0.070270;

    var col = textureSample(source_tex, linear_sampler, uv).rgb * w0;
    col += textureSample(source_tex, linear_sampler, uv + d * 1.384615).rgb * w1;
    col += textureSample(source_tex, linear_sampler, uv - d * 1.384615).rgb * w1;
    col += textureSample(source_tex, linear_sampler, uv + d * 3.230769).rgb * w2;
    col += textureSample(source_tex, linear_sampler, uv - d * 3.230769).rgb * w2;

    if (params.mode == 2u) {
        col *= params.intensity;
    }

    return vec4<f32>(col, 1.0);
}
