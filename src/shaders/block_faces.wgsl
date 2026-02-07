struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_dir: vec3<f32>,
    fog_density: f32,
    fog_color: vec3<f32>,
    ambient: f32,
    projectile_count: u32,
    _padding1: vec3<f32>,
    projectile_positions: array<vec4<f32>, 32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VertexInput {
    @location(0) uv: vec2<f32>,
    @location(1) pack0: u32,
    @location(2) pack1: u32,
    @location(3) color_rgba8: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

const CHUNK_SIZE_VOX: f32 = 12.0;
const VOXEL_SIZE: f32 = 0.25;

fn unpack_signed10(v: u32) -> i32 {
    return i32(v & 1023u) - 512;
}

fn unpack_rgba8(c: u32) -> vec4<f32> {
    let r = f32(c & 255u) / 255.0;
    let g = f32((c >> 8u) & 255u) / 255.0;
    let b = f32((c >> 16u) & 255u) / 255.0;
    let a = f32((c >> 24u) & 255u) / 255.0;
    return vec4<f32>(r, g, b, a);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let lx = f32(in.pack0 & 31u);
    let ly = f32((in.pack0 >> 5u) & 31u);
    let lz = f32((in.pack0 >> 10u) & 31u);
    let dir = (in.pack0 >> 15u) & 7u;
    let width = max(1.0, f32((in.pack0 >> 18u) & 31u));
    let height = max(1.0, f32((in.pack0 >> 23u) & 31u));

    let cx = unpack_signed10(in.pack1);
    let cy = unpack_signed10(in.pack1 >> 10u);
    let cz = unpack_signed10(in.pack1 >> 20u);

    var face_pos = vec3<f32>(lx, ly, lz);
    var normal = vec3<f32>(0.0, 1.0, 0.0);

    if (dir == 0u) {
        face_pos = vec3<f32>(lx, ly + in.uv.x * width, lz + in.uv.y * height);
        normal = vec3<f32>(1.0, 0.0, 0.0);
    } else if (dir == 1u) {
        face_pos = vec3<f32>(lx, ly + in.uv.x * width, lz + (1.0 - in.uv.y) * height);
        normal = vec3<f32>(-1.0, 0.0, 0.0);
    } else if (dir == 2u) {
        face_pos = vec3<f32>(lx + in.uv.x * width, ly, lz + in.uv.y * height);
        normal = vec3<f32>(0.0, 1.0, 0.0);
    } else if (dir == 3u) {
        face_pos = vec3<f32>(lx + in.uv.x * width, ly, lz + (1.0 - in.uv.y) * height);
        normal = vec3<f32>(0.0, -1.0, 0.0);
    } else if (dir == 4u) {
        face_pos = vec3<f32>(lx + in.uv.x * width, ly + in.uv.y * height, lz);
        normal = vec3<f32>(0.0, 0.0, 1.0);
    } else {
        face_pos = vec3<f32>(lx + (1.0 - in.uv.x) * width, ly + in.uv.y * height, lz);
        normal = vec3<f32>(0.0, 0.0, -1.0);
    }

    let chunk_origin_vox = vec3<f32>(f32(cx), f32(cy), f32(cz)) * CHUNK_SIZE_VOX;
    let world_pos = (chunk_origin_vox + face_pos) * VOXEL_SIZE;

    var out: VertexOutput;
    out.world_pos = world_pos;
    out.normal = normal;
    out.color = unpack_rgba8(in.color_rgba8);
    out.clip_position = uniforms.view_proj * vec4<f32>(world_pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let sun_dir = normalize(uniforms.sun_dir);
    let n_dot_l = max(dot(n, sun_dir), 0.0);
    let lit = in.color.rgb * (0.35 + uniforms.ambient * 0.45 + n_dot_l * 0.85);
    return vec4<f32>(lit, 1.0);
}
