//! Building Materials
//!
//! Different materials have different properties affecting building:
//! - Cost (resources needed)
//! - Strength (structural support capacity)
//! - Build time (how fast to place)
//! - Visual appearance
//! - Physics properties (friction, break threshold, density)

use glam::Vec3;

/// Physics properties for materials
/// Used by the block physics system for realistic movement and destruction
#[derive(Debug, Clone, Copy)]
pub struct MaterialPhysics {
    /// Static friction coefficient (0.0-1.0) - resistance to start moving
    pub friction_static: f32,
    /// Dynamic friction coefficient (0.0-1.0) - resistance while moving
    pub friction_dynamic: f32,
    /// Force threshold (Newtons) to disintegrate the block
    pub break_threshold: f32,
    /// Density in kg/m³ for mass calculation
    pub density: f32,
    /// Restitution/bounce coefficient (0.0 = no bounce, 1.0 = perfect bounce)
    pub restitution: f32,
}

impl MaterialPhysics {
    /// Calculate mass from volume (in m³)
    pub fn mass_from_volume(&self, volume: f32) -> f32 {
        self.density * volume
    }
}

/// Physics properties for each material type (indexed by material u8)
/// Indices: 0=Stone Gray, 1=Wood Brown, 2=Stone Dark, 3=Sandstone, 4=Slate,
///          5=Brick Red, 6=Moss Green, 7=Metal Gray, 8=Marble White, 9=Obsidian
pub const MATERIAL_PHYSICS: &[MaterialPhysics] = &[
    // 0: Stone Gray
    MaterialPhysics {
        friction_static: 0.7,
        friction_dynamic: 0.5,
        break_threshold: 5000.0,
        density: 2500.0,
        restitution: 0.2,
    },
    // 1: Wood Brown
    MaterialPhysics {
        friction_static: 0.5,
        friction_dynamic: 0.4,
        break_threshold: 1500.0,
        density: 600.0,
        restitution: 0.3,
    },
    // 2: Stone Dark
    MaterialPhysics {
        friction_static: 0.75,
        friction_dynamic: 0.55,
        break_threshold: 6000.0,
        density: 2700.0,
        restitution: 0.15,
    },
    // 3: Sandstone
    MaterialPhysics {
        friction_static: 0.6,
        friction_dynamic: 0.45,
        break_threshold: 2500.0,
        density: 2200.0,
        restitution: 0.2,
    },
    // 4: Slate
    MaterialPhysics {
        friction_static: 0.55,
        friction_dynamic: 0.4,
        break_threshold: 3500.0,
        density: 2800.0,
        restitution: 0.15,
    },
    // 5: Brick Red
    MaterialPhysics {
        friction_static: 0.65,
        friction_dynamic: 0.5,
        break_threshold: 3000.0,
        density: 1900.0,
        restitution: 0.25,
    },
    // 6: Moss Green (organic, soft)
    MaterialPhysics {
        friction_static: 0.8,
        friction_dynamic: 0.6,
        break_threshold: 800.0,
        density: 500.0,
        restitution: 0.4,
    },
    // 7: Metal Gray
    MaterialPhysics {
        friction_static: 0.3,
        friction_dynamic: 0.2,
        break_threshold: 10000.0,
        density: 7800.0,
        restitution: 0.5,
    },
    // 8: Marble White
    MaterialPhysics {
        friction_static: 0.4,
        friction_dynamic: 0.3,
        break_threshold: 4000.0,
        density: 2700.0,
        restitution: 0.25,
    },
    // 9: Obsidian Black (volcanic glass - hard but brittle)
    MaterialPhysics {
        friction_static: 0.35,
        friction_dynamic: 0.25,
        break_threshold: 2000.0, // Brittle despite being hard
        density: 2400.0,
        restitution: 0.1,
    },
];

/// Get physics properties for a material index
/// Returns stone physics as default for invalid indices
pub fn get_material_physics(material_index: u8) -> &'static MaterialPhysics {
    MATERIAL_PHYSICS
        .get(material_index as usize)
        .unwrap_or(&MATERIAL_PHYSICS[0])
}

/// Material types for building
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Material {
    /// Fast to build, burns, weak
    Wood,
    /// Strong, slow to build, main castle material
    Stone,
    /// Very strong, expensive, for gates and reinforcement
    Iron,
    /// Cheap, weak, for roofs and temporary structures
    Thatch,
    /// Binds stone, increases structural integrity
    Mortar,
    /// Dirt/earth for foundations
    Earth,
}

/// Properties of a material
#[derive(Debug, Clone, Copy)]
pub struct MaterialProperties {
    /// Material identifier
    pub material: Material,
    /// Display name
    pub name: &'static str,
    /// Base color (RGB 0-1)
    pub color: Vec3,
    /// Strength multiplier (1.0 = baseline stone)
    pub strength: f32,
    /// Gold cost per 1 dm³ block
    pub gold_cost: u32,
    /// Stone cost per block (only stone material uses this)
    pub stone_cost: u32,
    /// Wood cost per block
    pub wood_cost: u32,
    /// Build speed multiplier (1.0 = normal)
    pub build_speed: f32,
    /// Can this material burn?
    pub flammable: bool,
    /// Weight in kg per dm³
    pub weight: f32,
    /// Maximum cantilever distance (in blocks) before needing support
    pub cantilever_limit: u32,
}

impl MaterialProperties {
    /// Get the primary color with slight variation for visual interest
    pub fn varied_color(&self, seed: u32) -> Vec3 {
        let variation = 0.05;
        let hash = ((seed * 2654435761) % 1000) as f32 / 1000.0 - 0.5;
        Vec3::new(
            (self.color.x + hash * variation).clamp(0.0, 1.0),
            (self.color.y + hash * variation).clamp(0.0, 1.0),
            (self.color.z + hash * variation).clamp(0.0, 1.0),
        )
    }
}

/// Static material property definitions
pub const MATERIALS: &[MaterialProperties] = &[
    MaterialProperties {
        material: Material::Wood,
        name: "Wood",
        color: Vec3::new(0.55, 0.35, 0.15), // Brown
        strength: 0.5,
        gold_cost: 1,
        stone_cost: 0,
        wood_cost: 1,
        build_speed: 2.0, // Twice as fast
        flammable: true,
        weight: 0.6, // Light
        cantilever_limit: 2,
    },
    MaterialProperties {
        material: Material::Stone,
        name: "Stone",
        color: Vec3::new(0.5, 0.5, 0.5), // Gray
        strength: 1.0,
        gold_cost: 2,
        stone_cost: 1,
        wood_cost: 0,
        build_speed: 1.0,
        flammable: false,
        weight: 2.4, // Heavy
        cantilever_limit: 1,
    },
    MaterialProperties {
        material: Material::Iron,
        name: "Iron",
        color: Vec3::new(0.3, 0.3, 0.35), // Dark gray-blue
        strength: 2.0,
        gold_cost: 5,
        stone_cost: 0,
        wood_cost: 0,
        build_speed: 0.5, // Slow
        flammable: false,
        weight: 7.8, // Very heavy
        cantilever_limit: 3,
    },
    MaterialProperties {
        material: Material::Thatch,
        name: "Thatch",
        color: Vec3::new(0.7, 0.65, 0.3), // Yellow-brown
        strength: 0.2,
        gold_cost: 0,
        stone_cost: 0,
        wood_cost: 1,
        build_speed: 3.0, // Very fast
        flammable: true,
        weight: 0.2, // Very light
        cantilever_limit: 1,
    },
    MaterialProperties {
        material: Material::Mortar,
        name: "Mortar",
        color: Vec3::new(0.85, 0.82, 0.75), // Off-white
        strength: 0.8,
        gold_cost: 1,
        stone_cost: 0,
        wood_cost: 0,
        build_speed: 0.8,
        flammable: false,
        weight: 1.8,
        cantilever_limit: 0, // Can't stand alone
    },
    MaterialProperties {
        material: Material::Earth,
        name: "Earth",
        color: Vec3::new(0.4, 0.3, 0.2), // Dark brown
        strength: 0.3,
        gold_cost: 0,
        stone_cost: 0,
        wood_cost: 0,
        build_speed: 1.5,
        flammable: false,
        weight: 1.5,
        cantilever_limit: 0, // Must be grounded
    },
];

impl Material {
    /// Get the properties for this material
    pub fn properties(&self) -> &'static MaterialProperties {
        MATERIALS.iter().find(|p| p.material == *self).unwrap()
    }

    /// Get material from index (for UI selection)
    pub fn from_index(index: usize) -> Option<Material> {
        match index {
            0 => Some(Material::Wood),
            1 => Some(Material::Stone),
            2 => Some(Material::Iron),
            3 => Some(Material::Thatch),
            4 => Some(Material::Mortar),
            5 => Some(Material::Earth),
            _ => None,
        }
    }

    /// Get index for this material
    pub fn to_index(&self) -> usize {
        match self {
            Material::Wood => 0,
            Material::Stone => 1,
            Material::Iron => 2,
            Material::Thatch => 3,
            Material::Mortar => 4,
            Material::Earth => 5,
        }
    }
}
