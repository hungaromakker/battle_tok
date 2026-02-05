//! Adaptive Step Size Module
//!
//! US-032: Distance-Based Step Function
//!
//! This module provides functions for calculating ray marching step sizes
//! based on distance from the camera. Closer objects require smaller steps
//! for precision, while distant objects can use larger steps for performance.
//!
//! ## Distance Bands
//!
//! | Distance Range | Step Size Range | Interpolation |
//! |----------------|-----------------|---------------|
//! | < 5m           | 0.1 - 0.5       | Linear lerp   |
//! | 5m - 50m       | 0.5 - 2.0       | Linear lerp   |
//! | > 50m          | 2.0 - 5.0       | Linear lerp (clamped at 200m) |
//!
//! The step sizes are designed to provide:
//! - High precision (0.1-0.5) for nearby objects where detail matters
//! - Medium precision (0.5-2.0) for mid-range where most gameplay occurs
//! - Lower precision (2.0-5.0) for distant objects where fine detail isn't visible

/// Linearly interpolate between two values.
///
/// # Arguments
/// * `a` - Start value
/// * `b` - End value
/// * `t` - Interpolation factor (0.0 = a, 1.0 = b)
///
/// # Returns
/// Interpolated value between a and b
#[inline]
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Calculate the base step size for ray marching based on distance from camera.
///
/// This function provides smooth interpolation between distance bands to avoid
/// visual discontinuities (popping) as objects transition between bands.
///
/// # Arguments
/// * `distance` - Distance from camera in meters (SI units: 1 unit = 1 meter)
///
/// # Returns
/// Base step size in meters:
/// - 0.1-0.5 for distance < 5m (lerp)
/// - 0.5-2.0 for distance 5-50m (lerp)
/// - 2.0-5.0 for distance > 50m (lerp, clamped at 200m)
///
/// # Examples
/// ```
/// use battle_tok_engine::render::adaptive_step::base_step_for_distance;
///
/// // Very close: high precision
/// let step = base_step_for_distance(0.0);
/// assert!((step - 0.1).abs() < 0.001);
///
/// // At 5m boundary: 0.5 step
/// let step = base_step_for_distance(5.0);
/// assert!((step - 0.5).abs() < 0.001);
///
/// // At 50m boundary: 2.0 step
/// let step = base_step_for_distance(50.0);
/// assert!((step - 2.0).abs() < 0.001);
///
/// // Very far: clamped at 5.0
/// let step = base_step_for_distance(500.0);
/// assert!((step - 5.0).abs() < 0.001);
/// ```
pub fn base_step_for_distance(distance: f32) -> f32 {
    // Ensure distance is non-negative
    let distance = distance.max(0.0);

    if distance < 5.0 {
        // Close range: 0.1 at 0m, 0.5 at 5m
        let t = distance / 5.0;
        lerp(0.1, 0.5, t)
    } else if distance < 50.0 {
        // Mid range: 0.5 at 5m, 2.0 at 50m
        let t = (distance - 5.0) / 45.0;
        lerp(0.5, 2.0, t)
    } else {
        // Far range: 2.0 at 50m, 5.0 at 200m (clamped)
        // Using 150m range (50 to 200) for smooth transition
        let t = ((distance - 50.0) / 150.0).min(1.0);
        lerp(2.0, 5.0, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.0001;

    fn approx_eq(a: f32, b: f32) -> bool {
        (a - b).abs() < EPSILON
    }

    // =========================================================================
    // Boundary Value Tests
    // =========================================================================

    #[test]
    fn test_distance_zero() {
        // At distance 0, should return minimum step (0.1)
        let step = base_step_for_distance(0.0);
        assert!(approx_eq(step, 0.1), "At 0m: expected 0.1, got {}", step);
    }

    #[test]
    fn test_distance_5m_boundary() {
        // At 5m, should return 0.5 (boundary between close and mid range)
        let step = base_step_for_distance(5.0);
        assert!(approx_eq(step, 0.5), "At 5m: expected 0.5, got {}", step);
    }

    #[test]
    fn test_distance_50m_boundary() {
        // At 50m, should return 2.0 (boundary between mid and far range)
        let step = base_step_for_distance(50.0);
        assert!(approx_eq(step, 2.0), "At 50m: expected 2.0, got {}", step);
    }

    #[test]
    fn test_distance_200m_clamp() {
        // At 200m, should reach maximum step (5.0)
        let step = base_step_for_distance(200.0);
        assert!(approx_eq(step, 5.0), "At 200m: expected 5.0, got {}", step);
    }

    #[test]
    fn test_distance_clamped_above_200m() {
        // Beyond 200m, should stay clamped at 5.0
        let step = base_step_for_distance(500.0);
        assert!(
            approx_eq(step, 5.0),
            "At 500m: expected 5.0 (clamped), got {}",
            step
        );

        let step = base_step_for_distance(1000.0);
        assert!(
            approx_eq(step, 5.0),
            "At 1000m: expected 5.0 (clamped), got {}",
            step
        );
    }

    // =========================================================================
    // Interpolation Tests (verify smooth transitions)
    // =========================================================================

    #[test]
    fn test_close_range_midpoint() {
        // At 2.5m (midpoint of 0-5), should be midpoint of 0.1-0.5 = 0.3
        let step = base_step_for_distance(2.5);
        assert!(approx_eq(step, 0.3), "At 2.5m: expected 0.3, got {}", step);
    }

    #[test]
    fn test_mid_range_midpoint() {
        // At 27.5m (midpoint of 5-50), should be midpoint of 0.5-2.0 = 1.25
        let step = base_step_for_distance(27.5);
        assert!(
            approx_eq(step, 1.25),
            "At 27.5m: expected 1.25, got {}",
            step
        );
    }

    #[test]
    fn test_far_range_midpoint() {
        // At 125m (midpoint of 50-200), should be midpoint of 2.0-5.0 = 3.5
        let step = base_step_for_distance(125.0);
        assert!(approx_eq(step, 3.5), "At 125m: expected 3.5, got {}", step);
    }

    // =========================================================================
    // Continuity Tests (verify no discontinuities at boundaries)
    // =========================================================================

    #[test]
    fn test_continuity_at_5m() {
        // Values just before and after 5m should be nearly equal
        let before = base_step_for_distance(4.999);
        let after = base_step_for_distance(5.001);
        let at = base_step_for_distance(5.0);

        assert!(
            (before - at).abs() < 0.001,
            "Discontinuity at 5m: {} vs {}",
            before,
            at
        );
        assert!(
            (after - at).abs() < 0.001,
            "Discontinuity at 5m: {} vs {}",
            after,
            at
        );
    }

    #[test]
    fn test_continuity_at_50m() {
        // Values just before and after 50m should be nearly equal
        let before = base_step_for_distance(49.999);
        let after = base_step_for_distance(50.001);
        let at = base_step_for_distance(50.0);

        assert!(
            (before - at).abs() < 0.001,
            "Discontinuity at 50m: {} vs {}",
            before,
            at
        );
        assert!(
            (after - at).abs() < 0.001,
            "Discontinuity at 50m: {} vs {}",
            after,
            at
        );
    }

    // =========================================================================
    // Monotonicity Test (step size should always increase with distance)
    // =========================================================================

    #[test]
    fn test_monotonically_increasing() {
        let distances = [
            0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 10.0, 20.0, 30.0, 40.0, 50.0, 75.0, 100.0, 150.0, 200.0,
            300.0,
        ];

        for i in 1..distances.len() {
            let prev_step = base_step_for_distance(distances[i - 1]);
            let curr_step = base_step_for_distance(distances[i]);

            assert!(
                curr_step >= prev_step,
                "Step size decreased from {}m ({}) to {}m ({})",
                distances[i - 1],
                prev_step,
                distances[i],
                curr_step
            );
        }
    }

    // =========================================================================
    // Edge Cases
    // =========================================================================

    #[test]
    fn test_negative_distance_clamped() {
        // Negative distances should be treated as 0
        let step = base_step_for_distance(-1.0);
        assert!(
            approx_eq(step, 0.1),
            "Negative distance: expected 0.1, got {}",
            step
        );

        let step = base_step_for_distance(-100.0);
        assert!(
            approx_eq(step, 0.1),
            "Negative distance: expected 0.1, got {}",
            step
        );
    }

    #[test]
    fn test_very_small_distance() {
        // Very small positive distance should be near 0.1
        let step = base_step_for_distance(0.001);
        assert!(
            step > 0.1 && step < 0.11,
            "Very small distance: got {}",
            step
        );
    }

    #[test]
    fn test_very_large_distance() {
        // Very large distance should be clamped to 5.0
        let step = base_step_for_distance(10000.0);
        assert!(
            approx_eq(step, 5.0),
            "Very large distance: expected 5.0, got {}",
            step
        );
    }

    // =========================================================================
    // Value Range Tests
    // =========================================================================

    #[test]
    fn test_close_range_values() {
        // All values in close range should be between 0.1 and 0.5
        for d in [0.0, 0.5, 1.0, 2.0, 3.0, 4.0, 4.9] {
            let step = base_step_for_distance(d);
            assert!(
                step >= 0.1 && step <= 0.5,
                "Close range ({:.1}m): {} not in [0.1, 0.5]",
                d,
                step
            );
        }
    }

    #[test]
    fn test_mid_range_values() {
        // All values in mid range should be between 0.5 and 2.0
        for d in [5.0, 10.0, 20.0, 30.0, 40.0, 49.9] {
            let step = base_step_for_distance(d);
            assert!(
                step >= 0.5 && step <= 2.0,
                "Mid range ({:.1}m): {} not in [0.5, 2.0]",
                d,
                step
            );
        }
    }

    #[test]
    fn test_far_range_values() {
        // All values in far range should be between 2.0 and 5.0
        for d in [50.0, 75.0, 100.0, 150.0, 200.0, 500.0] {
            let step = base_step_for_distance(d);
            assert!(
                step >= 2.0 && step <= 5.0,
                "Far range ({:.1}m): {} not in [2.0, 5.0]",
                d,
                step
            );
        }
    }

    // =========================================================================
    // Lerp Helper Tests
    // =========================================================================

    #[test]
    fn test_lerp_at_boundaries() {
        assert!(approx_eq(lerp(0.0, 1.0, 0.0), 0.0));
        assert!(approx_eq(lerp(0.0, 1.0, 1.0), 1.0));
    }

    #[test]
    fn test_lerp_midpoint() {
        assert!(approx_eq(lerp(0.0, 1.0, 0.5), 0.5));
        assert!(approx_eq(lerp(2.0, 10.0, 0.5), 6.0));
    }

    #[test]
    fn test_lerp_arbitrary() {
        assert!(approx_eq(lerp(0.1, 0.5, 0.25), 0.2));
        assert!(approx_eq(lerp(0.5, 2.0, 0.5), 1.25));
    }
}
