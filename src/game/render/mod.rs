//! Game Render Module
//!
//! Game-specific rendering components including uniforms and shader source.

pub mod preview;
pub mod shader;
pub mod uniforms;
pub mod walls;

pub use preview::{
    GHOST_PREVIEW_COLOR, calculate_ghost_color, generate_block_preview_mesh,
    generate_hex_grid_overlay,
};
pub use shader::SHADER_SOURCE;
pub use uniforms::{FogPostParams, LavaParams, SkyStormParams, TerrainParams, TonemapParams};
pub use uniforms::{HexPrismModelUniforms, SdfCannonData, SdfCannonUniforms, Uniforms};
pub use walls::{MergedMeshBuffers, create_test_walls};
