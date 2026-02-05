//! Material System Coordinator
//!
//! Unified material management system that organizes render pipelines and provides
//! a clean interface for switching materials per-object. This centralizes creation
//! of all material pipelines and provides shared scene uniforms across materials.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Initialize
//! let material_system = MaterialSystem::new(&device, surface_format);
//!
//! // Each frame - update shared scene uniforms
//! material_system.update_scene_uniforms(&queue, view_proj, camera_pos, time, &scene_config);
//!
//! // Render with specific materials
//! material_system.render_with_material(
//!     &mut render_pass,
//!     MaterialType::CastleStone,
//!     &vertex_buffer,
//!     &index_buffer,
//!     index_count,
//! );
//! ```

use std::collections::HashMap;
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

/// Material type enumeration for all supported materials.
///
/// Each variant corresponds to a specific shader pipeline optimized for
/// different surface types in the game world.
#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum MaterialType {
    /// Terrain material - procedural hex terrain with elevation-based colors
    Terrain,
    /// Castle stone material - medieval stone with mortar, grime, and torch lighting
    CastleStone,
    /// Wood plank material - brown wood with grain variation
    WoodPlank,
    /// Chain metal material - metallic steel with Fresnel rim highlights
    ChainMetal,
    /// Flag material - team-colored cloth with wind animation
    Flag,
    /// Lava material - animated flowing lava with emissive cracks
    Lava,
    /// Ember particle material - glowing billboard particles (uses separate system)
    EmberParticle,
}

impl MaterialType {
    /// Get all material types as an iterator
    pub fn all() -> impl Iterator<Item = MaterialType> {
        [
            MaterialType::Terrain,
            MaterialType::CastleStone,
            MaterialType::WoodPlank,
            MaterialType::ChainMetal,
            MaterialType::Flag,
            MaterialType::Lava,
            MaterialType::EmberParticle,
        ]
        .into_iter()
    }

    /// Get a human-readable name for the material type
    pub fn name(&self) -> &'static str {
        match self {
            MaterialType::Terrain => "Terrain",
            MaterialType::CastleStone => "Castle Stone",
            MaterialType::WoodPlank => "Wood Plank",
            MaterialType::ChainMetal => "Chain Metal",
            MaterialType::Flag => "Flag",
            MaterialType::Lava => "Lava",
            MaterialType::EmberParticle => "Ember Particle",
        }
    }
}

/// Scene configuration for updating shared uniforms
#[derive(Clone, Copy, Debug)]
pub struct SceneConfig {
    /// Sun direction (normalized)
    pub sun_dir: Vec3,
    /// Fog density (0.0 to 1.0)
    pub fog_density: f32,
    /// Fog color (RGB)
    pub fog_color: Vec3,
    /// Ambient light strength (0.0 to 1.0)
    pub ambient: f32,
}

impl Default for SceneConfig {
    fn default() -> Self {
        Self {
            sun_dir: Vec3::new(0.5, 0.7, 0.3).normalize(),
            fog_density: 0.5,
            fog_color: Vec3::new(0.12, 0.1, 0.08),
            ambient: 0.3,
        }
    }
}

impl SceneConfig {
    /// Create a preset for apocalyptic battle arena atmosphere
    pub fn battle_arena() -> Self {
        Self {
            sun_dir: Vec3::new(0.0, 0.15, -1.0).normalize(),
            fog_density: 0.6,
            fog_color: Vec3::new(0.35, 0.15, 0.1), // Orange-brown fog
            ambient: 0.2,
        }
    }

    /// Create a preset for bright exterior
    pub fn exterior() -> Self {
        Self {
            sun_dir: Vec3::new(0.4, 0.8, 0.2).normalize(),
            fog_density: 0.3,
            fog_color: Vec3::new(0.5, 0.55, 0.6),
            ambient: 0.4,
        }
    }

    /// Create a preset for dark dungeon
    pub fn dungeon() -> Self {
        Self {
            sun_dir: Vec3::new(0.2, 0.3, -0.9).normalize(),
            fog_density: 0.8,
            fog_color: Vec3::new(0.05, 0.05, 0.07),
            ambient: 0.15,
        }
    }
}

/// Shared scene uniforms passed to all materials.
///
/// This struct is uploaded once per frame and shared across all material pipelines.
/// Layout uses scalar fields for vec3 to ensure proper GPU alignment.
///
/// Total size: 128 bytes (aligned to 16)
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct SceneUniforms {
    /// View-projection matrix (64 bytes)
    pub view_proj: [[f32; 4]; 4],
    /// Camera position X
    pub camera_pos_x: f32,
    /// Camera position Y
    pub camera_pos_y: f32,
    /// Camera position Z
    pub camera_pos_z: f32,
    /// Current time in seconds
    pub time: f32,
    /// Sun direction X (normalized)
    pub sun_dir_x: f32,
    /// Sun direction Y (normalized)
    pub sun_dir_y: f32,
    /// Sun direction Z (normalized)
    pub sun_dir_z: f32,
    /// Fog density (0.0 to 1.0)
    pub fog_density: f32,
    /// Fog color R
    pub fog_color_r: f32,
    /// Fog color G
    pub fog_color_g: f32,
    /// Fog color B
    pub fog_color_b: f32,
    /// Ambient light strength
    pub ambient: f32,
    /// Padding to align to 128 bytes
    pub _pad1: f32,
    pub _pad2: f32,
    pub _pad3: f32,
    pub _pad4: f32,
}

// Verify struct size at compile time (must be multiple of 16)
const _: () = assert!(std::mem::size_of::<SceneUniforms>() == 128);

impl SceneUniforms {
    /// Create scene uniforms from components
    pub fn new(
        view_proj: Mat4,
        camera_pos: Vec3,
        time: f32,
        config: &SceneConfig,
    ) -> Self {
        Self {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos_x: camera_pos.x,
            camera_pos_y: camera_pos.y,
            camera_pos_z: camera_pos.z,
            time,
            sun_dir_x: config.sun_dir.x,
            sun_dir_y: config.sun_dir.y,
            sun_dir_z: config.sun_dir.z,
            fog_density: config.fog_density,
            fog_color_r: config.fog_color.x,
            fog_color_g: config.fog_color.y,
            fog_color_b: config.fog_color.z,
            ambient: config.ambient,
            _pad1: 0.0,
            _pad2: 0.0,
            _pad3: 0.0,
            _pad4: 0.0,
        }
    }
}

/// Material registration entry containing pipeline and bind group information
pub struct MaterialEntry {
    /// The render pipeline for this material
    pub pipeline: wgpu::RenderPipeline,
    /// Material-specific bind group (if any)
    pub bind_group: Option<wgpu::BindGroup>,
    /// Whether this material uses the shared scene bind group at slot 0
    pub uses_scene_uniforms: bool,
}

/// Material System Coordinator
///
/// Manages all material pipelines and provides a unified interface for:
/// - Centralizing pipeline creation
/// - Sharing scene uniforms across materials
/// - Easy material switching during rendering
/// - Simplified material registration for new materials
pub struct MaterialSystem {
    /// Registered material pipelines and bind groups
    materials: HashMap<MaterialType, MaterialEntry>,
    /// Shared scene uniform buffer
    scene_uniform_buffer: wgpu::Buffer,
    /// Shared scene bind group
    scene_bind_group: wgpu::BindGroup,
    /// Scene bind group layout (for external pipeline creation)
    scene_bind_group_layout: wgpu::BindGroupLayout,
    /// Current scene configuration
    scene_config: SceneConfig,
}

impl MaterialSystem {
    /// Create a new material system with shared scene uniforms.
    ///
    /// This creates the scene bind group layout and buffer but does NOT create
    /// any material pipelines. Use `register_material()` to add materials, or
    /// use the individual material structs (CastleMaterial, etc.) which manage
    /// their own pipelines.
    ///
    /// # Arguments
    /// * `device` - The wgpu device to create GPU resources on
    pub fn new(device: &wgpu::Device) -> Self {
        // Create scene bind group layout (shared by all materials that use scene uniforms)
        let scene_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Scene Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create scene uniform buffer
        let scene_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Scene Uniform Buffer"),
            size: std::mem::size_of::<SceneUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create scene bind group
        let scene_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Scene Bind Group"),
            layout: &scene_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scene_uniform_buffer.as_entire_binding(),
            }],
        });

        println!("[MaterialSystem] Initialized with shared scene uniforms");

        Self {
            materials: HashMap::new(),
            scene_uniform_buffer,
            scene_bind_group,
            scene_bind_group_layout,
            scene_config: SceneConfig::default(),
        }
    }

    /// Get the scene bind group layout for external pipeline creation.
    ///
    /// Materials that want to use shared scene uniforms should include this
    /// layout in their pipeline layout at bind group index 0.
    pub fn scene_bind_group_layout(&self) -> &wgpu::BindGroupLayout {
        &self.scene_bind_group_layout
    }

    /// Get the scene bind group for external binding.
    pub fn scene_bind_group(&self) -> &wgpu::BindGroup {
        &self.scene_bind_group
    }

    /// Get the current scene configuration
    pub fn scene_config(&self) -> &SceneConfig {
        &self.scene_config
    }

    /// Set the scene configuration
    pub fn set_scene_config(&mut self, config: SceneConfig) {
        self.scene_config = config;
    }

    /// Register a material with its pipeline and optional bind group.
    ///
    /// # Arguments
    /// * `material_type` - The type of material being registered
    /// * `pipeline` - The render pipeline for this material
    /// * `bind_group` - Optional material-specific bind group
    /// * `uses_scene_uniforms` - Whether this material uses scene uniforms at slot 0
    pub fn register_material(
        &mut self,
        material_type: MaterialType,
        pipeline: wgpu::RenderPipeline,
        bind_group: Option<wgpu::BindGroup>,
        uses_scene_uniforms: bool,
    ) {
        println!(
            "[MaterialSystem] Registered material: {} (uses_scene_uniforms: {})",
            material_type.name(),
            uses_scene_uniforms
        );
        self.materials.insert(
            material_type,
            MaterialEntry {
                pipeline,
                bind_group,
                uses_scene_uniforms,
            },
        );
    }

    /// Check if a material is registered
    pub fn has_material(&self, material_type: MaterialType) -> bool {
        self.materials.contains_key(&material_type)
    }

    /// Get a registered material entry
    pub fn get_material(&self, material_type: MaterialType) -> Option<&MaterialEntry> {
        self.materials.get(&material_type)
    }

    /// Get the number of registered materials
    pub fn material_count(&self) -> usize {
        self.materials.len()
    }

    /// Update the shared scene uniforms buffer.
    ///
    /// Call this once per frame before any rendering. The uniforms will be
    /// available to all materials that use the scene bind group.
    ///
    /// # Arguments
    /// * `queue` - The wgpu queue for buffer writes
    /// * `view_proj` - Combined view-projection matrix
    /// * `camera_pos` - World-space camera position
    /// * `time` - Current time in seconds
    pub fn update_scene_uniforms(
        &self,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        camera_pos: Vec3,
        time: f32,
    ) {
        let uniforms = SceneUniforms::new(view_proj, camera_pos, time, &self.scene_config);
        queue.write_buffer(&self.scene_uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Update scene uniforms with a custom configuration.
    ///
    /// # Arguments
    /// * `queue` - The wgpu queue for buffer writes
    /// * `view_proj` - Combined view-projection matrix
    /// * `camera_pos` - World-space camera position
    /// * `time` - Current time in seconds
    /// * `config` - Custom scene configuration
    pub fn update_scene_uniforms_with_config(
        &self,
        queue: &wgpu::Queue,
        view_proj: Mat4,
        camera_pos: Vec3,
        time: f32,
        config: &SceneConfig,
    ) {
        let uniforms = SceneUniforms::new(view_proj, camera_pos, time, config);
        queue.write_buffer(&self.scene_uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Render geometry with a specific material.
    ///
    /// This method:
    /// 1. Sets the pipeline for the specified material
    /// 2. Binds the scene uniform bind group at slot 0 (if the material uses it)
    /// 3. Binds the material-specific bind group at slot 1 (if present)
    /// 4. Sets vertex and index buffers
    /// 5. Issues the draw call
    ///
    /// # Arguments
    /// * `pass` - The active render pass
    /// * `material` - Which material to use
    /// * `vertex_buffer` - Vertex buffer to bind
    /// * `index_buffer` - Index buffer to bind
    /// * `index_count` - Number of indices to draw
    ///
    /// # Returns
    /// `true` if the material was found and rendering was issued, `false` otherwise
    pub fn render_with_material<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        material: MaterialType,
        vertex_buffer: &'a wgpu::Buffer,
        index_buffer: &'a wgpu::Buffer,
        index_count: u32,
    ) -> bool {
        if let Some(entry) = self.materials.get(&material) {
            pass.set_pipeline(&entry.pipeline);

            if entry.uses_scene_uniforms {
                pass.set_bind_group(0, &self.scene_bind_group, &[]);
            }

            if let Some(ref mat_bind_group) = entry.bind_group {
                let slot = if entry.uses_scene_uniforms { 1 } else { 0 };
                pass.set_bind_group(slot, mat_bind_group, &[]);
            }

            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..index_count, 0, 0..1);
            true
        } else {
            false
        }
    }

    /// Render geometry with a specific material using u16 indices.
    ///
    /// Same as `render_with_material` but uses 16-bit indices.
    pub fn render_with_material_u16<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        material: MaterialType,
        vertex_buffer: &'a wgpu::Buffer,
        index_buffer: &'a wgpu::Buffer,
        index_count: u32,
    ) -> bool {
        if let Some(entry) = self.materials.get(&material) {
            pass.set_pipeline(&entry.pipeline);

            if entry.uses_scene_uniforms {
                pass.set_bind_group(0, &self.scene_bind_group, &[]);
            }

            if let Some(ref mat_bind_group) = entry.bind_group {
                let slot = if entry.uses_scene_uniforms { 1 } else { 0 };
                pass.set_bind_group(slot, mat_bind_group, &[]);
            }

            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..index_count, 0, 0..1);
            true
        } else {
            false
        }
    }

    /// Bind a material's pipeline and bind groups without issuing a draw call.
    ///
    /// Use this when you need more control over vertex/index buffer binding
    /// or draw call parameters.
    ///
    /// # Arguments
    /// * `pass` - The active render pass
    /// * `material` - Which material to bind
    ///
    /// # Returns
    /// `true` if the material was found and bound, `false` otherwise
    pub fn bind_material<'a>(
        &'a self,
        pass: &mut wgpu::RenderPass<'a>,
        material: MaterialType,
    ) -> bool {
        if let Some(entry) = self.materials.get(&material) {
            pass.set_pipeline(&entry.pipeline);

            if entry.uses_scene_uniforms {
                pass.set_bind_group(0, &self.scene_bind_group, &[]);
            }

            if let Some(ref mat_bind_group) = entry.bind_group {
                let slot = if entry.uses_scene_uniforms { 1 } else { 0 };
                pass.set_bind_group(slot, mat_bind_group, &[]);
            }
            true
        } else {
            false
        }
    }

    /// Get a list of all registered material types
    pub fn registered_materials(&self) -> Vec<MaterialType> {
        self.materials.keys().copied().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_uniforms_size() {
        assert_eq!(std::mem::size_of::<SceneUniforms>(), 128);
    }

    #[test]
    fn test_material_type_all() {
        let all: Vec<_> = MaterialType::all().collect();
        assert_eq!(all.len(), 7);
        assert!(all.contains(&MaterialType::Terrain));
        assert!(all.contains(&MaterialType::CastleStone));
        assert!(all.contains(&MaterialType::WoodPlank));
        assert!(all.contains(&MaterialType::ChainMetal));
        assert!(all.contains(&MaterialType::Flag));
        assert!(all.contains(&MaterialType::Lava));
        assert!(all.contains(&MaterialType::EmberParticle));
    }

    #[test]
    fn test_material_type_names() {
        assert_eq!(MaterialType::Terrain.name(), "Terrain");
        assert_eq!(MaterialType::CastleStone.name(), "Castle Stone");
        assert_eq!(MaterialType::WoodPlank.name(), "Wood Plank");
        assert_eq!(MaterialType::ChainMetal.name(), "Chain Metal");
        assert_eq!(MaterialType::Flag.name(), "Flag");
        assert_eq!(MaterialType::Lava.name(), "Lava");
        assert_eq!(MaterialType::EmberParticle.name(), "Ember Particle");
    }

    #[test]
    fn test_scene_config_default() {
        let config = SceneConfig::default();
        assert!(config.fog_density > 0.0);
        assert!(config.ambient > 0.0);
    }

    #[test]
    fn test_scene_config_presets() {
        let arena = SceneConfig::battle_arena();
        let exterior = SceneConfig::exterior();
        let dungeon = SceneConfig::dungeon();

        // Battle arena should have orange-ish fog
        assert!(arena.fog_color.x > arena.fog_color.z);

        // Exterior should have lighter fog density
        assert!(exterior.fog_density < arena.fog_density);

        // Dungeon should have darkest ambient
        assert!(dungeon.ambient < exterior.ambient);
    }
}
