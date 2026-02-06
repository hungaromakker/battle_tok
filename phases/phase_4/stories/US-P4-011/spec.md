# US-P4-011: Asset File Save/Load (.btasset)

## Description
Create `src/game/asset_editor/asset_file.rs` implementing the `.btasset` binary file format for persisting editor assets to disk. The format uses a fixed-size header with magic bytes, version, vertex/index counts, and offsets to metadata and variety parameter sections. Vertex and index data is written as raw bytes (via `bytemuck`), while metadata and variety parameters are appended as JSON. The asset editor is a **separate binary** (`battle_editor`); `battle_arena.rs` is never modified.

## The Core Concept / Why This Matters
A custom binary format is essential for fast, compact asset storage. Unlike generic formats (OBJ, glTF), `.btasset` stores exactly what the engine needs — vertex positions, normals, colors, indices, metadata, and variety parameters — in a single file with zero parsing overhead for the geometry data. The fixed header enables quick validation and random access to sections. JSON metadata sections keep human-readable info (name, category, tags) easily inspectable while the bulk geometry remains compact binary. Round-trip fidelity is critical: what you save must be exactly what you load.

## Goal
Create `src/game/asset_editor/asset_file.rs` with `save_btasset()` and `load_btasset()` functions implementing a compact binary format with lossless round-trip for mesh data, metadata, and variety parameters.

## Files to Create/Modify
- **Create** `src/game/asset_editor/asset_file.rs` — `BtassetHeader`, `AssetMetadata`, `LoadedAsset`, save/load functions
- **Modify** `src/game/asset_editor/mod.rs` — Add `pub mod asset_file;`, wire Stage 5 (Save) UI to call save/load functions

## Implementation Steps
1. Define `BtassetHeader` as a `#[repr(C)]` struct deriving `bytemuck::Pod` and `bytemuck::Zeroable`:
   ```rust
   #[repr(C)]
   #[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
   pub struct BtassetHeader {
       pub magic: [u8; 4],           // "BTAS"
       pub version: u32,             // 1
       pub vertex_count: u32,        // number of vertices
       pub index_count: u32,         // number of indices
       pub metadata_offset: u32,     // byte offset to JSON metadata
       pub variety_offset: u32,      // byte offset to variety JSON
       pub _reserved: [u8; 8],       // future use, zeroed
   }
   ```
   Total header size: exactly 32 bytes.
2. Define `AssetMetadata` struct with `serde::Serialize` + `Deserialize`:
   - `name: String`, `category: String`, `tags: Vec<String>`, `created_at: String`, `vertex_count: u32`, `index_count: u32`
3. Define `LoadedAsset` struct containing: `vertices: Vec<Vertex>`, `indices: Vec<u32>`, `metadata: AssetMetadata`, `variety_params: Option<VarietyParams>`
4. Implement `save_btasset(path, vertices, indices, metadata, variety_params) -> Result<(), AssetFileError>`:
   - Compute offsets: header (32 bytes) → vertex data (40 bytes × count) → index data (4 bytes × count) → metadata JSON → variety JSON
   - Write header with magic `b"BTAS"`, version 1, counts, offsets
   - Write vertex data as raw bytes via `bytemuck::cast_slice()`
   - Write index data as raw bytes via `bytemuck::cast_slice()`
   - Serialize metadata to JSON, write bytes
   - Serialize variety params to JSON (if present), write bytes
5. Implement `load_btasset(path) -> Result<LoadedAsset, AssetFileError>`:
   - Read entire file into buffer
   - Validate minimum size (>= 32 bytes), check magic == `b"BTAS"`, check version == 1
   - Read header via `bytemuck::from_bytes()`
   - Extract vertex slice and cast via `bytemuck::cast_slice()`
   - Extract index slice and cast via `bytemuck::cast_slice()`
   - Parse metadata JSON from `metadata_offset` to `variety_offset`
   - Parse variety JSON from `variety_offset` to end of file (if present)
6. Define `AssetFileError` enum covering: `InvalidMagic`, `UnsupportedVersion`, `FileTooShort`, `IoError(std::io::Error)`, `JsonError(serde_json::Error)`
7. File location convention: `assets/world/<category>/<name>.btasset` (e.g., `assets/world/tree/oak_tree.btasset`)
8. Wire Stage 5 UI in `mod.rs`: when in Stage 5, show save dialog fields (name, category, tags). On save, call `save_btasset()` with current mesh data. On load, call `load_btasset()` and replace current editor mesh.

## Code Patterns
```rust
use bytemuck;
use serde::{Serialize, Deserialize};
use std::io::Write;
use std::fs;

pub fn save_btasset(
    path: &std::path::Path,
    vertices: &[Vertex],
    indices: &[u32],
    metadata: &AssetMetadata,
    variety_params: Option<&VarietyParams>,
) -> Result<(), AssetFileError> {
    let vertex_bytes = bytemuck::cast_slice::<Vertex, u8>(vertices);
    let index_bytes = bytemuck::cast_slice::<u32, u8>(indices);
    let metadata_json = serde_json::to_vec(metadata)?;
    let variety_json = variety_params.map(|v| serde_json::to_vec(v)).transpose()?;

    let metadata_offset = 32 + vertex_bytes.len() as u32 + index_bytes.len() as u32;
    let variety_offset = metadata_offset + metadata_json.len() as u32;

    let header = BtassetHeader {
        magic: *b"BTAS",
        version: 1,
        vertex_count: vertices.len() as u32,
        index_count: indices.len() as u32,
        metadata_offset,
        variety_offset,
        _reserved: [0u8; 8],
    };

    let mut file = fs::File::create(path)?;
    file.write_all(bytemuck::bytes_of(&header))?;
    file.write_all(vertex_bytes)?;
    file.write_all(index_bytes)?;
    file.write_all(&metadata_json)?;
    if let Some(vj) = variety_json {
        file.write_all(&vj)?;
    }
    Ok(())
}
```

## Acceptance Criteria
- [ ] `asset_file.rs` exists with `BtassetHeader`, `AssetMetadata`, `LoadedAsset` types
- [ ] `BtassetHeader` is exactly 32 bytes with `b"BTAS"` magic, `#[repr(C)]`, and derives `bytemuck::Pod`
- [ ] `save_btasset()` writes header + vertex data + index data + metadata JSON + variety JSON
- [ ] `load_btasset()` reads and parses all sections correctly
- [ ] Vertex data is 40 bytes per vertex: position `[f32;3]` + normal `[f32;3]` + color `[f32;4]`
- [ ] Index data is `u32` array written as raw bytes
- [ ] Round-trip is lossless: save then load produces identical vertex, index, metadata, and variety data
- [ ] Error handling covers: file too short, invalid magic, unsupported version, IO errors, JSON parse errors
- [ ] Stage 5 UI shows save dialog with name, category, and tags fields
- [ ] `battle_arena.rs` is NOT modified
- [ ] `cargo check --bin battle_editor` passes with 0 errors
- [ ] `cargo check --bin battle_arena` passes with 0 errors

## Verification Commands
- `cmd`: `cargo check --bin battle_editor 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_editor binary compiles with asset_file module`
- `cmd`: `cargo check --bin battle_arena 2>&1; echo EXIT:$?`
  `expect_contains`: `EXIT:0`
  `description`: `battle_arena still compiles unchanged`
- `cmd`: `test -f src/game/asset_editor/asset_file.rs && echo EXISTS`
  `expect_contains`: `EXISTS`
  `description`: `asset_file.rs file exists`
- `cmd`: `grep -c 'BtassetHeader\|save_btasset\|load_btasset\|AssetMetadata' src/game/asset_editor/asset_file.rs`
  `expect_gt`: 0
  `description`: `Core asset file types and functions exist`
- `cmd`: `grep -c 'BTAS\|magic\|vertex_count\|metadata_offset' src/game/asset_editor/asset_file.rs`
  `expect_gt`: 0
  `description`: `Header fields are defined`
- `cmd`: `grep -c 'pub mod asset_file' src/game/asset_editor/mod.rs`
  `expect_gt`: 0
  `description`: `asset_file module registered in mod.rs`

## Success Looks Like
The artist finishes painting a tree asset and switches to Stage 5 (Save). They type "Oak Tree" as the name, select "tree" category, add tags "deciduous" and "forest". They click Save and a file `assets/world/tree/oak_tree.btasset` appears on disk. They close the editor, reopen it, click Load, select the file, and the exact same tree appears — every vertex position, normal, and color is identical. The file is compact (a 500-vertex tree is about 20KB + metadata). Loading is instantaneous.

## Dependencies
- Depends on: US-P4-006 (needs a complete mesh with vertex data to save)

## Complexity
- Complexity: normal
- Min iterations: 1
