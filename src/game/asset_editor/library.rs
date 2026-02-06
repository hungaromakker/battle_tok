//! Asset Library Panel
//!
//! Browsable, filterable grid of saved `.btasset` files.
//! Toggled with F10 in the editor. Reads/writes `assets/world/library.json`
//! for the persistent index. Displays assets in a 4-column grid (120x120 cells)
//! with category tabs and text search.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game::asset_editor::AssetCategory;
use crate::game::types::Vertex;
use crate::game::ui::text::{add_quad, draw_text};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Path to the persistent library index.
const LIBRARY_PATH: &str = "assets/world/library.json";

/// Number of columns in the grid.
const GRID_COLS: usize = 4;

/// Size of each grid cell in pixels.
const CELL_SIZE: f32 = 120.0;

/// Padding between cells in pixels.
const CELL_PADDING: f32 = 8.0;

/// Width of the library panel in pixels.
const PANEL_WIDTH: f32 = 520.0;

// ============================================================================
// ASSET ENTRY
// ============================================================================

/// A single saved asset in the library index.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetEntry {
    /// Unique identifier (slugified name).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Asset category.
    pub category: AssetCategory,
    /// Searchable tags (e.g., "oak", "deciduous", "tall").
    pub tags: Vec<String>,
    /// File path relative to assets/world/.
    pub path: String,
    /// Vertex count for metadata display.
    pub vertex_count: u32,
    /// Bounding box dimensions for scale info.
    pub bounds_size: Vec3,
}

// ============================================================================
// LIBRARY ACTION
// ============================================================================

/// Actions returned by click handling.
pub enum LibraryAction {
    /// User selected an asset at this index in the filtered list.
    SelectAsset(usize),
    /// User clicked a category tab.
    ChangeCategory(AssetCategory),
    /// User clicked the "All" tab.
    ClearCategory,
}

// ============================================================================
// ASSET LIBRARY
// ============================================================================

/// The asset library panel state.
pub struct AssetLibrary {
    /// All indexed asset entries.
    pub entries: Vec<AssetEntry>,
    /// Whether the panel is visible (toggled by F10).
    pub visible: bool,
    /// Currently selected index into the filtered list.
    pub selected: Option<usize>,
    /// Category filter. None = show all.
    pub filter_category: Option<AssetCategory>,
    /// Live search query.
    pub search_text: String,
    /// Vertical scroll offset for long lists.
    pub scroll_offset: f32,
}

impl AssetLibrary {
    /// Load the library from `assets/world/library.json`, or return empty.
    pub fn load() -> Self {
        let entries: Vec<AssetEntry> = if let Ok(data) = std::fs::read_to_string(LIBRARY_PATH) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };
        Self {
            entries,
            visible: false,
            selected: None,
            filter_category: None,
            search_text: String::new(),
            scroll_offset: 0.0,
        }
    }

    /// Persist the library index to `assets/world/library.json`.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let json = serde_json::to_string_pretty(&self.entries)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::create_dir_all("assets/world")?;
        std::fs::write(LIBRARY_PATH, json)?;
        Ok(())
    }

    /// Add or update an entry (deduplicates by path).
    pub fn add_entry(&mut self, entry: AssetEntry) {
        self.entries.retain(|e| e.path != entry.path);
        self.entries.push(entry);
        let _ = self.save();
    }

    /// Remove an entry by path.
    pub fn remove_entry(&mut self, path: &str) {
        self.entries.retain(|e| e.path != path);
        let _ = self.save();
    }

    // ========================================================================
    // FILTERING
    // ========================================================================

    /// Return entries matching the current category filter and search text.
    pub fn filtered_entries(&self) -> Vec<&AssetEntry> {
        self.entries
            .iter()
            .filter(|e| {
                if let Some(cat) = &self.filter_category {
                    if e.category != *cat {
                        return false;
                    }
                }
                if !self.search_text.is_empty() {
                    let q = self.search_text.to_lowercase();
                    let name_match = e.name.to_lowercase().contains(&q);
                    let tag_match = e.tags.iter().any(|t| t.to_lowercase().contains(&q));
                    if !name_match && !tag_match {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    // ========================================================================
    // RENDERING
    // ========================================================================

    /// All category variants for tab rendering.
    const ALL_CATEGORIES: [AssetCategory; 6] = [
        AssetCategory::Tree,
        AssetCategory::Grass,
        AssetCategory::Rock,
        AssetCategory::Structure,
        AssetCategory::Prop,
        AssetCategory::Decoration,
    ];

    /// Generate the library panel overlay mesh.
    ///
    /// Uses `add_quad()` and `draw_text()` from the UI text module.
    pub fn generate_panel(&self, screen_w: f32, screen_h: f32) -> (Vec<Vertex>, Vec<u32>) {
        let mut verts = Vec::new();
        let mut idxs = Vec::new();

        if !self.visible {
            return (verts, idxs);
        }

        let panel_left = screen_w - PANEL_WIDTH;

        let to_ndc = |px: f32, py: f32| -> [f32; 3] {
            [
                (px / screen_w) * 2.0 - 1.0,
                1.0 - (py / screen_h) * 2.0,
                0.0,
            ]
        };

        // Semi-transparent dark background
        add_quad(
            &mut verts,
            &mut idxs,
            to_ndc(panel_left, 0.0),
            to_ndc(screen_w, 0.0),
            to_ndc(screen_w, screen_h),
            to_ndc(panel_left, screen_h),
            [0.08, 0.08, 0.12, 0.92],
        );

        // Title
        draw_text(
            &mut verts,
            &mut idxs,
            "ASSET LIBRARY",
            panel_left + 12.0,
            12.0,
            2.5,
            [1.0, 0.9, 0.5, 1.0],
            screen_w,
            screen_h,
        );

        // ---- Category tabs ----
        let tab_y = 40.0;
        let tab_h = 24.0;
        let tab_w = 70.0;
        let tab_spacing = 2.0;

        // "All" tab
        let all_active = self.filter_category.is_none();
        let all_bg = if all_active {
            [0.3, 0.5, 0.8, 1.0]
        } else {
            [0.18, 0.18, 0.22, 0.9]
        };
        let all_x = panel_left + 8.0;
        add_quad(
            &mut verts,
            &mut idxs,
            to_ndc(all_x, tab_y),
            to_ndc(all_x + tab_w, tab_y),
            to_ndc(all_x + tab_w, tab_y + tab_h),
            to_ndc(all_x, tab_y + tab_h),
            all_bg,
        );
        draw_text(
            &mut verts,
            &mut idxs,
            "ALL",
            all_x + 4.0,
            tab_y + 4.0,
            2.0,
            [0.9, 0.9, 0.9, 1.0],
            screen_w,
            screen_h,
        );

        // Category tabs
        for (i, cat) in Self::ALL_CATEGORIES.iter().enumerate() {
            let tx = all_x + (i as f32 + 1.0) * (tab_w + tab_spacing);
            let active = self.filter_category == Some(*cat);
            let bg = if active {
                [0.3, 0.5, 0.8, 1.0]
            } else {
                [0.18, 0.18, 0.22, 0.9]
            };
            add_quad(
                &mut verts,
                &mut idxs,
                to_ndc(tx, tab_y),
                to_ndc(tx + tab_w, tab_y),
                to_ndc(tx + tab_w, tab_y + tab_h),
                to_ndc(tx, tab_y + tab_h),
                bg,
            );
            // Truncate long names for tabs
            let label = match cat {
                AssetCategory::Tree => "TREE",
                AssetCategory::Grass => "GRASS",
                AssetCategory::Rock => "ROCK",
                AssetCategory::Structure => "STRUCT",
                AssetCategory::Prop => "PROP",
                AssetCategory::Decoration => "DECOR",
            };
            draw_text(
                &mut verts,
                &mut idxs,
                label,
                tx + 4.0,
                tab_y + 4.0,
                2.0,
                [0.9, 0.9, 0.9, 1.0],
                screen_w,
                screen_h,
            );
        }

        // ---- Search bar ----
        let search_y = tab_y + tab_h + 8.0;
        let search_bar_left = panel_left + 8.0;
        let search_bar_right = screen_w - 8.0;
        let search_bar_h = 24.0;

        // Search bar background
        add_quad(
            &mut verts,
            &mut idxs,
            to_ndc(search_bar_left, search_y),
            to_ndc(search_bar_right, search_y),
            to_ndc(search_bar_right, search_y + search_bar_h),
            to_ndc(search_bar_left, search_y + search_bar_h),
            [0.14, 0.14, 0.18, 0.9],
        );

        let search_display = format!("SEARCH: {}_", self.search_text);
        draw_text(
            &mut verts,
            &mut idxs,
            &search_display,
            search_bar_left + 4.0,
            search_y + 4.0,
            2.0,
            [0.7, 0.7, 0.7, 1.0],
            screen_w,
            screen_h,
        );

        // ---- Grid ----
        let grid_top = search_y + search_bar_h + 8.0;
        let filtered = self.filtered_entries();

        if filtered.is_empty() {
            draw_text(
                &mut verts,
                &mut idxs,
                "NO ASSETS FOUND",
                panel_left + 12.0,
                grid_top + 20.0,
                2.0,
                [0.5, 0.5, 0.5, 0.8],
                screen_w,
                screen_h,
            );
        }

        for (i, entry) in filtered.iter().enumerate() {
            let col = i % GRID_COLS;
            let row = i / GRID_COLS;
            let x = panel_left + CELL_PADDING + col as f32 * (CELL_SIZE + CELL_PADDING);
            let y = grid_top + row as f32 * (CELL_SIZE + CELL_PADDING) - self.scroll_offset;

            // Skip cells outside the visible area
            if y + CELL_SIZE < 0.0 || y > screen_h {
                continue;
            }

            let is_selected = self.selected == Some(i);
            let bg = if is_selected {
                [0.3, 0.45, 0.6, 0.9]
            } else {
                [0.14, 0.14, 0.18, 0.8]
            };

            // Cell background
            add_quad(
                &mut verts,
                &mut idxs,
                to_ndc(x, y),
                to_ndc(x + CELL_SIZE, y),
                to_ndc(x + CELL_SIZE, y + CELL_SIZE),
                to_ndc(x, y + CELL_SIZE),
                bg,
            );

            // Category label (top-left)
            draw_text(
                &mut verts,
                &mut idxs,
                &format!("{:?}", entry.category),
                x + 4.0,
                y + 4.0,
                1.5,
                [0.5, 0.6, 0.7, 0.8],
                screen_w,
                screen_h,
            );

            // Vertex count (middle)
            draw_text(
                &mut verts,
                &mut idxs,
                &format!("{}V", entry.vertex_count),
                x + 4.0,
                y + CELL_SIZE - 36.0,
                1.5,
                [0.5, 0.5, 0.5, 0.7],
                screen_w,
                screen_h,
            );

            // Asset name (bottom)
            draw_text(
                &mut verts,
                &mut idxs,
                &entry.name,
                x + 4.0,
                y + CELL_SIZE - 20.0,
                2.0,
                [0.9, 0.9, 0.9, 1.0],
                screen_w,
                screen_h,
            );
        }

        (verts, idxs)
    }

    // ========================================================================
    // INPUT HANDLING
    // ========================================================================

    /// Handle a mouse click. Returns a `LibraryAction` if the click was consumed.
    pub fn handle_click(
        &mut self,
        x: f32,
        y: f32,
        screen_w: f32,
        _screen_h: f32,
    ) -> Option<LibraryAction> {
        if !self.visible {
            return None;
        }

        let panel_left = screen_w - PANEL_WIDTH;
        if x < panel_left {
            return None;
        }

        // ---- Category tabs (y = 40..64) ----
        let tab_y = 40.0;
        let tab_h = 24.0;
        let tab_w = 70.0;
        let tab_spacing = 2.0;

        if y >= tab_y && y < tab_y + tab_h {
            let all_x = panel_left + 8.0;

            // "All" tab
            if x >= all_x && x < all_x + tab_w {
                self.filter_category = None;
                self.selected = None;
                self.scroll_offset = 0.0;
                return Some(LibraryAction::ClearCategory);
            }

            // Category tabs
            for (i, cat) in Self::ALL_CATEGORIES.iter().enumerate() {
                let tx = all_x + (i as f32 + 1.0) * (tab_w + tab_spacing);
                if x >= tx && x < tx + tab_w {
                    self.filter_category = Some(*cat);
                    self.selected = None;
                    self.scroll_offset = 0.0;
                    return Some(LibraryAction::ChangeCategory(*cat));
                }
            }
        }

        // ---- Grid cells ----
        let search_y = tab_y + tab_h + 8.0;
        let search_bar_h = 24.0;
        let grid_top = search_y + search_bar_h + 8.0;

        let grid_x = x - panel_left - CELL_PADDING;
        let grid_y = y - grid_top + self.scroll_offset;

        if grid_x >= 0.0 && grid_y >= 0.0 {
            let col = (grid_x / (CELL_SIZE + CELL_PADDING)) as usize;
            let row = (grid_y / (CELL_SIZE + CELL_PADDING)) as usize;

            if col < GRID_COLS {
                let index = row * GRID_COLS + col;
                let filtered = self.filtered_entries();
                if index < filtered.len() {
                    self.selected = Some(index);
                    return Some(LibraryAction::SelectAsset(index));
                }
            }
        }

        // Click was inside the panel but didn't hit anything specific
        None
    }

    /// Handle scroll wheel input.
    pub fn handle_scroll(&mut self, delta: f32) {
        if !self.visible {
            return;
        }
        self.scroll_offset = (self.scroll_offset - delta * 30.0).max(0.0);
    }

    /// Handle a character input for the search bar.
    pub fn handle_char(&mut self, c: char) {
        if !self.visible {
            return;
        }
        if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
            self.search_text.push(c);
            self.selected = None;
            self.scroll_offset = 0.0;
        }
    }

    /// Handle backspace in the search bar.
    pub fn handle_backspace(&mut self) {
        if !self.visible {
            return;
        }
        self.search_text.pop();
        self.selected = None;
        self.scroll_offset = 0.0;
    }

    /// Get the selected entry from the filtered list, if any.
    pub fn selected_entry(&self) -> Option<&AssetEntry> {
        let filtered = self.filtered_entries();
        self.selected.and_then(|i| filtered.get(i).copied())
    }

    /// Returns true if the panel is consuming mouse events (visible and mouse is inside).
    pub fn contains_point(&self, x: f32, screen_w: f32) -> bool {
        self.visible && x >= screen_w - PANEL_WIDTH
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(name: &str, category: AssetCategory) -> AssetEntry {
        AssetEntry {
            id: name.to_lowercase().replace(' ', "_"),
            name: name.to_string(),
            category,
            tags: vec!["test".to_string()],
            path: format!(
                "assets/world/{}.btasset",
                name.to_lowercase().replace(' ', "_")
            ),
            vertex_count: 100,
            bounds_size: Vec3::new(1.0, 2.0, 1.0),
        }
    }

    #[test]
    fn test_asset_library_add_entry() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: false,
            selected: None,
            filter_category: None,
            search_text: String::new(),
            scroll_offset: 0.0,
        };
        let entry = make_entry("Oak Tree", AssetCategory::Tree);
        lib.entries.push(entry);
        assert_eq!(lib.entries.len(), 1);
        assert_eq!(lib.entries[0].name, "Oak Tree");
    }

    #[test]
    fn test_filtered_entries_no_filter() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: false,
            selected: None,
            filter_category: None,
            search_text: String::new(),
            scroll_offset: 0.0,
        };
        lib.entries
            .push(make_entry("Oak Tree", AssetCategory::Tree));
        lib.entries
            .push(make_entry("Granite Rock", AssetCategory::Rock));
        assert_eq!(lib.filtered_entries().len(), 2);
    }

    #[test]
    fn test_filtered_entries_category() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: false,
            selected: None,
            filter_category: Some(AssetCategory::Rock),
            search_text: String::new(),
            scroll_offset: 0.0,
        };
        lib.entries
            .push(make_entry("Oak Tree", AssetCategory::Tree));
        lib.entries
            .push(make_entry("Granite Rock", AssetCategory::Rock));
        let filtered = lib.filtered_entries();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Granite Rock");
    }

    #[test]
    fn test_filtered_entries_search() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: false,
            selected: None,
            filter_category: None,
            search_text: "oak".to_string(),
            scroll_offset: 0.0,
        };
        lib.entries
            .push(make_entry("Oak Tree", AssetCategory::Tree));
        lib.entries
            .push(make_entry("Granite Rock", AssetCategory::Rock));
        let filtered = lib.filtered_entries();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "Oak Tree");
    }

    #[test]
    fn test_filtered_entries_search_by_tag() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: false,
            selected: None,
            filter_category: None,
            search_text: "test".to_string(),
            scroll_offset: 0.0,
        };
        lib.entries
            .push(make_entry("Oak Tree", AssetCategory::Tree));
        // All entries have "test" tag, so all should match
        assert_eq!(lib.filtered_entries().len(), 1);
    }

    #[test]
    fn test_generate_panel_hidden() {
        let lib = AssetLibrary {
            entries: Vec::new(),
            visible: false,
            selected: None,
            filter_category: None,
            search_text: String::new(),
            scroll_offset: 0.0,
        };
        let (verts, idxs) = lib.generate_panel(1280.0, 800.0);
        assert!(verts.is_empty());
        assert!(idxs.is_empty());
    }

    #[test]
    fn test_generate_panel_visible() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: true,
            selected: None,
            filter_category: None,
            search_text: String::new(),
            scroll_offset: 0.0,
        };
        lib.entries
            .push(make_entry("Oak Tree", AssetCategory::Tree));
        let (verts, idxs) = lib.generate_panel(1280.0, 800.0);
        assert!(!verts.is_empty());
        assert!(!idxs.is_empty());
    }

    #[test]
    fn test_handle_scroll() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: true,
            selected: None,
            filter_category: None,
            search_text: String::new(),
            scroll_offset: 100.0,
        };
        lib.handle_scroll(1.0);
        assert!(lib.scroll_offset < 100.0); // scrolled up
    }

    #[test]
    fn test_handle_char() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: true,
            selected: None,
            filter_category: None,
            search_text: String::new(),
            scroll_offset: 0.0,
        };
        lib.handle_char('o');
        lib.handle_char('a');
        lib.handle_char('k');
        assert_eq!(lib.search_text, "oak");
    }

    #[test]
    fn test_handle_backspace() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: true,
            selected: None,
            filter_category: None,
            search_text: "oak".to_string(),
            scroll_offset: 0.0,
        };
        lib.handle_backspace();
        assert_eq!(lib.search_text, "oa");
    }

    #[test]
    fn test_dedup_by_path() {
        let mut lib = AssetLibrary {
            entries: Vec::new(),
            visible: false,
            selected: None,
            filter_category: None,
            search_text: String::new(),
            scroll_offset: 0.0,
        };
        let e1 = make_entry("Oak Tree", AssetCategory::Tree);
        let path = e1.path.clone();
        lib.entries.push(e1);
        assert_eq!(lib.entries.len(), 1);

        // Adding with same path should replace
        let mut e2 = make_entry("Updated Oak", AssetCategory::Tree);
        e2.path = path;
        lib.add_entry(e2);
        assert_eq!(lib.entries.len(), 1);
        assert_eq!(lib.entries[0].name, "Updated Oak");
    }

    #[test]
    fn test_contains_point() {
        let lib = AssetLibrary {
            entries: Vec::new(),
            visible: true,
            selected: None,
            filter_category: None,
            search_text: String::new(),
            scroll_offset: 0.0,
        };
        // Panel occupies screen_w - 520 .. screen_w
        assert!(lib.contains_point(800.0, 1280.0)); // 800 >= 760
        assert!(!lib.contains_point(700.0, 1280.0)); // 700 < 760
    }
}
