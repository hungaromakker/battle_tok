//! Resource Management
//!
//! Tracks player resources: Gold, Stone, Wood, Food, Iron

use std::collections::HashMap;

/// Types of resources
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Gold,
    Stone,
    Wood,
    Food,
    Iron,
}

impl ResourceType {
    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            ResourceType::Gold => "Gold",
            ResourceType::Stone => "Stone",
            ResourceType::Wood => "Wood",
            ResourceType::Food => "Food",
            ResourceType::Iron => "Iron",
        }
    }

    /// Icon character for UI
    pub fn icon(&self) -> char {
        match self {
            ResourceType::Gold => 'G',
            ResourceType::Stone => 'S',
            ResourceType::Wood => 'W',
            ResourceType::Food => 'F',
            ResourceType::Iron => 'I',
        }
    }

    /// Color for UI (RGB 0-255)
    pub fn color(&self) -> [u8; 3] {
        match self {
            ResourceType::Gold => [255, 215, 0],
            ResourceType::Stone => [128, 128, 128],
            ResourceType::Wood => [139, 69, 19],
            ResourceType::Food => [50, 205, 50],
            ResourceType::Iron => [70, 70, 80],
        }
    }
}

/// Starting resources for a new game
pub const STARTING_RESOURCES: [(ResourceType, i32); 5] = [
    (ResourceType::Gold, 100),
    (ResourceType::Stone, 100),
    (ResourceType::Wood, 50),
    (ResourceType::Food, 10),
    (ResourceType::Iron, 0),
];

/// Player's resource inventory
#[derive(Debug, Clone)]
pub struct Resources {
    /// Current amounts
    amounts: HashMap<ResourceType, i32>,
    /// Maximum storage capacity (-1 = unlimited)
    max_capacity: HashMap<ResourceType, i32>,
    /// Income per day
    income: HashMap<ResourceType, i32>,
    /// Expenses per day
    expenses: HashMap<ResourceType, i32>,
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

impl Resources {
    /// Create with starting resources
    pub fn new() -> Self {
        let mut amounts = HashMap::new();
        let mut max_capacity = HashMap::new();

        for (res_type, amount) in STARTING_RESOURCES {
            amounts.insert(res_type, amount);
            max_capacity.insert(res_type, -1); // Unlimited by default
        }

        Self {
            amounts,
            max_capacity,
            income: HashMap::new(),
            expenses: HashMap::new(),
        }
    }

    /// Get current amount of a resource
    pub fn get(&self, res_type: ResourceType) -> i32 {
        self.amounts.get(&res_type).copied().unwrap_or(0)
    }

    /// Set amount of a resource
    pub fn set(&mut self, res_type: ResourceType, amount: i32) {
        let max = self.max_capacity.get(&res_type).copied().unwrap_or(-1);
        let clamped = if max >= 0 { amount.min(max) } else { amount };
        self.amounts.insert(res_type, clamped.max(0));
    }

    /// Add to a resource (returns actual amount added, respecting capacity)
    pub fn add(&mut self, res_type: ResourceType, amount: i32) -> i32 {
        let current = self.get(res_type);
        let max = self.max_capacity.get(&res_type).copied().unwrap_or(-1);

        let new_amount = current + amount;
        let clamped = if max >= 0 {
            new_amount.min(max)
        } else {
            new_amount
        };
        let actual_added = clamped - current;

        self.amounts.insert(res_type, clamped.max(0));
        actual_added
    }

    /// Remove from a resource (returns false if insufficient)
    pub fn remove(&mut self, res_type: ResourceType, amount: i32) -> bool {
        let current = self.get(res_type);
        if current >= amount {
            self.amounts.insert(res_type, current - amount);
            true
        } else {
            false
        }
    }

    /// Check if we have enough of a resource
    pub fn has(&self, res_type: ResourceType, amount: i32) -> bool {
        self.get(res_type) >= amount
    }

    /// Check if we can afford a cost
    pub fn can_afford(&self, costs: &[(ResourceType, i32)]) -> bool {
        costs.iter().all(|(res, amount)| self.has(*res, *amount))
    }

    /// Pay a cost (returns false if can't afford)
    pub fn pay(&mut self, costs: &[(ResourceType, i32)]) -> bool {
        if !self.can_afford(costs) {
            return false;
        }

        for (res, amount) in costs {
            self.remove(*res, *amount);
        }
        true
    }

    /// Set storage capacity for a resource
    pub fn set_capacity(&mut self, res_type: ResourceType, capacity: i32) {
        self.max_capacity.insert(res_type, capacity);
        // Clamp current amount if over new capacity
        if capacity >= 0 {
            let current = self.get(res_type);
            if current > capacity {
                self.set(res_type, capacity);
            }
        }
    }

    /// Get storage capacity
    pub fn get_capacity(&self, res_type: ResourceType) -> i32 {
        self.max_capacity.get(&res_type).copied().unwrap_or(-1)
    }

    /// Set daily income for a resource
    pub fn set_income(&mut self, res_type: ResourceType, amount: i32) {
        self.income.insert(res_type, amount);
    }

    /// Get daily income
    pub fn get_income(&self, res_type: ResourceType) -> i32 {
        self.income.get(&res_type).copied().unwrap_or(0)
    }

    /// Set daily expenses for a resource
    pub fn set_expenses(&mut self, res_type: ResourceType, amount: i32) {
        self.expenses.insert(res_type, amount);
    }

    /// Get daily expenses
    pub fn get_expenses(&self, res_type: ResourceType) -> i32 {
        self.expenses.get(&res_type).copied().unwrap_or(0)
    }

    /// Get net daily change
    pub fn get_net(&self, res_type: ResourceType) -> i32 {
        self.get_income(res_type) - self.get_expenses(res_type)
    }

    /// Process end of day (apply income and expenses)
    pub fn process_day_end(&mut self) -> DayReport {
        let mut report = DayReport::default();

        for res_type in [
            ResourceType::Gold,
            ResourceType::Stone,
            ResourceType::Wood,
            ResourceType::Food,
            ResourceType::Iron,
        ] {
            let income = self.get_income(res_type);
            let expenses = self.get_expenses(res_type);
            let before = self.get(res_type);

            // Add income
            let actual_income = self.add(res_type, income);

            // Remove expenses
            let can_pay = self.remove(res_type, expenses);
            let actual_expense = if can_pay {
                expenses
            } else {
                before + actual_income
            };

            let after = self.get(res_type);

            if income > 0 || expenses > 0 {
                report.changes.push(ResourceChange {
                    res_type,
                    income: actual_income,
                    expense: actual_expense,
                    before,
                    after,
                    deficit: !can_pay,
                });
            }
        }

        report
    }

    /// Get all resource amounts
    pub fn all(&self) -> impl Iterator<Item = (ResourceType, i32)> + '_ {
        self.amounts.iter().map(|(&t, &a)| (t, a))
    }
}

/// Report of resource changes after a day
#[derive(Debug, Clone, Default)]
pub struct DayReport {
    pub changes: Vec<ResourceChange>,
}

impl DayReport {
    /// Check if any resource went into deficit
    pub fn has_deficit(&self) -> bool {
        self.changes.iter().any(|c| c.deficit)
    }

    /// Get deficit resources
    pub fn deficits(&self) -> Vec<ResourceType> {
        self.changes
            .iter()
            .filter(|c| c.deficit)
            .map(|c| c.res_type)
            .collect()
    }
}

/// Change to a single resource type
#[derive(Debug, Clone)]
pub struct ResourceChange {
    pub res_type: ResourceType,
    pub income: i32,
    pub expense: i32,
    pub before: i32,
    pub after: i32,
    pub deficit: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_resources() {
        let res = Resources::new();

        assert_eq!(res.get(ResourceType::Gold), 100);
        assert_eq!(res.get(ResourceType::Stone), 100);
        assert_eq!(res.get(ResourceType::Food), 10);
    }

    #[test]
    fn test_add_remove() {
        let mut res = Resources::new();

        res.add(ResourceType::Gold, 50);
        assert_eq!(res.get(ResourceType::Gold), 150);

        assert!(res.remove(ResourceType::Gold, 100));
        assert_eq!(res.get(ResourceType::Gold), 50);

        assert!(!res.remove(ResourceType::Gold, 100)); // Not enough
        assert_eq!(res.get(ResourceType::Gold), 50); // Unchanged
    }

    #[test]
    fn test_capacity() {
        let mut res = Resources::new();

        res.set_capacity(ResourceType::Gold, 150);
        res.add(ResourceType::Gold, 100); // Would be 200, capped at 150
        assert_eq!(res.get(ResourceType::Gold), 150);
    }

    #[test]
    fn test_day_processing() {
        let mut res = Resources::new();

        res.set_income(ResourceType::Food, 5);
        res.set_expenses(ResourceType::Food, 3);

        let report = res.process_day_end();

        // 10 + 5 - 3 = 12
        assert_eq!(res.get(ResourceType::Food), 12);
        assert!(!report.has_deficit());
    }

    #[test]
    fn test_deficit() {
        let mut res = Resources::new();
        res.set(ResourceType::Food, 2);
        res.set_expenses(ResourceType::Food, 5);

        let report = res.process_day_end();

        assert!(report.has_deficit());
        assert_eq!(res.get(ResourceType::Food), 0);
    }
}
