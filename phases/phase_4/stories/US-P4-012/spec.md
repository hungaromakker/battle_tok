# US-P4-012: Asset Library Panel

## Description
Create `src/game/asset_editor/library.rs` with an `AssetLibrary` struct that provides a browsable, filterable overlay panel for managing saved `.btasset` files. The panel displays assets in a grid layout with category tabs and search filtering. It reads and writes `assets/world/library.json` as a persistent index. The panel is toggled with F10 and uses the existing `add_quad()` and `draw_text()` UI primitives. The asset editor is a **separate binary** (`battle_editor`); `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
As artists create dozens of assets (trees, rocks, grass, structures), they need a way to browse, find, and select them without navigating the filesystem. The library panel acts as a visual catalog — organized by category, searchable by name and tags, displaying key stats like vertex count and bounding box size. When an asset is selected from the library, it can be loaded into the editor for modification or chosen for world placement. This closes the loop between creation and reuse, making the asset pipeline self-contained within the editor.

## Goal
Create `src/game/asset_editor/library.rs` with an `AssetLibrary` struct providing a grid-based overlay panel with category tabs, search filtering, and asset selection, persisted to `assets/world/library.json`.

## Files to Create/Modify
- **Create** `src/game/asset_editor/library.rs` — `AssetLibrary`, `LibraryEntry`, `LibraryAction` types, panel rendering and interaction logic
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod library;`, add `library: AssetLibrary` field to `AssetEditor`, route F10 toggle and panel input

## Implementation Steps
1. Define `LibraryEntry` struct:
   ```rust
   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct LibraryEntry {
       pub id: String,              // unique identifier
       pub name: String,            // display name
       pub category: String,        // tree, rock, grass, structure, prop, decoration
       pub tags: Vec<String>,       // searchable tags
       pub path: String,            // relative path to .btasset file
       pub vertex_count: u32,       // for display
       pub bounds_size: [f32; 3],   // bounding box dimensions
   }
   ```
2. Define `AssetLibrary` struct:
   - `entries: Vec<LibraryEntry>` — all known assets
   - `visible: bool` — panel open/closed
   - `selected_category: Option<String>` — active filter tab
   - `search_query: String` — text search filter
   - `selected_index: Option<usize>` — highlighted entry in filtered list
   - `scroll_offset: f32` — for scrolling long lists
3. Define `LibraryAction` enum: `Select(usize)`, `LoadForEdit(String)`, `Delete(String)`, `Close`
4. Implement `load_library(path) -> Vec<LibraryEntry>`:
   - Read `assets/world/library.json`
   - Deserialize with `serde_json`
   - If file doesn't exist, return empty Vec
5. Implement `save_library(path, entries)`:
   - Serialize entries to pretty JSON
   - Write to `assets/world/library.json`
6. Implement `add_entry(&mut self, entry: LibraryEntry)`:
   - Push to `entries`, save library
7. Implement `filtered_entries(&self) -> Vec<&LibraryEntry>`:
   - Filter by `selected_category` if set
   - Filter by `search_query` matching name or tags (case-insensitive)
8. Implement panel rendering using `add_quad()` and `draw_text()`:
   - Panel background: semi-transparent dark overlay on right side of screen
   - Category tabs at top: "All", "Trees", "Rocks", "Grass", "Structures", "Props", "Decorations"
   - Grid layout: 4 columns, 120x120px cells
   - Each cell shows: asset name (truncated), vertex count, category icon placeholder
   - Selected cell has highlighted border
9. Implement click handling:
   - Map mouse position to grid cell index
   - Map clicks on category tabs to filter changes
   - Click on asset selects it for placement or loading
10. Wire into `mod.rs`: F10 toggles `library.visible`, panel renders as overlay on top of current stage. Auto-add entry when `save_btasset()` completes.

## Code Patterns
```rust
use crate::game::ui::text::{add_quad, draw_text};

pub struct AssetLibrary {
    pub entries: Vec<LibraryEntry>,
    pub visible: bool,
    pub selected_category: Option<String>,
    pub search_query: String,
    pub selected_index: Option<usize>,
    scroll_offset: f32,
}

impl AssetLibrary {
    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn filtered_entries(&self) -> Vec<&LibraryEntry> {
        self.entries.iter().filter(|e| {
            let cat_match = self.selected_category.as_ref()
                .map_or(true, |c| &e.category == c);
            let search_match = self.search_query.is_empty()
                || e.name.to_lowercase().contains(&self.search_query.to_lowercase())
                || e.tags.iter().any(|t| t.to_lowercase().contains(&self.search_query.to_lowercase()));
            cat_match && search_match
        }).collect()
    }

    pub fn render_panel(&self, quads: &mut Vec<QuadInstance>, texts: &mut Vec<TextEntry>) {
        if !self.visible { return; }
        // Panel background
        add_quad(quads, panel_x, panel_y, panel_w, panel_h, [0.1, 0.1, 0.12, 0.95]);
        // Category tabs, grid cells, labels...
    }
}
```

## Acceptance Criteria
- [ ] `library.rs` exists with `AssetLibrary`, `LibraryEntry`, `LibraryAction` types
- [ ] Library reads/writes `assets/world/library.json` for persistent index
- [ ] F10 key toggles library panel visibility
- [ ] Category filtering shows only assets matching selected category
- [ ] Search filters by name and tags (case-insensitive)
- [ ] Grid layout renders 4 columns with 120x120 cells showing asset names
- [ ] Selected asset is visually highlighted
- [ ] Click handling identifies which grid cell or tab was clicked
- [ ] New saves automatically add entries to the library
- [ ] All UI rendering uses `add_quad()` and `draw_text()`
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo check --bin battle_arena` passes with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with library module`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles unchanged`
- `cmd`: `test -f src/game/asset_editor/library.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `library.rs file exists`
- `cmd`: `grep -c 'AssetLibrary\|LibraryEntry\|LibraryAction' src/game/asset_editor/library.rs`
  `expect_gt`: 0
  `description`: `Core library types are defined`
- `cmd`: `grep -c 'library.json\|filtered_entries\|render_panel' src/game/asset_editor/library.rs`
  `expect_gt`: 0
  `description`: `Library functions are implemented`
- `cmd`: `grep -c 'pub mod library' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `library module registered in mod.rs`

## Success Looks Like
The artist presses F10 and a library panel slides in from the right. It shows all saved assets in a grid with names and vertex counts. They click the "Trees" tab and see only tree assets. They type "oak" in the search and the grid filters to matching assets. They click an asset and it highlights, ready for placement or editing. When they save a new asset, it immediately appears in the library. The panel feels like a built-in file browser tailored for game assets.

## Dependencies
- Depends on: US-P4-011 (needs save/load to populate library with entries)

## Complexity
- Complexity: normal
- Min iterations: 1
