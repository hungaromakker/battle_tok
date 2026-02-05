//! Meteor lifecycle management system.
//!
//! Owns the collection of active meteors and their spawner, providing
//! spawn / update / iterate operations with zero GPU coupling.
//! Meteors are atmospheric fireballs that add visual drama to the battlefield.

use glam::Vec3;

use crate::game::destruction::{DebrisParticle, Meteor, MeteorSpawner, spawn_meteor_impact};

/// Data returned for each meteor that impacts the ground during an update.
pub struct MeteorImpact {
    /// World-space position where the meteor hit.
    pub position: Vec3,
    /// Pre-spawned fire debris particles for the caller to manage.
    pub debris: Vec<DebrisParticle>,
}

/// Manages the full lifecycle of atmospheric meteors.
///
/// Encapsulates spawning, physics integration, impact detection, and
/// debris generation so that callers only need to collect impacts.
pub struct MeteorSystem {
    meteors: Vec<Meteor>,
    spawner: MeteorSpawner,
    /// Number of debris particles to spawn per impact.
    debris_per_impact: usize,
}

impl MeteorSystem {
    /// Create a new meteor system centered on the arena.
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self {
            meteors: Vec::new(),
            spawner: MeteorSpawner::new(center, radius),
            debris_per_impact: 15,
        }
    }

    /// Update spawner and all meteors.
    ///
    /// Returns a [`MeteorImpact`] for each meteor that hit the ground this
    /// frame — the caller should feed the `debris` into their particle list.
    pub fn update(&mut self, delta: f32) -> Vec<MeteorImpact> {
        // Try to spawn a new meteor
        if let Some(new_meteor) = self.spawner.update(delta, self.meteors.len()) {
            self.meteors.push(new_meteor);
        }

        // Update existing meteors and collect ground impacts
        let mut impacts = Vec::new();

        for meteor in &mut self.meteors {
            if let Some(impact_pos) = meteor.update(delta) {
                let debris = spawn_meteor_impact(impact_pos, self.debris_per_impact);
                impacts.push(MeteorImpact {
                    position: impact_pos,
                    debris,
                });
            }
        }

        // Remove dead meteors
        self.meteors.retain(|m| m.is_alive());

        impacts
    }

    /// Iterate over active meteors (e.g. for rendering fireballs and trails).
    pub fn iter(&self) -> impl Iterator<Item = &Meteor> {
        self.meteors.iter()
    }

    /// Number of currently active meteors.
    pub fn count(&self) -> usize {
        self.meteors.len()
    }

    /// Access the spawner configuration.
    pub fn spawner(&self) -> &MeteorSpawner {
        &self.spawner
    }

    /// Mutably access the spawner configuration.
    pub fn spawner_mut(&mut self) -> &mut MeteorSpawner {
        &mut self.spawner
    }
}
