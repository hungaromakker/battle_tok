use std::collections::HashMap;

use glam::Vec3;

use super::types::{BuildAudioEvent, BuildAudioEventKind, VoxelCoord};
use super::world::VoxelWorld;

#[derive(Debug, Clone)]
struct VoxelCluster {
    id: u64,
    voxels: Vec<VoxelCoord>,
    center: Vec3,
    velocity: Vec3,
    age_s: f32,
    settled: bool,
}

#[derive(Default)]
pub struct ClusterPhysics {
    next_id: u64,
    active: HashMap<u64, VoxelCluster>,
}

impl ClusterPhysics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn spawn_components(
        &mut self,
        world: &mut VoxelWorld,
        components: Vec<Vec<VoxelCoord>>,
        audio_events: &mut Vec<BuildAudioEvent>,
    ) {
        for component in components {
            if component.is_empty() {
                continue;
            }
            let mut center = Vec3::ZERO;
            for coord in &component {
                center += VoxelWorld::voxel_to_world_center(*coord);
                let _ = world.remove(*coord);
            }
            center /= component.len() as f32;

            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1).max(1);
            self.active.insert(
                id,
                VoxelCluster {
                    id,
                    voxels: component,
                    center,
                    velocity: Vec3::new(0.0, 1.5, 0.0),
                    age_s: 0.0,
                    settled: false,
                },
            );

            audio_events.push(BuildAudioEvent {
                kind: BuildAudioEventKind::CollapseStart,
                world_pos: center,
                material: 0,
            });
        }
    }

    pub fn tick(&mut self, dt: f32, audio_events: &mut Vec<BuildAudioEvent>) {
        if dt <= 0.0 {
            return;
        }
        let mut settled_ids = Vec::new();
        for cluster in self.active.values_mut() {
            if cluster.settled {
                continue;
            }
            cluster.age_s += dt;
            cluster.velocity.y -= 9.81 * dt;
            cluster.center += cluster.velocity * dt;
            if cluster.center.y <= 0.0 {
                cluster.center.y = 0.0;
                cluster.velocity = Vec3::ZERO;
                cluster.settled = true;
                settled_ids.push(cluster.id);
            } else if cluster.age_s >= 3.5 {
                cluster.settled = true;
                settled_ids.push(cluster.id);
            }
        }

        for id in settled_ids {
            if let Some(cluster) = self.active.remove(&id) {
                audio_events.push(BuildAudioEvent {
                    kind: BuildAudioEventKind::CollapseSettle,
                    world_pos: cluster.center,
                    material: 0,
                });
                let _ = cluster.voxels;
            }
        }
    }
}
