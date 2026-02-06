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

use crate::game::asset_editor::image_trace::ImageTrace;
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
    /// Arc tool: 3-click arc creation via circumscribed circle computation.
    Arc,
    /// Eraser tool: circle cursor that removes points within radius.
    Eraser,
}

impl std::fmt::Display for DrawTool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrawTool::Freehand => write!(f, "Freehand"),
            DrawTool::Line => write!(f, "Line"),
            DrawTool::Arc => write!(f, "Arc"),
            DrawTool::Eraser => write!(f, "Eraser"),
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

    /// Whether X-axis mirror symmetry is enabled.
    /// When active, a dashed vertical line is rendered at x=0 and all outlines
    /// are mirrored across the Y axis during rendering.
    pub mirror_x: bool,
    /// Accumulated click points for the arc tool (up to 3).
    pub arc_points: Vec<[f32; 2]>,
    /// Eraser circle radius in canvas units.
    pub eraser_radius: f32,

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

    /// Optional background reference image for tracing.
    pub image_trace: Option<ImageTrace>,
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
    /// Color for the mirror symmetry dashed line at x=0.
    const MIRROR_LINE_COLOR: [f32; 4] = [1.0, 0.4, 0.4, 0.7];
    /// Color for mirrored (reflected) outlines.
    const MIRROR_OUTLINE_COLOR: [f32; 4] = [0.7, 0.7, 1.0, 0.5];
    /// Color for the eraser cursor circle.
    const ERASER_CURSOR_COLOR: [f32; 4] = [1.0, 0.3, 0.3, 0.6];
    /// Color for arc tool click-point markers.
    const ARC_POINT_COLOR: [f32; 4] = [1.0, 0.8, 0.2, 0.9];
    /// Minimum eraser radius in canvas units.
    const MIN_ERASER_RADIUS: f32 = 0.1;
    /// Maximum eraser radius in canvas units.
    const MAX_ERASER_RADIUS: f32 = 3.0;
    /// Eraser radius step per bracket key press.
    const ERASER_RADIUS_STEP: f32 = 0.1;
    /// Default eraser radius in canvas units.
    const DEFAULT_ERASER_RADIUS: f32 = 0.5;
    /// Number of line segments used to approximate arc curves.
    const ARC_SEGMENTS: usize = 32;
    /// Dash length for the mirror symmetry line (in canvas units).
    const MIRROR_DASH_LENGTH: f32 = 0.3;
    /// Gap length for the mirror symmetry line (in canvas units).
    const MIRROR_GAP_LENGTH: f32 = 0.15;
    /// Number of line segments used to render the eraser cursor circle.
    const ERASER_CURSOR_SEGMENTS: usize = 24;

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
            mirror_x: false,
            arc_points: Vec::new(),
            eraser_radius: Self::DEFAULT_ERASER_RADIUS,
            drawing: false,
            line_start: None,
            current_mouse_canvas: None,
            middle_mouse_held: false,
            last_mouse_screen: None,
            viewport_size: [1280.0, 800.0],
            image_trace: None,
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
            // Cancel any in-progress state from other tools
            self.line_start = None;
            self.arc_points.clear();
            self.tool = DrawTool::Freehand;
            println!("Canvas tool: Freehand");
        }
    }

    /// Switch to the line drawing tool.
    pub fn select_line(&mut self) {
        if self.tool != DrawTool::Line {
            // Cancel any in-progress state from other tools
            self.finish_freehand_stroke();
            self.arc_points.clear();
            self.tool = DrawTool::Line;
            println!("Canvas tool: Line");
        }
    }

    /// Switch to the arc drawing tool.
    pub fn select_arc(&mut self) {
        if self.tool != DrawTool::Arc {
            // Cancel any in-progress state from other tools
            self.finish_freehand_stroke();
            self.line_start = None;
            self.arc_points.clear();
            self.tool = DrawTool::Arc;
            println!("Canvas tool: Arc");
        }
    }

    /// Switch to the eraser tool.
    pub fn select_eraser(&mut self) {
        if self.tool != DrawTool::Eraser {
            // Cancel any in-progress state from other tools
            self.finish_freehand_stroke();
            self.line_start = None;
            self.arc_points.clear();
            self.tool = DrawTool::Eraser;
            println!("Canvas tool: Eraser (radius: {:.1})", self.eraser_radius);
        }
    }

    /// Toggle X-axis mirror symmetry.
    pub fn toggle_mirror(&mut self) {
        self.mirror_x = !self.mirror_x;
        println!(
            "Mirror X: {}",
            if self.mirror_x { "on" } else { "off" }
        );
    }

    /// Increase eraser radius by one step, clamped to maximum.
    pub fn increase_eraser_radius(&mut self) {
        self.eraser_radius =
            (self.eraser_radius + Self::ERASER_RADIUS_STEP).min(Self::MAX_ERASER_RADIUS);
        println!("Eraser radius: {:.1}", self.eraser_radius);
    }

    /// Decrease eraser radius by one step, clamped to minimum.
    pub fn decrease_eraser_radius(&mut self) {
        self.eraser_radius =
            (self.eraser_radius - Self::ERASER_RADIUS_STEP).max(Self::MIN_ERASER_RADIUS);
        println!("Eraser radius: {:.1}", self.eraser_radius);
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
            DrawTool::Arc => {
                self.handle_arc_click(canvas_pos);
            }
            DrawTool::Eraser => {
                self.drawing = true;
                self.handle_eraser_at(canvas_pos);
            }
        }
    }

    /// Handle left mouse button release.
    pub fn on_left_release(&mut self) {
        if self.tool == DrawTool::Freehand && self.drawing {
            self.finish_freehand_stroke();
        }
        if self.tool == DrawTool::Eraser && self.drawing {
            self.drawing = false;
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

        // Eraser: erase while dragging with left mouse held
        if self.tool == DrawTool::Eraser && self.drawing {
            self.handle_eraser_at(canvas_pos);
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

    /// Handle a click for the arc tool.
    /// Three clicks define the arc: first and third are endpoints, second is
    /// a through-point. A circumscribed circle is computed and arc points are
    /// generated along the shorter arc from point 1 to point 3 passing
    /// through point 2.
    fn handle_arc_click(&mut self, canvas_pos: [f32; 2]) {
        self.arc_points.push(canvas_pos);
        println!("Arc tool: point {} set", self.arc_points.len());

        if self.arc_points.len() == 3 {
            let p1 = self.arc_points[0];
            let p2 = self.arc_points[1];
            let p3 = self.arc_points[2];

            if let Some((center, radius)) = circumscribed_circle(p1, p2, p3) {
                // Compute angles from center to each point
                let a1 = (p1[1] - center[1]).atan2(p1[0] - center[0]);
                let a2 = (p2[1] - center[1]).atan2(p2[0] - center[0]);
                let a3 = (p3[1] - center[1]).atan2(p3[0] - center[0]);

                // Determine sweep direction: we want to go from a1 to a3
                // passing through a2. Check both clockwise and counter-clockwise.
                let arc_points = generate_arc_points(center, radius, a1, a2, a3, Self::ARC_SEGMENTS);

                if arc_points.len() >= 2 {
                    let outline = Outline2D {
                        points: arc_points,
                        closed: false,
                    };
                    self.outlines.push(outline);
                    println!("Arc tool: arc created with {} points", self.outlines.last().map_or(0, |o| o.len()));
                }
            } else {
                println!("Arc tool: collinear points, creating straight line instead");
                let outline = Outline2D {
                    points: vec![p1, p3],
                    closed: false,
                };
                self.outlines.push(outline);
            }

            self.arc_points.clear();
        }
    }

    /// Handle eraser action at the given canvas position.
    /// Removes points within `eraser_radius` of the cursor and splits
    /// outlines at the resulting gaps.
    fn handle_eraser_at(&mut self, cursor: [f32; 2]) {
        let radius = self.eraser_radius;
        let mut new_outlines: Vec<Outline2D> = Vec::new();

        for outline in self.outlines.drain(..) {
            let result = erase_near(&outline, cursor, radius);
            new_outlines.extend(result);
        }

        self.outlines = new_outlines;
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

        // 2. Mirror symmetry dashed line at x=0
        if self.mirror_x {
            self.render_mirror_line(vertices, indices);
        }

        // 3. Completed outlines
        for outline in &self.outlines {
            self.render_outline(outline, Self::OUTLINE_COLOR, Self::LINE_HALF_WIDTH, vertices, indices);
        }

        // 4. Mirror: render reflected copies of completed outlines
        if self.mirror_x {
            for outline in &self.outlines {
                let mirrored = mirror_outline_x(outline);
                self.render_outline(&mirrored, Self::MIRROR_OUTLINE_COLOR, Self::LINE_HALF_WIDTH, vertices, indices);
            }
        }

        // 5. Active (in-progress) outline
        if let Some(ref active) = self.active_outline {
            self.render_outline(active, Self::ACTIVE_COLOR, Self::LINE_HALF_WIDTH, vertices, indices);
            // Mirror the active outline too
            if self.mirror_x {
                let mirrored = mirror_outline_x(active);
                self.render_outline(&mirrored, Self::MIRROR_OUTLINE_COLOR, Self::LINE_HALF_WIDTH, vertices, indices);
            }
        }

        // 6. Line tool preview
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
                // Mirror the line preview too
                if self.mirror_x {
                    self.render_segment(
                        [-start[0], start[1]],
                        [-mouse[0], mouse[1]],
                        Self::MIRROR_OUTLINE_COLOR,
                        Self::LINE_HALF_WIDTH,
                        vertices,
                        indices,
                    );
                }
            }
        }

        // 7. Arc tool: render click-point markers and preview
        if self.tool == DrawTool::Arc {
            self.render_arc_preview(vertices, indices);
        }

        // 8. Eraser tool: render cursor circle
        if self.tool == DrawTool::Eraser {
            if let Some(mouse) = self.current_mouse_canvas {
                self.render_eraser_cursor(mouse, vertices, indices);
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

    /// Render a dashed vertical line at x=0 to indicate mirror symmetry axis.
    fn render_mirror_line(&self, vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) {
        let half_y = Self::DEFAULT_HALF_EXTENT / self.zoom;

        let min_y = self.pan[1] - half_y;
        let max_y = self.pan[1] + half_y;

        // Scale dash/gap with zoom so they remain a consistent visual size
        let dash = Self::MIRROR_DASH_LENGTH / self.zoom.sqrt();
        let gap = Self::MIRROR_GAP_LENGTH / self.zoom.sqrt();
        let hw = Self::LINE_HALF_WIDTH * 1.5;

        let mut y = min_y;
        while y < max_y {
            let y_end = (y + dash).min(max_y);
            self.render_segment(
                [0.0, y],
                [0.0, y_end],
                Self::MIRROR_LINE_COLOR,
                hw,
                vertices,
                indices,
            );
            y += dash + gap;
        }
    }

    /// Render arc tool preview: small markers at each clicked point,
    /// and a preview curve from existing points through the current mouse position.
    fn render_arc_preview(&self, vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) {
        let marker_size = 0.08 / self.zoom.sqrt();

        // Render markers at each accumulated arc point
        for pt in &self.arc_points {
            let tl = self.canvas_to_ndc(pt[0] - marker_size, pt[1] + marker_size);
            let tr = self.canvas_to_ndc(pt[0] + marker_size, pt[1] + marker_size);
            let br = self.canvas_to_ndc(pt[0] + marker_size, pt[1] - marker_size);
            let bl = self.canvas_to_ndc(pt[0] - marker_size, pt[1] - marker_size);
            add_quad(vertices, indices, tl, tr, br, bl, Self::ARC_POINT_COLOR);
        }

        // Preview the arc with current mouse as the next point
        if let Some(mouse) = self.current_mouse_canvas {
            match self.arc_points.len() {
                1 => {
                    // One point placed: show a line preview to mouse
                    self.render_segment(
                        self.arc_points[0],
                        mouse,
                        Self::PREVIEW_COLOR,
                        Self::LINE_HALF_WIDTH,
                        vertices,
                        indices,
                    );
                }
                2 => {
                    // Two points placed: show arc preview through mouse
                    let p1 = self.arc_points[0];
                    let p2 = self.arc_points[1];
                    let p3 = mouse;
                    if let Some((center, radius)) = circumscribed_circle(p1, p2, p3) {
                        let a1 = (p1[1] - center[1]).atan2(p1[0] - center[0]);
                        let a2 = (p2[1] - center[1]).atan2(p2[0] - center[0]);
                        let a3 = (p3[1] - center[1]).atan2(p3[0] - center[0]);
                        let preview_points =
                            generate_arc_points(center, radius, a1, a2, a3, Self::ARC_SEGMENTS);
                        let preview_outline = Outline2D {
                            points: preview_points,
                            closed: false,
                        };
                        self.render_outline(
                            &preview_outline,
                            Self::PREVIEW_COLOR,
                            Self::LINE_HALF_WIDTH,
                            vertices,
                            indices,
                        );
                    } else {
                        // Collinear: just show a line
                        self.render_segment(
                            p1,
                            p3,
                            Self::PREVIEW_COLOR,
                            Self::LINE_HALF_WIDTH,
                            vertices,
                            indices,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    /// Render the eraser cursor as a circle at the given canvas position.
    fn render_eraser_cursor(
        &self,
        center: [f32; 2],
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u32>,
    ) {
        let segments = Self::ERASER_CURSOR_SEGMENTS;
        let r = self.eraser_radius;

        for i in 0..segments {
            let angle_a = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let angle_b = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;

            let ax = center[0] + r * angle_a.cos();
            let ay = center[1] + r * angle_a.sin();
            let bx = center[0] + r * angle_b.cos();
            let by = center[1] + r * angle_b.sin();

            self.render_segment(
                [ax, ay],
                [bx, by],
                Self::ERASER_CURSOR_COLOR,
                Self::LINE_HALF_WIDTH,
                vertices,
                indices,
            );
        }
    }
}

// ============================================================================
// CIRCUMSCRIBED CIRCLE & ARC GENERATION
// ============================================================================

/// Compute the circumscribed circle (circumcircle) of three points.
/// Returns `Some((center, radius))` if the points are not collinear,
/// `None` otherwise.
fn circumscribed_circle(p1: [f32; 2], p2: [f32; 2], p3: [f32; 2]) -> Option<([f32; 2], f32)> {
    let ax = p1[0];
    let ay = p1[1];
    let bx = p2[0];
    let by = p2[1];
    let cx = p3[0];
    let cy = p3[1];

    let d = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if d.abs() < 1e-10 {
        return None; // collinear
    }

    let ux = ((ax * ax + ay * ay) * (by - cy)
        + (bx * bx + by * by) * (cy - ay)
        + (cx * cx + cy * cy) * (ay - by))
        / d;
    let uy = ((ax * ax + ay * ay) * (cx - bx)
        + (bx * bx + by * by) * (ax - cx)
        + (cx * cx + cy * cy) * (bx - ax))
        / d;

    let r = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();
    Some(([ux, uy], r))
}

/// Generate points along an arc from angle `a1` to angle `a3`, passing through
/// the angle `a2`. The arc is on a circle with the given `center` and `radius`.
///
/// The direction (clockwise vs counter-clockwise) is chosen so that `a2` lies
/// on the arc between `a1` and `a3`.
fn generate_arc_points(
    center: [f32; 2],
    radius: f32,
    a1: f32,
    a2: f32,
    a3: f32,
    num_segments: usize,
) -> Vec<[f32; 2]> {
    use std::f32::consts::TAU;

    // Normalize angle difference to [0, TAU)
    let normalize = |a: f32| -> f32 { ((a % TAU) + TAU) % TAU };

    let start = a1;

    // Compute the CCW sweep from a1 to a3
    let sweep_ccw = normalize(a3 - a1);
    // And the CW sweep (going the other way)
    let sweep_cw = TAU - sweep_ccw;

    // Check if a2 falls within the CCW sweep from a1
    let a2_offset = normalize(a2 - a1);

    // If a2 is within the CCW arc from a1 to a3, use CCW direction; otherwise CW
    let sweep = if a2_offset <= sweep_ccw {
        sweep_ccw
    } else {
        -sweep_cw
    };

    let mut points = Vec::with_capacity(num_segments + 1);
    for i in 0..=num_segments {
        let t = i as f32 / num_segments as f32;
        let angle = start + sweep * t;
        let x = center[0] + radius * angle.cos();
        let y = center[1] + radius * angle.sin();
        points.push([x, y]);
    }

    points
}

// ============================================================================
// ERASER LOGIC
// ============================================================================

/// Erase points within `radius` of `cursor` from an outline,
/// splitting the outline at gaps where points were removed.
/// Returns the remaining outline segments (each with at least 2 points).
fn erase_near(outline: &Outline2D, cursor: [f32; 2], radius: f32) -> Vec<Outline2D> {
    let mut segments: Vec<Vec<[f32; 2]>> = vec![vec![]];

    for &pt in &outline.points {
        let dx = pt[0] - cursor[0];
        let dy = pt[1] - cursor[1];
        let dist = (dx * dx + dy * dy).sqrt();

        if dist > radius {
            segments.last_mut().unwrap().push(pt);
        } else if !segments.last().unwrap().is_empty() {
            segments.push(vec![]);
        }
    }

    segments
        .into_iter()
        .filter(|s| s.len() >= 2)
        .map(|points| Outline2D {
            points,
            closed: false,
        })
        .collect()
}

// ============================================================================
// MIRROR UTILITY
// ============================================================================

/// Create a mirrored copy of an outline across the Y axis (x negated).
fn mirror_outline_x(outline: &Outline2D) -> Outline2D {
    Outline2D {
        points: outline.points.iter().map(|p| [-p[0], p[1]]).collect(),
        closed: outline.closed,
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
        assert_eq!(format!("{}", DrawTool::Arc), "Arc");
        assert_eq!(format!("{}", DrawTool::Eraser), "Eraser");
    }

    // -- Arc tool tests --

    #[test]
    fn test_circumscribed_circle_known_triangle() {
        // Right triangle at (0,0), (1,0), (0,1) -- circumcircle center at (0.5, 0.5)
        let result = circumscribed_circle([0.0, 0.0], [1.0, 0.0], [0.0, 1.0]);
        assert!(result.is_some());
        let (center, radius) = result.unwrap();
        assert!(
            (center[0] - 0.5).abs() < 0.001,
            "Center X should be ~0.5, got {}",
            center[0]
        );
        assert!(
            (center[1] - 0.5).abs() < 0.001,
            "Center Y should be ~0.5, got {}",
            center[1]
        );
        let expected_r = (0.5_f32 * 0.5 + 0.5 * 0.5).sqrt();
        assert!(
            (radius - expected_r).abs() < 0.001,
            "Radius should be ~{}, got {}",
            expected_r,
            radius
        );
    }

    #[test]
    fn test_circumscribed_circle_collinear_returns_none() {
        let result = circumscribed_circle([0.0, 0.0], [1.0, 1.0], [2.0, 2.0]);
        assert!(result.is_none(), "Collinear points should return None");
    }

    #[test]
    fn test_generate_arc_points_count() {
        let points = generate_arc_points(
            [0.0, 0.0],
            1.0,
            0.0,
            std::f32::consts::FRAC_PI_2,
            std::f32::consts::PI,
            16,
        );
        assert_eq!(points.len(), 17, "Should have num_segments + 1 points");
    }

    #[test]
    fn test_arc_tool_selection() {
        let mut canvas = Canvas2D::new();
        canvas.select_arc();
        assert_eq!(canvas.tool, DrawTool::Arc);
        assert!(canvas.arc_points.is_empty());
    }

    #[test]
    fn test_arc_tool_clears_on_switch() {
        let mut canvas = Canvas2D::new();
        canvas.select_arc();
        canvas.arc_points.push([1.0, 0.0]);
        canvas.arc_points.push([0.0, 1.0]);
        // Switch to freehand should clear arc points
        canvas.select_freehand();
        assert_eq!(canvas.tool, DrawTool::Freehand);
        assert!(canvas.arc_points.is_empty());
    }

    // -- Eraser tests --

    #[test]
    fn test_erase_near_removes_points() {
        let outline = Outline2D {
            points: vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [2.0, 0.0],
                [3.0, 0.0],
                [4.0, 0.0],
            ],
            closed: false,
        };
        // Erase around x=2 with radius 0.5 -- should remove point [2.0, 0.0]
        let result = erase_near(&outline, [2.0, 0.0], 0.5);
        assert_eq!(result.len(), 2, "Should split into 2 segments");
        assert_eq!(result[0].points.len(), 2); // [0,0], [1,0]
        assert_eq!(result[1].points.len(), 2); // [3,0], [4,0]
    }

    #[test]
    fn test_erase_near_no_match() {
        let outline = Outline2D {
            points: vec![[0.0, 0.0], [1.0, 0.0], [2.0, 0.0]],
            closed: false,
        };
        // Eraser far away -- should return the whole outline unchanged
        let result = erase_near(&outline, [10.0, 10.0], 0.5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].points.len(), 3);
    }

    #[test]
    fn test_erase_near_all_removed() {
        let outline = Outline2D {
            points: vec![[0.0, 0.0], [0.1, 0.0]],
            closed: false,
        };
        // Eraser covers all points
        let result = erase_near(&outline, [0.0, 0.0], 5.0);
        assert!(result.is_empty(), "All points erased, no segments remain");
    }

    #[test]
    fn test_eraser_tool_selection() {
        let mut canvas = Canvas2D::new();
        canvas.select_eraser();
        assert_eq!(canvas.tool, DrawTool::Eraser);
    }

    #[test]
    fn test_eraser_radius_adjustment() {
        let mut canvas = Canvas2D::new();
        let initial = canvas.eraser_radius;

        canvas.increase_eraser_radius();
        assert!(canvas.eraser_radius > initial);

        canvas.decrease_eraser_radius();
        assert!((canvas.eraser_radius - initial).abs() < 0.001);

        // Test clamping at minimum
        for _ in 0..100 {
            canvas.decrease_eraser_radius();
        }
        assert!(canvas.eraser_radius >= Canvas2D::MIN_ERASER_RADIUS);

        // Test clamping at maximum
        for _ in 0..100 {
            canvas.increase_eraser_radius();
        }
        assert!(canvas.eraser_radius <= Canvas2D::MAX_ERASER_RADIUS);
    }

    // -- Mirror tests --

    #[test]
    fn test_mirror_toggle() {
        let mut canvas = Canvas2D::new();
        assert!(!canvas.mirror_x);
        canvas.toggle_mirror();
        assert!(canvas.mirror_x);
        canvas.toggle_mirror();
        assert!(!canvas.mirror_x);
    }

    #[test]
    fn test_mirror_outline_x() {
        let outline = Outline2D {
            points: vec![[1.0, 2.0], [3.0, 4.0]],
            closed: true,
        };
        let mirrored = mirror_outline_x(&outline);
        assert_eq!(mirrored.points[0], [-1.0, 2.0]);
        assert_eq!(mirrored.points[1], [-3.0, 4.0]);
        assert!(mirrored.closed);
    }

    #[test]
    fn test_mirror_render_produces_extra_vertices() {
        let mut canvas = Canvas2D::new();
        canvas.outlines.push(Outline2D {
            points: vec![[1.0, 0.0], [2.0, 1.0], [3.0, 0.0]],
            closed: false,
        });

        // Render without mirror
        let mut v1 = Vec::new();
        let mut i1 = Vec::new();
        canvas.render(&mut v1, &mut i1);
        let count_no_mirror = v1.len();

        // Render with mirror
        canvas.mirror_x = true;
        let mut v2 = Vec::new();
        let mut i2 = Vec::new();
        canvas.render(&mut v2, &mut i2);
        let count_with_mirror = v2.len();

        assert!(
            count_with_mirror > count_no_mirror,
            "Mirror should produce additional vertices: {} vs {}",
            count_with_mirror,
            count_no_mirror
        );
    }
}
