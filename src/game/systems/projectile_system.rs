//! Projectile lifecycle management system.
//!
//! Owns the collection of active projectiles and their physics config,
//! providing fire / update / clear / iterate operations with zero GPU coupling.

use glam::Vec3;

use crate::physics::ballistics::{BallisticsConfig, Projectile, ProjectileState};

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
}

/// Manages the full lifecycle of ballistic projectiles.
///
/// Encapsulates spawning, physics integration, expiry, and iteration so
/// that callers only need to supply collision results back via [`remove`].
pub struct ProjectileSystem {
    projectiles: Vec<Projectile>,
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
        if self.projectiles.len() >= self.max_projectiles {
            return false;
        }
        let projectile = Projectile::spawn(position, direction, speed, 5.0);
        self.projectiles.push(projectile);
        true
    }

    /// Spawn a pre-built projectile (e.g. one returned by `Cannon::fire()`).
    ///
    /// Returns `true` if the projectile was added.
    pub fn fire_projectile(&mut self, projectile: Projectile) -> bool {
        if self.projectiles.len() >= self.max_projectiles {
            return false;
        }
        self.projectiles.push(projectile);
        true
    }

    /// Integrate physics for every active projectile.
    ///
    /// Returns a [`ProjectileUpdate`] for each projectile that is still
    /// `Flying` after integration — the caller should use these to run
    /// world-level collision checks and call [`remove`] for any hits.
    ///
    /// Projectiles that are `Expired` or `Hit` (ground) are removed
    /// automatically.
    pub fn update(&mut self, delta: f32) -> Vec<ProjectileUpdate> {
        let mut updates = Vec::new();
        let mut keep = vec![true; self.projectiles.len()];

        for (i, projectile) in self.projectiles.iter_mut().enumerate() {
            let prev_pos = projectile.position;
            let state = projectile.integrate(&self.config, delta);

            match state {
                ProjectileState::Flying => {
                    updates.push(ProjectileUpdate {
                        index: i,
                        prev_pos,
                        new_pos: projectile.position,
                        state,
                    });
                }
                // Ground hit or expired — auto-remove
                _ => {
                    keep[i] = false;
                }
            }
        }

        // Remove expired/hit projectiles (iterate in reverse to preserve indices)
        let mut idx = self.projectiles.len();
        while idx > 0 {
            idx -= 1;
            if !keep[idx] {
                self.projectiles.swap_remove(idx);
            }
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
        self.projectiles.iter()
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
