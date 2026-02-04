//! Building Block Shapes (Phase 2)
//!
//! Provides various SDF-based building primitives for construction.
//! Each shape has:
//! - SDF function for merging operations
//! - Mesh generation for rendering
//! - AABB for collision detection
//!
//! # Shapes
//!
//! - **Cube**: Standard voxel block with configurable size
//! - **Cylinder**: Vertical pillars and columns
//! - **Sphere**: Round objects and domes
//! - **Dome**: Half-sphere for roofs
//! - **Arch**: For doorways and windows
//! - **Wedge**: For ramps and roof slopes

use bytemuck::{Pod, Zeroable};
use glam::{Vec3, Quat};
use std::f32::consts::PI;

// ============================================================================
// SHAPE ENUM AND BUILDING BLOCK STRUCT
// ============================================================================

/// Types of building block shapes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BuildingBlockShape {
    /// Standard box/cube with half-extents (half-width, half-height, half-depth)
    Cube { half_extents: Vec3 },
    /// Cylinder with radius and height
    Cylinder { radius: f32, height: f32 },
    /// Sphere with radius
    Sphere { radius: f32 },
    /// Half-sphere (dome) with radius
    Dome { radius: f32 },
    /// Arch with width, height, and depth (thickness)
    Arch { width: f32, height: f32, depth: f32 },
    /// Wedge/ramp with size (base width, height, depth)
    Wedge { size: Vec3 },
}

impl Default for BuildingBlockShape {
    fn default() -> Self {
        Self::Cube { half_extents: Vec3::splat(0.5) }
    }
}

/// A placed building block in the world
#[derive(Debug, Clone)]
pub struct BuildingBlock {
    /// Shape of the block
    pub shape: BuildingBlockShape,
    /// Position in world space
    pub position: Vec3,
    /// Rotation as quaternion
    pub rotation: Quat,
    /// Material index (for color/texture)
    pub material: u8,
    /// If part of a merged group, this holds the group ID
    pub merged_group_id: Option<u32>,
    /// Unique ID for this block
    pub id: u32,
}

impl BuildingBlock {
    /// Create a new building block
    pub fn new(shape: BuildingBlockShape, position: Vec3, material: u8) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        Self {
            shape,
            position,
            rotation: Quat::IDENTITY,
            material,
            merged_group_id: None,
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        }
    }
    
    /// Create a block with rotation
    pub fn with_rotation(mut self, rotation: Quat) -> Self {
        self.rotation = rotation;
        self
    }
    
    /// Calculate the AABB (Axis-Aligned Bounding Box) for this block
    pub fn aabb(&self) -> AABB {
        match self.shape {
            BuildingBlockShape::Cube { half_extents } => {
                // For rotated cubes, compute rotated corners and find bounds
                let corners = [
                    Vec3::new(-1.0, -1.0, -1.0),
                    Vec3::new( 1.0, -1.0, -1.0),
                    Vec3::new(-1.0,  1.0, -1.0),
                    Vec3::new( 1.0,  1.0, -1.0),
                    Vec3::new(-1.0, -1.0,  1.0),
                    Vec3::new( 1.0, -1.0,  1.0),
                    Vec3::new(-1.0,  1.0,  1.0),
                    Vec3::new( 1.0,  1.0,  1.0),
                ];
                
                let mut min = Vec3::splat(f32::MAX);
                let mut max = Vec3::splat(f32::MIN);
                
                for corner in corners {
                    let world_corner = self.position + self.rotation * (corner * half_extents);
                    min = min.min(world_corner);
                    max = max.max(world_corner);
                }
                
                AABB { min, max }
            }
            BuildingBlockShape::Cylinder { radius, height } => {
                // Conservative AABB for cylinder
                let half_height = height * 0.5;
                AABB {
                    min: self.position - Vec3::new(radius, half_height, radius),
                    max: self.position + Vec3::new(radius, half_height, radius),
                }
            }
            BuildingBlockShape::Sphere { radius } => {
                AABB {
                    min: self.position - Vec3::splat(radius),
                    max: self.position + Vec3::splat(radius),
                }
            }
            BuildingBlockShape::Dome { radius } => {
                // Dome extends from base to top
                AABB {
                    min: self.position - Vec3::new(radius, 0.0, radius),
                    max: self.position + Vec3::new(radius, radius, radius),
                }
            }
            BuildingBlockShape::Arch { width, height, depth } => {
                let half_width = width * 0.5;
                let half_depth = depth * 0.5;
                AABB {
                    min: self.position - Vec3::new(half_width, 0.0, half_depth),
                    max: self.position + Vec3::new(half_width, height, half_depth),
                }
            }
            BuildingBlockShape::Wedge { size } => {
                let half = size * 0.5;
                AABB {
                    min: self.position - half,
                    max: self.position + half,
                }
            }
        }
    }
    
    /// Evaluate the SDF at a world-space point
    pub fn sdf(&self, world_point: Vec3) -> f32 {
        // Transform point to local space
        let local_point = self.rotation.inverse() * (world_point - self.position);
        
        match self.shape {
            BuildingBlockShape::Cube { half_extents } => {
                sdf_box(local_point, half_extents)
            }
            BuildingBlockShape::Cylinder { radius, height } => {
                sdf_cylinder(local_point, radius, height)
            }
            BuildingBlockShape::Sphere { radius } => {
                sdf_sphere(local_point, radius)
            }
            BuildingBlockShape::Dome { radius } => {
                sdf_dome(local_point, radius)
            }
            BuildingBlockShape::Arch { width, height, depth } => {
                sdf_arch(local_point, width, height, depth)
            }
            BuildingBlockShape::Wedge { size } => {
                sdf_wedge(local_point, size)
            }
        }
    }
    
    /// Generate mesh vertices and indices for this block
    pub fn generate_mesh(&self) -> (Vec<BlockVertex>, Vec<u32>) {
        match self.shape {
            BuildingBlockShape::Cube { half_extents } => {
                mesh_box(self.position, self.rotation, half_extents, self.material)
            }
            BuildingBlockShape::Cylinder { radius, height } => {
                mesh_cylinder(self.position, self.rotation, radius, height, self.material, 16)
            }
            BuildingBlockShape::Sphere { radius } => {
                mesh_sphere(self.position, radius, self.material, 16, 12)
            }
            BuildingBlockShape::Dome { radius } => {
                mesh_dome(self.position, self.rotation, radius, self.material, 16, 8)
            }
            BuildingBlockShape::Arch { width, height, depth } => {
                mesh_arch(self.position, self.rotation, width, height, depth, self.material, 12)
            }
            BuildingBlockShape::Wedge { size } => {
                mesh_wedge(self.position, self.rotation, size, self.material)
            }
        }
    }
}

// ============================================================================
// AABB (Axis-Aligned Bounding Box)
// ============================================================================

/// Axis-aligned bounding box for collision detection
#[derive(Debug, Clone, Copy)]
pub struct AABB {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB {
    /// Create a new AABB
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }
    
    /// Check if two AABBs intersect
    pub fn intersects(&self, other: &AABB) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x &&
        self.min.y <= other.max.y && self.max.y >= other.min.y &&
        self.min.z <= other.max.z && self.max.z >= other.min.z
    }
    
    /// Check if a point is inside the AABB
    pub fn contains(&self, point: Vec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x &&
        point.y >= self.min.y && point.y <= self.max.y &&
        point.z >= self.min.z && point.z <= self.max.z
    }
    
    /// Expand AABB to include a point
    pub fn expand(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }
    
    /// Get the center of the AABB
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
    
    /// Get the size/extents of the AABB
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
}

// ============================================================================
// VERTEX FORMAT
// ============================================================================

/// Vertex for building block meshes
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable, Debug)]
pub struct BlockVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

impl BlockVertex {
    pub fn new(position: Vec3, normal: Vec3, color: [f32; 4]) -> Self {
        Self {
            position: position.to_array(),
            normal: normal.to_array(),
            color,
        }
    }
}

// ============================================================================
// SDF PRIMITIVES
// ============================================================================

/// SDF for a box (cube/rectangular prism)
pub fn sdf_box(p: Vec3, half_extents: Vec3) -> f32 {
    let q = p.abs() - half_extents;
    q.max(Vec3::ZERO).length() + q.max_element().min(0.0)
}

/// SDF for a cylinder (vertical, centered at origin)
pub fn sdf_cylinder(p: Vec3, radius: f32, height: f32) -> f32 {
    let half_height = height * 0.5;
    let d = Vec3::new(p.x, p.z, 0.0).length() - radius;
    let h = p.y.abs() - half_height;
    d.max(h).min(0.0) + Vec3::new(d.max(0.0), h.max(0.0), 0.0).length()
}

/// SDF for a sphere
pub fn sdf_sphere(p: Vec3, radius: f32) -> f32 {
    p.length() - radius
}

/// SDF for a dome (half-sphere, base at y=0)
pub fn sdf_dome(p: Vec3, radius: f32) -> f32 {
    // Upper half-sphere
    let sphere_d = sdf_sphere(p, radius);
    // Cut off bottom half
    let plane_d = -p.y;
    sphere_d.max(plane_d)
}

/// SDF for an arch (semicircular opening in a rectangular block)
pub fn sdf_arch(p: Vec3, width: f32, height: f32, depth: f32) -> f32 {
    let half_width = width * 0.5;
    let half_depth = depth * 0.5;
    let arch_radius = half_width;
    
    // Outer block
    let block_d = sdf_box(p - Vec3::new(0.0, height * 0.5, 0.0), Vec3::new(half_width, height * 0.5, half_depth));
    
    // Arch cutout (cylinder shape)
    let arch_center = Vec3::new(0.0, arch_radius, 0.0);
    let to_center = p - arch_center;
    let cylinder_d = Vec3::new(to_center.x, to_center.y, 0.0).length() - arch_radius;
    
    // Only cut where z is within depth
    let z_in = p.z.abs() - half_depth;
    let arch_cutout = cylinder_d.max(z_in);
    
    // Subtract arch from block
    block_d.max(-arch_cutout)
}

/// SDF for a wedge/ramp (triangular prism)
pub fn sdf_wedge(p: Vec3, size: Vec3) -> f32 {
    let half = size * 0.5;
    
    // The wedge goes from (0,0) at back to (size.y) at front
    // Using a plane to cut the box diagonally
    let box_d = sdf_box(p, half);
    
    // Diagonal plane: y - slope * z = 0, where slope = size.y / size.z
    let slope = size.y / size.z;
    let plane_d = p.y + slope * p.z - half.y;
    
    box_d.max(plane_d)
}

/// Smooth union of two SDFs (for blending shapes together)
pub fn sdf_smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
    d2 + (d1 - d2) * h - k * h * (1.0 - h)
}

/// Hard union (minimum)
pub fn sdf_union(d1: f32, d2: f32) -> f32 {
    d1.min(d2)
}

/// Hard intersection (maximum)
pub fn sdf_intersection(d1: f32, d2: f32) -> f32 {
    d1.max(d2)
}

/// Subtraction (d1 minus d2)
pub fn sdf_subtraction(d1: f32, d2: f32) -> f32 {
    d1.max(-d2)
}

// ============================================================================
// MESH GENERATION
// ============================================================================

/// Get color for a material index
fn material_color(material: u8) -> [f32; 4] {
    match material {
        0 => [0.6, 0.6, 0.6, 1.0],   // Stone gray
        1 => [0.7, 0.5, 0.3, 1.0],   // Wood brown
        2 => [0.4, 0.4, 0.45, 1.0],  // Stone dark
        3 => [0.8, 0.7, 0.5, 1.0],   // Sandstone
        4 => [0.3, 0.3, 0.35, 1.0],  // Slate
        5 => [0.6, 0.3, 0.2, 1.0],   // Brick red
        6 => [0.2, 0.4, 0.2, 1.0],   // Moss green
        7 => [0.5, 0.5, 0.6, 1.0],   // Metal gray
        8 => [0.9, 0.9, 0.85, 1.0],  // Marble white
        9 => [0.2, 0.2, 0.3, 1.0],   // Obsidian black
        _ => [0.5, 0.5, 0.5, 1.0],   // Default gray
    }
}

/// Generate mesh for a box
fn mesh_box(position: Vec3, rotation: Quat, half_extents: Vec3, material: u8) -> (Vec<BlockVertex>, Vec<u32>) {
    let color = material_color(material);
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    
    // Face data: (normal, vertices)
    let faces = [
        // +X face
        (Vec3::X, [
            Vec3::new( 1.0, -1.0, -1.0),
            Vec3::new( 1.0,  1.0, -1.0),
            Vec3::new( 1.0,  1.0,  1.0),
            Vec3::new( 1.0, -1.0,  1.0),
        ]),
        // -X face
        (Vec3::NEG_X, [
            Vec3::new(-1.0, -1.0,  1.0),
            Vec3::new(-1.0,  1.0,  1.0),
            Vec3::new(-1.0,  1.0, -1.0),
            Vec3::new(-1.0, -1.0, -1.0),
        ]),
        // +Y face
        (Vec3::Y, [
            Vec3::new(-1.0,  1.0, -1.0),
            Vec3::new(-1.0,  1.0,  1.0),
            Vec3::new( 1.0,  1.0,  1.0),
            Vec3::new( 1.0,  1.0, -1.0),
        ]),
        // -Y face
        (Vec3::NEG_Y, [
            Vec3::new(-1.0, -1.0,  1.0),
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new( 1.0, -1.0, -1.0),
            Vec3::new( 1.0, -1.0,  1.0),
        ]),
        // +Z face
        (Vec3::Z, [
            Vec3::new(-1.0, -1.0,  1.0),
            Vec3::new( 1.0, -1.0,  1.0),
            Vec3::new( 1.0,  1.0,  1.0),
            Vec3::new(-1.0,  1.0,  1.0),
        ]),
        // -Z face
        (Vec3::NEG_Z, [
            Vec3::new( 1.0, -1.0, -1.0),
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(-1.0,  1.0, -1.0),
            Vec3::new( 1.0,  1.0, -1.0),
        ]),
    ];
    
    for (normal, face_verts) in faces {
        let base_idx = vertices.len() as u32;
        let rotated_normal = rotation * normal;
        
        for v in face_verts {
            let local_pos = v * half_extents;
            let world_pos = position + rotation * local_pos;
            vertices.push(BlockVertex::new(world_pos, rotated_normal, color));
        }
        
        // Two triangles per face
        indices.extend_from_slice(&[
            base_idx, base_idx + 1, base_idx + 2,
            base_idx, base_idx + 2, base_idx + 3,
        ]);
    }
    
    (vertices, indices)
}

/// Generate mesh for a cylinder
fn mesh_cylinder(position: Vec3, rotation: Quat, radius: f32, height: f32, material: u8, segments: u32) -> (Vec<BlockVertex>, Vec<u32>) {
    let color = material_color(material);
    let half_height = height * 0.5;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Generate side vertices
    for i in 0..=segments {
        let angle = (i as f32 / segments as f32) * PI * 2.0;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let normal = rotation * Vec3::new(angle.cos(), 0.0, angle.sin());
        
        // Bottom vertex
        let bottom_pos = position + rotation * Vec3::new(x, -half_height, z);
        vertices.push(BlockVertex::new(bottom_pos, normal, color));
        
        // Top vertex
        let top_pos = position + rotation * Vec3::new(x, half_height, z);
        vertices.push(BlockVertex::new(top_pos, normal, color));
    }
    
    // Side indices
    for i in 0..segments {
        let base = i * 2;
        indices.extend_from_slice(&[
            base, base + 2, base + 1,
            base + 1, base + 2, base + 3,
        ]);
    }
    
    // Top cap center
    let top_center_idx = vertices.len() as u32;
    let top_normal = rotation * Vec3::Y;
    vertices.push(BlockVertex::new(position + rotation * Vec3::new(0.0, half_height, 0.0), top_normal, color));
    
    // Top cap ring
    let top_ring_start = vertices.len() as u32;
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * PI * 2.0;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let pos = position + rotation * Vec3::new(x, half_height, z);
        vertices.push(BlockVertex::new(pos, top_normal, color));
    }
    
    // Top cap indices
    for i in 0..segments {
        let next = (i + 1) % segments;
        indices.extend_from_slice(&[
            top_center_idx,
            top_ring_start + next,
            top_ring_start + i,
        ]);
    }
    
    // Bottom cap center
    let bottom_center_idx = vertices.len() as u32;
    let bottom_normal = rotation * Vec3::NEG_Y;
    vertices.push(BlockVertex::new(position + rotation * Vec3::new(0.0, -half_height, 0.0), bottom_normal, color));
    
    // Bottom cap ring
    let bottom_ring_start = vertices.len() as u32;
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * PI * 2.0;
        let x = angle.cos() * radius;
        let z = angle.sin() * radius;
        let pos = position + rotation * Vec3::new(x, -half_height, z);
        vertices.push(BlockVertex::new(pos, bottom_normal, color));
    }
    
    // Bottom cap indices (reversed winding)
    for i in 0..segments {
        let next = (i + 1) % segments;
        indices.extend_from_slice(&[
            bottom_center_idx,
            bottom_ring_start + i,
            bottom_ring_start + next,
        ]);
    }
    
    (vertices, indices)
}

/// Generate mesh for a sphere
fn mesh_sphere(position: Vec3, radius: f32, material: u8, segments: u32, rings: u32) -> (Vec<BlockVertex>, Vec<u32>) {
    let color = material_color(material);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Generate vertices
    for ring in 0..=rings {
        let phi = (ring as f32 / rings as f32) * PI;
        let y = phi.cos();
        let ring_radius = phi.sin();
        
        for seg in 0..=segments {
            let theta = (seg as f32 / segments as f32) * PI * 2.0;
            let x = ring_radius * theta.cos();
            let z = ring_radius * theta.sin();
            
            let normal = Vec3::new(x, y, z);
            let pos = position + normal * radius;
            vertices.push(BlockVertex::new(pos, normal, color));
        }
    }
    
    // Generate indices
    let stride = segments + 1;
    for ring in 0..rings {
        for seg in 0..segments {
            let current = ring * stride + seg;
            let next = current + stride;
            
            // Two triangles per quad
            if ring != 0 {
                indices.extend_from_slice(&[current, next, current + 1]);
            }
            if ring != rings - 1 {
                indices.extend_from_slice(&[current + 1, next, next + 1]);
            }
        }
    }
    
    (vertices, indices)
}

/// Generate mesh for a dome (half-sphere)
fn mesh_dome(position: Vec3, rotation: Quat, radius: f32, material: u8, segments: u32, rings: u32) -> (Vec<BlockVertex>, Vec<u32>) {
    let color = material_color(material);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Generate upper hemisphere vertices
    for ring in 0..=rings {
        let phi = (ring as f32 / rings as f32) * PI * 0.5; // Only upper half
        let y = phi.cos();
        let ring_radius = phi.sin();
        
        for seg in 0..=segments {
            let theta = (seg as f32 / segments as f32) * PI * 2.0;
            let x = ring_radius * theta.cos();
            let z = ring_radius * theta.sin();
            
            let normal = rotation * Vec3::new(x, y, z);
            let pos = position + rotation * (Vec3::new(x, y, z) * radius);
            vertices.push(BlockVertex::new(pos, normal, color));
        }
    }
    
    // Generate indices for dome
    let stride = segments + 1;
    for ring in 0..rings {
        for seg in 0..segments {
            let current = ring * stride + seg;
            let next = current + stride;
            
            if ring != 0 {
                indices.extend_from_slice(&[current, next, current + 1]);
            }
            indices.extend_from_slice(&[current + 1, next, next + 1]);
        }
    }
    
    // Add bottom cap (flat base)
    let base_center_idx = vertices.len() as u32;
    let base_normal = rotation * Vec3::NEG_Y;
    vertices.push(BlockVertex::new(position, base_normal, color));
    
    let base_ring_start = vertices.len() as u32;
    for seg in 0..segments {
        let theta = (seg as f32 / segments as f32) * PI * 2.0;
        let x = theta.cos() * radius;
        let z = theta.sin() * radius;
        let pos = position + rotation * Vec3::new(x, 0.0, z);
        vertices.push(BlockVertex::new(pos, base_normal, color));
    }
    
    // Base cap indices
    for seg in 0..segments {
        let next = (seg + 1) % segments;
        indices.extend_from_slice(&[
            base_center_idx,
            base_ring_start + seg,
            base_ring_start + next,
        ]);
    }
    
    (vertices, indices)
}

/// Generate mesh for an arch
fn mesh_arch(position: Vec3, rotation: Quat, width: f32, height: f32, depth: f32, material: u8, arch_segments: u32) -> (Vec<BlockVertex>, Vec<u32>) {
    let color = material_color(material);
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    let half_width = width * 0.5;
    let half_depth = depth * 0.5;
    let arch_radius = half_width;
    let pillar_height = height - arch_radius;
    
    // Left pillar
    let (left_verts, left_idx) = mesh_box(
        position + rotation * Vec3::new(-half_width + arch_radius * 0.25, pillar_height * 0.5, 0.0),
        rotation,
        Vec3::new(arch_radius * 0.25, pillar_height * 0.5, half_depth),
        material
    );
    let base_idx = vertices.len() as u32;
    vertices.extend(left_verts);
    indices.extend(left_idx.iter().map(|i| i + base_idx));
    
    // Right pillar
    let (right_verts, right_idx) = mesh_box(
        position + rotation * Vec3::new(half_width - arch_radius * 0.25, pillar_height * 0.5, 0.0),
        rotation,
        Vec3::new(arch_radius * 0.25, pillar_height * 0.5, half_depth),
        material
    );
    let base_idx = vertices.len() as u32;
    vertices.extend(right_verts);
    indices.extend(right_idx.iter().map(|i| i + base_idx));
    
    // Top block above arch
    let (top_verts, top_idx) = mesh_box(
        position + rotation * Vec3::new(0.0, height - arch_radius * 0.25, 0.0),
        rotation,
        Vec3::new(half_width, arch_radius * 0.25, half_depth),
        material
    );
    let base_idx = vertices.len() as u32;
    vertices.extend(top_verts);
    indices.extend(top_idx.iter().map(|i| i + base_idx));
    
    // Arch ring (outer surface) - front and back
    for front in [true, false] {
        let z_offset = if front { half_depth } else { -half_depth };
        let z_normal = if front { 1.0 } else { -1.0 };
        
        // Arc vertices
        let arc_base = vertices.len() as u32;
        for i in 0..=arch_segments {
            let angle = (i as f32 / arch_segments as f32) * PI;
            let x = angle.cos() * arch_radius;
            let y = angle.sin() * arch_radius + pillar_height;
            
            let normal = rotation * Vec3::new(0.0, 0.0, z_normal);
            let pos = position + rotation * Vec3::new(x, y, z_offset);
            vertices.push(BlockVertex::new(pos, normal, color));
        }
        
        // Inner arc (slightly smaller radius for arch thickness)
        let inner_radius = arch_radius * 0.7;
        let inner_base = vertices.len() as u32;
        for i in 0..=arch_segments {
            let angle = (i as f32 / arch_segments as f32) * PI;
            let x = angle.cos() * inner_radius;
            let y = angle.sin() * inner_radius + pillar_height;
            
            let normal = rotation * Vec3::new(0.0, 0.0, z_normal);
            let pos = position + rotation * Vec3::new(x, y, z_offset);
            vertices.push(BlockVertex::new(pos, normal, color));
        }
        
        // Connect outer and inner arcs with quads
        for i in 0..arch_segments {
            let o1 = arc_base + i;
            let o2 = arc_base + i + 1;
            let i1 = inner_base + i;
            let i2 = inner_base + i + 1;
            
            if front {
                indices.extend_from_slice(&[o1, o2, i1, i1, o2, i2]);
            } else {
                indices.extend_from_slice(&[o1, i1, o2, i1, i2, o2]);
            }
        }
    }
    
    (vertices, indices)
}

/// Generate mesh for a wedge/ramp
fn mesh_wedge(position: Vec3, rotation: Quat, size: Vec3, material: u8) -> (Vec<BlockVertex>, Vec<u32>) {
    let color = material_color(material);
    let half = size * 0.5;
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    
    // Wedge vertices (6 vertices for triangular prism)
    // Back face (rectangle)
    // Front face (triangle - collapsed top edge)
    
    let v = [
        // Back bottom-left
        Vec3::new(-half.x, -half.y, -half.z),
        // Back bottom-right
        Vec3::new( half.x, -half.y, -half.z),
        // Back top-left
        Vec3::new(-half.x,  half.y, -half.z),
        // Back top-right
        Vec3::new( half.x,  half.y, -half.z),
        // Front bottom-left
        Vec3::new(-half.x, -half.y,  half.z),
        // Front bottom-right
        Vec3::new( half.x, -half.y,  half.z),
    ];
    
    // Transform all vertices
    let world_v: Vec<Vec3> = v.iter().map(|&local| position + rotation * local).collect();
    
    // Back face (quad)
    let back_normal = rotation * Vec3::NEG_Z;
    let base = vertices.len() as u32;
    vertices.push(BlockVertex::new(world_v[0], back_normal, color));
    vertices.push(BlockVertex::new(world_v[1], back_normal, color));
    vertices.push(BlockVertex::new(world_v[3], back_normal, color));
    vertices.push(BlockVertex::new(world_v[2], back_normal, color));
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    
    // Bottom face (quad)
    let bottom_normal = rotation * Vec3::NEG_Y;
    let base = vertices.len() as u32;
    vertices.push(BlockVertex::new(world_v[0], bottom_normal, color));
    vertices.push(BlockVertex::new(world_v[4], bottom_normal, color));
    vertices.push(BlockVertex::new(world_v[5], bottom_normal, color));
    vertices.push(BlockVertex::new(world_v[1], bottom_normal, color));
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    
    // Slope face (quad)
    let slope_normal = rotation * Vec3::new(0.0, size.z, size.y).normalize();
    let base = vertices.len() as u32;
    vertices.push(BlockVertex::new(world_v[4], slope_normal, color));
    vertices.push(BlockVertex::new(world_v[2], slope_normal, color));
    vertices.push(BlockVertex::new(world_v[3], slope_normal, color));
    vertices.push(BlockVertex::new(world_v[5], slope_normal, color));
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    
    // Left face (triangle)
    let left_normal = rotation * Vec3::NEG_X;
    let base = vertices.len() as u32;
    vertices.push(BlockVertex::new(world_v[0], left_normal, color));
    vertices.push(BlockVertex::new(world_v[2], left_normal, color));
    vertices.push(BlockVertex::new(world_v[4], left_normal, color));
    indices.extend_from_slice(&[base, base + 1, base + 2]);
    
    // Right face (triangle)
    let right_normal = rotation * Vec3::X;
    let base = vertices.len() as u32;
    vertices.push(BlockVertex::new(world_v[1], right_normal, color));
    vertices.push(BlockVertex::new(world_v[5], right_normal, color));
    vertices.push(BlockVertex::new(world_v[3], right_normal, color));
    indices.extend_from_slice(&[base, base + 1, base + 2]);
    
    (vertices, indices)
}

// ============================================================================
// BUILDING BLOCK MANAGER
// ============================================================================

/// Manages a collection of building blocks
#[derive(Default)]
pub struct BuildingBlockManager {
    /// All blocks in the scene
    blocks: Vec<BuildingBlock>,
    /// Next group ID for merging
    next_group_id: u32,
    /// Whether the mesh needs regeneration
    mesh_dirty: bool,
}

impl BuildingBlockManager {
    /// Create a new empty manager
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Add a new block
    pub fn add_block(&mut self, block: BuildingBlock) -> u32 {
        let id = block.id;
        self.blocks.push(block);
        self.mesh_dirty = true;
        id
    }
    
    /// Remove a block by ID
    pub fn remove_block(&mut self, id: u32) -> Option<BuildingBlock> {
        if let Some(pos) = self.blocks.iter().position(|b| b.id == id) {
            self.mesh_dirty = true;
            Some(self.blocks.remove(pos))
        } else {
            None
        }
    }
    
    /// Get a block by ID
    pub fn get_block(&self, id: u32) -> Option<&BuildingBlock> {
        self.blocks.iter().find(|b| b.id == id)
    }
    
    /// Get a mutable block by ID
    pub fn get_block_mut(&mut self, id: u32) -> Option<&mut BuildingBlock> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }
    
    /// Get all blocks
    pub fn blocks(&self) -> &[BuildingBlock] {
        &self.blocks
    }
    
    /// Check if mesh needs regeneration
    pub fn needs_mesh_update(&self) -> bool {
        self.mesh_dirty
    }
    
    /// Clear the dirty flag
    pub fn clear_dirty(&mut self) {
        self.mesh_dirty = false;
    }
    
    /// Generate combined mesh for all blocks
    pub fn generate_combined_mesh(&self) -> (Vec<BlockVertex>, Vec<u32>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        
        for block in &self.blocks {
            let (block_verts, block_idx) = block.generate_mesh();
            let base_idx = vertices.len() as u32;
            vertices.extend(block_verts);
            indices.extend(block_idx.iter().map(|i| i + base_idx));
        }
        
        (vertices, indices)
    }
    
    /// Find blocks that intersect with a given AABB
    pub fn find_intersecting(&self, aabb: &AABB) -> Vec<u32> {
        self.blocks
            .iter()
            .filter(|b| b.aabb().intersects(aabb))
            .map(|b| b.id)
            .collect()
    }
    
    /// Find the closest block to a point (using SDF)
    pub fn find_closest(&self, point: Vec3, max_distance: f32) -> Option<(u32, f32)> {
        let mut closest: Option<(u32, f32)> = None;
        
        for block in &self.blocks {
            let dist = block.sdf(point);
            if dist < max_distance {
                if let Some((_, best_dist)) = closest {
                    if dist < best_dist {
                        closest = Some((block.id, dist));
                    }
                } else {
                    closest = Some((block.id, dist));
                }
            }
        }
        
        closest
    }
    
    /// Evaluate combined SDF at a point
    pub fn combined_sdf(&self, point: Vec3) -> f32 {
        let mut min_dist = f32::MAX;
        for block in &self.blocks {
            min_dist = min_dist.min(block.sdf(point));
        }
        min_dist
    }
    
    /// Evaluate smooth union SDF for a group of blocks
    pub fn smooth_union_sdf(&self, point: Vec3, group_id: u32, smoothness: f32) -> f32 {
        let mut result = f32::MAX;
        let mut first = true;
        
        for block in &self.blocks {
            if block.merged_group_id == Some(group_id) {
                let d = block.sdf(point);
                if first {
                    result = d;
                    first = false;
                } else {
                    result = sdf_smooth_union(result, d, smoothness);
                }
            }
        }
        
        result
    }
    
    /// Create a new merge group from selected blocks
    pub fn create_merge_group(&mut self, block_ids: &[u32]) -> u32 {
        let group_id = self.next_group_id;
        self.next_group_id += 1;
        
        for id in block_ids {
            if let Some(block) = self.get_block_mut(*id) {
                block.merged_group_id = Some(group_id);
            }
        }
        
        self.mesh_dirty = true;
        group_id
    }
    
    /// Clear count
    pub fn len(&self) -> usize {
        self.blocks.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_cube_sdf() {
        let block = BuildingBlock::new(
            BuildingBlockShape::Cube { half_extents: Vec3::splat(1.0) },
            Vec3::ZERO,
            0
        );
        
        // Inside
        assert!(block.sdf(Vec3::ZERO) < 0.0);
        // On surface
        assert!((block.sdf(Vec3::new(1.0, 0.0, 0.0))).abs() < 0.01);
        // Outside
        assert!(block.sdf(Vec3::new(2.0, 0.0, 0.0)) > 0.0);
    }
    
    #[test]
    fn test_sphere_sdf() {
        let block = BuildingBlock::new(
            BuildingBlockShape::Sphere { radius: 1.0 },
            Vec3::ZERO,
            0
        );
        
        // Inside
        assert!(block.sdf(Vec3::ZERO) < 0.0);
        // On surface
        assert!((block.sdf(Vec3::new(1.0, 0.0, 0.0))).abs() < 0.01);
        // Outside
        assert!(block.sdf(Vec3::new(2.0, 0.0, 0.0)) > 0.0);
    }
    
    #[test]
    fn test_smooth_union() {
        let d1 = 0.5;
        let d2 = 0.5;
        let smoothness = 0.2;
        
        let result = sdf_smooth_union(d1, d2, smoothness);
        // Smooth union should be less than or equal to hard union
        assert!(result <= d1.min(d2));
    }
    
    #[test]
    fn test_aabb_intersection() {
        let a = AABB::new(Vec3::ZERO, Vec3::ONE);
        let b = AABB::new(Vec3::splat(0.5), Vec3::splat(1.5));
        let c = AABB::new(Vec3::splat(2.0), Vec3::splat(3.0));
        
        assert!(a.intersects(&b));
        assert!(!a.intersects(&c));
    }
    
    #[test]
    fn test_mesh_generation() {
        let block = BuildingBlock::new(
            BuildingBlockShape::Cube { half_extents: Vec3::splat(1.0) },
            Vec3::ZERO,
            0
        );
        
        let (verts, indices) = block.generate_mesh();
        assert_eq!(verts.len(), 24); // 6 faces * 4 vertices
        assert_eq!(indices.len(), 36); // 6 faces * 2 triangles * 3 indices
    }
}
