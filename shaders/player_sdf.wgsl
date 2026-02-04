// =============================================================================
// Player Model SDF for Magic Engine
// =============================================================================
// A simplified player figure designed for first/third person views.
// Uses smooth blending (smin) for organic form transitions.
//
// Height: 1.8m standing upright (1 unit = 1 meter, SI units)
// Centered at origin with feet at y=0
//
// This module is designed to be included in the main raymarcher shader
// or used standalone for player rendering.
// =============================================================================

// =============================================================================
// SDF PRIMITIVE FUNCTIONS
// =============================================================================
// These functions are prefixed with "player_" to avoid name collisions when
// this file is concatenated with other shaders. When included in the main
// raymarcher, you may use the existing sdf_* functions instead.
// =============================================================================

// Signed distance to a sphere centered at origin
fn player_sdf_sphere(p: vec3<f32>, r: f32) -> f32 {
    return length(p) - r;
}

// Signed distance to a capsule between two points
fn player_sdf_capsule(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

// Signed distance to a vertical capsule centered at origin
// h: height of the cylindrical part (total height = h + 2*r)
// r: radius of the capsule
fn player_sdf_capsule_vertical(p: vec3<f32>, h: f32, r: f32) -> f32 {
    let half_h = h * 0.5;
    let pa = p - vec3<f32>(0.0, -half_h, 0.0);
    let ba = vec3<f32>(0.0, h, 0.0);
    let t = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * t) - r;
}

// Smooth minimum (polynomial smooth min) for organic blending
// k: smoothness factor (0.1 recommended for organic joints)
fn player_smin(a: f32, b: f32, k: f32) -> f32 {
    if (k <= 0.0) {
        return min(a, b);
    }
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// =============================================================================
// PLAYER DIMENSIONS (SI units: 1 unit = 1 meter)
// =============================================================================

// Body parameters
const PLAYER_BODY_RADIUS: f32 = 0.3;      // Torso capsule radius
const PLAYER_BODY_HEIGHT: f32 = 1.8;      // Total body height (capsule + caps)
const PLAYER_BODY_CAPSULE_H: f32 = 1.2;   // Cylindrical portion height (1.8 - 2*0.3)

// Head parameters
const PLAYER_HEAD_RADIUS: f32 = 0.2;
const PLAYER_HEAD_Y: f32 = 1.7;           // Head center Y position (near top of body)

// Arm parameters
const PLAYER_ARM_RADIUS: f32 = 0.08;
const PLAYER_ARM_LENGTH: f32 = 0.6;       // Length from shoulder to elbow area
const PLAYER_SHOULDER_Y: f32 = 1.4;       // Shoulder attachment height
const PLAYER_SHOULDER_X: f32 = 0.35;      // Shoulder offset from center

// Hand parameters
const PLAYER_HAND_RADIUS: f32 = 0.1;

// Smooth union factor for organic joints
const PLAYER_BLEND_K: f32 = 0.1;

// =============================================================================
// PLAYER ANIMATION PARAMETERS
// =============================================================================

/// Player animation state for procedural animation
/// All angles are in radians
struct PlayerAnimation {
    // Head animation
    head_pitch: f32,      // Head tilt forward/back (-0.5 to 0.5 radians)
    head_yaw: f32,        // Head turn left/right (-1.0 to 1.0 radians)

    // Arm animation (each arm)
    left_arm_swing: f32,  // Arm forward/back swing (-1.0 to 1.0 radians)
    right_arm_swing: f32,
    left_arm_raise: f32,  // Arm raise/lower (0 = down, PI/2 = horizontal)
    right_arm_raise: f32,
}

/// Get default animation state (standing still, arms at sides)
fn player_animation_default() -> PlayerAnimation {
    var anim: PlayerAnimation;
    anim.head_pitch = 0.0;
    anim.head_yaw = 0.0;
    anim.left_arm_swing = 0.0;
    anim.right_arm_swing = 0.0;
    anim.left_arm_raise = 0.0;
    anim.right_arm_raise = 0.0;
    return anim;
}

/// Create walking animation state based on time
/// phase: walking cycle phase (0 to 2*PI)
/// speed: walking speed multiplier (1.0 = normal)
fn player_animation_walk(phase: f32, speed: f32) -> PlayerAnimation {
    var anim: PlayerAnimation;

    // Head bobs slightly while walking
    anim.head_pitch = sin(phase * 2.0) * 0.05 * speed;
    anim.head_yaw = 0.0;

    // Arms swing opposite to legs (natural walking motion)
    let arm_swing_amount = 0.4 * speed;
    anim.left_arm_swing = sin(phase) * arm_swing_amount;
    anim.right_arm_swing = -sin(phase) * arm_swing_amount;

    // Arms stay mostly down while walking
    anim.left_arm_raise = 0.1;
    anim.right_arm_raise = 0.1;

    return anim;
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// Rotate a point around the Y axis
fn rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(
        p.x * c + p.z * s,
        p.y,
        -p.x * s + p.z * c
    );
}

/// Rotate a point around the X axis
fn rotate_x(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(
        p.x,
        p.y * c - p.z * s,
        p.y * s + p.z * c
    );
}

/// Rotate a point around the Z axis
fn rotate_z(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(
        p.x * c - p.y * s,
        p.x * s + p.y * c,
        p.z
    );
}

// =============================================================================
// PLAYER SDF FUNCTIONS
// =============================================================================

/// Signed distance to the player body (capsule)
/// p: point to evaluate in player local space (player at origin, facing +Z)
fn sdf_player_body(p: vec3<f32>) -> f32 {
    // Body is a vertical capsule centered at y = 0.9 (midpoint of 1.8m height)
    // The capsule cylindrical portion is PLAYER_BODY_CAPSULE_H with radius PLAYER_BODY_RADIUS
    let body_center = vec3<f32>(0.0, 0.9, 0.0);
    let body_p = p - body_center;
    return player_sdf_capsule_vertical(body_p, PLAYER_BODY_CAPSULE_H, PLAYER_BODY_RADIUS);
}

/// Signed distance to the player head (sphere with animation)
/// p: point to evaluate in player local space
/// anim: animation state for head movement
fn sdf_player_head(p: vec3<f32>, anim: PlayerAnimation) -> f32 {
    // Head position with animation offset
    let head_center = vec3<f32>(0.0, PLAYER_HEAD_Y, 0.0);
    var head_p = p - head_center;

    // Apply head rotation (pitch and yaw)
    // First rotate around Y (yaw/turn), then around X (pitch/tilt)
    head_p = rotate_y(head_p, -anim.head_yaw);
    head_p = rotate_x(head_p, -anim.head_pitch);

    return player_sdf_sphere(head_p, PLAYER_HEAD_RADIUS);
}

/// Signed distance to the left arm (capsule from shoulder to hand area)
/// p: point to evaluate in player local space
/// anim: animation state for arm movement
fn sdf_player_left_arm(p: vec3<f32>, anim: PlayerAnimation) -> f32 {
    // Shoulder position (left side)
    let shoulder = vec3<f32>(-PLAYER_SHOULDER_X, PLAYER_SHOULDER_Y, 0.0);

    // Calculate arm end position based on animation
    // Arm swings forward/back and can raise
    var arm_dir = vec3<f32>(0.0, -1.0, 0.0);  // Default: pointing down

    // Apply arm raise (rotate around Z axis, toward body)
    arm_dir = rotate_z(arm_dir, -anim.left_arm_raise);

    // Apply arm swing (rotate around X axis)
    arm_dir = rotate_x(arm_dir, anim.left_arm_swing);

    let arm_end = shoulder + arm_dir * PLAYER_ARM_LENGTH;

    return player_sdf_capsule(p, shoulder, arm_end, PLAYER_ARM_RADIUS);
}

/// Signed distance to the right arm (capsule from shoulder to hand area)
/// p: point to evaluate in player local space
/// anim: animation state for arm movement
fn sdf_player_right_arm(p: vec3<f32>, anim: PlayerAnimation) -> f32 {
    // Shoulder position (right side)
    let shoulder = vec3<f32>(PLAYER_SHOULDER_X, PLAYER_SHOULDER_Y, 0.0);

    // Calculate arm end position based on animation
    var arm_dir = vec3<f32>(0.0, -1.0, 0.0);  // Default: pointing down

    // Apply arm raise (rotate around Z axis, away from body)
    arm_dir = rotate_z(arm_dir, anim.right_arm_raise);

    // Apply arm swing (rotate around X axis)
    arm_dir = rotate_x(arm_dir, anim.right_arm_swing);

    let arm_end = shoulder + arm_dir * PLAYER_ARM_LENGTH;

    return player_sdf_capsule(p, shoulder, arm_end, PLAYER_ARM_RADIUS);
}

/// Signed distance to the left hand (sphere at arm end)
/// p: point to evaluate in player local space
/// anim: animation state for hand position
fn sdf_player_left_hand(p: vec3<f32>, anim: PlayerAnimation) -> f32 {
    // Calculate hand position (at end of arm)
    let shoulder = vec3<f32>(-PLAYER_SHOULDER_X, PLAYER_SHOULDER_Y, 0.0);

    var arm_dir = vec3<f32>(0.0, -1.0, 0.0);
    arm_dir = rotate_z(arm_dir, -anim.left_arm_raise);
    arm_dir = rotate_x(arm_dir, anim.left_arm_swing);

    // Hand is at the end of the arm, plus a small offset for the hand sphere
    let hand_center = shoulder + arm_dir * (PLAYER_ARM_LENGTH + PLAYER_HAND_RADIUS * 0.5);

    return player_sdf_sphere(p - hand_center, PLAYER_HAND_RADIUS);
}

/// Signed distance to the right hand (sphere at arm end)
/// p: point to evaluate in player local space
/// anim: animation state for hand position
fn sdf_player_right_hand(p: vec3<f32>, anim: PlayerAnimation) -> f32 {
    // Calculate hand position (at end of arm)
    let shoulder = vec3<f32>(PLAYER_SHOULDER_X, PLAYER_SHOULDER_Y, 0.0);

    var arm_dir = vec3<f32>(0.0, -1.0, 0.0);
    arm_dir = rotate_z(arm_dir, anim.right_arm_raise);
    arm_dir = rotate_x(arm_dir, anim.right_arm_swing);

    // Hand is at the end of the arm, plus a small offset for the hand sphere
    let hand_center = shoulder + arm_dir * (PLAYER_ARM_LENGTH + PLAYER_HAND_RADIUS * 0.5);

    return player_sdf_sphere(p - hand_center, PLAYER_HAND_RADIUS);
}

// =============================================================================
// MAIN PLAYER SDF
// =============================================================================

/// Signed distance to the complete player model
/// p: point to evaluate in player local space (player at origin, facing +Z)
/// Returns: signed distance to the player surface
///
/// Body parts breakdown:
/// - Body: 1 capsule (radius 0.3, height 1.8)
/// - Head: 1 sphere (radius 0.2) with animation support
/// - Arms: 2 capsules (radius 0.08) at shoulders
/// - Hands: 2 spheres (radius 0.1) at arm ends
/// Total: 6 primitives with smooth union blending
fn sdf_player(p: vec3<f32>) -> f32 {
    // Use default animation (standing still)
    return sdf_player_animated(p, player_animation_default());
}

/// Signed distance to the complete player model with animation
/// p: point to evaluate in player local space (player at origin, facing +Z)
/// anim: animation state for procedural animation
/// Returns: signed distance to the player surface
fn sdf_player_animated(p: vec3<f32>, anim: PlayerAnimation) -> f32 {
    var d: f32 = 1000.0;

    // 1. Body capsule (main torso)
    let body = sdf_player_body(p);
    d = body;

    // 2. Head sphere with animation (smooth union with body for neck area)
    let head = sdf_player_head(p, anim);
    d = player_smin(d, head, PLAYER_BLEND_K);

    // 3. Left arm capsule (smooth union with body at shoulder)
    let left_arm = sdf_player_left_arm(p, anim);
    d = player_smin(d, left_arm, PLAYER_BLEND_K);

    // 4. Right arm capsule (smooth union with body at shoulder)
    let right_arm = sdf_player_right_arm(p, anim);
    d = player_smin(d, right_arm, PLAYER_BLEND_K);

    // 5. Left hand sphere (smooth union with arm at wrist)
    let left_hand = sdf_player_left_hand(p, anim);
    d = player_smin(d, left_hand, PLAYER_BLEND_K);

    // 6. Right hand sphere (smooth union with arm at wrist)
    let right_hand = sdf_player_right_hand(p, anim);
    d = player_smin(d, right_hand, PLAYER_BLEND_K);

    return d;
}

// =============================================================================
// PLAYER WORLD-SPACE SDF
// =============================================================================

/// Signed distance to the player model at a world position with rotation
/// p: point to evaluate in world space
/// player_pos: player position in world space (feet position)
/// player_yaw: player facing direction (rotation around Y axis, radians)
/// anim: animation state
/// Returns: signed distance to the player surface
fn sdf_player_world(p: vec3<f32>, player_pos: vec3<f32>, player_yaw: f32, anim: PlayerAnimation) -> f32 {
    // Transform point to player local space
    var local_p = p - player_pos;
    local_p = rotate_y(local_p, -player_yaw);

    return sdf_player_animated(local_p, anim);
}

/// Simplified world-space player SDF without animation
/// p: point to evaluate in world space
/// player_pos: player position in world space (feet position)
/// player_yaw: player facing direction (rotation around Y axis, radians)
/// Returns: signed distance to the player surface
fn sdf_player_world_static(p: vec3<f32>, player_pos: vec3<f32>, player_yaw: f32) -> f32 {
    return sdf_player_world(p, player_pos, player_yaw, player_animation_default());
}

// =============================================================================
// LOD VARIANTS
// =============================================================================

/// Low-detail player SDF for distant rendering (~3 primitives)
/// Uses a single body capsule + head sphere (no arms)
/// p: point to evaluate in player local space
fn sdf_player_lod_low(p: vec3<f32>) -> f32 {
    var d: f32 = 1000.0;

    // Simplified body (single capsule)
    let body = sdf_player_body(p);
    d = body;

    // Head (no animation at low LOD)
    let head_center = vec3<f32>(0.0, PLAYER_HEAD_Y, 0.0);
    let head = player_sdf_sphere(p - head_center, PLAYER_HEAD_RADIUS);
    d = player_smin(d, head, PLAYER_BLEND_K);

    return d;
}

/// Silhouette-level player SDF for very distant rendering (~1 primitive)
/// Uses a single elongated capsule for the entire body
/// p: point to evaluate in player local space
fn sdf_player_lod_silhouette(p: vec3<f32>) -> f32 {
    // Single capsule approximating entire body from feet to head
    let body_center = vec3<f32>(0.0, 0.9, 0.0);
    let body_p = p - body_center;
    // Taller, thinner capsule for silhouette
    return player_sdf_capsule_vertical(body_p, 1.5, 0.25);
}

/// LOD-aware player SDF that selects detail level based on distance
/// p: point to evaluate in player local space
/// camera_distance: distance from camera to the player
/// anim: animation state (used for full detail only)
/// Returns: signed distance
fn sdf_player_lod(p: vec3<f32>, camera_distance: f32, anim: PlayerAnimation) -> f32 {
    // Distance thresholds for LOD transitions
    const LOD_FULL_DISTANCE: f32 = 20.0;      // Full detail up to 20m
    const LOD_LOW_DISTANCE: f32 = 50.0;       // Low detail 20-50m
    // Beyond 50m: silhouette mode

    if (camera_distance < LOD_FULL_DISTANCE) {
        return sdf_player_animated(p, anim);
    } else if (camera_distance < LOD_LOW_DISTANCE) {
        return sdf_player_lod_low(p);
    } else {
        return sdf_player_lod_silhouette(p);
    }
}

// =============================================================================
// PLAYER CULLING NOTES
// =============================================================================
//
// IMPORTANT: The player model should be EXCLUDED from tile-based culling.
//
// Reasons:
// 1. The player is always visible (first-person hands, third-person body)
// 2. The player position changes every frame, making culling overhead wasteful
// 3. There's only one player, so per-tile lists don't help
//
// Implementation approach:
// - In scene_sdf(), evaluate the player SDF separately AFTER entity loop
// - Do NOT add player to entity_buffer (it's not a placed entity)
// - Render player with special handling (first-person: arms only, third-person: full)
//
// Example integration in raymarcher.wgsl scene_sdf():
//
// fn scene_sdf(p: vec3<f32>) -> HitResult {
//     var result: HitResult;
//     // ... evaluate terrain, entities, markers ...
//
//     // Evaluate player (always, excluded from culling)
//     let player_d = sdf_player_world(p, player_position, player_yaw, player_anim);
//     if (player_d < result.dist) {
//         result.dist = player_d;
//         result.entity_index = PLAYER_ENTITY_INDEX;  // Special constant
//         result.is_player = 1u;
//     }
//
//     return result;
// }
//
// =============================================================================
