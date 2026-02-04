//! Shader Loading Utilities
//!
//! Provides utilities for loading and compiling WGSL shaders for the render pipeline.
//! Supports both embedded (compile-time) and runtime shader loading.

use std::path::Path;

/// Shader source that can be either embedded at compile time or loaded at runtime.
pub enum ShaderSource {
    /// Embedded shader source (faster, no file I/O at runtime)
    Embedded(&'static str),
    /// Runtime-loaded shader source
    Runtime(String),
}

impl ShaderSource {
    /// Get the shader source as a string slice.
    pub fn as_str(&self) -> &str {
        match self {
            ShaderSource::Embedded(s) => s,
            ShaderSource::Runtime(s) => s.as_str(),
        }
    }
}

/// Load a shader from the filesystem at runtime.
///
/// # Arguments
/// * `path` - Path to the WGSL shader file
///
/// # Returns
/// The shader source as a string, or an error if the file couldn't be read.
pub fn load_shader_file(path: impl AsRef<Path>) -> Result<ShaderSource, std::io::Error> {
    let source = std::fs::read_to_string(path)?;
    Ok(ShaderSource::Runtime(source))
}

/// Create a wgpu shader module from the given source.
///
/// # Arguments
/// * `device` - The wgpu device to create the shader module on
/// * `label` - Label for debugging
/// * `source` - The WGSL shader source
pub fn create_shader_module(
    device: &wgpu::Device,
    label: &str,
    source: &ShaderSource,
) -> wgpu::ShaderModule {
    device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(label),
        source: wgpu::ShaderSource::Wgsl(source.as_str().into()),
    })
}

/// Common shader paths used by the engine.
pub mod paths {
    /// Main SDF core test shader
    pub const SDF_CORE_TEST: &str = "src/shaders/sdf_core_test.wgsl";

    /// Sky gradient shader
    pub const SKY_GRADIENT: &str = "engine/shaders/sky_gradient.wgsl";

    /// Lighting utilities
    pub const LIGHTING: &str = "shaders/lighting.wgsl";

    /// Ray marcher core
    pub const RAYMARCHER: &str = "shaders/raymarcher.wgsl";

    /// SDF primitives library (engine module)
    /// Contains: sphere, box, capsule, torus, cylinder, ellipsoid, rounded_box
    /// Plus smooth blending operations: smin, smax, smin_lod
    /// Plus domain operations: translate, scale, repeat, mirror
    pub const SDF_PRIMITIVES: &str = "engine/shaders/sdf_primitives.wgsl";

    /// SDF primitives library (legacy location)
    pub const SDF_PRIMITIVES_LEGACY: &str = "shaders/sdf_primitives.wgsl";

    /// SDF human figure module (~50 primitives)
    /// Contains: sdf_human, sdf_human_lod_low, sdf_human_lod_silhouette, sdf_human_lod
    pub const SDF_HUMAN: &str = "engine/shaders/sdf_human.wgsl";

    /// Noise functions
    pub const NOISE: &str = "shaders/noise.wgsl";

    /// Handle overlay shader
    pub const HANDLE_OVERLAY: &str = "shaders/handle_overlay.wgsl";
}

/// Embedded shaders that are compiled into the binary.
/// These are loaded at compile time for faster startup.
pub mod embedded {
    /// The main SDF core test shader, embedded at compile time.
    pub const SDF_CORE_TEST: &str = include_str!("../../../src/shaders/sdf_core_test.wgsl");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_source_embedded() {
        let source = ShaderSource::Embedded("void main() {}");
        assert_eq!(source.as_str(), "void main() {}");
    }

    #[test]
    fn test_shader_source_runtime() {
        let source = ShaderSource::Runtime("void main() {}".to_string());
        assert_eq!(source.as_str(), "void main() {}");
    }
}
