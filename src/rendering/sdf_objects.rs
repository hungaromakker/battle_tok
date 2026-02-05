//! SDF Object Framework
//!
//! Provides types for defining Signed Distance Field (SDF) objects
//! that can be rendered via ray marching. SDF objects are defined by
//! mathematical functions rather than meshes, enabling infinite detail
//! and smooth curves perfect for siege weapons like cannons.
//!
//! # Example
//!
//! ```ignore
//! use battle_tok_engine::rendering::sdf_objects::{SdfPrimitive, SdfOperation, SdfObject};
//! use glam::{Mat4, Vec3};
//!
//! // Create a cannon barrel (cylinder with rounded cap)
//! let barrel = SdfPrimitive::Cylinder { radius: 0.3, height: 4.0 };
//! let body = SdfPrimitive::RoundedBox {
//!     half_extents: Vec3::new(1.0, 0.5, 0.75),
//!     radius: 0.1,
//! };
//!
//! let mut cannon = SdfObject::new();
//! cannon.add_primitive(barrel, Mat4::from_translation(Vec3::new(0.0, 0.5, 2.0)));
//! cannon.add_primitive(body, Mat4::IDENTITY);
//! cannon.add_operation(SdfOperation::SmoothUnion { k: 0.3 });
//! ```

use glam::{Mat4, Vec3};

/// SDF primitive shapes that can be combined to create complex objects.
///
/// Each primitive defines a signed distance function where:
/// - Positive distance = outside the shape
/// - Zero = on the surface
/// - Negative distance = inside the shape
#[derive(Clone, Debug, PartialEq)]
pub enum SdfPrimitive {
    /// Sphere centered at origin
    Sphere {
        /// Radius of the sphere
        radius: f32,
    },

    /// Cylinder aligned along Y-axis, centered at origin
    Cylinder {
        /// Radius of the cylinder
        radius: f32,
        /// Total height of the cylinder (extends height/2 above and below origin)
        height: f32,
    },

    /// Axis-aligned box centered at origin
    Box {
        /// Half-extents in each axis (total size = 2 * half_extents)
        half_extents: Vec3,
    },

    /// Box with rounded edges centered at origin
    RoundedBox {
        /// Half-extents before rounding (inner box size)
        half_extents: Vec3,
        /// Rounding radius for edges and corners
        radius: f32,
    },

    /// Capsule (line segment with rounded ends) aligned along Y-axis
    Capsule {
        /// Radius of the capsule (cylinder and hemisphere radius)
        radius: f32,
        /// Height of the cylindrical portion (total height = height + 2*radius)
        height: f32,
    },
}

impl SdfPrimitive {
    /// Creates a new sphere primitive.
    pub fn sphere(radius: f32) -> Self {
        Self::Sphere { radius }
    }

    /// Creates a new cylinder primitive.
    pub fn cylinder(radius: f32, height: f32) -> Self {
        Self::Cylinder { radius, height }
    }

    /// Creates a new box primitive.
    pub fn box_shape(half_extents: Vec3) -> Self {
        Self::Box { half_extents }
    }

    /// Creates a new rounded box primitive.
    pub fn rounded_box(half_extents: Vec3, radius: f32) -> Self {
        Self::RoundedBox {
            half_extents,
            radius,
        }
    }

    /// Creates a new capsule primitive.
    pub fn capsule(radius: f32, height: f32) -> Self {
        Self::Capsule { radius, height }
    }
}

/// Boolean and smooth operations for combining SDF primitives.
///
/// These operations correspond to standard CSG (Constructive Solid Geometry)
/// operations but can also include smooth blending versions.
#[derive(Clone, Debug, PartialEq)]
pub enum SdfOperation {
    /// Standard union - takes minimum distance (combines shapes)
    Union,

    /// Standard intersection - takes maximum distance (keeps overlap)
    Intersection,

    /// Standard subtraction - subtracts second from first
    Subtraction,

    /// Smooth union - blends shapes together with smooth transition
    SmoothUnion {
        /// Smoothing factor (higher = more blending, typical range 0.1-0.5)
        k: f32,
    },

    /// Smooth subtraction - carves with smooth edges
    SmoothSubtraction {
        /// Smoothing factor (higher = more blending)
        k: f32,
    },
}

impl SdfOperation {
    /// Creates a smooth union operation with the given blending factor.
    pub fn smooth_union(k: f32) -> Self {
        Self::SmoothUnion { k }
    }

    /// Creates a smooth subtraction operation with the given blending factor.
    pub fn smooth_subtraction(k: f32) -> Self {
        Self::SmoothSubtraction { k }
    }
}

/// A positioned primitive within an SDF object.
///
/// Each primitive has a local transform that positions it relative to
/// the object's origin.
#[derive(Clone, Debug)]
pub struct PositionedPrimitive {
    /// The primitive shape
    pub primitive: SdfPrimitive,
    /// Local transform (relative to object origin)
    pub transform: Mat4,
}

/// A composite SDF object made from multiple primitives combined with operations.
///
/// SDF objects define complex shapes through composition:
/// 1. Add primitives with local transforms
/// 2. Add operations that describe how primitives combine
/// 3. Set an overall transform for the entire object
///
/// Operations are applied in order, combining primitives pairwise.
/// For N primitives, you typically need N-1 operations.
#[derive(Clone, Debug)]
pub struct SdfObject {
    /// Primitives that make up this object (with local transforms)
    pub primitives: Vec<PositionedPrimitive>,
    /// Operations describing how primitives combine
    pub operations: Vec<SdfOperation>,
    /// World transform for the entire object
    pub transform: Mat4,
}

impl Default for SdfObject {
    fn default() -> Self {
        Self::new()
    }
}

impl SdfObject {
    /// Creates a new empty SDF object.
    pub fn new() -> Self {
        Self {
            primitives: Vec::new(),
            operations: Vec::new(),
            transform: Mat4::IDENTITY,
        }
    }

    /// Adds a primitive with its local transform.
    pub fn add_primitive(&mut self, primitive: SdfPrimitive, transform: Mat4) {
        self.primitives.push(PositionedPrimitive {
            primitive,
            transform,
        });
    }

    /// Adds a primitive positioned at the given translation (no rotation/scale).
    pub fn add_primitive_at(&mut self, primitive: SdfPrimitive, position: Vec3) {
        self.add_primitive(primitive, Mat4::from_translation(position));
    }

    /// Adds an operation to combine primitives.
    pub fn add_operation(&mut self, operation: SdfOperation) {
        self.operations.push(operation);
    }

    /// Sets the world transform for the entire object.
    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
    }

    /// Sets the object's world position.
    pub fn set_position(&mut self, position: Vec3) {
        // Preserve rotation and scale, update translation
        let (scale, rotation, _) = self.transform.to_scale_rotation_translation();
        self.transform = Mat4::from_scale_rotation_translation(scale, rotation, position);
    }

    // Builder-style methods that return Self for method chaining

    /// Builder method: adds a primitive with its local transform.
    pub fn with_primitive(mut self, primitive: SdfPrimitive, transform: Mat4) -> Self {
        self.add_primitive(primitive, transform);
        self
    }

    /// Builder method: adds a primitive positioned at the given translation.
    pub fn with_primitive_at(mut self, primitive: SdfPrimitive, position: Vec3) -> Self {
        self.add_primitive_at(primitive, position);
        self
    }

    /// Builder method: adds an operation to combine primitives.
    pub fn with_operation(mut self, operation: SdfOperation) -> Self {
        self.add_operation(operation);
        self
    }

    /// Builder method: sets the world transform for the entire object.
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.set_transform(transform);
        self
    }

    /// Builder method: sets the object's world position.
    pub fn with_position(mut self, position: Vec3) -> Self {
        self.set_position(position);
        self
    }

    /// Returns the number of primitives in this object.
    pub fn primitive_count(&self) -> usize {
        self.primitives.len()
    }

    /// Returns the number of operations in this object.
    pub fn operation_count(&self) -> usize {
        self.operations.len()
    }

    /// Validates that the object has the correct number of operations.
    ///
    /// For N primitives, there should be N-1 operations to combine them.
    /// Returns true if valid, false otherwise.
    pub fn is_valid(&self) -> bool {
        if self.primitives.is_empty() {
            return true; // Empty object is valid
        }
        // N primitives need N-1 operations
        self.operations.len() >= self.primitives.len().saturating_sub(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_primitive() {
        let sphere = SdfPrimitive::sphere(1.0);
        match sphere {
            SdfPrimitive::Sphere { radius } => assert_eq!(radius, 1.0),
            _ => panic!("Expected Sphere"),
        }
    }

    #[test]
    fn test_cylinder_primitive() {
        let cylinder = SdfPrimitive::cylinder(0.5, 2.0);
        match cylinder {
            SdfPrimitive::Cylinder { radius, height } => {
                assert_eq!(radius, 0.5);
                assert_eq!(height, 2.0);
            }
            _ => panic!("Expected Cylinder"),
        }
    }

    #[test]
    fn test_rounded_box_primitive() {
        let rbox = SdfPrimitive::rounded_box(Vec3::new(1.0, 0.5, 0.75), 0.1);
        match rbox {
            SdfPrimitive::RoundedBox {
                half_extents,
                radius,
            } => {
                assert_eq!(half_extents, Vec3::new(1.0, 0.5, 0.75));
                assert_eq!(radius, 0.1);
            }
            _ => panic!("Expected RoundedBox"),
        }
    }

    #[test]
    fn test_smooth_union_operation() {
        let op = SdfOperation::smooth_union(0.3);
        match op {
            SdfOperation::SmoothUnion { k } => assert_eq!(k, 0.3),
            _ => panic!("Expected SmoothUnion"),
        }
    }

    #[test]
    fn test_sdf_object_creation() {
        let obj = SdfObject::new();
        assert!(obj.primitives.is_empty());
        assert!(obj.operations.is_empty());
        assert!(obj.is_valid());
    }

    #[test]
    fn test_sdf_object_add_primitives() {
        let mut obj = SdfObject::new();
        obj.add_primitive(SdfPrimitive::sphere(1.0), Mat4::IDENTITY);
        obj.add_primitive_at(SdfPrimitive::cylinder(0.5, 2.0), Vec3::new(0.0, 1.0, 0.0));

        assert_eq!(obj.primitive_count(), 2);
    }

    #[test]
    fn test_sdf_object_validity() {
        let mut obj = SdfObject::new();
        obj.add_primitive(SdfPrimitive::sphere(1.0), Mat4::IDENTITY);
        obj.add_primitive(SdfPrimitive::sphere(0.5), Mat4::IDENTITY);

        // 2 primitives need 1 operation
        assert!(!obj.is_valid());

        obj.add_operation(SdfOperation::SmoothUnion { k: 0.3 });
        assert!(obj.is_valid());
    }

    #[test]
    fn test_cannon_composition() {
        // Test creating a cannon-like object
        let mut cannon = SdfObject::new();

        // Barrel: cylinder radius 0.3, length 4.0
        cannon.add_primitive_at(
            SdfPrimitive::cylinder(0.3, 4.0),
            Vec3::new(0.0, 0.5, 2.0), // Positioned forward
        );

        // Body: rounded box 2.0 x 1.0 x 1.5
        cannon.add_primitive(
            SdfPrimitive::rounded_box(Vec3::new(1.0, 0.5, 0.75), 0.1),
            Mat4::IDENTITY,
        );

        // Smooth union
        cannon.add_operation(SdfOperation::smooth_union(0.3));

        assert_eq!(cannon.primitive_count(), 2);
        assert_eq!(cannon.operation_count(), 1);
        assert!(cannon.is_valid());
    }
}
