// Handle Overlay Shader
// Renders rig handles as colored spheres, bones as lines, and manipulation gizmos

// ============================================================================
// UNIFORM BINDINGS
// ============================================================================

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    resolution: vec2<f32>,
    step_count: u32,
    lod_debug_mode: u32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// ============================================================================
// HANDLE STORAGE BUFFER
// ============================================================================

struct GpuHandle {
    position: vec3<f32>,
    handle_type: u32,
    color: vec3<f32>,
    selected: f32,
    radius: f32,
    id: u32,
    is_ik_target: u32,
    _padding: u32,
}

struct HandleBuffer {
    count: u32,
    _bone_count: u32,
    _padding: vec2<u32>,
    handles: array<GpuHandle>,
}

@group(0) @binding(1)
var<storage, read> handle_buffer: HandleBuffer;

// ============================================================================
// BONE STORAGE BUFFER
// ============================================================================

struct GpuBone {
    start_pos: vec3<f32>,
    chain_index: u32,
    end_pos: vec3<f32>,
    bone_index: u32,
    color: vec3<f32>,
    thickness: f32,
}

struct BoneBuffer {
    _handle_count: u32,
    count: u32,
    _padding: vec2<u32>,
    bones: array<GpuBone>,
}

@group(0) @binding(2)
var<storage, read> bone_buffer: BoneBuffer;

// ============================================================================
// CONSTANTS
// ============================================================================

// Selection highlight color (bright cyan)
const SELECTION_HIGHLIGHT: vec3<f32> = vec3<f32>(0.3, 1.0, 1.0);
// IK target highlight (bright magenta)
const IK_TARGET_HIGHLIGHT: vec3<f32> = vec3<f32>(1.0, 0.3, 0.8);

// ============================================================================
// HANDLE VERTEX SHADER
// Renders handles as billboarded quads with SDF spheres
// ============================================================================

struct HandleVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) selected: f32,
    @location(3) is_ik_target: f32,
    @location(4) world_pos: vec3<f32>,
}

@vertex
fn vs_handle(@builtin(vertex_index) vertex_index: u32) -> HandleVertexOutput {
    // Each handle uses 6 vertices (2 triangles) for a billboard quad
    let handle_index = vertex_index / 6u;
    let local_vertex = vertex_index % 6u;

    // Quad vertices (two triangles)
    var quad_positions = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 1.0,  1.0),
        vec2<f32>(-1.0,  1.0)
    );

    let h = handle_buffer.handles[handle_index];
    let quad_offset = quad_positions[local_vertex];

    // Calculate billboard orientation
    let to_camera = normalize(uniforms.camera_pos - h.position);
    let world_up = vec3<f32>(0.0, 1.0, 0.0);
    let right = normalize(cross(world_up, to_camera));
    let up = cross(to_camera, right);

    // Scale the quad by handle radius (with extra margin for anti-aliasing)
    let billboard_size = h.radius * 2.0;
    let world_offset = right * quad_offset.x * billboard_size + up * quad_offset.y * billboard_size;
    let world_pos = h.position + world_offset;

    var output: HandleVertexOutput;
    output.position = uniforms.view_proj * vec4<f32>(world_pos, 1.0);
    output.uv = quad_offset;
    output.color = h.color;
    output.selected = h.selected;
    output.is_ik_target = f32(h.is_ik_target);
    output.world_pos = h.position;

    return output;
}

@fragment
fn fs_handle(input: HandleVertexOutput) -> @location(0) vec4<f32> {
    // SDF circle for smooth sphere appearance
    let dist = length(input.uv);

    // Smooth edge with anti-aliasing
    let edge_softness = fwidth(dist) * 1.5;
    let alpha = 1.0 - smoothstep(0.8 - edge_softness, 0.8 + edge_softness, dist);

    if (alpha < 0.01) {
        discard;
    }

    // Base color
    var color = input.color;

    // Add highlight for IK targets
    if (input.is_ik_target > 0.5) {
        color = mix(color, IK_TARGET_HIGHLIGHT, 0.3);
    }

    // Selection highlight - bright ring around selected handles
    if (input.selected > 0.5) {
        let ring_inner = 0.6;
        let ring_outer = 0.75;
        let ring_dist = smoothstep(ring_inner, ring_inner + 0.05, dist) *
                        (1.0 - smoothstep(ring_outer, ring_outer + 0.05, dist));
        color = mix(color, SELECTION_HIGHLIGHT, ring_dist * 0.8);

        // Also brighten the whole handle slightly
        color = mix(color, SELECTION_HIGHLIGHT, 0.2);
    }

    // Simple lighting (hemisphere with view direction)
    let light_dir = normalize(vec3<f32>(0.3, 0.8, 0.5));
    let sphere_normal = normalize(vec3<f32>(input.uv.x, input.uv.y, sqrt(max(0.0, 1.0 - dist * dist))));
    let ndotl = max(dot(sphere_normal, light_dir), 0.0);
    let ambient = 0.4;
    let diffuse = ndotl * 0.6;

    let final_color = color * (ambient + diffuse);

    return vec4<f32>(final_color, alpha);
}

// ============================================================================
// BONE VERTEX SHADER
// Renders bones as lines between joints
// ============================================================================

struct BoneVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_bone(@builtin(vertex_index) vertex_index: u32) -> BoneVertexOutput {
    // Each bone uses 2 vertices (1 line)
    let bone_index = vertex_index / 2u;
    let is_end = vertex_index % 2u;

    let bone = bone_buffer.bones[bone_index];

    var world_pos: vec3<f32>;
    if (is_end == 0u) {
        world_pos = bone.start_pos;
    } else {
        world_pos = bone.end_pos;
    }

    var output: BoneVertexOutput;
    output.position = uniforms.view_proj * vec4<f32>(world_pos, 1.0);
    output.color = bone.color;

    return output;
}

@fragment
fn fs_bone(input: BoneVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}

// ============================================================================
// GIZMO VERTEX SHADER
// Renders manipulation gizmo lines with vertex colors
// ============================================================================

struct GizmoVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
}

@vertex
fn vs_gizmo(
    @location(0) position: vec3<f32>,
    @location(1) axis: u32,
    @location(2) color: vec3<f32>,
) -> GizmoVertexOutput {
    var output: GizmoVertexOutput;
    output.position = uniforms.view_proj * vec4<f32>(position, 1.0);
    output.color = color;

    return output;
}

@fragment
fn fs_gizmo(input: GizmoVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}
