//! Asset Editor Module
//!
//! Contains the core types and logic for the standalone asset editor binary.
//! The editor allows creating game assets through a multi-stage pipeline:
//! Draw2D -> Extrude -> Sculpt -> Color -> Save.

use glam::Vec3;

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

/// The main asset editor state.
/// Manages the editing pipeline and current draft asset.
pub struct AssetEditor {
    /// Current editing stage
    pub stage: EditorStage,
    /// The asset currently being edited
    pub draft: AssetDraft,
    /// Whether the editor is actively editing (vs idle/menu)
    pub active: bool,
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
        }
    }

    /// Switch to a different editor stage.
    pub fn set_stage(&mut self, stage: EditorStage) {
        println!("Editor stage: {} -> {}", self.stage, stage);
        self.stage = stage;
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
    /// Currently a stub -- rendering is handled by the binary's wgpu loop.
    pub fn render(&self) {
        // Stub: will be filled in by subsequent stories
        // The binary handles actual wgpu rendering; this will provide
        // render data (meshes, UI elements) to the binary.
    }

    /// Get the window title string showing the current stage.
    pub fn window_title(&self) -> String {
        format!(
            "Battle T\u{00f6}k \u{2014} Asset Editor [{}/5: {}]",
            self.stage.number(),
            self.stage.name()
        )
    }

    /// Reset the editor to start a new asset.
    pub fn reset(&mut self) {
        self.draft = AssetDraft::default();
        self.stage = EditorStage::Draw2D;
        self.active = true;
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
