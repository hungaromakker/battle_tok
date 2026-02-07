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
