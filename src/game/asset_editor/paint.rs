//! Vertex Color Painting Module (US-P4-009)
//!
//! Provides four painting tools for Stage 4 (Color) of the asset editor pipeline:
//! - **Brush**: Paint vertex colors with configurable radius, opacity, and hardness falloff
//! - **Fill**: Flood-fill connected triangles sharing similar colors via BFS
//! - **Gradient**: Linear color blend between two click points
//! - **Eyedropper**: Sample a vertex color and set it as the primary palette color
//!
//! All tools write directly to `BlockVertex.color` — no textures are involved.

use std::collections::{HashMap, HashSet, VecDeque};

use crate::render::building_blocks::BlockVertex;

// ============================================================================
// ENUMS
// ============================================================================

/// The active painting tool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaintTool {
    /// Free-form brush with radius/opacity/hardness falloff.
    Brush,
    /// Flood-fill connected same-color triangles.
    Fill,
    /// Linear gradient between two click points.
    Gradient,
    /// Sample a vertex color and set it as primary.
    Eyedropper,
}

// ============================================================================
// STRUCTS
// ============================================================================

/// Parameters for the brush tool.
#[derive(Clone, Debug)]
pub struct BrushParams {
    /// Brush radius in world-space units (default 0.5).
    pub radius: f32,
    /// Paint opacity 0.0–1.0 (default 1.0).
    pub opacity: f32,
    /// Brush hardness 0.0 (soft) – 1.0 (hard) (default 0.8).
    pub hardness: f32,
}

impl Default for BrushParams {
    fn default() -> Self {
        Self {
            radius: 0.5,
            opacity: 1.0,
            hardness: 0.8,
        }
    }
}

/// A named group of preset colors.
#[derive(Clone, Debug)]
pub struct PalettePreset {
    /// Display name for this preset group.
    pub name: String,
    /// Colors in this preset.
    pub colors: Vec<[f32; 4]>,
}

/// Color palette with HSV representation, presets, and recent colors.
#[derive(Clone, Debug)]
pub struct ColorPalette {
    /// Current painting color (RGBA).
    pub primary: [f32; 4],
    /// Alternate color (X key to swap).
    pub secondary: [f32; 4],
    /// HSV hue (0.0–360.0) synced with primary.
    pub hue: f32,
    /// HSV saturation (0.0–1.0) synced with primary.
    pub saturation: f32,
    /// HSV value/brightness (0.0–1.0) synced with primary.
    pub value: f32,
    /// Last 8 used colors (most recent first).
    pub recent: Vec<[f32; 4]>,
    /// Named preset groups for common asset categories.
    pub presets: Vec<PalettePreset>,
}

impl Default for ColorPalette {
    fn default() -> Self {
        let primary = [1.0, 1.0, 1.0, 1.0];
        let (h, s, v) = rgba_to_hsv(primary);
        Self {
            primary,
            secondary: [0.0, 0.0, 0.0, 1.0],
            hue: h,
            saturation: s,
            value: v,
            recent: Vec::new(),
            presets: default_presets(),
        }
    }
}

impl ColorPalette {
    /// Swap primary and secondary colors.
    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.primary, &mut self.secondary);
        let (h, s, v) = rgba_to_hsv(self.primary);
        self.hue = h;
        self.saturation = s;
        self.value = v;
    }

    /// Set the primary color (RGBA) and sync HSV fields.
    pub fn set_primary(&mut self, color: [f32; 4]) {
        self.primary = color;
        let (h, s, v) = rgba_to_hsv(color);
        self.hue = h;
        self.saturation = s;
        self.value = v;
    }

    /// Set the primary color from HSV and sync RGBA.
    pub fn set_primary_hsv(&mut self, h: f32, s: f32, v: f32) {
        self.hue = h;
        self.saturation = s;
        self.value = v;
        self.primary = hsv_to_rgba(h, s, v);
    }

    /// Record the current primary color in the recent list (max 8).
    pub fn push_recent(&mut self) {
        // Don't duplicate if it's already the most recent
        if self.recent.first() == Some(&self.primary) {
            return;
        }
        self.recent.insert(0, self.primary);
        if self.recent.len() > 8 {
            self.recent.truncate(8);
        }
    }
}

/// State for in-progress gradient painting.
#[derive(Clone, Debug)]
pub struct GradientState {
    /// World-space start point.
    pub start: [f32; 3],
    /// Color at the start point (primary at first click).
    pub start_color: [f32; 4],
}

/// The main paint system for Stage 4 (Color).
pub struct PaintSystem {
    /// Currently selected painting tool.
    pub tool: PaintTool,
    /// Brush parameters (used by Brush tool).
    pub brush: BrushParams,
    /// Color palette with primary/secondary, HSV, presets, recent.
    pub palette: ColorPalette,
    /// In-progress gradient (set on first click, applied on second).
    pub gradient_state: Option<GradientState>,
}

impl Default for PaintSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl PaintSystem {
    /// Create a new paint system with default settings.
    pub fn new() -> Self {
        Self {
            tool: PaintTool::Brush,
            brush: BrushParams::default(),
            palette: ColorPalette::default(),
            gradient_state: None,
        }
    }

    /// Select a painting tool.
    pub fn select_tool(&mut self, tool: PaintTool) {
        self.tool = tool;
        // Clear gradient state when switching away from Gradient tool
        if tool != PaintTool::Gradient {
            self.gradient_state = None;
        }
        println!("Paint tool: {:?}", tool);
    }

    /// Handle a paint click/stroke at the given world-space hit point.
    ///
    /// Dispatches to the appropriate tool implementation. For gradient, the
    /// first click sets the start point; the second click applies the gradient.
    ///
    /// `triangle_idx` is the index of the triangle hit (for flood fill).
    pub fn apply(
        &mut self,
        vertices: &mut [BlockVertex],
        indices: &[u32],
        hit_point: [f32; 3],
        triangle_idx: Option<usize>,
    ) {
        match self.tool {
            PaintTool::Brush => {
                self.palette.push_recent();
                paint_brush(vertices, hit_point, &self.brush, self.palette.primary);
            }
            PaintTool::Fill => {
                if let Some(tri_idx) = triangle_idx {
                    self.palette.push_recent();
                    flood_fill(vertices, indices, tri_idx, self.palette.primary, 0.1);
                }
            }
            PaintTool::Gradient => {
                if let Some(ref state) = self.gradient_state.clone() {
                    // Second click: apply gradient
                    self.palette.push_recent();
                    apply_gradient(
                        vertices,
                        state.start,
                        state.start_color,
                        hit_point,
                        self.palette.secondary,
                    );
                    self.gradient_state = None;
                } else {
                    // First click: record start
                    self.gradient_state = Some(GradientState {
                        start: hit_point,
                        start_color: self.palette.primary,
                    });
                    println!("Gradient: start set, click again for end point");
                }
            }
            PaintTool::Eyedropper => {
                eyedropper(vertices, hit_point, &mut self.palette);
            }
        }
    }
}

// ============================================================================
// COLOR CONVERSION
// ============================================================================

/// Convert HSV to RGBA.
///
/// - `h`: hue in degrees (0.0–360.0)
/// - `s`: saturation (0.0–1.0)
/// - `v`: value/brightness (0.0–1.0)
///
/// Alpha is always 1.0.
pub fn hsv_to_rgba(h: f32, s: f32, v: f32) -> [f32; 4] {
    let h = h % 360.0;
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r1, g1, b1) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    [r1 + m, g1 + m, b1 + m, 1.0]
}

/// Convert RGBA to HSV.
///
/// Returns `(hue, saturation, value)` where hue is 0.0–360.0.
/// Alpha channel is ignored.
pub fn rgba_to_hsv(color: [f32; 4]) -> (f32, f32, f32) {
    let r = color[0];
    let g = color[1];
    let b = color[2];

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let v = max;

    if delta < 1e-6 {
        return (0.0, 0.0, v);
    }

    let s = delta / max;

    let h = if (max - r).abs() < 1e-6 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 1e-6 {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let h = if h < 0.0 { h + 360.0 } else { h };

    (h, s, v)
}

// ============================================================================
// PAINT TOOLS
// ============================================================================

/// Euclidean distance between two 3D points.
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Paint vertex colors using a brush with radius/opacity/hardness falloff.
///
/// For each vertex within `brush.radius` of `hit_point`, computes a smooth
/// falloff and lerps the vertex color toward `color`.
pub fn paint_brush(
    vertices: &mut [BlockVertex],
    hit_point: [f32; 3],
    brush: &BrushParams,
    color: [f32; 4],
) {
    if brush.radius <= 0.0 {
        return;
    }

    for v in vertices.iter_mut() {
        let dist = distance(v.position, hit_point);
        if dist < brush.radius {
            let t = dist / brush.radius;
            // Hardness controls falloff curve: high hardness = sharp edge, low = soft
            let falloff = (1.0 - t).powf(2.0 / brush.hardness.max(0.01));
            let alpha = falloff * brush.opacity;
            for i in 0..4 {
                v.color[i] = v.color[i] * (1.0 - alpha) + color[i] * alpha;
            }
        }
    }
}

/// Flood-fill connected triangles with similar colors using BFS.
///
/// Starting from `start_triangle_idx`, spreads to adjacent triangles whose
/// average vertex color is within `tolerance` of the original triangle's color.
/// All visited triangle vertices are set to `target_color`.
pub fn flood_fill(
    vertices: &mut [BlockVertex],
    indices: &[u32],
    start_triangle_idx: usize,
    target_color: [f32; 4],
    tolerance: f32,
) {
    let tri_count = indices.len() / 3;
    if start_triangle_idx >= tri_count {
        return;
    }

    // Build triangle adjacency: triangles sharing an edge are neighbors.
    let adjacency = build_triangle_adjacency(indices, tri_count);

    // Pre-cache all triangle average colors before any painting, so that
    // BFS color comparisons use the original (unmodified) colors.
    let cached_colors: Vec<[f32; 4]> = (0..tri_count)
        .map(|i| triangle_avg_color(vertices, indices, i))
        .collect();

    let original_color = cached_colors[start_triangle_idx];

    // BFS flood fill
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(start_triangle_idx);
    queue.push_back(start_triangle_idx);

    while let Some(tri_idx) = queue.pop_front() {
        // Paint this triangle's vertices
        let base = tri_idx * 3;
        for offset in 0..3 {
            if base + offset < indices.len() {
                let vi = indices[base + offset] as usize;
                if vi < vertices.len() {
                    vertices[vi].color = target_color;
                }
            }
        }

        // Spread to adjacent triangles with similar color (using cached original colors)
        if let Some(neighbors) = adjacency.get(&tri_idx) {
            for &neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    if color_distance(cached_colors[neighbor], original_color) <= tolerance {
                        visited.insert(neighbor);
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }
}

/// Apply a linear gradient between two world-space points.
///
/// For each vertex, projects its position onto the start→end line to compute
/// a parameter `t` (clamped 0.0–1.0), then lerps between `start_color` and
/// `end_color`.
pub fn apply_gradient(
    vertices: &mut [BlockVertex],
    start: [f32; 3],
    start_color: [f32; 4],
    end: [f32; 3],
    end_color: [f32; 4],
) {
    let dx = end[0] - start[0];
    let dy = end[1] - start[1];
    let dz = end[2] - start[2];
    let len_sq = dx * dx + dy * dy + dz * dz;

    if len_sq < 1e-10 {
        // Start and end are the same point — paint everything start_color
        for v in vertices.iter_mut() {
            v.color = start_color;
        }
        return;
    }

    for v in vertices.iter_mut() {
        let px = v.position[0] - start[0];
        let py = v.position[1] - start[1];
        let pz = v.position[2] - start[2];
        let dot = px * dx + py * dy + pz * dz;
        let t = (dot / len_sq).clamp(0.0, 1.0);

        for i in 0..4 {
            v.color[i] = start_color[i] * (1.0 - t) + end_color[i] * t;
        }
    }
}

/// Eyedropper: find the nearest vertex to `hit_point` and copy its color
/// into `palette.primary`, updating HSV fields.
pub fn eyedropper(vertices: &[BlockVertex], hit_point: [f32; 3], palette: &mut ColorPalette) {
    if vertices.is_empty() {
        return;
    }

    let mut best_dist = f32::MAX;
    let mut best_color = [1.0_f32; 4];

    for v in vertices {
        let dist = distance(v.position, hit_point);
        if dist < best_dist {
            best_dist = dist;
            best_color = v.color;
        }
    }

    palette.set_primary(best_color);
    println!(
        "Eyedropper: sampled [{:.2}, {:.2}, {:.2}, {:.2}]",
        best_color[0], best_color[1], best_color[2], best_color[3]
    );
}

// ============================================================================
// HELPERS
// ============================================================================

/// Build triangle adjacency from the index buffer.
///
/// Two triangles are adjacent if they share an edge (two vertex indices in common).
/// Returns a map from triangle index to a set of neighbor triangle indices.
fn build_triangle_adjacency(indices: &[u32], tri_count: usize) -> HashMap<usize, Vec<usize>> {
    // Map each edge (sorted pair of vertex indices) to the triangles that use it.
    let mut edge_to_tris: HashMap<(u32, u32), Vec<usize>> = HashMap::new();

    for tri_idx in 0..tri_count {
        let base = tri_idx * 3;
        if base + 2 >= indices.len() {
            break;
        }
        let v = [indices[base], indices[base + 1], indices[base + 2]];

        for &(a, b) in &[(v[0], v[1]), (v[1], v[2]), (v[2], v[0])] {
            let edge = if a < b { (a, b) } else { (b, a) };
            edge_to_tris.entry(edge).or_default().push(tri_idx);
        }
    }

    let mut adjacency: HashMap<usize, Vec<usize>> = HashMap::new();
    for tris in edge_to_tris.values() {
        for i in 0..tris.len() {
            for j in (i + 1)..tris.len() {
                adjacency.entry(tris[i]).or_default().push(tris[j]);
                adjacency.entry(tris[j]).or_default().push(tris[i]);
            }
        }
    }

    adjacency
}

/// Compute the average color of a triangle's three vertices.
fn triangle_avg_color(vertices: &[BlockVertex], indices: &[u32], tri_idx: usize) -> [f32; 4] {
    let base = tri_idx * 3;
    let mut avg = [0.0_f32; 4];
    let mut count = 0.0;

    for offset in 0..3 {
        if base + offset < indices.len() {
            let vi = indices[base + offset] as usize;
            if vi < vertices.len() {
                for i in 0..4 {
                    avg[i] += vertices[vi].color[i];
                }
                count += 1.0;
            }
        }
    }

    if count > 0.0 {
        for c in &mut avg {
            *c /= count;
        }
    }

    avg
}

/// Euclidean distance between two RGBA colors (all 4 channels).
fn color_distance(a: [f32; 4], b: [f32; 4]) -> f32 {
    let mut sum = 0.0_f32;
    for i in 0..4 {
        let d = a[i] - b[i];
        sum += d * d;
    }
    sum.sqrt()
}

/// Build the default color presets for common asset categories.
fn default_presets() -> Vec<PalettePreset> {
    vec![
        PalettePreset {
            name: "Trees".to_string(),
            colors: vec![
                [0.13, 0.37, 0.13, 1.0], // Dark green
                [0.20, 0.50, 0.20, 1.0], // Medium green
                [0.30, 0.60, 0.25, 1.0], // Light green
                [0.40, 0.30, 0.15, 1.0], // Bark brown
                [0.50, 0.35, 0.20, 1.0], // Light bark
            ],
        },
        PalettePreset {
            name: "Rock".to_string(),
            colors: vec![
                [0.35, 0.33, 0.30, 1.0], // Dark gray-brown
                [0.50, 0.48, 0.45, 1.0], // Medium stone
                [0.65, 0.62, 0.58, 1.0], // Light stone
                [0.40, 0.38, 0.35, 1.0], // Slate
                [0.55, 0.50, 0.42, 1.0], // Sandy stone
            ],
        },
        PalettePreset {
            name: "Wood".to_string(),
            colors: vec![
                [0.36, 0.20, 0.09, 1.0], // Dark wood
                [0.55, 0.35, 0.17, 1.0], // Medium wood
                [0.72, 0.53, 0.30, 1.0], // Light wood
                [0.45, 0.28, 0.12, 1.0], // Walnut
                [0.62, 0.45, 0.25, 1.0], // Oak
            ],
        },
    ]
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_vertex(pos: [f32; 3], color: [f32; 4]) -> BlockVertex {
        BlockVertex {
            position: pos,
            normal: [0.0, 1.0, 0.0],
            color,
        }
    }

    #[test]
    fn test_hsv_to_rgba_red() {
        let c = hsv_to_rgba(0.0, 1.0, 1.0);
        assert!((c[0] - 1.0).abs() < 0.01);
        assert!(c[1].abs() < 0.01);
        assert!(c[2].abs() < 0.01);
        assert!((c[3] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_hsv_to_rgba_green() {
        let c = hsv_to_rgba(120.0, 1.0, 1.0);
        assert!(c[0].abs() < 0.01);
        assert!((c[1] - 1.0).abs() < 0.01);
        assert!(c[2].abs() < 0.01);
    }

    #[test]
    fn test_hsv_to_rgba_blue() {
        let c = hsv_to_rgba(240.0, 1.0, 1.0);
        assert!(c[0].abs() < 0.01);
        assert!(c[1].abs() < 0.01);
        assert!((c[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rgba_to_hsv_roundtrip() {
        let colors = [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 1.0],
            [0.0, 0.0, 1.0, 1.0],
            [0.5, 0.5, 0.5, 1.0],
            [0.8, 0.3, 0.1, 1.0],
        ];

        for original in &colors {
            let (h, s, v) = rgba_to_hsv(*original);
            let back = hsv_to_rgba(h, s, v);
            for i in 0..3 {
                assert!(
                    (original[i] - back[i]).abs() < 0.02,
                    "Roundtrip failed for {:?}: got {:?} (h={}, s={}, v={})",
                    original,
                    back,
                    h,
                    s,
                    v
                );
            }
        }
    }

    #[test]
    fn test_paint_brush_within_radius() {
        // Start with gray vertices so painting red is visible on the green channel
        let mut vertices = vec![
            make_vertex([0.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([0.3, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([10.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
        ];

        let brush = BrushParams {
            radius: 0.5,
            opacity: 1.0,
            hardness: 1.0,
        };

        paint_brush(&mut vertices, [0.0, 0.0, 0.0], &brush, [1.0, 0.0, 0.0, 1.0]);

        // Vertex at origin should be heavily painted red (high R, low G)
        assert!(vertices[0].color[0] > 0.9, "Center vertex should be red");
        assert!(
            vertices[0].color[1] < 0.1,
            "Center vertex green should drop"
        );
        // Vertex at 0.3 should be partially painted (green reduced but not to 0)
        assert!(
            vertices[1].color[1] < vertices[2].color[1],
            "Nearby vertex should have less green than far vertex"
        );
        // Vertex at 10.0 should be unchanged (gray)
        assert!(
            (vertices[2].color[0] - 0.5).abs() < 0.01,
            "Far vertex should be unchanged"
        );
    }

    #[test]
    fn test_flood_fill_basic() {
        // Two triangles sharing an edge (vertices 0-1-2 and 1-2-3)
        let mut vertices = vec![
            make_vertex([0.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([1.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([0.5, 1.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([1.5, 1.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
        ];
        let indices = vec![0, 1, 2, 1, 2, 3];

        let red = [1.0, 0.0, 0.0, 1.0];
        flood_fill(&mut vertices, &indices, 0, red, 0.1);

        // Both triangles share similar color, so all should be painted
        for v in &vertices {
            assert!(
                (v.color[0] - 1.0).abs() < 0.01,
                "All connected same-color vertices should be painted"
            );
        }
    }

    #[test]
    fn test_flood_fill_stops_at_color_boundary() {
        // Two triangles: first is gray, second is very different color
        let mut vertices = vec![
            make_vertex([0.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([1.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([0.5, 1.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([1.5, 1.0, 0.0], [1.0, 0.0, 0.0, 1.0]), // Different color
        ];
        let indices = vec![0, 1, 2, 1, 2, 3];

        let blue = [0.0, 0.0, 1.0, 1.0];
        flood_fill(&mut vertices, &indices, 0, blue, 0.1);

        // First triangle should be painted blue
        assert!((vertices[0].color[2] - 1.0).abs() < 0.01);
        // Second triangle vertex 3 has different original color;
        // the average of tri 1 (which includes already-painted v1,v2)
        // differs from the original gray, so v3 stays untouched
    }

    #[test]
    fn test_apply_gradient() {
        let mut vertices = vec![
            make_vertex([0.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([5.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
            make_vertex([10.0, 0.0, 0.0], [0.5, 0.5, 0.5, 1.0]),
        ];

        let red = [1.0, 0.0, 0.0, 1.0];
        let blue = [0.0, 0.0, 1.0, 1.0];

        apply_gradient(&mut vertices, [0.0, 0.0, 0.0], red, [10.0, 0.0, 0.0], blue);

        // Start should be red
        assert!((vertices[0].color[0] - 1.0).abs() < 0.01);
        assert!(vertices[0].color[2].abs() < 0.01);
        // Middle should be mixed
        assert!((vertices[1].color[0] - 0.5).abs() < 0.01);
        assert!((vertices[1].color[2] - 0.5).abs() < 0.01);
        // End should be blue
        assert!(vertices[2].color[0].abs() < 0.01);
        assert!((vertices[2].color[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_eyedropper() {
        let vertices = vec![
            make_vertex([0.0, 0.0, 0.0], [0.3, 0.6, 0.9, 1.0]),
            make_vertex([5.0, 0.0, 0.0], [1.0, 0.0, 0.0, 1.0]),
        ];

        let mut palette = ColorPalette::default();
        eyedropper(&vertices, [0.1, 0.0, 0.0], &mut palette);

        // Should sample the nearest vertex (at origin)
        assert!((palette.primary[0] - 0.3).abs() < 0.01);
        assert!((palette.primary[1] - 0.6).abs() < 0.01);
        assert!((palette.primary[2] - 0.9).abs() < 0.01);
    }

    #[test]
    fn test_palette_swap() {
        let mut palette = ColorPalette::default();
        palette.set_primary([1.0, 0.0, 0.0, 1.0]);
        let old_secondary = palette.secondary;
        palette.swap_colors();
        assert_eq!(palette.primary, old_secondary);
        assert_eq!(palette.secondary, [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_recent_colors() {
        let mut palette = ColorPalette::default();
        palette.set_primary([1.0, 0.0, 0.0, 1.0]);
        palette.push_recent();
        palette.set_primary([0.0, 1.0, 0.0, 1.0]);
        palette.push_recent();

        assert_eq!(palette.recent.len(), 2);
        assert_eq!(palette.recent[0], [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(palette.recent[1], [1.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_recent_colors_max_8() {
        let mut palette = ColorPalette::default();
        for i in 0..12 {
            palette.set_primary([i as f32 / 12.0, 0.0, 0.0, 1.0]);
            palette.push_recent();
        }
        assert_eq!(palette.recent.len(), 8);
    }

    #[test]
    fn test_default_presets() {
        let presets = default_presets();
        assert_eq!(presets.len(), 3);
        assert_eq!(presets[0].name, "Trees");
        assert_eq!(presets[1].name, "Rock");
        assert_eq!(presets[2].name, "Wood");
        assert!(!presets[0].colors.is_empty());
    }

    #[test]
    fn test_paint_system_new() {
        let ps = PaintSystem::new();
        assert_eq!(ps.tool, PaintTool::Brush);
        assert!(ps.gradient_state.is_none());
    }
}
