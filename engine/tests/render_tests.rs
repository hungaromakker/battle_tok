//! Render Tests - Buffer Creation and Uniform Serialization
//!
//! Tests for the render module including uniform structs, buffer data,
//! and GPU-compatible struct serialization.

use battle_tok_engine::render::uniforms::{
    EntityBufferData, PlacedEntity, SkySettings, TestUniforms,
    pack_color, ENTITY_COLORS,
};

// ============================================================================
// TestUniforms Tests
// ============================================================================

#[test]
fn test_test_uniforms_default() {
    let uniforms = TestUniforms::default();

    // Check default camera position
    assert_eq!(uniforms.camera_pos, [0.0, 2.0, 8.0]);
    // Check default resolution
    assert_eq!(uniforms.resolution, [1920.0, 1080.0]);
    // Check default time
    assert_eq!(uniforms.time, 0.0);
    // Check default debug mode
    assert_eq!(uniforms.debug_mode, 0);
    // Check default human visibility
    assert_eq!(uniforms.human_visible, 1);
    // Check default camera target
    assert_eq!(uniforms.camera_target, [0.0, 0.0, 0.0]);
    // Check default grid size
    assert_eq!(uniforms.grid_size, 1.0);
    // Check default volume grid visibility
    assert_eq!(uniforms.volume_grid_visible, 0);
    // Check default placement height
    assert_eq!(uniforms.placement_height, 0.0);
    // Check default HUD visibility
    assert_eq!(uniforms.show_hud, 1);
}

#[test]
fn test_test_uniforms_new() {
    let camera_pos = [5.0, 10.0, 15.0];
    let camera_target = [1.0, 2.0, 3.0];
    let uniforms = TestUniforms::new(camera_pos, camera_target);

    assert_eq!(uniforms.camera_pos, camera_pos);
    assert_eq!(uniforms.camera_target, camera_target);
    // Other fields should be default
    assert_eq!(uniforms.time, 0.0);
    assert_eq!(uniforms.debug_mode, 0);
}

#[test]
fn test_test_uniforms_set_resolution() {
    let mut uniforms = TestUniforms::default();
    uniforms.set_resolution(2560, 1440);

    assert_eq!(uniforms.resolution, [2560.0, 1440.0]);
}

#[test]
fn test_test_uniforms_set_time() {
    let mut uniforms = TestUniforms::default();
    uniforms.set_time(5.5);

    assert_eq!(uniforms.time, 5.5);
}

#[test]
fn test_test_uniforms_bytemuck_pod() {
    // Ensure TestUniforms can be converted to bytes for GPU buffer
    let uniforms = TestUniforms::default();
    let bytes: &[u8] = bytemuck::bytes_of(&uniforms);

    // Should have bytes
    assert!(!bytes.is_empty());

    // Size should match struct size
    assert_eq!(bytes.len(), std::mem::size_of::<TestUniforms>());
}

// ============================================================================
// PlacedEntity Tests
// ============================================================================

#[test]
fn test_placed_entity_size_48_bytes() {
    // Critical: PlacedEntity MUST be 48 bytes to match WGSL layout
    assert_eq!(
        std::mem::size_of::<PlacedEntity>(),
        48,
        "PlacedEntity must be exactly 48 bytes to match WGSL struct layout"
    );
}

#[test]
fn test_placed_entity_default() {
    let entity = PlacedEntity::default();

    assert_eq!(entity.position, [0.0, 0.0, 0.0]);
    assert_eq!(entity.entity_type, 0);
    assert_eq!(entity.scale, [0.5, 0.5, 0.5]);
    assert_eq!(entity.color_packed, 0xFF8800); // Orange default
}

#[test]
fn test_placed_entity_new() {
    let position = [1.0, 2.0, 3.0];
    let entity_type = 2; // Capsule
    let scale = 1.5;
    let color = 0xFF0000; // Red

    let entity = PlacedEntity::new(position, entity_type, scale, color);

    assert_eq!(entity.position, position);
    assert_eq!(entity.entity_type, entity_type);
    assert_eq!(entity.scale, [scale, scale, scale]);
    assert_eq!(entity.color_packed, color);
}

#[test]
fn test_placed_entity_bytemuck_pod() {
    let entity = PlacedEntity::default();
    let bytes: &[u8] = bytemuck::bytes_of(&entity);

    assert_eq!(bytes.len(), 48);
}

// ============================================================================
// EntityBufferData Tests
// ============================================================================

#[test]
fn test_entity_buffer_data_size() {
    // EntityBufferData = 16 bytes header + 64 * 48 bytes entities
    let expected_size = 16 + 48 * 64;
    assert_eq!(
        std::mem::size_of::<EntityBufferData>(),
        expected_size,
        "EntityBufferData size mismatch"
    );
}

#[test]
fn test_entity_buffer_data_default() {
    let buffer = EntityBufferData::default();

    assert_eq!(buffer.count, 0);
    assert_eq!(buffer._pad0, 0);
    assert_eq!(buffer._pad1, 0);
    assert_eq!(buffer._pad2, 0);

    // All entities should be default
    for entity in &buffer.entities {
        assert_eq!(entity.position, [0.0, 0.0, 0.0]);
    }
}

#[test]
fn test_entity_buffer_data_bytemuck_pod() {
    let buffer = EntityBufferData::default();
    let bytes: &[u8] = bytemuck::bytes_of(&buffer);

    assert_eq!(bytes.len(), std::mem::size_of::<EntityBufferData>());
}

// ============================================================================
// Color Packing Tests
// ============================================================================

#[test]
fn test_pack_color_basic() {
    // Test packing RGB to u32
    let packed = pack_color(255, 128, 64);

    // Format: 0x00RRGGBB
    assert_eq!(packed, 0x00FF8040);
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
fn test_entity_colors_array() {
    // Ensure we have 8 predefined colors
    assert_eq!(ENTITY_COLORS.len(), 8);

    // All colors should have valid RGB components
    for (r, g, b) in ENTITY_COLORS.iter() {
        // Each component should be 0-255
        assert!(*r <= 255);
        assert!(*g <= 255);
        assert!(*b <= 255);
    }
}

// ============================================================================
// SkySettings Tests
// ============================================================================

#[test]
fn test_sky_settings_size() {
    // SkySettings must be 352 bytes to match WGSL layout
    assert_eq!(
        std::mem::size_of::<SkySettings>(),
        352,
        "SkySettings must be exactly 352 bytes to match WGSL layout"
    );
}

#[test]
fn test_sky_settings_default() {
    let sky = SkySettings::default();

    // Check default time (noon)
    assert_eq!(sky.time_of_day, 0.25);
    // Check cycle speed
    assert_eq!(sky.cycle_speed, 0.01);
    // Check sun enabled
    assert_eq!(sky.sun_enabled, 1);
    // Check stars enabled
    assert_eq!(sky.stars_enabled, 1);
    // Check aurora enabled
    assert_eq!(sky.aurora_enabled, 1);
    // Check moon enabled
    assert_eq!(sky.moon_enabled, 1);
}

#[test]
fn test_sky_settings_bytemuck_pod() {
    let sky = SkySettings::default();
    let bytes: &[u8] = bytemuck::bytes_of(&sky);

    assert_eq!(bytes.len(), 352);
}
