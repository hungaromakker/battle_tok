//! Sky/Weather Module
//!
//! This module provides procedural sky rendering with day/night cycle,
//! weather systems, seasons, and atmosphere effects.
//!
//! Adapted from bevy_sky_gradient (TanTanDev) for pure wgpu.
//!
//! # Features
//! - Procedural day/night cycle with smooth transitions
//! - Sun with configurable color, strength, and sharpness
//! - Moon with accurate lunar phases (29.5 day cycle)
//! - Stars with twinkling and slow rotation
//! - Aurora borealis with animated curtains
//! - Volumetric Perlin noise clouds
//! - Weather system (clear, cloudy, rain, storm)
//! - Season effects (spring, summer, fall, winter)
//! - Temperature and humidity atmosphere effects

/// Weather type enum for the sky system
///
/// Controls cloud coverage, rain intensity, and overall atmosphere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum WeatherType {
    /// Clear sky with no clouds
    Clear = 0,
    /// Partly cloudy with light cloud coverage (~30%)
    #[default]
    PartlyCloudy = 1,
    /// Cloudy with moderate cloud coverage (~60%)
    Cloudy = 2,
    /// Overcast with heavy cloud coverage (~90%)
    Overcast = 3,
    /// Rain with clouds and precipitation
    Rain = 4,
    /// Storm with heavy clouds, rain, and lightning
    Storm = 5,
}

impl WeatherType {
    /// Returns the default cloud coverage for this weather type
    pub fn default_cloud_coverage(&self) -> f32 {
        match self {
            WeatherType::Clear => 0.0,
            WeatherType::PartlyCloudy => 0.3,
            WeatherType::Cloudy => 0.6,
            WeatherType::Overcast => 0.9,
            WeatherType::Rain => 0.85,
            WeatherType::Storm => 0.95,
        }
    }

    /// Returns the default rain intensity for this weather type
    pub fn default_rain_intensity(&self) -> f32 {
        match self {
            WeatherType::Clear | WeatherType::PartlyCloudy | WeatherType::Cloudy | WeatherType::Overcast => 0.0,
            WeatherType::Rain => 0.6,
            WeatherType::Storm => 1.0,
        }
    }

    /// Returns the default lightning intensity for this weather type
    pub fn default_lightning_intensity(&self) -> f32 {
        match self {
            WeatherType::Storm => 0.8,
            _ => 0.0,
        }
    }

    /// Cycles to the next weather type
    pub fn next(&self) -> Self {
        match self {
            WeatherType::Clear => WeatherType::PartlyCloudy,
            WeatherType::PartlyCloudy => WeatherType::Cloudy,
            WeatherType::Cloudy => WeatherType::Overcast,
            WeatherType::Overcast => WeatherType::Rain,
            WeatherType::Rain => WeatherType::Storm,
            WeatherType::Storm => WeatherType::Clear,
        }
    }
}

impl From<u32> for WeatherType {
    fn from(value: u32) -> Self {
        match value {
            0 => WeatherType::Clear,
            1 => WeatherType::PartlyCloudy,
            2 => WeatherType::Cloudy,
            3 => WeatherType::Overcast,
            4 => WeatherType::Rain,
            5 => WeatherType::Storm,
            _ => WeatherType::PartlyCloudy,
        }
    }
}

/// Season enum for the sky system
///
/// Affects sky colors, temperature defaults, and atmosphere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u32)]
pub enum Season {
    /// Spring - mild temperatures, moderate humidity
    Spring = 0,
    /// Summer - warm temperatures, lower humidity
    #[default]
    Summer = 1,
    /// Fall - cool temperatures, higher humidity
    Fall = 2,
    /// Winter - cold temperatures, moderate humidity
    Winter = 3,
}

impl Season {
    /// Returns the default temperature for this season
    /// Range: -1.0 (freezing) to 1.0 (hot)
    pub fn default_temperature(&self) -> f32 {
        match self {
            Season::Spring => 0.0,
            Season::Summer => 0.7,
            Season::Fall => -0.2,
            Season::Winter => -0.8,
        }
    }

    /// Returns the default humidity for this season
    /// Range: 0.0 (dry) to 1.0 (humid)
    pub fn default_humidity(&self) -> f32 {
        match self {
            Season::Spring => 0.5,
            Season::Summer => 0.3,
            Season::Fall => 0.6,
            Season::Winter => 0.4,
        }
    }

    /// Cycles to the next season
    pub fn next(&self) -> Self {
        match self {
            Season::Spring => Season::Summer,
            Season::Summer => Season::Fall,
            Season::Fall => Season::Winter,
            Season::Winter => Season::Spring,
        }
    }
}

impl From<u32> for Season {
    fn from(value: u32) -> Self {
        match value {
            0 => Season::Spring,
            1 => Season::Summer,
            2 => Season::Fall,
            3 => Season::Winter,
            _ => Season::Summer,
        }
    }
}

/// Moon phase enum for display purposes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoonPhase {
    NewMoon,
    WaxingCrescent,
    FirstQuarter,
    WaxingGibbous,
    FullMoon,
    WaningGibbous,
    LastQuarter,
    WaningCrescent,
}

impl MoonPhase {
    /// Get the moon phase from the current phase value (0.0 - 1.0)
    pub fn from_phase(phase: f32) -> Self {
        let index = ((phase * 8.0) as u32) % 8;
        match index {
            0 => MoonPhase::NewMoon,
            1 => MoonPhase::WaxingCrescent,
            2 => MoonPhase::FirstQuarter,
            3 => MoonPhase::WaxingGibbous,
            4 => MoonPhase::FullMoon,
            5 => MoonPhase::WaningGibbous,
            6 => MoonPhase::LastQuarter,
            7 => MoonPhase::WaningCrescent,
            _ => MoonPhase::NewMoon,
        }
    }

    /// Get the name of this moon phase
    pub fn name(&self) -> &'static str {
        match self {
            MoonPhase::NewMoon => "New Moon",
            MoonPhase::WaxingCrescent => "Waxing Crescent",
            MoonPhase::FirstQuarter => "First Quarter",
            MoonPhase::WaxingGibbous => "Waxing Gibbous",
            MoonPhase::FullMoon => "Full Moon",
            MoonPhase::WaningGibbous => "Waning Gibbous",
            MoonPhase::LastQuarter => "Last Quarter",
            MoonPhase::WaningCrescent => "Waning Crescent",
        }
    }
}

/// Sky settings uniform buffer struct
///
/// This struct must match the WGSL `SkySettings` struct layout exactly.
/// It contains all parameters for the procedural sky rendering system.
///
/// # Memory Layout
/// This struct uses `#[repr(C)]` to ensure consistent memory layout
/// with the WGSL shader. Padding fields (`_padN`) are required for
/// proper GPU buffer alignment.
///
/// # Time of Day
/// The `time_of_day` field uses the following convention:
/// - 0.0 = Sunrise
/// - 0.25 = Noon
/// - 0.5 = Sunset
/// - 0.75 = Midnight
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkySettings {
    // ========================================
    // Time settings (16 bytes)
    // ========================================
    /// Time of day (0.0 = sunrise, 0.25 = noon, 0.5 = sunset, 0.75 = midnight)
    pub time_of_day: f32,
    /// Speed of day/night cycle (0 = paused)
    pub cycle_speed: f32,
    /// Elapsed time for animations
    pub elapsed_time: f32,
    /// Padding for 16-byte alignment
    pub _pad0: f32,

    // ========================================
    // Sun settings (48 bytes)
    // ========================================
    /// Sun direction (calculated from time_of_day in shader)
    pub sun_dir: [f32; 3],
    /// Sun disc edge sharpness
    pub sun_sharpness: f32,
    /// Sun color (RGBA)
    pub sun_color: [f32; 4],
    /// Sun brightness multiplier
    pub sun_strength: f32,
    /// Sun visibility toggle (0 = off, 1 = on)
    pub sun_enabled: u32,
    /// Padding for alignment
    pub _pad1: f32,
    /// Padding for alignment
    pub _pad2: f32,

    // ========================================
    // Gradient colors (96 bytes)
    // Day/night palette for sky gradient
    // ========================================
    /// Day sky color at horizon (RGBA)
    pub day_horizon: [f32; 4],
    /// Day sky color at zenith (RGBA)
    pub day_zenith: [f32; 4],
    /// Sunset sky color at horizon (RGBA)
    pub sunset_horizon: [f32; 4],
    /// Sunset sky color at zenith (RGBA)
    pub sunset_zenith: [f32; 4],
    /// Night sky color at horizon (RGBA)
    pub night_horizon: [f32; 4],
    /// Night sky color at zenith (RGBA)
    pub night_zenith: [f32; 4],

    // ========================================
    // Stars settings (16 bytes)
    // ========================================
    /// Stars visibility toggle (0 = off, 1 = on)
    pub stars_enabled: u32,
    /// Threshold for star visibility (higher = fewer stars)
    pub stars_threshold: f32,
    /// Speed of star twinkling
    pub stars_blink_speed: f32,
    /// Density of stars (higher = more stars)
    pub stars_density: f32,

    // ========================================
    // Aurora settings (48 bytes)
    // ========================================
    /// Aurora visibility toggle (0 = off, 1 = on)
    pub aurora_enabled: u32,
    /// Aurora brightness multiplier
    pub aurora_intensity: f32,
    /// Aurora animation speed
    pub aurora_speed: f32,
    /// Aurora height in sky
    pub aurora_height: f32,
    /// Aurora color at bottom of curtain (RGBA)
    pub aurora_color_bottom: [f32; 4],
    /// Aurora color at top of curtain (RGBA)
    pub aurora_color_top: [f32; 4],

    // ========================================
    // Weather system (32 bytes)
    // ========================================
    /// Weather type (0=clear, 1=partly_cloudy, 2=cloudy, 3=overcast, 4=rain, 5=storm)
    pub weather_type: u32,
    /// Cloud coverage (0.0 = no clouds, 1.0 = full coverage)
    pub cloud_coverage: f32,
    /// Cloud density/opacity (0.0 = transparent, 1.0 = opaque)
    pub cloud_density: f32,
    /// Cloud movement speed
    pub cloud_speed: f32,

    // ========================================
    // Cloud appearance (16 bytes)
    // ========================================
    /// Base height of cloud layer
    pub cloud_height: f32,
    /// Vertical extent of clouds
    pub cloud_thickness: f32,
    /// Size/frequency of cloud formations
    pub cloud_scale: f32,
    /// Edge definition (0 = soft, 1 = hard)
    pub cloud_sharpness: f32,

    // ========================================
    // Season settings (16 bytes)
    // ========================================
    /// Current season (0=spring, 1=summer, 2=fall, 3=winter)
    pub season: u32,
    /// How strongly season affects sky colors
    pub season_intensity: f32,
    /// Padding for alignment
    pub _pad3: f32,
    /// Padding for alignment
    pub _pad4: f32,

    // ========================================
    // Temperature effects (16 bytes)
    // ========================================
    /// Temperature (-1.0 = freezing, 0.0 = mild, 1.0 = hot)
    pub temperature: f32,
    /// Humidity (0.0 = dry, 1.0 = humid) - affects haze
    pub humidity: f32,
    /// Wind speed - affects cloud animation
    pub wind_speed: f32,
    /// Wind direction in radians
    pub wind_direction: f32,

    // ========================================
    // Rain/precipitation (16 bytes)
    // ========================================
    /// Rain intensity (0.0 = none, 1.0 = heavy)
    pub rain_intensity: f32,
    /// Visibility reduction from rain
    pub rain_visibility: f32,
    /// Lightning flash intensity (0.0 = none, 1.0 = frequent)
    pub lightning_intensity: f32,
    /// Horizon haze toggle (0 = off, 1 = on) - press K to toggle
    pub haze_enabled: u32,

    // ========================================
    // Fog settings (16 bytes)
    // ========================================
    /// Fog density (0.005 = good visibility, 0.02 = dense fog)
    /// At 0.005 with fog_start=50, fog is ~50% at 200 units
    pub fog_density: f32,
    /// Distance at which fog starts (units/meters)
    pub fog_start_distance: f32,
    /// Fog visibility toggle (0 = off, 1 = on) - press L to toggle
    pub fog_enabled: u32,
    /// Padding for alignment
    pub _pad_fog1: f32,

    // ========================================
    // Moon system (48 bytes)
    // ========================================
    /// Moon visibility toggle (0 = off, 1 = on)
    pub moon_enabled: u32,
    /// Moon phase (0.0 = new moon, 0.5 = full moon, 1.0 = new moon again)
    pub moon_phase: f32,
    /// Current day in lunar cycle (0-29.5)
    pub lunar_day: f32,
    /// Moon disc edge sharpness
    pub moon_sharpness: f32,
    /// Moon color (RGBA)
    pub moon_color: [f32; 4],
    /// Moon brightness (varies with phase)
    pub moon_strength: f32,
    /// Moon apparent size
    pub moon_size: f32,
    /// Padding for alignment
    pub _pad6: f32,
    /// Padding for alignment
    pub _pad7: f32,
}

impl Default for SkySettings {
    fn default() -> Self {
        Self {
            // Time - start at noon
            time_of_day: 0.25,
            cycle_speed: 0.01,
            elapsed_time: 0.0,
            _pad0: 0.0,

            // Sun
            sun_dir: [0.0, 1.0, 0.0],
            sun_sharpness: 256.0,
            sun_color: [1.0, 0.95, 0.8, 1.0],
            sun_strength: 1.5,
            sun_enabled: 1,
            _pad1: 0.0,
            _pad2: 0.0,

            // Day colors (bright blue sky)
            day_horizon: [0.6, 0.75, 0.9, 1.0],
            day_zenith: [0.3, 0.5, 0.85, 1.0],

            // Sunset colors (warm oranges and purples)
            sunset_horizon: [0.9, 0.5, 0.3, 1.0],
            sunset_zenith: [0.5, 0.3, 0.6, 1.0],

            // Night colors (deep blue/black)
            night_horizon: [0.05, 0.08, 0.15, 1.0],
            night_zenith: [0.02, 0.03, 0.08, 1.0],

            // Stars
            stars_enabled: 1,
            stars_threshold: 0.85,
            stars_blink_speed: 2.0,
            stars_density: 30.0,

            // Aurora
            aurora_enabled: 1,
            aurora_intensity: 0.8,
            aurora_speed: 0.1,
            aurora_height: 5.0,
            aurora_color_bottom: [0.1, 0.8, 0.4, 1.0], // Green
            aurora_color_top: [0.3, 0.4, 0.9, 1.0],    // Blue-purple

            // Weather - default to partly cloudy
            weather_type: WeatherType::PartlyCloudy as u32,
            cloud_coverage: 0.3,
            cloud_density: 0.6,
            cloud_speed: 0.02,

            // Cloud appearance
            cloud_height: 3.0,
            cloud_thickness: 0.5,
            cloud_scale: 1.0,
            cloud_sharpness: 0.5,

            // Season - default summer
            season: Season::Summer as u32,
            season_intensity: 0.5,
            _pad3: 0.0,
            _pad4: 0.0,

            // Temperature - mild/warm
            temperature: 0.2,
            humidity: 0.3,
            wind_speed: 0.5,
            wind_direction: 0.0,

            // No rain by default
            rain_intensity: 0.0,
            rain_visibility: 1.0,
            lightning_intensity: 0.0,
            haze_enabled: 1, // Haze ON by default, press K to toggle

            // Fog settings - very good visibility for large world exploration
            // At 0.001 with fog_start=500, fog is subtle even at long distances
            // This allows seeing the horizon clearly on the 10km world
            fog_density: 0.001,
            fog_start_distance: 500.0,
            fog_enabled: 1, // Fog ON by default, press L to toggle
            _pad_fog1: 0.0,

            // Moon system - starts at waxing crescent
            moon_enabled: 1,
            moon_phase: 0.1,
            lunar_day: 3.0,
            moon_sharpness: 128.0,
            moon_color: [0.9, 0.92, 0.95, 1.0],
            moon_strength: 0.4,
            moon_size: 0.05,
            _pad6: 0.0,
            _pad7: 0.0,
        }
    }
}

impl SkySettings {
    /// Create sky settings for a specific time of day
    pub fn with_time(time_of_day: f32) -> Self {
        let mut settings = Self::default();
        settings.time_of_day = time_of_day.rem_euclid(1.0);
        settings
    }

    /// Create sky settings for noon
    pub fn noon() -> Self {
        Self::with_time(0.25)
    }

    /// Create sky settings for midnight
    pub fn midnight() -> Self {
        Self::with_time(0.75)
    }

    /// Create sky settings for sunrise
    pub fn sunrise() -> Self {
        Self::with_time(0.0)
    }

    /// Create sky settings for sunset
    pub fn sunset() -> Self {
        Self::with_time(0.5)
    }

    /// Set the weather type and update related settings
    pub fn set_weather(&mut self, weather: WeatherType) {
        self.weather_type = weather as u32;
        self.cloud_coverage = weather.default_cloud_coverage();
        self.rain_intensity = weather.default_rain_intensity();
        self.lightning_intensity = weather.default_lightning_intensity();
    }

    /// Set the season and update related settings
    pub fn set_season(&mut self, season: Season) {
        self.season = season as u32;
        self.temperature = season.default_temperature();
        self.humidity = season.default_humidity();
    }

    /// Get the current weather type
    pub fn get_weather(&self) -> WeatherType {
        WeatherType::from(self.weather_type)
    }

    /// Get the current season
    pub fn get_season(&self) -> Season {
        Season::from(self.season)
    }

    /// Get the current moon phase
    pub fn get_moon_phase(&self) -> MoonPhase {
        MoonPhase::from_phase(self.moon_phase)
    }

    /// Advance time by delta (typically frame time * cycle_speed)
    pub fn advance_time(&mut self, delta: f32) {
        let old_time = self.time_of_day;
        self.time_of_day = (self.time_of_day + delta).rem_euclid(1.0);

        // Check if a full day has passed (for lunar cycle)
        if self.time_of_day < old_time {
            self.advance_lunar_day(1.0);
        }
    }

    /// Advance the lunar cycle by a number of days
    pub fn advance_lunar_day(&mut self, days: f32) {
        self.lunar_day = (self.lunar_day + days).rem_euclid(29.5);
        self.moon_phase = self.lunar_day / 29.5;

        // Update moon brightness based on phase (full moon is brightest)
        let illumination = (self.moon_phase * std::f32::consts::TAU).cos();
        self.moon_strength = 0.3 + illumination.abs() * 0.4;
    }

    /// Set to full moon
    pub fn set_full_moon(&mut self) {
        self.lunar_day = 14.75;
        self.moon_phase = 0.5;
        self.moon_strength = 0.7;
    }

    /// Set to new moon
    pub fn set_new_moon(&mut self) {
        self.lunar_day = 0.0;
        self.moon_phase = 0.0;
        self.moon_strength = 0.3;
    }

    /// Get a human-readable time description
    pub fn get_time_name(&self) -> &'static str {
        match (self.time_of_day * 4.0) as u32 {
            0 => "Sunrise",
            1 => "Day",
            2 => "Sunset",
            _ => "Night",
        }
    }

    /// Check if it's currently night time
    pub fn is_night(&self) -> bool {
        self.time_of_day >= 0.55 || self.time_of_day < 0.1
    }

    /// Get night visibility factor (0.0 = day, 1.0 = full night)
    pub fn get_night_visibility(&self) -> f32 {
        if self.time_of_day >= 0.55 && self.time_of_day < 0.65 {
            (self.time_of_day - 0.55) / 0.1
        } else if self.time_of_day >= 0.65 || self.time_of_day < 0.05 {
            1.0
        } else if self.time_of_day >= 0.05 && self.time_of_day < 0.1 {
            1.0 - (self.time_of_day - 0.05) / 0.05
        } else {
            0.0
        }
    }

    /// Size of this struct in bytes (for GPU buffer allocation)
    pub const SIZE: usize = std::mem::size_of::<Self>();
}

// Compile-time assertions to verify struct size matches expected WGSL layout
const _: () = {
    // Each section contributes:
    // Time: 16 bytes
    // Sun: 48 bytes
    // Gradients: 96 bytes
    // Stars: 16 bytes
    // Aurora: 48 bytes
    // Weather: 32 bytes (16 + 16)
    // Season: 16 bytes
    // Temperature: 16 bytes
    // Rain: 16 bytes
    // Fog: 16 bytes
    // Moon: 48 bytes
    // Total: 368 bytes
    assert!(
        std::mem::size_of::<SkySettings>() == 368,
        "SkySettings must be 368 bytes to match WGSL layout"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sky_settings_size() {
        assert_eq!(std::mem::size_of::<SkySettings>(), 368);
    }

    #[test]
    fn test_weather_type_conversion() {
        assert_eq!(WeatherType::from(0), WeatherType::Clear);
        assert_eq!(WeatherType::from(1), WeatherType::PartlyCloudy);
        assert_eq!(WeatherType::from(5), WeatherType::Storm);
        assert_eq!(WeatherType::from(99), WeatherType::PartlyCloudy);
    }

    #[test]
    fn test_season_conversion() {
        assert_eq!(Season::from(0), Season::Spring);
        assert_eq!(Season::from(1), Season::Summer);
        assert_eq!(Season::from(3), Season::Winter);
        assert_eq!(Season::from(99), Season::Summer);
    }

    #[test]
    fn test_moon_phase() {
        assert_eq!(MoonPhase::from_phase(0.0), MoonPhase::NewMoon);
        assert_eq!(MoonPhase::from_phase(0.5), MoonPhase::FullMoon);
        assert_eq!(MoonPhase::from_phase(0.25), MoonPhase::FirstQuarter);
    }

    #[test]
    fn test_time_advancement() {
        let mut settings = SkySettings::default();
        settings.time_of_day = 0.9;
        settings.lunar_day = 0.0;

        settings.advance_time(0.2); // Should wrap around

        assert!(settings.time_of_day < 0.2);
        assert!(settings.lunar_day > 0.0); // Lunar day should have advanced
    }

    #[test]
    fn test_night_visibility() {
        let noon = SkySettings::noon();
        assert_eq!(noon.get_night_visibility(), 0.0);

        let midnight = SkySettings::midnight();
        assert_eq!(midnight.get_night_visibility(), 1.0);
    }
}
