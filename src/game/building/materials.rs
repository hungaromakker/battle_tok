//! Building Materials
//!
//! Different materials have different properties affecting building:
//! - Cost (resources needed)
//! - Strength (structural support capacity)
//! - Build time (how fast to place)
//! - Visual appearance

use glam::Vec3;

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
