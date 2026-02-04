// =============================================================================
// SDF Baking Compute Shader
// =============================================================================
// US-009: Bakes SDF equations into 64³ 3D textures for efficient runtime lookup.
//
// This compute shader evaluates an SDF primitive at each voxel position and
// stores the signed distance value in a 3D texture. The resulting texture
// can be sampled during ray marching instead of computing the SDF analytically,
// trading memory for computation time.
//
// Workgroup size: 8×8×8 = 512 threads
// Grid size: 64×64×64 = 262,144 voxels
// Dispatch: (64/8, 64/8, 64/8) = (8, 8, 8) workgroups = 512 workgroups total
//
// Performance target: <5ms per entity on RTX 4070 Ti
// =============================================================================

// =============================================================================
// CONSTANTS
// =============================================================================

// Voxel grid dimensions (64³)
const GRID_SIZE: u32 = 64u;
const GRID_SIZE_F: f32 = 64.0;

// SDF primitive type constants (matching raymarcher.wgsl)
const SDF_SPHERE: u32 = 0u;
const SDF_BOX: u32 = 1u;
const SDF_CAPSULE: u32 = 2u;
const SDF_ELLIPSOID: u32 = 3u;
const SDF_CYLINDER: u32 = 4u;
const SDF_TORUS: u32 = 5u;
const SDF_CONE: u32 = 6u;
const SDF_ROUNDED_BOX: u32 = 7u;

// =============================================================================
// BAKE PARAMETERS UNIFORM
// =============================================================================

// BakeParams: Configuration for baking a single SDF entity into a 3D texture
//
// Memory layout (112 bytes total):
// - position: vec3<f32> (12 bytes) @ offset 0   - Entity center position (for reference, not used in local space)
// - sdf_type: u32 (4 bytes) @ offset 12         - SDF primitive type
// - scale: vec3<f32> (12 bytes) @ offset 16     - Scale in each dimension
// - _pad0: u32 (4 bytes) @ offset 28            - Padding
// - rotation: vec4<f32> (16 bytes) @ offset 32  - Quaternion rotation (x, y, z, w)
// - bounds_min: vec3<f32> (12 bytes) @ offset 48 - Minimum bounds of voxel grid in local space
// - _pad1: u32 (4 bytes) @ offset 60            - Padding
// - bounds_max: vec3<f32> (12 bytes) @ offset 64 - Maximum bounds of voxel grid in local space
// - _pad2: u32 (4 bytes) @ offset 76            - Padding
// - noise_amplitude: f32 (4 bytes) @ offset 80  - Amplitude of noise displacement
// - noise_frequency: f32 (4 bytes) @ offset 84  - Frequency multiplier for noise
// - noise_octaves: u32 (4 bytes) @ offset 88    - Number of FBM octaves (1-8)
// - use_noise: u32 (4 bytes) @ offset 92        - Whether to apply noise (0 or 1)
// - brick_offset: u32 (4 bytes) @ offset 96     - Offset into brick_data buffer
// - _pad3a: u32 (4 bytes) @ offset 100          - Padding
// - _pad3b: u32 (4 bytes) @ offset 104          - Padding
// - _pad3c: u32 (4 bytes) @ offset 108          - Padding
//
struct BakeParams {
    // Entity position in world space (reference only)
    position: vec3<f32>,
    // SDF primitive type (0=sphere, 1=box, 2=capsule, etc.)
    sdf_type: u32,
    // Scale of the entity in each dimension
    scale: vec3<f32>,
    // Padding for alignment
    _pad0: u32,
    // Rotation quaternion (x, y, z, w)
    rotation: vec4<f32>,
    // Minimum bounds of the voxel grid in local space
    bounds_min: vec3<f32>,
    // Padding for alignment
    _pad1: u32,
    // Maximum bounds of the voxel grid in local space
    bounds_max: vec3<f32>,
    // Padding for alignment
    _pad2: u32,
    // Noise displacement amplitude
    noise_amplitude: f32,
    // Noise frequency multiplier
    noise_frequency: f32,
    // Number of FBM octaves (1-8)
    noise_octaves: u32,
    // Whether to apply noise displacement (0 = no, 1 = yes)
    use_noise: u32,
    // Offset into brick_data storage buffer for this brick
    brick_offset: u32,
    // Padding to maintain 16-byte alignment
    _pad3a: u32,
    _pad3b: u32,
    _pad3c: u32,
}

// =============================================================================
// BINDINGS
// =============================================================================

// Group 0, Binding 0: Bake parameters uniform buffer
@group(0) @binding(0)
var<uniform> params: BakeParams;

// Group 0, Binding 1: Output storage buffer for brick data
// Stores signed distance values as a flat array of f32
@group(0) @binding(1)
var<storage, read_write> brick_data: array<f32>;

// =============================================================================
// HASH FUNCTIONS (for noise generation)
// =============================================================================

// 3D to 3D hash function for gradient noise
fn hash33(p: vec3<f32>) -> vec3<f32> {
    var p3 = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

// =============================================================================
// NOISE FUNCTIONS
// =============================================================================

// 3D Gradient noise with smoothstep interpolation
fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Smoothstep interpolation (3t² - 2t³)
    let u = f * f * (3.0 - 2.0 * f);

    // Sample gradients at 8 corners and compute dot products
    return mix(
        mix(
            mix(dot(hash33(i + vec3<f32>(0.0, 0.0, 0.0)) * 2.0 - 1.0, f - vec3<f32>(0.0, 0.0, 0.0)),
                dot(hash33(i + vec3<f32>(1.0, 0.0, 0.0)) * 2.0 - 1.0, f - vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(dot(hash33(i + vec3<f32>(0.0, 1.0, 0.0)) * 2.0 - 1.0, f - vec3<f32>(0.0, 1.0, 0.0)),
                dot(hash33(i + vec3<f32>(1.0, 1.0, 0.0)) * 2.0 - 1.0, f - vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(dot(hash33(i + vec3<f32>(0.0, 0.0, 1.0)) * 2.0 - 1.0, f - vec3<f32>(0.0, 0.0, 1.0)),
                dot(hash33(i + vec3<f32>(1.0, 0.0, 1.0)) * 2.0 - 1.0, f - vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(dot(hash33(i + vec3<f32>(0.0, 1.0, 1.0)) * 2.0 - 1.0, f - vec3<f32>(0.0, 1.0, 1.0)),
                dot(hash33(i + vec3<f32>(1.0, 1.0, 1.0)) * 2.0 - 1.0, f - vec3<f32>(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

// Fractal Brownian Motion with configurable octaves
fn fbm(p: vec3<f32>, octaves: u32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;

    let max_octaves = min(octaves, 8u);
    for (var i = 0u; i < max_octaves; i = i + 1u) {
        value += amplitude * noise(pos * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    return value;
}

// =============================================================================
// QUATERNION OPERATIONS
// =============================================================================

// Rotate a vector by a quaternion
fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let u = q.xyz;
    let s = q.w;
    return 2.0 * dot(u, v) * u
         + (s * s - dot(u, u)) * v
         + 2.0 * s * cross(u, v);
}

// Inverse rotate (conjugate quaternion)
fn quat_rotate_inv(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let q_conj = vec4<f32>(-q.xyz, q.w);
    return quat_rotate(q_conj, v);
}

// =============================================================================
// SDF PRIMITIVE FUNCTIONS
// =============================================================================

// Signed distance to a sphere centered at origin
fn sdf_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

// Signed distance to an axis-aligned box centered at origin
fn sdf_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Signed distance to a vertical capsule centered at origin
fn sdf_capsule(p: vec3<f32>, h: f32, r: f32) -> f32 {
    let half_h = h * 0.5;
    let p_clamped = vec3<f32>(p.x, clamp(p.y, -half_h, half_h), p.z);
    return length(p - p_clamped) - r;
}

// Signed distance to an ellipsoid centered at origin
fn sdf_ellipsoid(p: vec3<f32>, r: vec3<f32>) -> f32 {
    let k0 = length(p / r);
    let k1 = length(p / (r * r));
    return k0 * (k0 - 1.0) / k1;
}

// Signed distance to a cylinder centered at origin, aligned along Y axis
fn sdf_cylinder(p: vec3<f32>, h: f32, r: f32) -> f32 {
    let d = abs(vec2<f32>(length(p.xz), p.y)) - vec2<f32>(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

// Signed distance to a torus centered at origin, lying in the XZ plane
fn sdf_torus(p: vec3<f32>, t: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

// Signed distance to a cone with tip at origin, opening along +Y
fn sdf_cone(p: vec3<f32>, c: vec2<f32>, h: f32) -> f32 {
    let q = h * vec2<f32>(c.x / c.y, -1.0);
    let w = vec2<f32>(length(p.xz), p.y);
    let a = w - q * clamp(dot(w, q) / dot(q, q), 0.0, 1.0);
    let b = w - q * vec2<f32>(clamp(w.x / q.x, 0.0, 1.0), 1.0);
    let k = sign(q.y);
    let d = min(dot(a, a), dot(b, b));
    let s = max(k * (w.x * q.y - w.y * q.x), k * (w.y - q.y));
    return sqrt(d) * sign(s);
}

// Signed distance to a rounded box centered at origin
fn sdf_rounded_box(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

// =============================================================================
// SDF EVALUATION
// =============================================================================

// Evaluate the SDF for the current entity at a given local-space position
fn evaluate_sdf(local_p: vec3<f32>) -> f32 {
    var d: f32;

    // Get scale - use uniform scale (x component) for sphere/capsule,
    // full vec3 for box/ellipsoid
    let scale = params.scale;

    switch (params.sdf_type) {
        case SDF_SPHERE: {
            // Sphere: scale.x is radius
            d = sdf_sphere(local_p, scale.x);
        }
        case SDF_BOX: {
            // Box: scale is half-extents
            d = sdf_box(local_p, scale);
        }
        case SDF_CAPSULE: {
            // Capsule: scale.x is radius, scale.y is height
            d = sdf_capsule(local_p, scale.y, scale.x);
        }
        case SDF_ELLIPSOID: {
            // Ellipsoid: scale is radii in each dimension
            d = sdf_ellipsoid(local_p, scale);
        }
        case SDF_CYLINDER: {
            // Cylinder: scale.x is radius, scale.y is half-height
            d = sdf_cylinder(local_p, scale.y, scale.x);
        }
        case SDF_TORUS: {
            // Torus: scale.x is major radius, scale.y is minor radius
            d = sdf_torus(local_p, vec2<f32>(scale.x, scale.y));
        }
        case SDF_CONE: {
            // Cone: scale.x is base radius, scale.y is height
            // Calculate sin/cos of half-angle: tan(angle) = radius/height
            let angle = atan2(scale.x, scale.y);
            let c = vec2<f32>(sin(angle), cos(angle));
            d = sdf_cone(local_p, c, scale.y);
        }
        case SDF_ROUNDED_BOX: {
            // Rounded box: scale.x/y/z is inner half-extents, rounded by min(scale) * 0.2
            let corner_radius = min(min(scale.x, scale.y), scale.z) * 0.2;
            d = sdf_rounded_box(local_p, scale - vec3<f32>(corner_radius), corner_radius);
        }
        default: {
            // Default to sphere
            d = sdf_sphere(local_p, scale.x);
        }
    }

    // Apply noise displacement if enabled
    if (params.use_noise != 0u) {
        let noise_pos = local_p * params.noise_frequency;
        let displacement = fbm(noise_pos, params.noise_octaves) * params.noise_amplitude;
        d = d + displacement;
    }

    return d;
}

// =============================================================================
// COMPUTE SHADER MAIN ENTRY POINT
// =============================================================================

// Workgroup size: 4×4×4 = 64 threads
// Kept at 64 to stay within llvmpipe's 256 max invocations limit
// Covers the 64³ grid with 16³ = 4096 workgroups
@compute @workgroup_size(4, 4, 4)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Check bounds - skip if outside the 64³ grid
    if (global_id.x >= GRID_SIZE || global_id.y >= GRID_SIZE || global_id.z >= GRID_SIZE) {
        return;
    }

    // Calculate normalized coordinates (0.0 to 1.0) for this voxel
    // Add 0.5 to sample at voxel centers
    let normalized = (vec3<f32>(global_id) + vec3<f32>(0.5)) / GRID_SIZE_F;

    // Map normalized coordinates to local-space position within bounds
    // bounds_min and bounds_max define the volume in local (entity) space
    let local_p = mix(params.bounds_min, params.bounds_max, normalized);

    // Evaluate the SDF at this position
    let distance = evaluate_sdf(local_p);

    // Store the signed distance value in the brick data buffer
    // Index formula: brick_offset + z * 4096 + y * 64 + x
    let index = params.brick_offset + global_id.z * 4096u + global_id.y * 64u + global_id.x;
    brick_data[index] = distance;
}
