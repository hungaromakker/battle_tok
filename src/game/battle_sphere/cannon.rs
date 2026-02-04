//! Cannon Module
//!
//! Provides the Cannon struct with aiming controls and SDF generation.
//! The cannon can aim using barrel_elevation (pitch) and barrel_azimuth (yaw).
//!
//! # Controls
//! - Arrow UP/DOWN: adjust barrel_elevation (clamped -10 to 45 degrees)
//! - Arrow LEFT/RIGHT: adjust barrel_azimuth (clamped -45 to 45 degrees)
//!
//! # Example
//! ```ignore
//! use magic_engine::game::battle_sphere::cannon::Cannon;
//! use glam::Vec3;
//!
//! let mut cannon = Cannon::new(Vec3::new(0.0, 1.0, -50.0));
//! cannon.aim_up(0.5); // Increase elevation
//! let sdf = cannon.to_sdf();
//! let (barrel_tip, direction) = cannon.get_barrel_tip_and_direction();
//! ```

use glam::{Mat4, Quat, Vec3};

use crate::rendering::sdf_objects::{SdfObject, SdfOperation, SdfPrimitive};

/// Cannon dimensions (in meters)
pub mod dimensions {
    /// Barrel radius
    pub const BARREL_RADIUS: f32 = 0.3;
    /// Barrel length from pivot point to tip
    pub const BARREL_LENGTH: f32 = 4.0;
    /// Body half-extents (x, y, z)
    pub const BODY_HALF_EXTENTS: (f32, f32, f32) = (1.0, 0.5, 0.75);
    /// Body rounding radius
    pub const BODY_ROUNDING: f32 = 0.1;
    /// Smooth union blend factor
    pub const SMOOTH_UNION_K: f32 = 0.3;
    /// Height of the barrel pivot above the cannon base
    pub const BARREL_PIVOT_HEIGHT: f32 = 0.5;
}

/// Cannon aiming limits (in degrees)
pub mod limits {
    /// Minimum barrel elevation (degrees) - aiming down
    pub const MIN_ELEVATION: f32 = -10.0;
    /// Maximum barrel elevation (degrees) - aiming up
    pub const MAX_ELEVATION: f32 = 45.0;
    /// Minimum barrel azimuth (degrees) - aiming left
    pub const MIN_AZIMUTH: f32 = -45.0;
    /// Maximum barrel azimuth (degrees) - aiming right
    pub const MAX_AZIMUTH: f32 = 45.0;
}

/// Cannon aim speed settings
pub mod aim_speed {
    /// Base rotation speed in degrees per second
    pub const ROTATION_SPEED: f32 = 30.0;
    /// Smoothing factor for interpolation (0.0-1.0, higher = faster response)
    pub const SMOOTHING_FACTOR: f32 = 0.15;
}

/// A siege cannon with aiming controls.
///
/// The cannon consists of a body (rounded box) and a barrel (cylinder).
/// The barrel can be aimed using elevation (up/down) and azimuth (left/right).
#[derive(Debug, Clone)]
pub struct Cannon {
    /// Position of the cannon base in world space
    pub position: Vec3,
    /// Current barrel elevation angle in degrees (-10 to 45)
    pub barrel_elevation: f32,
    /// Current barrel azimuth angle in degrees (-45 to 45)
    pub barrel_azimuth: f32,
    /// Target barrel elevation for smooth interpolation
    target_elevation: f32,
    /// Target barrel azimuth for smooth interpolation
    target_azimuth: f32,
    /// Base rotation of the entire cannon (Y-axis rotation)
    pub base_rotation: f32,
}

impl Default for Cannon {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            barrel_elevation: 15.0, // Default: 15 degrees up
            barrel_azimuth: 0.0,    // Default: center
            target_elevation: 15.0,
            target_azimuth: 0.0,
            base_rotation: 0.0,
        }
    }
}

impl Cannon {
    /// Create a new cannon at the given position.
    ///
    /// # Arguments
    /// * `position` - World position for the cannon base
    pub fn new(position: Vec3) -> Self {
        Self {
            position,
            ..Default::default()
        }
    }

    /// Create a new cannon at the given position facing a direction.
    ///
    /// # Arguments
    /// * `position` - World position for the cannon base
    /// * `facing_direction` - Direction the cannon should face (XZ plane)
    pub fn new_facing(position: Vec3, facing_direction: Vec3) -> Self {
        let mut cannon = Self::new(position);
        // Calculate base rotation from facing direction
        let facing_xz = Vec3::new(facing_direction.x, 0.0, facing_direction.z).normalize_or_zero();
        cannon.base_rotation = facing_xz.x.atan2(facing_xz.z);
        cannon
    }

    /// Adjust barrel elevation (positive = up, negative = down).
    /// Uses smooth interpolation - call update() each frame.
    ///
    /// # Arguments
    /// * `delta` - Change in degrees
    pub fn aim_up(&mut self, delta: f32) {
        self.target_elevation = (self.target_elevation + delta)
            .clamp(limits::MIN_ELEVATION, limits::MAX_ELEVATION);
    }

    /// Adjust barrel azimuth (positive = right, negative = left).
    /// Uses smooth interpolation - call update() each frame.
    ///
    /// # Arguments
    /// * `delta` - Change in degrees
    pub fn aim_right(&mut self, delta: f32) {
        self.target_azimuth = (self.target_azimuth + delta)
            .clamp(limits::MIN_AZIMUTH, limits::MAX_AZIMUTH);
    }

    /// Directly set target elevation (degrees).
    pub fn set_target_elevation(&mut self, elevation: f32) {
        self.target_elevation = elevation.clamp(limits::MIN_ELEVATION, limits::MAX_ELEVATION);
    }

    /// Directly set target azimuth (degrees).
    pub fn set_target_azimuth(&mut self, azimuth: f32) {
        self.target_azimuth = azimuth.clamp(limits::MIN_AZIMUTH, limits::MAX_AZIMUTH);
    }

    /// Update the cannon state for smooth interpolation.
    /// Call this once per frame.
    ///
    /// # Arguments
    /// * `delta_time` - Time since last frame in seconds
    pub fn update(&mut self, delta_time: f32) {
        // Smooth interpolation toward target angles
        let smoothing = 1.0 - (1.0 - aim_speed::SMOOTHING_FACTOR).powf(delta_time * 60.0);

        self.barrel_elevation +=
            (self.target_elevation - self.barrel_elevation) * smoothing;
        self.barrel_azimuth +=
            (self.target_azimuth - self.barrel_azimuth) * smoothing;
    }

    /// Get the barrel rotation quaternion (combines elevation and azimuth).
    pub fn get_barrel_rotation(&self) -> Quat {
        let elevation_rad = self.barrel_elevation.to_radians();
        let azimuth_rad = self.barrel_azimuth.to_radians();
        let base_rad = self.base_rotation;

        // Apply rotations: base rotation (Y) * azimuth (Y) * elevation (X)
        Quat::from_rotation_y(base_rad)
            * Quat::from_rotation_y(azimuth_rad)
            * Quat::from_rotation_x(elevation_rad)
    }

    /// Get the barrel pivot position in world space.
    fn get_barrel_pivot(&self) -> Vec3 {
        self.position + Vec3::new(0.0, dimensions::BARREL_PIVOT_HEIGHT, 0.0)
    }

    /// Get the barrel tip position and firing direction in world space.
    ///
    /// Returns `(tip_position, direction)` where direction is normalized.
    pub fn get_barrel_tip_and_direction(&self) -> (Vec3, Vec3) {
        let rotation = self.get_barrel_rotation();
        let pivot = self.get_barrel_pivot();

        // Forward direction for the barrel (local +Z becomes firing direction)
        let local_forward = Vec3::new(0.0, 0.0, 1.0);
        let direction = rotation * local_forward;

        // Tip position = pivot + direction * barrel_length
        let tip_position = pivot + direction * dimensions::BARREL_LENGTH;

        (tip_position, direction)
    }

    /// Convert the cannon to an SDF object for rendering.
    pub fn to_sdf(&self) -> SdfObject {
        // Calculate transforms
        let base_rotation_mat = Mat4::from_rotation_y(self.base_rotation);
        let world_transform = Mat4::from_translation(self.position) * base_rotation_mat;

        // Barrel rotation (relative to body)
        let barrel_rotation = Quat::from_rotation_y(self.barrel_azimuth.to_radians())
            * Quat::from_rotation_x(self.barrel_elevation.to_radians());
        let barrel_rotation_mat = Mat4::from_quat(barrel_rotation);

        // Barrel: cylinder oriented along Z-axis
        // Position at pivot height, rotated, then offset along Z to center the barrel
        let barrel_local_offset = Vec3::new(0.0, 0.0, dimensions::BARREL_LENGTH / 2.0);
        let rotated_offset = barrel_rotation * barrel_local_offset;
        let barrel_position = Vec3::new(0.0, dimensions::BARREL_PIVOT_HEIGHT, 0.0) + rotated_offset;

        // Create the barrel transform: translate to position, then rotate
        let barrel_transform =
            Mat4::from_translation(barrel_position) * barrel_rotation_mat;

        // Body: rounded box at base
        let body_transform = Mat4::IDENTITY;

        SdfObject::new()
            // Barrel (cylinder)
            .with_primitive(
                SdfPrimitive::Cylinder {
                    radius: dimensions::BARREL_RADIUS,
                    height: dimensions::BARREL_LENGTH,
                },
                barrel_transform,
            )
            // Body (rounded box)
            .with_primitive(
                SdfPrimitive::RoundedBox {
                    half_extents: Vec3::new(
                        dimensions::BODY_HALF_EXTENTS.0,
                        dimensions::BODY_HALF_EXTENTS.1,
                        dimensions::BODY_HALF_EXTENTS.2,
                    ),
                    radius: dimensions::BODY_ROUNDING,
                },
                body_transform,
            )
            .with_operation(SdfOperation::SmoothUnion {
                k: dimensions::SMOOTH_UNION_K,
            })
            .with_transform(world_transform)
    }

    /// Check if cannon is currently aiming (not at rest).
    pub fn is_aiming(&self) -> bool {
        let elevation_diff = (self.target_elevation - self.barrel_elevation).abs();
        let azimuth_diff = (self.target_azimuth - self.barrel_azimuth).abs();
        elevation_diff > 0.01 || azimuth_diff > 0.01
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cannon_default() {
        let cannon = Cannon::default();
        assert_eq!(cannon.position, Vec3::ZERO);
        assert_eq!(cannon.barrel_elevation, 15.0);
        assert_eq!(cannon.barrel_azimuth, 0.0);
    }

    #[test]
    fn test_cannon_new() {
        let pos = Vec3::new(10.0, 5.0, -50.0);
        let cannon = Cannon::new(pos);
        assert_eq!(cannon.position, pos);
    }

    #[test]
    fn test_aim_up_clamped() {
        let mut cannon = Cannon::default();
        cannon.aim_up(100.0); // Try to go beyond max
        assert_eq!(cannon.target_elevation, limits::MAX_ELEVATION);

        cannon.aim_up(-200.0); // Try to go beyond min
        assert_eq!(cannon.target_elevation, limits::MIN_ELEVATION);
    }

    #[test]
    fn test_aim_right_clamped() {
        let mut cannon = Cannon::default();
        cannon.aim_right(100.0);
        assert_eq!(cannon.target_azimuth, limits::MAX_AZIMUTH);

        cannon.aim_right(-200.0);
        assert_eq!(cannon.target_azimuth, limits::MIN_AZIMUTH);
    }

    #[test]
    fn test_barrel_tip_at_default() {
        let cannon = Cannon::default();
        let (tip, direction) = cannon.get_barrel_tip_and_direction();

        // At 15 degrees elevation, 0 azimuth:
        // When rotating around X axis by positive angle, +Z rotates toward -Y (right-hand rule)
        // So direction.y = -sin(15), direction.z = cos(15)
        let elevation_rad = 15.0_f32.to_radians();
        let expected_dir = Vec3::new(0.0, -elevation_rad.sin(), elevation_rad.cos()).normalize();

        assert!(
            (direction.x - expected_dir.x).abs() < 0.01,
            "x mismatch: got {}, expected {}",
            direction.x,
            expected_dir.x
        );
        assert!(
            (direction.y - expected_dir.y).abs() < 0.01,
            "y mismatch: got {}, expected {}",
            direction.y,
            expected_dir.y
        );
        assert!(
            (direction.z - expected_dir.z).abs() < 0.01,
            "z mismatch: got {}, expected {}",
            direction.z,
            expected_dir.z
        );

        // Tip should be barrel_length away from pivot in that direction
        let expected_tip = cannon.get_barrel_pivot() + expected_dir * dimensions::BARREL_LENGTH;
        assert!((tip - expected_tip).length() < 0.01);
    }

    #[test]
    fn test_to_sdf_has_parts() {
        let cannon = Cannon::default();
        let sdf = cannon.to_sdf();

        assert_eq!(sdf.primitive_count(), 2); // Barrel + body
        assert_eq!(sdf.operation_count(), 1); // SmoothUnion
    }

    #[test]
    fn test_smooth_interpolation() {
        let mut cannon = Cannon::default();
        cannon.aim_up(30.0); // Target = 45

        // After one update, should have moved toward target
        cannon.update(0.016); // ~60fps
        assert!(cannon.barrel_elevation > 15.0);
        assert!(cannon.barrel_elevation < 45.0);
    }

    #[test]
    fn test_is_aiming() {
        let mut cannon = Cannon::default();
        assert!(!cannon.is_aiming());

        cannon.aim_up(10.0);
        assert!(cannon.is_aiming());

        // Simulate reaching target
        cannon.barrel_elevation = cannon.target_elevation;
        cannon.barrel_azimuth = cannon.target_azimuth;
        assert!(!cannon.is_aiming());
    }
}
