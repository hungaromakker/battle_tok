//! Population System
//!
//! Manages villagers, troops, morale, and AI job assignment.
//! - 1 villager = 1 food unit per day
//! - AI auto-assigns workers to jobs
//! - Morale affected by flag visibility

pub mod villager;
pub mod morale;
pub mod job_ai;

pub use villager::{Villager, VillagerRole, VillagerStats, Population};
pub use morale::{Morale, MoraleModifier, MoraleState};
pub use job_ai::{JobAssignment, JobAI, JobPriority};
