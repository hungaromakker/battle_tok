use std::collections::HashSet;

use glam::IVec3;

use super::types::{BrickLeaf64, BrickNode, VoxelCoord};
use super::world::{CHUNK_EDGE_I32, VoxelWorld};

#[derive(Default)]
pub struct BrickTree {
    pub nodes: Vec<BrickNode>,
    pub leaves: Vec<BrickLeaf64>,
    dirty_chunks: HashSet<IVec3>,
}

impl BrickTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mark_chunk_dirty(&mut self, chunk: IVec3) {
        self.dirty_chunks.insert(chunk);
    }

    pub fn rebuild_from_world(&mut self, world: &VoxelWorld) {
        self.nodes.clear();
        self.leaves.clear();
        self.dirty_chunks.clear();

        let mut by_chunk: std::collections::HashMap<IVec3, Vec<(VoxelCoord, super::types::VoxelCell)>> =
            std::collections::HashMap::new();
        for (coord, cell) in world.occupied_cells_snapshot() {
            let chunk = IVec3::new(
                coord.x.div_euclid(CHUNK_EDGE_I32),
                coord.y.div_euclid(CHUNK_EDGE_I32),
                coord.z.div_euclid(CHUNK_EDGE_I32),
            );
            by_chunk.entry(chunk).or_default().push((coord, cell));
        }

        let mut chunk_entries: Vec<_> = by_chunk.into_iter().collect();
        chunk_entries.sort_by_key(|(chunk, _)| Self::pack_chunk_coord(*chunk));

        for (chunk_key, cells) in chunk_entries {
            let leaf_base = self.leaves.len() as u32;
            let mut node_mask = 0u64;

            // Chunk (16^3) -> root split into 4x4x4 children, each child stores
            // one 4x4x4 BrickLeaf64.
            for child_z in 0..4 {
                for child_y in 0..4 {
                    for child_x in 0..4 {
                        let child_idx = child_x + child_y * 4 + child_z * 16;
                        let mut leaf = BrickLeaf64::default();
                        let base_x = child_x * 4;
                        let base_y = child_y * 4;
                        let base_z = child_z * 4;

                        for (coord, cell) in &cells {
                            let lx = coord.x.rem_euclid(CHUNK_EDGE_I32);
                            let ly = coord.y.rem_euclid(CHUNK_EDGE_I32);
                            let lz = coord.z.rem_euclid(CHUNK_EDGE_I32);
                            if lx < base_x
                                || lx >= base_x + 4
                                || ly < base_y
                                || ly >= base_y + 4
                                || lz < base_z
                                || lz >= base_z + 4
                            {
                                continue;
                            }
                            let sub_x = lx - base_x;
                            let sub_y = ly - base_y;
                            let sub_z = lz - base_z;
                            let sub_idx = (sub_x + sub_y * 4 + sub_z * 16) as usize;
                            leaf.occupancy_mask |= 1u64 << sub_idx;
                            leaf.material[sub_idx] = cell.material;
                            leaf.color_rgb[sub_idx] = cell.color_rgb;
                            leaf.normal_oct[sub_idx] = cell.normal_oct;
                            leaf.hp[sub_idx] = cell.hp;
                        }

                        if leaf.occupancy_mask != 0 {
                            node_mask |= 1u64 << child_idx;
                            self.leaves.push(leaf);
                        }
                    }
                }
            }

            self.nodes.push(BrickNode {
                child_mask: node_mask,
                child_base_index: u32::MAX,
                leaf_payload_index: leaf_base,
                lod_meta: Self::pack_chunk_coord(chunk_key),
                _pad0: 0,
            });
        }
    }

    fn pack_chunk_coord(chunk: IVec3) -> u32 {
        let pack_axis = |v: i32| -> u32 { ((v.clamp(-512, 511) + 512) as u32) & 0x3ff };
        pack_axis(chunk.x) | (pack_axis(chunk.y) << 10) | (pack_axis(chunk.z) << 20)
    }
}
