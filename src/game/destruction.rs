//! Physics-Based Destruction System
//!
//! Handles falling prisms, debris particles, and falling meteors when structures lose support.

use glam::Vec3;

use super::terrain::terrain_height_at;

/// Gravity constant (m/s²)
pub const GRAVITY: f32 = 9.81;

/// A hex-prism that is falling due to lost support
#[derive(Clone)]
pub struct FallingPrism {
    pub coord: (i32, i32, i32),
    pub position: Vec3,
    pub velocity: Vec3,
    pub angular_velocity: Vec3,
    pub rotation: Vec3,
    pub material: u8,
    pub lifetime: f32,
    pub grounded: bool,
}

impl FallingPrism {
    pub fn new(coord: (i32, i32, i32), position: Vec3, material: u8) -> Self {
        let rand_x = ((coord.0 as f32 * 12.9898).sin() * 43758.5453).fract() - 0.5;
        let rand_z = ((coord.1 as f32 * 78.233).sin() * 43758.5453).fract() - 0.5;
        
        Self {
            coord,
            position,
            velocity: Vec3::new(rand_x * 2.0, 0.0, rand_z * 2.0),
            angular_velocity: Vec3::new(rand_x * 5.0, rand_z * 3.0, rand_x * 4.0),
            rotation: Vec3::ZERO,
            material,
            lifetime: 0.0,
            grounded: false,
        }
    }
    
    pub fn update(&mut self, delta_time: f32) {
        if self.grounded {
            return;
        }
        
        self.lifetime += delta_time;
        self.velocity.y -= GRAVITY * delta_time;
        self.position += self.velocity * delta_time;
        self.rotation += self.angular_velocity * delta_time;
        
        let ground_height = terrain_height_at(self.position.x, self.position.z, 0.0);
        if self.position.y < ground_height + 0.1 {
            self.position.y = ground_height + 0.1;
            self.grounded = true;
            self.velocity = Vec3::ZERO;
        }
    }
}

/// A debris particle from destroyed prisms
#[derive(Clone)]
pub struct DebrisParticle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub size: f32,
    pub color: [f32; 4],
    pub lifetime: f32,
    pub grounded: bool,
}

impl DebrisParticle {
    pub fn new(position: Vec3, velocity: Vec3, material: u8) -> Self {
        let size = 0.03 + (position.x * 12.9898).sin().abs() * 0.05;
        let color = get_material_color(material);
        
        Self {
            position,
            velocity,
            size,
            color,
            lifetime: 3.0 + (position.z * 78.233).sin().abs() * 2.0,
            grounded: false,
        }
    }
    
    pub fn update(&mut self, delta_time: f32) {
        if self.grounded {
            self.lifetime -= delta_time;
            return;
        }
        
        self.lifetime -= delta_time;
        self.velocity.y -= GRAVITY * delta_time;
        self.velocity *= 0.99;
        self.position += self.velocity * delta_time;
        
        let ground_height = terrain_height_at(self.position.x, self.position.z, 0.0);
        if self.position.y < ground_height + self.size {
            self.position.y = ground_height + self.size;
            if self.velocity.y.abs() > 0.5 {
                self.velocity.y *= -0.3;
                self.velocity.x *= 0.8;
                self.velocity.z *= 0.8;
            } else {
                self.grounded = true;
                self.velocity = Vec3::ZERO;
            }
        }
    }
    
    pub fn is_alive(&self) -> bool {
        self.lifetime > 0.0
    }
}

/// Get material color for debris
pub fn get_material_color(material: u8) -> [f32; 4] {
    match material {
        0 => [0.6, 0.6, 0.6, 1.0],
        1 => [0.7, 0.5, 0.3, 1.0],
        2 => [0.4, 0.4, 0.45, 1.0],
        3 => [0.8, 0.7, 0.5, 1.0],
        4 => [0.3, 0.3, 0.35, 1.0],
        5 => [0.6, 0.3, 0.2, 1.0],
        6 => [0.2, 0.4, 0.2, 1.0],
        7 => [0.5, 0.5, 0.6, 1.0],
        _ => [0.5, 0.5, 0.5, 1.0],
    }
}

/// Spawn debris particles from a destroyed/falling prism
pub fn spawn_debris(position: Vec3, material: u8, count: usize) -> Vec<DebrisParticle> {
    let mut particles = Vec::with_capacity(count);

    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let height_offset = ((i as f32 * 0.618).fract() - 0.5) * 0.3;
        let speed = 2.0 + (i as f32 * 1.618).fract() * 4.0;

        let velocity = Vec3::new(
            angle.cos() * speed,
            speed * 0.5 + (i as f32 * 0.414).fract() * 3.0,
            angle.sin() * speed,
        );

        let spawn_pos = position + Vec3::new(
            (angle + 0.5).cos() * 0.1,
            height_offset,
            (angle + 0.5).sin() * 0.1,
        );

        particles.push(DebrisParticle::new(spawn_pos, velocity, material));
    }

    particles
}

// ============================================================================
// METEOR / FIREBALL SYSTEM
// ============================================================================

/// A falling meteor/fireball for apocalyptic atmosphere
#[derive(Clone)]
pub struct Meteor {
    pub position: Vec3,
    pub velocity: Vec3,
    pub size: f32,
    /// HDR emissive color (fire orange-red, values 3.5+ for bloom)
    pub color: [f32; 4],
    pub lifetime: f32,
    pub trail_timer: f32,
    pub active: bool,
    /// Rotation angles (euler) for tumbling visual effect
    pub rotation: Vec3,
    /// Angular velocity for tumbling
    pub angular_velocity: Vec3,
}

impl Meteor {
    /// Create a new meteor falling from the sky
    pub fn new(target_x: f32, target_z: f32, seed: f32) -> Self {
        // Start high in the sky, offset from target
        let start_height = 80.0 + seed.fract() * 40.0;
        let offset_x = ((seed * 12.9898).sin() * 43758.5453).fract() * 20.0 - 10.0;
        let offset_z = ((seed * 78.233).sin() * 43758.5453).fract() * 20.0 - 10.0;

        let position = Vec3::new(
            target_x + offset_x * 2.0,
            start_height,
            target_z + offset_z * 2.0,
        );

        // Calculate velocity to aim roughly at target area
        let direction = Vec3::new(
            target_x - position.x,
            -start_height,
            target_z - position.z,
        ).normalize();

        let speed = 25.0 + seed.fract() * 15.0;
        let velocity = direction * speed;

        // Size varies
        let size = 0.5 + seed.fract() * 1.0;

        // HDR emissive fire color - bright enough for bloom (3.5+)
        let brightness = 3.5 + seed.fract() * 2.0; // 3.5 to 5.5 for dramatic fireballs
        let color = [brightness, brightness * 0.28, brightness * 0.06, 1.0];

        // Tumbling rotation - random angular velocity for visual interest
        let ang_x = ((seed * 3.14159).sin() * 43758.5453).fract() * 4.0 - 2.0;
        let ang_y = ((seed * 2.71828).cos() * 43758.5453).fract() * 3.0 - 1.5;
        let ang_z = ((seed * 1.61803).sin() * 43758.5453).fract() * 5.0 - 2.5;

        Self {
            position,
            velocity,
            size,
            color,
            lifetime: 10.0,
            trail_timer: 0.0,
            active: true,
            rotation: Vec3::ZERO,
            angular_velocity: Vec3::new(ang_x, ang_y, ang_z),
        }
    }

    /// Update meteor physics and tumbling rotation
    pub fn update(&mut self, delta_time: f32) -> Option<Vec3> {
        if !self.active {
            return None;
        }

        self.lifetime -= delta_time;
        self.trail_timer += delta_time;

        // Slight gravity (less than real - these are magical fireballs)
        self.velocity.y -= GRAVITY * 0.3 * delta_time;

        // Move
        self.position += self.velocity * delta_time;

        // Tumbling rotation for visual interest
        self.rotation += self.angular_velocity * delta_time;

        // Check ground impact
        let ground_height = terrain_height_at(self.position.x, self.position.z, 0.0);
        if self.position.y < ground_height + self.size * 0.5 {
            self.active = false;
            return Some(self.position); // Return impact position for explosion
        }

        // Time out
        if self.lifetime <= 0.0 {
            self.active = false;
        }

        None
    }

    /// Check if meteor should spawn a trail particle (every 50ms)
    pub fn should_spawn_trail(&mut self) -> bool {
        if self.trail_timer >= 0.05 {
            self.trail_timer = 0.0;
            true
        } else {
            false
        }
    }

    /// Get the trail particle spawn position (slightly behind meteor)
    pub fn trail_spawn_position(&self) -> [f32; 3] {
        // Offset slightly in opposite direction of velocity for trailing effect
        let offset = self.velocity.normalize_or_zero() * -self.size * 0.5;
        let pos = self.position + offset;
        [pos.x, pos.y, pos.z]
    }

    pub fn is_alive(&self) -> bool {
        self.active && self.lifetime > 0.0
    }
}

/// Meteor spawner - creates meteors at regular intervals
pub struct MeteorSpawner {
    pub spawn_timer: f32,
    pub spawn_interval: f32,
    pub arena_center: Vec3,
    pub arena_radius: f32,
    pub seed_counter: f32,
    pub max_meteors: usize,
}

impl MeteorSpawner {
    pub fn new(arena_center: Vec3, arena_radius: f32) -> Self {
        Self {
            spawn_timer: 0.0,
            spawn_interval: 2.5, // New meteor every 2.5 seconds
            arena_center,
            arena_radius,
            seed_counter: 0.0,
            max_meteors: 8,
        }
    }

    /// Update spawner and potentially spawn new meteors
    pub fn update(&mut self, delta_time: f32, current_meteors: usize) -> Option<Meteor> {
        self.spawn_timer += delta_time;

        if self.spawn_timer >= self.spawn_interval && current_meteors < self.max_meteors {
            self.spawn_timer = 0.0;
            self.seed_counter += 1.0;

            // Random position within arena
            let angle = self.seed_counter * 2.399963; // Golden angle
            let dist = (self.seed_counter * 0.618).fract() * self.arena_radius * 0.8;

            let target_x = self.arena_center.x + angle.cos() * dist;
            let target_z = self.arena_center.z + angle.sin() * dist;

            Some(Meteor::new(target_x, target_z, self.seed_counter))
        } else {
            None
        }
    }
}

/// Spawn fire debris when meteor impacts - dramatic HDR burst
pub fn spawn_meteor_impact(position: Vec3, count: usize) -> Vec<DebrisParticle> {
    let mut particles = Vec::with_capacity(count);

    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let speed = 5.0 + (i as f32 * 1.618).fract() * 10.0;

        let velocity = Vec3::new(
            angle.cos() * speed,
            speed * 0.8 + (i as f32 * 0.414).fract() * 8.0,
            angle.sin() * speed,
        );

        let spawn_pos = position + Vec3::new(
            (angle + 0.5).cos() * 0.2,
            0.1,
            (angle + 0.5).sin() * 0.2,
        );

        // Fire debris (material 10 = fire) with bright HDR colors for bloom
        let mut particle = DebrisParticle::new(spawn_pos, velocity, 10);
        // HDR orange fire - values 3.5+ for dramatic bloom effect
        let brightness = 3.5 + (i as f32 * 0.618).fract() * 1.5;
        particle.color = [brightness, brightness * 0.28, brightness * 0.05, 1.0];
        particle.size = 0.08 + (i as f32 * 0.618).fract() * 0.12;
        particle.lifetime = 1.5 + (i as f32 * 0.414).fract() * 1.0;
        particles.push(particle);
    }

    particles
}

/// Create trail ember particle data for the particle system
/// Returns (position, color, size) tuple for spawning via ParticleSystem
pub fn meteor_trail_ember(meteor: &Meteor, seed: f32) -> ([f32; 3], [f32; 3], f32) {
    // Position: slightly randomized around meteor trail position
    let base_pos = meteor.trail_spawn_position();
    let offset_x = ((seed * 12.9898).sin() * 43758.5453).fract() * 0.3 - 0.15;
    let offset_z = ((seed * 78.233).sin() * 43758.5453).fract() * 0.3 - 0.15;
    let position = [
        base_pos[0] + offset_x,
        base_pos[1],
        base_pos[2] + offset_z,
    ];

    // HDR ember color - bright orange/yellow for trail (slightly less bright than meteor core)
    let brightness = 2.5 + (seed * 0.618).fract() * 1.0;
    let color = [brightness, brightness * 0.35, brightness * 0.08];

    // Size: small ember particles
    let size = 0.1 + (seed * 1.414).fract() * 0.15;

    (position, color, size)
}
