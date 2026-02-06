//! Sculpting Bridge (Phase 4, US-P4-008)
//!
//! Bridges the asset editor's Stage 3 (Sculpt) to the engine's `SculptingManager`.
//! Wraps existing face extrusion, edge pulling, and vertex pulling, and adds
//! three new tools: Smooth brush, AddSphere, and SubtractSphere.
//!
//! ## Iteration 2 Changes
//! - Direct mesh-level face extrude, vertex pull, and edge pull operations
//!   that work on the editor's `Mesh` type (the engine's `SculptingManager`
//!   operates on `BuildingBlockManager`, so we provide a mesh-native adaptor)
//! - SDF grid caching for faster repeated sphere stamps
//! - Improved sign determination in approximate_mesh_sdf

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
    /// Extrude selected face along its normal
    FaceExtrude,
    /// Pull a vertex along drag direction
    VertexPull,
    /// Pull an edge along drag direction
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
// MESH-LEVEL SELECTION STATE
// ============================================================================

/// A face selected on the editor mesh (triangle index + cached normal).
#[derive(Debug, Clone, Copy)]
pub struct MeshFaceSelection {
    /// Triangle index (into mesh.indices, i.e. triangle = indices[tri_idx*3 .. tri_idx*3+3])
    pub tri_index: usize,
    /// Face normal (outward)
    pub normal: Vec3,
    /// Center of the face
    pub center: Vec3,
}

/// A vertex selected on the editor mesh.
#[derive(Debug, Clone, Copy)]
pub struct MeshVertexSelection {
    /// Vertex index in `mesh.vertices`
    pub vertex_index: usize,
    /// Original position at the time of selection
    pub original_pos: Vec3,
}

/// An edge selected on the editor mesh.
#[derive(Debug, Clone, Copy)]
pub struct MeshEdgeSelection {
    /// First vertex index of the edge
    pub v0: usize,
    /// Second vertex index of the edge
    pub v1: usize,
    /// Original positions at the time of selection
    pub original_v0: Vec3,
    pub original_v1: Vec3,
}

/// Active drag state for mesh-level face/vertex/edge operations.
#[derive(Debug, Clone)]
enum MeshDragState {
    Idle,
    FaceDrag {
        selection: MeshFaceSelection,
        /// Vertex indices of the selected face
        face_verts: [usize; 3],
        /// Original positions before drag
        original_positions: [Vec3; 3],
        /// Start drag position
        start_pos: Vec3,
    },
    VertexDrag {
        selection: MeshVertexSelection,
        start_pos: Vec3,
    },
    EdgeDrag {
        selection: MeshEdgeSelection,
        start_pos: Vec3,
    },
}

// ============================================================================
// SCULPT BRIDGE
// ============================================================================

/// Bridges the asset editor to the engine's sculpting system.
///
/// Provides six sculpting tools operating on the editor's `Mesh`:
/// - FaceExtrude, VertexPull, EdgePull: direct mesh-level operations
/// - Smooth, AddSphere, SubtractSphere: new tools using SDF booleans
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
    /// Engine sculpting manager (mode delegation for block-based workflows)
    sculpting_manager: SculptingManager,
    /// Cached SDF grid for the current mesh (invalidated on mesh changes)
    mesh_sdf_cache: Option<SdfCache>,
    /// Current mesh AABB (min, max) — updated on mesh modification
    mesh_bounds: (Vec3, Vec3),
    /// Whether the mesh has been modified since last GPU upload
    dirty: bool,
    /// Active drag state for face/vertex/edge operations
    drag_state: MeshDragState,
}

/// Cached SDF grid for speeding up repeated sphere stamp operations.
struct SdfCache {
    /// Flattened 3D grid of SDF values (resolution^3)
    grid: Vec<f32>,
    /// Grid resolution per axis
    resolution: u32,
    /// AABB min of the cached region
    min: Vec3,
    /// AABB max of the cached region
    max: Vec3,
}

impl SdfCache {
    /// Sample the cached SDF at a world-space point via trilinear interpolation.
    fn sample(&self, p: Vec3) -> f32 {
        let size = self.max - self.min;
        // Normalize to [0, resolution-1] range
        let local = (p - self.min) / size * (self.resolution as f32 - 1.0);

        let ix = (local.x.floor() as i32).clamp(0, self.resolution as i32 - 2) as usize;
        let iy = (local.y.floor() as i32).clamp(0, self.resolution as i32 - 2) as usize;
        let iz = (local.z.floor() as i32).clamp(0, self.resolution as i32 - 2) as usize;

        let fx = (local.x - ix as f32).clamp(0.0, 1.0);
        let fy = (local.y - iy as f32).clamp(0.0, 1.0);
        let fz = (local.z - iz as f32).clamp(0.0, 1.0);

        let res = self.resolution as usize;
        let idx = |x: usize, y: usize, z: usize| -> usize { (z * res + y) * res + x };

        // Trilinear interpolation
        let c000 = self.grid[idx(ix, iy, iz)];
        let c100 = self.grid[idx(ix + 1, iy, iz)];
        let c010 = self.grid[idx(ix, iy + 1, iz)];
        let c110 = self.grid[idx(ix + 1, iy + 1, iz)];
        let c001 = self.grid[idx(ix, iy, iz + 1)];
        let c101 = self.grid[idx(ix + 1, iy, iz + 1)];
        let c011 = self.grid[idx(ix, iy + 1, iz + 1)];
        let c111 = self.grid[idx(ix + 1, iy + 1, iz + 1)];

        let c00 = c000 * (1.0 - fx) + c100 * fx;
        let c10 = c010 * (1.0 - fx) + c110 * fx;
        let c01 = c001 * (1.0 - fx) + c101 * fx;
        let c11 = c011 * (1.0 - fx) + c111 * fx;

        let c0 = c00 * (1.0 - fy) + c10 * fy;
        let c1 = c01 * (1.0 - fy) + c11 * fy;

        c0 * (1.0 - fz) + c1 * fz
    }
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
            drag_state: MeshDragState::Idle,
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
            // Cancel in-progress operations when switching tools
            match self.active_tool {
                SculptTool::FaceExtrude | SculptTool::VertexPull | SculptTool::EdgePull => {
                    self.sculpting_manager.cancel();
                    self.drag_state = MeshDragState::Idle;
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

    /// Invalidate the SDF cache (must be called when the mesh changes externally).
    pub fn invalidate_cache(&mut self) {
        self.mesh_sdf_cache = None;
    }

    /// Build (or return existing) SDF cache for the given mesh.
    fn ensure_sdf_cache(&mut self, mesh: &Mesh, extra_min: Vec3, extra_max: Vec3) {
        // Check if existing cache covers the required bounds
        if let Some(ref cache) = self.mesh_sdf_cache {
            let covers_min = cache.min.x <= extra_min.x
                && cache.min.y <= extra_min.y
                && cache.min.z <= extra_min.z;
            let covers_max = cache.max.x >= extra_max.x
                && cache.max.y >= extra_max.y
                && cache.max.z >= extra_max.z;
            if covers_min && covers_max {
                return; // Cache is still valid
            }
        }

        // Rebuild cache
        let (mut min_b, mut max_b) = mesh_bounds(mesh);
        min_b = min_b.min(extra_min);
        max_b = max_b.max(extra_max);
        let margin = (max_b - min_b).max_element() * 0.1;
        min_b -= Vec3::splat(margin);
        max_b += Vec3::splat(margin);

        let res = self.mc_resolution;
        let size = max_b - min_b;
        let mut grid = vec![0.0f32; (res * res * res) as usize];

        for iz in 0..res {
            for iy in 0..res {
                for ix in 0..res {
                    let p = min_b
                        + Vec3::new(ix as f32, iy as f32, iz as f32) / (res as f32 - 1.0) * size;
                    let idx = ((iz * res + iy) * res + ix) as usize;
                    grid[idx] = approximate_mesh_sdf(p, mesh);
                }
            }
        }

        self.mesh_sdf_cache = Some(SdfCache {
            grid,
            resolution: res,
            min: min_b,
            max: max_b,
        });
        self.mesh_bounds = (min_b, max_b);
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
                    self.mesh_sdf_cache = None; // Invalidate cache
                    self.dirty = true;
                }
            }
            SculptTool::AddSphere => {
                if is_pressed {
                    self.stamp_sphere_cached(mesh, cursor_world_pos, true);
                    self.dirty = true;
                }
            }
            SculptTool::SubtractSphere => {
                if is_pressed {
                    self.stamp_sphere_cached(mesh, cursor_world_pos, false);
                    self.dirty = true;
                }
            }
            SculptTool::FaceExtrude => {
                self.handle_face_extrude(mesh, cursor_world_pos, is_pressed, is_dragging);
            }
            SculptTool::VertexPull => {
                self.handle_vertex_pull(mesh, cursor_world_pos, is_pressed, is_dragging);
            }
            SculptTool::EdgePull => {
                self.handle_edge_pull(mesh, cursor_world_pos, is_pressed, is_dragging);
            }
        }
    }

    /// Stamp a sphere using the SDF cache for faster repeated operations.
    fn stamp_sphere_cached(&mut self, mesh: &mut Mesh, center: Vec3, add: bool) {
        if mesh.vertices.is_empty() {
            return;
        }

        let radius = self.brush_radius;
        let smooth_k = self.smooth_factor;
        let mc_resolution = self.mc_resolution;

        // Compute bounds including sphere
        let sphere_min = center - Vec3::splat(radius);
        let sphere_max = center + Vec3::splat(radius);

        // Build SDF cache if needed
        self.ensure_sdf_cache(mesh, sphere_min, sphere_max);

        // Clone old mesh for color transfer
        let old_mesh = Mesh {
            vertices: mesh.vertices.clone(),
            indices: mesh.indices.clone(),
        };

        // Use cached bounds (which are already expanded)
        let cache = self.mesh_sdf_cache.as_ref().unwrap();
        let min_b = cache.min;
        let max_b = cache.max;

        // Combined SDF closure using cached grid
        let combined_sdf = |p: Vec3| -> f32 {
            let mesh_d = cache.sample(p);
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

        // Invalidate cache since mesh changed
        self.mesh_sdf_cache = None;
    }

    // ========================================================================
    // FACE EXTRUDE (mesh-level)
    // ========================================================================

    /// Handle face extrude input: select a face on press, extrude along normal
    /// while dragging, finalize on release.
    fn handle_face_extrude(
        &mut self,
        mesh: &mut Mesh,
        cursor_pos: Vec3,
        is_pressed: bool,
        is_dragging: bool,
    ) {
        if is_pressed {
            // Find nearest triangle face to the cursor
            if let Some(sel) = find_nearest_face(mesh, cursor_pos) {
                let i0 = mesh.indices[sel.tri_index * 3] as usize;
                let i1 = mesh.indices[sel.tri_index * 3 + 1] as usize;
                let i2 = mesh.indices[sel.tri_index * 3 + 2] as usize;

                self.drag_state = MeshDragState::FaceDrag {
                    selection: sel,
                    face_verts: [i0, i1, i2],
                    original_positions: [
                        Vec3::from(mesh.vertices[i0].position),
                        Vec3::from(mesh.vertices[i1].position),
                        Vec3::from(mesh.vertices[i2].position),
                    ],
                    start_pos: cursor_pos,
                };
            }
        } else if is_dragging {
            if let MeshDragState::FaceDrag {
                ref selection,
                face_verts,
                original_positions,
                start_pos,
            } = self.drag_state
            {
                // Project drag vector onto the face normal
                let drag_vec = cursor_pos - start_pos;
                let extrude_dist = drag_vec.dot(selection.normal);
                let offset = selection.normal * extrude_dist;

                // Move face vertices along the normal
                for (vi, &vert_idx) in face_verts.iter().enumerate() {
                    let new_pos = original_positions[vi] + offset;
                    mesh.vertices[vert_idx].position = new_pos.to_array();
                }
                recompute_normals(mesh);
                self.mesh_sdf_cache = None;
                self.dirty = true;
            }
        } else {
            // Release: finalize the drag
            if matches!(self.drag_state, MeshDragState::FaceDrag { .. }) {
                self.drag_state = MeshDragState::Idle;
            }
        }
    }

    // ========================================================================
    // VERTEX PULL (mesh-level)
    // ========================================================================

    /// Handle vertex pull input: select nearest vertex on press, move it while
    /// dragging, finalize on release.
    fn handle_vertex_pull(
        &mut self,
        mesh: &mut Mesh,
        cursor_pos: Vec3,
        is_pressed: bool,
        is_dragging: bool,
    ) {
        if is_pressed {
            if let Some(sel) = find_nearest_vertex(mesh, cursor_pos, self.brush_radius) {
                self.drag_state = MeshDragState::VertexDrag {
                    selection: sel,
                    start_pos: cursor_pos,
                };
            }
        } else if is_dragging {
            if let MeshDragState::VertexDrag {
                ref selection,
                start_pos,
            } = self.drag_state
            {
                let drag_vec = cursor_pos - start_pos;
                let new_pos = selection.original_pos + drag_vec;
                mesh.vertices[selection.vertex_index].position = new_pos.to_array();
                recompute_normals(mesh);
                self.mesh_sdf_cache = None;
                self.dirty = true;
            }
        } else if matches!(self.drag_state, MeshDragState::VertexDrag { .. }) {
            self.drag_state = MeshDragState::Idle;
        }
    }

    // ========================================================================
    // EDGE PULL (mesh-level)
    // ========================================================================

    /// Handle edge pull input: select nearest edge on press, move both vertices
    /// while dragging, finalize on release.
    fn handle_edge_pull(
        &mut self,
        mesh: &mut Mesh,
        cursor_pos: Vec3,
        is_pressed: bool,
        is_dragging: bool,
    ) {
        if is_pressed {
            if let Some(sel) = find_nearest_edge(mesh, cursor_pos, self.brush_radius) {
                self.drag_state = MeshDragState::EdgeDrag {
                    selection: sel,
                    start_pos: cursor_pos,
                };
            }
        } else if is_dragging {
            if let MeshDragState::EdgeDrag {
                ref selection,
                start_pos,
            } = self.drag_state
            {
                let drag_vec = cursor_pos - start_pos;
                let new_v0 = selection.original_v0 + drag_vec;
                let new_v1 = selection.original_v1 + drag_vec;
                mesh.vertices[selection.v0].position = new_v0.to_array();
                mesh.vertices[selection.v1].position = new_v1.to_array();
                recompute_normals(mesh);
                self.mesh_sdf_cache = None;
                self.dirty = true;
            }
        } else if matches!(self.drag_state, MeshDragState::EdgeDrag { .. }) {
            self.drag_state = MeshDragState::Idle;
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
// MESH-LEVEL SELECTION HELPERS
// ============================================================================

/// Find the nearest triangle face to `cursor_pos`.
fn find_nearest_face(mesh: &Mesh, cursor_pos: Vec3) -> Option<MeshFaceSelection> {
    let tri_count = mesh.indices.len() / 3;
    if tri_count == 0 {
        return None;
    }

    let mut best_dist_sq = f32::MAX;
    let mut best_tri = 0;
    let mut best_center = Vec3::ZERO;
    let mut best_normal = Vec3::Y;

    for t in 0..tri_count {
        let i0 = mesh.indices[t * 3] as usize;
        let i1 = mesh.indices[t * 3 + 1] as usize;
        let i2 = mesh.indices[t * 3 + 2] as usize;

        let p0 = Vec3::from(mesh.vertices[i0].position);
        let p1 = Vec3::from(mesh.vertices[i1].position);
        let p2 = Vec3::from(mesh.vertices[i2].position);

        let center = (p0 + p1 + p2) / 3.0;
        let dist_sq = cursor_pos.distance_squared(center);

        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            best_tri = t;
            best_center = center;
            let e1 = p1 - p0;
            let e2 = p2 - p0;
            best_normal = e1.cross(e2).normalize_or_zero();
        }
    }

    Some(MeshFaceSelection {
        tri_index: best_tri,
        normal: best_normal,
        center: best_center,
    })
}

/// Find the nearest vertex within `max_dist` of `cursor_pos`.
fn find_nearest_vertex(
    mesh: &Mesh,
    cursor_pos: Vec3,
    max_dist: f32,
) -> Option<MeshVertexSelection> {
    let max_dist_sq = max_dist * max_dist;
    let mut best_dist_sq = f32::MAX;
    let mut best_idx = 0;

    for (i, v) in mesh.vertices.iter().enumerate() {
        let d = Vec3::from(v.position).distance_squared(cursor_pos);
        if d < best_dist_sq {
            best_dist_sq = d;
            best_idx = i;
        }
    }

    if best_dist_sq <= max_dist_sq {
        Some(MeshVertexSelection {
            vertex_index: best_idx,
            original_pos: Vec3::from(mesh.vertices[best_idx].position),
        })
    } else {
        None
    }
}

/// Find the nearest edge within `max_dist` of `cursor_pos`.
fn find_nearest_edge(mesh: &Mesh, cursor_pos: Vec3, max_dist: f32) -> Option<MeshEdgeSelection> {
    let max_dist_sq = max_dist * max_dist;
    let mut best_dist_sq = f32::MAX;
    let mut best_v0 = 0;
    let mut best_v1 = 0;

    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let verts = [tri[0] as usize, tri[1] as usize, tri[2] as usize];

        // Check each edge of the triangle
        for &(a, b) in &[(0, 1), (1, 2), (2, 0)] {
            let va = Vec3::from(mesh.vertices[verts[a]].position);
            let vb = Vec3::from(mesh.vertices[verts[b]].position);
            let edge_center = (va + vb) * 0.5;
            let d = cursor_pos.distance_squared(edge_center);
            if d < best_dist_sq {
                best_dist_sq = d;
                best_v0 = verts[a];
                best_v1 = verts[b];
            }
        }
    }

    if best_dist_sq <= max_dist_sq {
        Some(MeshEdgeSelection {
            v0: best_v0,
            v1: best_v1,
            original_v0: Vec3::from(mesh.vertices[best_v0].position),
            original_v1: Vec3::from(mesh.vertices[best_v1].position),
        })
    } else {
        None
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
/// Computes unsigned distance as the minimum distance to any triangle, then
/// determines sign by the dot product of the displacement (from the closest
/// surface point to `p`) with the nearest triangle's face normal. Positive
/// means outside the mesh (same side as the normal), negative means inside.
fn approximate_mesh_sdf(p: Vec3, mesh: &Mesh) -> f32 {
    let mut best_dist_sq = f32::MAX;
    let mut best_closest = p;
    let mut best_normal = Vec3::Y;

    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }
        let v0 = Vec3::from(mesh.vertices[tri[0] as usize].position);
        let v1 = Vec3::from(mesh.vertices[tri[1] as usize].position);
        let v2 = Vec3::from(mesh.vertices[tri[2] as usize].position);

        let (dist_sq, closest) = point_triangle_dist_sq(p, v0, v1, v2);

        if dist_sq < best_dist_sq {
            best_dist_sq = dist_sq;
            best_closest = closest;
            let edge1 = v1 - v0;
            let edge2 = v2 - v0;
            best_normal = edge1.cross(edge2);
        }
    }

    let dist = best_dist_sq.sqrt();
    // Sign: positive if p is on the outward side of the nearest triangle
    let displacement = p - best_closest;
    let sign = if best_normal.dot(displacement) >= 0.0 {
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
// SPHERE STAMP / CARVE (non-cached fallback, used by tests)
// ============================================================================

/// Stamp or carve a sphere into the mesh using SDF booleans + Marching Cubes.
///
/// If `add` is true, uses `smooth_union`; if false, uses `smooth_subtraction`.
/// This standalone function does not use the SDF cache and is kept for testing
/// and for use without a `SculptBridge` instance.
#[cfg(test)]
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
    let old_mesh = Mesh {
        vertices: mesh.vertices.clone(),
        indices: mesh.indices.clone(),
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

    /// Create a simple box mesh (6 faces, 24 verts).
    fn test_box_mesh() -> Mesh {
        crate::game::types::generate_box(Vec3::ZERO, Vec3::splat(0.5), [0.5, 0.5, 0.5, 1.0])
    }

    #[test]
    fn test_sculpt_bridge_new() {
        let bridge = SculptBridge::new();
        assert_eq!(bridge.active_tool, SculptTool::Smooth);
        assert!((bridge.brush_radius - 0.5).abs() < 1e-6);
        assert!((bridge.smooth_factor - 0.15).abs() < 1e-6);
        assert!((bridge.smooth_strength - 0.5).abs() < 1e-6);
        assert_eq!(bridge.mc_resolution, 48);
        assert!(!bridge.is_dirty());
    }

    #[test]
    fn test_sculpt_bridge_default() {
        let bridge = SculptBridge::default();
        assert_eq!(bridge.active_tool, SculptTool::Smooth);
    }

    #[test]
    fn test_sculpt_tool_labels() {
        assert_eq!(SculptTool::FaceExtrude.label(), "1: Face Extrude");
        assert_eq!(SculptTool::VertexPull.label(), "2: Vertex Pull");
        assert_eq!(SculptTool::EdgePull.label(), "3: Edge Pull");
        assert_eq!(SculptTool::Smooth.label(), "4: Smooth");
        assert_eq!(SculptTool::AddSphere.label(), "5: Add Sphere");
        assert_eq!(SculptTool::SubtractSphere.label(), "6: Subtract Sphere");
    }

    #[test]
    fn test_sculpt_tool_all() {
        let all = SculptTool::all();
        assert_eq!(all.len(), 6);
        assert_eq!(all[0], SculptTool::FaceExtrude);
        assert_eq!(all[5], SculptTool::SubtractSphere);
    }

    #[test]
    fn test_set_tool() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::AddSphere);
        assert_eq!(bridge.active_tool, SculptTool::AddSphere);
        bridge.set_tool(SculptTool::FaceExtrude);
        assert_eq!(bridge.active_tool, SculptTool::FaceExtrude);
        // Setting same tool again should be a no-op
        bridge.set_tool(SculptTool::FaceExtrude);
        assert_eq!(bridge.active_tool, SculptTool::FaceExtrude);
    }

    #[test]
    fn test_set_tool_cancels_engine_ops() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::FaceExtrude);
        // Switching from a block-based tool should cancel engine ops
        bridge.set_tool(SculptTool::Smooth);
        assert_eq!(bridge.active_tool, SculptTool::Smooth);
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
    fn test_build_adjacency_box() {
        let mesh = test_box_mesh();
        let adj = build_adjacency(&mesh);
        assert_eq!(adj.len(), mesh.vertices.len());
        // Each face has 4 vertices, each vertex is shared by at most 3 faces
        // in a box mesh with separate faces
        for neighbors in &adj {
            assert!(!neighbors.is_empty());
        }
    }

    #[test]
    fn test_build_adjacency_empty() {
        let mesh = Mesh::new();
        let adj = build_adjacency(&mesh);
        assert!(adj.is_empty());
    }

    #[test]
    fn test_smooth_brush_does_not_panic() {
        let mut mesh = test_box_mesh();
        let center = Vec3::ZERO;
        smooth_brush(&mut mesh, center, 1.0, 0.5);
        assert!(!mesh.vertices.is_empty());
    }

    #[test]
    fn test_smooth_brush_two_pass() {
        let mut mesh = test_triangle_mesh();
        let original_positions: Vec<[f32; 3]> = mesh.vertices.iter().map(|v| v.position).collect();

        smooth_brush(&mut mesh, Vec3::new(0.5, 0.0, 0.33), 2.0, 0.5);

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
    fn test_smooth_brush_outside_radius_no_change() {
        let mut mesh = test_triangle_mesh();
        let original_positions: Vec<[f32; 3]> = mesh.vertices.iter().map(|v| v.position).collect();

        // Center very far from the mesh -- no vertex should be affected
        smooth_brush(&mut mesh, Vec3::new(100.0, 100.0, 100.0), 0.1, 0.5);

        for (v, orig) in mesh.vertices.iter().zip(original_positions.iter()) {
            assert!(
                (v.position[0] - orig[0]).abs() < 1e-10,
                "Vertex outside radius should not move"
            );
        }
    }

    #[test]
    fn test_recompute_normals() {
        let mut mesh = test_triangle_mesh();
        for v in &mut mesh.vertices {
            v.normal = [0.0, 0.0, 0.0];
        }
        recompute_normals(&mut mesh);
        for v in &mesh.vertices {
            let n = Vec3::from(v.normal);
            assert!(n.length() > 0.99, "Normal should be normalized");
        }
    }

    #[test]
    fn test_recompute_normals_consistency() {
        // All vertices of a flat triangle should have the same normal
        let mut m = test_triangle_mesh();
        recompute_normals(&mut m);
        let rn0 = Vec3::from(m.vertices[0].normal);
        let rn1 = Vec3::from(m.vertices[1].normal);
        let rn2 = Vec3::from(m.vertices[2].normal);
        // All normals should be equal for a flat triangle
        assert!(rn0.distance(rn1) < 1e-5);
        assert!(rn1.distance(rn2) < 1e-5);
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
    fn test_mesh_bounds_triangle() {
        let mesh = test_triangle_mesh();
        let (min, max) = mesh_bounds(&mesh);
        assert!((min.x - 0.0).abs() < 1e-6);
        assert!((max.x - 1.0).abs() < 1e-6);
        assert!((max.z - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_nearest_vertex_color() {
        let mesh = test_triangle_mesh();
        // Query near vertex 0 (at origin) should return red
        let color = nearest_vertex_color(&mesh, Vec3::new(0.01, 0.0, 0.0));
        assert!((color[0] - 1.0).abs() < 1e-6);
        // Query near vertex 1 should return green
        let color = nearest_vertex_color(&mesh, Vec3::new(0.99, 0.0, 0.0));
        assert!((color[1] - 1.0).abs() < 1e-6);
        // Query near vertex 2 should return blue
        let color = nearest_vertex_color(&mesh, Vec3::new(0.5, 0.0, 0.99));
        assert!((color[2] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_stamp_sphere_add() {
        let mut mesh = test_box_mesh();
        stamp_sphere(&mut mesh, Vec3::new(0.5, 0.0, 0.0), 0.3, 0.15, 16, true);
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
    fn test_stamp_sphere_empty_mesh_noop() {
        let mut mesh = Mesh::new();
        stamp_sphere(&mut mesh, Vec3::ZERO, 0.5, 0.15, 16, true);
        assert!(mesh.vertices.is_empty());
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
    fn test_handle_input_subtract_sphere() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::SubtractSphere);
        bridge.mc_resolution = 16;
        let mut mesh = test_box_mesh();
        bridge.handle_input(&mut mesh, Vec3::ZERO, true, false);
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

    #[test]
    fn test_point_triangle_dist_sq_at_vertex() {
        let v0 = Vec3::ZERO;
        let v1 = Vec3::X;
        let v2 = Vec3::Z;
        let p = Vec3::ZERO;
        let (dist_sq, closest) = point_triangle_dist_sq(p, v0, v1, v2);
        assert!(dist_sq < 1e-10);
        assert!(closest.distance(p) < 1e-5);
    }

    #[test]
    fn test_point_triangle_dist_sq_above() {
        let v0 = Vec3::ZERO;
        let v1 = Vec3::X;
        let v2 = Vec3::Z;
        let p = Vec3::new(0.25, 1.0, 0.25); // Above center of triangle
        let (dist_sq, _closest) = point_triangle_dist_sq(p, v0, v1, v2);
        // Distance should be ~1.0 (straight up from the triangle plane)
        assert!((dist_sq - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_approximate_mesh_sdf_distance() {
        let mesh = test_box_mesh();
        // A point far from the box should have a larger distance than a point near the surface
        let d_far = approximate_mesh_sdf(Vec3::new(5.0, 0.0, 0.0), &mesh).abs();
        let d_near = approximate_mesh_sdf(Vec3::new(0.6, 0.0, 0.0), &mesh).abs();
        assert!(
            d_far > d_near,
            "Far point should have larger |SDF| than near point"
        );
    }

    #[test]
    fn test_find_nearest_face() {
        let mesh = test_triangle_mesh();
        let sel = find_nearest_face(&mesh, Vec3::new(0.5, 0.0, 0.33));
        assert!(sel.is_some());
        let sel = sel.unwrap();
        assert_eq!(sel.tri_index, 0);
        assert!(sel.normal.length() > 0.99);
    }

    #[test]
    fn test_find_nearest_face_empty() {
        let mesh = Mesh::new();
        let sel = find_nearest_face(&mesh, Vec3::ZERO);
        assert!(sel.is_none());
    }

    #[test]
    fn test_find_nearest_vertex() {
        let mesh = test_triangle_mesh();
        let sel = find_nearest_vertex(&mesh, Vec3::new(0.01, 0.0, 0.0), 0.5);
        assert!(sel.is_some());
        assert_eq!(sel.unwrap().vertex_index, 0);
    }

    #[test]
    fn test_find_nearest_vertex_out_of_range() {
        let mesh = test_triangle_mesh();
        let sel = find_nearest_vertex(&mesh, Vec3::new(100.0, 0.0, 0.0), 0.5);
        assert!(sel.is_none());
    }

    #[test]
    fn test_find_nearest_edge() {
        let mesh = test_triangle_mesh();
        // Point near the midpoint of edge 0->1 (at [0.5, 0, 0])
        let sel = find_nearest_edge(&mesh, Vec3::new(0.5, 0.0, 0.0), 1.0);
        assert!(sel.is_some());
        let sel = sel.unwrap();
        // Edge 0->1 has midpoint at [0.5, 0, 0]
        assert!((sel.v0 == 0 && sel.v1 == 1) || (sel.v0 == 1 && sel.v1 == 0));
    }

    #[test]
    fn test_find_nearest_edge_out_of_range() {
        let mesh = test_triangle_mesh();
        let sel = find_nearest_edge(&mesh, Vec3::new(100.0, 0.0, 0.0), 0.5);
        assert!(sel.is_none());
    }

    #[test]
    fn test_handle_face_extrude_press_and_drag() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::FaceExtrude);
        let mut mesh = test_triangle_mesh();

        // Press to select face
        bridge.handle_input(&mut mesh, Vec3::new(0.5, 0.0, 0.33), true, false);
        assert!(matches!(bridge.drag_state, MeshDragState::FaceDrag { .. }));

        // Drag to extrude
        bridge.handle_input(&mut mesh, Vec3::new(0.5, 1.0, 0.33), false, true);
        assert!(bridge.is_dirty());
    }

    #[test]
    fn test_handle_face_extrude_release() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::FaceExtrude);
        let mut mesh = test_triangle_mesh();

        // Press to select
        bridge.handle_input(&mut mesh, Vec3::new(0.5, 0.0, 0.33), true, false);
        // Release (not pressing, not dragging)
        bridge.handle_input(&mut mesh, Vec3::new(0.5, 0.0, 0.33), false, false);
        assert!(matches!(bridge.drag_state, MeshDragState::Idle));
    }

    #[test]
    fn test_handle_vertex_pull_press_and_drag() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::VertexPull);
        let mut mesh = test_triangle_mesh();

        // Press to select vertex 0 (at origin)
        bridge.handle_input(&mut mesh, Vec3::new(0.01, 0.0, 0.0), true, false);
        assert!(matches!(
            bridge.drag_state,
            MeshDragState::VertexDrag { .. }
        ));

        // Drag the vertex
        bridge.handle_input(&mut mesh, Vec3::new(0.01, 0.5, 0.0), false, true);
        assert!(bridge.is_dirty());
        // Vertex 0 should have moved
        let new_y = mesh.vertices[0].position[1];
        assert!(new_y.abs() > 0.1, "Vertex should have moved");
    }

    #[test]
    fn test_handle_edge_pull_press_and_drag() {
        let mut bridge = SculptBridge::new();
        bridge.set_tool(SculptTool::EdgePull);
        let mut mesh = test_triangle_mesh();

        // Press to select edge near midpoint of edge 0-1
        bridge.handle_input(&mut mesh, Vec3::new(0.5, 0.0, 0.0), true, false);
        assert!(matches!(bridge.drag_state, MeshDragState::EdgeDrag { .. }));

        // Drag the edge
        bridge.handle_input(&mut mesh, Vec3::new(0.5, 0.5, 0.0), false, true);
        assert!(bridge.is_dirty());
    }

    #[test]
    fn test_sculpting_manager_accessor() {
        let bridge = SculptBridge::new();
        assert!(bridge.sculpting_manager().is_enabled());
    }

    #[test]
    fn test_sculpting_manager_mut_accessor() {
        let mut bridge = SculptBridge::new();
        bridge.sculpting_manager_mut().set_material(5);
        // Just ensure it doesn't panic
    }

    #[test]
    fn test_invalidate_cache() {
        let mut bridge = SculptBridge::new();
        bridge.mc_resolution = 8; // Small for fast test
        let mesh = test_box_mesh();

        // Build cache
        bridge.ensure_sdf_cache(&mesh, Vec3::splat(-1.0), Vec3::splat(1.0));
        assert!(bridge.mesh_sdf_cache.is_some());

        // Invalidate
        bridge.invalidate_cache();
        assert!(bridge.mesh_sdf_cache.is_none());
    }

    #[test]
    fn test_sdf_cache_sample() {
        let mut bridge = SculptBridge::new();
        bridge.mc_resolution = 8;
        let mesh = test_box_mesh();

        bridge.ensure_sdf_cache(&mesh, Vec3::splat(-2.0), Vec3::splat(2.0));
        let cache = bridge.mesh_sdf_cache.as_ref().unwrap();

        // A point far from the mesh surface should have a larger |SDF| than
        // a point near the surface
        let d_far = cache.sample(Vec3::new(1.5, 0.0, 0.0)).abs();
        let d_near = cache.sample(Vec3::new(0.55, 0.0, 0.0)).abs();
        assert!(
            d_far > d_near,
            "Far point should have larger |SDF| than near-surface point"
        );
    }

    #[test]
    fn test_sdf_cache_reuse() {
        let mut bridge = SculptBridge::new();
        bridge.mc_resolution = 8;
        let mesh = test_box_mesh();

        // Build cache with bounds
        bridge.ensure_sdf_cache(&mesh, Vec3::splat(-1.0), Vec3::splat(1.0));
        assert!(bridge.mesh_sdf_cache.is_some());

        // Call again with same or smaller bounds — should reuse
        bridge.ensure_sdf_cache(&mesh, Vec3::splat(-0.5), Vec3::splat(0.5));
        assert!(bridge.mesh_sdf_cache.is_some()); // Still has cache (not rebuilt)
    }
}
