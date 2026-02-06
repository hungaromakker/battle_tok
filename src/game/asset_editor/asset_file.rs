//! Asset File Save/Load (.btasset)
//!
//! Binary file format for persisting editor assets to disk.
//! Layout: fixed 32-byte header | raw vertex data | raw index data | metadata JSON | variety JSON.
//!
//! The header contains magic bytes, version, counts, and byte offsets so each
//! section can be read independently. Geometry is written as raw bytes for
//! zero-overhead round-trip fidelity. Metadata and variety params are JSON for
//! human-inspectability.

use std::path::Path;

use bytemuck::{Pod, Zeroable};
use serde::{Deserialize, Serialize};

use crate::game::asset_editor::variety::VarietyParams;
use crate::game::types::Vertex;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Magic bytes identifying a .btasset file.
pub const BTASSET_MAGIC: [u8; 4] = *b"BTAS";

/// Current file format version.
const BTASSET_VERSION: u32 = 1;

/// Size of the header in bytes. Must always be 32.
const HEADER_SIZE: u32 = 32;

// ============================================================================
// HEADER
// ============================================================================

/// Fixed-size binary header for the .btasset format.
///
/// Total size: exactly 32 bytes.
/// - `magic` (4) + `version` (4) + `vertex_count` (4) + `index_count` (4)
///   + `metadata_offset` (4) + `variety_offset` (4) + `_reserved` (8) = 32.
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct BtassetHeader {
    /// Magic bytes: always `b"BTAS"`.
    pub magic: [u8; 4],
    /// File format version (currently 1).
    pub version: u32,
    /// Number of vertices in the mesh.
    pub vertex_count: u32,
    /// Number of triangle indices in the mesh.
    pub index_count: u32,
    /// Byte offset from the start of the file to the metadata JSON section.
    pub metadata_offset: u32,
    /// Byte offset from the start of the file to the variety params JSON section.
    pub variety_offset: u32,
    /// Reserved for future use; must be zeroed.
    pub _reserved: [u8; 8],
}

static_assertions::assert_eq_size!(BtassetHeader, [u8; 32]);

// ============================================================================
// METADATA
// ============================================================================

/// Human-readable metadata stored as JSON inside the .btasset file.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AssetMetadata {
    /// Display name of the asset (e.g. "Oak Tree").
    pub name: String,
    /// Category slug (e.g. "tree", "rock").
    pub category: String,
    /// Searchable tags.
    pub tags: Vec<String>,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// Cached vertex count (matches header).
    pub vertex_count: u32,
    /// Cached index count (matches header).
    pub index_count: u32,
}

// ============================================================================
// LOADED ASSET
// ============================================================================

/// A fully loaded .btasset: geometry + metadata + optional variety params.
pub struct LoadedAsset {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub metadata: AssetMetadata,
    pub variety_params: Option<VarietyParams>,
}

impl std::fmt::Debug for LoadedAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedAsset")
            .field("vertices_len", &self.vertices.len())
            .field("indices_len", &self.indices.len())
            .field("metadata", &self.metadata)
            .field("variety_params", &self.variety_params)
            .finish()
    }
}

// ============================================================================
// ERROR TYPE
// ============================================================================

/// Errors that can occur during .btasset save/load.
#[derive(Debug)]
pub enum AssetFileError {
    /// File is smaller than the 32-byte header.
    FileTooShort,
    /// Magic bytes do not match `b"BTAS"`.
    InvalidMagic,
    /// File version is not supported.
    UnsupportedVersion(u32),
    /// Standard I/O error.
    IoError(std::io::Error),
    /// JSON serialization/deserialization error.
    JsonError(serde_json::Error),
}

impl std::fmt::Display for AssetFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssetFileError::FileTooShort => write!(f, "file too short for btasset header"),
            AssetFileError::InvalidMagic => write!(f, "invalid magic bytes (expected BTAS)"),
            AssetFileError::UnsupportedVersion(v) => {
                write!(f, "unsupported btasset version: {v}")
            }
            AssetFileError::IoError(e) => write!(f, "IO error: {e}"),
            AssetFileError::JsonError(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl std::error::Error for AssetFileError {}

impl From<std::io::Error> for AssetFileError {
    fn from(e: std::io::Error) -> Self {
        AssetFileError::IoError(e)
    }
}

impl From<serde_json::Error> for AssetFileError {
    fn from(e: serde_json::Error) -> Self {
        AssetFileError::JsonError(e)
    }
}

// ============================================================================
// SAVE
// ============================================================================

/// Write a .btasset file to disk.
///
/// File layout:
/// ```text
/// [BtassetHeader 32 bytes]
/// [vertex data: vertex_count * 40 bytes]
/// [index data:  index_count  *  4 bytes]
/// [metadata JSON bytes]
/// [variety params JSON bytes (optional)]
/// ```
pub fn save_btasset(
    path: &Path,
    vertices: &[Vertex],
    indices: &[u32],
    metadata: &AssetMetadata,
    variety_params: Option<&VarietyParams>,
) -> Result<(), AssetFileError> {
    use std::io::Write;

    let vertex_bytes = bytemuck::cast_slice::<Vertex, u8>(vertices);
    let index_bytes = bytemuck::cast_slice::<u32, u8>(indices);
    let metadata_json = serde_json::to_vec(metadata)?;
    let variety_json = variety_params.map(serde_json::to_vec).transpose()?;

    let metadata_offset = HEADER_SIZE + vertex_bytes.len() as u32 + index_bytes.len() as u32;
    let variety_offset = metadata_offset + metadata_json.len() as u32;

    let header = BtassetHeader {
        magic: BTASSET_MAGIC,
        version: BTASSET_VERSION,
        vertex_count: vertices.len() as u32,
        index_count: indices.len() as u32,
        metadata_offset,
        variety_offset,
        _reserved: [0u8; 8],
    };

    // Ensure parent directories exist.
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut file = std::fs::File::create(path)?;
    file.write_all(bytemuck::bytes_of(&header))?;
    file.write_all(vertex_bytes)?;
    file.write_all(index_bytes)?;
    file.write_all(&metadata_json)?;
    if let Some(vj) = variety_json {
        file.write_all(&vj)?;
    }
    Ok(())
}

// ============================================================================
// LOAD
// ============================================================================

/// Read a .btasset file from disk and reconstruct all sections.
pub fn load_btasset(path: &Path) -> Result<LoadedAsset, AssetFileError> {
    let data = std::fs::read(path)?;

    if data.len() < HEADER_SIZE as usize {
        return Err(AssetFileError::FileTooShort);
    }

    let header: &BtassetHeader = bytemuck::from_bytes(&data[..HEADER_SIZE as usize]);

    if header.magic != BTASSET_MAGIC {
        return Err(AssetFileError::InvalidMagic);
    }
    if header.version != BTASSET_VERSION {
        return Err(AssetFileError::UnsupportedVersion(header.version));
    }

    // Vertex data starts right after the header.
    let vertex_byte_count = header.vertex_count as usize * std::mem::size_of::<Vertex>();
    let vertex_start = HEADER_SIZE as usize;
    let vertex_end = vertex_start + vertex_byte_count;

    // Index data follows vertices.
    let index_byte_count = header.index_count as usize * std::mem::size_of::<u32>();
    let index_start = vertex_end;
    let index_end = index_start + index_byte_count;

    if data.len() < index_end {
        return Err(AssetFileError::FileTooShort);
    }

    let vertices: Vec<Vertex> =
        bytemuck::cast_slice::<u8, Vertex>(&data[vertex_start..vertex_end]).to_vec();
    let indices: Vec<u32> = bytemuck::cast_slice::<u8, u32>(&data[index_start..index_end]).to_vec();

    // Metadata JSON: from metadata_offset to variety_offset.
    let meta_start = header.metadata_offset as usize;
    let meta_end = header.variety_offset as usize;
    if data.len() < meta_start {
        return Err(AssetFileError::FileTooShort);
    }
    let metadata: AssetMetadata = serde_json::from_slice(&data[meta_start..meta_end])?;

    // Variety params JSON: from variety_offset to end of file (may be empty).
    let variety_params = if (header.variety_offset as usize) < data.len() {
        let variety_slice = &data[header.variety_offset as usize..];
        if variety_slice.is_empty() {
            None
        } else {
            Some(serde_json::from_slice(variety_slice)?)
        }
    } else {
        None
    };

    Ok(LoadedAsset {
        vertices,
        indices,
        metadata,
        variety_params,
    })
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_vertices() -> Vec<Vertex> {
        vec![
            Vertex {
                position: [1.0, 2.0, 3.0],
                normal: [0.0, 1.0, 0.0],
                color: [1.0, 0.0, 0.0, 1.0],
            },
            Vertex {
                position: [4.0, 5.0, 6.0],
                normal: [0.0, 0.0, 1.0],
                color: [0.0, 1.0, 0.0, 1.0],
            },
            Vertex {
                position: [7.0, 8.0, 9.0],
                normal: [1.0, 0.0, 0.0],
                color: [0.0, 0.0, 1.0, 1.0],
            },
        ]
    }

    fn make_test_metadata() -> AssetMetadata {
        AssetMetadata {
            name: "Test Asset".to_string(),
            category: "tree".to_string(),
            tags: vec!["deciduous".to_string(), "forest".to_string()],
            created_at: "2026-02-06T12:00:00Z".to_string(),
            vertex_count: 3,
            index_count: 3,
        }
    }

    #[test]
    fn test_header_size() {
        assert_eq!(std::mem::size_of::<BtassetHeader>(), 32);
    }

    #[test]
    fn test_vertex_size() {
        // position [f32;3] = 12 + normal [f32;3] = 12 + color [f32;4] = 16 => 40
        assert_eq!(std::mem::size_of::<Vertex>(), 40);
    }

    #[test]
    fn test_round_trip_without_variety() {
        let dir = std::env::temp_dir().join("btasset_test_no_variety");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.btasset");

        let vertices = make_test_vertices();
        let indices: Vec<u32> = vec![0, 1, 2];
        let metadata = make_test_metadata();

        save_btasset(&path, &vertices, &indices, &metadata, None).unwrap();
        let loaded = load_btasset(&path).unwrap();

        // Vertices are bitwise identical.
        assert_eq!(loaded.vertices.len(), vertices.len());
        for (a, b) in loaded.vertices.iter().zip(vertices.iter()) {
            assert_eq!(a.position, b.position);
            assert_eq!(a.normal, b.normal);
            assert_eq!(a.color, b.color);
        }

        // Indices are identical.
        assert_eq!(loaded.indices, indices);

        // Metadata round-trips.
        assert_eq!(loaded.metadata, metadata);

        // No variety params.
        assert!(loaded.variety_params.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_round_trip_with_variety() {
        let dir = std::env::temp_dir().join("btasset_test_variety");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test_v.btasset");

        let vertices = make_test_vertices();
        let indices: Vec<u32> = vec![0, 1, 2];
        let metadata = make_test_metadata();
        let variety = VarietyParams::default();

        save_btasset(&path, &vertices, &indices, &metadata, Some(&variety)).unwrap();
        let loaded = load_btasset(&path).unwrap();

        assert_eq!(loaded.vertices.len(), vertices.len());
        assert_eq!(loaded.indices, indices);
        assert_eq!(loaded.metadata, metadata);

        let lv = loaded.variety_params.expect("variety params should exist");
        assert_eq!(lv.scale_min, variety.scale_min);
        assert_eq!(lv.scale_max, variety.scale_max);
        assert_eq!(lv.random_y_rotation, variety.random_y_rotation);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_invalid_magic() {
        let dir = std::env::temp_dir().join("btasset_test_magic");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("bad_magic.btasset");

        // Write 32 bytes with wrong magic.
        let mut bad = [0u8; 32];
        bad[0..4].copy_from_slice(b"NOPE");
        std::fs::write(&path, &bad).unwrap();

        match load_btasset(&path) {
            Err(AssetFileError::InvalidMagic) => {}
            other => panic!("expected InvalidMagic, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_file_too_short() {
        let dir = std::env::temp_dir().join("btasset_test_short");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("short.btasset");

        std::fs::write(&path, &[0u8; 10]).unwrap();

        match load_btasset(&path) {
            Err(AssetFileError::FileTooShort) => {}
            other => panic!("expected FileTooShort, got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_unsupported_version() {
        let dir = std::env::temp_dir().join("btasset_test_version");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("bad_version.btasset");

        let mut header = BtassetHeader::zeroed();
        header.magic = BTASSET_MAGIC;
        header.version = 99;
        let bytes = bytemuck::bytes_of(&header);
        std::fs::write(&path, bytes).unwrap();

        match load_btasset(&path) {
            Err(AssetFileError::UnsupportedVersion(99)) => {}
            other => panic!("expected UnsupportedVersion(99), got {other:?}"),
        }

        let _ = std::fs::remove_dir_all(&dir);
    }
}
