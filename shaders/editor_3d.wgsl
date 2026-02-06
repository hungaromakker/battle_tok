// Editor 3D Mesh Shader
//
// Renders 3D meshes in the Asset Editor (stages 2-5) with basic
// Lambert diffuse + ambient lighting and vertex colors.
// Used with the orbit camera's view-projection matrix.

struct Uniforms {
    view_projection: mat4x4<f32>,
    light_dir: vec3<f32>,
    _pad: f32,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) normal: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_projection * vec4<f32>(in.position, 1.0);
    out.color = in.color;
    out.normal = in.normal;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let ambient = 0.3;
    let n = normalize(in.normal);
    let l = normalize(uniforms.light_dir);
    let diffuse = max(dot(n, l), 0.0);
    let lighting = ambient + (1.0 - ambient) * diffuse;
    return vec4<f32>(in.color.rgb * lighting, in.color.a);
}
