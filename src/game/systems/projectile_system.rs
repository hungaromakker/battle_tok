//! Projectile lifecycle management system.
//!
//! Owns the collection of active projectiles and their physics config,
//! providing fire / update / clear / iterate operations with zero GPU coupling.

use glam::Vec3;

use crate::physics::ballistics::{BallisticsConfig, Projectile, ProjectileState};

/// Projectile archetype used for gameplay behaviors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectileKind {
    Cannonball,
    Rocket,
}

#[derive(Debug, Clone, Copy)]
struct ActiveProjectile {
    projectile: Projectile,
    kind: ProjectileKind,
}

/// Per-projectile data returned by [`ProjectileSystem::update`] so the caller
/// can run collision checks against the world without reaching into the system.
pub struct ProjectileUpdate {
    /// Index into the internal projectile list (valid until next mutation).
    pub index: usize,
    /// Position before this frame's integration step.
    pub prev_pos: Vec3,
    /// Position after this frame's integration step.
    pub new_pos: Vec3,
    /// Physics state after integration.
    pub state: ProjectileState,
    /// Gameplay type of this projectile.
    pub kind: ProjectileKind,
}

/// Manages the full lifecycle of ballistic projectiles.
///
/// Encapsulates spawning, physics integration, expiry, and iteration so
/// that callers only need to supply collision results back via [`remove`].
pub struct ProjectileSystem {
    projectiles: Vec<ActiveProjectile>,
    config: BallisticsConfig,
    /// Maximum number of simultaneously active projectiles.
    pub max_projectiles: usize,
}

impl ProjectileSystem {
    /// Create a new system with the given ballistics configuration.
    pub fn new(config: BallisticsConfig) -> Self {
        Self {
            projectiles: Vec::new(),
            config,
            max_projectiles: 32,
        }
    }

    /// Spawn a new projectile if under the active limit.
    ///
    /// Returns `true` if the projectile was added.
    pub fn fire(&mut self, position: Vec3, direction: Vec3, speed: f32) -> bool {
        self.fire_with_kind(position, direction, speed, ProjectileKind::Cannonball)
    }

    /// Spawn a projectile with a specific gameplay archetype.
    ///
    /// Returns `true` if the projectile was added.
    pub fn fire_with_kind(
        &mut self,
        position: Vec3,
        direction: Vec3,
        speed: f32,
        kind: ProjectileKind,
    ) -> bool {
        if self.projectiles.len() >= self.max_projectiles {
            return false;
        }

        let mut projectile = match kind {
            ProjectileKind::Cannonball => {
                let mut p = Projectile::spawn(position, direction, speed * 0.82, 5.0);
                p.radius = 0.36;
                p.drag_coefficient = 0.36;
                p
            }
            ProjectileKind::Rocket => {
                // Rockets are a bit faster and less affected by drag.
                let mut p = Projectile::spawn(position, direction, speed * 1.1, 3.0);
                p.radius = 0.24;
                p.drag_coefficient = 0.22;
                p
            }
        };
        projectile.active = true;

        self.projectiles.push(ActiveProjectile { projectile, kind });
        true
    }

    /// Spawn a pre-built projectile (e.g. one returned by `Cannon::fire()`).
    ///
    /// Returns `true` if the projectile was added.
    pub fn fire_projectile(&mut self, projectile: Projectile) -> bool {
        self.fire_projectile_with_kind(projectile, ProjectileKind::Cannonball)
    }

    /// Spawn a pre-built projectile with explicit gameplay archetype.
    ///
    /// Returns `true` if the projectile was added.
    pub fn fire_projectile_with_kind(
        &mut self,
        projectile: Projectile,
        kind: ProjectileKind,
    ) -> bool {
        if self.projectiles.len() >= self.max_projectiles {
            return false;
        }
        self.projectiles.push(ActiveProjectile { projectile, kind });
        true
    }

    /// Integrate physics for every active projectile.
    ///
    /// Returns a [`ProjectileUpdate`] for every projectile. Callers own
    /// the remove policy (for wall hits / ground hits / expiry) so indices
    /// remain valid for a full frame.
    pub fn update(&mut self, delta: f32) -> Vec<ProjectileUpdate> {
        let mut updates = Vec::with_capacity(self.projectiles.len());

        for (i, active) in self.projectiles.iter_mut().enumerate() {
            let prev_pos = active.projectile.position;
            let state = active.projectile.integrate(&self.config, delta);

            updates.push(ProjectileUpdate {
                index: i,
                prev_pos,
                new_pos: active.projectile.position,
                state,
                kind: active.kind,
            });
        }

        updates
    }

    /// Remove a projectile by index (after external collision detection).
    pub fn remove(&mut self, index: usize) {
        if index < self.projectiles.len() {
            self.projectiles.swap_remove(index);
        }
    }

    /// Remove all projectiles.
    pub fn clear(&mut self) {
        self.projectiles.clear();
    }

    /// Number of currently active projectiles.
    pub fn active_count(&self) -> usize {
        self.projectiles.len()
    }

    /// Iterate over active projectiles (e.g. for mesh generation).
    pub fn iter(&self) -> impl Iterator<Item = &Projectile> {
        self.projectiles.iter().map(|active| &active.projectile)
    }

    /// Iterate over active projectiles with their gameplay archetype.
    pub fn iter_with_kind(&self) -> impl Iterator<Item = (&Projectile, ProjectileKind)> {
        self.projectiles
            .iter()
            .map(|active| (&active.projectile, active.kind))
    }

    /// Access the ballistics configuration.
    pub fn config(&self) -> &BallisticsConfig {
        &self.config
    }

    /// Mutably access the ballistics configuration.
    pub fn config_mut(&mut self) -> &mut BallisticsConfig {
        &mut self.config
    }
}
