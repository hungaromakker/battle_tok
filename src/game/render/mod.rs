//! Game Render Module
//!
//! Game-specific rendering components including uniforms and shader source.

pub mod uniforms;
pub mod shader;
pub mod walls;

pub use uniforms::{Uniforms, HexPrismModelUniforms, SdfCannonUniforms, SdfCannonData};
pub use shader::SHADER_SOURCE;
pub use walls::{MergedMeshBuffers, create_test_walls};
