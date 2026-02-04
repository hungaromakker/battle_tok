//! CPU-Side Froxel Assignment Fallback (US-0M10)
//!
//! Provides a CPU fallback for assigning SDFs to froxels when compute shaders
//! are unavailable (e.g., software renderer). The logic matches the GPU compute
//! shader: for each froxel, test all SDF bounds against the froxel AABB using
//! sphere-AABB intersection, and build per-froxel SDF index lists.

use super::froxel_buffers::{FroxelBounds, FroxelBoundsBuffer, FroxelSDFListBuffer};
use super::froxel_assignment::SdfBounds;
use super::froxel_config::TOTAL_FROXELS;

/// Assign SDFs to froxels on the CPU using sphere-AABB intersection.
///
/// This is the software renderer fallback that matches the GPU compute shader logic.
/// For each froxel, tests all entity bounding spheres against the froxel AABB.
///
/// # Arguments
///
/// * `entities` - Slice of SDF bounds (world-space AABBs for each entity)
/// * `froxel_bounds` - Precomputed world-space AABBs for all froxels
///
/// # Returns
///
/// A `FroxelSDFListBuffer` containing per-froxel lists of intersecting SDF indices.
pub fn assign_sdfs_to_froxels(
    entities: &[SdfBounds],
    froxel_bounds: &FroxelBoundsBuffer,
) -> Box<FroxelSDFListBuffer> {
    let mut result = Box::new(FroxelSDFListBuffer::new());

    // Pre-compute bounding spheres for all entities (center + radius)
    let spheres: Vec<([f32; 3], f32)> = entities
        .iter()
        .map(|e| {
            let center = e.center();
            let size = e.size();
            // Bounding sphere radius = half-diagonal of the AABB
            let radius = (size[0] * size[0] + size[1] * size[1] + size[2] * size[2]).sqrt() * 0.5;
            (center, radius)
        })
        .collect();

    // For each froxel, test all entities
    for fi in 0..(TOTAL_FROXELS as usize) {
        let fb = &froxel_bounds.bounds[fi];
        let list = &mut result.lists[fi];

        for (ei, (center, radius)) in spheres.iter().enumerate() {
            if sphere_aabb_intersect(*center, *radius, fb) {
                if !list.add(ei as u32) {
                    break; // froxel list full
                }
            }
        }
    }

    result
}

/// Sphere-AABB intersection test matching GPU compute shader logic.
///
/// Tests whether a sphere (defined by center + radius) intersects an AABB.
/// Uses the classic closest-point-on-AABB approach: find the point on the AABB
/// closest to the sphere center, then check if the distance is within the radius.
#[inline]
fn sphere_aabb_intersect(center: [f32; 3], radius: f32, aabb: &FroxelBounds) -> bool {
    // Find the closest point on the AABB to the sphere center
    let cx = center[0].clamp(aabb.min_x, aabb.max_x);
    let cy = center[1].clamp(aabb.min_y, aabb.max_y);
    let cz = center[2].clamp(aabb.min_z, aabb.max_z);

    // Squared distance from sphere center to closest point
    let dx = center[0] - cx;
    let dy = center[1] - cy;
    let dz = center[2] - cz;
    let dist_sq = dx * dx + dy * dy + dz * dz;

    dist_sq <= radius * radius
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::froxel_buffers::FroxelBounds;

    #[test]
    fn test_sphere_aabb_intersect_inside() {
        let aabb = FroxelBounds::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        // Sphere center inside AABB
        assert!(sphere_aabb_intersect([5.0, 5.0, 5.0], 1.0, &aabb));
    }

    #[test]
    fn test_sphere_aabb_intersect_touching() {
        let aabb = FroxelBounds::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        // Sphere just touching the face
        assert!(sphere_aabb_intersect([11.0, 5.0, 5.0], 1.0, &aabb));
    }

    #[test]
    fn test_sphere_aabb_intersect_miss() {
        let aabb = FroxelBounds::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        // Sphere too far away
        assert!(!sphere_aabb_intersect([15.0, 5.0, 5.0], 1.0, &aabb));
    }

    #[test]
    fn test_sphere_aabb_intersect_corner() {
        let aabb = FroxelBounds::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        // Sphere near corner but not touching (distance to corner > radius)
        let d = (3.0_f32).sqrt(); // distance from (11,11,11) to corner (10,10,10) = sqrt(3) ≈ 1.732
        assert!(!sphere_aabb_intersect([11.0, 11.0, 11.0], d - 0.01, &aabb));
        assert!(sphere_aabb_intersect([11.0, 11.0, 11.0], d + 0.01, &aabb));
    }

    #[test]
    fn test_assign_sdfs_to_froxels_empty() {
        let froxel_bounds = FroxelBoundsBuffer::new();
        let result = assign_sdfs_to_froxels(&[], &froxel_bounds);
        // All lists should be empty
        for list in &result.lists {
            assert!(list.is_empty());
        }
    }

    #[test]
    fn test_assign_sdfs_to_froxels_single_entity() {
        use crate::render::froxel_bounds::{CameraProjection, calculate_froxel_bounds};

        let camera = CameraProjection::default();
        let froxel_bounds = calculate_froxel_bounds(&camera);

        // Place an entity at (0, 5, -10) — directly in front of the default camera
        let entity = SdfBounds::from_center_extents([0.0, 5.0, -10.0], [2.0, 2.0, 2.0]);
        let result = assign_sdfs_to_froxels(&[entity], &froxel_bounds);

        // At least one froxel should contain this entity
        let total_assigned: u32 = result.lists.iter().map(|l| l.count).sum();
        assert!(
            total_assigned > 0,
            "Entity in front of camera should be assigned to at least one froxel"
        );
    }

    #[test]
    fn test_assign_sdfs_to_froxels_entity_behind_camera() {
        use crate::render::froxel_bounds::{CameraProjection, calculate_froxel_bounds};

        let camera = CameraProjection::default();
        let froxel_bounds = calculate_froxel_bounds(&camera);

        // Place entity behind camera (camera looks toward -Z, so +Z is behind)
        let entity = SdfBounds::from_center_extents([0.0, 5.0, 100.0], [1.0, 1.0, 1.0]);
        let result = assign_sdfs_to_froxels(&[entity], &froxel_bounds);

        // Should not be in any froxel (it's behind the camera)
        let total_assigned: u32 = result.lists.iter().map(|l| l.count).sum();
        assert_eq!(
            total_assigned, 0,
            "Entity behind camera should not be assigned to any froxel"
        );
    }
}
