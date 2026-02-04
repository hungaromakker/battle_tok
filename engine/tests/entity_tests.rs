//! Entity Tests - Struct Size, Color Packing, and Grid Snapping
//!
//! Tests for the entity system including PlacedEntity, EntityBufferData,
//! color packing/unpacking, and grid snapping functionality.

use glam::Vec3;
use magic_engine::render::entities::{
    PlacedEntity, EntityBufferData, entity_type,
    pack_color, unpack_color, ENTITY_COLORS,
};
use magic_engine::world::{GridConfig, snap_to_grid, clamp_to_map};

// ============================================================================
// PlacedEntity Size Tests (Critical - Must be 48 bytes for GPU compatibility)
// ============================================================================

#[test]
fn test_placed_entity_size_exact_48_bytes() {
    // This is THE critical test - PlacedEntity MUST be exactly 48 bytes
    // to match WGSL struct layout for GPU buffer operations
    assert_eq!(
        std::mem::size_of::<PlacedEntity>(),
        48,
        "CRITICAL: PlacedEntity must be exactly 48 bytes to match WGSL struct layout!\n\
         Expected: 48 bytes\n\
         Actual: {} bytes\n\
         WGSL Layout:\n\
         - offset 0:  position (vec3<f32>) = 12 bytes\n\
         - offset 12: _pad_after_pos = 4 bytes\n\
         - offset 16: entity_type (u32) = 4 bytes\n\
         - offset 20: _pad_before_scale = 12 bytes\n\
         - offset 32: scale (vec3<f32>) = 12 bytes\n\
         - offset 44: color_packed (u32) = 4 bytes\n\
         - Total: 48 bytes",
        std::mem::size_of::<PlacedEntity>()
    );
}

#[test]
fn test_entity_buffer_data_size() {
    // EntityBufferData = 16 bytes header + 64 entities * 48 bytes each
    let expected_size = 16 + (64 * 48);
    assert_eq!(
        std::mem::size_of::<EntityBufferData>(),
        expected_size,
        "EntityBufferData size mismatch: expected {} bytes, got {} bytes",
        expected_size,
        std::mem::size_of::<EntityBufferData>()
    );
}

#[test]
fn test_placed_entity_bytemuck_compatible() {
    // Must be Pod and Zeroable for GPU buffer operations
    let entity = PlacedEntity::default();
    let bytes: &[u8] = bytemuck::bytes_of(&entity);

    assert_eq!(bytes.len(), 48);

    // Should be able to convert back
    let entity_back: &PlacedEntity = bytemuck::from_bytes(bytes);
    assert_eq!(entity_back.position, entity.position);
    assert_eq!(entity_back.entity_type, entity.entity_type);
}

#[test]
fn test_entity_buffer_data_bytemuck_compatible() {
    let buffer = EntityBufferData::default();
    let bytes: &[u8] = bytemuck::bytes_of(&buffer);

    assert_eq!(bytes.len(), std::mem::size_of::<EntityBufferData>());
}

// ============================================================================
// PlacedEntity Field Tests
// ============================================================================

#[test]
fn test_placed_entity_default() {
    let entity = PlacedEntity::default();

    assert_eq!(entity.position, [0.0, 0.0, 0.0]);
    assert_eq!(entity._pad_after_pos, 0);
    assert_eq!(entity.entity_type, 0);
    assert_eq!(entity._pad_before_scale, [0, 0, 0]);
    assert_eq!(entity.scale, [0.5, 0.5, 0.5]);
    assert_eq!(entity.color_packed, 0xFF8800); // Orange default
}

#[test]
fn test_entity_buffer_data_default() {
    let buffer = EntityBufferData::default();

    assert_eq!(buffer.count, 0);
    assert_eq!(buffer._pad0, 0);
    assert_eq!(buffer._pad1, 0);
    assert_eq!(buffer._pad2, 0);

    // All entities should be default
    for (i, entity) in buffer.entities.iter().enumerate() {
        assert_eq!(
            entity.position,
            [0.0, 0.0, 0.0],
            "Entity {} position should be default",
            i
        );
    }
}

#[test]
fn test_entity_buffer_data_capacity() {
    let buffer = EntityBufferData::default();

    // Should have exactly 64 entity slots
    assert_eq!(buffer.entities.len(), 64);
}

// ============================================================================
// Entity Type Constants Tests
// ============================================================================

#[test]
fn test_entity_type_sphere() {
    assert_eq!(entity_type::SPHERE, 0);
}

#[test]
fn test_entity_type_box() {
    assert_eq!(entity_type::BOX, 1);
}

#[test]
fn test_entity_type_capsule() {
    assert_eq!(entity_type::CAPSULE, 2);
}

#[test]
fn test_entity_type_torus() {
    assert_eq!(entity_type::TORUS, 3);
}

#[test]
fn test_entity_type_cylinder() {
    assert_eq!(entity_type::CYLINDER, 4);
}

#[test]
fn test_entity_types_unique() {
    let types = [
        entity_type::SPHERE,
        entity_type::BOX,
        entity_type::CAPSULE,
        entity_type::TORUS,
        entity_type::CYLINDER,
    ];

    // All types should be unique
    for i in 0..types.len() {
        for j in (i + 1)..types.len() {
            assert_ne!(types[i], types[j], "Entity types {} and {} should be unique", i, j);
        }
    }
}

// ============================================================================
// Color Packing/Unpacking Tests
// ============================================================================

#[test]
fn test_pack_color_format() {
    // Format should be 0x00RRGGBB
    let packed = pack_color(0xAB, 0xCD, 0xEF);
    assert_eq!(packed, 0x00ABCDEF);
}

#[test]
fn test_pack_color_red() {
    let packed = pack_color(255, 0, 0);
    assert_eq!(packed, 0x00FF0000);
}

#[test]
fn test_pack_color_green() {
    let packed = pack_color(0, 255, 0);
    assert_eq!(packed, 0x0000FF00);
}

#[test]
fn test_pack_color_blue() {
    let packed = pack_color(0, 0, 255);
    assert_eq!(packed, 0x000000FF);
}

#[test]
fn test_pack_color_white() {
    let packed = pack_color(255, 255, 255);
    assert_eq!(packed, 0x00FFFFFF);
}

#[test]
fn test_pack_color_black() {
    let packed = pack_color(0, 0, 0);
    assert_eq!(packed, 0x00000000);
}

#[test]
fn test_unpack_color_red() {
    let (r, g, b) = unpack_color(0x00FF0000);
    assert_eq!((r, g, b), (255, 0, 0));
}

#[test]
fn test_unpack_color_green() {
    let (r, g, b) = unpack_color(0x0000FF00);
    assert_eq!((r, g, b), (0, 255, 0));
}

#[test]
fn test_unpack_color_blue() {
    let (r, g, b) = unpack_color(0x000000FF);
    assert_eq!((r, g, b), (0, 0, 255));
}

#[test]
fn test_pack_unpack_roundtrip() {
    // Test that pack and unpack are inverses of each other
    for r in (0..=255).step_by(17) {
        for g in (0..=255).step_by(17) {
            for b in (0..=255).step_by(17) {
                let packed = pack_color(r, g, b);
                let (ur, ug, ub) = unpack_color(packed);
                assert_eq!(
                    (ur, ug, ub),
                    (r, g, b),
                    "Pack/unpack roundtrip failed for ({}, {}, {})",
                    r, g, b
                );
            }
        }
    }
}

#[test]
fn test_entity_colors_array() {
    // Should have exactly 8 predefined colors
    assert_eq!(ENTITY_COLORS.len(), 8);
}

#[test]
fn test_entity_colors_valid_rgb() {
    for (i, (r, g, b)) in ENTITY_COLORS.iter().enumerate() {
        // All components should be valid u8 values (0-255)
        assert!(*r <= 255, "Color {} red component invalid", i);
        assert!(*g <= 255, "Color {} green component invalid", i);
        assert!(*b <= 255, "Color {} blue component invalid", i);
    }
}

#[test]
fn test_entity_colors_packable() {
    // All predefined colors should pack correctly
    for (r, g, b) in ENTITY_COLORS.iter() {
        let packed = pack_color(*r, *g, *b);
        let (ur, ug, ub) = unpack_color(packed);
        assert_eq!((*r, *g, *b), (ur, ug, ub));
    }
}

// ============================================================================
// Grid Snapping Tests
// ============================================================================

#[test]
fn test_grid_config_default() {
    let config = GridConfig::default();

    assert_eq!(config.small_grid_size, 1.0);
    assert_eq!(config.large_grid_size, 10.0);
    assert_eq!(config.map_size, 50.0);
    assert!(config.snap_enabled);
    assert!(!config.volume_grid_visible);
    assert_eq!(config.placement_height, 0.0);
}

#[test]
fn test_grid_config_new() {
    let config = GridConfig::new(2.0, 20.0, 100.0);

    assert_eq!(config.small_grid_size, 2.0);
    assert_eq!(config.large_grid_size, 20.0);
    assert_eq!(config.map_size, 100.0);
    assert!(config.snap_enabled);
}

#[test]
fn test_snap_to_grid_basic() {
    let config = GridConfig::default(); // grid_size = 1.0

    let pos = Vec3::new(1.3, 5.0, 2.7);
    let snapped = config.snap_to_grid(pos);

    assert_eq!(snapped.x, 1.0);
    assert_eq!(snapped.y, 5.0); // Y unchanged
    assert_eq!(snapped.z, 3.0);
}

#[test]
fn test_snap_to_grid_negative() {
    let config = GridConfig::default();

    let pos = Vec3::new(-1.6, 0.0, -2.3);
    let snapped = config.snap_to_grid(pos);

    assert_eq!(snapped.x, -2.0);
    assert_eq!(snapped.z, -2.0);
}

#[test]
fn test_snap_to_grid_preserves_y() {
    let config = GridConfig::default();

    let pos = Vec3::new(0.5, 123.456, 0.5);
    let snapped = config.snap_to_grid(pos);

    assert_eq!(snapped.y, 123.456, "Y coordinate should be preserved");
}

#[test]
fn test_snap_to_grid_disabled() {
    let mut config = GridConfig::default();
    config.snap_enabled = false;

    let pos = Vec3::new(1.3, 5.0, 2.7);
    let snapped = config.snap_to_grid(pos);

    // Should return unchanged position when snapping is disabled
    assert_eq!(snapped, pos);
}

#[test]
fn test_snap_to_grid_custom_size() {
    let mut config = GridConfig::default();
    config.small_grid_size = 0.5;

    let pos = Vec3::new(1.3, 0.0, 2.6);
    let snapped = config.snap_to_grid(pos);

    assert_eq!(snapped.x, 1.5);
    assert_eq!(snapped.z, 2.5);
}

#[test]
fn test_clamp_to_map_basic() {
    let config = GridConfig::default(); // map_size = 50.0

    let pos = Vec3::new(100.0, 25.0, -75.0);
    let clamped = config.clamp_to_map(pos);

    assert_eq!(clamped.x, 50.0);
    assert_eq!(clamped.y, 25.0); // Y unchanged
    assert_eq!(clamped.z, -50.0);
}

#[test]
fn test_clamp_to_map_within_bounds() {
    let config = GridConfig::default();

    let pos = Vec3::new(25.0, 10.0, -30.0);
    let clamped = config.clamp_to_map(pos);

    // Position within bounds should be unchanged
    assert_eq!(clamped.x, 25.0);
    assert_eq!(clamped.z, -30.0);
}

#[test]
fn test_clamp_to_map_preserves_y() {
    let config = GridConfig::default();

    let pos = Vec3::new(100.0, 999.0, 100.0);
    let clamped = config.clamp_to_map(pos);

    assert_eq!(clamped.y, 999.0, "Y coordinate should be preserved");
}

#[test]
fn test_snap_and_clamp() {
    let config = GridConfig::default();

    let pos = Vec3::new(100.3, 5.0, -75.7);
    let result = config.snap_and_clamp(pos);

    // Should be snapped to grid then clamped to map
    assert_eq!(result.x, 50.0); // Clamped to map_size
    assert_eq!(result.y, 5.0);  // Y unchanged
    assert_eq!(result.z, -50.0); // Clamped to -map_size
}

#[test]
fn test_standalone_snap_to_grid() {
    let pos = Vec3::new(1.6, 10.0, 3.2);
    let snapped = snap_to_grid(pos, 1.0);

    assert_eq!(snapped.x, 2.0);
    assert_eq!(snapped.y, 10.0); // Y unchanged
    assert_eq!(snapped.z, 3.0);
}

#[test]
fn test_standalone_clamp_to_map() {
    let pos = Vec3::new(100.0, 5.0, -100.0);
    let clamped = clamp_to_map(pos, 50.0);

    assert_eq!(clamped.x, 50.0);
    assert_eq!(clamped.y, 5.0); // Y unchanged
    assert_eq!(clamped.z, -50.0);
}

// ============================================================================
// Entity Creation and Placement Tests
// ============================================================================

#[test]
fn test_entity_color_cycling() {
    // Test that color index cycles through all colors
    for i in 0..16 {
        let color_index = i % ENTITY_COLORS.len();
        let (r, g, b) = ENTITY_COLORS[color_index];
        let packed = pack_color(r, g, b);

        // Color should be valid
        assert!(packed <= 0x00FFFFFF);
    }
}
