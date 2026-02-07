struct VoxelShellUniforms {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad0: f32,
    resolution: vec2<f32>,
    node_count: u32,
    leaf_count: u32,
    sun_dir: vec3<f32>,
    _pad1: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: VoxelShellUniforms;

// Raw u32 word streams uploaded from BrickNode/BrickLeaf64 arrays.
@group(0) @binding(1)
var<storage, read> node_words: array<u32>;
@group(0) @binding(2)
var<storage, read> leaf_words: array<u32>;

const VOXEL_SIZE: f32 = 0.25;
const CHUNK_EDGE: i32 = 16;
const LEAF_STRIDE_WORDS: u32 = 130u;
const NODE_STRIDE_WORDS: u32 = 6u;

struct VSOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

struct FSOut {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

fn decode_chunk_coord(lod_meta: u32) -> vec3<i32> {
    let x = i32(lod_meta & 0x3ffu) - 512;
    let y = i32((lod_meta >> 10u) & 0x3ffu) - 512;
    let z = i32((lod_meta >> 20u) & 0x3ffu) - 512;
    return vec3<i32>(x, y, z);
}

fn pack_chunk_coord(chunk: vec3<i32>) -> u32 {
    let x = u32(clamp(chunk.x, -512, 511) + 512) & 0x3ffu;
    let y = u32(clamp(chunk.y, -512, 511) + 512) & 0x3ffu;
    let z = u32(clamp(chunk.z, -512, 511) + 512) & 0x3ffu;
    return x | (y << 10u) | (z << 20u);
}

fn mask_has_bit(lo: u32, hi: u32, idx: u32) -> bool {
    if (idx < 32u) {
        return ((lo >> idx) & 1u) != 0u;
    }
    return ((hi >> (idx - 32u)) & 1u) != 0u;
}

fn mask_prefix_count(lo: u32, hi: u32, idx: u32) -> u32 {
    if (idx == 0u) {
        return 0u;
    }
    if (idx < 32u) {
        let mask = (1u << idx) - 1u;
        return countOneBits(lo & mask);
    }
    if (idx == 32u) {
        return countOneBits(lo);
    }
    let hi_bits = idx - 32u;
    let hi_mask = (1u << hi_bits) - 1u;
    return countOneBits(lo) + countOneBits(hi & hi_mask);
}

fn div_floor_16(v: i32) -> i32 {
    var q = v / CHUNK_EDGE;
    if ((v % CHUNK_EDGE) < 0) {
        q = q - 1;
    }
    return q;
}

fn local_voxel_coord(v: i32) -> i32 {
    let q = div_floor_16(v);
    return v - q * CHUNK_EDGE;
}

fn read_leaf_byte(leaf_start_word: u32, byte_offset: u32) -> u32 {
    let wi = leaf_start_word + (byte_offset / 4u);
    let shift = (byte_offset % 4u) * 8u;
    return (leaf_words[wi] >> shift) & 0xffu;
}

fn decode_oct_normal(oct_xy: vec2<u32>) -> vec3<f32> {
    let ex = (f32(oct_xy.x) / 255.0) * 2.0 - 1.0;
    let ey = (f32(oct_xy.y) / 255.0) * 2.0 - 1.0;
    var v = vec3<f32>(ex, ey, 1.0 - abs(ex) - abs(ey));
    if (v.z < 0.0) {
        let nx = (1.0 - abs(v.y)) * select(-1.0, 1.0, v.x >= 0.0);
        let ny = (1.0 - abs(v.x)) * select(-1.0, 1.0, v.y >= 0.0);
        v.x = nx;
        v.y = ny;
    }
    return normalize(v);
}

fn find_node_index(chunk: vec3<i32>) -> u32 {
    if (uniforms.node_count == 0u) {
        return 0xffffffffu;
    }
    let key = pack_chunk_coord(chunk);
    var lo: u32 = 0u;
    var hi: u32 = uniforms.node_count;
    loop {
        if (lo >= hi) {
            break;
        }
        let mid = (lo + hi) / 2u;
        let node_base = mid * NODE_STRIDE_WORDS;
        let mid_key = node_words[node_base + 4u];
        if (mid_key < key) {
            lo = mid + 1u;
        } else {
            hi = mid;
        }
    }
    if (lo < uniforms.node_count) {
        let node_base = lo * NODE_STRIDE_WORDS;
        if (node_words[node_base + 4u] == key) {
            return lo;
        }
    }
    return 0xffffffffu;
}

struct VoxelQuery {
    occupied: bool,
    color: vec3<f32>,
    normal: vec3<f32>,
}

fn query_voxel(world_voxel: vec3<i32>) -> VoxelQuery {
    let chunk = vec3<i32>(
        div_floor_16(world_voxel.x),
        div_floor_16(world_voxel.y),
        div_floor_16(world_voxel.z)
    );
    let local = vec3<u32>(
        u32(local_voxel_coord(world_voxel.x)),
        u32(local_voxel_coord(world_voxel.y)),
        u32(local_voxel_coord(world_voxel.z))
    );

    let node_index = find_node_index(chunk);

    if (node_index == 0xffffffffu) {
        return VoxelQuery(false, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0));
    }

    let node_base = node_index * NODE_STRIDE_WORDS;
    let child_mask_lo = node_words[node_base + 0u];
    let child_mask_hi = node_words[node_base + 1u];
    let leaf_payload_index = node_words[node_base + 3u];

    let child_x = local.x / 4u;
    let child_y = local.y / 4u;
    let child_z = local.z / 4u;
    let child_idx = child_x + child_y * 4u + child_z * 16u;
    if (!mask_has_bit(child_mask_lo, child_mask_hi, child_idx)) {
        return VoxelQuery(false, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0));
    }

    let child_rank = mask_prefix_count(child_mask_lo, child_mask_hi, child_idx);
    let leaf_index = leaf_payload_index + child_rank;
    if (leaf_index >= uniforms.leaf_count) {
        return VoxelQuery(false, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0));
    }

    let sub_x = local.x % 4u;
    let sub_y = local.y % 4u;
    let sub_z = local.z % 4u;
    let sub_idx = sub_x + sub_y * 4u + sub_z * 16u;
    let leaf_base = leaf_index * LEAF_STRIDE_WORDS;
    let occ_lo = leaf_words[leaf_base + 0u];
    let occ_hi = leaf_words[leaf_base + 1u];
    if (!mask_has_bit(occ_lo, occ_hi, sub_idx)) {
        return VoxelQuery(false, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0));
    }

    let color_byte_base = 72u + sub_idx * 3u;
    let r = read_leaf_byte(leaf_base, color_byte_base + 0u);
    let g = read_leaf_byte(leaf_base, color_byte_base + 1u);
    let b = read_leaf_byte(leaf_base, color_byte_base + 2u);

    let normal_byte_base = 264u + sub_idx * 2u;
    let nx = read_leaf_byte(leaf_base, normal_byte_base + 0u);
    let ny = read_leaf_byte(leaf_base, normal_byte_base + 1u);

    return VoxelQuery(
        true,
        vec3<f32>(f32(r), f32(g), f32(b)) / 255.0,
        decode_oct_normal(vec2<u32>(nx, ny))
    );
}

fn get_ray_direction(uv: vec2<f32>) -> vec3<f32> {
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
    let near_clip = vec4<f32>(ndc.x, ndc.y, 0.0, 1.0);
    let far_clip = vec4<f32>(ndc.x, ndc.y, 1.0, 1.0);
    let near_world4 = uniforms.inv_view_proj * near_clip;
    let far_world4 = uniforms.inv_view_proj * far_clip;
    let near_world = near_world4.xyz / near_world4.w;
    let far_world = far_world4.xyz / far_world4.w;
    return normalize(far_world - near_world);
}

fn sign_i(v: f32) -> i32 {
    if (v > 0.0) {
        return 1;
    }
    if (v < 0.0) {
        return -1;
    }
    return 0;
}

fn dda_t_delta(dir_component: f32) -> f32 {
    if (abs(dir_component) < 1e-6) {
        return 1e20;
    }
    return VOXEL_SIZE / abs(dir_component);
}

fn dda_t_max(origin_component: f32, dir_component: f32, cell: i32, step: i32) -> f32 {
    if (step == 0 || abs(dir_component) < 1e-6) {
        return 1e20;
    }
    let boundary = select(
        f32(cell) * VOXEL_SIZE,
        (f32(cell) + 1.0) * VOXEL_SIZE,
        step > 0
    );
    return (boundary - origin_component) / dir_component;
}

@vertex
fn vs_main(@builtin(vertex_index) vid: u32) -> VSOut {
    var out: VSOut;
    let x = f32((vid << 1u) & 2u);
    let y = f32(vid & 2u);
    out.uv = vec2<f32>(x, y);
    out.position = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

@fragment
fn fs_main(in: VSOut) -> FSOut {
    var out: FSOut;
    let ray_origin = uniforms.camera_pos;
    let ray_dir = get_ray_direction(in.uv);

    var cell = vec3<i32>(floor(ray_origin / VOXEL_SIZE));
    let step = vec3<i32>(sign_i(ray_dir.x), sign_i(ray_dir.y), sign_i(ray_dir.z));
    var t_max_x = dda_t_max(ray_origin.x, ray_dir.x, cell.x, step.x);
    var t_max_y = dda_t_max(ray_origin.y, ray_dir.y, cell.y, step.y);
    var t_max_z = dda_t_max(ray_origin.z, ray_dir.z, cell.z, step.z);
    let t_delta_x = dda_t_delta(ray_dir.x);
    let t_delta_y = dda_t_delta(ray_dir.y);
    let t_delta_z = dda_t_delta(ray_dir.z);

    var t = 0.0;
    let max_dist = 128.0;
    for (var i: u32 = 0u; i < 1024u; i = i + 1u) {
        if (t > max_dist) {
            discard;
        }

        let q = query_voxel(cell);
        if (q.occupied) {
            let hit_pos = ray_origin + ray_dir * t;
            let n = normalize(q.normal);
            let sun = normalize(uniforms.sun_dir);
            let ndotl = max(dot(n, sun), 0.0);
            let ambient = 0.30;
            let lit = q.color * (ambient + ndotl * 0.95);

            let clip = uniforms.view_proj * vec4<f32>(hit_pos, 1.0);
            let depth = clip.z / clip.w;
            if (depth < 0.0 || depth > 1.0) {
                discard;
            }

            out.color = vec4<f32>(lit, 1.0);
            out.depth = depth;
            return out;
        }

        if (t_max_x <= t_max_y && t_max_x <= t_max_z) {
            cell.x = cell.x + step.x;
            t = t_max_x;
            t_max_x = t_max_x + t_delta_x;
        } else if (t_max_y <= t_max_x && t_max_y <= t_max_z) {
            cell.y = cell.y + step.y;
            t = t_max_y;
            t_max_y = t_max_y + t_delta_y;
        } else {
            cell.z = cell.z + step.z;
            t = t_max_z;
            t_max_z = t_max_z + t_delta_z;
        }
    }

    discard;
}
