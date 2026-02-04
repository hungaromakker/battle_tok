// SDF Primitive Functions for Sketch Engine
// This module provides signed distance functions for basic primitives
// and operations for combining them into complex scenes.

// ============================================================================
// SDF Primitive Types
// ============================================================================
// 0 = Sphere
// 1 = Box
// 2 = Capsule

// ============================================================================
// Entity Structure (must match sketch-render/src/entity_buffer.rs)
// ============================================================================

struct Entity {
    position: vec3f,
    scale: f32,
    rotation: vec4f,  // Quaternion (x, y, z, w)
    sdf_type: u32,
    seed: u32,
    color: vec3f,
    _pad: f32,
}

// ============================================================================
// Storage Buffer Bindings
// ============================================================================

@group(0) @binding(0)
var<storage, read> entities: array<Entity>;

@group(0) @binding(1)
var<uniform> entity_count: u32;

// ============================================================================
// Quaternion Operations
// ============================================================================

// Rotate a vector by a quaternion
fn quat_rotate(q: vec4f, v: vec3f) -> vec3f {
    let u = q.xyz;
    let s = q.w;
    return 2.0 * dot(u, v) * u
         + (s * s - dot(u, u)) * v
         + 2.0 * s * cross(u, v);
}

// Inverse rotate (conjugate quaternion)
fn quat_rotate_inv(q: vec4f, v: vec3f) -> vec3f {
    let q_conj = vec4f(-q.xyz, q.w);
    return quat_rotate(q_conj, v);
}

// ============================================================================
// SDF Primitive Functions
// ============================================================================

// Signed distance to a sphere centered at origin
// p: point to evaluate
// r: radius of sphere
fn sdf_sphere(p: vec3f, r: f32) -> f32 {
    return length(p) - r;
}

// Signed distance to an axis-aligned box centered at origin
// p: point to evaluate
// b: half-extents (box dimensions / 2)
fn sdf_box(p: vec3f, b: vec3f) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3f(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

// Signed distance to a vertical capsule centered at origin
// p: point to evaluate
// h: height of the cylindrical part (total height = h + 2*r)
// r: radius of the capsule
fn sdf_capsule(p: vec3f, h: f32, r: f32) -> f32 {
    // Capsule aligned along Y axis, centered at origin
    let half_h = h * 0.5;
    let pa = p - vec3f(0.0, -half_h, 0.0);
    let ba = vec3f(0.0, h, 0.0);
    let t = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * t) - r;
}

// Alternative capsule: between two arbitrary points
// p: point to evaluate
// a: start point of capsule axis
// b: end point of capsule axis
// r: radius
fn sdf_capsule_ab(p: vec3f, a: vec3f, b: vec3f, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

// ============================================================================
// Blending Operations
// ============================================================================

// Smooth minimum (polynomial smooth min)
// Creates smooth blending/union between two SDF values
// a, b: distance values to blend
// k: smoothness factor (larger = smoother blend, 0 = hard union)
fn smooth_min(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 0.0) {
        return min(a, b);
    }
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// Alias for smooth_min
fn smin(a: f32, b: f32, k: f32) -> f32 {
    return smooth_min(a, b, k);
}

// LOD-aware smooth minimum
// Adapts smoothness based on distance for performance optimization
// a, b: distance values to blend
// k: base smoothness factor
// distance: distance from camera to the evaluation point
// Near objects (distance < 100) use specified k for full quality
// Far objects (distance > 100) use larger k for cheaper, less detailed blending
fn smin_lod(a: f32, b: f32, k: f32, distance: f32) -> f32 {
    // Scale k based on distance: k * clamp(distance / 100.0, 0.1, 2.0)
    // - At distance 10: k * 0.1 (tighter blend, more detail)
    // - At distance 100: k * 1.0 (base smoothness)
    // - At distance 200+: k * 2.0 (looser blend, cheaper)
    let lod_scale = clamp(distance / 100.0, 0.1, 2.0);
    let lod_k = k * lod_scale;
    return smooth_min(a, b, lod_k);
}

// Smooth maximum (useful for smooth subtraction)
// a, b: distance values
// k: smoothness factor
fn smooth_max(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 0.0) {
        return max(a, b);
    }
    let h = max(k - abs(-a - b), 0.0) / k;
    return max(-a, b) + h * h * k * 0.25;
}

// Hard union
fn op_union(a: f32, b: f32) -> f32 {
    return min(a, b);
}

// Hard subtraction (subtract a from b)
fn op_subtract(a: f32, b: f32) -> f32 {
    return max(-a, b);
}

// Hard intersection
fn op_intersect(a: f32, b: f32) -> f32 {
    return max(a, b);
}

// ============================================================================
// Entity SDF Evaluation
// ============================================================================

// Evaluate SDF for a single entity at world position p
fn sdf_entity(p: vec3f, entity: Entity) -> f32 {
    // Transform point to entity's local space
    let local_p = quat_rotate_inv(entity.rotation, p - entity.position);

    // Evaluate based on SDF type
    var d: f32;
    switch (entity.sdf_type) {
        case 0u: {
            // Sphere: scale is radius
            d = sdf_sphere(local_p, entity.scale);
        }
        case 1u: {
            // Box: scale is uniform half-extent
            d = sdf_box(local_p, vec3f(entity.scale));
        }
        case 2u: {
            // Capsule: scale is radius, height is 2x scale
            d = sdf_capsule(local_p, entity.scale * 2.0, entity.scale * 0.5);
        }
        default: {
            // Unknown type: treat as sphere
            d = sdf_sphere(local_p, entity.scale);
        }
    }

    return d;
}

// ============================================================================
// Scene SDF Evaluation
// ============================================================================

// Evaluate the complete scene SDF at world position p
// Combines all entities using smooth union
fn sdf_scene(p: vec3f) -> f32 {
    // Start with a large distance (nothing hit)
    var d = 1000.0;

    // Default blend factor for smooth union
    let blend_k = 0.3;

    // Combine all entities
    let count = entity_count;
    for (var i = 0u; i < count; i++) {
        let entity_d = sdf_entity(p, entities[i]);
        d = smooth_min(d, entity_d, blend_k);
    }

    // Add ground plane at y = 0 (optional, can be removed if not needed)
    // let ground = p.y;
    // d = min(d, ground);

    return d;
}

// Scene evaluation with material index output
// Returns the distance and sets the material_idx to the closest entity index
fn sdf_scene_with_material(p: vec3f, material_idx: ptr<function, u32>) -> f32 {
    var d = 1000.0;
    *material_idx = 0u;

    let count = entity_count;
    for (var i = 0u; i < count; i++) {
        let entity_d = sdf_entity(p, entities[i]);
        if (entity_d < d) {
            d = entity_d;
            *material_idx = i;
        }
    }

    return d;
}

// Get color for closest entity at point p
fn get_scene_color(p: vec3f) -> vec3f {
    var closest_idx = 0u;
    var min_d = 1000.0;

    let count = entity_count;
    for (var i = 0u; i < count; i++) {
        let entity_d = sdf_entity(p, entities[i]);
        if (entity_d < min_d) {
            min_d = entity_d;
            closest_idx = i;
        }
    }

    if (min_d < 1000.0) {
        return entities[closest_idx].color;
    }

    // Default color if no entity found
    return vec3f(0.5, 0.5, 0.5);
}
