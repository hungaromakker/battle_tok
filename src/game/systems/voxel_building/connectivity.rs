use std::collections::{HashSet, VecDeque};

use super::types::VoxelCoord;
use super::world::VoxelWorld;

pub fn disconnected_components(world: &VoxelWorld) -> Vec<Vec<VoxelCoord>> {
    let occupied = world.occupied_coords();
    if occupied.is_empty() {
        return Vec::new();
    }

    let occupied_set: HashSet<VoxelCoord> = occupied.iter().copied().collect();
    let mut stable = HashSet::new();
    let mut queue = VecDeque::new();

    for coord in &occupied {
        if coord.y <= 0 {
            queue.push_back(*coord);
            stable.insert(*coord);
        }
    }

    while let Some(coord) = queue.pop_front() {
        for neighbor in neighbors6(coord) {
            if !occupied_set.contains(&neighbor) || stable.contains(&neighbor) {
                continue;
            }
            stable.insert(neighbor);
            queue.push_back(neighbor);
        }
    }

    let mut remaining: HashSet<VoxelCoord> = occupied_set.difference(&stable).copied().collect();
    let mut components = Vec::new();

    while let Some(seed) = remaining.iter().next().copied() {
        remaining.remove(&seed);
        let mut component = vec![seed];
        let mut bfs = VecDeque::from([seed]);
        while let Some(node) = bfs.pop_front() {
            for n in neighbors6(node) {
                if remaining.remove(&n) {
                    component.push(n);
                    bfs.push_back(n);
                }
            }
        }
        components.push(component);
    }

    components
}

pub fn unsupported_from_region(
    occupied_region: &HashSet<VoxelCoord>,
    anchored_region: &HashSet<VoxelCoord>,
    boundary_supported: &HashSet<VoxelCoord>,
) -> Vec<VoxelCoord> {
    if occupied_region.is_empty() {
        return Vec::new();
    }

    let mut stable = HashSet::new();
    let mut queue = VecDeque::new();

    for coord in anchored_region.iter().chain(boundary_supported.iter()) {
        if occupied_region.contains(coord) && stable.insert(*coord) {
            queue.push_back(*coord);
        }
    }

    while let Some(coord) = queue.pop_front() {
        for neighbor in neighbors6(coord) {
            if !occupied_region.contains(&neighbor) || stable.contains(&neighbor) {
                continue;
            }
            stable.insert(neighbor);
            queue.push_back(neighbor);
        }
    }

    occupied_region
        .difference(&stable)
        .copied()
        .collect::<Vec<VoxelCoord>>()
}

#[inline]
pub fn neighbors6(c: VoxelCoord) -> [VoxelCoord; 6] {
    [
        VoxelCoord::new(c.x + 1, c.y, c.z),
        VoxelCoord::new(c.x - 1, c.y, c.z),
        VoxelCoord::new(c.x, c.y + 1, c.z),
        VoxelCoord::new(c.x, c.y - 1, c.z),
        VoxelCoord::new(c.x, c.y, c.z + 1),
        VoxelCoord::new(c.x, c.y, c.z - 1),
    ]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn unsupported_from_region_keeps_anchor_connected_cells() {
        let occupied: HashSet<VoxelCoord> = [
            VoxelCoord::new(0, 0, 0),
            VoxelCoord::new(1, 0, 0),
            VoxelCoord::new(2, 0, 0),
        ]
        .into_iter()
        .collect();
        let anchored: HashSet<VoxelCoord> = [VoxelCoord::new(0, 0, 0)].into_iter().collect();
        let boundary: HashSet<VoxelCoord> = HashSet::new();
        let unsupported = unsupported_from_region(&occupied, &anchored, &boundary);
        assert!(unsupported.is_empty());
    }

    #[test]
    fn unsupported_from_region_reports_disconnected_cells() {
        let occupied: HashSet<VoxelCoord> = [
            VoxelCoord::new(0, 0, 0),
            VoxelCoord::new(1, 0, 0),
            VoxelCoord::new(8, 0, 0),
        ]
        .into_iter()
        .collect();
        let anchored: HashSet<VoxelCoord> = [VoxelCoord::new(0, 0, 0)].into_iter().collect();
        let boundary: HashSet<VoxelCoord> = HashSet::new();
        let unsupported = unsupported_from_region(&occupied, &anchored, &boundary);
        assert_eq!(unsupported, vec![VoxelCoord::new(8, 0, 0)]);
    }
}
