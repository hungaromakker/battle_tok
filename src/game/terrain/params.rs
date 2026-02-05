//! Terrain Parameters
//!
//! Configurable parameters for procedural terrain generation.

/// Water level relative to base terrain height
pub const WATER_LEVEL: f32 = 0.5;

/// Adjustable terrain generation parameters
#[derive(Clone, Copy)]
pub struct TerrainParams {
    pub height_scale: f32,
    pub mountains: f32,
    pub rocks: f32,
    pub hills: f32,
    pub detail: f32,
    pub water: f32,
}

impl Default for TerrainParams {
    fn default() -> Self {
        // F1 preset: flat terrain for building and combat testing
        Self {
            height_scale: 0.1,
            mountains: 0.0,
            rocks: 0.0,
            hills: 0.2,
            detail: 0.1,
            water: 0.0,
        }
    }
}

// Global terrain parameters (mutable via UI)
// Default: F1 flat terrain for building and combat testing
static mut TERRAIN_PARAMS: TerrainParams = TerrainParams {
    height_scale: 0.1,
    mountains: 0.0,
    rocks: 0.0,
    hills: 0.2,
    detail: 0.1,
    water: 0.0,
};

/// Get current terrain parameters
pub fn get_terrain_params() -> TerrainParams {
    unsafe { TERRAIN_PARAMS }
}

/// Set terrain parameters (called from UI)
pub fn set_terrain_params(params: TerrainParams) {
    unsafe {
        TERRAIN_PARAMS = params;
    }
}
