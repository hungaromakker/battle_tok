//! Variety System
//!
//! Deterministic variety generation for asset instances. Produces unique visual
//! variations of the same base asset using seed-based pseudo-random numbers.
//! The same world position always maps to the same seed, ensuring consistent
//! appearance across sessions without storing per-instance data.
//!
//! This is a pure math/data module with no rendering or UI dependencies.

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

/// A concrete variety instance produced by `generate_variety`. Contains the
/// sampled values that should be applied to a single asset placement.
#[derive(Clone, Debug)]
pub struct VarietyInstance {
    /// Non-uniform scale (x, y, z). Y may differ due to `scale_y_bias`.
    pub scale: [f32; 3],
    /// Y-axis rotation in radians.
    pub y_rotation: f32,
    /// Tilt angles (x-axis, z-axis) in radians.
    pub tilt: [f32; 2],
    /// Hue shift in degrees to apply to the base color.
    pub hue_shift: f32,
    /// Saturation offset to apply to the base color.
    pub saturation_shift: f32,
    /// Brightness (value) offset to apply to the base color.
    pub brightness_shift: f32,
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
        Self {
            state: seed.max(1),
        }
    }

    /// Advance the state and return the next pseudo-random `u32`.
    pub fn next_u32(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        self.state
    }

    /// Return a pseudo-random `f32` in `[0.0, 1.0]`.
    pub fn next_f32(&mut self) -> f32 {
        (self.next_u32() as f32) / (u32::MAX as f32)
    }

    /// Return a pseudo-random `f32` in `[min, max]`.
    pub fn range(&mut self, min: f32, max: f32) -> f32 {
        min + self.next_f32() * (max - min)
    }
}

// ============================================================================
// SEED GENERATION
// ============================================================================

/// Derive a deterministic seed from a world position. The same (x, z)
/// coordinates always produce the same seed, ensuring assets placed at
/// the same location look identical across sessions.
pub fn seed_from_position(world_x: f32, world_z: f32) -> u32 {
    let x_bits = world_x.to_bits();
    let z_bits = world_z.to_bits();
    let hash = x_bits.wrapping_mul(73_856_093) ^ z_bits.wrapping_mul(19_349_663);
    // xorshift32 requires a non-zero seed
    hash.max(1)
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
    let base_scale = rng.range(params.scale_min, params.scale_max);
    let y_bias = rng.range(-params.scale_y_bias, params.scale_y_bias);

    // Rotation
    let y_rotation = if params.random_y_rotation {
        rng.range(0.0, std::f32::consts::TAU)
    } else {
        0.0
    };

    // Tilt (convert degrees to radians)
    let tilt_x = rng
        .range(-params.tilt_max_degrees, params.tilt_max_degrees)
        .to_radians();
    let tilt_z = rng
        .range(-params.tilt_max_degrees, params.tilt_max_degrees)
        .to_radians();

    // Color shifts
    let hue_shift = rng.range(-params.hue_shift_range, params.hue_shift_range);
    let saturation_shift = rng.range(-params.saturation_range, params.saturation_range);
    let brightness_shift = rng.range(-params.brightness_range, params.brightness_range);

    VarietyInstance {
        scale: [base_scale, base_scale + y_bias, base_scale],
        y_rotation,
        tilt: [tilt_x, tilt_z],
        hue_shift,
        saturation_shift,
        brightness_shift,
    }
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
            scale_max: 1.3,
            scale_y_bias: 0.15,
            random_y_rotation: true,
            tilt_max_degrees: 8.0,
            hue_shift_range: 15.0,
            saturation_range: 0.15,
            brightness_range: 0.12,
            noise_displacement: 0.03,
            noise_frequency: 1.5,
        }
    }

    /// Preset for grass: smaller scale range, full Y rotation, more tilt
    /// (grass bends), and subtle color variation.
    pub fn grass_preset() -> Self {
        Self {
            scale_min: 0.8,
            scale_max: 1.2,
            scale_y_bias: 0.2,
            random_y_rotation: true,
            tilt_max_degrees: 15.0,
            hue_shift_range: 10.0,
            saturation_range: 0.1,
            brightness_range: 0.08,
            noise_displacement: 0.01,
            noise_frequency: 2.0,
        }
    }

    /// Preset for rocks: moderate scale variation, no Y rotation, minimal
    /// tilt, and subtle color variation.
    pub fn rock_preset() -> Self {
        Self {
            scale_min: 0.8,
            scale_max: 1.3,
            scale_y_bias: 0.1,
            random_y_rotation: false,
            tilt_max_degrees: 3.0,
            hue_shift_range: 5.0,
            saturation_range: 0.05,
            brightness_range: 0.08,
            noise_displacement: 0.02,
            noise_frequency: 1.0,
        }
    }

    /// Preset for structures: minimal variation across all axes. Buildings
    /// and man-made objects should stay upright and consistent.
    pub fn structure_preset() -> Self {
        Self {
            scale_min: 0.98,
            scale_max: 1.02,
            scale_y_bias: 0.0,
            random_y_rotation: false,
            tilt_max_degrees: 0.0,
            hue_shift_range: 0.0,
            saturation_range: 0.0,
            brightness_range: 0.0,
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
        let s1 = seed_from_position(10.5, -3.2);
        let s2 = seed_from_position(10.5, -3.2);
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_seed_from_position_different_positions() {
        let s1 = seed_from_position(0.0, 0.0);
        let s2 = seed_from_position(1.0, 0.0);
        let s3 = seed_from_position(0.0, 1.0);
        // Different positions should almost certainly produce different seeds
        assert_ne!(s1, s2);
        assert_ne!(s1, s3);
    }

    #[test]
    fn test_seed_from_position_nonzero() {
        // Even (0,0) should produce a non-zero seed
        let s = seed_from_position(0.0, 0.0);
        assert_ne!(s, 0);
    }

    #[test]
    fn test_generate_variety_deterministic() {
        let params = VarietyParams::tree_preset();
        let v1 = generate_variety(&params, 999);
        let v2 = generate_variety(&params, 999);
        assert_eq!(v1.scale, v2.scale);
        assert_eq!(v1.y_rotation, v2.y_rotation);
        assert_eq!(v1.tilt, v2.tilt);
        assert_eq!(v1.hue_shift, v2.hue_shift);
        assert_eq!(v1.saturation_shift, v2.saturation_shift);
        assert_eq!(v1.brightness_shift, v2.brightness_shift);
    }

    #[test]
    fn test_generate_variety_respects_ranges() {
        let params = VarietyParams::tree_preset();
        for seed in 1..200 {
            let v = generate_variety(&params, seed);
            // Scale X and Z should be within [scale_min, scale_max]
            assert!(
                v.scale[0] >= params.scale_min && v.scale[0] <= params.scale_max,
                "scale_x {} out of range",
                v.scale[0]
            );
            assert!(
                v.scale[2] >= params.scale_min && v.scale[2] <= params.scale_max,
                "scale_z {} out of range",
                v.scale[2]
            );
            // Scale Y includes bias
            let y_min = params.scale_min - params.scale_y_bias;
            let y_max = params.scale_max + params.scale_y_bias;
            assert!(
                v.scale[1] >= y_min && v.scale[1] <= y_max,
                "scale_y {} out of range [{}, {}]",
                v.scale[1],
                y_min,
                y_max
            );
        }
    }

    #[test]
    fn test_generate_variety_no_rotation_when_disabled() {
        let params = VarietyParams::structure_preset();
        let v = generate_variety(&params, 42);
        assert_eq!(v.y_rotation, 0.0);
    }

    #[test]
    fn test_apply_color_variety_identity() {
        // Zero shifts should return the same color (modulo float precision)
        let instance = VarietyInstance {
            scale: [1.0, 1.0, 1.0],
            y_rotation: 0.0,
            tilt: [0.0, 0.0],
            hue_shift: 0.0,
            saturation_shift: 0.0,
            brightness_shift: 0.0,
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
            scale: [1.0, 1.0, 1.0],
            y_rotation: 0.0,
            tilt: [0.0, 0.0],
            hue_shift: 30.0,
            saturation_shift: 0.1,
            brightness_shift: -0.05,
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
        assert_eq!(params.hue_shift_range, 0.0);
        assert_eq!(params.saturation_range, 0.0);
        assert_eq!(params.brightness_range, 0.0);
    }
}
