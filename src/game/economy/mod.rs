//! Economy System
//!
//! Manages resources, production, and consumption for the game.
//! Starting resources: 100 Gold, 100 Stone, 10 Food
//! Day cycle: 10 minutes
//! 1 villager = 1 food unit per day

pub mod day_cycle;
pub mod production;
pub mod resources;

pub use day_cycle::{DAY_DURATION_SECONDS, DayCycle, TimeOfDay};
pub use production::{ProductionBuilding, ProductionType};
pub use resources::{ResourceType, Resources, STARTING_RESOURCES};
