// =============================================================================
// SDF Human Figure Module for Magic Engine
// =============================================================================
// A detailed human figure composed of approximately 50 SDF primitives.
// Uses smooth blending (smin) for organic form transitions.
//
// Height: ~1.75m standing upright
// Centered at origin with feet at y=0
//
// This module imports primitive functions from sdf_primitives.wgsl
// Note: WGSL does not yet support true module imports. Include both files
// or copy the required functions.
// =============================================================================

// =============================================================================
// REQUIRED IMPORTS (copy from sdf_primitives.wgsl or include both files)
// =============================================================================

// SDF Primitives
fn sdf_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

fn sdf_ellipsoid(p: vec3<f32>, r: vec3<f32>) -> f32 {
    let k0 = length(p / r);
    let k1 = length(p / (r * r));
    return k0 * (k0 - 1.0) / k1;
}

fn sdf_capsule(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

fn sdf_rounded_box(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

fn sdf_box(p: vec3<f32>, b: vec3<f32>) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

fn sdf_cylinder(p: vec3<f32>, h: f32, r: f32) -> f32 {
    let d = abs(vec2<f32>(length(p.xz), p.y)) - vec2<f32>(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

fn sdf_torus(p: vec3<f32>, t: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

// Smooth minimum for organic blending
fn smin(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 0.0) {
        return min(a, b);
    }
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// =============================================================================
// HUMAN FIGURE SDF (~50 primitives)
// =============================================================================

/// Signed distance to a detailed human figure
/// p: point to evaluate
/// Returns: signed distance to the human surface
///
/// Body parts breakdown (~50 primitives):
/// - Head: 5 primitives (skull, jaw, 2 ears, nose)
/// - Neck: 2 primitives (main neck, adam's apple)
/// - Torso: 8 primitives (chest, shoulders, abdomen, lower back, pelvis, clavicles)
/// - Arms: 12 primitives (2x upper arm, elbow, forearm, wrist, hand)
/// - Hands: 12 primitives (2x palm, thumb, 4 fingers simplified)
/// - Legs: 10 primitives (2x thigh, knee, calf, ankle, foot)
/// - Additional detail: 3 primitives (shoulder muscles, etc.)
fn sdf_human(p: vec3<f32>) -> f32 {
    var d = 1000.0;

    // =========================================================================
    // HEAD (5 primitives)
    // =========================================================================

    // 1. Main skull (ellipsoid)
    let head_pos = p - vec3<f32>(0.0, 1.58, 0.0);
    let skull = sdf_ellipsoid(head_pos, vec3<f32>(0.095, 0.12, 0.11));
    d = skull;

    // 2. Jaw / lower face (ellipsoid, positioned lower and forward)
    let jaw_pos = p - vec3<f32>(0.0, 1.50, 0.02);
    let jaw = sdf_ellipsoid(jaw_pos, vec3<f32>(0.07, 0.06, 0.07));
    d = smin(d, jaw, 0.04);

    // 3. Left ear (small ellipsoid)
    let l_ear_pos = p - vec3<f32>(-0.10, 1.55, 0.0);
    let l_ear = sdf_ellipsoid(l_ear_pos, vec3<f32>(0.015, 0.03, 0.01));
    d = smin(d, l_ear, 0.015);

    // 4. Right ear (small ellipsoid)
    let r_ear_pos = p - vec3<f32>(0.10, 1.55, 0.0);
    let r_ear = sdf_ellipsoid(r_ear_pos, vec3<f32>(0.015, 0.03, 0.01));
    d = smin(d, r_ear, 0.015);

    // 5. Nose (small capsule)
    let nose = sdf_capsule(p,
        vec3<f32>(0.0, 1.52, 0.10),
        vec3<f32>(0.0, 1.48, 0.12),
        0.015);
    d = smin(d, nose, 0.02);

    // =========================================================================
    // NECK (2 primitives)
    // =========================================================================

    // 6. Main neck (capsule)
    let neck = sdf_capsule(p,
        vec3<f32>(0.0, 1.35, 0.0),
        vec3<f32>(0.0, 1.46, 0.0),
        0.045);
    d = smin(d, neck, 0.06);

    // 7. Adam's apple area (subtle bump)
    let adams_pos = p - vec3<f32>(0.0, 1.40, 0.04);
    let adams = sdf_ellipsoid(adams_pos, vec3<f32>(0.02, 0.015, 0.015));
    d = smin(d, adams, 0.02);

    // =========================================================================
    // TORSO - UPPER (8 primitives)
    // =========================================================================

    // 8. Upper chest / pectorals (main mass)
    let chest_pos = p - vec3<f32>(0.0, 1.18, 0.0);
    let chest = sdf_ellipsoid(chest_pos, vec3<f32>(0.17, 0.18, 0.10));
    d = smin(d, chest, 0.08);

    // 9. Left pectoral detail
    let l_pec_pos = p - vec3<f32>(-0.07, 1.20, 0.06);
    let l_pec = sdf_ellipsoid(l_pec_pos, vec3<f32>(0.06, 0.04, 0.03));
    d = smin(d, l_pec, 0.04);

    // 10. Right pectoral detail
    let r_pec_pos = p - vec3<f32>(0.07, 1.20, 0.06);
    let r_pec = sdf_ellipsoid(r_pec_pos, vec3<f32>(0.06, 0.04, 0.03));
    d = smin(d, r_pec, 0.04);

    // 11. Left shoulder / deltoid
    let l_shoulder_pos = p - vec3<f32>(-0.18, 1.28, 0.0);
    let l_shoulder = sdf_ellipsoid(l_shoulder_pos, vec3<f32>(0.06, 0.05, 0.05));
    d = smin(d, l_shoulder, 0.05);

    // 12. Right shoulder / deltoid
    let r_shoulder_pos = p - vec3<f32>(0.18, 1.28, 0.0);
    let r_shoulder = sdf_ellipsoid(r_shoulder_pos, vec3<f32>(0.06, 0.05, 0.05));
    d = smin(d, r_shoulder, 0.05);

    // 13. Upper back (latissimus)
    let back_pos = p - vec3<f32>(0.0, 1.15, -0.05);
    let back = sdf_ellipsoid(back_pos, vec3<f32>(0.14, 0.15, 0.08));
    d = smin(d, back, 0.06);

    // 14. Left clavicle area
    let l_clav = sdf_capsule(p,
        vec3<f32>(-0.02, 1.30, 0.04),
        vec3<f32>(-0.16, 1.30, 0.0),
        0.02);
    d = smin(d, l_clav, 0.03);

    // 15. Right clavicle area
    let r_clav = sdf_capsule(p,
        vec3<f32>(0.02, 1.30, 0.04),
        vec3<f32>(0.16, 1.30, 0.0),
        0.02);
    d = smin(d, r_clav, 0.03);

    // =========================================================================
    // TORSO - LOWER (5 primitives)
    // =========================================================================

    // 16. Abdomen / core
    let abdomen_pos = p - vec3<f32>(0.0, 0.95, 0.0);
    let abdomen = sdf_ellipsoid(abdomen_pos, vec3<f32>(0.13, 0.13, 0.09));
    d = smin(d, abdomen, 0.10);

    // 17. Lower back / lumbar
    let lumbar_pos = p - vec3<f32>(0.0, 0.90, -0.04);
    let lumbar = sdf_ellipsoid(lumbar_pos, vec3<f32>(0.11, 0.10, 0.06));
    d = smin(d, lumbar, 0.08);

    // 18. Pelvis / hips (wide ellipsoid)
    let pelvis_pos = p - vec3<f32>(0.0, 0.80, 0.0);
    let pelvis = sdf_ellipsoid(pelvis_pos, vec3<f32>(0.15, 0.08, 0.10));
    d = smin(d, pelvis, 0.08);

    // 19. Left hip joint area
    let l_hip_joint_pos = p - vec3<f32>(-0.10, 0.78, 0.0);
    let l_hip_joint = sdf_sphere(l_hip_joint_pos, 0.05);
    d = smin(d, l_hip_joint, 0.04);

    // 20. Right hip joint area
    let r_hip_joint_pos = p - vec3<f32>(0.10, 0.78, 0.0);
    let r_hip_joint = sdf_sphere(r_hip_joint_pos, 0.05);
    d = smin(d, r_hip_joint, 0.04);

    // =========================================================================
    // LEFT ARM (6 primitives)
    // =========================================================================

    // 21. Left upper arm (bicep/tricep)
    let l_upper_arm = sdf_capsule(p,
        vec3<f32>(-0.22, 1.26, 0.0),
        vec3<f32>(-0.28, 0.98, 0.0),
        0.05);
    d = smin(d, l_upper_arm, 0.04);

    // 22. Left bicep bulge
    let l_bicep_pos = p - vec3<f32>(-0.24, 1.12, 0.02);
    let l_bicep = sdf_ellipsoid(l_bicep_pos, vec3<f32>(0.03, 0.06, 0.03));
    d = smin(d, l_bicep, 0.03);

    // 23. Left elbow
    let l_elbow_pos = p - vec3<f32>(-0.28, 0.96, -0.01);
    let l_elbow = sdf_sphere(l_elbow_pos, 0.035);
    d = smin(d, l_elbow, 0.03);

    // 24. Left forearm
    let l_forearm = sdf_capsule(p,
        vec3<f32>(-0.28, 0.94, 0.0),
        vec3<f32>(-0.32, 0.68, 0.0),
        0.038);
    d = smin(d, l_forearm, 0.03);

    // 25. Left wrist
    let l_wrist_pos = p - vec3<f32>(-0.33, 0.65, 0.0);
    let l_wrist = sdf_ellipsoid(l_wrist_pos, vec3<f32>(0.025, 0.03, 0.02));
    d = smin(d, l_wrist, 0.02);

    // 26. Left hand (simplified palm)
    let l_hand_pos = p - vec3<f32>(-0.35, 0.58, 0.0);
    let l_hand = sdf_ellipsoid(l_hand_pos, vec3<f32>(0.025, 0.05, 0.015));
    d = smin(d, l_hand, 0.02);

    // =========================================================================
    // RIGHT ARM (6 primitives)
    // =========================================================================

    // 27. Right upper arm (bicep/tricep)
    let r_upper_arm = sdf_capsule(p,
        vec3<f32>(0.22, 1.26, 0.0),
        vec3<f32>(0.28, 0.98, 0.0),
        0.05);
    d = smin(d, r_upper_arm, 0.04);

    // 28. Right bicep bulge
    let r_bicep_pos = p - vec3<f32>(0.24, 1.12, 0.02);
    let r_bicep = sdf_ellipsoid(r_bicep_pos, vec3<f32>(0.03, 0.06, 0.03));
    d = smin(d, r_bicep, 0.03);

    // 29. Right elbow
    let r_elbow_pos = p - vec3<f32>(0.28, 0.96, -0.01);
    let r_elbow = sdf_sphere(r_elbow_pos, 0.035);
    d = smin(d, r_elbow, 0.03);

    // 30. Right forearm
    let r_forearm = sdf_capsule(p,
        vec3<f32>(0.28, 0.94, 0.0),
        vec3<f32>(0.32, 0.68, 0.0),
        0.038);
    d = smin(d, r_forearm, 0.03);

    // 31. Right wrist
    let r_wrist_pos = p - vec3<f32>(0.33, 0.65, 0.0);
    let r_wrist = sdf_ellipsoid(r_wrist_pos, vec3<f32>(0.025, 0.03, 0.02));
    d = smin(d, r_wrist, 0.02);

    // 32. Right hand (simplified palm)
    let r_hand_pos = p - vec3<f32>(0.35, 0.58, 0.0);
    let r_hand = sdf_ellipsoid(r_hand_pos, vec3<f32>(0.025, 0.05, 0.015));
    d = smin(d, r_hand, 0.02);

    // =========================================================================
    // LEFT LEG (8 primitives)
    // =========================================================================

    // 33. Left thigh (upper leg)
    let l_thigh = sdf_capsule(p,
        vec3<f32>(-0.10, 0.75, 0.0),
        vec3<f32>(-0.11, 0.48, 0.0),
        0.07);
    d = smin(d, l_thigh, 0.05);

    // 34. Left quadricep bulge
    let l_quad_pos = p - vec3<f32>(-0.10, 0.60, 0.03);
    let l_quad = sdf_ellipsoid(l_quad_pos, vec3<f32>(0.04, 0.08, 0.03));
    d = smin(d, l_quad, 0.04);

    // 35. Left knee
    let l_knee_pos = p - vec3<f32>(-0.11, 0.46, 0.02);
    let l_knee = sdf_ellipsoid(l_knee_pos, vec3<f32>(0.04, 0.04, 0.035));
    d = smin(d, l_knee, 0.03);

    // 36. Left shin/calf (lower leg)
    let l_shin = sdf_capsule(p,
        vec3<f32>(-0.11, 0.44, 0.0),
        vec3<f32>(-0.11, 0.12, 0.0),
        0.05);
    d = smin(d, l_shin, 0.04);

    // 37. Left calf muscle
    let l_calf_pos = p - vec3<f32>(-0.11, 0.35, -0.03);
    let l_calf = sdf_ellipsoid(l_calf_pos, vec3<f32>(0.035, 0.08, 0.04));
    d = smin(d, l_calf, 0.03);

    // 38. Left ankle
    let l_ankle_pos = p - vec3<f32>(-0.11, 0.08, 0.0);
    let l_ankle = sdf_ellipsoid(l_ankle_pos, vec3<f32>(0.03, 0.035, 0.025));
    d = smin(d, l_ankle, 0.02);

    // 39. Left heel
    let l_heel_pos = p - vec3<f32>(-0.11, 0.03, -0.03);
    let l_heel = sdf_ellipsoid(l_heel_pos, vec3<f32>(0.025, 0.025, 0.03));
    d = smin(d, l_heel, 0.02);

    // 40. Left foot
    let l_foot_pos = p - vec3<f32>(-0.11, 0.025, 0.05);
    let l_foot = sdf_rounded_box(l_foot_pos, vec3<f32>(0.04, 0.02, 0.08), 0.015);
    d = smin(d, l_foot, 0.02);

    // =========================================================================
    // RIGHT LEG (8 primitives)
    // =========================================================================

    // 41. Right thigh (upper leg)
    let r_thigh = sdf_capsule(p,
        vec3<f32>(0.10, 0.75, 0.0),
        vec3<f32>(0.11, 0.48, 0.0),
        0.07);
    d = smin(d, r_thigh, 0.05);

    // 42. Right quadricep bulge
    let r_quad_pos = p - vec3<f32>(0.10, 0.60, 0.03);
    let r_quad = sdf_ellipsoid(r_quad_pos, vec3<f32>(0.04, 0.08, 0.03));
    d = smin(d, r_quad, 0.04);

    // 43. Right knee
    let r_knee_pos = p - vec3<f32>(0.11, 0.46, 0.02);
    let r_knee = sdf_ellipsoid(r_knee_pos, vec3<f32>(0.04, 0.04, 0.035));
    d = smin(d, r_knee, 0.03);

    // 44. Right shin/calf (lower leg)
    let r_shin = sdf_capsule(p,
        vec3<f32>(0.11, 0.44, 0.0),
        vec3<f32>(0.11, 0.12, 0.0),
        0.05);
    d = smin(d, r_shin, 0.04);

    // 45. Right calf muscle
    let r_calf_pos = p - vec3<f32>(0.11, 0.35, -0.03);
    let r_calf = sdf_ellipsoid(r_calf_pos, vec3<f32>(0.035, 0.08, 0.04));
    d = smin(d, r_calf, 0.03);

    // 46. Right ankle
    let r_ankle_pos = p - vec3<f32>(0.11, 0.08, 0.0);
    let r_ankle = sdf_ellipsoid(r_ankle_pos, vec3<f32>(0.03, 0.035, 0.025));
    d = smin(d, r_ankle, 0.02);

    // 47. Right heel
    let r_heel_pos = p - vec3<f32>(0.11, 0.03, -0.03);
    let r_heel = sdf_ellipsoid(r_heel_pos, vec3<f32>(0.025, 0.025, 0.03));
    d = smin(d, r_heel, 0.02);

    // 48. Right foot
    let r_foot_pos = p - vec3<f32>(0.11, 0.025, 0.05);
    let r_foot = sdf_rounded_box(r_foot_pos, vec3<f32>(0.04, 0.02, 0.08), 0.015);
    d = smin(d, r_foot, 0.02);

    // =========================================================================
    // ADDITIONAL DETAIL (2 primitives to reach ~50)
    // =========================================================================

    // 49. Left trapezius muscle
    let l_trap = sdf_capsule(p,
        vec3<f32>(-0.05, 1.32, -0.02),
        vec3<f32>(-0.14, 1.28, -0.03),
        0.025);
    d = smin(d, l_trap, 0.03);

    // 50. Right trapezius muscle
    let r_trap = sdf_capsule(p,
        vec3<f32>(0.05, 1.32, -0.02),
        vec3<f32>(0.14, 1.28, -0.03),
        0.025);
    d = smin(d, r_trap, 0.03);

    return d;
}

// =============================================================================
// HUMAN FIGURE WITH POSE SUPPORT
// =============================================================================

/// Human figure with basic pose parameters
/// p: point to evaluate
/// pose: pose configuration structure
/// Returns: signed distance
///
/// Note: Full pose support requires skeletal animation system.
/// This is a placeholder for future pose implementation.
struct HumanPose {
    // Head
    head_tilt: f32,      // Forward/back tilt (-1 to 1)
    head_turn: f32,      // Left/right turn (-1 to 1)

    // Arms
    l_arm_raise: f32,    // 0 = down, 1 = horizontal, 2 = up
    r_arm_raise: f32,
    l_arm_forward: f32,  // 0 = back, 1 = forward
    r_arm_forward: f32,

    // Legs
    l_leg_forward: f32,  // -1 = back, 0 = neutral, 1 = forward
    r_leg_forward: f32,
}

/// Get default standing pose
fn human_pose_default() -> HumanPose {
    var pose: HumanPose;
    pose.head_tilt = 0.0;
    pose.head_turn = 0.0;
    pose.l_arm_raise = 0.0;
    pose.r_arm_raise = 0.0;
    pose.l_arm_forward = 0.0;
    pose.r_arm_forward = 0.0;
    pose.l_leg_forward = 0.0;
    pose.r_leg_forward = 0.0;
    return pose;
}

// =============================================================================
// SIMPLIFIED HUMAN (for LOD / performance)
// =============================================================================

/// Simplified human figure for distant rendering (~15 primitives)
/// p: point to evaluate
/// Returns: signed distance
fn sdf_human_lod_low(p: vec3<f32>) -> f32 {
    var d = 1000.0;

    // Head (1 primitive)
    let head_pos = p - vec3<f32>(0.0, 1.55, 0.0);
    let head = sdf_ellipsoid(head_pos, vec3<f32>(0.10, 0.12, 0.10));
    d = head;

    // Neck (1 primitive)
    let neck = sdf_capsule(p,
        vec3<f32>(0.0, 1.35, 0.0),
        vec3<f32>(0.0, 1.45, 0.0),
        0.04);
    d = smin(d, neck, 0.05);

    // Torso - single mass (1 primitive)
    let torso_pos = p - vec3<f32>(0.0, 1.0, 0.0);
    let torso = sdf_ellipsoid(torso_pos, vec3<f32>(0.16, 0.30, 0.10));
    d = smin(d, torso, 0.08);

    // Hips (1 primitive)
    let hips_pos = p - vec3<f32>(0.0, 0.75, 0.0);
    let hips = sdf_ellipsoid(hips_pos, vec3<f32>(0.14, 0.10, 0.10));
    d = smin(d, hips, 0.08);

    // Left arm (2 primitives)
    let l_arm_upper = sdf_capsule(p,
        vec3<f32>(-0.20, 1.25, 0.0),
        vec3<f32>(-0.28, 0.95, 0.0),
        0.045);
    d = smin(d, l_arm_upper, 0.04);

    let l_arm_lower = sdf_capsule(p,
        vec3<f32>(-0.28, 0.95, 0.0),
        vec3<f32>(-0.34, 0.60, 0.0),
        0.035);
    d = smin(d, l_arm_lower, 0.03);

    // Right arm (2 primitives)
    let r_arm_upper = sdf_capsule(p,
        vec3<f32>(0.20, 1.25, 0.0),
        vec3<f32>(0.28, 0.95, 0.0),
        0.045);
    d = smin(d, r_arm_upper, 0.04);

    let r_arm_lower = sdf_capsule(p,
        vec3<f32>(0.28, 0.95, 0.0),
        vec3<f32>(0.34, 0.60, 0.0),
        0.035);
    d = smin(d, r_arm_lower, 0.03);

    // Left leg (3 primitives)
    let l_leg_upper = sdf_capsule(p,
        vec3<f32>(-0.10, 0.72, 0.0),
        vec3<f32>(-0.11, 0.42, 0.0),
        0.065);
    d = smin(d, l_leg_upper, 0.05);

    let l_leg_lower = sdf_capsule(p,
        vec3<f32>(-0.11, 0.42, 0.0),
        vec3<f32>(-0.11, 0.08, 0.0),
        0.045);
    d = smin(d, l_leg_lower, 0.04);

    let l_foot_pos = p - vec3<f32>(-0.11, 0.03, 0.04);
    let l_foot = sdf_rounded_box(l_foot_pos, vec3<f32>(0.04, 0.025, 0.08), 0.015);
    d = smin(d, l_foot, 0.02);

    // Right leg (3 primitives)
    let r_leg_upper = sdf_capsule(p,
        vec3<f32>(0.10, 0.72, 0.0),
        vec3<f32>(0.11, 0.42, 0.0),
        0.065);
    d = smin(d, r_leg_upper, 0.05);

    let r_leg_lower = sdf_capsule(p,
        vec3<f32>(0.11, 0.42, 0.0),
        vec3<f32>(0.11, 0.08, 0.0),
        0.045);
    d = smin(d, r_leg_lower, 0.04);

    let r_foot_pos = p - vec3<f32>(0.11, 0.03, 0.04);
    let r_foot = sdf_rounded_box(r_foot_pos, vec3<f32>(0.04, 0.025, 0.08), 0.015);
    d = smin(d, r_foot, 0.02);

    return d;
}

/// Silhouette-level human figure for very distant rendering (~5 primitives)
/// p: point to evaluate
/// Returns: signed distance
fn sdf_human_lod_silhouette(p: vec3<f32>) -> f32 {
    var d = 1000.0;

    // Head
    let head_pos = p - vec3<f32>(0.0, 1.55, 0.0);
    let head = sdf_sphere(head_pos, 0.11);
    d = head;

    // Body (single capsule from neck to hips)
    let body = sdf_capsule(p,
        vec3<f32>(0.0, 1.40, 0.0),
        vec3<f32>(0.0, 0.75, 0.0),
        0.14);
    d = smin(d, body, 0.08);

    // Legs (single merged mass)
    let legs = sdf_capsule(p,
        vec3<f32>(0.0, 0.75, 0.0),
        vec3<f32>(0.0, 0.05, 0.0),
        0.08);
    d = smin(d, legs, 0.06);

    // Arms (single merged mass on each side)
    let l_arm = sdf_capsule(p,
        vec3<f32>(-0.18, 1.25, 0.0),
        vec3<f32>(-0.30, 0.65, 0.0),
        0.04);
    d = smin(d, l_arm, 0.04);

    let r_arm = sdf_capsule(p,
        vec3<f32>(0.18, 1.25, 0.0),
        vec3<f32>(0.30, 0.65, 0.0),
        0.04);
    d = smin(d, r_arm, 0.04);

    return d;
}

/// LOD-aware human SDF that selects detail level based on distance
/// p: point to evaluate
/// camera_distance: distance from camera to the human
/// Returns: signed distance
fn sdf_human_lod(p: vec3<f32>, camera_distance: f32) -> f32 {
    if (camera_distance >= 50.0) {
        return sdf_human_lod_silhouette(p);
    } else if (camera_distance >= 20.0) {
        return sdf_human_lod_low(p);
    } else {
        return sdf_human(p);
    }
}
