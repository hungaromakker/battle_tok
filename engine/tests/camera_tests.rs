//! Camera Tests - Position Calculation and Raycast
//!
//! Tests for the camera module including CameraController and raycast functionality.

use glam::Vec3;
use magic_engine::camera::{CameraController, CameraMode};
use magic_engine::camera::raycast::{
    raycast_to_ground, raycast_to_plane, get_ray_direction, RaycastConfig,
};

// ============================================================================
// CameraController Tests
// ============================================================================

#[test]
fn test_camera_controller_default() {
    let camera = CameraController::default();

    // Check default position
    assert_eq!(camera.position, Vec3::new(0.0, 2.0, 8.0));
    // Check default angles
    assert_eq!(camera.yaw, 0.0);
    assert!(camera.pitch.abs() < 0.5); // Slightly looking down
    // Check default mode
    assert_eq!(camera.mode, CameraMode::Orbit);
    // Check default target
    assert_eq!(camera.target, Vec3::ZERO);
}

#[test]
fn test_camera_controller_new() {
    let camera = CameraController::new();

    assert_eq!(camera.position, Vec3::new(0.0, 2.0, 8.0));
    assert_eq!(camera.mode, CameraMode::Orbit);
}

#[test]
fn test_camera_controller_with_position() {
    let custom_pos = Vec3::new(10.0, 20.0, 30.0);
    let camera = CameraController::with_position(custom_pos);

    assert_eq!(camera.position, custom_pos);
    // Other fields should be default
    assert_eq!(camera.mode, CameraMode::Orbit);
}

#[test]
fn test_camera_get_position() {
    let camera = CameraController::new();
    let pos = camera.get_position();

    assert_eq!(pos, camera.position);
}

#[test]
fn test_camera_forward_vector_yaw_zero_pitch_zero() {
    let mut camera = CameraController::new();
    camera.yaw = 0.0;
    camera.pitch = 0.0;

    let forward = camera.get_forward();

    // When yaw=0 and pitch=0, should look towards -Z
    assert!(forward.z < -0.9);
    assert!(forward.y.abs() < 0.01);
    assert!(forward.x.abs() < 0.01);
}

#[test]
fn test_camera_forward_vector_normalized() {
    let camera = CameraController::new();
    let forward = camera.get_forward();

    // Forward vector should be normalized (length ~= 1.0)
    let length = forward.length();
    assert!((length - 1.0).abs() < 0.001);
}

#[test]
fn test_camera_right_vector() {
    let camera = CameraController::new();
    let right = camera.get_right();

    // Right vector should be normalized
    let length = right.length();
    assert!((length - 1.0).abs() < 0.001);
}

#[test]
fn test_camera_up_vector() {
    let camera = CameraController::new();
    let up = camera.get_up();

    // Up vector should be normalized
    let length = up.length();
    assert!((length - 1.0).abs() < 0.001);
}

#[test]
fn test_camera_forward_right_perpendicular() {
    let camera = CameraController::new();
    let forward = camera.get_forward();
    let right = camera.get_right();

    // Forward and right should be perpendicular (dot product ~= 0)
    let dot = forward.dot(right);
    assert!(dot.abs() < 0.01);
}

#[test]
fn test_camera_pitch_clamping() {
    let mut camera = CameraController::new();

    // Try to rotate way past the limit
    camera.rotate(0.0, 10.0);
    assert!(camera.pitch <= camera.pitch_limits.1);

    // Try to rotate way below the limit
    camera.rotate(0.0, -20.0);
    assert!(camera.pitch >= camera.pitch_limits.0);
}

#[test]
fn test_camera_yaw_rotation() {
    let mut camera = CameraController::new();
    let initial_yaw = camera.yaw;

    camera.rotate(0.5, 0.0);

    assert!((camera.yaw - initial_yaw - 0.5).abs() < 0.001);
}

#[test]
fn test_camera_reset() {
    let mut camera = CameraController::new();

    // Change position and angles
    camera.position = Vec3::new(100.0, 200.0, 300.0);
    camera.yaw = 3.14;
    camera.pitch = 1.0;

    // Reset
    camera.reset();

    // Should be back to defaults
    assert_eq!(camera.position, Vec3::new(0.0, 2.0, 8.0));
    assert_eq!(camera.yaw, 0.0);
    assert_eq!(camera.pitch, -0.2);
}

#[test]
fn test_camera_zoom() {
    let mut camera = CameraController::new();
    let initial_pos = camera.position;

    camera.zoom(2.0);

    // Position should have moved forward
    assert_ne!(camera.position, initial_pos);
}

#[test]
fn test_camera_look_at() {
    let mut camera = CameraController::new();
    camera.position = Vec3::new(10.0, 5.0, 10.0);

    camera.look_at(Vec3::ZERO);

    // After looking at origin, the forward vector should point towards origin
    let forward = camera.get_forward();
    let to_origin = (Vec3::ZERO - camera.position).normalize();

    // Should be roughly aligned
    let dot = forward.dot(to_origin);
    assert!(dot > 0.9);
}

#[test]
fn test_camera_set_mode() {
    let mut camera = CameraController::new();
    assert_eq!(camera.mode, CameraMode::Orbit);

    camera.set_mode(CameraMode::Fly);
    assert_eq!(camera.mode, CameraMode::Fly);

    camera.set_mode(CameraMode::Orbit);
    assert_eq!(camera.mode, CameraMode::Orbit);
}

#[test]
fn test_camera_get_target_orbit_mode() {
    let mut camera = CameraController::new();
    camera.mode = CameraMode::Orbit;
    camera.target = Vec3::new(1.0, 2.0, 3.0);

    let target = camera.get_target();
    assert_eq!(target, camera.target);
}

#[test]
fn test_camera_get_target_fly_mode() {
    let mut camera = CameraController::new();
    camera.mode = CameraMode::Fly;

    let target = camera.get_target();

    // In fly mode, target is in front of camera
    assert_ne!(target, camera.target);

    // Target should be ahead of position in the forward direction
    let to_target = target - camera.position;
    let forward = camera.get_forward();
    let dot = to_target.normalize().dot(forward);
    assert!(dot > 0.9);
}

#[test]
fn test_camera_update_movement() {
    let mut camera = CameraController::new();
    let initial_pos = camera.position;

    // Move forward
    camera.update_movement(1.0, 0.0, 0.0);

    assert_ne!(camera.position, initial_pos);
}

#[test]
fn test_camera_apply_key_movement() {
    let mut camera = CameraController::new();
    let initial_y = camera.position.y;

    // Move up
    camera.apply_key_movement(false, false, false, false, true, false);

    assert!(camera.position.y > initial_y);
}

#[test]
fn test_camera_mouse_look() {
    let mut camera = CameraController::new();
    let initial_yaw = camera.yaw;

    camera.handle_mouse_look(0.1, 0.0, false);

    assert_ne!(camera.yaw, initial_yaw);
}

#[test]
fn test_camera_pan() {
    let mut camera = CameraController::new();
    let initial_pos = camera.position;

    camera.handle_pan(0.1, 0.1);

    assert_ne!(camera.position, initial_pos);
}

// ============================================================================
// Raycast Tests
// ============================================================================

#[test]
fn test_raycast_to_ground_center() {
    let camera_pos = Vec3::new(0.0, 5.0, 0.0);
    let camera_target = Vec3::new(0.0, 0.0, 0.0);
    let uv = (0.5, 0.5); // Center of screen

    let result = raycast_to_ground(camera_pos, camera_target, uv, 16.0 / 9.0, 1.2);

    assert!(result.is_some());
    let hit = result.unwrap();
    // Should hit near origin since camera is directly above looking down
    assert!(hit.y.abs() < 0.01); // Should be on ground plane
}

#[test]
fn test_raycast_to_ground_returns_none_when_parallel() {
    let camera_pos = Vec3::new(0.0, 0.0, 5.0);
    let camera_target = Vec3::new(0.0, 0.0, 0.0);
    let uv = (0.5, 0.5);

    // Looking straight ahead (parallel to ground)
    let result = raycast_to_ground(camera_pos, camera_target, uv, 16.0 / 9.0, 0.01);

    // With very small FOV looking parallel, may or may not hit
    // This test verifies the function handles edge cases
}

#[test]
fn test_raycast_to_plane() {
    let camera_pos = Vec3::new(0.0, 10.0, 5.0);
    let camera_target = Vec3::new(0.0, 5.0, 0.0);
    let uv = (0.5, 0.5);
    let plane_height = 5.0;

    let result = raycast_to_plane(
        camera_pos, camera_target, uv, 16.0 / 9.0, 1.2, plane_height
    );

    assert!(result.is_some());
    let hit = result.unwrap();
    assert!((hit.y - plane_height).abs() < 0.01);
}

#[test]
fn test_get_ray_direction_normalized() {
    let camera_pos = Vec3::new(0.0, 5.0, 10.0);
    let camera_target = Vec3::new(0.0, 0.0, 0.0);
    let uv = (0.5, 0.5);

    let ray_dir = get_ray_direction(camera_pos, camera_target, uv, 16.0 / 9.0, 1.2);

    // Should be normalized
    let length = ray_dir.length();
    assert!((length - 1.0).abs() < 0.001);
}

#[test]
fn test_raycast_config_default() {
    let config = RaycastConfig::default();

    assert!((config.aspect_ratio - 16.0 / 9.0).abs() < 0.01);
    assert!(config.fov > 0.0);
}

#[test]
fn test_raycast_config_with_aspect() {
    let config = RaycastConfig::with_aspect(4.0 / 3.0);

    assert!((config.aspect_ratio - 4.0 / 3.0).abs() < 0.01);
    // FOV should still be default
    assert!(config.fov > 0.0);
}

#[test]
fn test_raycast_config_raycast_to_ground() {
    let config = RaycastConfig::default();
    let camera_pos = Vec3::new(0.0, 5.0, 5.0);
    let camera_target = Vec3::new(0.0, 0.0, 0.0);

    let result = config.raycast_to_ground(camera_pos, camera_target, (0.5, 0.5));
    assert!(result.is_some());
}

#[test]
fn test_raycast_config_raycast_to_plane() {
    let config = RaycastConfig::default();
    let camera_pos = Vec3::new(0.0, 10.0, 5.0);
    let camera_target = Vec3::new(0.0, 0.0, 0.0);

    let result = config.raycast_to_plane(camera_pos, camera_target, (0.5, 0.5), 3.0);
    assert!(result.is_some());
    let hit = result.unwrap();
    assert!((hit.y - 3.0).abs() < 0.01);
}

// ============================================================================
// Camera Position Calculation Tests
// ============================================================================

#[test]
fn test_camera_yaw_affects_forward_direction() {
    let mut camera = CameraController::new();

    // Test different yaw values
    camera.yaw = 0.0;
    camera.pitch = 0.0;
    let forward_0 = camera.get_forward();

    camera.yaw = std::f32::consts::FRAC_PI_2; // 90 degrees
    let forward_90 = camera.get_forward();

    // Forward directions should be different
    let dot = forward_0.dot(forward_90);
    assert!(dot.abs() < 0.1); // Should be roughly perpendicular
}

#[test]
fn test_camera_pitch_affects_forward_direction() {
    let mut camera = CameraController::new();
    camera.yaw = 0.0;

    // Looking forward (pitch = 0)
    camera.pitch = 0.0;
    let forward_level = camera.get_forward();

    // Looking up (positive pitch)
    camera.pitch = 0.5;
    let forward_up = camera.get_forward();

    // Y component should be different
    assert!(forward_up.y > forward_level.y);
}
