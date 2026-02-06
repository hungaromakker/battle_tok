//! Variety System
//!
//! Deterministic variety generation for asset instances. Produces unique visual
//! variations of the same base asset using seed-based pseudo-random numbers.
//! The same world position always maps to the same seed, ensuring consistent
//! appearance across sessions without storing per-instance data.
//!
//! This is a pure math/data module with no rendering or UI dependencies.

use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

// ============================================================================
// TYPES
// ============================================================================

/// Parameters controlling the range of variety applied to asset instances.
/// Each field defines a range or toggle that `generate_variety` samples from.
/// Derives `Serialize`/`Deserialize` for storage in the `.btasset` format.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VarietyParams {
    /// Minimum uniform scale factor (e.g., 0.8).
    pub scale_min: f32,
    /// Maximum uniform scale factor (e.g., 1.2).
    pub scale_max: f32,
    /// Extra Y-axis stretch range applied on top of the uniform scale (e.g., 0.1).
    pub scale_y_bias: f32,
    /// Whether to apply a full 360-degree random Y rotation.
    pub random_y_rotation: bool,
    /// Maximum tilt from vertical in degrees (e.g., 5.0).
    pub tilt_max_degrees: f32,
    /// Maximum hue shift in degrees (e.g., 15.0).
    pub hue_shift_range: f32,
    /// Maximum saturation deviation (e.g., 0.1).
    pub saturation_range: f32,
    /// Maximum brightness deviation (e.g., 0.1).
    pub brightness_range: f32,
    /// Vertex noise displacement amplitude (e.g., 0.02).
    pub noise_displacement: f32,
    /// Vertex noise frequency (e.g., 1.0).
    pub noise_frequency: f32,
}

impl Default for VarietyParams {
    fn default() -> Self {
        Self {
            scale_min: 0.8,
            scale_max: 1.2,
            scale_y_bias: 0.0,
            random_y_rotation: true,
            tilt_max_degrees: 5.0,
            hue_shift_range: 15.0,
            saturation_range: 0.1,
            brightness_range: 0.1,
            noise_displacement: 0.0,
            noise_frequency: 1.0,
        }
    }
}

/// A concrete variety instance produced by `generate_variety`. Contains the
/// sampled values that should be applied to a single asset placement.
#[derive(Clone, Debug)]
pub struct VarietyInstance {
    /// Non-uniform scale (x, y, z). Y may differ due to `scale_y_bias`.
    pub scale: Vec3,
    /// Y-axis rotation in radians.
    pub rotation_y: f32,
    /// Tilt angle from vertical in radians.
    pub tilt_angle: f32,
    /// Direction of tilt around the Y axis in radians.
    pub tilt_axis: f32,
    /// Hue shift in degrees to apply to the base color.
    pub hue_shift: f32,
    /// Saturation offset to apply to the base color.
    pub saturation_shift: f32,
    /// Brightness (value) offset to apply to the base color.
    pub brightness_shift: f32,
    /// Seed for noise displacement (for downstream vertex displacement).
    pub noise_seed: u32,
}

// ============================================================================
// SIMPLE RNG (xorshift32)
// ============================================================================

/// A minimal deterministic pseudo-random number generator using the xorshift32
/// algorithm. Given the same seed, it always produces the same sequence.
pub struct SimpleRng {
    state: u32,
}

impl SimpleRng {
    /// Create a new RNG with the given seed. A seed of 0 is bumped to 1
    /// because xorshift32 requires a non-zero state.
    pub fn new(seed: u32) -> Self {
        Self { state: seed.max(1) }
    }

    /// Advance the state and return the next pseudo-random `u32`.
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x
    }

    /// Return a pseudo-random `f32` in `[0.0, 1.0]`.
    pub fn next_f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    /// Return a pseudo-random `f32` in `[min, max]`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

// ============================================================================
// SEED GENERATION
// ============================================================================

/// Derive a deterministic seed from a world position using FNV-1a hashing.
/// The same (x, y, z) coordinates always produce the same seed, ensuring
/// assets placed at the same location look identical across sessions.
pub fn seed_from_position(x: f32, y: f32, z: f32) -> u32 {
    let ix = (x * 1000.0) as i32;
    let iy = (y * 1000.0) as i32;
    let iz = (z * 1000.0) as i32;
    // FNV-1a hash
    let mut h: u32 = 2_166_136_261;
    h ^= ix as u32;
    h = h.wrapping_mul(16_777_619);
    h ^= iy as u32;
    h = h.wrapping_mul(16_777_619);
    h ^= iz as u32;
    h = h.wrapping_mul(16_777_619);
    // xorshift32 requires a non-zero seed
    if h == 0 { 1 } else { h }
}

// ============================================================================
// VARIETY GENERATION
// ============================================================================

/// Generate a `VarietyInstance` by sampling each parameter range using a
/// deterministic RNG seeded with `seed`. The same seed and params always
/// produce the same instance.
pub fn generate_variety(params: &VarietyParams, seed: u32) -> VarietyInstance {
    let mut rng = SimpleRng::new(seed);

    // Scale
    let scale_base = rng.range(params.scale_min, params.scale_max);
    let scale_y = scale_base * (1.0 + rng.range(-params.scale_y_bias, params.scale_y_bias));

    // Rotation
    let rotation_y = if params.random_y_rotation {
        rng.range(0.0, std::f32::consts::TAU)
    } else {
        0.0
    };

    // Tilt
    let tilt_angle = rng.range(0.0, params.tilt_max_degrees.to_radians());
    let tilt_axis = rng.range(0.0, std::f32::consts::TAU);

    // Color shifts
    let hue_shift = rng.range(-params.hue_shift_range, params.hue_shift_range);
    let saturation_shift = rng.range(-params.saturation_range, params.saturation_range);
    let brightness_shift = rng.range(-params.brightness_range, params.brightness_range);

    // Noise seed for downstream vertex displacement
    let noise_seed = rng.next_u32();

    VarietyInstance {
        scale: Vec3::new(scale_base, scale_y, scale_base),
        rotation_y,
        tilt_angle,
        tilt_axis,
        hue_shift,
        saturation_shift,
        brightness_shift,
        noise_seed,
    }
}

// ============================================================================
// TRANSFORM
// ============================================================================

/// Produce a `Mat4` combining scale, Y rotation, tilt, and translation
/// from a `VarietyInstance` and a world position.
pub fn variety_to_transform(instance: &VarietyInstance, position: Vec3) -> Mat4 {
    let y_rot = Quat::from_rotation_y(instance.rotation_y);
    let tilt_dir = Vec3::new(instance.tilt_axis.cos(), 0.0, instance.tilt_axis.sin());
    let tilt_rot = Quat::from_axis_angle(tilt_dir, instance.tilt_angle);
    let rotation = tilt_rot * y_rot;
    Mat4::from_scale_rotation_translation(instance.scale, rotation, position)
}

// ============================================================================
// COLOR VARIETY
// ============================================================================

/// Apply color variety to an RGBA color using the shifts stored in a
/// `VarietyInstance`. Converts RGB to HSV, applies hue/saturation/brightness
/// offsets, clamps to valid ranges, and converts back to RGBA.
pub fn apply_color_variety(color: [f32; 4], instance: &VarietyInstance) -> [f32; 4] {
    let (h, s, v) = rgb_to_hsv(color[0], color[1], color[2]);

    // Apply shifts and clamp
    let h_new = (h + instance.hue_shift).rem_euclid(360.0);
    let s_new = (s + instance.saturation_shift).clamp(0.0, 1.0);
    let v_new = (v + instance.brightness_shift).clamp(0.0, 1.0);

    let (r, g, b) = hsv_to_rgb(h_new, s_new, v_new);
    [r, g, b, color[3]]
}

/// Convert RGB (each in 0..1) to HSV (H in 0..360, S and V in 0..1).
fn rgb_to_hsv(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let h = if delta < 1e-6 {
        0.0
    } else if (max - r).abs() < 1e-6 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 1e-6 {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };
    let s = if max < 1e-6 { 0.0 } else { delta / max };
    let v = max;

    (h, s, v)
}

/// Convert HSV (H in 0..360, S and V in 0..1) to RGB (each in 0..1).
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let c = v * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (r1 + m, g1 + m, b1 + m)
}

// ============================================================================
// CATEGORY PRESETS
// ============================================================================

impl VarietyParams {
    /// Preset for trees: large scale range, full Y rotation, moderate tilt,
    /// and significant color variation for an organic look.
    pub fn tree_preset() -> Self {
        Self {
            scale_min: 0.7,
            scale_max: 1.4,
            scale_y_bias: 0.3,
            random_y_rotation: true,
            tilt_max_degrees: 8.0,
            hue_shift_range: 20.0,
            saturation_range: 0.15,
            brightness_range: 0.2,
            noise_displacement: 0.08,
            noise_frequency: 1.5,
        }
    }

    /// Preset for grass: wide scale range, full Y rotation, more tilt
    /// (grass bends), and notable color variation.
    pub fn grass_preset() -> Self {
        Self {
            scale_min: 0.5,
            scale_max: 1.5,
            scale_y_bias: 0.5,
            random_y_rotation: true,
            tilt_max_degrees: 15.0,
            hue_shift_range: 25.0,
            saturation_range: 0.2,
            brightness_range: 0.25,
            noise_displacement: 0.03,
            noise_frequency: 3.0,
        }
    }

    /// Preset for rocks: moderate scale variation, full Y rotation, significant
    /// tilt, subtle color variation, and more noise displacement.
    pub fn rock_preset() -> Self {
        Self {
            scale_min: 0.6,
            scale_max: 1.6,
            scale_y_bias: 0.1,
            random_y_rotation: true,
            tilt_max_degrees: 20.0,
            hue_shift_range: 8.0,
            saturation_range: 0.05,
            brightness_range: 0.15,
            noise_displacement: 0.12,
            noise_frequency: 1.0,
        }
    }

    /// Preset for structures: minimal variation across all axes. Buildings
    /// and man-made objects should stay upright and consistent.
    pub fn structure_preset() -> Self {
        Self {
            scale_min: 0.95,
            scale_max: 1.05,
            scale_y_bias: 0.0,
            random_y_rotation: false,
            tilt_max_degrees: 0.0,
            hue_shift_range: 5.0,
            saturation_range: 0.05,
            brightness_range: 0.1,
            noise_displacement: 0.0,
            noise_frequency: 0.0,
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_rng_deterministic() {
        let mut rng1 = SimpleRng::new(42);
        let mut rng2 = SimpleRng::new(42);
        for _ in 0..100 {
            assert_eq!(rng1.next_u32(), rng2.next_u32());
        }
    }

    #[test]
    fn test_simple_rng_zero_seed_bumped() {
        let mut rng = SimpleRng::new(0);
        // Should not panic or loop forever
        let val = rng.next_u32();
        assert_ne!(val, 0);
    }

    #[test]
    fn test_simple_rng_range() {
        let mut rng = SimpleRng::new(123);
        for _ in 0..200 {
            let v = rng.range(2.0, 5.0);
            assert!(v >= 2.0 && v <= 5.0, "range value {v} out of bounds");
        }
    }

    #[test]
    fn test_seed_from_position_deterministic() {
        let s1 = seed_from_position(10.5, 0.0, -3.2);
        let s2 = seed_from_position(10.5, 0.0, -3.2);
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_seed_from_position_different_positions() {
        let s1 = seed_from_position(0.0, 0.0, 0.0);
        let s2 = seed_from_position(1.0, 0.0, 0.0);
        let s3 = seed_from_position(0.0, 0.0, 1.0);
        // Different positions should almost certainly produce different seeds
        assert_ne!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_seed_from_position_nonzero() {
        // Even (0,0,0) should produce a non-zero seed
        let s = seed_from_position(0.0, 0.0, 0.0);
        assert_ne!(s, 0);
    }

    #[test]
    fn test_generate_variety_deterministic() {
        let params = VarietyParams::tree_preset();
        let v1 = generate_variety(&params, 999);
        let v2 = generate_variety(&params, 999);
        assert_eq!(v1.scale, v2.scale);
        assert_eq!(v1.rotation_y, v2.rotation_y);
        assert_eq!(v1.tilt_angle, v2.tilt_angle);
        assert_eq!(v1.tilt_axis, v2.tilt_axis);
        assert_eq!(v1.hue_shift, v2.hue_shift);
        assert_eq!(v1.saturation_shift, v2.saturation_shift);
        assert_eq!(v1.brightness_shift, v2.brightness_shift);
        assert_eq!(v1.noise_seed, v2.noise_seed);
    }

    #[test]
    fn test_generate_variety_different_seeds_differ() {
        let params = VarietyParams::tree_preset();
        let v1 = generate_variety(&params, 100);
        let v2 = generate_variety(&params, 200);
        // At least one field should differ
        let differs = v1.scale != v2.scale
            || v1.rotation_y != v2.rotation_y
            || v1.tilt_angle != v2.tilt_angle
            || v1.hue_shift != v2.hue_shift;
        assert!(
            differs,
            "Different seeds should produce different instances"
        );
    }

    #[test]
    fn test_generate_variety_respects_ranges() {
        let params = VarietyParams::tree_preset();
        for seed in 1..200 {
            let v = generate_variety(&params, seed);
            // Scale X and Z should be within [scale_min, scale_max]
            assert!(
                v.scale.x >= params.scale_min && v.scale.x <= params.scale_max,
                "scale_x {} out of range",
                v.scale.x
            );
            assert!(
                v.scale.z >= params.scale_min && v.scale.z <= params.scale_max,
                "scale_z {} out of range",
                v.scale.z
            );
            // Scale Y includes bias
            let y_min = params.scale_min * (1.0 - params.scale_y_bias);
            let y_max = params.scale_max * (1.0 + params.scale_y_bias);
            assert!(
                v.scale.y >= y_min && v.scale.y <= y_max,
                "scale_y {} out of range [{}, {}]",
                v.scale.y,
                y_min,
                y_max
            );
            // Tilt angle should be within [0, tilt_max_degrees in radians]
            assert!(
                v.tilt_angle >= 0.0 && v.tilt_angle <= params.tilt_max_degrees.to_radians(),
                "tilt_angle {} out of range",
                v.tilt_angle
            );
            // Hue shift
            assert!(
                v.hue_shift >= -params.hue_shift_range && v.hue_shift <= params.hue_shift_range,
                "hue_shift {} out of range",
                v.hue_shift
            );
        }
    }

    #[test]
    fn test_generate_variety_no_rotation_when_disabled() {
        let params = VarietyParams::structure_preset();
        let v = generate_variety(&params, 42);
        assert_eq!(v.rotation_y, 0.0);
    }

    #[test]
    fn test_variety_to_transform_identity_like() {
        let instance = VarietyInstance {
            scale: Vec3::ONE,
            rotation_y: 0.0,
            tilt_angle: 0.0,
            tilt_axis: 0.0,
            hue_shift: 0.0,
            saturation_shift: 0.0,
            brightness_shift: 0.0,
            noise_seed: 0,
        };
        let mat = variety_to_transform(&instance, Vec3::ZERO);
        // Should be close to identity
        let diff = mat - Mat4::IDENTITY;
        for col in 0..4 {
            for row in 0..4 {
                assert!(
                    diff.col(col)[row].abs() < 1e-5,
                    "transform should be near identity"
                );
            }
        }
    }

    #[test]
    fn test_variety_to_transform_with_position() {
        let instance = VarietyInstance {
            scale: Vec3::ONE,
            rotation_y: 0.0,
            tilt_angle: 0.0,
            tilt_axis: 0.0,
            hue_shift: 0.0,
            saturation_shift: 0.0,
            brightness_shift: 0.0,
            noise_seed: 0,
        };
        let pos = Vec3::new(5.0, 10.0, -3.0);
        let mat = variety_to_transform(&instance, pos);
        // Translation column should contain the position
        let col3 = mat.col(3);
        assert!((col3[0] - 5.0).abs() < 1e-5);
        assert!((col3[1] - 10.0).abs() < 1e-5);
        assert!((col3[2] - (-3.0)).abs() < 1e-5);
    }

    #[test]
    fn test_variety_to_transform_with_scale() {
        let instance = VarietyInstance {
            scale: Vec3::new(2.0, 3.0, 2.0),
            rotation_y: 0.0,
            tilt_angle: 0.0,
            tilt_axis: 0.0,
            hue_shift: 0.0,
            saturation_shift: 0.0,
            brightness_shift: 0.0,
            noise_seed: 0,
        };
        let mat = variety_to_transform(&instance, Vec3::ZERO);
        // Diagonal should reflect scale
        assert!((mat.col(0)[0] - 2.0).abs() < 1e-5);
        assert!((mat.col(1)[1] - 3.0).abs() < 1e-5);
        assert!((mat.col(2)[2] - 2.0).abs() < 1e-5);
    }

    #[test]
    fn test_apply_color_variety_identity() {
        // Zero shifts should return the same color (modulo float precision)
        let instance = VarietyInstance {
            scale: Vec3::ONE,
            rotation_y: 0.0,
            tilt_angle: 0.0,
            tilt_axis: 0.0,
            hue_shift: 0.0,
            saturation_shift: 0.0,
            brightness_shift: 0.0,
            noise_seed: 0,
        };
        let color = [0.8, 0.2, 0.3, 1.0];
        let result = apply_color_variety(color, &instance);
        assert!((result[0] - color[0]).abs() < 0.01);
        assert!((result[1] - color[1]).abs() < 0.01);
        assert!((result[2] - color[2]).abs() < 0.01);
        assert_eq!(result[3], color[3]);
    }

    #[test]
    fn test_apply_color_variety_preserves_alpha() {
        let instance = VarietyInstance {
            scale: Vec3::ONE,
            rotation_y: 0.0,
            tilt_angle: 0.0,
            tilt_axis: 0.0,
            hue_shift: 30.0,
            saturation_shift: 0.1,
            brightness_shift: -0.05,
            noise_seed: 0,
        };
        let color = [0.5, 0.3, 0.7, 0.5];
        let result = apply_color_variety(color, &instance);
        assert_eq!(result[3], 0.5);
    }

    #[test]
    fn test_rgb_hsv_roundtrip() {
        let colors = [
            (1.0, 0.0, 0.0), // red
            (0.0, 1.0, 0.0), // green
            (0.0, 0.0, 1.0), // blue
            (0.5, 0.5, 0.5), // grey
            (0.8, 0.6, 0.2), // brownish
        ];
        for (r, g, b) in colors {
            let (h, s, v) = rgb_to_hsv(r, g, b);
            let (r2, g2, b2) = hsv_to_rgb(h, s, v);
            assert!(
                (r - r2).abs() < 0.01 && (g - g2).abs() < 0.01 && (b - b2).abs() < 0.01,
                "roundtrip failed for ({r}, {g}, {b}) -> ({h}, {s}, {v}) -> ({r2}, {g2}, {b2})"
            );
        }
    }

    #[test]
    fn test_default_variety_params() {
        let params = VarietyParams::default();
        assert_eq!(params.scale_min, 0.8);
        assert_eq!(params.scale_max, 1.2);
        assert_eq!(params.scale_y_bias, 0.0);
        assert!(params.random_y_rotation);
        assert_eq!(params.tilt_max_degrees, 5.0);
    }

    #[test]
    fn test_presets_exist() {
        let _tree = VarietyParams::tree_preset();
        let _grass = VarietyParams::grass_preset();
        let _rock = VarietyParams::rock_preset();
        let _structure = VarietyParams::structure_preset();
    }

    #[test]
    fn test_tree_preset_has_rotation() {
        let params = VarietyParams::tree_preset();
        assert!(params.random_y_rotation);
    }

    #[test]
    fn test_structure_preset_minimal_variation() {
        let params = VarietyParams::structure_preset();
        assert!(!params.random_y_rotation);
        assert_eq!(params.tilt_max_degrees, 0.0);
        assert_eq!(params.noise_displacement, 0.0);
    }
}
