//! Shared Types Module
//!
//! Contains vertex types, mesh structures, noise functions, and camera
//! that are shared across game modules.

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};

// ============================================================================
// GPU VERTEX TYPES
// ============================================================================

/// Vertex for terrain and objects
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

// ============================================================================
// MESH STRUCTURE
// ============================================================================

/// A mesh with vertices and indices
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn merge(&mut self, other: &Mesh) {
        let base_idx = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base_idx));
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CAMERA SYSTEM
// ============================================================================

/// Simple camera for arena view
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub move_speed: f32,
    pub look_sensitivity: f32,
    pub fov: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            position: Vec3::new(0.0, 15.0, 50.0),
            yaw: 0.0,
            pitch: -0.2,
            move_speed: 20.0,
            look_sensitivity: 0.003,
            fov: 60.0_f32.to_radians(),
            near: 0.1,
            far: 1000.0,
        }
    }
}

impl Camera {
    pub fn get_forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
        .normalize()
    }

    pub fn get_right(&self) -> Vec3 {
        self.get_forward().cross(Vec3::Y).normalize()
    }

    pub fn get_view_matrix(&self) -> Mat4 {
        let target = self.position + self.get_forward();
        Mat4::look_at_rh(self.position, target, Vec3::Y)
    }

    pub fn get_projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect, self.near, self.far)
    }

    pub fn handle_mouse_look(&mut self, delta_x: f32, delta_y: f32) {
        self.yaw += delta_x * self.look_sensitivity;
        self.pitch -= delta_y * self.look_sensitivity;
        let pitch_limit = 89.0_f32.to_radians();
        self.pitch = self.pitch.clamp(-pitch_limit, pitch_limit);
    }

    pub fn update_movement(&mut self, forward: f32, right: f32, up: f32, delta_time: f32, sprint: bool) {
        let speed = if sprint {
            self.move_speed * 2.5
        } else {
            self.move_speed
        };

        let forward_dir = self.get_forward();
        let right_dir = self.get_right();

        let forward_xz = Vec3::new(forward_dir.x, 0.0, forward_dir.z).normalize_or_zero();
        let right_xz = Vec3::new(right_dir.x, 0.0, right_dir.z).normalize_or_zero();

        self.position += forward_xz * forward * speed * delta_time;
        self.position += right_xz * right * speed * delta_time;
        self.position.y += up * speed * delta_time;
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

// ============================================================================
// PROCEDURAL NOISE FUNCTIONS
// ============================================================================

/// Simple hash function for noise generation
pub fn hash_2d(x: f32, y: f32) -> f32 {
    let n = (x * 127.1 + y * 311.7).sin() * 43758.5453;
    n.fract()
}

/// Smoothstep interpolation
pub fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// 2D value noise for terrain height
pub fn noise_2d(x: f32, y: f32) -> f32 {
    let ix = x.floor();
    let iy = y.floor();
    let fx = x - ix;
    let fy = y - iy;
    
    let v00 = hash_2d(ix, iy);
    let v10 = hash_2d(ix + 1.0, iy);
    let v01 = hash_2d(ix, iy + 1.0);
    let v11 = hash_2d(ix + 1.0, iy + 1.0);
    
    let sx = smoothstep(fx);
    let sy = smoothstep(fy);
    
    let v0 = v00 + sx * (v10 - v00);
    let v1 = v01 + sx * (v11 - v01);
    
    v0 + sy * (v1 - v0)
}

/// Fractal Brownian Motion noise for natural terrain
pub fn fbm_noise(x: f32, z: f32, octaves: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    
    for _ in 0..octaves {
        value += amplitude * noise_2d(x * frequency, z * frequency);
        max_value += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    
    value / max_value
}

/// Ridged noise for rocky mountain formations
pub fn ridged_noise(x: f32, z: f32, octaves: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    
    for _ in 0..octaves {
        let n = 1.0 - noise_2d(x * frequency, z * frequency).abs();
        value += amplitude * n * n;
        max_value += amplitude;
        amplitude *= 0.5;
        frequency *= 2.2;
    }
    
    value / max_value
}

/// Turbulent noise for detailed rocky surfaces
pub fn turbulent_noise(x: f32, z: f32, octaves: u32) -> f32 {
    let mut value = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;
    
    for _ in 0..octaves {
        value += amplitude * noise_2d(x * frequency, z * frequency).abs();
        max_value += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    
    value / max_value
}

// ============================================================================
// MESH GENERATION PRIMITIVES
// ============================================================================

/// Generate an axis-aligned box mesh
pub fn generate_box(center: Vec3, half_extents: Vec3, color: [f32; 4]) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let (hx, hy, hz) = (half_extents.x, half_extents.y, half_extents.z);

    let corners = [
        Vec3::new(-hx, -hy, -hz),
        Vec3::new(hx, -hy, -hz),
        Vec3::new(hx, hy, -hz),
        Vec3::new(-hx, hy, -hz),
        Vec3::new(-hx, -hy, hz),
        Vec3::new(hx, -hy, hz),
        Vec3::new(hx, hy, hz),
        Vec3::new(-hx, hy, hz),
    ];

    let faces = [
        ([0, 1, 2, 3], Vec3::new(0.0, 0.0, -1.0)),
        ([5, 4, 7, 6], Vec3::new(0.0, 0.0, 1.0)),
        ([4, 0, 3, 7], Vec3::new(-1.0, 0.0, 0.0)),
        ([1, 5, 6, 2], Vec3::new(1.0, 0.0, 0.0)),
        ([3, 2, 6, 7], Vec3::new(0.0, 1.0, 0.0)),
        ([4, 5, 1, 0], Vec3::new(0.0, -1.0, 0.0)),
    ];

    for (face_indices, normal) in &faces {
        let base = vertices.len() as u32;
        for &i in face_indices {
            let pos = center + corners[i];
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [normal.x, normal.y, normal.z],
                color,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh { vertices, indices }
}

/// Generate an oriented box (for barrel)
pub fn generate_oriented_box(
    center: Vec3,
    size: Vec3,
    forward: Vec3,
    up: Vec3,
    color: [f32; 4],
) -> Mesh {
    let right = forward.cross(up).normalize();
    let up = right.cross(forward).normalize();

    let (hx, hy, hz) = (size.x / 2.0, size.y / 2.0, size.z / 2.0);

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let transform = |local: Vec3| -> Vec3 { center + right * local.x + up * local.y + forward * local.z };

    let corners = [
        Vec3::new(-hx, -hy, -hz),
        Vec3::new(hx, -hy, -hz),
        Vec3::new(hx, hy, -hz),
        Vec3::new(-hx, hy, -hz),
        Vec3::new(-hx, -hy, hz),
        Vec3::new(hx, -hy, hz),
        Vec3::new(hx, hy, hz),
        Vec3::new(-hx, hy, hz),
    ];

    let faces = [
        ([0, 1, 2, 3], -forward),
        ([5, 4, 7, 6], forward),
        ([4, 0, 3, 7], -right),
        ([1, 5, 6, 2], right),
        ([3, 2, 6, 7], up),
        ([4, 5, 1, 0], -up),
    ];

    for (face_indices, normal) in &faces {
        let base = vertices.len() as u32;
        for &i in face_indices {
            let pos = transform(corners[i]);
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [normal.x, normal.y, normal.z],
                color,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    Mesh { vertices, indices }
}

/// Generate a rotated box (for falling prisms)
pub fn generate_rotated_box(center: Vec3, half_extents: Vec3, rotation: Vec3, color: [f32; 4]) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    let (sx, cx) = rotation.x.sin_cos();
    let (sy, cy) = rotation.y.sin_cos();
    let (sz, cz) = rotation.z.sin_cos();
    
    let rotate = |v: Vec3| -> Vec3 {
        let y1 = v.y * cx - v.z * sx;
        let z1 = v.y * sx + v.z * cx;
        let x1 = v.x;
        let x2 = x1 * cy + z1 * sy;
        let z2 = -x1 * sy + z1 * cy;
        let y2 = y1;
        let x3 = x2 * cz - y2 * sz;
        let y3 = x2 * sz + y2 * cz;
        let z3 = z2;
        Vec3::new(x3, y3, z3)
    };
    
    let (hx, hy, hz) = (half_extents.x, half_extents.y, half_extents.z);
    
    let corners: [Vec3; 8] = [
        Vec3::new(-hx, -hy, -hz),
        Vec3::new( hx, -hy, -hz),
        Vec3::new( hx,  hy, -hz),
        Vec3::new(-hx,  hy, -hz),
        Vec3::new(-hx, -hy,  hz),
        Vec3::new( hx, -hy,  hz),
        Vec3::new( hx,  hy,  hz),
        Vec3::new(-hx,  hy,  hz),
    ];
    
    let world_corners: Vec<Vec3> = corners.iter().map(|&c| center + rotate(c)).collect();
    
    let faces: [([usize; 4], Vec3); 6] = [
        ([0, 3, 2, 1], Vec3::new(0.0, 0.0, -1.0)),
        ([4, 5, 6, 7], Vec3::new(0.0, 0.0, 1.0)),
        ([0, 4, 7, 3], Vec3::new(-1.0, 0.0, 0.0)),
        ([1, 2, 6, 5], Vec3::new(1.0, 0.0, 0.0)),
        ([3, 7, 6, 2], Vec3::new(0.0, 1.0, 0.0)),
        ([0, 1, 5, 4], Vec3::new(0.0, -1.0, 0.0)),
    ];
    
    for (face_indices, local_normal) in &faces {
        let base = vertices.len() as u32;
        let world_normal = rotate(*local_normal);
        
        for &i in face_indices {
            let pos = world_corners[i];
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [world_normal.x, world_normal.y, world_normal.z],
                color,
            });
        }
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    
    Mesh { vertices, indices }
}

/// Generate a sphere mesh for projectiles
pub fn generate_sphere(center: Vec3, radius: f32, color: [f32; 4], segments: u32) -> Mesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for lat in 0..=segments {
        let theta = (lat as f32) * std::f32::consts::PI / (segments as f32);
        let sin_theta = theta.sin();
        let cos_theta = theta.cos();

        for lon in 0..=segments {
            let phi = (lon as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
            let sin_phi = phi.sin();
            let cos_phi = phi.cos();

            let x = sin_theta * cos_phi;
            let y = cos_theta;
            let z = sin_theta * sin_phi;

            let pos = center + Vec3::new(x, y, z) * radius;
            vertices.push(Vertex {
                position: [pos.x, pos.y, pos.z],
                normal: [x, y, z],
                color,
            });
        }
    }

    for lat in 0..segments {
        for lon in 0..segments {
            let first = lat * (segments + 1) + lon;
            let second = first + segments + 1;

            indices.push(first);
            indices.push(second);
            indices.push(first + 1);

            indices.push(second);
            indices.push(second + 1);
            indices.push(first + 1);
        }
    }

    Mesh { vertices, indices }
}
