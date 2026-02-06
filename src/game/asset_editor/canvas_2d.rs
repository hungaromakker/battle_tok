//! 2D Canvas Drawing Module
//!
//! Provides the drawing canvas for Stage 1 (Draw2D) of the Asset Editor.
//! Users sketch 2D outlines/silhouettes that will later be extruded into 3D meshes.
//!
//! Features:
//! - Freehand drawing tool (D key) with RDP simplification on mouse-up
//! - Line tool (L key) for straight edge segments
//! - Orthographic canvas with grid, zoom, and pan
//! - Undo support (Ctrl+Z removes last outline)

use crate::game::types::Vertex;
use crate::game::ui::add_quad;

// ============================================================================
// ENUMS
// ============================================================================

/// The active drawing tool on the 2D canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawTool {
    /// Freehand drawing: click-drag to draw smooth strokes.
    Freehand,
    /// Line tool: click-click to create straight segments.
    Line,
}

impl std::fmt::Display for DrawTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrawTool::Freehand => write!(f, "Freehand"),
            DrawTool::Line => write!(f, "Line"),
        }
    }
}

// ============================================================================
// OUTLINE
// ============================================================================

/// A 2D outline consisting of connected points.
/// This is the primary output of the Draw2D stage and the input
/// for the Extrude stage.
#[derive(Debug, Clone)]
pub struct Outline2D {
    /// Points in canvas (world) coordinates.
    pub points: Vec<[f32; 2]>,
    /// Whether this outline forms a closed loop.
    pub closed: bool,
}

impl Outline2D {
    /// Create a new empty outline.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            closed: false,
        }
    }

    /// Create an outline with a single starting point.
    pub fn with_start(point: [f32; 2]) -> Self {
        Self {
            points: vec![point],
            closed: false,
        }
    }

    /// Return the number of points in this outline.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Return whether this outline has no points.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Return the last point, if any.
    pub fn last_point(&self) -> Option<[f32; 2]> {
        self.points.last().copied()
    }
}

impl Default for Outline2D {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// CANVAS 2D
// ============================================================================

/// The 2D drawing canvas used in the Draw2D stage.
///
/// Manages outlines, the active drawing tool, and canvas navigation
/// (zoom, pan, grid). Provides methods for handling input events and
/// rendering the canvas contents as vertex/index data.
pub struct Canvas2D {
    /// Completed outlines.
    pub outlines: Vec<Outline2D>,
    /// The outline currently being drawn (in-progress).
    pub active_outline: Option<Outline2D>,
    /// The currently selected drawing tool.
    pub tool: DrawTool,
    /// Zoom level. 1.0 = default (20x20 world units visible).
    /// Range: 0.1 to 10.0.
    pub zoom: f32,
    /// Camera pan offset in canvas (world) coordinates.
    pub pan: [f32; 2],
    /// Whether the background grid is visible.
    pub show_grid: bool,
    /// Grid cell size in canvas units.
    pub grid_size: f32,
    /// Whether snap-to-grid is enabled for drawing points.
    pub snap_to_grid: bool,

    // -- Internal state --
    /// Whether we are currently in a freehand drawing stroke.
    drawing: bool,
    /// Starting point for the line tool (waiting for second click).
    line_start: Option<[f32; 2]>,
    /// Current mouse position in canvas coordinates (for preview rendering).
    current_mouse_canvas: Option<[f32; 2]>,
    /// Whether the middle mouse button is held (for panning).
    middle_mouse_held: bool,
    /// Last mouse screen position for computing pan deltas.
    last_mouse_screen: Option<[f32; 2]>,
    /// Viewport dimensions (width, height) in pixels. Updated each frame.
    viewport_size: [f32; 2],
}

impl Default for Canvas2D {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas2D {
    // -- Constants --

    /// Default visible range: 20x20 world units (+-10 on each axis).
    const DEFAULT_HALF_EXTENT: f32 = 10.0;
    /// Minimum zoom level.
    const MIN_ZOOM: f32 = 0.1;
    /// Maximum zoom level.
    const MAX_ZOOM: f32 = 10.0;
    /// Zoom factor per scroll tick.
    const ZOOM_FACTOR: f32 = 1.1;
    /// RDP simplification epsilon for freehand strokes.
    const RDP_EPSILON: f32 = 0.05;
    /// Half-width of rendered outline segments in canvas units.
    const LINE_HALF_WIDTH: f32 = 0.02;
    /// Grid line half-width in canvas units.
    const GRID_LINE_HALF_WIDTH: f32 = 0.008;
    /// Color for completed outlines.
    const OUTLINE_COLOR: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
    /// Color for the active (in-progress) outline.
    const ACTIVE_COLOR: [f32; 4] = [0.5, 0.8, 1.0, 1.0];
    /// Color for grid lines.
    const GRID_COLOR: [f32; 4] = [0.3, 0.3, 0.3, 0.5];
    /// Color for the origin axes.
    const AXIS_COLOR: [f32; 4] = [0.5, 0.5, 0.5, 0.7];
    /// Color for line tool preview.
    const PREVIEW_COLOR: [f32; 4] = [0.5, 0.8, 1.0, 0.6];

    /// Create a new canvas with default settings.
    pub fn new() -> Self {
        Self {
            outlines: Vec::new(),
            active_outline: None,
            tool: DrawTool::Freehand,
            zoom: 1.0,
            pan: [0.0, 0.0],
            show_grid: true,
            grid_size: 1.0,
            snap_to_grid: false,
            drawing: false,
            line_start: None,
            current_mouse_canvas: None,
            middle_mouse_held: false,
            last_mouse_screen: None,
            viewport_size: [1280.0, 800.0],
        }
    }

    // ========================================================================
    // COORDINATE CONVERSION
    // ========================================================================

    /// Convert screen pixel coordinates to canvas (world) coordinates.
    ///
    /// The orthographic projection places the origin at the center of the
    /// viewport. With zoom=1.0, the visible area spans 20x20 world units
    /// (from -10 to +10 on each axis). Zoom multiplies the visible extent.
    pub fn screen_to_canvas(&self, screen_x: f32, screen_y: f32) -> [f32; 2] {
        let vw = self.viewport_size[0];
        let vh = self.viewport_size[1];

        // Normalized device coords: -1..+1
        let ndc_x = (screen_x / vw) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_y / vh) * 2.0; // Y flipped

        // Half-extent in world units, adjusted for zoom and aspect ratio
        let half_x = Self::DEFAULT_HALF_EXTENT / self.zoom;
        let half_y = Self::DEFAULT_HALF_EXTENT / self.zoom;

        // Aspect ratio correction: wider windows show more horizontally
        let aspect = vw / vh;
        let world_x = ndc_x * half_x * aspect + self.pan[0];
        let world_y = ndc_y * half_y + self.pan[1];

        [world_x, world_y]
    }

    /// Convert canvas (world) coordinates to NDC (-1..+1) for rendering.
    fn canvas_to_ndc(&self, cx: f32, cy: f32) -> [f32; 3] {
        let vw = self.viewport_size[0];
        let vh = self.viewport_size[1];
        let aspect = vw / vh;

        let half_x = Self::DEFAULT_HALF_EXTENT / self.zoom;
        let half_y = Self::DEFAULT_HALF_EXTENT / self.zoom;

        let ndc_x = (cx - self.pan[0]) / (half_x * aspect);
        let ndc_y = (cy - self.pan[1]) / half_y;

        [ndc_x, ndc_y, 0.0]
    }

    /// Snap a canvas point to the nearest grid intersection if snap is enabled.
    fn maybe_snap(&self, point: [f32; 2]) -> [f32; 2] {
        if self.snap_to_grid && self.grid_size > 0.0 {
            [
                (point[0] / self.grid_size).round() * self.grid_size,
                (point[1] / self.grid_size).round() * self.grid_size,
            ]
        } else {
            point
        }
    }

    // ========================================================================
    // VIEWPORT
    // ========================================================================

    /// Update the viewport dimensions. Call this on window resize.
    pub fn set_viewport_size(&mut self, width: f32, height: f32) {
        if width > 0.0 && height > 0.0 {
            self.viewport_size = [width, height];
        }
    }

    // ========================================================================
    // TOOL SWITCHING
    // ========================================================================

    /// Switch to the freehand drawing tool.
    pub fn select_freehand(&mut self) {
        if self.tool != DrawTool::Freehand {
            // Cancel any in-progress line tool state
            self.line_start = None;
            self.tool = DrawTool::Freehand;
            println!("Canvas tool: Freehand");
        }
    }

    /// Switch to the line drawing tool.
    pub fn select_line(&mut self) {
        if self.tool != DrawTool::Line {
            // Cancel any in-progress freehand state
            self.finish_freehand_stroke();
            self.tool = DrawTool::Line;
            println!("Canvas tool: Line");
        }
    }

    /// Toggle grid visibility.
    pub fn toggle_grid(&mut self) {
        self.show_grid = !self.show_grid;
        println!("Grid: {}", if self.show_grid { "on" } else { "off" });
    }

    // ========================================================================
    // MOUSE INPUT HANDLERS
    // ========================================================================

    /// Handle left mouse button press at the given screen coordinates.
    pub fn on_left_press(&mut self, screen_x: f32, screen_y: f32) {
        let canvas_pos = self.screen_to_canvas(screen_x, screen_y);
        let canvas_pos = self.maybe_snap(canvas_pos);

        match self.tool {
            DrawTool::Freehand => {
                self.drawing = true;
                self.active_outline = Some(Outline2D::with_start(canvas_pos));
            }
            DrawTool::Line => {
                self.handle_line_click(canvas_pos);
            }
        }
    }

    /// Handle left mouse button release.
    pub fn on_left_release(&mut self) {
        if self.tool == DrawTool::Freehand && self.drawing {
            self.finish_freehand_stroke();
        }
    }

    /// Handle mouse movement at the given screen coordinates.
    pub fn on_mouse_move(&mut self, screen_x: f32, screen_y: f32) {
        let canvas_pos = self.screen_to_canvas(screen_x, screen_y);
        self.current_mouse_canvas = Some(canvas_pos);

        // Freehand: add points while drawing
        if self.tool == DrawTool::Freehand && self.drawing {
            let snapped = self.maybe_snap(canvas_pos);
            if let Some(ref mut outline) = self.active_outline {
                // Only add if moved a minimum distance to avoid duplicates
                if let Some(last) = outline.last_point() {
                    let dx = snapped[0] - last[0];
                    let dy = snapped[1] - last[1];
                    let dist_sq = dx * dx + dy * dy;
                    // Minimum distance threshold (in canvas units)
                    if dist_sq > 0.0001 {
                        outline.points.push(snapped);
                    }
                }
            }
        }

        // Middle-mouse pan
        if self.middle_mouse_held {
            if let Some(last) = self.last_mouse_screen {
                let dx = screen_x - last[0];
                let dy = screen_y - last[1];

                let vw = self.viewport_size[0];
                let vh = self.viewport_size[1];
                let aspect = vw / vh;
                let half_x = Self::DEFAULT_HALF_EXTENT / self.zoom;
                let half_y = Self::DEFAULT_HALF_EXTENT / self.zoom;

                // Convert pixel delta to world delta
                self.pan[0] -= dx / vw * 2.0 * half_x * aspect;
                self.pan[1] += dy / vh * 2.0 * half_y;
            }
            self.last_mouse_screen = Some([screen_x, screen_y]);
        }
    }

    /// Handle middle mouse button press (start pan).
    pub fn on_middle_press(&mut self, screen_x: f32, screen_y: f32) {
        self.middle_mouse_held = true;
        self.last_mouse_screen = Some([screen_x, screen_y]);
    }

    /// Handle middle mouse button release (stop pan).
    pub fn on_middle_release(&mut self) {
        self.middle_mouse_held = false;
        self.last_mouse_screen = None;
    }

    /// Handle right mouse button press (alternative pan).
    pub fn on_right_press(&mut self, screen_x: f32, screen_y: f32) {
        // Right-mouse also pans (as per status.json acceptance criteria)
        self.on_middle_press(screen_x, screen_y);
    }

    /// Handle right mouse button release.
    pub fn on_right_release(&mut self) {
        self.on_middle_release();
    }

    /// Handle scroll wheel for zooming.
    /// Positive `delta` zooms in, negative zooms out.
    pub fn on_scroll(&mut self, delta: f32) {
        if delta > 0.0 {
            self.zoom *= Self::ZOOM_FACTOR;
        } else if delta < 0.0 {
            self.zoom /= Self::ZOOM_FACTOR;
        }
        self.zoom = self.zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
    }

    // ========================================================================
    // UNDO
    // ========================================================================

    /// Undo the last completed outline.
    pub fn undo(&mut self) {
        if self.outlines.pop().is_some() {
            println!("Canvas: undo (outlines remaining: {})", self.outlines.len());
        }
    }

    // ========================================================================
    // INTERNAL DRAWING LOGIC
    // ========================================================================

    /// Finish an in-progress freehand stroke: apply RDP simplification
    /// and move the outline from active to completed.
    fn finish_freehand_stroke(&mut self) {
        self.drawing = false;
        if let Some(mut outline) = self.active_outline.take() {
            if outline.points.len() >= 2 {
                let simplified = rdp_simplify(&outline.points, Self::RDP_EPSILON);
                println!(
                    "RDP: {} points -> {} points",
                    outline.points.len(),
                    simplified.len()
                );
                outline.points = simplified;
                self.outlines.push(outline);
            }
            // Discard single-point outlines (just a click with no drag)
        }
    }

    /// Handle a click for the line tool.
    fn handle_line_click(&mut self, canvas_pos: [f32; 2]) {
        if let Some(start) = self.line_start.take() {
            // Second click: create a 2-point outline (or extend the last one)
            if let Some(last_outline) = self.outlines.last_mut() {
                // If the last outline's end point is near the start of this segment,
                // extend it instead of creating a new outline
                if let Some(last_pt) = last_outline.last_point() {
                    let dx = last_pt[0] - start[0];
                    let dy = last_pt[1] - start[1];
                    if dx * dx + dy * dy < 0.01 {
                        last_outline.points.push(canvas_pos);
                        println!(
                            "Line tool: extended outline ({} points)",
                            last_outline.points.len()
                        );
                        return;
                    }
                }
            }
            // Create a new 2-point outline
            let outline = Outline2D {
                points: vec![start, canvas_pos],
                closed: false,
            };
            self.outlines.push(outline);
            println!("Line tool: new segment");
        } else {
            // First click: record start
            self.line_start = Some(canvas_pos);
            println!("Line tool: start point set");
        }
    }

    // ========================================================================
    // RENDERING
    // ========================================================================

    /// Generate vertex and index data for the entire canvas.
    ///
    /// Returns `(vertices, indices)` ready for GPU upload.
    /// All coordinates are in NDC (-1..+1) space.
    pub fn render(&self, vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) {
        // 1. Grid
        if self.show_grid {
            self.render_grid(vertices, indices);
        }

        // 2. Completed outlines
        for outline in &self.outlines {
            self.render_outline(outline, Self::OUTLINE_COLOR, Self::LINE_HALF_WIDTH, vertices, indices);
        }

        // 3. Active (in-progress) outline
        if let Some(ref active) = self.active_outline {
            self.render_outline(active, Self::ACTIVE_COLOR, Self::LINE_HALF_WIDTH, vertices, indices);
        }

        // 4. Line tool preview
        if self.tool == DrawTool::Line {
            if let (Some(start), Some(mouse)) = (self.line_start, self.current_mouse_canvas) {
                self.render_segment(
                    start,
                    mouse,
                    Self::PREVIEW_COLOR,
                    Self::LINE_HALF_WIDTH,
                    vertices,
                    indices,
                );
            }
        }
    }

    /// Render the background grid as thin quads.
    fn render_grid(&self, vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) {
        let vw = self.viewport_size[0];
        let vh = self.viewport_size[1];
        let aspect = vw / vh;

        let half_x = Self::DEFAULT_HALF_EXTENT / self.zoom * aspect;
        let half_y = Self::DEFAULT_HALF_EXTENT / self.zoom;

        // Visible range in world coordinates
        let min_x = self.pan[0] - half_x;
        let max_x = self.pan[0] + half_x;
        let min_y = self.pan[1] - half_y;
        let max_y = self.pan[1] + half_y;

        let gs = self.grid_size;
        if gs <= 0.0 {
            return;
        }

        // Grid line half-width in canvas units (scale with zoom so it stays visible)
        let hw = Self::GRID_LINE_HALF_WIDTH / self.zoom.sqrt();

        // Vertical lines
        let start_x = (min_x / gs).floor() as i32;
        let end_x = (max_x / gs).ceil() as i32;
        for i in start_x..=end_x {
            let x = i as f32 * gs;
            let color = if i == 0 {
                Self::AXIS_COLOR
            } else {
                Self::GRID_COLOR
            };
            let width = if i == 0 { hw * 2.0 } else { hw };

            let tl = self.canvas_to_ndc(x - width, max_y);
            let tr = self.canvas_to_ndc(x + width, max_y);
            let br = self.canvas_to_ndc(x + width, min_y);
            let bl = self.canvas_to_ndc(x - width, min_y);
            add_quad(vertices, indices, tl, tr, br, bl, color);
        }

        // Horizontal lines
        let start_y = (min_y / gs).floor() as i32;
        let end_y = (max_y / gs).ceil() as i32;
        for i in start_y..=end_y {
            let y = i as f32 * gs;
            let color = if i == 0 {
                Self::AXIS_COLOR
            } else {
                Self::GRID_COLOR
            };
            let width = if i == 0 { hw * 2.0 } else { hw };

            let tl = self.canvas_to_ndc(min_x, y + width);
            let tr = self.canvas_to_ndc(max_x, y + width);
            let br = self.canvas_to_ndc(max_x, y - width);
            let bl = self.canvas_to_ndc(min_x, y - width);
            add_quad(vertices, indices, tl, tr, br, bl, color);
        }
    }

    /// Render an outline as a series of thin quad segments.
    fn render_outline(
        &self,
        outline: &Outline2D,
        color: [f32; 4],
        half_width: f32,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u32>,
    ) {
        if outline.points.len() < 2 {
            return;
        }

        for i in 0..outline.points.len() - 1 {
            self.render_segment(
                outline.points[i],
                outline.points[i + 1],
                color,
                half_width,
                vertices,
                indices,
            );
        }

        // Close the outline if flagged
        if outline.closed && outline.points.len() >= 3 {
            let last = outline.points.len() - 1;
            self.render_segment(
                outline.points[last],
                outline.points[0],
                color,
                half_width,
                vertices,
                indices,
            );
        }
    }

    /// Render a single line segment as a thin quad.
    fn render_segment(
        &self,
        a: [f32; 2],
        b: [f32; 2],
        color: [f32; 4],
        half_width: f32,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u32>,
    ) {
        let dx = b[0] - a[0];
        let dy = b[1] - a[1];
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-8 {
            return;
        }

        // Perpendicular direction, scaled by half_width
        let px = -dy / len * half_width;
        let py = dx / len * half_width;

        let tl = self.canvas_to_ndc(a[0] + px, a[1] + py);
        let tr = self.canvas_to_ndc(a[0] - px, a[1] - py);
        let br = self.canvas_to_ndc(b[0] - px, b[1] - py);
        let bl = self.canvas_to_ndc(b[0] + px, b[1] + py);

        add_quad(vertices, indices, tl, tr, br, bl, color);
    }
}

// ============================================================================
// RAMER-DOUGLAS-PEUCKER SIMPLIFICATION
// ============================================================================

/// Compute the perpendicular distance from point `p` to the line defined
/// by `line_start` and `line_end`.
fn perp_distance(p: [f32; 2], line_start: [f32; 2], line_end: [f32; 2]) -> f32 {
    let dx = line_end[0] - line_start[0];
    let dy = line_end[1] - line_start[1];
    let len_sq = dx * dx + dy * dy;

    if len_sq < 1e-12 {
        // Degenerate line: just return distance to start
        let ex = p[0] - line_start[0];
        let ey = p[1] - line_start[1];
        return (ex * ex + ey * ey).sqrt();
    }

    // Area of triangle formed by the three points, divided by base length
    let area = ((line_end[0] - line_start[0]) * (line_start[1] - p[1])
        - (line_start[0] - p[0]) * (line_end[1] - line_start[1]))
        .abs();

    area / len_sq.sqrt()
}

/// Ramer-Douglas-Peucker polyline simplification.
///
/// Recursively removes points that are within `epsilon` distance of the
/// line between the first and last point. This dramatically reduces
/// point counts from freehand strokes while preserving shape.
pub fn rdp_simplify(points: &[[f32; 2]], epsilon: f32) -> Vec<[f32; 2]> {
    if points.len() <= 2 {
        return points.to_vec();
    }

    let first = points[0];
    let last = *points.last().unwrap();

    // Find the point with maximum distance from the first-to-last line
    let mut max_dist: f32 = 0.0;
    let mut max_idx: usize = 0;

    for (i, p) in points.iter().enumerate().skip(1).take(points.len() - 2) {
        let dist = perp_distance(*p, first, last);
        if dist > max_dist {
            max_dist = dist;
            max_idx = i;
        }
    }

    if max_dist > epsilon {
        // Recursively simplify both halves
        let mut left = rdp_simplify(&points[..=max_idx], epsilon);
        let right = rdp_simplify(&points[max_idx..], epsilon);
        left.pop(); // Remove duplicate point at the split
        left.extend(right);
        left
    } else {
        // All points are within epsilon -- keep only endpoints
        vec![first, last]
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rdp_simplify_straight_line() {
        // Points on a straight line should simplify to just the endpoints
        let points = vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0], [3.0, 3.0]];
        let result = rdp_simplify(&points, 0.05);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0], [0.0, 0.0]);
        assert_eq!(result[1], [3.0, 3.0]);
    }

    #[test]
    fn test_rdp_simplify_preserves_corners() {
        // An L-shape should preserve the corner
        let points = vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [2.0, 2.0]];
        let result = rdp_simplify(&points, 0.05);
        assert!(result.len() >= 3, "L-shape corner should be preserved");
        assert_eq!(result[0], [0.0, 0.0]);
        assert_eq!(*result.last().unwrap(), [2.0, 2.0]);
    }

    #[test]
    fn test_rdp_simplify_two_points() {
        let points = vec![[0.0, 0.0], [5.0, 5.0]];
        let result = rdp_simplify(&points, 0.05);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_rdp_simplify_single_point() {
        let points = vec![[1.0, 1.0]];
        let result = rdp_simplify(&points, 0.05);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_rdp_simplify_empty() {
        let points: Vec<[f32; 2]> = vec![];
        let result = rdp_simplify(&points, 0.05);
        assert!(result.is_empty());
    }

    #[test]
    fn test_perp_distance_on_line() {
        let dist = perp_distance([1.0, 1.0], [0.0, 0.0], [2.0, 2.0]);
        assert!(dist < 0.001, "Point on line should have ~0 distance");
    }

    #[test]
    fn test_perp_distance_off_line() {
        let dist = perp_distance([1.0, 0.0], [0.0, 0.0], [0.0, 2.0]);
        assert!((dist - 1.0).abs() < 0.001, "Point should be 1 unit from line");
    }

    #[test]
    fn test_canvas_screen_to_canvas_center() {
        let canvas = Canvas2D::new();
        // Center of a 1280x800 viewport should map to origin
        let pos = canvas.screen_to_canvas(640.0, 400.0);
        assert!(pos[0].abs() < 0.01, "Center X should be near 0");
        assert!(pos[1].abs() < 0.01, "Center Y should be near 0");
    }

    #[test]
    fn test_canvas_zoom_clamp() {
        let mut canvas = Canvas2D::new();
        // Zoom in past maximum
        for _ in 0..100 {
            canvas.on_scroll(1.0);
        }
        assert!(canvas.zoom <= Canvas2D::MAX_ZOOM);

        // Zoom out past minimum
        for _ in 0..200 {
            canvas.on_scroll(-1.0);
        }
        assert!(canvas.zoom >= Canvas2D::MIN_ZOOM);
    }

    #[test]
    fn test_canvas_tool_switching() {
        let mut canvas = Canvas2D::new();
        assert_eq!(canvas.tool, DrawTool::Freehand);

        canvas.select_line();
        assert_eq!(canvas.tool, DrawTool::Line);

        canvas.select_freehand();
        assert_eq!(canvas.tool, DrawTool::Freehand);
    }

    #[test]
    fn test_canvas_grid_toggle() {
        let mut canvas = Canvas2D::new();
        assert!(canvas.show_grid);
        canvas.toggle_grid();
        assert!(!canvas.show_grid);
        canvas.toggle_grid();
        assert!(canvas.show_grid);
    }

    #[test]
    fn test_canvas_undo() {
        let mut canvas = Canvas2D::new();
        canvas.outlines.push(Outline2D {
            points: vec![[0.0, 0.0], [1.0, 1.0]],
            closed: false,
        });
        canvas.outlines.push(Outline2D {
            points: vec![[2.0, 2.0], [3.0, 3.0]],
            closed: false,
        });
        assert_eq!(canvas.outlines.len(), 2);

        canvas.undo();
        assert_eq!(canvas.outlines.len(), 1);

        canvas.undo();
        assert!(canvas.outlines.is_empty());
    }

    #[test]
    fn test_outline_default() {
        let outline = Outline2D::new();
        assert!(outline.is_empty());
        assert!(!outline.closed);
    }

    #[test]
    fn test_outline_with_start() {
        let outline = Outline2D::with_start([1.0, 2.0]);
        assert_eq!(outline.len(), 1);
        assert_eq!(outline.last_point(), Some([1.0, 2.0]));
    }

    #[test]
    fn test_snap_to_grid() {
        let mut canvas = Canvas2D::new();
        canvas.snap_to_grid = true;
        canvas.grid_size = 1.0;
        let snapped = canvas.maybe_snap([0.3, 0.7]);
        assert_eq!(snapped, [0.0, 1.0]);

        canvas.snap_to_grid = false;
        let not_snapped = canvas.maybe_snap([0.3, 0.7]);
        assert_eq!(not_snapped, [0.3, 0.7]);
    }

    #[test]
    fn test_render_produces_vertices() {
        let mut canvas = Canvas2D::new();
        canvas.outlines.push(Outline2D {
            points: vec![[0.0, 0.0], [1.0, 1.0], [2.0, 0.0]],
            closed: false,
        });

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        canvas.render(&mut vertices, &mut indices);

        // Should have at least the outline segments + grid
        assert!(!vertices.is_empty(), "Rendering should produce vertices");
        assert!(!indices.is_empty(), "Rendering should produce indices");
    }

    #[test]
    fn test_draw_tool_display() {
        assert_eq!(format!("{}", DrawTool::Freehand), "Freehand");
        assert_eq!(format!("{}", DrawTool::Line), "Line");
    }
}
