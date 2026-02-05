//! UI Module
//!
//! User interface components for the game.

pub mod overlay;
pub mod slider;
pub mod terrain_editor;
pub mod text;
pub mod top_bar;

pub use overlay::StartOverlay;
pub use slider::UISlider;
pub use terrain_editor::TerrainEditorUI;
pub use text::{add_quad, draw_text, get_char_bitmap};
pub use top_bar::{TOP_BAR_HEIGHT, TopBar};
