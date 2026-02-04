//! Game Render Module
//!
//! Game-specific rendering components including uniforms and shader source.

pub mod uniforms;
pub mod shader;
pub mod walls;
pub mod preview;

pub use uniforms::{Uniforms, HexPrismModelUniforms, SdfCannonUniforms, SdfCannonData};
pub use uniforms::{TerrainParams, LavaParams, SkyStormParams, FogPostParams, TonemapParams};
pub use shader::SHADER_SOURCE;
pub use walls::{MergedMeshBuffers, create_test_walls};
pub use preview::{generate_hex_grid_overlay, calculate_ghost_color, GHOST_PREVIEW_COLOR, generate_block_preview_mesh};
