//! BuildingSystemV2 - deterministic structural graph for Forts-style building.
//!
//! This module models gravity-first static support.
//! A block is stable when it is terrain-anchored or sits above a stable block.

use std::collections::{HashMap, HashSet};

use glam::{IVec3, Vec3};

#[derive(Debug, Clone)]
pub struct StructuralNode {
    pub block_id: u32,
    pub cell: IVec3,
    pub material: u8,
    pub terrain_anchor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaceError {
    Occupied,
    NeedsSupport,
}

/// Deterministic structural solver with below-support propagation.
#[derive(Debug, Default)]
pub struct BuildingSystemV2 {
    nodes: HashMap<u32, StructuralNode>,
    by_cell: HashMap<IVec3, u32>,
    terrain_anchors: HashSet<u32>,
    stable_nodes: HashSet<u32>,
    unstable_nodes: HashSet<u32>,
}

impl BuildingSystemV2 {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.nodes.clear();
        self.by_cell.clear();
        self.terrain_anchors.clear();
        self.stable_nodes.clear();
        self.unstable_nodes.clear();
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn stable_count(&self) -> usize {
        self.stable_nodes.len()
    }

    pub fn unstable_block_ids(&self) -> Vec<u32> {
        self.unstable_nodes.iter().copied().collect()
    }

    pub fn block_id_at_cell(&self, cell: IVec3) -> Option<u32> {
        self.by_cell.get(&cell).copied()
    }

    pub fn contains_block_id(&self, block_id: u32) -> bool {
        self.nodes.contains_key(&block_id)
    }

    pub fn is_stable(&self, block_id: u32) -> bool {
        self.stable_nodes.contains(&block_id)
    }

    pub fn is_occupied(&self, cell: IVec3) -> bool {
        self.by_cell.contains_key(&cell)
    }

    pub fn can_place(&self, cell: IVec3, terrain_anchor: bool) -> Result<(), PlaceError> {
        if self.is_occupied(cell) {
            return Err(PlaceError::Occupied);
        }
        if terrain_anchor || self.has_stable_below(cell) {
            Ok(())
        } else {
            Err(PlaceError::NeedsSupport)
        }
    }

    pub fn insert_block(
        &mut self,
        block_id: u32,
        cell: IVec3,
        material: u8,
        terrain_anchor: bool,
    ) -> Result<(), PlaceError> {
        self.can_place(cell, terrain_anchor)?;

        let node = StructuralNode {
            block_id,
            cell,
            material,
            terrain_anchor,
        };
        self.nodes.insert(block_id, node);
        self.by_cell.insert(cell, block_id);
        if terrain_anchor {
            self.terrain_anchors.insert(block_id);
        }

        self.recompute_support();

        if !self.stable_nodes.contains(&block_id) {
            self.remove_block_internal(block_id);
            self.recompute_support();
            return Err(PlaceError::NeedsSupport);
        }

        Ok(())
    }

    /// Remove a block and return currently unstable block IDs after recompute.
    pub fn remove_block(&mut self, block_id: u32) -> Vec<u32> {
        self.remove_block_internal(block_id);
        self.recompute_support();
        self.unstable_block_ids()
    }

    /// Detach a block from the static support graph without touching render/physics managers.
    ///
    /// Used when a block transitions into dynamic falling-rubble simulation.
    pub fn detach_block(&mut self, block_id: u32) -> Vec<u32> {
        self.remove_block_internal(block_id);
        self.recompute_support();
        self.unstable_block_ids()
    }

    pub fn world_to_cell(position: Vec3, grid_size: f32) -> IVec3 {
        IVec3::new(
            (position.x / grid_size).round() as i32,
            (position.y / grid_size).round() as i32,
            (position.z / grid_size).round() as i32,
        )
    }

    fn remove_block_internal(&mut self, block_id: u32) {
        if let Some(node) = self.nodes.remove(&block_id) {
            self.by_cell.remove(&node.cell);
        }
        self.terrain_anchors.remove(&block_id);
        self.stable_nodes.remove(&block_id);
        self.unstable_nodes.remove(&block_id);
    }

    fn has_stable_below(&self, cell: IVec3) -> bool {
        let below = cell + IVec3::new(0, -1, 0);
        self.by_cell
            .get(&below)
            .is_some_and(|below_id| self.stable_nodes.contains(below_id))
    }

    fn recompute_support(&mut self) {
        self.stable_nodes.clear();
        self.unstable_nodes.clear();

        self.terrain_anchors
            .retain(|id| self.nodes.contains_key(id));

        let mut node_ids: Vec<u32> = self.nodes.keys().copied().collect();
        node_ids.sort_by_key(|id| self.nodes.get(id).map(|n| n.cell.y).unwrap_or(i32::MAX));

        for id in node_ids {
            let Some(node) = self.nodes.get(&id) else {
                continue;
            };
            if node.terrain_anchor || self.terrain_anchors.contains(&id) {
                self.stable_nodes.insert(id);
                continue;
            }
            let below = node.cell + IVec3::new(0, -1, 0);
            if let Some(below_id) = self.by_cell.get(&below)
                && self.stable_nodes.contains(below_id)
            {
                self.stable_nodes.insert(id);
            }
        }

        for id in self.nodes.keys().copied() {
            if !self.stable_nodes.contains(&id) {
                self.unstable_nodes.insert(id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_floating_first_block() {
        let mut v2 = BuildingSystemV2::new();
        let result = v2.insert_block(1, IVec3::new(0, 3, 0), 0, false);
        assert_eq!(result, Err(PlaceError::NeedsSupport));
        assert_eq!(v2.node_count(), 0);
    }

    #[test]
    fn accepts_ground_anchor_then_stack() {
        let mut v2 = BuildingSystemV2::new();
        assert!(v2.insert_block(1, IVec3::new(0, 0, 0), 0, true).is_ok());
        assert!(v2.insert_block(2, IVec3::new(0, 1, 0), 0, false).is_ok());
        assert!(v2.is_stable(1));
        assert!(v2.is_stable(2));
        assert_eq!(v2.stable_count(), 2);
    }

    #[test]
    fn removing_anchor_marks_component_unstable() {
        let mut v2 = BuildingSystemV2::new();
        assert!(v2.insert_block(1, IVec3::new(0, 0, 0), 0, true).is_ok());
        assert!(v2.insert_block(2, IVec3::new(0, 1, 0), 0, false).is_ok());
        assert!(v2.insert_block(3, IVec3::new(0, 2, 0), 0, false).is_ok());

        let unstable = v2.remove_block(1);
        assert!(unstable.contains(&2));
        assert!(unstable.contains(&3));
        assert!(!v2.is_stable(2));
        assert!(!v2.is_stable(3));
    }

    #[test]
    fn rejects_lateral_only_support() {
        let mut v2 = BuildingSystemV2::new();
        assert!(v2.insert_block(1, IVec3::new(0, 0, 0), 0, true).is_ok());
        assert_eq!(
            v2.insert_block(2, IVec3::new(1, 0, 0), 0, false),
            Err(PlaceError::NeedsSupport)
        );
    }
}
