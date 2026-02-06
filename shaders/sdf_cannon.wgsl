// SDF Cannon Shader
// Ray marches against cannon SDF primitives to render the siege cannon
// with smooth curves via signed distance fields.
//
// This shader renders a fullscreen quad and traces rays through each pixel
// to find intersections with the cannon SDF (cylinder barrel + rounded box body).

// ============================================================================
// UNIFORM BINDINGS
// ============================================================================

struct Uniforms {
    view_proj: mat4x4<f32>,          // View-projection matrix
    inv_view_proj: mat4x4<f32>,      // Inverse view-projection for ray direction
    camera_pos: vec3<f32>,           // Camera world position
    time: f32,                       // Animation time
    sun_dir: vec3<f32>,              // Sun direction (normalized)
    fog_density: f32,                // Fog density factor
    fog_color: vec3<f32>,            // Fog color (matches terrain)
    ambient: f32,                    // Ambient light strength
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// ============================================================================
// CANNON SDF UNIFORM
// ============================================================================

struct CannonData {
    // Transform (world position and rotation)
    world_pos: vec3<f32>,
    _pad0: f32,
    // Barrel rotation quaternion
    barrel_rotation: vec4<f32>,      // Quaternion (x, y, z, w)
    // Cannon color
    color: vec3<f32>,
    _pad1: f32,
}

@group(0) @binding(1)
var<uniform> cannon: CannonData;

// ============================================================================
// CONSTANTS
// ============================================================================

// Ray marching parameters
const MAX_STEPS: u32 = 64u;
const MAX_DIST: f32 = 500.0;
const SURFACE_DIST: f32 = 0.01;  // Surface hit threshold

// Cannon dimensions (must match cannon.rs)
const BARREL_RADIUS: f32 = 0.45;
const BARREL_LENGTH: f32 = 4.0;
const BODY_HALF_X: f32 = 1.25;    // Half-extent X (total 2.5)
const BODY_HALF_Y: f32 = 0.70;    // Half-extent Y (total 1.4)
const BODY_HALF_Z: f32 = 1.10;    // Half-extent Z (total 2.2)
const BODY_ROUNDING: f32 = 0.18;  // Edge rounding radius
const WHEEL_RADIUS: f32 = 0.42;
const SMOOTH_K: f32 = 0.28;       // Smooth union blending factor

// Material color (bronze/iron cannon)
const CANNON_COLOR: vec3<f32> = vec3<f32>(0.45, 0.35, 0.25);

// ============================================================================
// QUATERNION OPERATIONS
// ============================================================================

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

// ============================================================================
// SDF PRIMITIVE FUNCTIONS
// ============================================================================

// Signed distance to a sphere centered at origin
fn sdf_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

// Signed distance to an axis-aligned box centered at origin
// b: half-extents (box dimensions / 2)
fn sdf_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Signed distance to a rounded box (box with rounded edges)
// b: half-extents (inner box size)
// r: rounding radius
fn sdf_rounded_box(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - b + r;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

// Signed distance to a cylinder aligned along Y-axis, centered at origin
// h: half-height
// r: radius
fn sdf_cylinder(p: vec3<f32>, h: f32, r: f32) -> f32 {
    let d = abs(vec2<f32>(length(p.xz), p.y)) - vec2<f32>(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

// Signed distance to a capped cylinder between two points
// Useful for arbitrarily oriented cylinders
fn sdf_capped_cylinder_ab(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let ba = b - a;
    let pa = p - a;
    let baba = dot(ba, ba);
    let paba = dot(pa, ba);
    let x = length(pa * baba - ba * paba) - r * baba;
    let y = abs(paba - baba * 0.5) - baba * 0.5;
    let x2 = x * x;
    let y2 = y * y * baba;
    let d = select(
        select(0.0, x2, x > 0.0) + select(0.0, y2, y > 0.0),
        -min(x2, y2),
        max(x, y) < 0.0
    );
    return sign(d) * sqrt(abs(d)) / baba;
}

// ============================================================================
// SDF OPERATIONS
// ============================================================================

// Smooth minimum (polynomial smooth min)
// Creates smooth blending/union between two SDF values
// k: smoothness factor (larger = smoother blend, 0 = hard union)
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 0.0) {
        return min(a, b);
    }
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// Hard union
fn op_union(a: f32, b: f32) -> f32 {
    return min(a, b);
}

// ============================================================================
// CANNON SDF EVALUATION
// ============================================================================

// Evaluate the cannon SDF at a world-space point
fn sdf_cannon(world_p: vec3<f32>) -> f32 {
    // Transform world point to cannon local space
    let local_p = world_p - cannon.world_pos;

    // Carriage/body
    let body_center = vec3<f32>(0.0, BODY_HALF_Y * 0.95, 0.0);
    let body_local = local_p - body_center;
    let body_d = sdf_rounded_box(
        body_local,
        vec3<f32>(BODY_HALF_X, BODY_HALF_Y, BODY_HALF_Z),
        BODY_ROUNDING,
    );

    // Wheel hubs
    let wheel_y = 0.30;
    let wheel_offset_x = BODY_HALF_X - 0.12;
    let wheel_l = sdf_sphere(local_p - vec3<f32>(-wheel_offset_x, wheel_y, 0.0), WHEEL_RADIUS);
    let wheel_r = sdf_sphere(local_p - vec3<f32>( wheel_offset_x, wheel_y, 0.0), WHEEL_RADIUS);

    // Barrel from attachment point along cannon-forward axis
    let barrel_attach = vec3<f32>(0.0, BODY_HALF_Y + 0.08, BODY_HALF_Z * 0.45);
    let barrel_dir = normalize(quat_rotate(cannon.barrel_rotation, vec3<f32>(0.0, 0.0, -1.0)));
    let barrel_start = barrel_attach + barrel_dir * 0.20;
    let barrel_end = barrel_start + barrel_dir * BARREL_LENGTH;
    let barrel_d = sdf_capped_cylinder_ab(local_p, barrel_start, barrel_end, BARREL_RADIUS);

    let breech_center = barrel_attach - barrel_dir * 0.18;
    let breech_d = sdf_sphere(local_p - breech_center, BARREL_RADIUS * 1.25);

    var d = smooth_min(body_d, barrel_d, SMOOTH_K);
    d = smooth_min(d, breech_d, SMOOTH_K * 0.8);
    d = min(d, wheel_l);
    d = min(d, wheel_r);
    return d;
}

// ============================================================================
// NORMAL CALCULATION VIA GRADIENT ESTIMATION
// ============================================================================

// Calculate surface normal using gradient estimation
// (central differences method)
fn calc_normal(p: vec3<f32>) -> vec3<f32> {
    let e = 0.001; // Small epsilon for gradient estimation
    let d = sdf_cannon(p);
    let n = vec3<f32>(
        sdf_cannon(p + vec3<f32>(e, 0.0, 0.0)) - sdf_cannon(p - vec3<f32>(e, 0.0, 0.0)),
        sdf_cannon(p + vec3<f32>(0.0, e, 0.0)) - sdf_cannon(p - vec3<f32>(0.0, e, 0.0)),
        sdf_cannon(p + vec3<f32>(0.0, 0.0, e)) - sdf_cannon(p - vec3<f32>(0.0, 0.0, e))
    );
    return normalize(n);
}

// ============================================================================
// RAY MARCHING
// ============================================================================

struct RayResult {
    hit: bool,
    position: vec3<f32>,
    distance: f32,
}

fn ray_march(origin: vec3<f32>, direction: vec3<f32>) -> RayResult {
    var result: RayResult;
    result.hit = false;
    result.position = origin;
    result.distance = 0.0;

    var t: f32 = 0.0;

    for (var i: u32 = 0u; i < MAX_STEPS; i = i + 1u) {
        let p = origin + direction * t;
        let d = sdf_cannon(p);

        if (d < SURFACE_DIST) {
            result.hit = true;
            result.position = p;
            result.distance = t;
            return result;
        }

        if (t > MAX_DIST) {
            break;
        }

        // Adaptive step: use SDF distance but clamp to reasonable range
        t = t + max(d, 0.01);
    }

    result.distance = t;
    return result;
}

// ============================================================================
// LIGHTING
// ============================================================================

// Lambert diffuse lighting
fn calc_lighting(pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
    let sun_dir = normalize(uniforms.sun_dir);

    // Lambert diffuse
    let ndotl = max(dot(normal, sun_dir), 0.0);
    let diffuse = ndotl * 0.7;

    // Ambient
    let ambient = uniforms.ambient;

    // Hemisphere ambient (sky contribution from above)
    let sky_factor = (normal.y + 1.0) * 0.5;
    let hemisphere_ambient = mix(0.2, 0.4, sky_factor);

    // Combined lighting
    let lighting = ambient + diffuse + hemisphere_ambient * 0.3;

    // Apply to cannon color
    var color = cannon.color * lighting;

    // Add subtle rim lighting
    let view_dir = normalize(uniforms.camera_pos - pos);
    let rim = pow(1.0 - max(dot(view_dir, normal), 0.0), 3.0);
    color = color + vec3<f32>(0.1, 0.12, 0.15) * rim * 0.5;

    return color;
}

// ============================================================================
// RAY DIRECTION FROM SCREEN UV
// ============================================================================

fn get_ray_direction(uv: vec2<f32>) -> vec3<f32> {
    // Convert UV [0,1] to NDC [-1,1]
    let ndc_x = uv.x * 2.0 - 1.0;
    let ndc_y = 1.0 - uv.y * 2.0;  // Flip Y for clip space

    // Use two points on the ray (near and far planes) to compute direction
    let near_clip = vec4<f32>(ndc_x, ndc_y, -1.0, 1.0);
    let far_clip = vec4<f32>(ndc_x, ndc_y, 1.0, 1.0);

    // Transform both points from clip space to world space
    let near_world_h = uniforms.inv_view_proj * near_clip;
    let far_world_h = uniforms.inv_view_proj * far_clip;

    // Perspective divide
    let near_world = near_world_h.xyz / near_world_h.w;
    let far_world = far_world_h.xyz / far_world_h.w;

    // Ray direction is from near to far
    return normalize(far_world - near_world);
}

// ============================================================================
// VERTEX SHADER - FULLSCREEN TRIANGLE
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle (covers entire screen with one triangle)
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );

    var output: VertexOutput;
    output.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    output.uv = positions[vertex_index] * 0.5 + 0.5;
    output.uv.y = 1.0 - output.uv.y;  // Flip Y for screen coords
    return output;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

struct FragmentOutput {
    @location(0) color: vec4<f32>,
    @builtin(frag_depth) depth: f32,
}

@fragment
fn fs_main(input: VertexOutput) -> FragmentOutput {
    let uv = input.uv;

    // Get ray origin and direction
    let ray_origin = uniforms.camera_pos;
    let ray_dir = get_ray_direction(uv);

    // Ray march against cannon SDF
    let result = ray_march(ray_origin, ray_dir);

    if (!result.hit) {
        // Miss - return transparent (allows terrain to show through)
        discard;
    }

    // Calculate surface normal
    let normal = calc_normal(result.position);

    // Calculate lighting
    var color = calc_lighting(result.position, normal);

    // Apply fog (match terrain fog)
    let distance = length(uniforms.camera_pos - result.position);
    let fog_amount = 1.0 - exp(-distance * uniforms.fog_density);
    color = mix(color, uniforms.fog_color, fog_amount);

    // Calculate proper depth value for depth buffer interaction
    // Transform hit position to clip space to get correct depth
    let clip_pos = uniforms.view_proj * vec4<f32>(result.position, 1.0);
    let ndc_depth = clip_pos.z / clip_pos.w;  // Normalized device coordinates depth
    
    var output: FragmentOutput;
    output.color = vec4<f32>(color, 1.0);
    output.depth = ndc_depth;  // Write depth for proper occlusion
    return output;
}
