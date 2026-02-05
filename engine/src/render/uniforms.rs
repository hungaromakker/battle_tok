//! Uniform Structs for GPU Shaders
//!
//! Contains GPU-compatible uniform buffer structures that must match WGSL layout exactly.
//! Extracted from sdf_core_test.rs for reuse across the engine.

/// Minimal uniforms for the test shader.
/// Must match the WGSL struct layout exactly!
///
/// WGSL std140 layout (128 bytes total - using scalar padding to avoid vec3 alignment issues):
///   offset  0: camera_pos (vec3<f32>)    = 12 bytes
///   offset 12: time (f32)                = 4 bytes
///   offset 16: resolution (vec2<f32>)    = 8 bytes
///   offset 24: debug_mode (u32)          = 4 bytes
///   offset 28: human_visible (u32)       = 4 bytes
///   offset 32: camera_target (vec3<f32>) = 12 bytes (vec3 aligned to 16)
///   offset 44: grid_size (f32)           = 4 bytes
///   offset 48: volume_grid_visible (u32) = 4 bytes
///   offset 52: placement_height (f32)    = 4 bytes
///   offset 56: show_hud (u32)            = 4 bytes
///   offset 60: camera_pitch (f32)        = 4 bytes
///   offset 64: camera_mode (u32)         = 4 bytes
///   offset 68: show_perf_overlay (u32)   = 4 bytes
///   offset 72: perf_fps (f32)            = 4 bytes
///   offset 76: perf_frame_time_ms (f32)  = 4 bytes
///   offset 80: perf_entity_count (u32)   = 4 bytes
///   offset 84: perf_baked_sdf_count (u32)= 4 bytes
///   offset 88: perf_tile_buffer_kb (f32) = 4 bytes
///   offset 92: perf_gpu_memory_mb (f32)  = 4 bytes
///   offset 96: perf_active_tile_count (u32) = 4 bytes
///   offset 100: use_sky_cubemap (u32)     = 4 bytes
///   offset 104: _pad1 (u32)              = 4 bytes
///   offset 108: _pad2 (u32)              = 4 bytes (aligns player_position to 16)
///   offset 112: player_position (vec3<f32>) = 12 bytes (vec3 aligned to 16)
///   offset 124: _pad3 (u32)              = 4 bytes
///   Total: 128 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TestUniforms {
    pub camera_pos: [f32; 3],
    pub time: f32,
    pub resolution: [f32; 2],
    pub debug_mode: u32,
    pub human_visible: u32,
    pub camera_target: [f32; 3],
    pub grid_size: f32,
    pub volume_grid_visible: u32,
    pub placement_height: f32,
    pub show_hud: u32,
    /// Camera pitch in radians (for first-person body visibility)
    /// Negative = looking down, positive = looking up
    pub camera_pitch: f32,
    /// Camera mode: 0 = third-person, 1 = first-person
    pub camera_mode: u32,
    /// Performance overlay toggle: 0 = hidden, 1 = visible (F12 to toggle)
    pub show_perf_overlay: u32,
    /// Current FPS (updated every 0.5 seconds)
    pub perf_fps: f32,
    /// Frame time in milliseconds
    pub perf_frame_time_ms: f32,
    /// Number of placed entities (0-64)
    pub perf_entity_count: u32,
    /// Number of allocated baked SDFs (0-256)
    pub perf_baked_sdf_count: u32,
    /// Tile buffer memory usage in KB
    pub perf_tile_buffer_kb: f32,
    /// Estimated GPU memory usage in MB
    pub perf_gpu_memory_mb: f32,
    /// Number of active tiles (tiles with >0 entities)
    pub perf_active_tile_count: u32,
    /// 1 = sample pre-baked sky cubemap, 0 = procedural sky
    pub use_sky_cubemap: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    /// Player position for first-person body rendering
    pub player_position: [f32; 3],
    pub _pad3: u32,
}

impl Default for TestUniforms {
    fn default() -> Self {
        Self {
            camera_pos: [0.0, 2.0, 8.0],
            time: 0.0,
            resolution: [1920.0, 1080.0],
            debug_mode: 0,
            human_visible: 1,
            camera_target: [0.0, 0.0, 0.0],
            grid_size: 1.0,
            volume_grid_visible: 0,
            placement_height: 0.0,
            show_hud: 1,
            camera_pitch: 0.0,
            camera_mode: 0,       // Third-person by default
            show_perf_overlay: 0, // Hidden by default, F12 to toggle
            perf_fps: 0.0,
            perf_frame_time_ms: 0.0,
            perf_entity_count: 0,
            perf_baked_sdf_count: 0,
            perf_tile_buffer_kb: 0.0,
            perf_gpu_memory_mb: 0.0,
            perf_active_tile_count: 0,
            use_sky_cubemap: 1, // Enable cubemap sampling by default
            _pad1: 0,
            _pad2: 0,
            player_position: [0.0, 0.0, 0.0],
            _pad3: 0,
        }
    }
}

impl TestUniforms {
    /// Create new uniforms with the given camera position and target.
    pub fn new(camera_pos: [f32; 3], camera_target: [f32; 3]) -> Self {
        Self {
            camera_pos,
            camera_target,
            ..Default::default()
        }
    }

    /// Update resolution based on window size.
    pub fn set_resolution(&mut self, width: u32, height: u32) {
        self.resolution = [width as f32, height as f32];
    }

    /// Update time for animations.
    pub fn set_time(&mut self, time: f32) {
        self.time = time;
    }
}

/// Sky settings - must match WGSL SkySettings exactly!
/// Adapted from bevy_sky_gradient for procedural day/night cycle.
/// Extended with weather, season, and temperature systems.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkySettings {
    // Time settings
    pub time_of_day: f32,
    pub cycle_speed: f32,
    pub elapsed_time: f32,
    pub _pad0: f32,

    // Sun
    pub sun_dir: [f32; 3],
    pub sun_sharpness: f32,
    pub sun_color: [f32; 4],
    pub sun_strength: f32,
    pub sun_enabled: u32,
    pub _pad1: f32,
    pub _pad2: f32,

    // Gradient colors (day/night palette)
    pub day_horizon: [f32; 4],
    pub day_zenith: [f32; 4],
    pub sunset_horizon: [f32; 4],
    pub sunset_zenith: [f32; 4],
    pub night_horizon: [f32; 4],
    pub night_zenith: [f32; 4],

    // Stars
    pub stars_enabled: u32,
    pub stars_threshold: f32,
    pub stars_blink_speed: f32,
    pub stars_density: f32,

    // Aurora
    pub aurora_enabled: u32,
    pub aurora_intensity: f32,
    pub aurora_speed: f32,
    pub aurora_height: f32,
    pub aurora_color_bottom: [f32; 4],
    pub aurora_color_top: [f32; 4],

    // Weather system
    pub weather_type: u32,
    pub cloud_coverage: f32,
    pub cloud_density: f32,
    pub cloud_speed: f32,

    // Cloud appearance
    pub cloud_height: f32,
    pub cloud_thickness: f32,
    pub cloud_scale: f32,
    pub cloud_sharpness: f32,

    // Season
    pub season: u32,
    pub season_intensity: f32,
    pub _pad3: f32,
    pub _pad4: f32,

    // Temperature effects
    pub temperature: f32,
    pub humidity: f32,
    pub wind_speed: f32,
    pub wind_direction: f32,

    // Rain/precipitation
    pub rain_intensity: f32,
    pub rain_visibility: f32,
    pub lightning_intensity: f32,
    pub haze_enabled: u32, // 0 = haze OFF, 1 = haze ON (K key to toggle)

    // Fog settings
    pub fog_density: f32,
    pub fog_start_distance: f32,
    pub fog_enabled: u32, // 0 = fog OFF, 1 = fog ON (L key to toggle)
    pub _pad_fog1: f32,

    // Moon system
    pub moon_enabled: u32,
    pub moon_phase: f32,
    pub lunar_day: f32,
    pub moon_sharpness: f32,
    pub moon_color: [f32; 4],
    pub moon_strength: f32,
    pub moon_size: f32,
    pub _pad6: f32,
    pub _pad7: f32,
}

impl Default for SkySettings {
    fn default() -> Self {
        Self {
            time_of_day: 0.25,
            cycle_speed: 0.01,
            elapsed_time: 0.0,
            _pad0: 0.0,

            sun_dir: [0.0, 1.0, 0.0],
            sun_sharpness: 256.0,
            sun_color: [1.0, 0.95, 0.8, 1.0],
            sun_strength: 1.5,
            sun_enabled: 1,
            _pad1: 0.0,
            _pad2: 0.0,

            day_horizon: [0.6, 0.75, 0.9, 1.0],
            day_zenith: [0.3, 0.5, 0.85, 1.0],
            sunset_horizon: [0.9, 0.5, 0.3, 1.0],
            sunset_zenith: [0.5, 0.3, 0.6, 1.0],
            night_horizon: [0.05, 0.08, 0.15, 1.0],
            night_zenith: [0.02, 0.03, 0.08, 1.0],

            stars_enabled: 1,
            stars_threshold: 0.85,
            stars_blink_speed: 2.0,
            stars_density: 30.0,

            aurora_enabled: 1,
            aurora_intensity: 0.8,
            aurora_speed: 0.1,
            aurora_height: 5.0,
            aurora_color_bottom: [0.1, 0.8, 0.4, 1.0],
            aurora_color_top: [0.3, 0.4, 0.9, 1.0],

            weather_type: 1, // Partly cloudy
            cloud_coverage: 0.3,
            cloud_density: 0.6,
            cloud_speed: 0.02,

            cloud_height: 3.0,
            cloud_thickness: 0.5,
            cloud_scale: 1.0,
            cloud_sharpness: 0.5,

            season: 1, // Summer
            season_intensity: 0.5,
            _pad3: 0.0,
            _pad4: 0.0,

            temperature: 0.2,
            humidity: 0.3,
            wind_speed: 0.5,
            wind_direction: 0.0,

            rain_intensity: 0.0,
            rain_visibility: 1.0,
            lightning_intensity: 0.0,
            haze_enabled: 1, // Haze ON by default

            fog_density: 0.005,
            fog_start_distance: 50.0,
            fog_enabled: 1, // Fog ON by default
            _pad_fog1: 0.0,

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

/// Weather type enum for clarity.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum WeatherType {
    Clear = 0,
    PartlyCloudy = 1,
    Cloudy = 2,
    Overcast = 3,
    Rain = 4,
    Storm = 5,
}

/// Season enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u32)]
pub enum Season {
    Spring = 0,
    Summer = 1,
    Fall = 2,
    Winter = 3,
}

/// Placed entity data - must match WGSL PlacedEntity struct.
/// WGSL Layout (vec3<f32> is 16-byte aligned):
///   offset 0:  position (vec3<f32>) = 12 bytes
///   offset 12: _pad_after_pos       = 4 bytes
///   offset 16: entity_type (u32)    = 4 bytes
///   offset 20: _pad_before_scale    = 12 bytes
///   offset 32: scale (vec3<f32>)    = 12 bytes
///   offset 44: color_packed (u32)   = 4 bytes
///   Total: 48 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PlacedEntity {
    pub position: [f32; 3],
    pub _pad_after_pos: u32,
    pub entity_type: u32,
    pub _pad_before_scale: [u32; 3],
    pub scale: [f32; 3],
    pub color_packed: u32,
}

impl Default for PlacedEntity {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            _pad_after_pos: 0,
            entity_type: 0,
            _pad_before_scale: [0, 0, 0],
            scale: [0.5, 0.5, 0.5],
            color_packed: 0xFF8800, // Orange default
        }
    }
}

impl PlacedEntity {
    /// Create a new entity at the given position with type and scale.
    pub fn new(position: [f32; 3], entity_type: u32, scale: f32, color_packed: u32) -> Self {
        Self {
            position,
            _pad_after_pos: 0,
            entity_type,
            _pad_before_scale: [0, 0, 0],
            scale: [scale, scale, scale],
            color_packed,
        }
    }
}

/// Entity buffer header + entities - must match WGSL EntityBuffer struct.
///
/// Buffer layout (3,088 bytes total):
/// - count: 4 bytes (u32)
/// - padding: 12 bytes (3 × u32 for 16-byte alignment)
/// - entities: 64 × 48 = 3,072 bytes
/// Total: 16 + 3,072 = 3,088 bytes (~3 KB)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityBufferData {
    /// Number of active entities in the buffer (0-64)
    pub count: u32,
    /// Padding for 16-byte alignment
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    /// Array of placed entities (64 slots, each 48 bytes)
    pub entities: [PlacedEntity; 64],
}

impl Default for EntityBufferData {
    fn default() -> Self {
        Self {
            count: 0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            entities: [PlacedEntity::default(); 64],
        }
    }
}

// Compile-time assertion to verify struct sizes match WGSL layout
const _: () = {
    assert!(
        std::mem::size_of::<TestUniforms>() == 128,
        "TestUniforms must be 128 bytes to match WGSL"
    );
    assert!(
        std::mem::size_of::<PlacedEntity>() == 48,
        "PlacedEntity must be 48 bytes to match WGSL"
    );
    assert!(
        std::mem::size_of::<EntityBufferData>() == 16 + 48 * 64,
        "EntityBufferData size mismatch"
    );
};

/// Pack RGB values into a u32.
pub fn pack_color(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Predefined colors for placed objects.
pub const ENTITY_COLORS: [(u8, u8, u8); 8] = [
    (255, 100, 50),  // Orange
    (50, 200, 255),  // Cyan
    (255, 50, 150),  // Pink
    (150, 255, 50),  // Lime
    (255, 200, 50),  // Yellow
    (150, 50, 255),  // Purple
    (50, 255, 150),  // Mint
    (255, 150, 200), // Light pink
];
