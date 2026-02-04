// =============================================================================
// SDF Primitives Module for Magic Engine
// =============================================================================
// This module provides signed distance functions (SDFs) for basic 3D primitives
// and operations for combining them into complex scenes.
//
// Usage: Import these functions into your shader using WGSL module pattern.
// Note: WGSL does not yet support true module imports, so this file serves as
// a reference module that can be included via preprocessor or copy-paste.
// =============================================================================

// =============================================================================
// SDF PRIMITIVE FUNCTIONS
// =============================================================================

/// Signed distance to a sphere centered at origin
/// p: point to evaluate
/// r: radius of sphere
/// Returns: signed distance (negative inside, positive outside)
fn sdf_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

/// Signed distance to an ellipsoid centered at origin
/// p: point to evaluate
/// r: vec3 containing radii (rx, ry, rz) along each axis
/// Returns: signed distance (approximate, more accurate for low eccentricity)
fn sdf_ellipsoid(p: vec3<f32>, r: vec3<f32>) -> f32 {
    let k0 = length(p / r);
    let k1 = length(p / (r * r));
    return k0 * (k0 - 1.0) / k1;
}

/// Signed distance to an axis-aligned box centered at origin
/// p: point to evaluate
/// b: half-extents (box dimensions / 2)
/// Returns: signed distance
fn sdf_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

/// Signed distance to a rounded box centered at origin
/// p: point to evaluate
/// b: half-extents of the inner box
/// r: corner radius
/// Returns: signed distance
fn sdf_rounded_box(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

/// Signed distance to a capsule between two arbitrary points
/// p: point to evaluate
/// a: start point of capsule axis
/// b: end point of capsule axis
/// r: radius of the capsule
/// Returns: signed distance
fn sdf_capsule(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

/// Signed distance to a vertical capsule centered at origin
/// p: point to evaluate
/// h: height of the cylindrical part (total height = h + 2*r)
/// r: radius of the capsule
/// Returns: signed distance
fn sdf_capsule_vertical(p: vec3<f32>, h: f32, r: f32) -> f32 {
    // Capsule aligned along Y axis, centered at origin
    let half_h = h * 0.5;
    let pa = p - vec3<f32>(0.0, -half_h, 0.0);
    let ba = vec3<f32>(0.0, h, 0.0);
    let t = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * t) - r;
}

/// Signed distance to a cylinder centered at origin, aligned along Y axis
/// p: point to evaluate
/// h: half-height of the cylinder
/// r: radius of the cylinder
/// Returns: signed distance
fn sdf_cylinder(p: vec3<f32>, h: f32, r: f32) -> f32 {
    let d = abs(vec2<f32>(length(p.xz), p.y)) - vec2<f32>(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

/// Signed distance to a capped cylinder with arbitrary orientation
/// p: point to evaluate
/// a: start point of cylinder axis
/// b: end point of cylinder axis
/// r: radius of the cylinder
/// Returns: signed distance
fn sdf_capped_cylinder(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let ba = b - a;
    let pa = p - a;
    let baba = dot(ba, ba);
    let paba = dot(pa, ba);
    let x = length(pa * baba - ba * paba) - r * baba;
    let y = abs(paba - baba * 0.5) - baba * 0.5;
    let x2 = x * x;
    let y2 = y * y * baba;
    let d = select(
        select(0.0, y2, y > 0.0),
        select(x2, x2 + y2, y > 0.0),
        x > 0.0
    );
    let s = select(1.0, -1.0, max(x, y) < 0.0);
    return s * sqrt(d) / baba;
}

/// Signed distance to a torus centered at origin, lying in the XZ plane
/// p: point to evaluate
/// t: vec2(major_radius, minor_radius)
/// Returns: signed distance
fn sdf_torus(p: vec3<f32>, t: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

/// Signed distance to a cone with tip at origin, opening along +Y
/// p: point to evaluate
/// c: vec2(sin(angle), cos(angle)) where angle is half the cone angle
/// h: height of the cone
/// Returns: signed distance
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

/// Signed distance to an infinite plane
/// p: point to evaluate
/// n: normal of the plane (must be normalized)
/// h: height/distance of plane from origin along normal
/// Returns: signed distance
fn sdf_plane(p: vec3<f32>, n: vec3<f32>, h: f32) -> f32 {
    return dot(p, n) + h;
}

/// Signed distance to a ground plane at y=0
/// p: point to evaluate
/// Returns: signed distance (negative below, positive above)
fn sdf_ground(p: vec3<f32>) -> f32 {
    return p.y;
}

// =============================================================================
// BLENDING / BOOLEAN OPERATIONS
// =============================================================================

/// Smooth minimum (polynomial smooth min)
/// Creates smooth blending/union between two SDF values
/// a, b: distance values to blend
/// k: smoothness factor (larger = smoother blend, 0 = hard union)
/// Returns: smoothly blended minimum
fn smin(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 0.0) {
        return min(a, b);
    }
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

/// Alias for smin (smooth_min)
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    return smin(a, b, k);
}

/// Smooth maximum (useful for smooth subtraction/intersection)
/// a, b: distance values
/// k: smoothness factor
/// Returns: smoothly blended maximum
fn smax(a: f32, b: f32, k: f32) -> f32 {
    return -smin(-a, -b, k);
}

/// Alias for smax (smooth_max)
fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    return smax(a, b, k);
}

/// LOD-aware smooth minimum
/// Adapts smoothness based on distance for performance optimization
/// a, b: distance values to blend
/// k: base smoothness factor
/// distance: distance from camera to the evaluation point
/// Returns: LOD-adjusted smoothly blended minimum
fn smin_lod(a: f32, b: f32, k: f32, distance: f32) -> f32 {
    // Scale k based on distance
    // Near (distance < 100): tighter blend for detail
    // Far (distance > 100): looser blend for performance
    let lod_scale = clamp(distance / 100.0, 0.1, 2.0);
    let lod_k = k * lod_scale;
    return smin(a, b, lod_k);
}

/// Hard union (minimum of two SDFs)
/// a, b: distance values
/// Returns: minimum distance
fn op_union(a: f32, b: f32) -> f32 {
    return min(a, b);
}

/// Hard subtraction (subtract a from b)
/// a: SDF to subtract
/// b: SDF to subtract from
/// Returns: distance with a carved out of b
fn op_subtract(a: f32, b: f32) -> f32 {
    return max(-a, b);
}

/// Smooth subtraction
/// a: SDF to subtract
/// b: SDF to subtract from
/// k: smoothness factor
/// Returns: smoothly subtracted distance
fn op_subtract_smooth(a: f32, b: f32, k: f32) -> f32 {
    return smax(-a, b, k);
}

/// Hard intersection (maximum of two SDFs)
/// a, b: distance values
/// Returns: distance inside both shapes
fn op_intersect(a: f32, b: f32) -> f32 {
    return max(a, b);
}

/// Smooth intersection
/// a, b: distance values
/// k: smoothness factor
/// Returns: smoothly intersected distance
fn op_intersect_smooth(a: f32, b: f32, k: f32) -> f32 {
    return smax(a, b, k);
}

// =============================================================================
// DOMAIN OPERATIONS (Transformations)
// =============================================================================

/// Translate a point (for SDF evaluation at translated position)
/// p: point to transform
/// offset: translation offset
/// Returns: translated point
fn op_translate(p: vec3<f32>, offset: vec3<f32>) -> vec3<f32> {
    return p - offset;
}

/// Scale a point uniformly (remember to scale the result by s)
/// p: point to transform
/// s: scale factor
/// Returns: scaled point (divide SDF result by s for correct distance)
fn op_scale(p: vec3<f32>, s: f32) -> vec3<f32> {
    return p / s;
}

/// Repeat space infinitely in all directions
/// p: point to transform
/// c: cell size in each dimension
/// Returns: point within the repeated cell
fn op_repeat(p: vec3<f32>, c: vec3<f32>) -> vec3<f32> {
    return (p % c) - c * 0.5;
}

/// Repeat space with limited count
/// p: point to transform
/// c: cell spacing
/// l: limit (number of repetitions on each side)
/// Returns: point with limited repetition
fn op_repeat_limited(p: vec3<f32>, c: f32, l: vec3<f32>) -> vec3<f32> {
    return p - c * clamp(round(p / c), -l, l);
}

/// Mirror across XZ plane (y=0)
/// p: point to transform
/// Returns: mirrored point (always positive y)
fn op_mirror_xz(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(p.x, abs(p.y), p.z);
}

/// Mirror across YZ plane (x=0)
/// p: point to transform
/// Returns: mirrored point (always positive x)
fn op_mirror_yz(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(abs(p.x), p.y, p.z);
}

/// Mirror across XY plane (z=0)
/// p: point to transform
/// Returns: mirrored point (always positive z)
fn op_mirror_xy(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(p.x, p.y, abs(p.z));
}

// =============================================================================
// DOMAIN WARPING / DEFORMATION OPERATIONS
// =============================================================================

/// Twist: Rotate XZ plane based on Y position
/// Creates a spiral/corkscrew effect along the Y axis
/// p: point to transform
/// twist_amount: radians per unit of Y (higher = tighter twist)
/// Returns: twisted point for SDF evaluation
fn op_twist(p: vec3<f32>, twist_amount: f32) -> vec3<f32> {
    let c = cos(twist_amount * p.y);
    let s = sin(twist_amount * p.y);
    return vec3<f32>(c * p.x - s * p.z, p.y, s * p.x + c * p.z);
}

/// Bend: Curve space along X axis
/// Creates a smooth arc/bend deformation
/// p: point to transform
/// bend_amount: bend intensity (higher = tighter bend)
/// Returns: bent point for SDF evaluation
fn op_bend(p: vec3<f32>, bend_amount: f32) -> vec3<f32> {
    let c = cos(bend_amount * p.x);
    let s = sin(bend_amount * p.x);
    return vec3<f32>(c * p.x - s * p.y, s * p.x + c * p.y, p.z);
}

/// Cheap Bend: Simple parabolic bend (faster than trigonometric)
/// Creates a bowl/saddle deformation
/// p: point to transform
/// k: bend factor (positive = bowl, negative = saddle)
/// Returns: bent point for SDF evaluation
fn op_cheap_bend(p: vec3<f32>, k: f32) -> vec3<f32> {
    return vec3<f32>(p.x, p.y + k * p.x * p.x, p.z);
}

/// Taper: Scale XZ based on Y position
/// Creates a cone/tapered shape from any primitive
/// p: point to transform
/// taper_amount: scale change per unit Y (0.1 = 10% smaller per unit up)
/// Returns: tapered point for SDF evaluation
fn op_taper(p: vec3<f32>, taper_amount: f32) -> vec3<f32> {
    let scale = 1.0 - taper_amount * p.y;
    return vec3<f32>(p.x / scale, p.y, p.z / scale);
}

// =============================================================================
// QUATERNION OPERATIONS (for rotations)
// =============================================================================

/// Rotate a vector by a quaternion
/// q: quaternion (x, y, z, w)
/// v: vector to rotate
/// Returns: rotated vector
fn quat_rotate(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let u = q.xyz;
    let s = q.w;
    return 2.0 * dot(u, v) * u
         + (s * s - dot(u, u)) * v
         + 2.0 * s * cross(u, v);
}

/// Inverse rotate (conjugate quaternion)
/// q: quaternion (x, y, z, w)
/// v: vector to rotate inversely
/// Returns: inversely rotated vector
fn quat_rotate_inv(q: vec4<f32>, v: vec3<f32>) -> vec3<f32> {
    let q_conj = vec4<f32>(-q.xyz, q.w);
    return quat_rotate(q_conj, v);
}

/// Get quaternion inverse (conjugate for unit quaternions)
/// q: quaternion (x, y, z, w)
/// Returns: inverse quaternion
fn quat_inverse(q: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(-q.x, -q.y, -q.z, q.w);
}

// =============================================================================
// DISPLACEMENT / MODIFICATION OPERATIONS
// =============================================================================

/// Add displacement to an SDF (makes surface bumpy)
/// d: base SDF distance
/// displacement: displacement amount (positive = outward)
/// Returns: displaced distance
fn op_displace(d: f32, displacement: f32) -> f32 {
    return d + displacement;
}

/// Round edges of an SDF
/// d: base SDF distance
/// r: rounding radius
/// Returns: rounded distance
fn op_round(d: f32, r: f32) -> f32 {
    return d - r;
}

/// Create an onion/shell from an SDF
/// d: base SDF distance
/// thickness: shell thickness
/// Returns: shell distance
fn op_onion(d: f32, thickness: f32) -> f32 {
    return abs(d) - thickness;
}

/// Elongate an SDF along an axis
/// p: point to evaluate
/// h: elongation amount (vec3 for each axis)
/// Returns: elongated point for SDF evaluation
fn op_elongate(p: vec3<f32>, h: vec3<f32>) -> vec3<f32> {
    return p - clamp(p, -h, h);
}

// =============================================================================
// UTILITY FUNCTIONS
// =============================================================================

/// Calculate SDF normal using central differences
/// p: point on surface
/// sdf_func: SDF function to evaluate (not directly usable in WGSL, for documentation)
/// eps: epsilon for numerical differentiation
/// Note: In actual use, inline the SDF evaluation
fn calc_normal_template(p: vec3<f32>, eps: f32) -> vec3<f32> {
    // This is a template showing the pattern. In actual shader code:
    // let e = vec2<f32>(eps, 0.0);
    // return normalize(vec3<f32>(
    //     scene_sdf(p + e.xyy) - scene_sdf(p - e.xyy),
    //     scene_sdf(p + e.yxy) - scene_sdf(p - e.yxy),
    //     scene_sdf(p + e.yyx) - scene_sdf(p - e.yyx)
    // ));
    return vec3<f32>(0.0, 1.0, 0.0); // Placeholder
}
