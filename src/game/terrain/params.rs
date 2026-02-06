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
        // F2 preset: natural terrain with gentle hills and some rocks
        Self {
            height_scale: 0.3,
            mountains: 0.1,
            rocks: 0.1,
            hills: 0.4,
            detail: 0.2,
            water: 0.0,
        }
    }
}

// Global terrain parameters (mutable via UI)
// Default: F2 natural terrain with gentle hills
static mut TERRAIN_PARAMS: TerrainParams = TerrainParams {
    height_scale: 0.3,
    mountains: 0.1,
    rocks: 0.1,
    hills: 0.4,
    detail: 0.2,
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
