//! Population System
//!
//! Manages villagers, troops, morale, and AI job assignment.
//! - 1 villager = 1 food unit per day
//! - AI auto-assigns workers to jobs
//! - Morale affected by flag visibility

pub mod job_ai;
pub mod morale;
pub mod villager;

pub use job_ai::{JobAI, JobAssignment, JobPriority};
pub use morale::{Morale, MoraleModifier, MoraleState};
pub use villager::{Population, Villager, VillagerRole, VillagerStats};
