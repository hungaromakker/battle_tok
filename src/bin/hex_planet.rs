//! Hexagonal Terrain Demo
//!
//! Run with: `cargo run --bin sdf-demo`
//!
//! A mesh-based hexagonal terrain demo with elevations and free-view camera.
//! Uses Windows-style camera controls (non-reversed mouse look).
//!
//! Controls:
//! - WASD: Move camera
//! - Mouse right-drag: Look around (FPS style)
//! - Space: Move up
//! - Shift: Move down (or sprint when moving)
//! - Scroll: Zoom in/out
//! - R: Reset camera
//! - ESC: Exit

use std::sync::Arc;
use std::time::Instant;

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowAttributes, WindowId};

// ============================================================================
// DATA STRUCTURES
// ============================================================================

/// Vertex for hexagonal terrain mesh
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct HexVertex {
    position: [f32; 3],
    normal: [f32; 3],
    color: [f32; 4],
}

/// Uniforms sent to GPU
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct Uniforms {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 3],
    time: f32,
    sun_dir: [f32; 3],
    fog_density: f32,
    fog_color: [f32; 3],
    ambient: f32,
}

/// Simple free-view camera
struct Camera {
    position: Vec3,
    yaw: f32,   // Horizontal angle (radians)
    pitch: f32, // Vertical angle (radians)
    move_speed: f32,
    look_sensitivity: f32,
    fov: f32,
    near: f32,
    far: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            // Position camera outside the sphere, looking at center
            // Planet radius 1000, position ~2x radius away
            position: Vec3::new(0.0, 500.0, 2200.0),
            yaw: 0.0, // Looking toward -Z (toward planet center)
            pitch: -0.15, // Slight downward angle
            move_speed: 200.0, // Fast movement for planet scale
            look_sensitivity: 0.003,
            fov: 55.0_f32.to_radians(),
            near: 1.0,
            far: 6000.0, // Far enough to see whole planet
        }
    }
}

impl Camera {
    fn get_forward(&self) -> Vec3 {
        Vec3::new(
            self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
        .normalize()
    }

    fn get_right(&self) -> Vec3 {
        self.get_forward().cross(Vec3::Y).normalize()
    }

    fn get_view_matrix(&self) -> Mat4 {
        let target = self.position + self.get_forward();
        Mat4::look_at_rh(self.position, target, Vec3::Y)
    }

    fn get_projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect, self.near, self.far)
    }

    /// Handle mouse look - Windows style (non-reversed)
    fn handle_mouse_look(&mut self, delta_x: f32, delta_y: f32) {
        // Windows/FPS style: mouse right = look right (yaw increases)
        self.yaw += delta_x * self.look_sensitivity;
        // Mouse up = look up (pitch increases, but Y is inverted on screen)
        self.pitch -= delta_y * self.look_sensitivity;

        // Clamp pitch to prevent camera flip
        let pitch_limit = 89.0_f32.to_radians();
        self.pitch = self.pitch.clamp(-pitch_limit, pitch_limit);
    }

    fn update_movement(&mut self, forward: f32, right: f32, up: f32, delta_time: f32, sprint: bool) {
        let speed = if sprint {
            self.move_speed * 2.5
        } else {
            self.move_speed
        };

        let forward_dir = self.get_forward();
        let right_dir = self.get_right();

        // Horizontal movement (XZ plane)
        let forward_xz = Vec3::new(forward_dir.x, 0.0, forward_dir.z).normalize_or_zero();
        let right_xz = Vec3::new(right_dir.x, 0.0, right_dir.z).normalize_or_zero();

        self.position += forward_xz * forward * speed * delta_time;
        self.position += right_xz * right * speed * delta_time;
        self.position.y += up * speed * delta_time;
    }

    fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Movement key state
#[derive(Default)]
struct MovementKeys {
    forward: bool,
    backward: bool,
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    sprint: bool,
}

// ============================================================================
// MESH DATA STRUCTURES
// ============================================================================

/// Intermediate mesh representation for combining meshes
struct Mesh {
    vertices: Vec<HexVertex>,
    indices: Vec<u32>,
}

impl Mesh {
    fn new() -> Self {
        Self { vertices: Vec::new(), indices: Vec::new() }
    }
    
    fn merge(&mut self, other: &Mesh) {
        let base_idx = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&other.vertices);
        self.indices.extend(other.indices.iter().map(|i| i + base_idx));
    }
}

// ============================================================================
// HEX PLANET - CLEAN GEODESIC DUAL ALGORITHM
// ============================================================================

/// Generate hex planet using Conway dual of icosphere
/// Creates 12 pentagons (at icosahedron vertices) + hexagons everywhere else
fn generate_hex_planet(radius: f32, subdivisions: u32, extrusion: f32, bevel_ratio: f32) -> Mesh {
    let n = subdivisions.max(1);
    
    // Step 1: Generate subdivided icosphere
    let (ico_verts, ico_faces) = generate_icosphere(n);
    
    // Step 2: Compute face centroids (these become hex/pentagon centers in dual)
    let face_centers: Vec<Vec3> = ico_faces.iter().map(|face| {
        let centroid = (ico_verts[face[0]] + ico_verts[face[1]] + ico_verts[face[2]]) / 3.0;
        centroid.normalize()
    }).collect();
    
    // Step 3: Build adjacency - for each icosphere vertex, find surrounding faces
    let vertex_to_faces = build_vertex_adjacency(&ico_verts, &ico_faces);
    
    // Step 4: Create extruded hex/pentagon tiles
    let mut mesh = Mesh::new();
    
    for (vert_idx, adjacent_faces) in vertex_to_faces.iter().enumerate() {
        if adjacent_faces.is_empty() { continue; }
        
        let center_dir = ico_verts[vert_idx];
        
        // Get polygon corners from adjacent face centers
        let mut corners: Vec<Vec3> = adjacent_faces.iter()
            .map(|&fi| face_centers[fi])
            .collect();
        
        // Sort corners by angle for correct winding
        sort_polygon_corners(&mut corners, center_dir);
        
        // Generate tile color based on position
        let tile_color = get_tile_color(center_dir, vert_idx);
        
        // Create extruded tile
        create_extruded_tile(&mut mesh, center_dir, &corners, radius, extrusion, bevel_ratio, tile_color);
    }
    
    mesh
}

/// Generate subdivided icosphere (base for dual)
fn generate_icosphere(subdivisions: u32) -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let phi = (1.0 + 5.0_f32.sqrt()) / 2.0;
    
    // 12 icosahedron vertices
    let mut verts: Vec<Vec3> = vec![
        Vec3::new(-1.0, phi, 0.0).normalize(),
        Vec3::new(1.0, phi, 0.0).normalize(),
        Vec3::new(-1.0, -phi, 0.0).normalize(),
        Vec3::new(1.0, -phi, 0.0).normalize(),
        Vec3::new(0.0, -1.0, phi).normalize(),
        Vec3::new(0.0, 1.0, phi).normalize(),
        Vec3::new(0.0, -1.0, -phi).normalize(),
        Vec3::new(0.0, 1.0, -phi).normalize(),
        Vec3::new(phi, 0.0, -1.0).normalize(),
        Vec3::new(phi, 0.0, 1.0).normalize(),
        Vec3::new(-phi, 0.0, -1.0).normalize(),
        Vec3::new(-phi, 0.0, 1.0).normalize(),
    ];
    
    // 20 icosahedron faces
    let mut faces: Vec<[usize; 3]> = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    
    // Subdivide
    for _ in 0..subdivisions {
        let mut new_faces = Vec::new();
        let mut edge_midpoints: std::collections::HashMap<(usize, usize), usize> = 
            std::collections::HashMap::new();
        
        for face in &faces {
            let (v0, v1, v2) = (face[0], face[1], face[2]);
            let m01 = get_midpoint(&mut verts, &mut edge_midpoints, v0, v1);
            let m12 = get_midpoint(&mut verts, &mut edge_midpoints, v1, v2);
            let m20 = get_midpoint(&mut verts, &mut edge_midpoints, v2, v0);
            
            new_faces.push([v0, m01, m20]);
            new_faces.push([v1, m12, m01]);
            new_faces.push([v2, m20, m12]);
            new_faces.push([m01, m12, m20]);
        }
        faces = new_faces;
    }
    
    (verts, faces)
}

fn get_midpoint(
    verts: &mut Vec<Vec3>,
    cache: &mut std::collections::HashMap<(usize, usize), usize>,
    a: usize, b: usize
) -> usize {
    let key = if a < b { (a, b) } else { (b, a) };
    if let Some(&idx) = cache.get(&key) { return idx; }
    
    let mid = ((verts[a] + verts[b]) / 2.0).normalize();
    let idx = verts.len();
    verts.push(mid);
    cache.insert(key, idx);
    idx
}

fn build_vertex_adjacency(verts: &[Vec3], faces: &[[usize; 3]]) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); verts.len()];
    for (fi, face) in faces.iter().enumerate() {
        for &vi in face {
            if !adj[vi].contains(&fi) { adj[vi].push(fi); }
        }
    }
    adj
}

fn sort_polygon_corners(corners: &mut Vec<Vec3>, center: Vec3) {
    if corners.len() < 3 { return; }
    
    let up = if center.y.abs() < 0.99 { Vec3::Y } else { Vec3::X };
    let t1 = center.cross(up).normalize();
    let t2 = center.cross(t1).normalize();
    
    corners.sort_by(|a, b| {
        let da = *a - center;
        let db = *b - center;
        let angle_a = da.dot(t2).atan2(da.dot(t1));
        let angle_b = db.dot(t2).atan2(db.dot(t1));
        angle_a.partial_cmp(&angle_b).unwrap_or(std::cmp::Ordering::Equal)
    });
}

/// Get tile color - neutral stone with some variation
fn get_tile_color(dir: Vec3, idx: usize) -> [f32; 4] {
    // Hash-based variation for visual interest
    let hash = ((idx * 2654435761) % 1000) as f32 / 1000.0;
    
    // Latitude-based coloring
    let lat = dir.y.abs();
    
    if lat > 0.85 {
        // Polar caps - white/ice
        [0.9 + hash * 0.05, 0.92 + hash * 0.05, 0.95, 1.0]
    } else if lat > 0.6 {
        // Tundra - light grey-blue
        [0.65 + hash * 0.1, 0.68 + hash * 0.1, 0.72 + hash * 0.1, 1.0]
    } else if lat < 0.2 {
        // Equatorial - tan/desert
        [0.75 + hash * 0.1, 0.65 + hash * 0.1, 0.45 + hash * 0.1, 1.0]
    } else {
        // Temperate - neutral stone grey
        [0.55 + hash * 0.15, 0.52 + hash * 0.15, 0.48 + hash * 0.15, 1.0]
    }
}

/// Create an extruded polygon tile with beveled edges
fn create_extruded_tile(
    mesh: &mut Mesh,
    center_dir: Vec3,
    corners: &[Vec3],
    radius: f32,
    extrusion: f32,
    bevel_ratio: f32,
    color: [f32; 4],
) {
    let n = corners.len();
    if n < 3 { return; }
    
    let base_idx = mesh.vertices.len() as u32;
    
    // Colors for different parts
    let edge_color = [color[0] * 0.5, color[1] * 0.5, color[2] * 0.5, 1.0];
    // Rocky crust color - dark brown/grey rock (NOT void black)
    let crust_color = [0.18, 0.12, 0.08, 1.0]; // Dark brown rock
    
    // Compute tile geometry
    let outer_radius = radius + extrusion;
    let inner_radius = radius - extrusion * 0.5; // Tiles go down into crust
    let shrink = 1.0 - bevel_ratio;
    
    // === TOP SURFACE CENTER ===
    let top_center = center_dir * outer_radius;
    mesh.vertices.push(HexVertex {
        position: top_center.into(),
        normal: center_dir.into(),
        color,
    });
    
    // === TOP SURFACE CORNERS (shrunk for gap) ===
    for corner in corners {
        let dir = slerp(center_dir, *corner, shrink * 0.9);
        let pos = dir * outer_radius;
        mesh.vertices.push(HexVertex {
            position: pos.into(),
            normal: dir.into(),
            color,
        });
    }
    
    // === BEVEL EDGE (outer edge, slightly lower) ===
    for corner in corners {
        let dir = slerp(center_dir, *corner, shrink);
        let pos = dir * outer_radius - center_dir * (extrusion * 0.25);
        mesh.vertices.push(HexVertex {
            position: pos.into(),
            normal: dir.into(),
            color: edge_color,
        });
    }
    
    // === CRUST LAYER (rocky sides going down) ===
    for corner in corners {
        let dir = slerp(center_dir, *corner, shrink * 0.98);
        let pos = dir * inner_radius;
        mesh.vertices.push(HexVertex {
            position: pos.into(),
            normal: dir.into(), // Outward facing for visibility
            color: crust_color,
        });
    }
    
    let n = n as u32;
    
    // === INDEX GENERATION ===
    // Top surface fan
    for i in 0..n {
        let next = (i + 1) % n;
        mesh.indices.push(base_idx);           // center
        mesh.indices.push(base_idx + 1 + i);
        mesh.indices.push(base_idx + 1 + next);
    }
    
    // Bevel strip (top corners to bevel edge)
    for i in 0..n {
        let next = (i + 1) % n;
        let top = base_idx + 1 + i;
        let top_next = base_idx + 1 + next;
        let bevel = base_idx + 1 + n + i;
        let bevel_next = base_idx + 1 + n + next;
        
        mesh.indices.push(top);
        mesh.indices.push(bevel);
        mesh.indices.push(top_next);
        
        mesh.indices.push(top_next);
        mesh.indices.push(bevel);
        mesh.indices.push(bevel_next);
    }
    
    // Side walls (bevel to bottom)
    for i in 0..n {
        let next = (i + 1) % n;
        let bevel = base_idx + 1 + n + i;
        let bevel_next = base_idx + 1 + n + next;
        let bottom = base_idx + 1 + n * 2 + i;
        let bottom_next = base_idx + 1 + n * 2 + next;
        
        mesh.indices.push(bevel);
        mesh.indices.push(bottom);
        mesh.indices.push(bevel_next);
        
        mesh.indices.push(bevel_next);
        mesh.indices.push(bottom);
        mesh.indices.push(bottom_next);
    }
}

/// Spherical linear interpolation
fn slerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let dot = a.dot(b).clamp(-1.0, 1.0);
    let theta = dot.acos();
    
    if theta.abs() < 0.001 {
        return (a * (1.0 - t) + b * t).normalize();
    }
    
    let sin_theta = theta.sin();
    let wa = ((1.0 - t) * theta).sin() / sin_theta;
    let wb = (t * theta).sin() / sin_theta;
    (a * wa + b * wb).normalize()
}

// Legacy wrapper for compatibility (kept for potential future use)
#[allow(dead_code)]
fn generate_hex_sphere(radius: f32, subdivisions: u32) -> (Vec<HexVertex>, Vec<u32>) {
    let mesh = generate_hex_planet(radius, subdivisions, 8.0, 0.05);
    (mesh.vertices, mesh.indices)
}

// ============================================================================
// FLOATING ISLAND GENERATOR
// ============================================================================

/// Generate a floating island with tapered rock base and grass top
#[allow(dead_code)]
fn generate_floating_island(center: Vec3, size: Vec3) -> Mesh {
    let mut mesh = Mesh::new();
    
    // Generate tapered rock base
    generate_island_rock(&mut mesh, center, size);
    
    // Generate grass plane on top
    generate_grass_plane(&mut mesh, center + Vec3::Y * size.y * 0.5, size.x, size.z);
    
    mesh
}

/// Generate the rock base of a floating island (sphere with flat top, tapered bottom)
#[allow(dead_code)]
fn generate_island_rock(mesh: &mut Mesh, center: Vec3, size: Vec3) {
    let base_idx = mesh.vertices.len() as u32;
    let segments = 24;
    let rings = 16;
    
    let rock_color = [0.45, 0.40, 0.35, 1.0];
    let dark_rock = [0.25, 0.22, 0.20, 1.0];
    
    // Generate vertices for tapered rock
    for ring in 0..=rings {
        let v = ring as f32 / rings as f32;
        let y = 0.5 - v; // Top to bottom
        
        // Radius varies: full at top, tapered to point at bottom
        let taper = if y > 0.0 {
            1.0 // Full radius above center
        } else {
            (1.0 + y * 2.0).max(0.05) // Taper below
        };
        
        for seg in 0..=segments {
            let u = seg as f32 / segments as f32;
            let theta = u * std::f32::consts::TAU;
            
            // Add noise for rocky appearance
            let noise = ((theta * 5.0).sin() * 0.15 + (y * 8.0).sin() * 0.1) * taper;
            let r = taper + noise;
            
            let x = theta.cos() * r * size.x * 0.5;
            let z = theta.sin() * r * size.z * 0.5;
            let py = y * size.y;
            
            let pos = center + Vec3::new(x, py, z);
            let normal = Vec3::new(x, py * 0.5, z).normalize();
            
            // Darker at bottom
            let color = if y < -0.3 { dark_rock } else { rock_color };
            
            mesh.vertices.push(HexVertex {
                position: pos.into(),
                normal: normal.into(),
                color,
            });
        }
    }
    
    // Generate indices
    for ring in 0..rings {
        for seg in 0..segments {
            let curr = base_idx + ring * (segments + 1) + seg;
            let next = curr + 1;
            let below = curr + segments + 1;
            let below_next = below + 1;
            
            mesh.indices.push(curr);
            mesh.indices.push(below);
            mesh.indices.push(next);
            
            mesh.indices.push(next);
            mesh.indices.push(below);
            mesh.indices.push(below_next);
        }
    }
}

/// Generate grass plane on top of island
#[allow(dead_code)]
fn generate_grass_plane(mesh: &mut Mesh, center: Vec3, width: f32, depth: f32) {
    let base_idx = mesh.vertices.len() as u32;
    let grass_color = [0.35, 0.55, 0.25, 1.0];
    let grass_dark = [0.25, 0.45, 0.18, 1.0];
    
    let segments = 16;
    
    // Generate disc vertices
    mesh.vertices.push(HexVertex {
        position: center.into(),
        normal: [0.0, 1.0, 0.0],
        color: grass_color,
    });
    
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        let x = angle.cos() * width * 0.45;
        let z = angle.sin() * depth * 0.45;
        
        // Slight undulation
        let y = ((angle * 3.0).sin() * 0.5 + (angle * 7.0).cos() * 0.3) * 2.0;
        
        let pos = center + Vec3::new(x, y, z);
        let color = if i % 2 == 0 { grass_color } else { grass_dark };
        
        mesh.vertices.push(HexVertex {
            position: pos.into(),
            normal: [0.0, 1.0, 0.0],
            color,
        });
    }
    
    // Fan triangles
    for i in 0..segments as u32 {
        mesh.indices.push(base_idx);
        mesh.indices.push(base_idx + 1 + i);
        mesh.indices.push(base_idx + 2 + i);
    }
}

// ============================================================================
// FORTRESS GENERATOR
// ============================================================================

/// Generate complete fortress complex
#[allow(dead_code)]
fn generate_fortress(center: Vec3) -> Mesh {
    let mut mesh = Mesh::new();
    
    // Outer wall ring
    generate_wall_ring(&mut mesh, center, 120.0, 12.0, 18.0);
    
    // Corner towers (4)
    for i in 0..4 {
        let angle = (i as f32 / 4.0) * std::f32::consts::TAU + std::f32::consts::FRAC_PI_4;
        let tower_pos = center + Vec3::new(angle.cos() * 115.0, 0.0, angle.sin() * 115.0);
        generate_tower(&mut mesh, tower_pos, 18.0, 35.0);
    }
    
    // Central keep
    generate_keep(&mut mesh, center, Vec3::new(50.0, 40.0, 50.0));
    
    // Crystal pit in center
    generate_crystal_pit(&mut mesh, center, 25.0);
    
    mesh
}

/// Generate circular wall with crenellations
#[allow(dead_code)]
fn generate_wall_ring(mesh: &mut Mesh, center: Vec3, radius: f32, thickness: f32, height: f32) {
    let base_idx = mesh.vertices.len() as u32;
    let segments = 32;
    let wall_color = [0.5, 0.48, 0.45, 1.0];
    let dark_wall = [0.35, 0.33, 0.30, 1.0];
    
    // Inner and outer rings at bottom and top
    for ring in 0..4 {
        let is_outer = ring % 2 == 0;
        let is_top = ring >= 2;
        let r = if is_outer { radius + thickness * 0.5 } else { radius - thickness * 0.5 };
        let y = if is_top { height } else { 0.0 };
        
        for seg in 0..=segments {
            let angle = (seg as f32 / segments as f32) * std::f32::consts::TAU;
            let pos = center + Vec3::new(angle.cos() * r, y, angle.sin() * r);
            let normal = if is_outer {
                Vec3::new(angle.cos(), 0.0, angle.sin())
            } else {
                Vec3::new(-angle.cos(), 0.0, -angle.sin())
            };
            
            mesh.vertices.push(HexVertex {
                position: pos.into(),
                normal: normal.into(),
                color: if is_top { wall_color } else { dark_wall },
            });
        }
    }
    
    let verts_per_ring = (segments + 1) as u32;
    
    // Outer wall face
    for seg in 0..segments as u32 {
        let bot = base_idx + seg;
        let top = base_idx + verts_per_ring * 2 + seg;
        mesh.indices.push(bot);
        mesh.indices.push(top);
        mesh.indices.push(bot + 1);
        mesh.indices.push(bot + 1);
        mesh.indices.push(top);
        mesh.indices.push(top + 1);
    }
    
    // Inner wall face
    for seg in 0..segments as u32 {
        let bot = base_idx + verts_per_ring + seg;
        let top = base_idx + verts_per_ring * 3 + seg;
        mesh.indices.push(bot);
        mesh.indices.push(bot + 1);
        mesh.indices.push(top);
        mesh.indices.push(bot + 1);
        mesh.indices.push(top + 1);
        mesh.indices.push(top);
    }
    
    // Top walkway
    for seg in 0..segments as u32 {
        let outer = base_idx + verts_per_ring * 2 + seg;
        let inner = base_idx + verts_per_ring * 3 + seg;
        mesh.indices.push(outer);
        mesh.indices.push(inner);
        mesh.indices.push(outer + 1);
        mesh.indices.push(outer + 1);
        mesh.indices.push(inner);
        mesh.indices.push(inner + 1);
    }
    
    // Crenellations on top
    generate_crenellations(mesh, center, radius + thickness * 0.5, height, segments / 2);
}

/// Generate crenellations (merlons) on wall top
#[allow(dead_code)]
fn generate_crenellations(mesh: &mut Mesh, center: Vec3, radius: f32, wall_height: f32, count: u32) {
    let merlon_color = [0.52, 0.50, 0.47, 1.0];
    let merlon_height = 4.0;
    let merlon_width = 3.0;
    
    for i in 0..count {
        let angle = (i as f32 / count as f32) * std::f32::consts::TAU;
        let pos = center + Vec3::new(angle.cos() * radius, wall_height, angle.sin() * radius);
        generate_box(mesh, pos, Vec3::new(merlon_width, merlon_height, merlon_width), merlon_color);
    }
}

/// Generate a tower with cone roof
#[allow(dead_code)]
fn generate_tower(mesh: &mut Mesh, center: Vec3, radius: f32, height: f32) {
    let base_idx = mesh.vertices.len() as u32;
    let segments = 16;
    let tower_color = [0.48, 0.45, 0.42, 1.0];
    let roof_color = [0.35, 0.25, 0.20, 1.0];
    
    // Tower cylinder
    for ring in 0..2 {
        let y = ring as f32 * height;
        for seg in 0..=segments {
            let angle = (seg as f32 / segments as f32) * std::f32::consts::TAU;
            let pos = center + Vec3::new(angle.cos() * radius, y, angle.sin() * radius);
            let normal = Vec3::new(angle.cos(), 0.0, angle.sin());
            
            mesh.vertices.push(HexVertex {
                position: pos.into(),
                normal: normal.into(),
                color: tower_color,
            });
        }
    }
    
    // Cylinder indices
    let verts_per_ring = (segments + 1) as u32;
    for seg in 0..segments as u32 {
        let bot = base_idx + seg;
        let top = base_idx + verts_per_ring + seg;
        mesh.indices.push(bot);
        mesh.indices.push(top);
        mesh.indices.push(bot + 1);
        mesh.indices.push(bot + 1);
        mesh.indices.push(top);
        mesh.indices.push(top + 1);
    }
    
    // Cone roof
    let roof_base = mesh.vertices.len() as u32;
    let roof_height = height * 0.4;
    let apex = center + Vec3::Y * (height + roof_height);
    
    mesh.vertices.push(HexVertex {
        position: apex.into(),
        normal: [0.0, 1.0, 0.0],
        color: roof_color,
    });
    
    for seg in 0..=segments {
        let angle = (seg as f32 / segments as f32) * std::f32::consts::TAU;
        let pos = center + Vec3::new(angle.cos() * radius * 1.2, height, angle.sin() * radius * 1.2);
        let normal = Vec3::new(angle.cos(), 0.5, angle.sin()).normalize();
        
        mesh.vertices.push(HexVertex {
            position: pos.into(),
            normal: normal.into(),
            color: roof_color,
        });
    }
    
    // Cone indices
    for seg in 0..segments as u32 {
        mesh.indices.push(roof_base);
        mesh.indices.push(roof_base + 1 + seg);
        mesh.indices.push(roof_base + 2 + seg);
    }
}

/// Generate central keep structure
#[allow(dead_code)]
fn generate_keep(mesh: &mut Mesh, center: Vec3, size: Vec3) {
    let keep_color = [0.5, 0.47, 0.43, 1.0];
    generate_box(mesh, center + Vec3::Y * size.y * 0.5, size, keep_color);
    
    // Add smaller tower on top
    let tower_pos = center + Vec3::Y * size.y;
    generate_tower(mesh, tower_pos, size.x * 0.3, size.y * 0.6);
}

/// Generate crystal pit with glowing crystals
#[allow(dead_code)]
fn generate_crystal_pit(mesh: &mut Mesh, center: Vec3, radius: f32) {
    // Pit depression (dark ring)
    let pit_color = [0.1, 0.08, 0.08, 1.0];
    let segments = 16;
    let base_idx = mesh.vertices.len() as u32;
    
    // Pit rim
    mesh.vertices.push(HexVertex {
        position: (center - Vec3::Y * 5.0).into(),
        normal: [0.0, -1.0, 0.0],
        color: pit_color,
    });
    
    for seg in 0..=segments {
        let angle = (seg as f32 / segments as f32) * std::f32::consts::TAU;
        let pos = center + Vec3::new(angle.cos() * radius, -5.0, angle.sin() * radius);
        
        mesh.vertices.push(HexVertex {
            position: pos.into(),
            normal: [0.0, -1.0, 0.0],
            color: pit_color,
        });
    }
    
    for seg in 0..segments as u32 {
        mesh.indices.push(base_idx);
        mesh.indices.push(base_idx + 2 + seg);
        mesh.indices.push(base_idx + 1 + seg);
    }
    
    // Crystal spikes (glowing orange)
    let crystal_color = [1.0, 0.6, 0.1, 1.0]; // Emissive orange
    for i in 0..5 {
        let angle = (i as f32 / 5.0) * std::f32::consts::TAU + 0.3;
        let dist = radius * 0.4 * (0.5 + (i as f32 * 0.2));
        let crystal_pos = center + Vec3::new(angle.cos() * dist, -3.0, angle.sin() * dist);
        let crystal_height = 8.0 + (i as f32 * 2.0);
        generate_crystal_spike(mesh, crystal_pos, crystal_height, 2.0, crystal_color);
    }
}

/// Generate a crystal spike (4-sided pyramid)
#[allow(dead_code)]
fn generate_crystal_spike(mesh: &mut Mesh, base: Vec3, height: f32, width: f32, color: [f32; 4]) {
    let base_idx = mesh.vertices.len() as u32;
    let apex = base + Vec3::Y * height;
    
    // Apex
    mesh.vertices.push(HexVertex {
        position: apex.into(),
        normal: [0.0, 1.0, 0.0],
        color,
    });
    
    // Base corners
    for i in 0..4 {
        let angle = (i as f32 / 4.0) * std::f32::consts::TAU + std::f32::consts::FRAC_PI_4;
        let pos = base + Vec3::new(angle.cos() * width, 0.0, angle.sin() * width);
        let normal = Vec3::new(angle.cos(), 0.5, angle.sin()).normalize();
        
        mesh.vertices.push(HexVertex {
            position: pos.into(),
            normal: normal.into(),
            color,
        });
    }
    
    // Faces
    for i in 0..4u32 {
        let next = (i + 1) % 4;
        mesh.indices.push(base_idx);
        mesh.indices.push(base_idx + 1 + i);
        mesh.indices.push(base_idx + 1 + next);
    }
}

/// Generate a simple box
#[allow(dead_code)]
fn generate_box(mesh: &mut Mesh, center: Vec3, size: Vec3, color: [f32; 4]) {
    let half = size * 0.5;
    
    // 8 corners
    let corners = [
        Vec3::new(-half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, -half.z),
        Vec3::new(half.x, -half.y, half.z),
        Vec3::new(-half.x, -half.y, half.z),
        Vec3::new(-half.x, half.y, -half.z),
        Vec3::new(half.x, half.y, -half.z),
        Vec3::new(half.x, half.y, half.z),
        Vec3::new(-half.x, half.y, half.z),
    ];
    
    // Face definitions: [v0, v1, v2, v3], normal
    let faces: [([usize; 4], Vec3); 6] = [
        ([0, 1, 2, 3], -Vec3::Y), // Bottom
        ([4, 7, 6, 5], Vec3::Y),  // Top
        ([0, 4, 5, 1], -Vec3::Z), // Front
        ([2, 6, 7, 3], Vec3::Z),  // Back
        ([0, 3, 7, 4], -Vec3::X), // Left
        ([1, 5, 6, 2], Vec3::X),  // Right
    ];
    
    for (verts, normal) in &faces {
        let fi = mesh.vertices.len() as u32;
        for &vi in verts {
            mesh.vertices.push(HexVertex {
                position: (center + corners[vi]).into(),
                normal: (*normal).into(),
                color,
            });
        }
        mesh.indices.push(fi);
        mesh.indices.push(fi + 1);
        mesh.indices.push(fi + 2);
        mesh.indices.push(fi);
        mesh.indices.push(fi + 2);
        mesh.indices.push(fi + 3);
    }
}

// ============================================================================
// PROPS GENERATOR
// ============================================================================

/// Generate a trebuchet (simple placeholder)
#[allow(dead_code)]
fn generate_trebuchet(center: Vec3) -> Mesh {
    let mut mesh = Mesh::new();
    let wood_color = [0.4, 0.3, 0.2, 1.0];
    
    // Base platform
    generate_box(&mut mesh, center, Vec3::new(8.0, 2.0, 12.0), wood_color);
    
    // Vertical supports
    generate_box(&mut mesh, center + Vec3::new(-3.0, 6.0, 0.0), Vec3::new(1.5, 10.0, 1.5), wood_color);
    generate_box(&mut mesh, center + Vec3::new(3.0, 6.0, 0.0), Vec3::new(1.5, 10.0, 1.5), wood_color);
    
    // Throwing arm
    generate_box(&mut mesh, center + Vec3::new(0.0, 10.0, 0.0), Vec3::new(1.0, 1.0, 18.0), wood_color);
    
    mesh
}

/// Generate soldier (simple capsule placeholder)
#[allow(dead_code)]
fn generate_soldier(center: Vec3) -> Mesh {
    let mut mesh = Mesh::new();
    let armor_color = [0.4, 0.4, 0.45, 1.0];
    let skin_color = [0.8, 0.65, 0.5, 1.0];
    
    // Body (capsule approximated as cylinder + spheres)
    generate_box(&mut mesh, center + Vec3::Y * 0.9, Vec3::new(0.6, 1.2, 0.4), armor_color);
    
    // Head
    let head_base = mesh.vertices.len() as u32;
    let head_pos = center + Vec3::Y * 1.8;
    let segments = 8;
    
    mesh.vertices.push(HexVertex {
        position: (head_pos + Vec3::Y * 0.3).into(),
        normal: [0.0, 1.0, 0.0],
        color: skin_color,
    });
    
    for seg in 0..=segments {
        let angle = (seg as f32 / segments as f32) * std::f32::consts::TAU;
        let pos = head_pos + Vec3::new(angle.cos() * 0.2, 0.0, angle.sin() * 0.2);
        
        mesh.vertices.push(HexVertex {
            position: pos.into(),
            normal: Vec3::new(angle.cos(), 0.0, angle.sin()).into(),
            color: skin_color,
        });
    }
    
    for seg in 0..segments as u32 {
        mesh.indices.push(head_base);
        mesh.indices.push(head_base + 1 + seg);
        mesh.indices.push(head_base + 2 + seg);
    }
    
    mesh
}

/// Generate distant castle silhouette on small rock
#[allow(dead_code)]
fn generate_distant_castle(center: Vec3) -> Mesh {
    let mut mesh = Mesh::new();
    
    // Small floating rock
    generate_island_rock(&mut mesh, center, Vec3::new(40.0, 30.0, 40.0));
    
    // Simple castle silhouette
    let castle_color = [0.3, 0.28, 0.25, 1.0];
    let castle_base = center + Vec3::Y * 15.0;
    
    // Main keep
    generate_box(&mut mesh, castle_base + Vec3::Y * 10.0, Vec3::new(15.0, 20.0, 15.0), castle_color);
    
    // Tower
    generate_tower(&mut mesh, castle_base + Vec3::new(10.0, 0.0, 10.0), 5.0, 25.0);
    
    mesh
}

// ============================================================================
// SCENE COMPOSITION
// ============================================================================

/// Generate the complete scene - hex planet with magma core and crust
/// (Fortresses/buildings are player-built on hex tiles, not part of base scene)
fn generate_full_scene(planet_radius: f32, planet_subdivisions: u32) -> Mesh {
    let mut scene = Mesh::new();
    
    // Extrusion height for tiles
    let tile_extrusion = 15.0;
    
    // 1. Inner magma/mantle sphere - fills the core, visible in cracks
    // Sits just below where tile crust bottoms end
    let magma_radius = planet_radius - tile_extrusion * 0.6;
    let magma = generate_magma_sphere(magma_radius, 48);
    scene.merge(&magma);
    
    // 2. Hex tiles (continental crust) with rocky sides
    // - extrusion: how thick the tiles are
    // - bevel_ratio: gap size (0.08 = 8% gap for visible magma cracks)
    let tiles = generate_hex_planet(planet_radius, planet_subdivisions, tile_extrusion, 0.08);
    scene.merge(&tiles);
    
    scene
}

/// Generate inner magma sphere - glowing molten core visible through tile cracks
fn generate_magma_sphere(radius: f32, segments: u32) -> Mesh {
    let mut mesh = Mesh::new();
    let rings = segments / 2;
    
    // Magma colors - bright orange/red emissive
    let magma_bright = [1.0, 0.4, 0.05, 1.0];  // Bright orange (emissive)
    let magma_dark = [0.8, 0.15, 0.02, 1.0];   // Darker red-orange
    
    // Generate sphere vertices
    for ring in 0..=rings {
        let phi = (ring as f32 / rings as f32) * std::f32::consts::PI;
        let y = phi.cos();
        let ring_radius = phi.sin();
        
        for seg in 0..=segments {
            let theta = (seg as f32 / segments as f32) * std::f32::consts::TAU;
            let x = ring_radius * theta.cos();
            let z = ring_radius * theta.sin();
            
            let pos = Vec3::new(x, y, z) * radius;
            let normal = Vec3::new(x, y, z).normalize();
            
            // Vary color based on position for more interesting magma look
            let noise = ((theta * 5.0).sin() * (phi * 7.0).cos() + 1.0) * 0.5;
            let color = if noise > 0.6 { magma_bright } else { magma_dark };
            
            mesh.vertices.push(HexVertex {
                position: pos.into(),
                normal: normal.into(),
                color,
            });
        }
    }
    
    // Generate indices
    let verts_per_ring = segments + 1;
    for ring in 0..rings {
        for seg in 0..segments {
            let curr = ring * verts_per_ring + seg;
            let next = curr + 1;
            let below = curr + verts_per_ring;
            let below_next = below + 1;
            
            // Two triangles per quad (inward-facing normals for inside view)
            mesh.indices.push(curr);
            mesh.indices.push(next);
            mesh.indices.push(below);
            
            mesh.indices.push(next);
            mesh.indices.push(below_next);
            mesh.indices.push(below);
        }
    }
    
    mesh
}

// ============================================================================
// BIOME/ELEVATION (kept for reference)
// ============================================================================

/// Generate elevation for a point on the sphere using 3D noise
#[allow(dead_code)]
fn sphere_elevation(pos: Vec3) -> f32 {
    let scale1 = 3.0;
    let scale2 = 6.0;
    
    let noise1 = (pos.x * scale1).sin() * (pos.y * scale1 + 1.0).cos() * (pos.z * scale1).sin();
    let noise2 = (pos.x * scale2 + 2.0).sin() * (pos.y * scale2).cos() * (pos.z * scale2 + 1.5).sin();
    
    let combined = noise1 * 3.0 + noise2 * 1.5;
    
    combined.clamp(-2.0, 4.0)
}

/// Map sphere position and elevation to biome color
#[allow(dead_code)]
fn sphere_biome_color(pos: Vec3, elevation: f32) -> [f32; 4] {
    let latitude = pos.y.abs();
    
    if elevation < -0.5 {
        // Ocean - blue
        [0.15, 0.35, 0.65, 1.0]
    } else if elevation < 0.5 {
        // Coastal/beach - light tan
        [0.85, 0.78, 0.55, 1.0]
    } else if latitude > 0.75 {
        // Polar - white/ice
        [0.92, 0.94, 0.96, 1.0]
    } else if elevation > 2.5 {
        // Mountain peaks - gray rock
        [0.55, 0.52, 0.5, 1.0]
    } else if latitude < 0.25 {
        // Tropical - bright green jungle
        [0.2, 0.55, 0.25, 1.0]
    } else if latitude < 0.5 {
        // Temperate - medium green
        [0.35, 0.5, 0.3, 1.0]
    } else {
        // Taiga - dark green
        [0.25, 0.4, 0.25, 1.0]
    }
}

// ============================================================================
// APPLICATION STATE
// ============================================================================

struct AppState {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,

    // Render pipeline
    terrain_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,

    // Buffers
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,

    // Depth buffer
    depth_texture: wgpu::TextureView,

    // Mesh data
    index_count: u32,

    // Planet parameters (adjustable at runtime)
    planet_radius: f32,
    hex_subdivisions: u32,
    
    // Camera
    camera: Camera,
    movement_keys: MovementKeys,

    // Input state
    right_mouse_down: bool,
    last_mouse_pos: Option<(f64, f64)>,

    // Timing
    start_time: Instant,
    last_frame_time: Instant,
    
    // FPS tracking
    frame_count: u32,
    fps_update_time: Instant,
    current_fps: f32,
}

impl AppState {
    async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        // Create wgpu instance
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Create surface
        let surface = instance.create_surface(window.clone()).unwrap();

        // Request adapter
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find suitable GPU adapter");

        println!("[SDF Demo] Using GPU: {}", adapter.get_info().name);

        // Request device
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .expect("Failed to create device");

        // Configure surface
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Load shader
        let shader_source = include_str!("../../shaders/hex_terrain.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Hex Terrain Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Uniform Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create depth texture
        let depth_texture = create_depth_texture(&device, &config);

        // Create terrain render pipeline
        let terrain_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Terrain Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<HexVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        // position
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        // normal
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        // color
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Disable culling to see both sides
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create sky render pipeline (no vertex buffer, full-screen triangle)
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sky Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_sky"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_sky"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // Sky doesn't write depth
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Generate full scene: hex planet + floating island + fortress + props
        println!("[SDF Demo] Generating full scene...");
        let planet_radius = 1000.0;  // 1000 unit radius planet
        let hex_subdivisions = 3;    // ~640 hex tiles at level 3
        
        let scene = generate_full_scene(planet_radius, hex_subdivisions);
        let vertices = scene.vertices;
        let indices = scene.indices;
        
        let num_tiles = 10 * 4_u32.pow(hex_subdivisions) + 2;
        println!(
            "[SDF Demo] Scene: {} hex tiles + island + fortress + props",
            num_tiles
        );
        println!("[SDF Demo] Total: {} vertices, {} triangles", vertices.len(), indices.len() / 3);

        // Create vertex buffer - allocate larger size for full scene
        // Planet + island + fortress + props needs ~50-100MB at high detail
        let max_buffer_size = 64_000_000u64; // 64MB for full scene
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vertex Buffer"),
            size: max_buffer_size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&vertex_buffer, 0, bytemuck::cast_slice(&vertices));

        // Create index buffer - larger for dynamic adjustment
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Index Buffer"),
            size: max_buffer_size,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&index_buffer, 0, bytemuck::cast_slice(&indices));

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Uniform Buffer"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Uniform Bind Group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let now = Instant::now();

        Self {
            window,
            surface,
            device,
            queue,
            config,
            terrain_pipeline,
            sky_pipeline,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            bind_group,
            depth_texture,
            index_count: indices.len() as u32,
            planet_radius,
            hex_subdivisions,
            camera: Camera::default(),
            movement_keys: MovementKeys::default(),
            right_mouse_down: false,
            last_mouse_pos: None,
            start_time: now,
            last_frame_time: now,
            frame_count: 0,
            fps_update_time: now,
            current_fps: 0.0,
        }
    }

    /// Regenerate the full scene with current settings
    fn regenerate_planet(&mut self) {
        let scene = generate_full_scene(self.planet_radius, self.hex_subdivisions);
        
        let num_tiles = 10 * 4_u32.pow(self.hex_subdivisions) + 2;
        println!(
            "[Regenerated] {} tiles + scene ({} verts, {} tris) - subdiv: {}",
            num_tiles, scene.vertices.len(), scene.indices.len() / 3, self.hex_subdivisions
        );
        
        // Update buffers
        self.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&scene.vertices));
        self.queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&scene.indices));
        self.index_count = scene.indices.len() as u32;
    }

    /// Adjust hex count and regenerate
    fn adjust_hex_count(&mut self, delta: i32) {
        // Cap at 5 - grows exponentially: 4^5 = 10k tiles, 4^6 = 41k tiles (too many)
        let new_subdivisions = (self.hex_subdivisions as i32 + delta).clamp(1, 5) as u32;
        if new_subdivisions != self.hex_subdivisions {
            self.hex_subdivisions = new_subdivisions;
            self.regenerate_planet();
            self.print_status();
        }
    }
    
    /// Adjust planet radius and regenerate
    fn adjust_planet_size(&mut self, delta: f32) {
        let new_radius = (self.planet_radius + delta).clamp(100.0, 2000.0);
        if (new_radius - self.planet_radius).abs() > 0.1 {
            self.planet_radius = new_radius;
            self.regenerate_planet();
            self.print_status();
        }
    }
    
    /// Print current status
    fn print_status(&self) {
        // Icosphere subdivision: vertices = 10 * 4^n + 2
        // Each vertex becomes a hex/pentagon tile in the dual
        let n = self.hex_subdivisions;
        let num_tiles = 10 * 4_u32.pow(n) + 2;
        
        // Calculate approximate hex size in meters
        let surface_area = 4.0 * std::f32::consts::PI * self.planet_radius * self.planet_radius;
        let tile_area = surface_area / num_tiles as f32;
        let hex_size = (tile_area / 2.6).sqrt() * 2.0; // diameter
        
        println!(
            "[Status] Tiles: {} | Planet: {:.0}m | Hex: ~{:.0}m | FPS: {:.0}",
            num_tiles, self.planet_radius, hex_size, self.current_fps
        );
    }

    fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = create_depth_texture(&self.device, &self.config);
        }
    }

    fn update(&mut self) {
        let now = Instant::now();
        let delta_time = (now - self.last_frame_time).as_secs_f32();
        self.last_frame_time = now;
        
        // FPS tracking
        self.frame_count += 1;
        let fps_elapsed = (now - self.fps_update_time).as_secs_f32();
        if fps_elapsed >= 1.0 {
            self.current_fps = self.frame_count as f32 / fps_elapsed;
            self.frame_count = 0;
            self.fps_update_time = now;
            
            // Update window title with FPS
            let num_tiles = 10 * 4_u32.pow(self.hex_subdivisions) + 2;
            self.window.set_title(&format!(
                "Hex Planet | FPS: {:.0} | Tiles: {} | Radius: {:.0}m | Subdiv: {}",
                self.current_fps, num_tiles, self.planet_radius, self.hex_subdivisions
            ));
        }

        // Update camera movement
        let forward = if self.movement_keys.forward { 1.0 } else { 0.0 }
            - if self.movement_keys.backward { 1.0 } else { 0.0 };
        let right = if self.movement_keys.right { 1.0 } else { 0.0 }
            - if self.movement_keys.left { 1.0 } else { 0.0 };
        let up = if self.movement_keys.up { 1.0 } else { 0.0 }
            - if self.movement_keys.down { 1.0 } else { 0.0 };

        self.camera
            .update_movement(forward, right, up, delta_time, self.movement_keys.sprint);
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Calculate matrices
        let aspect = self.config.width as f32 / self.config.height as f32;
        let view_matrix = self.camera.get_view_matrix();
        let proj_matrix = self.camera.get_projection_matrix(aspect);
        let view_proj = proj_matrix * view_matrix;

        // Update uniforms
        let elapsed = self.start_time.elapsed().as_secs_f32();
        let uniforms = Uniforms {
            view_proj: view_proj.to_cols_array_2d(),
            camera_pos: self.camera.position.into(),
            time: elapsed,
            sun_dir: Vec3::new(0.4, 0.8, 0.3).normalize().into(), // Sun from upper-right
            fog_density: 0.0001, // Very light fog for space view
            fog_color: [0.01, 0.01, 0.02], // Very dark space background
            ambient: 0.3, // Decent ambient for visibility
        };
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        // Create command encoder
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.01,
                            g: 0.01,
                            b: 0.03,
                            a: 1.0,
                        }), // Dark space background
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // Draw starfield sky first
            render_pass.set_pipeline(&self.sky_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.draw(0..3, 0..1);

            // Draw planet
            render_pass.set_pipeline(&self.terrain_pipeline);
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..self.index_count, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    fn handle_key(&mut self, key: KeyCode, pressed: bool) {
        match key {
            KeyCode::KeyW => self.movement_keys.forward = pressed,
            KeyCode::KeyS => self.movement_keys.backward = pressed,
            KeyCode::KeyA => self.movement_keys.left = pressed,
            KeyCode::KeyD => self.movement_keys.right = pressed,
            KeyCode::Space => self.movement_keys.up = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                self.movement_keys.down = pressed;
                self.movement_keys.sprint = pressed;
            }
            KeyCode::KeyR if pressed => {
                self.camera.reset();
                println!("[SDF Demo] Camera reset");
            }
            // Hex count adjustment: +/- or Up/Down arrows
            KeyCode::Equal | KeyCode::NumpadAdd | KeyCode::ArrowUp if pressed => {
                self.adjust_hex_count(1);
            }
            KeyCode::Minus | KeyCode::NumpadSubtract | KeyCode::ArrowDown if pressed => {
                self.adjust_hex_count(-1);
            }
            // Page Up/Down for larger jumps
            KeyCode::PageUp if pressed => {
                self.adjust_hex_count(3);
            }
            KeyCode::PageDown if pressed => {
                self.adjust_hex_count(-3);
            }
            // Planet size: Numpad 6 (decrease) / Numpad 9 (increase)
            KeyCode::Numpad9 if pressed => {
                self.adjust_planet_size(10.0); // Increase radius by 10m
            }
            KeyCode::Numpad6 if pressed => {
                self.adjust_planet_size(-10.0); // Decrease radius by 10m
            }
            _ => {}
        }
    }

    fn handle_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if button == MouseButton::Right {
            self.right_mouse_down = pressed;

            // Grab cursor when right mouse is held for smooth look
            if pressed {
                let _ = self.window.set_cursor_grab(CursorGrabMode::Confined);
                self.window.set_cursor_visible(false);
            } else {
                let _ = self.window.set_cursor_grab(CursorGrabMode::None);
                self.window.set_cursor_visible(true);
                self.last_mouse_pos = None;
            }
        }
    }

    fn handle_mouse_move(&mut self, x: f64, y: f64) {
        if self.right_mouse_down {
            if let Some((last_x, last_y)) = self.last_mouse_pos {
                let delta_x = (x - last_x) as f32;
                let delta_y = (y - last_y) as f32;
                self.camera.handle_mouse_look(delta_x, delta_y);
            }
        }
        self.last_mouse_pos = Some((x, y));
    }

    fn handle_scroll(&mut self, delta: MouseScrollDelta) {
        let scroll_amount = match delta {
            MouseScrollDelta::LineDelta(_, y) => y * 2.0,
            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 * 0.1,
        };

        // Scroll zooms camera forward/backward
        let forward = self.camera.get_forward();
        self.camera.position += forward * scroll_amount;
    }
}

fn create_depth_texture(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Depth Texture"),
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

// ============================================================================
// APPLICATION HANDLER
// ============================================================================

struct App {
    state: Option<AppState>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_some() {
            return;
        }

        println!("[SDF Demo] Creating window...");
        let window_attrs = WindowAttributes::default()
            .with_title("Hexagonal Terrain Demo - WASD to move, Right-drag to look")
            .with_inner_size(PhysicalSize::new(1280, 720));

        let window = Arc::new(event_loop.create_window(window_attrs).unwrap());
        self.state = Some(pollster::block_on(AppState::new(window)));

        let s = self.state.as_ref().unwrap();
        s.print_status();
        println!("[SDF Demo] Ready! Controls:");
        println!("  WASD - Move camera");
        println!("  Right-drag - Look around");
        println!("  Space/Shift - Up/Down (Shift also = Sprint)");
        println!("  Scroll - Zoom");
        println!("  +/- or Up/Down - Adjust hex tile count");
        println!("  PageUp/PageDown - Large hex count adjustment");
        println!("  Numpad 6/9 - Adjust planet size");
        println!("  R - Reset camera");
        println!("  ESC - Exit");
        println!("");
        println!("FPS shown in window title. Adjust tiles/size to find optimal!");
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(state) = &mut self.state else {
            return;
        };

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                state.resize(new_size);
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key: PhysicalKey::Code(key),
                        state: key_state,
                        ..
                    },
                ..
            } => {
                let pressed = key_state == ElementState::Pressed;

                if key == KeyCode::Escape && pressed {
                    event_loop.exit();
                    return;
                }

                state.handle_key(key, pressed);
            }
            WindowEvent::MouseInput { button, state: btn_state, .. } => {
                state.handle_mouse_button(button, btn_state == ElementState::Pressed);
            }
            WindowEvent::CursorMoved { position, .. } => {
                state.handle_mouse_move(position.x, position.y);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                state.handle_scroll(delta);
            }
            WindowEvent::RedrawRequested => {
                state.update();

                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.window.inner_size()),
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(e) => eprintln!("Render error: {:?}", e),
                }

                state.window.request_redraw();
            }
            _ => {}
        }
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    println!("=== Hexagonal Terrain Demo ===");

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App { state: None };
    event_loop.run_app(&mut app).unwrap();
}
