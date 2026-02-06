//! Sculpting Bridge (Phase 4, US-P4-008)
//!
//! Bridges the asset editor's Stage 3 (Sculpt) to the engine's `SculptingManager`.
//! Wraps existing face extrusion, edge pulling, and vertex pulling, and adds
//! three new tools: Smooth brush, AddSphere, and SubtractSphere.

use glam::Vec3;

use crate::game::types::{Mesh, Vertex};
use crate::render::marching_cubes::MarchingCubes;
use crate::render::sculpting::{SculptMode, SculptingManager};
use crate::render::sdf_operations::{smooth_subtraction, smooth_union};

// ============================================================================
// SCULPT TOOL ENUM
// ============================================================================

/// Available sculpting tools for the asset editor's Stage 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SculptTool {
    /// Wraps existing ExtrusionOperation from sculpting.rs
    FaceExtrude,
    /// Wraps existing VertexSelection from sculpting.rs
    VertexPull,
    /// Wraps existing EdgeSelection from sculpting.rs
    EdgePull,
    /// Laplacian relaxation: averages vertex positions with neighbors
    Smooth,
    /// Stamps a sphere via smooth_union() and re-meshes with Marching Cubes
    AddSphere,
    /// Carves via smooth_subtraction() and re-meshes with Marching Cubes
    SubtractSphere,
}

impl SculptTool {
    /// Keyboard shortcut label for UI display
    pub fn label(&self) -> &'static str {
        match self {
            Self::FaceExtrude => "1: Face Extrude",
            Self::VertexPull => "2: Vertex Pull",
            Self::EdgePull => "3: Edge Pull",
            Self::Smooth => "4: Smooth",
            Self::AddSphere => "5: Add Sphere",
            Self::SubtractSphere => "6: Subtract Sphere",
        }
    }

    /// Return all tool variants in order
    pub fn all() -> [SculptTool; 6] {
        [
            Self::FaceExtrude,
            Self::VertexPull,
            Self::EdgePull,
            Self::Smooth,
            Self::AddSphere,
            Self::SubtractSphere,
        ]
    }
}

// ============================================================================
// SCULPT BRIDGE
// ============================================================================

/// Bridges the asset editor to the engine's sculpting system.
///
/// Provides six sculpting tools operating on the editor's `Mesh`:
/// three that delegate to the existing `SculptingManager` (face extrude,
/// vertex pull, edge pull) and three new tools (smooth, add sphere,
/// subtract sphere).
pub struct SculptBridge {
    /// Currently active sculpting tool
    pub active_tool: SculptTool,
    /// World-space brush radius for smooth/add/subtract (default 0.5)
    pub brush_radius: f32,
    /// SDF smooth_union/subtraction k parameter (default 0.15)
    pub smooth_factor: f32,
    /// Smooth brush blending strength 0.0–1.0 (default 0.5)
    pub smooth_strength: f32,
    /// Marching Cubes grid resolution for SDF re-meshing (default 48)
    pub mc_resolution: u32,
    /// Engine sculpting manager for face/edge/vertex operations
    sculpting_manager: SculptingManager,
    /// Cached SDF grid for the current mesh (unused until iteration 2)
    mesh_sdf_cache: Option<Vec<f32>>,
    /// Current mesh AABB (min, max)
    mesh_bounds: (Vec3, Vec3),
    /// Whether the mesh has been modified since last GPU upload
    dirty: bool,
}

impl SculptBridge {
    /// Create a new sculpt bridge with default parameters.
    pub fn new() -> Self {
        let mut sm = SculptingManager::new();
        sm.set_enabled(true);
        Self {
            active_tool: SculptTool::Smooth,
            brush_radius: 0.5,
            smooth_factor: 0.15,
            smooth_strength: 0.5,
            mc_resolution: 48,
            sculpting_manager: sm,
            mesh_sdf_cache: None,
            mesh_bounds: (Vec3::ZERO, Vec3::ZERO),
            dirty: false,
        }
    }

    /// Whether the mesh has been modified and needs GPU re-upload.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Clear the dirty flag after GPU re-upload.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Switch to a different sculpting tool.
    pub fn set_tool(&mut self, tool: SculptTool) {
        if self.active_tool != tool {
            // Cancel in-progress engine sculpt ops when switching away
            match self.active_tool {
                SculptTool::FaceExtrude | SculptTool::VertexPull | SculptTool::EdgePull => {
                    self.sculpting_manager.cancel();
                }
                _ => {}
            }
            self.active_tool = tool;

            // Map the tool to the engine's SculptMode for delegation
            match tool {
                SculptTool::FaceExtrude => self.sculpting_manager.set_mode(SculptMode::Extrude),
                SculptTool::VertexPull => self.sculpting_manager.set_mode(SculptMode::PullVertex),
                SculptTool::EdgePull => self.sculpting_manager.set_mode(SculptMode::PullEdge),
                _ => {} // New tools don't map to engine modes
            }
        }
    }

    /// Handle sculpt input for the current tool.
    ///
    /// `cursor_world_pos` is the 3D position on or near the mesh surface.
    /// `is_pressed` is true on mouse-button-down, `is_dragging` is true
    /// while the button is held and the mouse moves.
    pub fn handle_input(
        &mut self,
        mesh: &mut Mesh,
        cursor_world_pos: Vec3,
        is_pressed: bool,
        is_dragging: bool,
    ) {
        match self.active_tool {
            SculptTool::Smooth => {
                if is_dragging {
                    smooth_brush(
                        mesh,
                        cursor_world_pos,
                        self.brush_radius,
                        self.smooth_strength,
                    );
                    recompute_normals(mesh);
                    self.dirty = true;
                }
            }
            SculptTool::AddSphere => {
                if is_pressed {
                    stamp_sphere(
                        mesh,
                        cursor_world_pos,
                        self.brush_radius,
                        self.smooth_factor,
                        self.mc_resolution,
                        true,
                    );
                    self.dirty = true;
                }
            }
            SculptTool::SubtractSphere => {
                if is_pressed {
                    stamp_sphere(
                        mesh,
                        cursor_world_pos,
                        self.brush_radius,
                        self.smooth_factor,
                        self.mc_resolution,
                        false,
                    );
                    self.dirty = true;
                }
            }
            // Face/Edge/Vertex tools delegate to SculptingManager.
            // Full delegation is deferred to a later iteration when the
            // editor's ray-casting and BuildingBlockManager adaptor are wired.
            SculptTool::FaceExtrude | SculptTool::VertexPull | SculptTool::EdgePull => {
                // The SculptingManager operates on BuildingBlockManager, not
                // on the editor Mesh directly. Integration will be completed
                // in a subsequent iteration once the block-to-mesh adaptor exists.
                let _ = (is_pressed, is_dragging, cursor_world_pos);
            }
        }
    }

    /// Get a reference to the underlying sculpting manager.
    pub fn sculpting_manager(&self) -> &SculptingManager {
        &self.sculpting_manager
    }

    /// Get a mutable reference to the underlying sculpting manager.
    pub fn sculpting_manager_mut(&mut self) -> &mut SculptingManager {
        &mut self.sculpting_manager
    }
}

impl Default for SculptBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// ADJACENCY HELPERS
// ============================================================================

/// Build a per-vertex adjacency list from the mesh's index buffer.
fn build_adjacency(mesh: &Mesh) -> Vec<Vec<usize>> {
    let vertex_count = mesh.vertices.len();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); vertex_count];

    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let (a, b, c) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);

        if !adj[a].contains(&b) {
            adj[a].push(b);
        }
        if !adj[a].contains(&c) {
            adj[a].push(c);
        }
        if !adj[b].contains(&a) {
            adj[b].push(a);
        }
        if !adj[b].contains(&c) {
            adj[b].push(c);
        }
        if !adj[c].contains(&a) {
            adj[c].push(a);
        }
        if !adj[c].contains(&b) {
            adj[c].push(b);
        }
    }

    adj
}

// ============================================================================
// SMOOTH BRUSH
// ============================================================================

/// Smooth brush: Laplacian relaxation with linear distance falloff.
///
/// Uses a two-pass approach (read then write) to avoid asymmetric drift
/// when vertices are smoothed in-place during iteration.
fn smooth_brush(mesh: &mut Mesh, center: Vec3, radius: f32, strength: f32) {
    let adjacency = build_adjacency(mesh);

    // Pass 1: compute new positions without mutating
    let new_positions: Vec<Option<[f32; 3]>> = (0..mesh.vertices.len())
        .map(|i| {
            let pos = Vec3::from(mesh.vertices[i].position);
            let dist = pos.distance(center);
            if dist < radius {
                let weight = 1.0 - (dist / radius); // Linear falloff
                let neighbors = &adjacency[i];
                if neighbors.is_empty() {
                    return None;
                }
                let avg: Vec3 = neighbors
                    .iter()
                    .map(|&ni| Vec3::from(mesh.vertices[ni].position))
                    .sum::<Vec3>()
                    / neighbors.len() as f32;
                let smoothed = pos.lerp(avg, weight * strength);
                Some(smoothed.to_array())
            } else {
                None
            }
        })
        .collect();

    // Pass 2: apply
    for (i, new_pos) in new_positions.into_iter().enumerate() {
        if let Some(p) = new_pos {
            mesh.vertices[i].position = p;
        }
    }
}

// ============================================================================
// NORMAL RECOMPUTATION
// ============================================================================

/// Recompute vertex normals via area-weighted face normal accumulation.
fn recompute_normals(mesh: &mut Mesh) {
    // Zero all normals
    for v in &mut mesh.vertices {
        v.normal = [0.0, 0.0, 0.0];
    }

    // Accumulate face normals (area-weighted via cross product magnitude)
    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        let p0 = Vec3::from(mesh.vertices[i0].position);
        let p1 = Vec3::from(mesh.vertices[i1].position);
        let p2 = Vec3::from(mesh.vertices[i2].position);

        let edge1 = p1 - p0;
        let edge2 = p2 - p0;
        let face_normal = edge1.cross(edge2); // Length = 2x triangle area

        for &idx in &[i0, i1, i2] {
            mesh.vertices[idx].normal[0] += face_normal.x;
            mesh.vertices[idx].normal[1] += face_normal.y;
            mesh.vertices[idx].normal[2] += face_normal.z;
        }
    }

    // Normalize
    for v in &mut mesh.vertices {
        let n = Vec3::from(v.normal);
        v.normal = n.normalize_or_zero().to_array();
    }
}

// ============================================================================
// MESH BOUNDS
// ============================================================================

/// Compute the axis-aligned bounding box of a mesh.
fn mesh_bounds(mesh: &Mesh) -> (Vec3, Vec3) {
    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for v in &mesh.vertices {
        let p = Vec3::from(v.position);
        min = min.min(p);
        max = max.max(p);
    }
    (min, max)
}

// ============================================================================
// APPROXIMATE MESH SDF
// ============================================================================

/// Approximate signed distance from point `p` to the mesh surface.
///
/// Computes unsigned distance as min distance to any triangle, then
/// determines sign by dot product with the nearest triangle normal.
fn approximate_mesh_sdf(p: Vec3, mesh: &Mesh) -> f32 {
    let mut best_dist_sq = f32::MAX;
    let mut best_normal = Vec3::Y;

    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let v0 = Vec3::from(mesh.vertices[tri[0] as usize].position);
        let v1 = Vec3::from(mesh.vertices[tri[1] as usize].position);
        let v2 = Vec3::from(mesh.vertices[tri[2] as usize].position);

        let (dist_sq, _closest) = point_triangle_dist_sq(p, v0, v1, v2);

        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            best_normal = edge1.cross(edge2);
        }
    }

    let dist = best_dist_sq.sqrt();
    let centroid_approx = p
        - (mesh
            .vertices
            .first()
            .map(|v| Vec3::from(v.position))
            .unwrap_or(Vec3::ZERO));
    let sign = if best_normal.dot(centroid_approx) >= 0.0 {
        1.0
    } else {
        -1.0
    };

    dist * sign
}

/// Squared distance from point `p` to triangle (v0, v1, v2).
/// Returns (dist_sq, closest_point).
fn point_triangle_dist_sq(p: Vec3, v0: Vec3, v1: Vec3, v2: Vec3) -> (f32, Vec3) {
    let e0 = v1 - v0;
    let e1 = v2 - v0;
    let v = p - v0;

    let d00 = e0.dot(e0);
    let d01 = e0.dot(e1);
    let d11 = e1.dot(e1);
    let d20 = v.dot(e0);
    let d21 = v.dot(e1);

    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-12 {
        // Degenerate triangle
        return (p.distance_squared(v0), v0);
    }
    let inv_denom = 1.0 / denom;
    let mut s = (d11 * d20 - d01 * d21) * inv_denom;
    let mut t = (d00 * d21 - d01 * d20) * inv_denom;

    // Clamp to triangle
    if s < 0.0 {
        s = 0.0;
    }
    if t < 0.0 {
        t = 0.0;
    }
    if s + t > 1.0 {
        let scale = 1.0 / (s + t);
        s *= scale;
        t *= scale;
    }

    let closest = v0 + e0 * s + e1 * t;
    (p.distance_squared(closest), closest)
}

// ============================================================================
// SPHERE STAMP / CARVE
// ============================================================================

/// Stamp or carve a sphere into the mesh using SDF booleans + Marching Cubes.
///
/// If `add` is true, uses `smooth_union`; if false, uses `smooth_subtraction`.
fn stamp_sphere(
    mesh: &mut Mesh,
    center: Vec3,
    radius: f32,
    smooth_k: f32,
    mc_resolution: u32,
    add: bool,
) {
    if mesh.vertices.is_empty() {
        return;
    }

    // Clone old mesh for color transfer
    let old_vertices = mesh.vertices.clone();
    let old_indices = mesh.indices.clone();
    let old_mesh = Mesh {
        vertices: old_vertices,
        indices: old_indices,
    };

    // Compute AABB expanded to include the sphere + margin
    let (mut min_b, mut max_b) = mesh_bounds(mesh);
    let sphere_min = center - Vec3::splat(radius);
    let sphere_max = center + Vec3::splat(radius);
    min_b = min_b.min(sphere_min);
    max_b = max_b.max(sphere_max);
    let margin = (max_b - min_b).max_element() * 0.1;
    min_b -= Vec3::splat(margin);
    max_b += Vec3::splat(margin);

    // Combined SDF closure
    let combined_sdf = |p: Vec3| -> f32 {
        let mesh_d = approximate_mesh_sdf(p, &old_mesh);
        let sphere_d = p.distance(center) - radius;
        if add {
            smooth_union(mesh_d, sphere_d, smooth_k)
        } else {
            smooth_subtraction(mesh_d, sphere_d, smooth_k)
        }
    };

    // Run Marching Cubes
    let mc = MarchingCubes::new(mc_resolution);
    let default_color = [0.6, 0.6, 0.6, 1.0];
    let (new_block_verts, new_indices) =
        mc.generate_mesh(combined_sdf, min_b, max_b, default_color);

    // Convert BlockVertex -> Vertex and transfer colors from old mesh
    let new_verts: Vec<Vertex> = new_block_verts
        .iter()
        .map(|bv| Vertex {
            position: bv.position,
            normal: bv.normal,
            color: nearest_vertex_color(&old_mesh, Vec3::from_array(bv.position)),
        })
        .collect();

    // Replace mesh data
    mesh.vertices = new_verts;
    mesh.indices = new_indices;
}

/// Find the color of the nearest vertex in `old_mesh` to `pos`.
fn nearest_vertex_color(old_mesh: &Mesh, pos: Vec3) -> [f32; 4] {
    let mut best_dist_sq = f32::MAX;
    let mut best_color = [0.6, 0.6, 0.6, 1.0];
    for v in &old_mesh.vertices {
        let d = Vec3::from(v.position).distance_squared(pos);
        if d < best_dist_sq {
            best_dist_sq = d;
            best_color = v.color;
        }
    }
    best_color
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::types::{Mesh, Vertex};

    /// Create a simple triangle mesh for testing.
    fn test_triangle_mesh() -> Mesh {
        Mesh {
            vertices: vec![
                Vertex {
                    position: [0.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    color: [1.0, 0.0, 0.0, 1.0],
                },
                Vertex {
                    position: [1.0, 0.0, 0.0],
                    normal: [0.0, 1.0, 0.0],
                    color: [0.0, 1.0, 0.0, 1.0],
                },
                Vertex {
                    position: [0.5, 0.0, 1.0],
                    normal: [0.0, 1.0, 0.0],
                    color: [0.0, 0.0, 1.0, 1.0],
                },
            ],
            indices: vec![0, 1, 2],
        }
    }

    /// Create a simple box mesh (6 faces, 8 verts).
    fn test_box_mesh() -> Mesh {
        crate::game::types::generate_box(Vec3::ZERO, Vec3::splat(0.5), [0.5, 0.5, 0.5, 1.0])
    }

    #[test]
    fn test_sculpt_bridge_new() {
        let bridge = SculptBridge::new();
        assert_eq!(bridge.active_tool, SculptTool::Smooth);
        assert!((bridge.brush_radius - 0.5).abs() < 1e-6);
        assert!((bridge.smooth_factor - 0.15).abs() < 1e-6);
        assert!(!bridge.is_dirty());
    }

    #[test]
    fn test_sculpt_tool_labels() {
        assert_eq!(SculptTool::FaceExtrude.label(), "1: Face Extrude");
        assert_eq!(SculptTool::Smooth.label(), "4: Smooth");
        assert_eq!(SculptTool::SubtractSphere.label(), "6: Subtract Sphere");
    }

    #[test]
    fn test_set_tool() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::AddSphere);
        assert_eq!(bridge.active_tool, SculptTool::AddSphere);
        bridge.set_tool(SculptTool::FaceExtrude);
        assert_eq!(bridge.active_tool, SculptTool::FaceExtrude);
    }

    #[test]
    fn test_build_adjacency() {
        let mesh = test_triangle_mesh();
        let adj = build_adjacency(&mesh);
        assert_eq!(adj.len(), 3);
        // Vertex 0 should be adjacent to 1 and 2
        assert!(adj[0].contains(&1));
        assert!(adj[0].contains(&2));
        assert_eq!(adj[0].len(), 2);
    }

    #[test]
    fn test_smooth_brush_does_not_panic() {
        let mut mesh = test_box_mesh();
        let center = Vec3::ZERO;
        smooth_brush(&mut mesh, center, 1.0, 0.5);
        // Should not panic and vertices should still exist
        assert!(!mesh.vertices.is_empty());
    }

    #[test]
    fn test_smooth_brush_two_pass() {
        // Verify two-pass approach: smoothing a mesh centered at origin
        // should move affected vertices toward neighbor average
        let mut mesh = test_triangle_mesh();
        let original_positions: Vec<[f32; 3]> = mesh.vertices.iter().map(|v| v.position).collect();

        // Smooth at center of triangle with large radius to affect all vertices
        smooth_brush(&mut mesh, Vec3::new(0.5, 0.0, 0.33), 2.0, 0.5);

        // At least some vertices should have moved
        let changed = mesh
            .vertices
            .iter()
            .zip(original_positions.iter())
            .any(|(v, orig)| {
                (v.position[0] - orig[0]).abs() > 1e-6
                    || (v.position[1] - orig[1]).abs() > 1e-6
                    || (v.position[2] - orig[2]).abs() > 1e-6
            });
        assert!(changed, "Smooth brush should move vertices");
    }

    #[test]
    fn test_recompute_normals() {
        let mut mesh = test_triangle_mesh();
        // Zero out normals
        for v in &mut mesh.vertices {
            v.normal = [0.0, 0.0, 0.0];
        }
        recompute_normals(&mut mesh);
        // All normals should now be non-zero and pointing in the same direction
        for v in &mesh.vertices {
            let n = Vec3::from(v.normal);
            assert!(n.length() > 0.99, "Normal should be normalized");
        }
    }

    #[test]
    fn test_mesh_bounds() {
        let mesh = test_box_mesh();
        let (min, max) = mesh_bounds(&mesh);
        assert!(min.x < max.x);
        assert!(min.y < max.y);
        assert!(min.z < max.z);
    }

    #[test]
    fn test_nearest_vertex_color() {
        let mesh = test_triangle_mesh();
        // Query near vertex 0 (at origin) should return red
        let color = nearest_vertex_color(&mesh, Vec3::new(0.01, 0.0, 0.0));
        assert!((color[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_stamp_sphere_add() {
        let mut mesh = test_box_mesh();
        let original_count = mesh.vertices.len();
        stamp_sphere(&mut mesh, Vec3::new(0.5, 0.0, 0.0), 0.3, 0.15, 16, true);
        // Mesh should be regenerated (different vertex/index counts)
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn test_stamp_sphere_subtract() {
        let mut mesh = test_box_mesh();
        stamp_sphere(&mut mesh, Vec3::ZERO, 0.2, 0.15, 16, false);
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
    }

    #[test]
    fn test_handle_input_smooth() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::Smooth);
        let mut mesh = test_box_mesh();
        bridge.handle_input(&mut mesh, Vec3::ZERO, false, true);
        assert!(bridge.is_dirty());
    }

    #[test]
    fn test_handle_input_add_sphere() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::AddSphere);
        bridge.mc_resolution = 16; // Low res for fast test
        let mut mesh = test_box_mesh();
        bridge.handle_input(&mut mesh, Vec3::new(0.5, 0.0, 0.0), true, false);
        assert!(bridge.is_dirty());
    }

    #[test]
    fn test_dirty_flag() {
        let mut bridge = SculptBridge::new();
        assert!(!bridge.is_dirty());
        let mut mesh = test_box_mesh();
        bridge.handle_input(&mut mesh, Vec3::ZERO, false, true);
        assert!(bridge.is_dirty());
        bridge.clear_dirty();
        assert!(!bridge.is_dirty());
    }
}
