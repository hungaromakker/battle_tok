//! Destruction lifecycle system.
//!
//! Owns falling prisms and debris particles, encapsulating the full
//! destroy → cascade → fall → debris pipeline with zero GPU coupling.

use crate::game::destruction::{DebrisParticle, FallingPrism, spawn_debris};
use crate::game::physics::support::find_unsupported_cascade;
use crate::render::hex_prism::{DEFAULT_HEX_HEIGHT, DEFAULT_HEX_RADIUS, HexPrismGrid};

/// Maximum cascade levels to check above a destroyed prism.
const MAX_CASCADE_LEVELS: i32 = 3;

/// Safety timeout for falling prisms (seconds).
const FALLING_PRISM_MAX_LIFETIME: f32 = 10.0;

/// Debris particles spawned when a prism is directly destroyed.
const DEBRIS_PER_DESTROY: usize = 14;

/// Debris particles spawned when a falling prism hits the ground.
const DEBRIS_PER_GROUND_IMPACT: usize = 18;

/// Debris particles spawned when a falling prism collides with a wall prism.
const DEBRIS_PER_COLLISION: usize = 10;

/// Manages the full destruction lifecycle: destroy → cascade → fall → debris.
///
/// Call [`destroy_prism`](DestructionSystem::destroy_prism) when a projectile
/// hits a hex wall prism. The system handles cascade support checks, creates
/// falling prisms, and manages debris particles. Call
/// [`update`](DestructionSystem::update) each frame to tick physics.
pub struct DestructionSystem {
    falling_prisms: Vec<FallingPrism>,
    debris: Vec<DebrisParticle>,
    total_destroyed: u32,
}

impl Default for DestructionSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl DestructionSystem {
    /// Create an empty destruction system with no active falling prisms or debris.
    pub fn new() -> Self {
        Self {
            falling_prisms: Vec::new(),
            debris: Vec::new(),
            total_destroyed: 0,
        }
    }

    /// Destroy a prism at `coord` and trigger cascade support checks.
    ///
    /// Removes the prism from the grid, spawns debris at the impact site,
    /// then recursively finds and detaches any prisms that lost structural
    /// support.
    pub fn destroy_prism(&mut self, coord: (i32, i32, i32), hex_grid: &mut HexPrismGrid) {
        if let Some(prism) = hex_grid.remove_by_coord(coord) {
            self.total_destroyed += 1;

            // Spawn debris at the destroyed prism's location
            let debris = spawn_debris(prism.center, prism.material, DEBRIS_PER_DESTROY);
            self.debris.extend(debris);

            // Check for cascade — prisms that lost support
            self.check_support_cascade(coord, hex_grid);
        }
    }

    /// Recursively detach prisms that lost support after a destruction.
    fn check_support_cascade(
        &mut self,
        destroyed_coord: (i32, i32, i32),
        hex_grid: &mut HexPrismGrid,
    ) {
        let unsupported = find_unsupported_cascade(
            destroyed_coord,
            |q, r, l| hex_grid.contains(q, r, l),
            MAX_CASCADE_LEVELS,
        );

        for coord in unsupported {
            if let Some(prism) = hex_grid.remove_by_coord(coord) {
                self.falling_prisms
                    .push(FallingPrism::new(coord, prism.center, prism.material));

                // Recursively check for more cascade from this removal
                self.check_support_cascade(coord, hex_grid);
            }
        }
    }

    /// Tick falling-prism physics and debris lifetimes.
    ///
    /// Falling prisms that reach the ground are converted to debris bursts.
    /// Falling prisms that collide with remaining wall prisms destroy those
    /// prisms (triggering further cascades). Expired debris particles are
    /// removed.
    pub fn update(&mut self, delta: f32, hex_grid: &mut HexPrismGrid) {
        self.update_falling_prisms(delta, hex_grid);
        self.update_debris(delta);
    }

    /// Apply gravity and handle collisions for falling prisms.
    fn update_falling_prisms(&mut self, delta: f32, hex_grid: &mut HexPrismGrid) {
        // Update physics for each falling prism
        for prism in &mut self.falling_prisms {
            prism.update(delta);
        }

        // Collect collision results (can't mutate grid inside retain)
        let mut new_debris: Vec<DebrisParticle> = Vec::new();
        let mut prisms_to_destroy: Vec<(i32, i32, i32)> = Vec::new();

        self.falling_prisms.retain(|prism| {
            if prism.grounded {
                // Prism hit the ground — convert to debris burst
                new_debris.extend(spawn_debris(
                    prism.position,
                    prism.material,
                    DEBRIS_PER_GROUND_IMPACT,
                ));
                return false;
            }

            // Convert world position to approximate grid coordinates
            let approx_q = (prism.position.x / (DEFAULT_HEX_RADIUS * 1.732)).round() as i32;
            let approx_r = (prism.position.z / (DEFAULT_HEX_RADIUS * 1.5)).round() as i32;
            let approx_level = (prism.position.y / DEFAULT_HEX_HEIGHT).floor() as i32;

            // Check collision with nearby wall prisms
            for dq in -1..=1 {
                for dr in -1..=1 {
                    for dl in -1..=1 {
                        let check = (approx_q + dq, approx_r + dr, approx_level + dl);
                        if hex_grid.contains(check.0, check.1, check.2) {
                            prisms_to_destroy.push(check);
                            new_debris.extend(spawn_debris(
                                prism.position,
                                prism.material,
                                DEBRIS_PER_COLLISION,
                            ));
                            return false;
                        }
                    }
                }
            }

            // Keep falling if under safety timeout
            prism.lifetime < FALLING_PRISM_MAX_LIFETIME
        });

        self.debris.extend(new_debris);

        // Destroy wall prisms hit by falling debris (triggers further cascades)
        for coord in prisms_to_destroy {
            self.destroy_prism(coord, hex_grid);
        }
    }

    /// Tick debris lifetimes and remove expired particles.
    fn update_debris(&mut self, delta: f32) {
        for particle in &mut self.debris {
            particle.update(delta);
        }
        self.debris.retain(|p| p.is_alive());
    }

    /// Add externally-spawned debris (e.g. from block disintegration or meteor impacts).
    pub fn add_debris(&mut self, particles: Vec<DebrisParticle>) {
        self.debris.extend(particles);
    }

    /// Access falling prisms for rendering.
    pub fn falling_prisms(&self) -> &[FallingPrism] {
        &self.falling_prisms
    }

    /// Access debris particles for rendering.
    pub fn debris(&self) -> &[DebrisParticle] {
        &self.debris
    }

    /// Total prisms destroyed this session.
    pub fn total_destroyed(&self) -> u32 {
        self.total_destroyed
    }

    /// Number of currently falling prisms.
    pub fn falling_count(&self) -> usize {
        self.falling_prisms.len()
    }

    /// Number of active debris particles.
    pub fn debris_count(&self) -> usize {
        self.debris.len()
    }

    /// Remove all falling prisms and debris.
    pub fn clear(&mut self) {
        self.falling_prisms.clear();
        self.debris.clear();
    }
}
