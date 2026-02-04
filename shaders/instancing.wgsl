// Magic Engine - GPU Instancing Structures
//
// This shader module defines the CreatureInstance struct for GPU instanced rendering
// of up to 2000 creature entities. The layout matches the Rust CreatureInstance struct
// in src/engine/render/instancing.rs.

// ============================================================================
// CREATURE INSTANCE STRUCT
// ============================================================================
//
// CreatureInstance layout (48 bytes total):
// - position:        vec3<f32> @ offset 0   (12 bytes) - World position
// - _pad0:           u32       @ offset 12  (4 bytes)  - Padding for alignment
// - rotation:        vec4<f32> @ offset 16  (16 bytes) - Quaternion rotation (x, y, z, w)
// - scale:           f32       @ offset 32  (4 bytes)  - Uniform scale factor
// - baked_sdf_id:    u32       @ offset 36  (4 bytes)  - ID of the baked SDF model
// - animation_state: u32       @ offset 40  (4 bytes)  - Current animation state/frame
// - tint_color:      u32       @ offset 44  (4 bytes)  - Packed RGBA color (0xRRGGBBAA)
//
// Total: 12 + 4 + 16 + 4 + 4 + 4 + 4 = 48 bytes
//
// This struct is designed to be used as vertex instance data with step_mode = instance.
// The shader receives individual fields via vertex attributes at the following locations:
//   location 0: position (vec3<f32>)
//   location 1: rotation (vec4<f32>)
//   location 2: scale (f32)
//   location 3: baked_sdf_id (u32)
//   location 4: animation_state (u32)
//   location 5: tint_color (u32)

struct CreatureInstance {
    // Offset 0: World position (12 bytes)
    position: vec3<f32>,
    // Offset 12: Padding for 16-byte alignment of rotation (4 bytes)
    _pad0: u32,
    // Offset 16: Quaternion rotation (x, y, z, w) - 16 bytes
    rotation: vec4<f32>,
    // Offset 32: Uniform scale factor (4 bytes)
    scale: f32,
    // Offset 36: ID referencing a baked SDF model (4 bytes)
    baked_sdf_id: u32,
    // Offset 40: Animation state/frame identifier (4 bytes)
    animation_state: u32,
    // Offset 44: Packed RGBA tint color (0xRRGGBBAA format) (4 bytes)
    tint_color: u32,
    // Total: 48 bytes
}

// ============================================================================
// INSTANCE BUFFER
// ============================================================================
//
// The instance buffer holds up to MAX_CREATURE_INSTANCES (2000) instances.
// Total buffer size: 2000 * 48 bytes = 96,000 bytes = 96 KB

const MAX_CREATURE_INSTANCES: u32 = 2000u;

// Instance buffer binding (read-only storage buffer)
// Note: When using as storage buffer, bind to group 1, binding 0
// @group(1) @binding(0)
// var<storage, read> creature_instances: array<CreatureInstance>;

// ============================================================================
// UTILITY FUNCTIONS
// ============================================================================

// Unpack RGBA color from u32 (0xRRGGBBAA format) to vec4<f32> (0.0-1.0 range)
fn unpack_rgba(packed: u32) -> vec4<f32> {
    let r = f32((packed >> 24u) & 0xFFu) / 255.0;
    let g = f32((packed >> 16u) & 0xFFu) / 255.0;
    let b = f32((packed >> 8u) & 0xFFu) / 255.0;
    let a = f32(packed & 0xFFu) / 255.0;
    return vec4<f32>(r, g, b, a);
}

// Apply quaternion rotation to a point
fn rotate_by_quaternion(point: vec3<f32>, quat: vec4<f32>) -> vec3<f32> {
    // q = (x, y, z, w) where w is the scalar part
    let u = quat.xyz;
    let s = quat.w;

    // Rodrigues' rotation formula via quaternion
    // v' = v + 2.0 * s * (u × v) + 2.0 * (u × (u × v))
    let uv = cross(u, point);
    let uuv = cross(u, uv);
    return point + 2.0 * (s * uv + uuv);
}

// Transform a local-space point to world-space using instance data
fn transform_point(local_pos: vec3<f32>, instance: CreatureInstance) -> vec3<f32> {
    // Apply scale
    let scaled = local_pos * instance.scale;
    // Apply rotation
    let rotated = rotate_by_quaternion(scaled, instance.rotation);
    // Apply translation
    return rotated + instance.position;
}

// ============================================================================
// VERTEX INPUT STRUCT (for use with vertex attributes)
// ============================================================================
//
// When using instanced rendering with vertex attributes, the struct should be
// received in the vertex shader like this:
//
// struct InstanceInput {
//     @location(0) position: vec3<f32>,
//     @location(1) rotation: vec4<f32>,
//     @location(2) scale: f32,
//     @location(3) baked_sdf_id: u32,
//     @location(4) animation_state: u32,
//     @location(5) tint_color: u32,
// };
//
// @vertex
// fn vs_main(
//     @builtin(vertex_index) vertex_index: u32,
//     instance: InstanceInput,
// ) -> VertexOutput {
//     // Use instance.position, instance.rotation, etc.
// }
