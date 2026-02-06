//! Orbit Camera for 3D Asset Preview
//!
//! A spherical-coordinate orbit camera designed for the Asset Editor.
//! Used in stages 2-5 (Extrude, Sculpt, Color, Save) to inspect 3D meshes
//! from all angles. Stage 1 (Draw2D) uses an orthographic projection instead.
//!
//! Controls:
//! - Middle mouse drag: Orbit (rotate around target)
//! - Right mouse drag: Pan (translate target point)
//! - Scroll wheel: Zoom (change distance from target)

use glam::{Mat4, Vec3};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Default horizontal angle in degrees.
const DEFAULT_AZIMUTH: f32 = 30.0;
/// Default vertical angle in degrees.
const DEFAULT_ELEVATION: f32 = 25.0;
/// Default distance from target.
const DEFAULT_DISTANCE: f32 = 5.0;
/// Default field of view in degrees.
const DEFAULT_FOV: f32 = 45.0;
/// Near clip plane.
const DEFAULT_NEAR: f32 = 0.01;
/// Far clip plane.
const DEFAULT_FAR: f32 = 100.0;

/// Minimum zoom distance.
const MIN_DISTANCE: f32 = 0.5;
/// Maximum zoom distance.
const MAX_DISTANCE: f32 = 50.0;

/// Minimum elevation angle in degrees (prevent gimbal lock).
const MIN_ELEVATION: f32 = -89.0;
/// Maximum elevation angle in degrees (prevent gimbal lock).
const MAX_ELEVATION: f32 = 89.0;

/// Orbit sensitivity: degrees per pixel of mouse movement.
const ORBIT_SENSITIVITY: f32 = 0.3;
/// Pan sensitivity factor: multiplied by distance for depth-proportional panning.
const PAN_SENSITIVITY: f32 = 0.005;
/// Scroll zoom factor: how much each scroll tick affects distance.
const SCROLL_FACTOR: f32 = 0.1;

// ============================================================================
// MOUSE BUTTON ENUM
// ============================================================================

/// Mouse buttons relevant to the orbit camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrbitMouseButton {
    /// Middle mouse button -- used for orbiting.
    Middle,
    /// Right mouse button -- used for panning.
    Right,
}

// ============================================================================
// ORBIT CAMERA
// ============================================================================

/// A spherical-coordinate orbit camera for 3D asset preview.
///
/// The camera orbits around a `target` point using spherical coordinates
/// (azimuth, elevation, distance). This is the standard camera model used
/// in 3D modeling tools (Blender, Maya, etc.).
///
/// # Coordinate System
/// - Azimuth: horizontal angle in degrees (wraps 0-360)
/// - Elevation: vertical angle in degrees (clamped -89 to 89)
/// - Distance: zoom distance from target (clamped 0.5 to 50.0)
/// - Y is up
#[derive(Debug, Clone)]
pub struct OrbitCamera {
    /// Horizontal angle in degrees (wraps around).
    pub azimuth: f32,
    /// Vertical angle in degrees (clamped to [-89, 89]).
    pub elevation: f32,
    /// Distance from the target point (clamped to [0.5, 50.0]).
    pub distance: f32,
    /// The point the camera orbits around.
    pub target: Vec3,
    /// Viewport aspect ratio (width / height).
    pub aspect: f32,
    /// Vertical field of view in degrees.
    pub fov: f32,
    /// Near clip plane distance.
    pub near: f32,
    /// Far clip plane distance.
    pub far: f32,

    // -- Mouse interaction state --
    /// Whether the user is currently orbiting (middle mouse held).
    is_orbiting: bool,
    /// Whether the user is currently panning (right mouse held).
    is_panning: bool,
    /// Last known mouse position for computing deltas.
    last_mouse: [f32; 2],
}

impl OrbitCamera {
    /// Create a new orbit camera with sensible defaults.
    ///
    /// The initial view is slightly above and to the side of the target,
    /// giving a good overview of the asset being edited.
    pub fn new(aspect: f32) -> Self {
        Self {
            azimuth: DEFAULT_AZIMUTH,
            elevation: DEFAULT_ELEVATION,
            distance: DEFAULT_DISTANCE,
            target: Vec3::ZERO,
            aspect,
            fov: DEFAULT_FOV,
            near: DEFAULT_NEAR,
            far: DEFAULT_FAR,
            is_orbiting: false,
            is_panning: false,
            last_mouse: [0.0, 0.0],
        }
    }

    // ========================================================================
    // MATRIX COMPUTATION
    // ========================================================================

    /// Compute the eye (camera) position from spherical coordinates.
    ///
    /// Converts (azimuth, elevation, distance) to a Cartesian offset from
    /// the target point.
    fn eye_position(&self) -> Vec3 {
        let azim_rad = self.azimuth.to_radians();
        let elev_rad = self.elevation.to_radians();

        let cos_elev = elev_rad.cos();
        let offset = Vec3::new(
            self.distance * cos_elev * azim_rad.sin(),
            self.distance * elev_rad.sin(),
            self.distance * cos_elev * azim_rad.cos(),
        );

        self.target + offset
    }

    /// Compute the view (look-at) matrix from the current spherical coordinates.
    ///
    /// Returns a 4x4 column-major matrix as `[[f32; 4]; 4]` suitable for
    /// passing to wgpu uniform buffers.
    pub fn view_matrix(&self) -> [[f32; 4]; 4] {
        let eye = self.eye_position();
        let up = Vec3::Y;

        let mat = Mat4::look_at_rh(eye, self.target, up);
        mat.to_cols_array_2d()
    }

    /// Compute the perspective projection matrix.
    ///
    /// Returns a 4x4 column-major matrix as `[[f32; 4]; 4]` suitable for
    /// passing to wgpu uniform buffers. Uses a right-handed coordinate system
    /// with depth range [0, 1] (wgpu convention).
    pub fn projection_matrix(&self) -> [[f32; 4]; 4] {
        let mat = Mat4::perspective_rh(self.fov.to_radians(), self.aspect, self.near, self.far);
        mat.to_cols_array_2d()
    }

    /// Compute the combined view-projection matrix.
    ///
    /// Returns `projection * view` as a 4x4 column-major matrix.
    pub fn view_projection_matrix(&self) -> [[f32; 4]; 4] {
        let view = Mat4::look_at_rh(self.eye_position(), self.target, Vec3::Y);
        let proj = Mat4::perspective_rh(self.fov.to_radians(), self.aspect, self.near, self.far);

        let vp = proj * view;
        vp.to_cols_array_2d()
    }

    // ========================================================================
    // INPUT HANDLING
    // ========================================================================

    /// Handle a mouse button press or release.
    ///
    /// - Middle button: starts/stops orbiting
    /// - Right button: starts/stops panning
    ///
    /// Records the current mouse position on press so that subsequent
    /// `handle_mouse_move` calls can compute correct deltas.
    pub fn handle_mouse_drag(&mut self, button: OrbitMouseButton, pressed: bool) {
        match button {
            OrbitMouseButton::Middle => {
                self.is_orbiting = pressed;
            }
            OrbitMouseButton::Right => {
                self.is_panning = pressed;
            }
        }
    }

    /// Handle mouse movement. Call this on every `CursorMoved` event.
    ///
    /// - While orbiting (middle mouse held): dx rotates azimuth, dy rotates elevation
    /// - While panning (right mouse held): moves the target in camera-local right/up
    /// - Always updates `last_mouse` for the next delta computation
    pub fn handle_mouse_move(&mut self, x: f32, y: f32) {
        let dx = x - self.last_mouse[0];
        let dy = y - self.last_mouse[1];

        if self.is_orbiting {
            self.azimuth += dx * ORBIT_SENSITIVITY;
            self.elevation =
                (self.elevation - dy * ORBIT_SENSITIVITY).clamp(MIN_ELEVATION, MAX_ELEVATION);
        }

        if self.is_panning {
            self.handle_pan(
                -dx * PAN_SENSITIVITY * self.distance,
                dy * PAN_SENSITIVITY * self.distance,
            );
        }

        self.last_mouse = [x, y];
    }

    /// Handle scroll wheel input for zooming.
    ///
    /// Uses multiplicative zoom so that zooming feels consistent at all
    /// distances. Positive delta zooms in, negative zooms out.
    /// Distance is clamped to [`MIN_DISTANCE`, `MAX_DISTANCE`].
    pub fn handle_scroll(&mut self, delta: f32) {
        self.distance *= 1.0 - delta * SCROLL_FACTOR;
        self.distance = self.distance.clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    /// Pan the camera target in camera-local right/up directions.
    ///
    /// Computes the camera's local right and up vectors from the current
    /// azimuth and elevation, then offsets the target accordingly.
    fn handle_pan(&mut self, dx: f32, dy: f32) {
        let azim_rad = self.azimuth.to_radians();
        let elev_rad = self.elevation.to_radians();

        // Camera forward direction (from target toward eye)
        let cos_elev = elev_rad.cos();
        let forward = Vec3::new(
            cos_elev * azim_rad.sin(),
            elev_rad.sin(),
            cos_elev * azim_rad.cos(),
        );

        // Right vector: cross(forward, world_up), normalized
        let world_up = Vec3::Y;
        let right = forward.cross(world_up).normalize();

        // Camera-local up: cross(right, forward)
        let up = right.cross(forward).normalize();

        // Offset the target
        self.target += right * dx + up * dy;
    }

    /// Update the viewport aspect ratio after a window resize.
    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.aspect = width as f32 / height as f32;
        }
    }

    // ========================================================================
    // QUERIES
    // ========================================================================

    /// Returns `true` if the camera is currently in an interactive drag state
    /// (orbiting or panning).
    pub fn is_active(&self) -> bool {
        self.is_orbiting || self.is_panning
    }

    /// Reset the camera to default view settings.
    pub fn reset(&mut self) {
        self.azimuth = DEFAULT_AZIMUTH;
        self.elevation = DEFAULT_ELEVATION;
        self.distance = DEFAULT_DISTANCE;
        self.target = Vec3::ZERO;
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    #[test]
    fn test_new_defaults() {
        let cam = OrbitCamera::new(16.0 / 9.0);
        assert!(approx_eq(cam.azimuth, 30.0));
        assert!(approx_eq(cam.elevation, 25.0));
        assert!(approx_eq(cam.distance, 5.0));
        assert!(approx_eq(cam.target.x, 0.0));
        assert!(approx_eq(cam.target.y, 0.0));
        assert!(approx_eq(cam.target.z, 0.0));
        assert!(approx_eq(cam.fov, 45.0));
    }

    #[test]
    fn test_view_matrix_is_valid() {
        let cam = OrbitCamera::new(1.0);
        let view = cam.view_matrix();
        // The view matrix should not be all zeros
        let sum: f32 = view
            .iter()
            .flat_map(|row| row.iter())
            .map(|v| v.abs())
            .sum();
        assert!(sum > 0.0, "View matrix should not be all zeros");
    }

    #[test]
    fn test_projection_matrix_is_valid() {
        let cam = OrbitCamera::new(16.0 / 9.0);
        let proj = cam.projection_matrix();
        // proj[0][0] should be related to fov and aspect
        assert!(proj[0][0] > 0.0, "Projection [0][0] should be positive");
        assert!(proj[1][1] > 0.0, "Projection [1][1] should be positive");
        // Last column of perspective matrix
        assert!(approx_eq(proj[3][3], 0.0), "Perspective [3][3] should be 0");
    }

    #[test]
    fn test_orbit_changes_azimuth_elevation() {
        let mut cam = OrbitCamera::new(1.0);
        cam.handle_mouse_drag(OrbitMouseButton::Middle, true);

        // Simulate cursor at initial position
        cam.last_mouse = [100.0, 100.0];
        cam.handle_mouse_move(200.0, 150.0);

        // azimuth should have increased by (200-100)*0.3 = 30 degrees
        assert!(approx_eq(cam.azimuth, 30.0 + 30.0));
        // elevation should have decreased by (150-100)*0.3 = 15 degrees
        assert!(approx_eq(cam.elevation, 25.0 - 15.0));
    }

    #[test]
    fn test_elevation_clamped() {
        let mut cam = OrbitCamera::new(1.0);
        cam.handle_mouse_drag(OrbitMouseButton::Middle, true);
        cam.last_mouse = [100.0, 100.0];

        // Move mouse far down to try to exceed 89 degrees
        cam.handle_mouse_move(100.0, -300.0);
        assert!(cam.elevation <= 89.0);

        // Move mouse far up to try to go below -89 degrees
        cam.last_mouse = [100.0, 100.0];
        cam.handle_mouse_move(100.0, 900.0);
        assert!(cam.elevation >= -89.0);
    }

    #[test]
    fn test_scroll_zoom_in() {
        let mut cam = OrbitCamera::new(1.0);
        let initial_dist = cam.distance;
        cam.handle_scroll(1.0); // Positive = zoom in
        assert!(
            cam.distance < initial_dist,
            "Scroll in should decrease distance"
        );
    }

    #[test]
    fn test_scroll_zoom_out() {
        let mut cam = OrbitCamera::new(1.0);
        let initial_dist = cam.distance;
        cam.handle_scroll(-1.0); // Negative = zoom out
        assert!(
            cam.distance > initial_dist,
            "Scroll out should increase distance"
        );
    }

    #[test]
    fn test_scroll_clamp_min() {
        let mut cam = OrbitCamera::new(1.0);
        cam.distance = 0.6;
        // Zoom in a lot
        for _ in 0..100 {
            cam.handle_scroll(2.0);
        }
        assert!(
            cam.distance >= MIN_DISTANCE,
            "Distance should not go below MIN_DISTANCE"
        );
    }

    #[test]
    fn test_scroll_clamp_max() {
        let mut cam = OrbitCamera::new(1.0);
        cam.distance = 40.0;
        // Zoom out a lot
        for _ in 0..100 {
            cam.handle_scroll(-2.0);
        }
        assert!(
            cam.distance <= MAX_DISTANCE,
            "Distance should not exceed MAX_DISTANCE"
        );
    }

    #[test]
    fn test_pan_moves_target() {
        let mut cam = OrbitCamera::new(1.0);
        let initial_target = cam.target;
        cam.handle_mouse_drag(OrbitMouseButton::Right, true);
        cam.last_mouse = [100.0, 100.0];
        cam.handle_mouse_move(200.0, 200.0);

        assert!(
            cam.target != initial_target,
            "Panning should move the target"
        );
    }

    #[test]
    fn test_resize_updates_aspect() {
        let mut cam = OrbitCamera::new(1.0);
        cam.resize(1920, 1080);
        assert!(approx_eq(cam.aspect, 1920.0 / 1080.0));
    }

    #[test]
    fn test_resize_zero_ignored() {
        let mut cam = OrbitCamera::new(1.5);
        cam.resize(0, 0);
        assert!(approx_eq(cam.aspect, 1.5), "Zero resize should be ignored");
    }

    #[test]
    fn test_is_active() {
        let mut cam = OrbitCamera::new(1.0);
        assert!(!cam.is_active());

        cam.handle_mouse_drag(OrbitMouseButton::Middle, true);
        assert!(cam.is_active());

        cam.handle_mouse_drag(OrbitMouseButton::Middle, false);
        assert!(!cam.is_active());

        cam.handle_mouse_drag(OrbitMouseButton::Right, true);
        assert!(cam.is_active());
    }

    #[test]
    fn test_reset() {
        let mut cam = OrbitCamera::new(1.0);
        cam.azimuth = 180.0;
        cam.elevation = -45.0;
        cam.distance = 20.0;
        cam.target = Vec3::new(5.0, 3.0, 1.0);

        cam.reset();
        assert!(approx_eq(cam.azimuth, DEFAULT_AZIMUTH));
        assert!(approx_eq(cam.elevation, DEFAULT_ELEVATION));
        assert!(approx_eq(cam.distance, DEFAULT_DISTANCE));
        assert!(approx_eq(cam.target.x, 0.0));
    }

    #[test]
    fn test_view_projection_combines_correctly() {
        let cam = OrbitCamera::new(16.0 / 9.0);
        let vp = cam.view_projection_matrix();

        // Manually compute to verify
        let view = Mat4::from_cols_array_2d(&cam.view_matrix());
        let proj = Mat4::from_cols_array_2d(&cam.projection_matrix());
        let expected = proj * view;
        let expected_arr = expected.to_cols_array_2d();

        for i in 0..4 {
            for j in 0..4 {
                assert!(
                    (vp[i][j] - expected_arr[i][j]).abs() < 1e-4,
                    "VP matrix mismatch at [{i}][{j}]: {} vs {}",
                    vp[i][j],
                    expected_arr[i][j]
                );
            }
        }
    }

    #[test]
    fn test_no_movement_without_drag() {
        let mut cam = OrbitCamera::new(1.0);
        let initial_azimuth = cam.azimuth;
        let initial_elevation = cam.elevation;
        let initial_target = cam.target;

        // Move mouse without pressing any button
        cam.last_mouse = [100.0, 100.0];
        cam.handle_mouse_move(200.0, 200.0);

        assert!(approx_eq(cam.azimuth, initial_azimuth));
        assert!(approx_eq(cam.elevation, initial_elevation));
        assert!(cam.target == initial_target);
    }
}
