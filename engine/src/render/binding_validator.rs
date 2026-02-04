//! Shader Binding Validator (US-P2-014)
//!
//! Validates that all bind group layouts match their expected shader bindings
//! at startup. Catches mismatches between Rust-side layouts and WGSL declarations
//! before they cause GPU validation errors at render time.
//!
//! The expected bindings are defined here as the canonical source of truth,
//! matching the WGSL shader declarations. The actual bind group layout entries
//! used during pipeline creation are passed in for comparison.

use std::fmt;

/// Describes a single expected binding in a bind group layout.
#[derive(Debug, Clone)]
struct ExpectedBinding {
    binding: u32,
    binding_type: ExpectedBindingType,
    label: &'static str,
}

/// The type of a binding, matching wgpu::BindingType variants we use.
#[derive(Debug, Clone, PartialEq)]
enum ExpectedBindingType {
    UniformBuffer,
    StorageBufferReadOnly,
    StorageBufferReadWrite,
    TextureCube,
    Sampler,
}

impl fmt::Display for ExpectedBindingType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UniformBuffer => write!(f, "uniform buffer"),
            Self::StorageBufferReadOnly => write!(f, "storage buffer (read-only)"),
            Self::StorageBufferReadWrite => write!(f, "storage buffer (read-write)"),
            Self::TextureCube => write!(f, "texture cube"),
            Self::Sampler => write!(f, "sampler"),
        }
    }
}

/// Describes the expected layout for one bind group of a pipeline.
struct ExpectedBindGroup {
    pipeline_name: &'static str,
    group_index: u32,
    bindings: Vec<ExpectedBinding>,
}

/// Classifies a wgpu::BindGroupLayoutEntry into our ExpectedBindingType.
fn classify_entry(entry: &wgpu::BindGroupLayoutEntry) -> ExpectedBindingType {
    match &entry.ty {
        wgpu::BindingType::Buffer { ty, .. } => match ty {
            wgpu::BufferBindingType::Uniform => ExpectedBindingType::UniformBuffer,
            wgpu::BufferBindingType::Storage { read_only: true } => ExpectedBindingType::StorageBufferReadOnly,
            wgpu::BufferBindingType::Storage { read_only: false } => ExpectedBindingType::StorageBufferReadWrite,
        },
        wgpu::BindingType::Texture { view_dimension: wgpu::TextureViewDimension::Cube, .. } => {
            ExpectedBindingType::TextureCube
        }
        wgpu::BindingType::Sampler(_) => ExpectedBindingType::Sampler,
        _ => ExpectedBindingType::UniformBuffer, // fallback for unhandled types
    }
}

/// Validates actual bind group layout entries against expected bindings.
/// Returns the number of mismatches found.
fn validate_bind_group(
    expected: &ExpectedBindGroup,
    actual_entries: &[wgpu::BindGroupLayoutEntry],
) -> u32 {
    let mut mismatches = 0u32;

    for exp in &expected.bindings {
        match actual_entries.iter().find(|e| e.binding == exp.binding) {
            None => {
                eprintln!(
                    "[BindingValidator] MISMATCH in '{}' group {} binding {}: expected {} ({}), actual: MISSING",
                    expected.pipeline_name, expected.group_index, exp.binding, exp.binding_type, exp.label
                );
                mismatches += 1;
            }
            Some(actual) => {
                let actual_type = classify_entry(actual);
                if actual_type != exp.binding_type {
                    eprintln!(
                        "[BindingValidator] MISMATCH in '{}' group {} binding {}: expected {} ({}), actual: {}",
                        expected.pipeline_name, expected.group_index, exp.binding,
                        exp.binding_type, exp.label, actual_type
                    );
                    mismatches += 1;
                }
            }
        }
    }

    for actual in actual_entries {
        if !expected.bindings.iter().any(|e| e.binding == actual.binding) {
            let actual_type = classify_entry(actual);
            eprintln!(
                "[BindingValidator] EXTRA binding in '{}' group {} binding {}: type {} not in shader expectations",
                expected.pipeline_name, expected.group_index, actual.binding, actual_type
            );
            mismatches += 1;
        }
    }

    mismatches
}

/// Validate render pipeline bind groups (group 0 and group 1).
///
/// Call this from `RenderState::new()` after creating bind group layouts,
/// passing the same entry slices used for layout creation.
pub fn validate_render_bindings(
    group0_entries: &[wgpu::BindGroupLayoutEntry],
    group1_entries: &[wgpu::BindGroupLayoutEntry],
) -> u32 {
    let mut total = 0u32;

    // Render group 0: raymarcher.wgsl declarations
    let render_g0 = ExpectedBindGroup {
        pipeline_name: "Render",
        group_index: 0,
        bindings: vec![
            ExpectedBinding { binding: 0, binding_type: ExpectedBindingType::UniformBuffer, label: "GpuUniforms" },
            ExpectedBinding { binding: 1, binding_type: ExpectedBindingType::StorageBufferReadOnly, label: "EntityBuffer" },
            ExpectedBinding { binding: 2, binding_type: ExpectedBindingType::UniformBuffer, label: "SkySettings" },
            ExpectedBinding { binding: 3, binding_type: ExpectedBindingType::StorageBufferReadOnly, label: "FroxelBoundsBuffer" },
            ExpectedBinding { binding: 4, binding_type: ExpectedBindingType::StorageBufferReadOnly, label: "FroxelSDFListBuffer" },
            ExpectedBinding { binding: 5, binding_type: ExpectedBindingType::StorageBufferReadOnly, label: "TileBuffer" },
            ExpectedBinding { binding: 8, binding_type: ExpectedBindingType::TextureCube, label: "SkyCubemap" },
            ExpectedBinding { binding: 9, binding_type: ExpectedBindingType::Sampler, label: "SkySampler" },
        ],
    };
    total += validate_bind_group(&render_g0, group0_entries);

    // Render group 1: BrickCache SSBO (read-only for fragment shader)
    let render_g1 = ExpectedBindGroup {
        pipeline_name: "Render",
        group_index: 1,
        bindings: vec![
            ExpectedBinding { binding: 0, binding_type: ExpectedBindingType::StorageBufferReadOnly, label: "BrickCache SSBO" },
        ],
    };
    total += validate_bind_group(&render_g1, group1_entries);

    total
}

/// Validate all compute pipeline bind groups.
///
/// Call this from `ComputePipelines::new()` after creating bind group layouts,
/// passing the same entry slices used for layout creation.
pub fn validate_compute_bindings(
    sdf_bake_entries: &[wgpu::BindGroupLayoutEntry],
    froxel_clear_entries: &[wgpu::BindGroupLayoutEntry],
    froxel_assign_entries: &[wgpu::BindGroupLayoutEntry],
    tile_culling_entries: &[wgpu::BindGroupLayoutEntry],
) -> u32 {
    let mut total = 0u32;

    let sdf_bake = ExpectedBindGroup {
        pipeline_name: "SDF Bake",
        group_index: 0,
        bindings: vec![
            ExpectedBinding { binding: 0, binding_type: ExpectedBindingType::UniformBuffer, label: "BakeParams" },
            ExpectedBinding { binding: 1, binding_type: ExpectedBindingType::StorageBufferReadWrite, label: "SDF output" },
        ],
    };
    total += validate_bind_group(&sdf_bake, sdf_bake_entries);

    let froxel_clear = ExpectedBindGroup {
        pipeline_name: "Froxel Clear",
        group_index: 0,
        bindings: vec![
            ExpectedBinding { binding: 0, binding_type: ExpectedBindingType::StorageBufferReadWrite, label: "FroxelSDFListBuffer" },
        ],
    };
    total += validate_bind_group(&froxel_clear, froxel_clear_entries);

    let froxel_assign = ExpectedBindGroup {
        pipeline_name: "Froxel Assign",
        group_index: 0,
        bindings: vec![
            ExpectedBinding { binding: 0, binding_type: ExpectedBindingType::StorageBufferReadOnly, label: "SdfBoundsBuffer" },
            ExpectedBinding { binding: 1, binding_type: ExpectedBindingType::StorageBufferReadOnly, label: "FroxelBoundsBuffer" },
            ExpectedBinding { binding: 2, binding_type: ExpectedBindingType::StorageBufferReadWrite, label: "FroxelSDFListBuffer" },
            ExpectedBinding { binding: 3, binding_type: ExpectedBindingType::UniformBuffer, label: "AssignmentUniforms" },
        ],
    };
    total += validate_bind_group(&froxel_assign, froxel_assign_entries);

    let tile_culling = ExpectedBindGroup {
        pipeline_name: "Tile Culling",
        group_index: 0,
        bindings: vec![
            ExpectedBinding { binding: 0, binding_type: ExpectedBindingType::StorageBufferReadWrite, label: "TileBuffer" },
            ExpectedBinding { binding: 1, binding_type: ExpectedBindingType::StorageBufferReadOnly, label: "EntityBuffer" },
            ExpectedBinding { binding: 2, binding_type: ExpectedBindingType::UniformBuffer, label: "CullingUniforms" },
        ],
    };
    total += validate_bind_group(&tile_culling, tile_culling_entries);

    total
}

/// Run all binding validations at startup. Logs results and returns total mismatches.
pub fn validate_all_startup_bindings(
    render_group0: &[wgpu::BindGroupLayoutEntry],
    render_group1: &[wgpu::BindGroupLayoutEntry],
    sdf_bake: &[wgpu::BindGroupLayoutEntry],
    froxel_clear: &[wgpu::BindGroupLayoutEntry],
    froxel_assign: &[wgpu::BindGroupLayoutEntry],
    tile_culling: &[wgpu::BindGroupLayoutEntry],
) -> u32 {
    println!("[BindingValidator] Validating shader bindings against bind group layouts...");

    let mut total = 0u32;
    total += validate_render_bindings(render_group0, render_group1);
    total += validate_compute_bindings(sdf_bake, froxel_clear, froxel_assign, tile_culling);

    if total == 0 {
        println!("[BindingValidator] All shader bindings validated OK (6 bind groups across render + compute pipelines)");
    } else {
        eprintln!(
            "[BindingValidator] WARNING: {} binding mismatch(es) found! GPU validation errors may occur.",
            total
        );
    }

    total
}
