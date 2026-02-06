// Canvas 2D Image Trace Shader
//
// Renders a textured quad with adjustable opacity for background
// reference image tracing in the Asset Editor's Draw2D stage.

struct Uniforms {
    view_projection: mat4x4<f32>,
    opacity: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var t_texture: texture_2d<f32>;
@group(0) @binding(2) var t_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_projection * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_texture, t_sampler, in.uv);
    return vec4<f32>(color.rgb, color.a * uniforms.opacity);
}
