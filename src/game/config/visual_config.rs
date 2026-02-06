//! Visual Configuration
//!
//! Centralizes all visual atmosphere settings (fog, lighting, torches, lava glow)
//! used in the battle arena. Provides a single-source-of-truth for the apocalyptic
//! visual style so artists can tweak the feel without touching game code.

use glam::Vec3;

/// Debug toggles for runtime post-processing control.
#[derive(Clone, Debug)]
pub struct PostFxDebugToggles {
    pub postfx_enabled: bool,
    pub taa_enabled: bool,
    pub bloom_enabled: bool,
}

impl Default for PostFxDebugToggles {
    fn default() -> Self {
        Self {
            postfx_enabled: true,
            taa_enabled: true,
            bloom_enabled: true,
        }
    }
}

/// Haze settings used by the fog post pass.
#[derive(Clone, Debug)]
pub struct HazeConfig {
    pub enabled: bool,
    pub density: f32,
    pub height_fog_start: f32,
    pub height_fog_density: f32,
    pub max_opacity: f32,
    pub horizon_boost: f32,
}

impl Default for HazeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            density: 0.0012,
            height_fog_start: -1.0,
            height_fog_density: 0.035,
            max_opacity: 0.22,
            horizon_boost: 0.35,
        }
    }
}

/// Temporal anti-aliasing settings.
#[derive(Clone, Debug)]
pub struct TaaConfig {
    pub enabled: bool,
    pub history_weight: f32,
    pub new_weight: f32,
    pub depth_reject_threshold: f32,
}

impl Default for TaaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            history_weight: 0.90,
            new_weight: 0.10,
            depth_reject_threshold: 0.003,
        }
    }
}

/// Bloom settings.
#[derive(Clone, Debug)]
pub struct BloomConfig {
    pub enabled: bool,
    pub threshold: f32,
    pub knee: f32,
    pub intensity: f32,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            threshold: 1.0,
            knee: 0.4,
            intensity: 0.18,
        }
    }
}

/// Tonemap settings used in the final composite pass.
#[derive(Clone, Debug)]
pub struct TonemapConfig {
    pub exposure: f32,
    pub saturation: f32,
    pub contrast: f32,
}

impl Default for TonemapConfig {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            saturation: 1.12,
            contrast: 1.08,
        }
    }
}

/// Aggregate post-processing settings for battle_arena.
#[derive(Clone, Debug)]
pub struct PostFxConfig {
    pub lock_midday: bool,
    pub haze: HazeConfig,
    pub taa: TaaConfig,
    pub bloom: BloomConfig,
    pub tonemap: TonemapConfig,
    pub debug_toggles: PostFxDebugToggles,
}

impl Default for PostFxConfig {
    fn default() -> Self {
        Self {
            lock_midday: true,
            haze: HazeConfig::default(),
            taa: TaaConfig::default(),
            bloom: BloomConfig::default(),
            tonemap: TonemapConfig::default(),
            debug_toggles: PostFxDebugToggles::default(),
        }
    }
}

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

    // Post-processing
    pub postfx: PostFxConfig,
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

            // PostFx
            postfx: PostFxConfig::default(),
        }
    }
}

impl VisualConfig {
    /// Preset matching the current battle arena apocalyptic atmosphere.
    pub fn battle_arena() -> Self {
        Self::default()
    }
}
