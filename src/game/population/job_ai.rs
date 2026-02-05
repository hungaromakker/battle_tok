//! Job Assignment AI
//!
//! Automatically assigns idle villagers to jobs based on:
//! - Current resource needs
//! - Building availability
//! - Villager skills
//!
//! The player doesn't need to micromanage - AI handles job assignment.

use std::collections::HashMap;

use super::super::economy::production::{ProductionBuilding, ProductionType};
use super::super::economy::resources::{ResourceType, Resources};
use super::villager::{Population, VillagerRole};

/// Priority level for job types
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum JobPriority {
    /// Critical - must fill
    Critical = 4,
    /// High - fill if possible
    High = 3,
    /// Normal - fill when convenient
    Normal = 2,
    /// Low - fill last
    Low = 1,
}

/// A potential job assignment
#[derive(Debug, Clone)]
pub struct JobAssignment {
    /// Building to assign to
    pub building_id: u32,
    /// Role for the job
    pub role: VillagerRole,
    /// Priority
    pub priority: JobPriority,
    /// How many workers needed
    pub workers_needed: u32,
}

/// AI job assignment system
#[derive(Debug, Clone, Default)]
pub struct JobAI {
    /// Resource priorities (which resources need more workers)
    resource_priorities: HashMap<ResourceType, JobPriority>,
    /// Last update time
    last_update: f32,
    /// Update interval in game time
    update_interval: f32,
}

impl JobAI {
    pub fn new() -> Self {
        Self {
            resource_priorities: HashMap::new(),
            last_update: 0.0,
            update_interval: 0.5, // Update twice per day
        }
    }

    /// Analyze resources and determine priorities
    pub fn analyze_resources(&mut self, resources: &Resources) {
        self.resource_priorities.clear();

        // Check each resource
        for (res_type, _) in resources.all() {
            let current = resources.get(res_type);
            let income = resources.get_income(res_type);
            let expenses = resources.get_expenses(res_type);
            let net = income - expenses;

            // Calculate days until depletion
            let days_until_empty = if net < 0 && current > 0 {
                current as f32 / (-net as f32)
            } else {
                f32::INFINITY
            };

            let priority = if days_until_empty < 1.0 {
                JobPriority::Critical
            } else if days_until_empty < 3.0 {
                JobPriority::High
            } else if net < 0 {
                JobPriority::Normal
            } else {
                JobPriority::Low
            };

            self.resource_priorities.insert(res_type, priority);
        }

        // Food is always at least Normal priority (survival)
        let food_priority = self
            .resource_priorities
            .entry(ResourceType::Food)
            .or_insert(JobPriority::Low);
        if *food_priority == JobPriority::Low {
            *food_priority = JobPriority::Normal;
        }
    }

    /// Get priority for a production type
    fn production_priority(&self, prod_type: ProductionType) -> JobPriority {
        let res_type = prod_type.produces();
        self.resource_priorities
            .get(&res_type)
            .copied()
            .unwrap_or(JobPriority::Normal)
    }

    /// Generate job assignments for available buildings
    pub fn generate_assignments(
        &mut self,
        buildings: &[ProductionBuilding],
        _population: &Population,
        resources: &Resources,
    ) -> Vec<JobAssignment> {
        // Update analysis
        self.analyze_resources(resources);

        let mut assignments = Vec::new();

        for building in buildings {
            if !building.active {
                continue;
            }

            let max_workers = building.building_type.max_workers();
            let current_workers = building.workers;

            if current_workers < max_workers {
                let role = Self::building_to_role(building.building_type);
                let priority = self.production_priority(building.building_type);

                assignments.push(JobAssignment {
                    building_id: building.id,
                    role,
                    priority,
                    workers_needed: max_workers - current_workers,
                });
            }
        }

        // Sort by priority (highest first)
        assignments.sort_by(|a, b| b.priority.cmp(&a.priority));

        assignments
    }

    /// Convert production type to villager role
    fn building_to_role(prod_type: ProductionType) -> VillagerRole {
        match prod_type {
            ProductionType::Farm => VillagerRole::Farmer,
            ProductionType::LumberMill => VillagerRole::Lumberjack,
            ProductionType::Quarry => VillagerRole::Stonecutter,
            ProductionType::Mine => VillagerRole::Miner,
            ProductionType::Market => VillagerRole::Merchant,
        }
    }

    /// Auto-assign idle villagers to jobs
    /// Returns list of (villager_id, building_id, role) assignments made
    pub fn auto_assign(
        &mut self,
        assignments: &[JobAssignment],
        population: &mut Population,
        buildings: &mut [ProductionBuilding],
    ) -> Vec<(u32, u32, VillagerRole)> {
        let mut made_assignments = Vec::new();
        let mut building_workers: HashMap<u32, u32> = HashMap::new();

        // Track current workers per building
        for building in buildings.iter() {
            building_workers.insert(building.id, building.workers);
        }

        // Get idle villagers
        let idle_ids: Vec<u32> = population.idle_villagers().iter().map(|v| v.id).collect();

        let mut idle_iter = idle_ids.into_iter();

        // Process assignments by priority
        for assignment in assignments {
            let current = building_workers
                .get(&assignment.building_id)
                .copied()
                .unwrap_or(0);
            let max = assignment.workers_needed + current;

            while building_workers
                .get(&assignment.building_id)
                .copied()
                .unwrap_or(0)
                < max
            {
                if let Some(villager_id) = idle_iter.next() {
                    // Assign the villager
                    population.assign_role(
                        villager_id,
                        assignment.role,
                        Some(assignment.building_id),
                    );

                    // Update building workers
                    if let Some(building) = buildings
                        .iter_mut()
                        .find(|b| b.id == assignment.building_id)
                    {
                        building.add_worker();
                    }
                    *building_workers.entry(assignment.building_id).or_insert(0) += 1;

                    made_assignments.push((villager_id, assignment.building_id, assignment.role));
                } else {
                    // No more idle villagers
                    break;
                }
            }
        }

        made_assignments
    }

    /// Should we run an update? (based on game time)
    pub fn should_update(&self, game_time: f32) -> bool {
        game_time - self.last_update >= self.update_interval
    }

    /// Mark update done
    pub fn mark_updated(&mut self, game_time: f32) {
        self.last_update = game_time;
    }

    /// Set update interval
    pub fn set_update_interval(&mut self, interval: f32) {
        self.update_interval = interval.max(0.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_analysis() {
        let mut ai = JobAI::new();
        let mut resources = Resources::new();

        // Set food to be critical (low with negative income)
        resources.set(ResourceType::Food, 5);
        resources.set_expenses(ResourceType::Food, 10);

        ai.analyze_resources(&resources);

        assert_eq!(
            ai.resource_priorities.get(&ResourceType::Food),
            Some(&JobPriority::Critical)
        );
    }

    #[test]
    fn test_assignment_generation() {
        let mut ai = JobAI::new();
        let resources = Resources::new();
        let population = Population::new();

        let buildings = vec![
            ProductionBuilding::new(ProductionType::Farm, 1),
            ProductionBuilding::new(ProductionType::Quarry, 2),
        ];

        let assignments = ai.generate_assignments(&buildings, &population, &resources);

        // Should have assignments for both buildings
        assert!(!assignments.is_empty());
    }

    #[test]
    fn test_auto_assign() {
        let mut ai = JobAI::new();
        let mut resources = Resources::new();
        let mut population = Population::new();

        // Add some idle villagers
        population.add_villager();
        population.add_villager();

        let mut buildings = vec![ProductionBuilding::new(ProductionType::Farm, 1)];

        let assignments = ai.generate_assignments(&buildings, &population, &resources);
        let made = ai.auto_assign(&assignments, &mut population, &mut buildings);

        // Should have assigned both villagers
        assert_eq!(made.len(), 2);
        assert_eq!(population.idle_count(), 0);
        assert_eq!(buildings[0].workers, 2);
    }
}
