//! Asset Editor Module
//!
//! Contains the core types and logic for the standalone asset editor binary.
//! The editor allows creating game assets through a multi-stage pipeline:
//! Draw2D -> Extrude -> Sculpt -> Color -> Save.

pub mod asset_file;
pub mod canvas_2d;
pub mod extrude;
pub mod image_trace;
pub mod orbit_camera;
pub mod paint;
pub mod sculpt_bridge;
pub mod ui_panels;
pub mod undo;
pub mod variety;

use glam::Vec3;

use crate::game::types::Vertex;
use crate::render::building_blocks::BlockVertex;
use asset_file::{AssetMetadata, load_btasset, save_btasset};
use canvas_2d::Canvas2D;
use extrude::{Extruder, PumpProfile};
use orbit_camera::OrbitCamera;
use paint::PaintSystem;
use sculpt_bridge::SculptBridge;
use undo::UndoStack;
use variety::VarietyParams;

// ============================================================================
// ENUMS
// ============================================================================

/// The current stage of the asset editing pipeline.
/// Each stage represents a step in the asset creation workflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorStage {
    /// Draw 2D outlines/silhouettes
    Draw2D,
    /// Extrude 2D shapes into 3D geometry
    Extrude,
    /// Sculpt and refine 3D mesh
    Sculpt,
    /// Apply colors and materials
    Color,
    /// Save the completed asset
    Save,
}

impl EditorStage {
    /// Return the display name for this stage.
    pub fn name(&self) -> &'static str {
        match self {
            EditorStage::Draw2D => "Draw 2D",
            EditorStage::Extrude => "Extrude",
            EditorStage::Sculpt => "Sculpt",
            EditorStage::Color => "Color",
            EditorStage::Save => "Save",
        }
    }

    /// Return the stage number (1-5) for display.
    pub fn number(&self) -> u32 {
        match self {
            EditorStage::Draw2D => 1,
            EditorStage::Extrude => 2,
            EditorStage::Sculpt => 3,
            EditorStage::Color => 4,
            EditorStage::Save => 5,
        }
    }

    /// Create a stage from a key number (1-5). Returns None for invalid input.
    pub fn from_key(key: u32) -> Option<EditorStage> {
        match key {
            1 => Some(EditorStage::Draw2D),
            2 => Some(EditorStage::Extrude),
            3 => Some(EditorStage::Sculpt),
            4 => Some(EditorStage::Color),
            5 => Some(EditorStage::Save),
            _ => None,
        }
    }
}

impl std::fmt::Display for EditorStage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Categories of assets that can be created in the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetCategory {
    /// Trees and vegetation
    Tree,
    /// Grass and ground cover
    Grass,
    /// Rocks and boulders
    Rock,
    /// Buildings and structures
    Structure,
    /// Interactive props
    Prop,
    /// Decorative elements
    Decoration,
}

impl AssetCategory {
    /// Return the display name for this category.
    pub fn name(&self) -> &'static str {
        match self {
            AssetCategory::Tree => "Tree",
            AssetCategory::Grass => "Grass",
            AssetCategory::Rock => "Rock",
            AssetCategory::Structure => "Structure",
            AssetCategory::Prop => "Prop",
            AssetCategory::Decoration => "Decoration",
        }
    }
}

impl std::fmt::Display for AssetCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

// ============================================================================
// STRUCTS
// ============================================================================

/// A 2D outline point used in the Draw2D stage.
#[derive(Debug, Clone)]
pub struct OutlinePoint {
    /// Position in 2D editor space
    pub position: glam::Vec2,
}

/// Draft state for an asset being created in the editor.
/// Accumulates data as the user progresses through stages.
#[derive(Debug, Clone)]
pub struct AssetDraft {
    /// Name of the asset being created
    pub name: String,
    /// Category of the asset
    pub category: AssetCategory,
    /// 2D outline points (from Draw2D stage)
    pub outlines: Vec<OutlinePoint>,
    /// Generated mesh vertices (from Extrude/Sculpt stages)
    pub vertices: Vec<Vec3>,
    /// Generated mesh indices
    pub indices: Vec<u32>,
    /// Vertex colors (from Color stage)
    pub colors: Vec<[f32; 4]>,
    /// Number of varieties to generate
    pub variety_count: u32,
    /// Random seed for variety generation
    pub variety_seed: u32,
    /// Scale factor for the asset
    pub scale: f32,
}

impl Default for AssetDraft {
    fn default() -> Self {
        Self {
            name: String::from("Untitled"),
            category: AssetCategory::Prop,
            outlines: Vec::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            colors: Vec::new(),
            variety_count: 1,
            variety_seed: 42,
            scale: 1.0,
        }
    }
}

/// Stage 5 (Save) dialog state.
///
/// Holds the user-editable fields displayed when the editor is in the Save stage.
#[derive(Debug, Clone)]
pub struct SaveDialog {
    /// Asset name entered by the user.
    pub name: String,
    /// Category slug (e.g. "tree", "rock").
    pub category: String,
    /// Comma-separated tags.
    pub tags: String,
    /// Status message shown after a save or load attempt.
    pub status_message: String,
}

impl Default for SaveDialog {
    fn default() -> Self {
        Self {
            name: String::from("Untitled"),
            category: String::from("prop"),
            tags: String::new(),
            status_message: String::new(),
        }
    }
}

/// The main asset editor state.
/// Manages the editing pipeline and current draft asset.
pub struct AssetEditor {
    /// Current editing stage
    pub stage: EditorStage,
    /// The asset currently being edited
    pub draft: AssetDraft,
    /// Whether the editor is actively editing (vs idle/menu)
    pub active: bool,
    /// Undo/redo command stack
    pub undo_stack: UndoStack,
    /// Orbit camera for 3D preview in stages 2-5
    pub camera: OrbitCamera,
    /// 2D drawing canvas for the Draw2D stage (Stage 1)
    pub canvas: Canvas2D,
    /// Pump/inflate extruder for Stage 2 (Extrude)
    pub extruder: Extruder,
    /// Sculpting bridge for Stage 3 (Sculpt)
    pub sculpt: SculptBridge,
    /// Vertex color painting for Stage 4 (Color)
    pub paint: PaintSystem,
    /// Stage 5 save dialog fields
    pub save_dialog: SaveDialog,
}

impl Default for AssetEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetEditor {
    /// Create a new asset editor in the initial state.
    pub fn new() -> Self {
        Self {
            stage: EditorStage::Draw2D,
            draft: AssetDraft::default(),
            active: true,
            undo_stack: UndoStack::new(),
            camera: OrbitCamera::new(1280.0 / 800.0),
            canvas: Canvas2D::new(),
            extruder: Extruder::new(),
            sculpt: SculptBridge::new(),
            paint: PaintSystem::new(),
            save_dialog: SaveDialog::default(),
        }
    }

    /// Switch to a different editor stage.
    ///
    /// When entering the Extrude stage, automatically generates a 3D preview
    /// mesh from the current canvas outlines.
    pub fn set_stage(&mut self, stage: EditorStage) {
        println!("Editor stage: {} -> {}", self.stage, stage);
        self.stage = stage;

        // When entering Extrude stage, generate 3D mesh from canvas outlines
        if stage == EditorStage::Extrude {
            self.regenerate_extrude_mesh();
        }
    }

    /// Try to switch stage by key number (1-5).
    /// Returns true if the stage was changed.
    pub fn set_stage_by_key(&mut self, key: u32) -> bool {
        if let Some(stage) = EditorStage::from_key(key) {
            if stage != self.stage {
                self.set_stage(stage);
                return true;
            }
        }
        false
    }

    /// Update the editor state for the current frame.
    pub fn update(&mut self, _delta_time: f32) {
        // Stub: will be filled in by subsequent stories
        // Each stage will have its own update logic
    }

    /// Render the editor UI and viewport.
    /// Returns vertex/index data for the current stage.
    /// For Draw2D stage, delegates to the Canvas2D renderer.
    pub fn render_stage(&self, vertices: &mut Vec<Vertex>, indices: &mut Vec<u32>) {
        match self.stage {
            EditorStage::Draw2D => {
                self.canvas.render(vertices, indices);
            }
            _ => {
                // Other stages will be filled in by subsequent stories
            }
        }
    }

    /// Get the window title string showing the current stage.
    pub fn window_title(&self) -> String {
        format!(
            "Battle T\u{00f6}k \u{2014} Asset Editor [{}/5: {}]",
            self.stage.number(),
            self.stage.name()
        )
    }

    /// Returns `true` if the orbit camera should be active for the current stage.
    ///
    /// The orbit camera is used in stages 2-5 (Extrude, Sculpt, Color, Save)
    /// for 3D preview. Stage 1 (Draw2D) uses an orthographic 2D canvas.
    pub fn uses_orbit_camera(&self) -> bool {
        self.stage != EditorStage::Draw2D
    }

    /// Regenerate the 3D extrude mesh from the current canvas outlines.
    ///
    /// Called when entering the Extrude stage or when extrude parameters change.
    /// Passes the canvas outlines to the extruder, which evaluates the pump SDF
    /// and runs Marching Cubes to produce a triangle mesh.
    pub fn regenerate_extrude_mesh(&mut self) {
        let outlines = &self.canvas.outlines;
        if outlines.is_empty() {
            println!("Extrude: no outlines to extrude");
            return;
        }

        let success = self.extruder.generate_preview(outlines);
        if success {
            println!(
                "Extrude: generated mesh with {} vertices, {} indices",
                self.extruder.mesh_vertices.len(),
                self.extruder.mesh_indices.len()
            );
        } else {
            println!("Extrude: failed to generate mesh (need >= 3 points in a closed outline)");
        }
    }

    /// Cycle the pump profile to the next variant and regenerate the mesh.
    pub fn cycle_pump_profile(&mut self) {
        self.extruder.params.profile = match self.extruder.params.profile {
            PumpProfile::Elliptical => PumpProfile::Flat,
            PumpProfile::Flat => PumpProfile::Pointed,
            PumpProfile::Pointed => PumpProfile::Elliptical,
        };
        println!("Extrude: profile -> {:?}", self.extruder.params.profile);
        self.extruder.dirty = true;
        self.regenerate_extrude_mesh();
    }

    /// Adjust the inflation parameter and regenerate the mesh.
    pub fn adjust_inflation(&mut self, delta: f32) {
        self.extruder.params.inflation = (self.extruder.params.inflation + delta).clamp(0.0, 1.0);
        println!(
            "Extrude: inflation -> {:.2}",
            self.extruder.params.inflation
        );
        self.extruder.dirty = true;
        self.regenerate_extrude_mesh();
    }

    /// Adjust the thickness parameter and regenerate the mesh.
    pub fn adjust_thickness(&mut self, delta: f32) {
        self.extruder.params.thickness = (self.extruder.params.thickness + delta).clamp(0.1, 5.0);
        println!(
            "Extrude: thickness -> {:.1}",
            self.extruder.params.thickness
        );
        self.extruder.dirty = true;
        self.regenerate_extrude_mesh();
    }

    /// Reset the editor to start a new asset.
    pub fn reset(&mut self) {
        self.draft = AssetDraft::default();
        self.stage = EditorStage::Draw2D;
        self.active = true;
        self.undo_stack.clear();
        self.camera.reset();
        self.canvas = Canvas2D::new();
        self.sculpt = SculptBridge::new();
        self.paint = PaintSystem::new();
        self.save_dialog = SaveDialog::default();
    }

    /// Save the current mesh as a .btasset file.
    ///
    /// Uses the save dialog fields for metadata. Writes to
    /// `assets/world/<category>/<name>.btasset`.
    pub fn save_asset(&mut self) -> Result<std::path::PathBuf, String> {
        let vertices = self.collect_vertices();
        let indices = self.collect_indices();

        if vertices.is_empty() {
            return Err("No mesh data to save".to_string());
        }

        let tags: Vec<String> = self
            .save_dialog
            .tags
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();

        let metadata = AssetMetadata {
            name: self.save_dialog.name.clone(),
            category: self.save_dialog.category.clone(),
            tags,
            created_at: String::new(), // no chrono dependency; left empty
            vertex_count: vertices.len() as u32,
            index_count: indices.len() as u32,
        };

        let variety = VarietyParams::default();

        let file_name = self.save_dialog.name.to_lowercase().replace(' ', "_");
        let dir = std::path::PathBuf::from("assets/world").join(&self.save_dialog.category);
        let path = dir.join(format!("{file_name}.btasset"));

        save_btasset(&path, &vertices, &indices, &metadata, Some(&variety))
            .map_err(|e| format!("{e}"))?;

        let msg = format!("Saved: {}", path.display());
        println!("{msg}");
        self.save_dialog.status_message = msg;
        Ok(path)
    }

    /// Load a .btasset file and replace the current editor mesh.
    pub fn load_asset(&mut self, path: &std::path::Path) -> Result<(), String> {
        let loaded = load_btasset(path).map_err(|e| format!("{e}"))?;

        // Convert Vertex -> BlockVertex (identical layout, different type).
        self.extruder.mesh_vertices = loaded
            .vertices
            .iter()
            .map(|v| BlockVertex {
                position: v.position,
                normal: v.normal,
                color: v.color,
            })
            .collect();
        self.extruder.mesh_indices = loaded.indices;

        self.save_dialog.name = loaded.metadata.name.clone();
        self.save_dialog.category = loaded.metadata.category.clone();
        self.save_dialog.tags = loaded.metadata.tags.join(", ");

        let msg = format!("Loaded: {}", path.display());
        println!("{msg}");
        self.save_dialog.status_message = msg;
        Ok(())
    }

    /// Collect the current mesh vertices for saving.
    ///
    /// Converts from the extruder's `BlockVertex` to the file format's `Vertex`.
    /// Both types have identical `#[repr(C)]` layout (position + normal + color = 40 bytes).
    fn collect_vertices(&self) -> Vec<Vertex> {
        self.extruder
            .mesh_vertices
            .iter()
            .map(|bv| Vertex {
                position: bv.position,
                normal: bv.normal,
                color: bv.color,
            })
            .collect()
    }

    /// Collect the current mesh indices for saving.
    fn collect_indices(&self) -> Vec<u32> {
        self.extruder.mesh_indices.clone()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_stage_from_key() {
        assert_eq!(EditorStage::from_key(1), Some(EditorStage::Draw2D));
        assert_eq!(EditorStage::from_key(2), Some(EditorStage::Extrude));
        assert_eq!(EditorStage::from_key(3), Some(EditorStage::Sculpt));
        assert_eq!(EditorStage::from_key(4), Some(EditorStage::Color));
        assert_eq!(EditorStage::from_key(5), Some(EditorStage::Save));
        assert_eq!(EditorStage::from_key(0), None);
        assert_eq!(EditorStage::from_key(6), None);
    }

    #[test]
    fn test_editor_new() {
        let editor = AssetEditor::new();
        assert_eq!(editor.stage, EditorStage::Draw2D);
        assert!(editor.active);
        assert_eq!(editor.draft.name, "Untitled");
    }

    #[test]
    fn test_set_stage() {
        let mut editor = AssetEditor::new();
        editor.set_stage(EditorStage::Sculpt);
        assert_eq!(editor.stage, EditorStage::Sculpt);
    }

    #[test]
    fn test_set_stage_by_key() {
        let mut editor = AssetEditor::new();
        assert!(editor.set_stage_by_key(3));
        assert_eq!(editor.stage, EditorStage::Sculpt);
        // Same stage should return false
        assert!(!editor.set_stage_by_key(3));
    }

    #[test]
    fn test_window_title() {
        let editor = AssetEditor::new();
        let title = editor.window_title();
        assert!(title.contains("Asset Editor"));
        assert!(title.contains("Draw 2D"));
    }

    #[test]
    fn test_reset() {
        let mut editor = AssetEditor::new();
        editor.set_stage(EditorStage::Color);
        editor.draft.name = "My Tree".to_string();
        editor.reset();
        assert_eq!(editor.stage, EditorStage::Draw2D);
        assert_eq!(editor.draft.name, "Untitled");
    }
}
