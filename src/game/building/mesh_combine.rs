//! Mesh Combiner - Combine adjacent blocks into unified meshes
//!
//! Adjacent blocks are combined to:
//! - Reduce draw calls
//! - Remove internal faces
//! - Create smoother visuals with Stalberg deformation

use glam::{IVec3, Vec3};
use std::collections::{HashMap, HashSet};

use super::dual_grid::{CornerType, DualGrid, GridCell, BLOCK_SIZE};

/// A vertex in the combined mesh
#[derive(Debug, Clone, Copy)]
pub struct CombinedVertex {
    pub position: Vec3,
    pub normal: Vec3,
    pub color: Vec3,
    pub uv: [f32; 2],
}

/// A combined mesh from multiple blocks
#[derive(Debug, Clone)]
pub struct CombinedMesh {
    /// Unique ID for this mesh
    pub id: u32,
    /// Vertices
    pub vertices: Vec<CombinedVertex>,
    /// Indices
    pub indices: Vec<u32>,
    /// Bounding box min
    pub bounds_min: Vec3,
    /// Bounding box max
    pub bounds_max: Vec3,
    /// Is this mesh dirty (needs rebuild)?
    pub dirty: bool,
}

impl CombinedMesh {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            vertices: Vec::new(),
            indices: Vec::new(),
            bounds_min: Vec3::ZERO,
            bounds_max: Vec3::ZERO,
            dirty: true,
        }
    }

    /// Update bounds from vertices
    fn update_bounds(&mut self) {
        if self.vertices.is_empty() {
            self.bounds_min = Vec3::ZERO;
            self.bounds_max = Vec3::ZERO;
            return;
        }

        self.bounds_min = Vec3::splat(f32::MAX);
        self.bounds_max = Vec3::splat(f32::MIN);

        for v in &self.vertices {
            self.bounds_min = self.bounds_min.min(v.position);
            self.bounds_max = self.bounds_max.max(v.position);
        }
    }
}

/// Face direction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum FaceDir {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

impl FaceDir {
    fn normal(&self) -> Vec3 {
        match self {
            FaceDir::PosX => Vec3::X,
            FaceDir::NegX => Vec3::NEG_X,
            FaceDir::PosY => Vec3::Y,
            FaceDir::NegY => Vec3::NEG_Y,
            FaceDir::PosZ => Vec3::Z,
            FaceDir::NegZ => Vec3::NEG_Z,
        }
    }

    fn offset(&self) -> IVec3 {
        match self {
            FaceDir::PosX => IVec3::X,
            FaceDir::NegX => IVec3::NEG_X,
            FaceDir::PosY => IVec3::Y,
            FaceDir::NegY => IVec3::NEG_Y,
            FaceDir::PosZ => IVec3::Z,
            FaceDir::NegZ => IVec3::NEG_Z,
        }
    }

    fn vertices(&self) -> [Vec3; 4] {
        let h = BLOCK_SIZE / 2.0;
        match self {
            FaceDir::PosX => [
                Vec3::new(h, -h, -h),
                Vec3::new(h, h, -h),
                Vec3::new(h, h, h),
                Vec3::new(h, -h, h),
            ],
            FaceDir::NegX => [
                Vec3::new(-h, -h, h),
                Vec3::new(-h, h, h),
                Vec3::new(-h, h, -h),
                Vec3::new(-h, -h, -h),
            ],
            FaceDir::PosY => [
                Vec3::new(-h, h, -h),
                Vec3::new(-h, h, h),
                Vec3::new(h, h, h),
                Vec3::new(h, h, -h),
            ],
            FaceDir::NegY => [
                Vec3::new(-h, -h, h),
                Vec3::new(-h, -h, -h),
                Vec3::new(h, -h, -h),
                Vec3::new(h, -h, h),
            ],
            FaceDir::PosZ => [
                Vec3::new(-h, -h, h),
                Vec3::new(-h, h, h),
                Vec3::new(h, h, h),
                Vec3::new(h, -h, h),
            ],
            FaceDir::NegZ => [
                Vec3::new(h, -h, -h),
                Vec3::new(h, h, -h),
                Vec3::new(-h, h, -h),
                Vec3::new(-h, -h, -h),
            ],
        }
    }
}

/// Mesh combiner for building blocks
#[derive(Debug, Clone, Default)]
pub struct MeshCombiner {
    /// Combined meshes by region
    meshes: HashMap<IVec3, CombinedMesh>,
    /// Region size (in blocks)
    region_size: i32,
    /// Next mesh ID
    next_id: u32,
    /// Dirty regions that need rebuild
    dirty_regions: HashSet<IVec3>,
}

impl MeshCombiner {
    pub fn new(region_size: i32) -> Self {
        Self {
            meshes: HashMap::new(),
            region_size,
            next_id: 1,
            dirty_regions: HashSet::new(),
        }
    }

    /// Get region for a block position
    fn get_region(&self, pos: IVec3) -> IVec3 {
        IVec3::new(
            pos.x.div_euclid(self.region_size),
            pos.y.div_euclid(self.region_size),
            pos.z.div_euclid(self.region_size),
        )
    }

    /// Mark a region as dirty
    pub fn mark_dirty(&mut self, block_pos: IVec3) {
        let region = self.get_region(block_pos);
        self.dirty_regions.insert(region);

        // Also mark adjacent regions if near boundary
        let local = IVec3::new(
            block_pos.x.rem_euclid(self.region_size),
            block_pos.y.rem_euclid(self.region_size),
            block_pos.z.rem_euclid(self.region_size),
        );

        for (dim, val) in [(0, local.x), (1, local.y), (2, local.z)] {
            if val == 0 {
                let mut adj = region;
                adj[dim] -= 1;
                self.dirty_regions.insert(adj);
            }
            if val == self.region_size - 1 {
                let mut adj = region;
                adj[dim] += 1;
                self.dirty_regions.insert(adj);
            }
        }
    }

    /// Rebuild dirty regions from grid
    pub fn rebuild_dirty(&mut self, grid: &DualGrid) {
        let dirty: Vec<IVec3> = self.dirty_regions.drain().collect();
        for region in dirty {
            self.rebuild_region(region, grid);
        }
    }

    /// Rebuild a single region
    fn rebuild_region(&mut self, region: IVec3, grid: &DualGrid) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let region_min = region * self.region_size;
        let region_max = region_min + IVec3::splat(self.region_size);

        // Collect all solid cells in this region
        for (pos, cell) in grid.solid_cells() {
            if pos.x >= region_min.x
                && pos.x < region_max.x
                && pos.y >= region_min.y
                && pos.y < region_max.y
                && pos.z >= region_min.z
                && pos.z < region_max.z
            {
                self.add_cell_faces(
                    *pos,
                    cell,
                    grid,
                    &mut vertices,
                    &mut indices,
                );
            }
        }

        if vertices.is_empty() {
            self.meshes.remove(&region);
        } else {
            let mesh = self.meshes.entry(region).or_insert_with(|| {
                let id = self.next_id;
                self.next_id += 1;
                CombinedMesh::new(id)
            });

            mesh.vertices = vertices;
            mesh.indices = indices;
            mesh.update_bounds();
            mesh.dirty = true;
        }
    }

    /// Add faces for a cell (only external faces)
    fn add_cell_faces(
        &self,
        pos: IVec3,
        cell: &GridCell,
        grid: &DualGrid,
        vertices: &mut Vec<CombinedVertex>,
        indices: &mut Vec<u32>,
    ) {
        let center = grid.grid_to_world(pos);
        let color = cell.dominant_type().color();

        // Check each face direction
        for dir in [
            FaceDir::PosX,
            FaceDir::NegX,
            FaceDir::PosY,
            FaceDir::NegY,
            FaceDir::PosZ,
            FaceDir::NegZ,
        ] {
            let neighbor_pos = pos + dir.offset();

            // Check if neighbor is solid - if so, skip this face
            let neighbor_solid = grid
                .get_cell(neighbor_pos)
                .map(|c| !c.is_empty())
                .unwrap_or(false);

            if neighbor_solid {
                continue;
            }

            // Add face
            let base_idx = vertices.len() as u32;
            let normal = dir.normal();
            let face_verts = dir.vertices();

            for (i, &local_pos) in face_verts.iter().enumerate() {
                // Apply deformation at corners
                let corner_pos = IVec3::new(
                    pos.x + if local_pos.x > 0.0 { 1 } else { 0 },
                    pos.y + if local_pos.y > 0.0 { 1 } else { 0 },
                    pos.z + if local_pos.z > 0.0 { 1 } else { 0 },
                );
                let deform = grid.get_deformation(corner_pos);

                vertices.push(CombinedVertex {
                    position: center + local_pos + deform,
                    normal,
                    color,
                    uv: match i {
                        0 => [0.0, 0.0],
                        1 => [0.0, 1.0],
                        2 => [1.0, 1.0],
                        3 => [1.0, 0.0],
                        _ => [0.0, 0.0],
                    },
                });
            }

            // Two triangles per face
            indices.extend_from_slice(&[
                base_idx,
                base_idx + 1,
                base_idx + 2,
                base_idx,
                base_idx + 2,
                base_idx + 3,
            ]);
        }
    }

    /// Get all meshes
    pub fn meshes(&self) -> impl Iterator<Item = &CombinedMesh> {
        self.meshes.values()
    }

    /// Get mutable meshes (for marking clean after GPU upload)
    pub fn meshes_mut(&mut self) -> impl Iterator<Item = &mut CombinedMesh> {
        self.meshes.values_mut()
    }

    /// Get dirty mesh count
    pub fn dirty_count(&self) -> usize {
        self.meshes.values().filter(|m| m.dirty).count()
    }

    /// Clear all meshes
    pub fn clear(&mut self) {
        self.meshes.clear();
        self.dirty_regions.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_region_calculation() {
        let combiner = MeshCombiner::new(16);

        assert_eq!(combiner.get_region(IVec3::ZERO), IVec3::ZERO);
        assert_eq!(combiner.get_region(IVec3::new(15, 0, 0)), IVec3::ZERO);
        assert_eq!(combiner.get_region(IVec3::new(16, 0, 0)), IVec3::new(1, 0, 0));
        assert_eq!(
            combiner.get_region(IVec3::new(-1, 0, 0)),
            IVec3::new(-1, 0, 0)
        );
    }

    #[test]
    fn test_mesh_generation() {
        let mut combiner = MeshCombiner::new(16);
        let mut grid = DualGrid::new();

        // Add a single block
        grid.set_solid(IVec3::ZERO, CornerType::Stone);

        combiner.mark_dirty(IVec3::ZERO);
        combiner.rebuild_dirty(&grid);

        // Should have one mesh
        assert_eq!(combiner.meshes.len(), 1);

        let mesh = combiner.meshes.values().next().unwrap();
        // Single block has 6 faces * 4 verts = 24 vertices
        assert_eq!(mesh.vertices.len(), 24);
        // 6 faces * 2 triangles * 3 indices = 36 indices
        assert_eq!(mesh.indices.len(), 36);
    }

    #[test]
    fn test_face_culling() {
        let mut combiner = MeshCombiner::new(16);
        let mut grid = DualGrid::new();

        // Add two adjacent blocks
        grid.set_solid(IVec3::ZERO, CornerType::Stone);
        grid.set_solid(IVec3::X, CornerType::Stone);

        combiner.mark_dirty(IVec3::ZERO);
        combiner.mark_dirty(IVec3::X);
        combiner.rebuild_dirty(&grid);

        let mesh = combiner.meshes.values().next().unwrap();

        // Two blocks side by side should have 10 faces (6+6-2 shared)
        // 10 faces * 4 verts = 40 vertices
        assert_eq!(mesh.vertices.len(), 40);
    }
}
