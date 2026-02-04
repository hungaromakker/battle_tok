//! Production Buildings
//!
//! Buildings that generate resources over time

use super::resources::ResourceType;

/// Type of production building
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductionType {
    /// Produces food
    Farm,
    /// Produces wood
    LumberMill,
    /// Produces stone
    Quarry,
    /// Produces iron
    Mine,
    /// Produces gold (through trade/tax)
    Market,
}

impl ProductionType {
    /// Resource this building produces
    pub fn produces(&self) -> ResourceType {
        match self {
            ProductionType::Farm => ResourceType::Food,
            ProductionType::LumberMill => ResourceType::Wood,
            ProductionType::Quarry => ResourceType::Stone,
            ProductionType::Mine => ResourceType::Iron,
            ProductionType::Market => ResourceType::Gold,
        }
    }

    /// Base production per worker per day
    pub fn base_production(&self) -> i32 {
        match self {
            ProductionType::Farm => 3,
            ProductionType::LumberMill => 2,
            ProductionType::Quarry => 2,
            ProductionType::Mine => 1,
            ProductionType::Market => 5,
        }
    }

    /// Maximum workers this building can support
    pub fn max_workers(&self) -> u32 {
        match self {
            ProductionType::Farm => 5,
            ProductionType::LumberMill => 3,
            ProductionType::Quarry => 4,
            ProductionType::Mine => 3,
            ProductionType::Market => 2,
        }
    }

    /// Building cost
    pub fn build_cost(&self) -> Vec<(ResourceType, i32)> {
        match self {
            ProductionType::Farm => vec![(ResourceType::Wood, 20), (ResourceType::Gold, 10)],
            ProductionType::LumberMill => vec![(ResourceType::Stone, 15), (ResourceType::Gold, 20)],
            ProductionType::Quarry => vec![(ResourceType::Wood, 25), (ResourceType::Gold, 30)],
            ProductionType::Mine => vec![
                (ResourceType::Wood, 30),
                (ResourceType::Stone, 20),
                (ResourceType::Gold, 50),
            ],
            ProductionType::Market => vec![
                (ResourceType::Wood, 40),
                (ResourceType::Stone, 30),
                (ResourceType::Gold, 100),
            ],
        }
    }

    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            ProductionType::Farm => "Farm",
            ProductionType::LumberMill => "Lumber Mill",
            ProductionType::Quarry => "Quarry",
            ProductionType::Mine => "Mine",
            ProductionType::Market => "Market",
        }
    }
}

/// A production building instance
#[derive(Debug, Clone)]
pub struct ProductionBuilding {
    /// Type of building
    pub building_type: ProductionType,
    /// Unique ID
    pub id: u32,
    /// Current number of workers
    pub workers: u32,
    /// Upgrade level (1 = base)
    pub level: u32,
    /// Is building active?
    pub active: bool,
    /// Accumulated partial production (for sub-day calculations)
    accumulated: f32,
}

impl ProductionBuilding {
    pub fn new(building_type: ProductionType, id: u32) -> Self {
        Self {
            building_type,
            id,
            workers: 0,
            level: 1,
            active: true,
            accumulated: 0.0,
        }
    }

    /// Calculate daily production
    pub fn daily_production(&self) -> i32 {
        if !self.active || self.workers == 0 {
            return 0;
        }

        let base = self.building_type.base_production();
        let worker_mult = self.workers as f32;
        let level_mult = 1.0 + (self.level - 1) as f32 * 0.25; // +25% per level

        (base as f32 * worker_mult * level_mult) as i32
    }

    /// Update production (called each frame, accumulates partial production)
    /// Returns production to add when >= 1 unit accumulated
    pub fn update(&mut self, day_progress: f32) -> i32 {
        if !self.active || self.workers == 0 {
            return 0;
        }

        let daily = self.daily_production() as f32;
        self.accumulated += daily * day_progress;

        let whole = self.accumulated.floor() as i32;
        self.accumulated -= whole as f32;

        whole
    }

    /// Add a worker (returns false if at max)
    pub fn add_worker(&mut self) -> bool {
        if self.workers < self.building_type.max_workers() {
            self.workers += 1;
            true
        } else {
            false
        }
    }

    /// Remove a worker (returns false if none)
    pub fn remove_worker(&mut self) -> bool {
        if self.workers > 0 {
            self.workers -= 1;
            true
        } else {
            false
        }
    }

    /// Get upgrade cost to next level
    pub fn upgrade_cost(&self) -> Vec<(ResourceType, i32)> {
        let base_cost = self.building_type.build_cost();
        let multiplier = self.level as i32;

        base_cost
            .into_iter()
            .map(|(res, amount)| (res, amount * multiplier))
            .collect()
    }

    /// Upgrade the building
    pub fn upgrade(&mut self) {
        self.level += 1;
    }

    /// Resource type produced
    pub fn produces(&self) -> ResourceType {
        self.building_type.produces()
    }

    /// Get efficiency (0-1 based on workers vs max)
    pub fn efficiency(&self) -> f32 {
        if self.building_type.max_workers() == 0 {
            return 0.0;
        }
        self.workers as f32 / self.building_type.max_workers() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_production_building() {
        let mut farm = ProductionBuilding::new(ProductionType::Farm, 1);

        assert_eq!(farm.daily_production(), 0); // No workers

        farm.add_worker();
        assert_eq!(farm.daily_production(), 3); // 1 worker * 3 base

        farm.add_worker();
        assert_eq!(farm.daily_production(), 6); // 2 workers * 3 base
    }

    #[test]
    fn test_upgrade() {
        let mut farm = ProductionBuilding::new(ProductionType::Farm, 1);
        farm.workers = 2;

        assert_eq!(farm.daily_production(), 6); // Level 1

        farm.upgrade();
        // Level 2 = 1.25x multiplier
        assert_eq!(farm.daily_production(), 7); // 6 * 1.25 = 7.5 -> 7
    }

    #[test]
    fn test_max_workers() {
        let mut farm = ProductionBuilding::new(ProductionType::Farm, 1);

        for _ in 0..5 {
            assert!(farm.add_worker());
        }

        assert!(!farm.add_worker()); // At max
        assert_eq!(farm.workers, 5);
    }
}
