//! Entity System - PlacedEntity and EntityBufferData structs for GPU rendering
//!
//! This module contains the data structures for representing placed entities
//! in the SDF rendering system. The structs are GPU-compatible with careful
//! alignment to match WGSL struct layouts.

/// Precision class for raymarching step size control.
///
/// Different entity types require different precision levels for accurate rendering:
/// - **Player**: Highest precision (1.0x step multiplier) for the player model
/// - **Interactive**: High precision (1.2x) for creatures and interactive objects
/// - **Static**: Medium precision (1.5x) for static objects like trees, rocks
/// - **Terrain**: Lowest precision (2.0x) for terrain features that are large-scale
///
/// Lower multipliers mean smaller raymarching steps = higher precision but slower.
/// Higher multipliers mean larger raymarching steps = lower precision but faster.
#[repr(u32)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum PrecisionClass {
    /// Highest precision - for player model (1.0x step multiplier)
    Player = 0,
    /// High precision - for interactive objects and creatures (1.2x step multiplier)
    Interactive = 1,
    /// Medium precision - default for static objects (1.5x step multiplier)
    #[default]
    Static = 2,
    /// Low precision - for large terrain features (2.0x step multiplier)
    Terrain = 3,
}

impl PrecisionClass {
    /// Returns the step size multiplier for this precision class.
    ///
    /// Lower values = higher precision (smaller steps, more accurate)
    /// Higher values = lower precision (larger steps, faster rendering)
    #[inline]
    pub fn multiplier(&self) -> f32 {
        match self {
            PrecisionClass::Player => 1.0,
            PrecisionClass::Interactive => 1.2,
            PrecisionClass::Static => 1.5,
            PrecisionClass::Terrain => 2.0,
        }
    }

    /// Convert from u32 (for GPU buffer compatibility).
    #[inline]
    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => PrecisionClass::Player,
            1 => PrecisionClass::Interactive,
            2 => PrecisionClass::Static,
            3 => PrecisionClass::Terrain,
            _ => PrecisionClass::Static, // Default to Static for invalid values
        }
    }

    /// Convert to u32 (for GPU buffer compatibility).
    #[inline]
    pub fn to_u32(self) -> u32 {
        self as u32
    }
}

/// Placed entity data - must match WGSL PlacedEntity struct
/// WGSL Layout (vec3<f32> is 16-byte aligned):
///   offset 0:  position (vec3<f32>) = 12 bytes
///   offset 12: _pad_after_pos       = 4 bytes (to align entity_type to offset 16)
///   offset 16: entity_type (u32)    = 4 bytes
///   offset 20: _pad_before_scale    = 12 bytes (to align scale vec3 to offset 32)
///   offset 32: scale (vec3<f32>)    = 12 bytes
///   offset 44: color_packed (u32)   = 4 bytes
///   Total: 48 bytes
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PlacedEntity {
    /// World position of the entity (12 bytes)
    pub position: [f32; 3],
    /// Padding to align entity_type to offset 16 (4 bytes)
    pub _pad_after_pos: u32,
    /// Entity type: 0=Sphere, 1=Box, 2=Capsule, 3=Torus, 4=Cylinder (4 bytes)
    pub entity_type: u32,
    /// Padding to align scale vec3 to offset 32 (12 bytes)
    pub _pad_before_scale: [u32; 3],
    /// Scale in each axis (12 bytes)
    pub scale: [f32; 3],
    /// Packed RGB color as u32: (R << 16) | (G << 8) | B (4 bytes)
    pub color_packed: u32,
    // Total: 48 bytes - matches WGSL!
}

/// Entity buffer header + entities array - must match WGSL EntityBuffer struct
/// Contains a count and up to 64 placed entities for GPU rendering.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct EntityBufferData {
    /// Number of active entities in the buffer
    pub count: u32,
    /// Padding for 16-byte alignment
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    /// Array of placed entities (max 64)
    pub entities: [PlacedEntity; 64],
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

/// Pack RGB color components into a single u32 value
/// Format: 0x00RRGGBB
#[inline]
pub fn pack_color(r: u8, g: u8, b: u8) -> u32 {
    ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Unpack a u32 color value into RGB components
#[inline]
pub fn unpack_color(packed: u32) -> (u8, u8, u8) {
    let r = ((packed >> 16) & 0xFF) as u8;
    let g = ((packed >> 8) & 0xFF) as u8;
    let b = (packed & 0xFF) as u8;
    (r, g, b)
}

// Compile-time assertion to verify struct sizes match WGSL layout
const _: () = {
    assert!(
        std::mem::size_of::<PlacedEntity>() == 48,
        "PlacedEntity must be 48 bytes to match WGSL"
    );
    assert!(
        std::mem::size_of::<EntityBufferData>() == 16 + 48 * 64,
        "EntityBufferData size mismatch"
    );
};

/// Predefined colors for placed objects
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

/// Entity type constants matching shader definitions
pub mod entity_type {
    pub const SPHERE: u32 = 0;
    pub const BOX: u32 = 1;
    pub const CAPSULE: u32 = 2;
    pub const TORUS: u32 = 3;
    pub const CYLINDER: u32 = 4;
}

// ============================================================================
// GPU ENTITY (96 bytes) - For raymarcher.wgsl
// ============================================================================

/// GPU-compatible entity struct for the raymarcher shader.
/// This is a more advanced entity representation than PlacedEntity, supporting:
/// - Quaternion rotation
/// - Per-entity noise displacement
/// - LOD control
/// - Baked SDF references with smooth transition
///
/// WGSL Layout (112 bytes total, 7 rows of 16 bytes each):
///   Row 0 (offset 0-15):  position_x, position_y, position_z, sdf_type
///   Row 1 (offset 16-31): scale_x, scale_y, scale_z, seed
///   Row 2 (offset 32-47): rotation (vec4 quaternion)
///   Row 3 (offset 48-63): color_r, color_g, color_b, roughness
///   Row 4 (offset 64-79): metallic, selected, lod_octaves, use_noise
///   Row 5 (offset 80-95): noise_amplitude, noise_frequency, noise_octaves, baked_sdf_id
///   Row 6 (offset 96-111): bake_blend, precision_class, _pad6_1, _pad6_2
///
/// Byte offset summary:
///   offset  0: position_x (f32)
///   offset  4: position_y (f32)
///   offset  8: position_z (f32)
///   offset 12: sdf_type (u32)
///   offset 16: scale_x (f32)
///   offset 20: scale_y (f32)
///   offset 24: scale_z (f32)
///   offset 28: seed (f32)
///   offset 32: rotation.x (f32)
///   offset 36: rotation.y (f32)
///   offset 40: rotation.z (f32)
///   offset 44: rotation.w (f32)
///   offset 48: color_r (f32)
///   offset 52: color_g (f32)
///   offset 56: color_b (f32)
///   offset 60: roughness (f32)
///   offset 64: metallic (f32)
///   offset 68: selected (f32)
///   offset 72: lod_octaves (u32)
///   offset 76: use_noise (u32)
///   offset 80: noise_amplitude (f32)
///   offset 84: noise_frequency (f32)
///   offset 88: noise_octaves (u32)
///   offset 92: baked_sdf_id (u32) - Baked SDF slot ID (0 = not baked, use equation)
///   offset 96: bake_blend (f32) - Blend factor 0.0=equation, 1.0=baked (smooth transition)
///   offset 100: precision_class (u32) - Precision class (US-031): 0=Player, 1=Interactive, 2=Static, 3=Terrain
///   offset 104: _pad6_1 (u32) - Padding
///   offset 108: _pad6_2 (u32) - Padding
///   TOTAL: 112 bytes
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuEntity {
    // Row 0: position (3 floats) + sdf_type = 16 bytes
    pub position_x: f32,
    pub position_y: f32,
    pub position_z: f32,
    pub sdf_type: u32,

    // Row 1: scale (3 floats) + seed = 16 bytes
    pub scale_x: f32,
    pub scale_y: f32,
    pub scale_z: f32,
    pub seed: f32,

    // Row 2: rotation quaternion = 16 bytes
    pub rotation_x: f32,
    pub rotation_y: f32,
    pub rotation_z: f32,
    pub rotation_w: f32,

    // Row 3: color (3 floats) + roughness = 16 bytes
    pub color_r: f32,
    pub color_g: f32,
    pub color_b: f32,
    pub roughness: f32,

    // Row 4: metallic + selected + lod_octaves + use_noise = 16 bytes
    pub metallic: f32,
    pub selected: f32,
    pub lod_octaves: u32,
    pub use_noise: u32,

    // Row 5: noise params + baked_sdf_id = 16 bytes
    pub noise_amplitude: f32,
    pub noise_frequency: f32,
    pub noise_octaves: u32,
    /// Baked SDF slot ID. 0 = not baked (use equation evaluation), 1-255 = baked SDF slot
    pub baked_sdf_id: u32,

    // Row 6: bake transition + precision + padding = 16 bytes (US-023, US-031)
    /// Blend factor for smooth transition from equation to baked SDF.
    /// 0.0 = use equation only, 1.0 = use baked only, 0.0-1.0 = blend both
    pub bake_blend: f32,
    /// Precision class for raymarching step size control (US-031)
    /// 0=Player(1.0x), 1=Interactive(1.2x), 2=Static(1.5x), 3=Terrain(2.0x)
    pub precision_class: u32,
    /// Padding for 16-byte row alignment
    pub _pad6_1: u32,
    pub _pad6_2: u32,
}

impl Default for GpuEntity {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            position_z: 0.0,
            sdf_type: entity_type::SPHERE,
            scale_x: 1.0,
            scale_y: 1.0,
            scale_z: 1.0,
            seed: 0.0,
            rotation_x: 0.0,
            rotation_y: 0.0,
            rotation_z: 0.0,
            rotation_w: 1.0, // Identity quaternion
            color_r: 1.0,
            color_g: 0.5,
            color_b: 0.2,
            roughness: 0.5,
            metallic: 0.0,
            selected: 0.0,
            lod_octaves: 8, // Full detail by default
            use_noise: 0,   // No noise displacement by default
            noise_amplitude: 0.0,
            noise_frequency: 1.0,
            noise_octaves: 4,
            baked_sdf_id: 0, // Not baked by default (use equation evaluation)
            bake_blend: 0.0, // Start with equation rendering (US-023)
            precision_class: PrecisionClass::Static as u32, // Default precision (US-031)
            _pad6_1: 0,
            _pad6_2: 0,
        }
    }
}

impl GpuEntity {
    /// Create a new GpuEntity at the given position.
    pub fn new(position: [f32; 3], sdf_type: u32) -> Self {
        Self {
            position_x: position[0],
            position_y: position[1],
            position_z: position[2],
            sdf_type,
            ..Default::default()
        }
    }

    /// Set the position from an array.
    pub fn with_position(mut self, position: [f32; 3]) -> Self {
        self.position_x = position[0];
        self.position_y = position[1];
        self.position_z = position[2];
        self
    }

    /// Set uniform scale.
    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale_x = scale;
        self.scale_y = scale;
        self.scale_z = scale;
        self
    }

    /// Set non-uniform scale.
    pub fn with_scale_xyz(mut self, scale: [f32; 3]) -> Self {
        self.scale_x = scale[0];
        self.scale_y = scale[1];
        self.scale_z = scale[2];
        self
    }

    /// Set rotation from quaternion components.
    pub fn with_rotation(mut self, x: f32, y: f32, z: f32, w: f32) -> Self {
        self.rotation_x = x;
        self.rotation_y = y;
        self.rotation_z = z;
        self.rotation_w = w;
        self
    }

    /// Set color from RGB floats (0.0-1.0 range).
    pub fn with_color(mut self, r: f32, g: f32, b: f32) -> Self {
        self.color_r = r;
        self.color_g = g;
        self.color_b = b;
        self
    }

    /// Set the baked SDF slot ID.
    /// Use 0 for equation-based evaluation, 1-255 for baked SDF slots.
    pub fn with_baked_sdf_id(mut self, id: u32) -> Self {
        self.baked_sdf_id = id;
        self
    }

    /// Enable noise displacement.
    pub fn with_noise(mut self, amplitude: f32, frequency: f32, octaves: u32) -> Self {
        self.use_noise = 1;
        self.noise_amplitude = amplitude;
        self.noise_frequency = frequency;
        self.noise_octaves = octaves;
        self
    }

    /// Mark this entity as selected (for visual highlighting).
    pub fn with_selected(mut self, selected: bool) -> Self {
        self.selected = if selected { 1.0 } else { 0.0 };
        self
    }

    /// Set the bake blend factor for smooth transition (US-023).
    /// 0.0 = use equation only, 1.0 = use baked only, 0.0-1.0 = blend both
    pub fn with_bake_blend(mut self, blend: f32) -> Self {
        self.bake_blend = blend.clamp(0.0, 1.0);
        self
    }

    /// Set the precision class for raymarching step control (US-031).
    /// Controls how precisely this entity is rendered during raymarching.
    pub fn with_precision_class(mut self, class: PrecisionClass) -> Self {
        self.precision_class = class.to_u32();
        self
    }

    /// Get the precision class for this entity.
    pub fn get_precision_class(&self) -> PrecisionClass {
        PrecisionClass::from_u32(self.precision_class)
    }

    /// Get the step size multiplier for this entity's precision class.
    pub fn precision_multiplier(&self) -> f32 {
        self.get_precision_class().multiplier()
    }
}

/// GPU entity buffer for the raymarcher shader.
/// Contains a count header and up to 1024 GpuEntity structs.
///
/// Buffer layout:
/// - count: 4 bytes (u32)
/// - padding: 12 bytes (3 × u32 for 16-byte alignment)
/// - entities: 1024 × 112 = 114,688 bytes
/// Total: 16 + 114,688 = 114,704 bytes (~112 KB)
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuEntityBuffer {
    /// Number of active entities in the buffer (0-1024)
    pub count: u32,
    /// Padding for 16-byte alignment
    pub _pad0: u32,
    pub _pad1: u32,
    pub _pad2: u32,
    /// Array of GPU entities (max 1024)
    pub entities: [GpuEntity; 1024],
}

impl Default for GpuEntityBuffer {
    fn default() -> Self {
        Self {
            count: 0,
            _pad0: 0,
            _pad1: 0,
            _pad2: 0,
            entities: [GpuEntity::default(); 1024],
        }
    }
}

// Compile-time assertions for GpuEntity struct sizes
const _: () = {
    assert!(
        std::mem::size_of::<GpuEntity>() == 112,
        "GpuEntity must be 112 bytes to match WGSL struct"
    );
    assert!(
        std::mem::size_of::<GpuEntityBuffer>() == 16 + 112 * 1024,
        "GpuEntityBuffer size mismatch"
    );
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placed_entity_size() {
        assert_eq!(std::mem::size_of::<PlacedEntity>(), 48);
    }

    #[test]
    fn test_entity_buffer_data_size() {
        assert_eq!(std::mem::size_of::<EntityBufferData>(), 16 + 48 * 64);
    }

    #[test]
    fn test_pack_unpack_color() {
        let (r, g, b) = (255, 128, 64);
        let packed = pack_color(r, g, b);
        let (ur, ug, ub) = unpack_color(packed);
        assert_eq!((ur, ug, ub), (r, g, b));
    }

    #[test]
    fn test_default_entity() {
        let entity = PlacedEntity::default();
        assert_eq!(entity.position, [0.0, 0.0, 0.0]);
        assert_eq!(entity.entity_type, 0);
        assert_eq!(entity.scale, [0.5, 0.5, 0.5]);
    }

    // GpuEntity tests (112-byte struct for raymarcher.wgsl with bake_blend)

    #[test]
    fn test_gpu_entity_size() {
        assert_eq!(std::mem::size_of::<GpuEntity>(), 112);
    }

    #[test]
    fn test_gpu_entity_buffer_size() {
        assert_eq!(std::mem::size_of::<GpuEntityBuffer>(), 16 + 112 * 1024);
    }

    #[test]
    fn test_gpu_entity_default() {
        let entity = GpuEntity::default();
        assert_eq!(entity.position_x, 0.0);
        assert_eq!(entity.position_y, 0.0);
        assert_eq!(entity.position_z, 0.0);
        assert_eq!(entity.sdf_type, entity_type::SPHERE);
        assert_eq!(entity.scale_x, 1.0);
        assert_eq!(entity.scale_y, 1.0);
        assert_eq!(entity.scale_z, 1.0);
        // Default rotation is identity quaternion (0, 0, 0, 1)
        assert_eq!(entity.rotation_x, 0.0);
        assert_eq!(entity.rotation_y, 0.0);
        assert_eq!(entity.rotation_z, 0.0);
        assert_eq!(entity.rotation_w, 1.0);
        // Default is not baked (use equation evaluation)
        assert_eq!(entity.baked_sdf_id, 0);
        // Default bake blend is 0.0 (use equation only)
        assert_eq!(entity.bake_blend, 0.0);
    }

    #[test]
    fn test_gpu_entity_with_baked_sdf() {
        let entity = GpuEntity::new([1.0, 2.0, 3.0], entity_type::BOX)
            .with_baked_sdf_id(42);

        assert_eq!(entity.position_x, 1.0);
        assert_eq!(entity.position_y, 2.0);
        assert_eq!(entity.position_z, 3.0);
        assert_eq!(entity.sdf_type, entity_type::BOX);
        assert_eq!(entity.baked_sdf_id, 42);
    }

    #[test]
    fn test_gpu_entity_builder_pattern() {
        let entity = GpuEntity::default()
            .with_position([10.0, 20.0, 30.0])
            .with_scale(2.5)
            .with_color(1.0, 0.5, 0.25)
            .with_baked_sdf_id(100)
            .with_noise(0.1, 2.0, 4)
            .with_selected(true);

        assert_eq!(entity.position_x, 10.0);
        assert_eq!(entity.position_y, 20.0);
        assert_eq!(entity.position_z, 30.0);
        assert_eq!(entity.scale_x, 2.5);
        assert_eq!(entity.scale_y, 2.5);
        assert_eq!(entity.scale_z, 2.5);
        assert_eq!(entity.color_r, 1.0);
        assert_eq!(entity.color_g, 0.5);
        assert_eq!(entity.color_b, 0.25);
        assert_eq!(entity.baked_sdf_id, 100);
        assert_eq!(entity.use_noise, 1);
        assert_eq!(entity.noise_amplitude, 0.1);
        assert_eq!(entity.noise_frequency, 2.0);
        assert_eq!(entity.noise_octaves, 4);
        assert_eq!(entity.selected, 1.0);
    }

    #[test]
    fn test_gpu_entity_bake_blend() {
        // Test bake_blend builder method (US-023)
        let entity = GpuEntity::default()
            .with_baked_sdf_id(5)
            .with_bake_blend(0.5);

        assert_eq!(entity.baked_sdf_id, 5);
        assert_eq!(entity.bake_blend, 0.5);

        // Test clamping
        let entity_clamped = GpuEntity::default().with_bake_blend(2.0);
        assert_eq!(entity_clamped.bake_blend, 1.0);

        let entity_negative = GpuEntity::default().with_bake_blend(-0.5);
        assert_eq!(entity_negative.bake_blend, 0.0);
    }

    // PrecisionClass tests (US-031)

    #[test]
    fn test_precision_class_multipliers() {
        assert_eq!(PrecisionClass::Player.multiplier(), 1.0);
        assert_eq!(PrecisionClass::Interactive.multiplier(), 1.2);
        assert_eq!(PrecisionClass::Static.multiplier(), 1.5);
        assert_eq!(PrecisionClass::Terrain.multiplier(), 2.0);
    }

    #[test]
    fn test_precision_class_default() {
        let default: PrecisionClass = Default::default();
        assert_eq!(default, PrecisionClass::Static);
    }

    #[test]
    fn test_precision_class_u32_conversion() {
        // Test to_u32
        assert_eq!(PrecisionClass::Player.to_u32(), 0);
        assert_eq!(PrecisionClass::Interactive.to_u32(), 1);
        assert_eq!(PrecisionClass::Static.to_u32(), 2);
        assert_eq!(PrecisionClass::Terrain.to_u32(), 3);

        // Test from_u32
        assert_eq!(PrecisionClass::from_u32(0), PrecisionClass::Player);
        assert_eq!(PrecisionClass::from_u32(1), PrecisionClass::Interactive);
        assert_eq!(PrecisionClass::from_u32(2), PrecisionClass::Static);
        assert_eq!(PrecisionClass::from_u32(3), PrecisionClass::Terrain);

        // Test invalid value defaults to Static
        assert_eq!(PrecisionClass::from_u32(99), PrecisionClass::Static);
    }

    #[test]
    fn test_gpu_entity_precision_class_default() {
        let entity = GpuEntity::default();
        // Default precision_class should be Static (value 2)
        assert_eq!(entity.precision_class, PrecisionClass::Static as u32);
        assert_eq!(entity.get_precision_class(), PrecisionClass::Static);
        assert_eq!(entity.precision_multiplier(), 1.5);
    }

    #[test]
    fn test_gpu_entity_with_precision_class() {
        let entity = GpuEntity::default()
            .with_precision_class(PrecisionClass::Player);

        assert_eq!(entity.precision_class, 0);
        assert_eq!(entity.get_precision_class(), PrecisionClass::Player);
        assert_eq!(entity.precision_multiplier(), 1.0);

        let entity_terrain = GpuEntity::default()
            .with_precision_class(PrecisionClass::Terrain);

        assert_eq!(entity_terrain.precision_class, 3);
        assert_eq!(entity_terrain.get_precision_class(), PrecisionClass::Terrain);
        assert_eq!(entity_terrain.precision_multiplier(), 2.0);
    }
}
