//! Visual Configuration
//!
//! Centralizes all visual atmosphere settings (fog, lighting, torches, lava glow)
//! used in the battle arena. Provides a single-source-of-truth for the apocalyptic
//! visual style so artists can tweak the feel without touching game code.

use glam::Vec3;

/// Visual atmosphere configuration for the battle arena.
///
/// Captures fog, directional lighting, torch, and lava glow parameters
/// that define the apocalyptic visual style. Matches the hardcoded values
/// currently scattered across `battle_arena.rs` and engine subsystems.
#[derive(Clone, Debug)]
pub struct VisualConfig {
    // Fog
    /// Exponential fog density (higher = thicker fog)
    pub fog_density: f32,
    /// Fog color (RGB, linear space)
    pub fog_color: Vec3,

    // Directional light (sun)
    /// Sun direction vector (normalized)
    pub sun_direction: Vec3,
    /// Sun color (RGB, HDR values allowed)
    pub sun_color: Vec3,
    /// Ambient light intensity (0.0 = pitch black, 1.0 = full)
    pub ambient_intensity: f32,

    // Torches
    /// Base torch light intensity before flicker
    pub torch_intensity: f32,
    /// Flicker animation speed (radians per second)
    pub torch_flicker_speed: f32,
    /// Torch light influence radius in world units
    pub torch_radius: f32,
    /// Torch light color (RGB)
    pub torch_color: Vec3,

    // Lava glow (affects fog and sky)
    /// Lava emissive color (HDR orange-red)
    pub lava_glow_color: Vec3,
    /// Lava glow strength multiplier
    pub lava_glow_strength: f32,
}

impl Default for VisualConfig {
    /// Returns the default apocalyptic battle arena visual settings.
    ///
    /// These values match the current hardcoded constants in `battle_arena.rs`:
    /// - Fog: density 0.004, warm purple-brown color
    /// - Sun: low horizon orange-red for rim lighting
    /// - Ambient: 0.25 for rich contrast
    /// - Torches: warm orange, radius 10, flicker at 12 rad/s
    /// - Lava: HDR orange-red glow
    fn default() -> Self {
        Self {
            // Fog — minimal, let colors breathe
            fog_density: 0.002,
            fog_color: Vec3::new(0.20, 0.15, 0.12),

            // Directional light — bright warm sun for vibrant terrain
            sun_direction: Vec3::new(0.4, 0.6, -0.7),
            sun_color: Vec3::new(1.6, 1.2, 0.8),
            ambient_intensity: 0.45,

            // Torches
            torch_intensity: 1.0,
            torch_flicker_speed: 12.0,
            torch_radius: 10.0,
            torch_color: Vec3::new(1.0, 0.6, 0.2),

            // Lava glow
            lava_glow_color: Vec3::new(1.5, 0.4, 0.1),
            lava_glow_strength: 1.0,
        }
    }
}

impl VisualConfig {
    /// Preset matching the current battle arena apocalyptic atmosphere.
    pub fn battle_arena() -> Self {
        Self::default()
    }
}
