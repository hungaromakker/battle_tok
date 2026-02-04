//! Physics-Based Destruction System
//!
//! Handles falling prisms and debris particles when structures lose support.

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
