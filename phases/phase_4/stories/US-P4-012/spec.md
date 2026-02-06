# US-P4-012: Asset Library Panel

## Description
Create `src/game/asset_editor/library.rs` with a browsable, filterable grid of saved `.btasset` files. The library panel toggles with F10 in the editor, reads/writes `assets/world/library.json` for the index, displays assets in a 4-column grid (each cell 120x120 pixels) with category tabs across the top, and supports text search by name and tags. All UI is rendered using existing `add_quad()` and `draw_text()` primitives from `src/game/ui/text.rs`. Click to select an asset, then place it in the world via the placement system (US-P4-014). The editor is a separate binary (`cargo run --bin battle_editor`). `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
Once artists save assets via the `.btasset` format (US-P4-011), they need a way to browse and manage their collection. Without a library, artists must remember filenames and navigate the filesystem manually. The Asset Library provides an in-editor panel that catalogs all saved assets, supports category-based filtering (Tree, Grass, Rock, Structure, Prop, Decoration), and text search. This is the bridge between asset creation (Stages 1-5) and world population (US-P4-014 placement system). The library index (`library.json`) also serves as a manifest for the game engine to discover available assets at runtime.

The panel follows a familiar grid layout pattern seen in professional content creation tools -- thumbnails in a grid, filterable by category tabs and a search bar. The 120x120 pixel cells provide enough room to display the asset name and basic metadata while fitting 4 columns in a reasonable panel width.

## Goal
Create `src/game/asset_editor/library.rs` with `AssetLibrary` and `AssetEntry` structs providing a browsable, searchable, category-filtered grid panel for saved `.btasset` assets. Wire F10 toggle in `mod.rs`.

## Files to Create/Modify
- **Create** `src/game/asset_editor/library.rs` -- `AssetLibrary`, `AssetEntry`, grid rendering, search/filter, click handling, library.json I/O
- **Modify** `src/game/asset_editor/mod.rs` -- Add `pub mod library;`, add `library: AssetLibrary` field to `AssetEditor`, wire F10 toggle, connect save system to auto-add entries

## Implementation Steps

1. Define the `AssetEntry` struct to represent one saved asset in the library index:
   ```rust
   use crate::game::asset_editor::AssetCategory;
   use glam::Vec3;
   use serde::{Deserialize, Serialize};

   #[derive(Clone, Debug, Serialize, Deserialize)]
   pub struct AssetEntry {
       pub id: String,            // Unique identifier (e.g., UUID or slugified name)
       pub name: String,          // Human-readable display name
       pub category: AssetCategory,
       pub tags: Vec<String>,     // Searchable tags (e.g., "oak", "deciduous", "tall")
       pub path: String,          // File path relative to assets/world/
       pub vertex_count: u32,     // For metadata display
       pub bounds_size: Vec3,     // Bounding box dimensions for scale info
   }
   ```

2. Define the `AssetLibrary` struct to hold the full library state:
   ```rust
   pub struct AssetLibrary {
       pub entries: Vec<AssetEntry>,
       pub visible: bool,                          // Toggled by F10
       pub selected: Option<usize>,                // Index into filtered list
       pub filter_category: Option<AssetCategory>, // None = show all
       pub search_text: String,                    // Live search query
       pub scroll_offset: f32,                     // Vertical scroll for long lists
   }
   ```

3. Implement library JSON I/O for persistent index:
   ```rust
   const LIBRARY_PATH: &str = "assets/world/library.json";

   impl AssetLibrary {
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

       pub fn save(&self) -> Result<(), std::io::Error> {
           let json = serde_json::to_string_pretty(&self.entries)?;
           std::fs::create_dir_all("assets/world")?;
           std::fs::write(LIBRARY_PATH, json)?;
           Ok(())
       }

       pub fn add_entry(&mut self, entry: AssetEntry) {
           self.entries.retain(|e| e.path != entry.path);
           self.entries.push(entry);
           let _ = self.save();
       }

       pub fn remove_entry(&mut self, path: &str) {
           self.entries.retain(|e| e.path != path);
           let _ = self.save();
       }
   }
   ```

4. Implement filtering and search logic:
   ```rust
   impl AssetLibrary {
       pub fn filtered_entries(&self) -> Vec<&AssetEntry> {
           self.entries.iter().filter(|e| {
               if let Some(cat) = &self.filter_category {
                   if e.category != *cat { return false; }
               }
               if !self.search_text.is_empty() {
                   let q = self.search_text.to_lowercase();
                   let name_match = e.name.to_lowercase().contains(&q);
                   let tag_match = e.tags.iter().any(|t| t.to_lowercase().contains(&q));
                   if !name_match && !tag_match { return false; }
               }
               true
           }).collect()
       }
   }
   ```

5. Implement the grid panel rendering using `add_quad()` and `draw_text()`:
   ```rust
   use crate::game::ui::text::{add_quad, draw_text};

   impl AssetLibrary {
       pub fn generate_panel(&self, screen_w: f32, screen_h: f32) -> (Vec<Vertex>, Vec<u32>) {
           let mut verts = Vec::new();
           let mut idxs = Vec::new();
           if !self.visible { return (verts, idxs); }

           let panel_width = 520.0;
           let panel_left = screen_w - panel_width;

           // Semi-transparent dark background
           add_quad(&mut verts, &mut idxs,
               [panel_left, 0.0, 0.0], [screen_w, 0.0, 0.0],
               [screen_w, screen_h, 0.0], [panel_left, screen_h, 0.0],
               [0.08, 0.08, 0.12, 0.92]);

           // Title
           draw_text(&mut verts, &mut idxs, "Asset Library",
               panel_left + 12.0, 12.0, 0.5, [1.0, 1.0, 1.0, 1.0]);

           // Category tabs: "All", Tree, Grass, Rock, Structure, Prop, Decoration
           let tab_y = 40.0;
           let tab_w = 70.0;
           // ... render each tab with active highlight ...

           // Search bar
           let search_y = tab_y + 32.0;
           draw_text(&mut verts, &mut idxs,
               &format!("Search: {}_", self.search_text),
               panel_left + 12.0, search_y, 0.35, [0.7, 0.7, 0.7, 1.0]);

           // Grid: 4 columns, each cell 120x120 pixels, 8px padding
           let grid_top = search_y + 24.0;
           let cols = 4;
           let cell_size = 120.0;
           let padding = 8.0;
           let filtered = self.filtered_entries();

           for (i, entry) in filtered.iter().enumerate() {
               let col = i % cols;
               let row = i / cols;
               let x = panel_left + padding + col as f32 * (cell_size + padding);
               let y = grid_top + row as f32 * (cell_size + padding) - self.scroll_offset;

               if y + cell_size < 0.0 || y > screen_h { continue; }

               let is_selected = self.selected == Some(i);
               let bg = if is_selected { [0.3, 0.45, 0.6, 0.9] } else { [0.14, 0.14, 0.18, 0.8] };
               add_quad(&mut verts, &mut idxs,
                   [x, y, 0.0], [x + cell_size, y, 0.0],
                   [x + cell_size, y + cell_size, 0.0], [x, y + cell_size, 0.0], bg);

               // Asset name, category label, vertex count
               draw_text(&mut verts, &mut idxs, &entry.name,
                   x + 4.0, y + cell_size - 18.0, 0.3, [0.9, 0.9, 0.9, 1.0]);
               draw_text(&mut verts, &mut idxs, &format!("{:?}", entry.category),
                   x + 4.0, y + 4.0, 0.25, [0.5, 0.6, 0.7, 0.8]);
               draw_text(&mut verts, &mut idxs, &format!("{}v", entry.vertex_count),
                   x + 4.0, y + cell_size - 32.0, 0.25, [0.5, 0.5, 0.5, 0.7]);
           }

           (verts, idxs)
       }
   }
   ```

6. Implement click handling for grid cells and category tabs:
   ```rust
   pub enum LibraryAction {
       SelectAsset(usize),
       ChangeCategory(AssetCategory),
       ClearCategory,
   }

   impl AssetLibrary {
       pub fn handle_click(&mut self, x: f32, y: f32, screen_w: f32) -> Option<LibraryAction> {
           if !self.visible { return None; }
           let panel_left = screen_w - 520.0;
           if x < panel_left { return None; }

           // Check category tabs (y = 40..64)
           if y >= 40.0 && y < 64.0 {
               // Map x to tab index, return ChangeCategory or ClearCategory
           }

           // Check grid cells
           let grid_top = 96.0;
           let cell_size = 120.0;
           let padding = 8.0;
           let cols = 4;
           let grid_x = x - panel_left - padding;
           let grid_y = y - grid_top + self.scroll_offset;
           if grid_x >= 0.0 && grid_y >= 0.0 {
               let col = (grid_x / (cell_size + padding)) as usize;
               let row = (grid_y / (cell_size + padding)) as usize;
               if col < cols {
                   let index = row * cols + col;
                   let filtered = self.filtered_entries();
                   if index < filtered.len() {
                       self.selected = Some(index);
                       return Some(LibraryAction::SelectAsset(index));
                   }
               }
           }
           None
       }

       pub fn handle_scroll(&mut self, delta: f32) {
           if !self.visible { return; }
           self.scroll_offset = (self.scroll_offset - delta * 30.0).max(0.0);
       }
   }
   ```

7. Wire F10 toggle in `mod.rs` and connect save system:
   ```rust
   // In AssetEditor keyboard handling:
   if key == VirtualKeyCode::F10 {
       editor.library.visible = !editor.library.visible;
   }

   // After save_btasset() completes in stage 5:
   editor.library.add_entry(AssetEntry { /* ... */ });
   ```

## Code Patterns
Grid rendering follows the existing pattern -- generate vertex/index data using `add_quad()` and `draw_text()`, upload to GPU, render in UI pass:
```rust
for (i, entry) in filtered.iter().enumerate() {
    let col = i % cols;
    let row = i / cols;
    let x = panel_left + padding + col as f32 * (cell_size + padding);
    let y = grid_top + row as f32 * (cell_size + padding) - scroll_offset;
    add_quad(&mut verts, &mut idxs, /* cell quad */, bg_color);
    draw_text(&mut verts, &mut idxs, &entry.name, x + 4.0, y + cell_size - 18.0, 0.3, [0.9; 4]);
}
```

Library JSON I/O uses standard serde:
```rust
let entries: Vec<AssetEntry> = serde_json::from_str(&data)?;
let json = serde_json::to_string_pretty(&entries)?;
std::fs::write(LIBRARY_PATH, json)?;
```

## Acceptance Criteria
- [ ] `library.rs` exists with `AssetLibrary` and `AssetEntry` structs
- [ ] `AssetEntry` has fields: `id`, `name`, `category`, `tags`, `path`, `vertex_count`, `bounds_size`
- [ ] Library reads from and writes to `assets/world/library.json` persistently
- [ ] F10 key toggles library panel visibility
- [ ] Panel displays saved assets in a 4-column grid layout with 120x120 pixel cells
- [ ] Category tabs across the top filter displayed assets by `AssetCategory`
- [ ] Text search filters by name and tags (case-insensitive)
- [ ] Click selects an asset entry with visual highlight
- [ ] Scroll navigates long lists of assets
- [ ] New saves automatically add entries to the library via `add_entry()`
- [ ] All UI rendering uses `add_quad()` and `draw_text()` primitives
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check` passes with 0 errors

## Verification Commands
- `cmd`: `test -f /home/hungaromakker/battle_tok/src/game/asset_editor/library.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `library.rs module exists`
- `cmd`: `grep -c 'AssetLibrary\|AssetEntry\|LibraryAction' /home/hungaromakker/battle_tok/src/game/asset_editor/library.rs`
  `expect_gt`: 0
  `description`: `Library types defined`
- `cmd`: `grep -c 'library.json\|filtered_entries\|generate_panel' /home/hungaromakker/battle_tok/src/game/asset_editor/library.rs`
  `expect_gt`: 0
  `description`: `Library I/O and rendering functions exist`
- `cmd`: `grep -c 'pub mod library' /home/hungaromakker/battle_tok/src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `library module registered in mod.rs`
- `cmd`: `cd /home/hungaromakker/battle_tok && cargo check 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `Project compiles`

## Success Looks Like
The artist presses F10 and a library panel slides in from the right side of the editor window. It shows all saved assets in a 4-column grid. Each cell is 120x120 pixels, showing the asset name, category label, and vertex count. They click the "Rock" tab and see only rock assets. They type "granite" in the search bar and the grid filters to matching assets. They click an asset cell and it highlights in blue, ready for placement. When they save a new asset in Stage 5, it immediately appears in the library. Scrolling navigates through large collections. The panel feels like a built-in file browser tailored for game assets.

## Dependencies
- Depends on: US-P4-011 (needs save/load to populate library with entries)

## Complexity
- Complexity: normal
- Min iterations: 1
