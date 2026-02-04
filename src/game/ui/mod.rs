//! UI Module
//!
//! User interface components for the game.

pub mod text;
pub mod slider;
pub mod terrain_editor;
pub mod overlay;
pub mod top_bar;

pub use text::{add_quad, get_char_bitmap, draw_text};
pub use slider::UISlider;
pub use terrain_editor::TerrainEditorUI;
pub use overlay::StartOverlay;
pub use top_bar::{TopBar, TOP_BAR_HEIGHT};
