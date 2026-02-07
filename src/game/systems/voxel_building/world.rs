use std::collections::{HashMap, HashSet};

use glam::{IVec3, Vec3};

use super::types::{VoxelCell, VoxelCoord, VoxelHit};

pub const VOXEL_SIZE_METERS: f32 = 0.25;
pub const CHUNK_EDGE_I32: i32 = 16;
const CHUNK_CELL_COUNT: usize = (CHUNK_EDGE_I32 as usize).pow(3);

#[derive(Clone)]
struct VoxelChunk {
    cells: Vec<Option<VoxelCell>>,
}

impl Default for VoxelChunk {
    fn default() -> Self {
        Self {
            cells: vec![None; CHUNK_CELL_COUNT],
        }
    }
}

impl VoxelChunk {
    fn local_index(local: IVec3) -> Option<usize> {
        if local.x < 0
            || local.y < 0
            || local.z < 0
            || local.x >= CHUNK_EDGE_I32
            || local.y >= CHUNK_EDGE_I32
            || local.z >= CHUNK_EDGE_I32
        {
            return None;
        }
        let x = local.x as usize;
        let y = local.y as usize;
        let z = local.z as usize;
        Some(x + y * CHUNK_EDGE_I32 as usize + z * (CHUNK_EDGE_I32 as usize).pow(2))
    }

    fn get(&self, local: IVec3) -> Option<&VoxelCell> {
        let idx = Self::local_index(local)?;
        self.cells.get(idx)?.as_ref()
    }

    fn get_mut(&mut self, local: IVec3) -> Option<&mut VoxelCell> {
        let idx = Self::local_index(local)?;
        self.cells.get_mut(idx)?.as_mut()
    }

    fn set(&mut self, local: IVec3, cell: VoxelCell) -> bool {
        let Some(idx) = Self::local_index(local) else {
            return false;
        };
        self.cells[idx] = Some(cell);
        true
    }

    fn remove(&mut self, local: IVec3) -> Option<VoxelCell> {
        let idx = Self::local_index(local)?;
        self.cells[idx].take()
    }
}

#[derive(Default)]
pub struct VoxelWorld {
    chunks: HashMap<IVec3, VoxelChunk>,
    dirty_chunks: HashSet<IVec3>,
}

impl VoxelWorld {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn world_to_voxel_coord(p: Vec3) -> VoxelCoord {
        VoxelCoord::new(
            (p.x / VOXEL_SIZE_METERS).floor() as i32,
            (p.y / VOXEL_SIZE_METERS).floor() as i32,
            (p.z / VOXEL_SIZE_METERS).floor() as i32,
        )
    }

    pub fn voxel_to_world_center(coord: VoxelCoord) -> Vec3 {
        Vec3::new(
            (coord.x as f32 + 0.5) * VOXEL_SIZE_METERS,
            (coord.y as f32 + 0.5) * VOXEL_SIZE_METERS,
            (coord.z as f32 + 0.5) * VOXEL_SIZE_METERS,
        )
    }

    pub fn voxel_to_chunk_local(coord: VoxelCoord) -> (IVec3, IVec3) {
        let v = coord.as_ivec3();
        let chunk = IVec3::new(
            v.x.div_euclid(CHUNK_EDGE_I32),
            v.y.div_euclid(CHUNK_EDGE_I32),
            v.z.div_euclid(CHUNK_EDGE_I32),
        );
        let local = IVec3::new(
            v.x.rem_euclid(CHUNK_EDGE_I32),
            v.y.rem_euclid(CHUNK_EDGE_I32),
            v.z.rem_euclid(CHUNK_EDGE_I32),
        );
        (chunk, local)
    }

    pub fn get(&self, coord: VoxelCoord) -> Option<&VoxelCell> {
        let (chunk_key, local) = Self::voxel_to_chunk_local(coord);
        self.chunks.get(&chunk_key)?.get(local)
    }

    pub fn get_mut(&mut self, coord: VoxelCoord) -> Option<&mut VoxelCell> {
        let (chunk_key, local) = Self::voxel_to_chunk_local(coord);
        self.chunks.get_mut(&chunk_key)?.get_mut(local)
    }

    pub fn place(&mut self, coord: VoxelCoord, cell: VoxelCell) -> bool {
        let (chunk_key, local) = Self::voxel_to_chunk_local(coord);
        let chunk = self.chunks.entry(chunk_key).or_default();
        let changed = chunk.set(local, cell);
        if changed {
            self.dirty_chunks.insert(chunk_key);
        }
        changed
    }

    pub fn remove(&mut self, coord: VoxelCoord) -> Option<VoxelCell> {
        let (chunk_key, local) = Self::voxel_to_chunk_local(coord);
        let removed = self.chunks.get_mut(&chunk_key)?.remove(local);
        if removed.is_some() {
            self.dirty_chunks.insert(chunk_key);
        }
        removed
    }

    pub fn drain_dirty_chunks(&mut self) -> Vec<IVec3> {
        let mut chunks = Vec::with_capacity(self.dirty_chunks.len());
        chunks.extend(self.dirty_chunks.drain());
        chunks
    }

    pub fn occupied_coords(&self) -> Vec<VoxelCoord> {
        let mut out = Vec::new();
        for (chunk_key, chunk) in &self.chunks {
            for z in 0..CHUNK_EDGE_I32 {
                for y in 0..CHUNK_EDGE_I32 {
                    for x in 0..CHUNK_EDGE_I32 {
                        let local = IVec3::new(x, y, z);
                        if chunk.get(local).is_none() {
                            continue;
                        }
                        let world = IVec3::new(
                            chunk_key.x * CHUNK_EDGE_I32 + x,
                            chunk_key.y * CHUNK_EDGE_I32 + y,
                            chunk_key.z * CHUNK_EDGE_I32 + z,
                        );
                        out.push(VoxelCoord::from(world));
                    }
                }
            }
        }
        out
    }

    pub fn occupied_cells_snapshot(&self) -> Vec<(VoxelCoord, VoxelCell)> {
        let mut out = Vec::new();
        for (chunk_key, chunk) in &self.chunks {
            for z in 0..CHUNK_EDGE_I32 {
                for y in 0..CHUNK_EDGE_I32 {
                    for x in 0..CHUNK_EDGE_I32 {
                        let local = IVec3::new(x, y, z);
                        let Some(cell) = chunk.get(local).copied() else {
                            continue;
                        };
                        let world = IVec3::new(
                            chunk_key.x * CHUNK_EDGE_I32 + x,
                            chunk_key.y * CHUNK_EDGE_I32 + y,
                            chunk_key.z * CHUNK_EDGE_I32 + z,
                        );
                        out.push((VoxelCoord::from(world), cell));
                    }
                }
            }
        }
        out
    }

    pub fn raycast_voxel(&self, origin: Vec3, dir: Vec3, max_dist: f32) -> Option<VoxelHit> {
        let dir = dir.normalize_or_zero();
        if dir.length_squared() < 1e-8 || max_dist <= 0.0 {
            return None;
        }

        let mut cell = Self::world_to_voxel_coord(origin).as_ivec3();
        let step = IVec3::new(sign_i(dir.x), sign_i(dir.y), sign_i(dir.z));

        let mut t_max_x = dda_t_max(origin.x, dir.x, cell.x, step.x);
        let mut t_max_y = dda_t_max(origin.y, dir.y, cell.y, step.y);
        let mut t_max_z = dda_t_max(origin.z, dir.z, cell.z, step.z);
        let t_delta_x = dda_t_delta(dir.x);
        let t_delta_y = dda_t_delta(dir.y);
        let t_delta_z = dda_t_delta(dir.z);

        let mut t = 0.0f32;
        let mut hit_normal = IVec3::ZERO;
        let max_steps = (max_dist / VOXEL_SIZE_METERS).ceil() as usize + 4;

        for _ in 0..max_steps {
            let coord = VoxelCoord::from(cell);
            if self.get(coord).is_some() {
                return Some(VoxelHit {
                    coord,
                    world_pos: origin + dir * t,
                    normal: hit_normal,
                });
            }

            if t_max_x <= t_max_y && t_max_x <= t_max_z {
                cell.x += step.x;
                t = t_max_x;
                t_max_x += t_delta_x;
                hit_normal = IVec3::new(-step.x, 0, 0);
            } else if t_max_y <= t_max_x && t_max_y <= t_max_z {
                cell.y += step.y;
                t = t_max_y;
                t_max_y += t_delta_y;
                hit_normal = IVec3::new(0, -step.y, 0);
            } else {
                cell.z += step.z;
                t = t_max_z;
                t_max_z += t_delta_z;
                hit_normal = IVec3::new(0, 0, -step.z);
            }

            if t > max_dist {
                break;
            }
        }

        None
    }
}

fn sign_i(v: f32) -> i32 {
    if v > 0.0 {
        1
    } else if v < 0.0 {
        -1
    } else {
        0
    }
}

fn dda_t_delta(dir_component: f32) -> f32 {
    if dir_component.abs() < 1e-6 {
        f32::INFINITY
    } else {
        VOXEL_SIZE_METERS / dir_component.abs()
    }
}

fn dda_t_max(origin_component: f32, dir_component: f32, cell: i32, step: i32) -> f32 {
    if step == 0 || dir_component.abs() < 1e-6 {
        return f32::INFINITY;
    }
    let boundary = if step > 0 {
        (cell as f32 + 1.0) * VOXEL_SIZE_METERS
    } else {
        cell as f32 * VOXEL_SIZE_METERS
    };
    (boundary - origin_component) / dir_component
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn place_get_remove_roundtrip() {
        let mut world = VoxelWorld::new();
        let coord = VoxelCoord::new(2, 3, -1);
        let cell = VoxelCell {
            material: 1,
            hp: 20,
            max_hp: 20,
            color_rgb: [1, 2, 3],
            normal_oct: [4, 5],
        };
        assert!(world.place(coord, cell));
        assert!(world.get(coord).is_some());
        assert!(world.remove(coord).is_some());
        assert!(world.get(coord).is_none());
    }

    #[test]
    fn raycast_hits_first_occupied_voxel() {
        let mut world = VoxelWorld::new();
        world.place(
            VoxelCoord::new(2, 0, 0),
            VoxelCell {
                material: 0,
                hp: 10,
                max_hp: 10,
                color_rgb: [100, 100, 100],
                normal_oct: [128, 128],
            },
        );
        let hit = world.raycast_voxel(Vec3::new(0.01, 0.01, 0.01), Vec3::X, 8.0);
        assert!(hit.is_some());
        assert_eq!(hit.unwrap().coord, VoxelCoord::new(2, 0, 0));
    }
}
