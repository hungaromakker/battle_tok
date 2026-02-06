//! Pump/Inflate Extrusion Module (US-P4-006)
//!
//! Converts 2D outlines into 3D meshes using signed distance fields (SDFs)
//! and the engine's existing Marching Cubes implementation.
//!
//! The primary method is "Pump" extrusion, which inflates a 2D outline into
//! a rounded 3D shape by computing an SDF from the outline boundary distance
//! and a configurable depth profile (Elliptical, Flat, or Pointed).
//!
//! # Pipeline
//!
//! 1. Receive `Outline2D` from the Draw2D stage
//! 2. Convert `[f32; 2]` points to `Vec2` polygon
//! 3. Compute bounding box and max inscribed radius
//! 4. Evaluate SDF at each Marching Cubes grid point
//! 5. Generate triangle mesh via `MarchingCubes::generate_mesh`
//! 6. Upload vertex/index buffers to GPU

use glam::{Vec2, Vec3};
use wgpu::util::DeviceExt;

use crate::render::building_blocks::BlockVertex;
use crate::render::marching_cubes::MarchingCubes;

use super::canvas_2d::Outline2D;

// ============================================================================
// ENUMS
// ============================================================================

/// The extrusion method to apply to a 2D outline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtrudeMethod {
    /// Pump/inflate: SDF-based rounded extrusion (primary method).
    Pump,
    /// Linear extrusion along Z axis (future).
    Linear,
    /// Lathe/revolve around Y axis (future).
    Lathe,
}

/// Profile curve for the Pump extrusion depth.
///
/// Controls how the Z-depth varies from the outline boundary to its center:
/// - `Elliptical`: smooth dome shape (thickest at center, zero at boundary)
/// - `Flat`: uniform thickness everywhere inside
/// - `Pointed`: linear taper from boundary (zero) to center (max)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpProfile {
    /// Elliptical cross-section (smooth dome).
    Elliptical,
    /// Flat/uniform depth across the interior.
    Flat,
    /// Pointed/conical taper toward center.
    Pointed,
}

// ============================================================================
// PARAMETERS
// ============================================================================

/// Parameters controlling the extrusion process.
pub struct ExtrudeParams {
    /// Extrusion method (Pump, Linear, Lathe).
    pub method: ExtrudeMethod,
    /// Inflation amount for Pump extrusion. Range: 0.0-1.0, default 0.5.
    pub inflation: f32,
    /// Thickness multiplier. Range: 0.1-5.0, default 1.0.
    pub thickness: f32,
    /// Profile curve for Pump depth. Default: Elliptical.
    pub profile: PumpProfile,
    /// Linear extrusion depth (future use).
    pub depth: f32,
    /// Linear extrusion taper factor (future use).
    pub taper: f32,
    /// Number of segments for Lathe revolution (future use).
    pub segments: u32,
    /// Sweep angle in degrees for Lathe (future use).
    pub sweep_degrees: f32,
    /// Marching Cubes grid resolution per axis. Default: 48.
    pub mc_resolution: u32,
}

impl Default for ExtrudeParams {
    fn default() -> Self {
        Self {
            method: ExtrudeMethod::Pump,
            inflation: 0.5,
            thickness: 1.0,
            profile: PumpProfile::Elliptical,
            depth: 1.0,
            taper: 0.0,
            segments: 32,
            sweep_degrees: 360.0,
            mc_resolution: 48,
        }
    }
}

// ============================================================================
// EXTRUDER
// ============================================================================

/// The main extrusion state machine.
///
/// Holds parameters, generated mesh data, and optional GPU buffers.
/// Call `generate_preview` with outline data, then `upload_to_gpu` to
/// create renderable buffers.
pub struct Extruder {
    /// Current extrusion parameters.
    pub params: ExtrudeParams,
    /// Generated mesh vertices (CPU side).
    pub mesh_vertices: Vec<BlockVertex>,
    /// Generated mesh indices (CPU side).
    pub mesh_indices: Vec<u32>,
    /// Whether the mesh needs regeneration (params changed).
    pub dirty: bool,
    /// GPU vertex buffer (created by `upload_to_gpu`).
    pub gpu_vertex_buffer: Option<wgpu::Buffer>,
    /// GPU index buffer (created by `upload_to_gpu`).
    pub gpu_index_buffer: Option<wgpu::Buffer>,
}

impl Default for Extruder {
    fn default() -> Self {
        Self::new()
    }
}

impl Extruder {
    /// Create a new Extruder with default parameters and no mesh data.
    pub fn new() -> Self {
        Self {
            params: ExtrudeParams::default(),
            mesh_vertices: Vec::new(),
            mesh_indices: Vec::new(),
            dirty: true,
            gpu_vertex_buffer: None,
            gpu_index_buffer: None,
        }
    }

    /// Generate a 3D preview mesh from the given outlines.
    ///
    /// Converts the first valid (closed or 3+ point) outline into a polygon,
    /// evaluates the pump SDF across a Marching Cubes grid, and stores the
    /// resulting mesh in `mesh_vertices` / `mesh_indices`.
    ///
    /// Returns `true` if the mesh contains at least one triangle.
    pub fn generate_preview(&mut self, outlines: &[Outline2D]) -> bool {
        self.mesh_vertices.clear();
        self.mesh_indices.clear();
        self.dirty = false;

        // Collect all outline points into a single polygon.
        // Use the first outline with enough points.
        let polygon_points: Vec<[f32; 2]> = outlines
            .iter()
            .filter(|o| o.points.len() >= 3)
            .flat_map(|o| o.points.iter().copied())
            .collect();

        if polygon_points.len() < 3 {
            return false;
        }

        // Convert to Vec2 polygon
        let polygon: Vec<Vec2> = polygon_points
            .iter()
            .map(|p| Vec2::new(p[0], p[1]))
            .collect();

        // Compute bounding box and max inradius
        let (bb_min, bb_max) = outline_bounding_box(&polygon_points);
        let max_inradius = compute_max_inradius(&polygon_points);

        if max_inradius < 1e-6 {
            return false;
        }

        // Determine the maximum Z extent for the bounding volume
        let max_z = self.params.thickness * self.params.inflation;
        let margin = 0.5; // Extra margin around bounds

        let mc_min = Vec3::new(bb_min.x - margin, bb_min.y - margin, -max_z - margin);
        let mc_max = Vec3::new(bb_max.x + margin, bb_max.y + margin, max_z + margin);

        // Create MarchingCubes instance with configured resolution
        let mc = MarchingCubes::new(self.params.mc_resolution);

        // Capture params and polygon for the SDF closure
        let params = &self.params;
        let poly = &polygon;
        let inradius = max_inradius;

        let sdf = |p: Vec3| -> f32 { sdf_pumped(p, poly, params, inradius) };

        // Default color: neutral gray
        let color = [0.6, 0.6, 0.6, 1.0];

        let (vertices, indices) = mc.generate_mesh(sdf, mc_min, mc_max, color);

        self.mesh_vertices = vertices;
        self.mesh_indices = indices;

        !self.mesh_indices.is_empty()
    }

    /// Upload the current mesh data to GPU buffers.
    ///
    /// Creates new vertex and index buffers on the given device.
    /// Previous buffers are replaced (old ones will be dropped).
    pub fn upload_to_gpu(&mut self, device: &wgpu::Device) {
        if self.mesh_vertices.is_empty() || self.mesh_indices.is_empty() {
            self.gpu_vertex_buffer = None;
            self.gpu_index_buffer = None;
            return;
        }

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Extrude Vertex Buffer"),
            contents: bytemuck::cast_slice(&self.mesh_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Extrude Index Buffer"),
            contents: bytemuck::cast_slice(&self.mesh_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.gpu_vertex_buffer = Some(vertex_buffer);
        self.gpu_index_buffer = Some(index_buffer);
    }

    /// Return the number of indices (for draw calls).
    pub fn index_count(&self) -> u32 {
        self.mesh_indices.len() as u32
    }

    /// Return whether the extruder has a valid mesh ready.
    pub fn has_mesh(&self) -> bool {
        !self.mesh_indices.is_empty()
    }
}

// ============================================================================
// GEOMETRY HELPER FUNCTIONS
// ============================================================================

/// Compute the minimum distance from point `p` to the line segment `a`--`b`.
///
/// Uses the standard projection formula: project `p` onto the line,
/// clamp the parameter to [0, 1], then measure distance.
pub fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let ap = p - a;
    let len_sq = ab.length_squared();

    if len_sq < 1e-12 {
        // Degenerate segment: just distance to point a
        return ap.length();
    }

    let t = ap.dot(ab) / len_sq;
    let t_clamped = t.clamp(0.0, 1.0);
    let closest = a + ab * t_clamped;
    (p - closest).length()
}

/// Compute the minimum distance from `point` to any edge of `polygon`.
///
/// The polygon is treated as a closed loop: the last point connects back
/// to the first point.
pub fn min_distance_to_polygon(point: Vec2, polygon: &[Vec2]) -> f32 {
    let n = polygon.len();
    if n == 0 {
        return f32::MAX;
    }
    if n == 1 {
        return (point - polygon[0]).length();
    }

    let mut min_dist = f32::MAX;
    for i in 0..n {
        let j = (i + 1) % n;
        let d = point_segment_distance(point, polygon[i], polygon[j]);
        if d < min_dist {
            min_dist = d;
        }
    }
    min_dist
}

/// Test whether `point` lies inside `polygon` using the ray casting algorithm.
///
/// Casts a horizontal ray from the point to +X infinity and counts edge
/// crossings. An odd count means the point is inside.
pub fn point_in_polygon(point: Vec2, polygon: &[Vec2]) -> bool {
    let n = polygon.len();
    if n < 3 {
        return false;
    }

    let mut inside = false;
    let mut j = n - 1;

    for i in 0..n {
        let pi = polygon[i];
        let pj = polygon[j];

        // Check if the edge crosses the horizontal ray from `point`
        if (pi.y > point.y) != (pj.y > point.y) {
            let x_intersect = pj.x + (point.y - pj.y) / (pi.y - pj.y) * (pi.x - pj.x);
            if point.x < x_intersect {
                inside = !inside;
            }
        }
        j = i;
    }

    inside
}

/// Approximate the maximum inscribed radius of the polygon.
///
/// Samples interior points on a grid and finds the maximum distance to
/// any polygon edge. This gives an approximation of the largest circle
/// that fits inside the polygon.
pub fn compute_max_inradius(points: &[[f32; 2]]) -> f32 {
    let polygon: Vec<Vec2> = points.iter().map(|p| Vec2::new(p[0], p[1])).collect();

    if polygon.len() < 3 {
        return 0.0;
    }

    let (bb_min, bb_max) = outline_bounding_box(points);
    let size = bb_max - bb_min;
    let max_dim = size.x.max(size.y);

    if max_dim < 1e-6 {
        return 0.0;
    }

    // Sample on a grid (approx 20x20)
    let steps = 20;
    let step_x = size.x / steps as f32;
    let step_y = size.y / steps as f32;

    let mut max_radius: f32 = 0.0;

    for iy in 0..=steps {
        for ix in 0..=steps {
            let sample = Vec2::new(bb_min.x + ix as f32 * step_x, bb_min.y + iy as f32 * step_y);

            if point_in_polygon(sample, &polygon) {
                let dist = min_distance_to_polygon(sample, &polygon);
                if dist > max_radius {
                    max_radius = dist;
                }
            }
        }
    }

    max_radius
}

/// Compute the axis-aligned bounding box of outline points.
///
/// Returns `(min, max)` as `Vec2` values.
pub fn outline_bounding_box(points: &[[f32; 2]]) -> (Vec2, Vec2) {
    if points.is_empty() {
        return (Vec2::ZERO, Vec2::ZERO);
    }

    let mut min = Vec2::new(f32::MAX, f32::MAX);
    let mut max = Vec2::new(f32::MIN, f32::MIN);

    for p in points {
        min.x = min.x.min(p[0]);
        min.y = min.y.min(p[1]);
        max.x = max.x.max(p[0]);
        max.y = max.y.max(p[1]);
    }

    (min, max)
}

// ============================================================================
// 2D SDF (Signed Distance for polygon outlines)
// ============================================================================

/// Compute the 2D signed distance from a point to a closed polygon.
///
/// Returns negative values for points inside the polygon, positive
/// for points outside. The magnitude is the distance to the nearest edge.
#[allow(dead_code)] // Used by sdf_linear_extrude (US-P4-007)
fn sdf_2d_polygon(p: Vec2, polygon: &[Vec2]) -> f32 {
    let dist = min_distance_to_polygon(p, polygon);
    if point_in_polygon(p, polygon) {
        -dist
    } else {
        dist
    }
}

// ============================================================================
// SDF FUNCTIONS
// ============================================================================

/// Evaluate the "pumped" SDF at a 3D point.
///
/// The SDF is constructed from the 2D polygon boundary distance:
/// - In the XY plane, compute signed distance to the polygon boundary.
/// - Along Z, the allowed depth is determined by the profile curve
///   (Elliptical, Flat, or Pointed) scaled by inflation and thickness.
///
/// Negative values are inside the volume, positive values are outside.
///
/// # Profile Behavior
///
/// `normalized_dist` is 0 at the polygon boundary and 1 at the deepest
/// interior point (the polygon center, approximated by max inradius).
///
/// - **Elliptical**: `sqrt(1 - (1 - normalized_dist)^2)` -- smooth dome,
///   thickest at center, zero at boundary.
/// - **Flat**: 1.0 everywhere inside -- uniform slab.
/// - **Pointed**: `normalized_dist` -- linear taper from boundary to center.
fn sdf_pumped(p: Vec3, polygon: &[Vec2], params: &ExtrudeParams, max_inradius: f32) -> f32 {
    let p2d = Vec2::new(p.x, p.y);
    let dist_to_boundary = min_distance_to_polygon(p2d, polygon);
    let inside = point_in_polygon(p2d, polygon);

    // Normalize distance: 0 at boundary, 1 at center (max inradius)
    let normalized_dist = if inside {
        (dist_to_boundary / max_inradius).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Compute profile depth factor based on selected profile
    let profile_depth = match params.profile {
        PumpProfile::Elliptical => {
            if inside {
                // Elliptical: sqrt(1 - (1 - t)^2) where t = normalized_dist
                // At t=0 (boundary): sqrt(1 - 1) = 0
                // At t=1 (center):   sqrt(1 - 0) = 1
                (1.0 - (1.0 - normalized_dist).powi(2)).sqrt()
            } else {
                0.0
            }
        }
        PumpProfile::Flat => {
            if inside {
                1.0
            } else {
                0.0
            }
        }
        PumpProfile::Pointed => normalized_dist,
    };

    // Maximum Z extent at this XY position
    let max_z = params.thickness * params.inflation * profile_depth;

    // Distance along Z from the surface
    let z_dist = p.z.abs() - max_z;

    if inside {
        // Inside the polygon in XY -- SDF is determined by Z distance
        z_dist // negative = inside, positive = outside along Z
    } else {
        // Outside the polygon in XY -- combine 2D distance with Z overshoot
        let signed_dist_2d = dist_to_boundary; // positive outside
        (signed_dist_2d.powi(2) + z_dist.max(0.0).powi(2)).sqrt()
    }
}

// ============================================================================
// LINEAR EXTRUDE (SDF + Marching Cubes)
// ============================================================================

/// Evaluate the SDF for a linear extrusion of a 2D polygon along the Z axis.
///
/// The 2D outline defines the cross-section at z=0. The shape is extruded
/// from z=0 to z=depth. An optional taper factor (0.0 = no taper, 1.0 = full
/// taper to a point) linearly shrinks the cross-section toward z=depth.
///
/// The SDF is the intersection of:
/// - The (possibly tapered) 2D polygon boundary in XY
/// - A Z-slab from 0 to depth
#[allow(dead_code)] // US-P4-007: Linear extrusion
fn sdf_linear_extrude(p: Vec3, polygon: &[Vec2], depth: f32, taper: f32) -> f32 {
    if polygon.len() < 3 || depth < 1e-6 {
        return f32::MAX;
    }

    // Fraction along the extrusion axis (clamped for SDF evaluation outside bounds)
    let t = (p.z / depth).clamp(0.0, 1.0);

    // Scale factor due to taper: 1.0 at z=0, (1-taper) at z=depth
    let scale = 1.0 - taper * t;

    // For the 2D SDF, scale the query point inversely so the polygon
    // "appears" to shrink as z increases.
    let p2d = if scale > 1e-6 {
        Vec2::new(p.x / scale, p.y / scale)
    } else {
        // Fully tapered -- point is always outside
        return f32::MAX;
    };

    // 2D signed distance (negative inside)
    let d2d = sdf_2d_polygon(p2d, polygon) * scale;

    // Z-slab distance: inside when 0 <= p.z <= depth
    let d_z = (-p.z).max(p.z - depth);

    // Combine: union of the two constraints (max for intersection)
    if d2d > 0.0 && d_z > 0.0 {
        // Outside both -- Euclidean distance to the edge
        (d2d * d2d + d_z * d_z).sqrt()
    } else {
        // Inside at least one -- take the larger (closer to surface)
        d2d.max(d_z)
    }
}

// ============================================================================
// LATHE REVOLUTION (Direct Mesh Generation)
// ============================================================================

/// Generate a mesh by revolving a 2D profile around the Y axis.
///
/// The profile is a polyline where each point's X coordinate gives the
/// radius from the Y axis, and the Y coordinate gives the height.
/// The profile is swept by `sweep_degrees` (typically 360) around Y,
/// divided into `segments` angular steps.
///
/// Returns `(vertices, indices)` as `BlockVertex` data.
#[allow(dead_code)] // US-P4-007: Lathe revolution
fn lathe_mesh(
    profile: &[[f32; 2]],
    segments: u32,
    sweep_degrees: f32,
    color: [f32; 4],
) -> (Vec<BlockVertex>, Vec<u32>) {
    if profile.len() < 2 || segments == 0 {
        return (Vec::new(), Vec::new());
    }

    let sweep_rad = sweep_degrees.to_radians();
    let profile_len = profile.len();
    let ring_count = if (sweep_degrees - 360.0).abs() < 0.01 {
        segments // Full revolution: last ring == first ring
    } else {
        segments + 1 // Partial sweep: include both end caps
    };
    let is_full = (sweep_degrees - 360.0).abs() < 0.01;

    let vert_count = (ring_count as usize) * profile_len;
    let mut vertices = Vec::with_capacity(vert_count);
    let mut indices = Vec::new();

    // Generate vertices: one ring per angular step
    for seg in 0..ring_count {
        let angle = (seg as f32 / segments as f32) * sweep_rad;
        let cos_a = angle.cos();
        let sin_a = angle.sin();

        for pi in 0..profile_len {
            let r = profile[pi][0]; // radius from Y axis
            let y = profile[pi][1]; // height

            let position = Vec3::new(r * cos_a, y, r * sin_a);

            // Compute a preliminary normal from the profile tangent
            // rotated around the Y axis. We'll refine normals later
            // with recompute_lathe_normals() for sharp edges.
            let (prev, next) = if pi == 0 {
                (pi, pi + 1)
            } else if pi == profile_len - 1 {
                (pi - 1, pi)
            } else {
                (pi - 1, pi + 1)
            };

            let dr = profile[next][0] - profile[prev][0];
            let dy = profile[next][1] - profile[prev][1];
            // The outward-facing 2D normal in the profile plane:
            // tangent is (dr, dy), so outward normal is (dy, -dr)
            let profile_normal_r = dy;
            let profile_normal_y = -dr;
            let len =
                (profile_normal_r * profile_normal_r + profile_normal_y * profile_normal_y).sqrt();

            let normal = if len > 1e-8 {
                let nr = profile_normal_r / len;
                let ny = profile_normal_y / len;
                Vec3::new(nr * cos_a, ny, nr * sin_a)
            } else {
                Vec3::new(cos_a, 0.0, sin_a)
            };

            vertices.push(BlockVertex {
                position: position.to_array(),
                normal: normal.to_array(),
                color,
            });
        }
    }

    // Generate indices connecting adjacent rings into quads (2 triangles each)
    for seg in 0..segments {
        let ring_a = (seg as usize) * profile_len;
        let ring_b = if is_full && seg == segments - 1 {
            0 // Wrap around to the first ring
        } else {
            ((seg + 1) as usize) * profile_len
        };

        for pi in 0..profile_len - 1 {
            let a0 = ring_a + pi;
            let a1 = ring_a + pi + 1;
            let b0 = ring_b + pi;
            let b1 = ring_b + pi + 1;

            // Two triangles per quad
            indices.push(a0 as u32);
            indices.push(b0 as u32);
            indices.push(a1 as u32);

            indices.push(a1 as u32);
            indices.push(b0 as u32);
            indices.push(b1 as u32);
        }
    }

    (vertices, indices)
}

/// Recompute normals for a lathe mesh to handle sharp corners in the profile.
///
/// For each triangle, compute the face normal. Then, for each vertex,
/// accumulate face normals from adjacent triangles. This produces
/// smooth normals on curved surfaces but allows per-face normals at
/// sharp profile corners (where the angle between adjacent profile
/// segments exceeds a threshold).
///
/// This function operates in-place on the given vertex/index data.
#[allow(dead_code)] // US-P4-007: Lathe normal smoothing
fn recompute_lathe_normals(vertices: &mut [BlockVertex], indices: &[u32]) {
    if vertices.is_empty() || indices.len() < 3 {
        return;
    }

    // Zero out all normals
    for v in vertices.iter_mut() {
        v.normal = [0.0, 0.0, 0.0];
    }

    // Accumulate face normals (area-weighted by the cross product magnitude)
    let tri_count = indices.len() / 3;
    for t in 0..tri_count {
        let i0 = indices[t * 3] as usize;
        let i1 = indices[t * 3 + 1] as usize;
        let i2 = indices[t * 3 + 2] as usize;

        let p0 = Vec3::from_array(vertices[i0].position);
        let p1 = Vec3::from_array(vertices[i1].position);
        let p2 = Vec3::from_array(vertices[i2].position);

        let face_normal = (p1 - p0).cross(p2 - p0);
        // face_normal magnitude is proportional to triangle area -- this gives
        // area-weighted averaging, which is desirable for smooth normals.

        for &idx in &[i0, i1, i2] {
            vertices[idx].normal[0] += face_normal.x;
            vertices[idx].normal[1] += face_normal.y;
            vertices[idx].normal[2] += face_normal.z;
        }
    }

    // Normalize all normals
    for v in vertices.iter_mut() {
        let n = Vec3::from_array(v.normal);
        let len = n.length();
        if len > 1e-8 {
            v.normal = (n / len).to_array();
        } else {
            v.normal = [0.0, 1.0, 0.0]; // Fallback upward normal
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a simple square polygon for testing.
    fn test_square() -> Vec<Vec2> {
        vec![
            Vec2::new(-1.0, -1.0),
            Vec2::new(1.0, -1.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(-1.0, 1.0),
        ]
    }

    #[test]
    fn test_point_in_polygon_inside() {
        let square = test_square();
        assert!(point_in_polygon(Vec2::ZERO, &square));
        assert!(point_in_polygon(Vec2::new(0.5, 0.5), &square));
    }

    #[test]
    fn test_point_in_polygon_outside() {
        let square = test_square();
        assert!(!point_in_polygon(Vec2::new(2.0, 0.0), &square));
        assert!(!point_in_polygon(Vec2::new(0.0, -2.0), &square));
    }

    #[test]
    fn test_point_segment_distance() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(2.0, 0.0);
        let p = Vec2::new(1.0, 1.0);
        let dist = point_segment_distance(p, a, b);
        assert!((dist - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_point_segment_distance_at_endpoint() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(2.0, 0.0);
        let p = Vec2::new(-1.0, 0.0);
        let dist = point_segment_distance(p, a, b);
        assert!((dist - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_min_distance_to_polygon() {
        let square = test_square();
        // Point at center should be ~1.0 from nearest edge
        let dist = min_distance_to_polygon(Vec2::ZERO, &square);
        assert!((dist - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_outline_bounding_box() {
        let points = vec![[-1.0, -2.0], [3.0, 4.0], [0.0, 0.0]];
        let (min, max) = outline_bounding_box(&points);
        assert!((min.x - (-1.0)).abs() < 0.001);
        assert!((min.y - (-2.0)).abs() < 0.001);
        assert!((max.x - 3.0).abs() < 0.001);
        assert!((max.y - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_compute_max_inradius_square() {
        let points = vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        let r = compute_max_inradius(&points);
        // Max inradius of a 2x2 square is ~1.0
        assert!(r > 0.8, "Expected inradius near 1.0, got {}", r);
        assert!(r <= 1.05, "Expected inradius near 1.0, got {}", r);
    }

    #[test]
    fn test_sdf_pumped_center_inside() {
        let square = test_square();
        let params = ExtrudeParams::default();
        let inradius = 1.0;
        // Center of shape at z=0 should be inside (negative SDF)
        let val = sdf_pumped(Vec3::ZERO, &square, &params, inradius);
        assert!(val < 0.0, "Center should be inside, got {}", val);
    }

    #[test]
    fn test_sdf_pumped_outside_xy() {
        let square = test_square();
        let params = ExtrudeParams::default();
        let inradius = 1.0;
        // Far outside in XY should be positive
        let val = sdf_pumped(Vec3::new(5.0, 0.0, 0.0), &square, &params, inradius);
        assert!(val > 0.0, "Outside point should be positive, got {}", val);
    }

    #[test]
    fn test_sdf_pumped_outside_z() {
        let square = test_square();
        let params = ExtrudeParams::default();
        let inradius = 1.0;
        // Far above in Z should be positive
        let val = sdf_pumped(Vec3::new(0.0, 0.0, 10.0), &square, &params, inradius);
        assert!(val > 0.0, "Above point should be positive, got {}", val);
    }

    #[test]
    fn test_extruder_default() {
        let ext = Extruder::new();
        assert!(ext.dirty);
        assert!(ext.mesh_vertices.is_empty());
        assert!(ext.mesh_indices.is_empty());
        assert!(ext.gpu_vertex_buffer.is_none());
        assert!(ext.gpu_index_buffer.is_none());
    }

    #[test]
    fn test_generate_preview_empty() {
        let mut ext = Extruder::new();
        let result = ext.generate_preview(&[]);
        assert!(!result, "Empty outlines should produce no mesh");
    }

    #[test]
    fn test_generate_preview_too_few_points() {
        let mut ext = Extruder::new();
        let outline = Outline2D {
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            closed: false,
        };
        let result = ext.generate_preview(&[outline]);
        assert!(!result, "2-point outline should produce no mesh");
    }

    #[test]
    fn test_generate_preview_square() {
        let mut ext = Extruder::new();
        ext.params.mc_resolution = 16; // Low res for fast test
        let outline = Outline2D {
            points: vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]],
            closed: true,
        };
        let result = ext.generate_preview(&[outline]);
        assert!(result, "Square outline should produce a mesh");
        assert!(!ext.mesh_vertices.is_empty());
        assert!(!ext.mesh_indices.is_empty());
        assert_eq!(ext.mesh_indices.len() % 3, 0, "Indices should be triangles");
    }

    #[test]
    fn test_extruder_has_mesh() {
        let mut ext = Extruder::new();
        assert!(!ext.has_mesh());

        ext.params.mc_resolution = 16;
        let outline = Outline2D {
            points: vec![[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]],
            closed: true,
        };
        ext.generate_preview(&[outline]);
        assert!(ext.has_mesh());
    }
}
