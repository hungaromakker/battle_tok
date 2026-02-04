//! FPS Camera Controller Module
//!
//! Provides a simple first-person shooter style camera controller where
//! mouse movement directly rotates the camera without requiring any button
//! to be held down. This is ideal for FPS games where the cursor is locked
//! to the center of the screen.
//!
//! Key features:
//! - Direct mouse input → camera rotation (no button required)
//! - Configurable sensitivity (default: 0.002 rad/pixel)
//! - Pitch clamped to ±89 degrees to prevent gimbal lock
//! - NO smoothing - instant response for precise aiming

use glam::Vec3;

/// Pitch limit constant: -89 degrees in radians
const PITCH_LIMIT_MIN: f32 = -89.0 * std::f32::consts::PI / 180.0;
/// Pitch limit constant: +89 degrees in radians
const PITCH_LIMIT_MAX: f32 = 89.0 * std::f32::consts::PI / 180.0;

/// FPS Camera Controller
///
/// A simple, responsive camera controller designed for FPS-style games.
/// Mouse movement directly rotates the camera with no smoothing or interpolation.
///
/// ## Usage
/// ```rust,ignore
/// let mut camera = FPSCameraController::new();
///
/// // In your input loop, pass raw mouse delta (in pixels)
/// camera.apply_mouse_delta(mouse_dx, mouse_dy);
///
/// // Get direction vectors for rendering or movement
/// let forward = camera.get_forward();
/// let right = camera.get_right();
/// let up = camera.get_up();
/// ```
#[derive(Clone, Debug)]
pub struct FPSCameraController {
    /// Camera position in world space
    pub position: Vec3,
    /// Horizontal angle (radians) - unrestricted, wraps around
    pub yaw: f32,
    /// Vertical angle (radians) - clamped to pitch_limits
    pub pitch: f32,
    /// Mouse sensitivity in radians per pixel (default: 0.002)
    pub sensitivity: f32,
    /// Pitch limits (min, max) in radians
    pitch_limits: (f32, f32),
}

impl Default for FPSCameraController {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            sensitivity: 0.002, // 0.002 rad/pixel as per spec
            pitch_limits: (PITCH_LIMIT_MIN, PITCH_LIMIT_MAX),
        }
    }
}

impl FPSCameraController {
    /// Create a new FPS camera controller with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an FPS camera controller with a custom position
    pub fn with_position(position: Vec3) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    /// Create an FPS camera controller with custom sensitivity
    pub fn with_sensitivity(sensitivity: f32) -> Self {
        Self {
            sensitivity,
            ..Default::default()
        }
    }

    /// Get the current camera position
    #[inline]
    pub fn get_position(&self) -> Vec3 {
        self.position
    }

    /// Set the camera position directly
    #[inline]
    pub fn set_position(&mut self, position: Vec3) {
        self.position = position;
    }

    /// Get the current yaw angle in radians
    #[inline]
    pub fn get_yaw(&self) -> f32 {
        self.yaw
    }

    /// Set the yaw angle directly (in radians)
    #[inline]
    pub fn set_yaw(&mut self, yaw: f32) {
        self.yaw = yaw;
    }

    /// Get the current pitch angle in radians
    #[inline]
    pub fn get_pitch(&self) -> f32 {
        self.pitch
    }

    /// Set the pitch angle directly (in radians, will be clamped to limits)
    #[inline]
    pub fn set_pitch(&mut self, pitch: f32) {
        self.pitch = pitch.clamp(self.pitch_limits.0, self.pitch_limits.1);
    }

    /// Get the current mouse sensitivity in radians per pixel
    #[inline]
    pub fn get_sensitivity(&self) -> f32 {
        self.sensitivity
    }

    /// Set the mouse sensitivity in radians per pixel
    #[inline]
    pub fn set_sensitivity(&mut self, sensitivity: f32) {
        self.sensitivity = sensitivity;
    }

    /// Apply mouse movement delta to rotate the camera
    ///
    /// This is the primary input method. Pass raw mouse delta values (in pixels)
    /// and the camera will rotate accordingly. The response is instant with no
    /// smoothing or interpolation.
    ///
    /// # Arguments
    /// * `dx` - Mouse movement in X (pixels). Positive = move right = look right (increase yaw)
    /// * `dy` - Mouse movement in Y (pixels). Positive = move down = look down (decrease pitch)
    ///
    /// # Notes
    /// - Standard FPS convention: moving mouse right turns camera right
    /// - Standard FPS convention: moving mouse down (positive dy) looks down
    /// - Pitch is clamped to ±89 degrees to prevent gimbal lock
    /// - No smoothing is applied - movement is instantaneous
    pub fn apply_mouse_delta(&mut self, dx: f32, dy: f32) {
        // Apply sensitivity to convert pixels to radians
        // Positive dx = mouse moved right = look right = increase yaw
        self.yaw += dx * self.sensitivity;

        // Positive dy = mouse moved down = look down = decrease pitch
        // Note: pitch is positive up, negative down, so we subtract
        self.pitch -= dy * self.sensitivity;

        // Clamp pitch to prevent gimbal lock and camera flip
        self.pitch = self.pitch.clamp(self.pitch_limits.0, self.pitch_limits.1);
    }

    /// Get the camera's forward direction vector
    ///
    /// This is the direction the camera is looking, derived from yaw and pitch.
    /// The vector is normalized.
    ///
    /// # Coordinate System
    /// - +X = right
    /// - +Y = up
    /// - -Z = forward (OpenGL/Vulkan convention)
    ///
    /// When yaw=0 and pitch=0, camera looks toward -Z.
    #[inline]
    pub fn get_forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
        .normalize()
    }

    /// Get the camera's right direction vector
    ///
    /// Perpendicular to forward, in the horizontal plane.
    /// The vector is normalized.
    #[inline]
    pub fn get_right(&self) -> Vec3 {
        let forward = self.get_forward();
        forward.cross(Vec3::Y).normalize()
    }

    /// Get the camera's up direction vector
    ///
    /// Perpendicular to both forward and right.
    /// The vector is normalized.
    #[inline]
    pub fn get_up(&self) -> Vec3 {
        let forward = self.get_forward();
        let right = self.get_right();
        right.cross(forward).normalize()
    }

    /// Get the pitch limits in radians (min, max)
    #[inline]
    pub fn get_pitch_limits(&self) -> (f32, f32) {
        self.pitch_limits
    }

    /// Point the camera at a specific world position
    ///
    /// This sets the yaw and pitch to look at the given target.
    pub fn look_at(&mut self, target: Vec3) {
        let to_target = target - self.position;
        let distance = to_target.length();

        if distance > 0.001 {
            self.yaw = to_target.x.atan2(-to_target.z);
            self.pitch = (to_target.y / distance)
                .asin()
                .clamp(self.pitch_limits.0, self.pitch_limits.1);
        }
    }

    /// Reset camera orientation to default (looking toward -Z)
    pub fn reset_orientation(&mut self) {
        self.yaw = 0.0;
        self.pitch = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let camera = FPSCameraController::new();
        assert_eq!(camera.position, Vec3::ZERO);
        assert_eq!(camera.yaw, 0.0);
        assert_eq!(camera.pitch, 0.0);
        assert_eq!(camera.sensitivity, 0.002);
    }

    #[test]
    fn test_default_sensitivity_is_0002() {
        let camera = FPSCameraController::new();
        assert!((camera.sensitivity - 0.002).abs() < 0.0001);
    }

    #[test]
    fn test_pitch_limits_are_89_degrees() {
        let camera = FPSCameraController::new();
        let expected_limit = 89.0 * std::f32::consts::PI / 180.0;
        assert!((camera.pitch_limits.0 - (-expected_limit)).abs() < 0.001);
        assert!((camera.pitch_limits.1 - expected_limit).abs() < 0.001);
    }

    #[test]
    fn test_apply_mouse_delta_yaw() {
        let mut camera = FPSCameraController::new();
        camera.apply_mouse_delta(100.0, 0.0); // Move mouse right 100 pixels

        // Yaw should increase by 100 * 0.002 = 0.2 radians
        assert!((camera.yaw - 0.2).abs() < 0.001);
        assert_eq!(camera.pitch, 0.0); // Pitch unchanged
    }

    #[test]
    fn test_apply_mouse_delta_pitch() {
        let mut camera = FPSCameraController::new();
        camera.apply_mouse_delta(0.0, 100.0); // Move mouse down 100 pixels

        // Pitch should decrease by 100 * 0.002 = 0.2 radians (looking down)
        assert!((camera.pitch - (-0.2)).abs() < 0.001);
        assert_eq!(camera.yaw, 0.0); // Yaw unchanged
    }

    #[test]
    fn test_pitch_clamping_max() {
        let mut camera = FPSCameraController::new();
        // Try to look way up (negative dy = look up)
        camera.apply_mouse_delta(0.0, -100000.0);

        // Pitch should be clamped to +89 degrees
        let max_pitch = 89.0 * std::f32::consts::PI / 180.0;
        assert!((camera.pitch - max_pitch).abs() < 0.001);
    }

    #[test]
    fn test_pitch_clamping_min() {
        let mut camera = FPSCameraController::new();
        // Try to look way down (positive dy = look down)
        camera.apply_mouse_delta(0.0, 100000.0);

        // Pitch should be clamped to -89 degrees
        let min_pitch = -89.0 * std::f32::consts::PI / 180.0;
        assert!((camera.pitch - min_pitch).abs() < 0.001);
    }

    #[test]
    fn test_forward_vector_at_origin() {
        let camera = FPSCameraController::new();
        let forward = camera.get_forward();

        // When yaw=0 and pitch=0, should look towards -Z
        assert!(forward.x.abs() < 0.001);
        assert!(forward.y.abs() < 0.001);
        assert!((forward.z - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_forward_vector_normalized() {
        let mut camera = FPSCameraController::new();
        camera.apply_mouse_delta(123.0, 45.0);

        let forward = camera.get_forward();
        let length = forward.length();
        assert!((length - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_right_vector_perpendicular() {
        let mut camera = FPSCameraController::new();
        camera.apply_mouse_delta(50.0, 30.0);

        let forward = camera.get_forward();
        let right = camera.get_right();

        // Right should be perpendicular to forward
        let dot = forward.dot(right);
        assert!(dot.abs() < 0.001);
    }

    #[test]
    fn test_up_vector_perpendicular() {
        let mut camera = FPSCameraController::new();
        camera.apply_mouse_delta(50.0, 30.0);

        let forward = camera.get_forward();
        let right = camera.get_right();
        let up = camera.get_up();

        // Up should be perpendicular to both forward and right
        assert!(forward.dot(up).abs() < 0.001);
        assert!(right.dot(up).abs() < 0.001);
    }

    #[test]
    fn test_direction_vectors_normalized() {
        let mut camera = FPSCameraController::new();
        camera.apply_mouse_delta(100.0, 50.0);

        let forward = camera.get_forward();
        let right = camera.get_right();
        let up = camera.get_up();

        assert!((forward.length() - 1.0).abs() < 0.001);
        assert!((right.length() - 1.0).abs() < 0.001);
        assert!((up.length() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_with_position() {
        let pos = Vec3::new(10.0, 20.0, 30.0);
        let camera = FPSCameraController::with_position(pos);
        assert_eq!(camera.position, pos);
    }

    #[test]
    fn test_with_sensitivity() {
        let camera = FPSCameraController::with_sensitivity(0.005);
        assert_eq!(camera.sensitivity, 0.005);
    }

    #[test]
    fn test_set_pitch_clamped() {
        let mut camera = FPSCameraController::new();
        camera.set_pitch(10.0); // Way past the limit

        let max_pitch = 89.0 * std::f32::consts::PI / 180.0;
        assert!((camera.pitch - max_pitch).abs() < 0.001);
    }

    #[test]
    fn test_look_at() {
        let mut camera = FPSCameraController::new();
        camera.set_position(Vec3::new(0.0, 0.0, 10.0));
        camera.look_at(Vec3::new(0.0, 0.0, 0.0));

        // Should be looking toward -Z (forward)
        let forward = camera.get_forward();
        assert!(forward.z < 0.0);
    }

    #[test]
    fn test_reset_orientation() {
        let mut camera = FPSCameraController::new();
        camera.apply_mouse_delta(500.0, 200.0);
        assert!(camera.yaw != 0.0);
        assert!(camera.pitch != 0.0);

        camera.reset_orientation();
        assert_eq!(camera.yaw, 0.0);
        assert_eq!(camera.pitch, 0.0);
    }

    #[test]
    fn test_no_smoothing_instant_response() {
        let mut camera = FPSCameraController::new();
        let delta = 100.0;
        let expected_yaw = delta * camera.sensitivity;

        camera.apply_mouse_delta(delta, 0.0);

        // Response should be exactly expected with no smoothing
        assert!((camera.yaw - expected_yaw).abs() < 0.0001);
    }
}
