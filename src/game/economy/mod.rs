//! Economy System
//!
//! Manages resources, production, and consumption for the game.
//! Starting resources: 100 Gold, 100 Stone, 10 Food
//! Day cycle: 10 minutes
//! 1 villager = 1 food unit per day

pub mod resources;
pub mod day_cycle;
pub mod production;

pub use resources::{Resources, ResourceType, STARTING_RESOURCES};
pub use day_cycle::{DayCycle, TimeOfDay, DAY_DURATION_SECONDS};
pub use production::{ProductionBuilding, ProductionType};
