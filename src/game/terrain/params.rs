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
        Self {
            height_scale: 1.0,
            mountains: 0.7,
            rocks: 0.3,
            hills: 0.5,
            detail: 0.4,
            water: 0.3,
        }
    }
}

// Global terrain parameters (mutable via UI)
static mut TERRAIN_PARAMS: TerrainParams = TerrainParams {
    height_scale: 1.0,
    mountains: 0.7,
    rocks: 0.3,
    hills: 0.5,
    detail: 0.4,
    water: 0.3,
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
