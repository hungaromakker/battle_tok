//! Scene Module
//!
//! High-level scene compositions that wire together all game systems.

pub mod battle_scene;

pub use battle_scene::{BattleScene, ExplosionEvent, WeaponMode};
