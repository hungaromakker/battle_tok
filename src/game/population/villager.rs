//! Villager Management
//!
//! Each villager:
//! - Consumes 1 food per day
//! - Can be assigned to a role
//! - Has skill levels that improve over time

use std::collections::HashMap;

/// Roles a villager can perform
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VillagerRole {
    /// Not assigned, resting at home
    Idle,
    /// Working at a farm
    Farmer,
    /// Working at lumber mill
    Lumberjack,
    /// Working at quarry
    Stonecutter,
    /// Working at mine
    Miner,
    /// Working at market
    Merchant,
    /// Building structures
    Builder,
    /// Military unit
    Soldier,
    /// Archer unit
    Archer,
}

impl VillagerRole {
    /// Food consumption per day for this role
    pub fn food_consumption(&self) -> i32 {
        match self {
            VillagerRole::Idle => 1,
            VillagerRole::Soldier | VillagerRole::Archer => 2, // Military needs more food
            _ => 1,
        }
    }

    /// Gold upkeep per day (for military)
    pub fn gold_upkeep(&self) -> i32 {
        match self {
            VillagerRole::Soldier => 1,
            VillagerRole::Archer => 2,
            _ => 0,
        }
    }

    /// Is this a military role?
    pub fn is_military(&self) -> bool {
        matches!(self, VillagerRole::Soldier | VillagerRole::Archer)
    }

    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            VillagerRole::Idle => "Idle",
            VillagerRole::Farmer => "Farmer",
            VillagerRole::Lumberjack => "Lumberjack",
            VillagerRole::Stonecutter => "Stonecutter",
            VillagerRole::Miner => "Miner",
            VillagerRole::Merchant => "Merchant",
            VillagerRole::Builder => "Builder",
            VillagerRole::Soldier => "Soldier",
            VillagerRole::Archer => "Archer",
        }
    }
}

/// Statistics for a villager
#[derive(Debug, Clone)]
pub struct VillagerStats {
    /// Farming skill (0-100)
    pub farming: u32,
    /// Building skill (0-100)
    pub building: u32,
    /// Combat skill (0-100)
    pub combat: u32,
    /// Days since training in current role
    pub days_in_role: u32,
}

impl Default for VillagerStats {
    fn default() -> Self {
        Self {
            farming: 10,
            building: 10,
            combat: 10,
            days_in_role: 0,
        }
    }
}

impl VillagerStats {
    /// Train the villager in their current role
    pub fn train(&mut self, role: VillagerRole) {
        self.days_in_role += 1;

        // Gain skill based on role
        let skill_gain = 1;
        match role {
            VillagerRole::Farmer
            | VillagerRole::Lumberjack
            | VillagerRole::Stonecutter
            | VillagerRole::Miner
            | VillagerRole::Merchant => {
                self.farming = (self.farming + skill_gain).min(100);
            }
            VillagerRole::Builder => {
                self.building = (self.building + skill_gain).min(100);
            }
            VillagerRole::Soldier | VillagerRole::Archer => {
                self.combat = (self.combat + skill_gain).min(100);
            }
            VillagerRole::Idle => {}
        }
    }

    /// Reset days in role (when role changes)
    pub fn change_role(&mut self) {
        self.days_in_role = 0;
    }

    /// Get efficiency multiplier based on skills
    pub fn efficiency(&self, role: VillagerRole) -> f32 {
        let base_skill = match role {
            VillagerRole::Farmer
            | VillagerRole::Lumberjack
            | VillagerRole::Stonecutter
            | VillagerRole::Miner
            | VillagerRole::Merchant => self.farming,
            VillagerRole::Builder => self.building,
            VillagerRole::Soldier | VillagerRole::Archer => self.combat,
            VillagerRole::Idle => 0,
        };

        // 0.5 at skill 0, 1.5 at skill 100
        0.5 + (base_skill as f32 / 100.0)
    }
}

/// A single villager
#[derive(Debug, Clone)]
pub struct Villager {
    /// Unique ID
    pub id: u32,
    /// Current role
    pub role: VillagerRole,
    /// Stats and skills
    pub stats: VillagerStats,
    /// Which building they're assigned to (if any)
    pub assigned_building: Option<u32>,
    /// Individual morale (0-100)
    pub morale: u32,
    /// Is this villager housed?
    pub housed: bool,
}

impl Villager {
    pub fn new(id: u32) -> Self {
        Self {
            id,
            role: VillagerRole::Idle,
            stats: VillagerStats::default(),
            assigned_building: None,
            morale: 75, // Start with decent morale
            housed: false,
        }
    }

    /// Assign to a role (and optionally a building)
    pub fn assign(&mut self, role: VillagerRole, building: Option<u32>) {
        if self.role != role {
            self.stats.change_role();
        }
        self.role = role;
        self.assigned_building = building;
    }

    /// Process end of day
    pub fn end_of_day(&mut self, has_food: bool, has_housing: bool, morale_modifier: i32) {
        // Train in current role
        self.stats.train(self.role);

        // Update housing status
        self.housed = has_housing;

        // Update morale
        let mut morale_change = morale_modifier;

        if !has_food {
            morale_change -= 20; // Hungry
        }
        if !has_housing {
            morale_change -= 10; // Homeless
        }
        if self.role == VillagerRole::Idle {
            morale_change -= 5; // Unemployed
        }

        self.morale = (self.morale as i32 + morale_change).clamp(0, 100) as u32;
    }

    /// Will this villager leave? (very low morale)
    pub fn will_leave(&self) -> bool {
        self.morale < 10
    }

    /// Get food consumption for this villager
    pub fn food_consumption(&self) -> i32 {
        self.role.food_consumption()
    }

    /// Get gold upkeep for this villager
    pub fn gold_upkeep(&self) -> i32 {
        self.role.gold_upkeep()
    }
}

/// Population manager for a player
#[derive(Debug, Clone, Default)]
pub struct Population {
    /// All villagers
    villagers: Vec<Villager>,
    /// Next villager ID
    next_id: u32,
    /// Housing capacity
    housing_capacity: u32,
    /// Counts by role
    role_counts: HashMap<VillagerRole, u32>,
}

impl Population {
    pub fn new() -> Self {
        Self {
            villagers: Vec::new(),
            next_id: 1,
            housing_capacity: 0,
            role_counts: HashMap::new(),
        }
    }

    /// Add a new villager
    pub fn add_villager(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id += 1;

        let villager = Villager::new(id);
        self.update_role_count(VillagerRole::Idle, 1);
        self.villagers.push(villager);

        id
    }

    /// Remove a villager (they left due to low morale)
    pub fn remove_villager(&mut self, id: u32) -> bool {
        if let Some(idx) = self.villagers.iter().position(|v| v.id == id) {
            let villager = self.villagers.remove(idx);
            self.update_role_count(villager.role, -1);
            true
        } else {
            false
        }
    }

    /// Get villager by ID
    pub fn get(&self, id: u32) -> Option<&Villager> {
        self.villagers.iter().find(|v| v.id == id)
    }

    /// Get mutable villager by ID
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Villager> {
        self.villagers.iter_mut().find(|v| v.id == id)
    }

    /// Assign a villager to a role
    pub fn assign_role(&mut self, id: u32, role: VillagerRole, building: Option<u32>) -> bool {
        if let Some(villager) = self.get_mut(id) {
            let old_role = villager.role;
            villager.assign(role, building);

            if old_role != role {
                self.update_role_count(old_role, -1);
                self.update_role_count(role, 1);
            }
            true
        } else {
            false
        }
    }

    fn update_role_count(&mut self, role: VillagerRole, delta: i32) {
        let count = self.role_counts.entry(role).or_insert(0);
        *count = (*count as i32 + delta).max(0) as u32;
    }

    /// Get count by role
    pub fn count_by_role(&self, role: VillagerRole) -> u32 {
        self.role_counts.get(&role).copied().unwrap_or(0)
    }

    /// Get total population
    pub fn total(&self) -> u32 {
        self.villagers.len() as u32
    }

    /// Get idle (unassigned) count
    pub fn idle_count(&self) -> u32 {
        self.count_by_role(VillagerRole::Idle)
    }

    /// Get military count
    pub fn military_count(&self) -> u32 {
        self.count_by_role(VillagerRole::Soldier) + self.count_by_role(VillagerRole::Archer)
    }

    /// Get worker count (non-idle, non-military)
    pub fn worker_count(&self) -> u32 {
        self.total() - self.idle_count() - self.military_count()
    }

    /// Set housing capacity
    pub fn set_housing_capacity(&mut self, capacity: u32) {
        self.housing_capacity = capacity;
    }

    /// Get housing capacity
    pub fn housing_capacity(&self) -> u32 {
        self.housing_capacity
    }

    /// Get total food consumption per day
    pub fn total_food_consumption(&self) -> i32 {
        self.villagers.iter().map(|v| v.food_consumption()).sum()
    }

    /// Get total gold upkeep per day
    pub fn total_gold_upkeep(&self) -> i32 {
        self.villagers.iter().map(|v| v.gold_upkeep()).sum()
    }

    /// Process end of day for all villagers
    /// Returns list of villagers who left
    pub fn process_day_end(&mut self, food_available: bool, morale_modifier: i32) -> Vec<u32> {
        let housed_count = self.housing_capacity;
        let mut housed_given = 0;

        // First pass: update villagers
        for villager in &mut self.villagers {
            let has_housing = housed_given < housed_count;
            if has_housing {
                housed_given += 1;
            }

            villager.end_of_day(food_available, has_housing, morale_modifier);
        }

        // Second pass: collect those who left
        let leaving: Vec<u32> = self
            .villagers
            .iter()
            .filter(|v| v.will_leave())
            .map(|v| v.id)
            .collect();

        // Remove them
        for id in &leaving {
            self.remove_villager(*id);
        }

        leaving
    }

    /// Get average morale
    pub fn average_morale(&self) -> u32 {
        if self.villagers.is_empty() {
            return 100;
        }
        let total: u32 = self.villagers.iter().map(|v| v.morale).sum();
        total / self.villagers.len() as u32
    }

    /// Get all villagers
    pub fn all(&self) -> &[Villager] {
        &self.villagers
    }

    /// Get idle villagers
    pub fn idle_villagers(&self) -> Vec<&Villager> {
        self.villagers
            .iter()
            .filter(|v| v.role == VillagerRole::Idle)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_villager() {
        let mut pop = Population::new();

        let id = pop.add_villager();
        assert_eq!(id, 1);
        assert_eq!(pop.total(), 1);
        assert_eq!(pop.idle_count(), 1);
    }

    #[test]
    fn test_assign_role() {
        let mut pop = Population::new();
        let id = pop.add_villager();

        pop.assign_role(id, VillagerRole::Farmer, Some(1));

        assert_eq!(pop.idle_count(), 0);
        assert_eq!(pop.count_by_role(VillagerRole::Farmer), 1);

        let villager = pop.get(id).unwrap();
        assert_eq!(villager.role, VillagerRole::Farmer);
        assert_eq!(villager.assigned_building, Some(1));
    }

    #[test]
    fn test_food_consumption() {
        let mut pop = Population::new();
        pop.add_villager(); // 1 food
        pop.add_villager(); // 1 food

        let id = pop.add_villager();
        pop.assign_role(id, VillagerRole::Soldier, None); // 2 food

        assert_eq!(pop.total_food_consumption(), 4);
    }

    #[test]
    fn test_morale_leaving() {
        let mut pop = Population::new();
        let id = pop.add_villager();

        // Set very low morale directly
        pop.get_mut(id).unwrap().morale = 5;

        let leaving = pop.process_day_end(false, -10);

        assert_eq!(leaving.len(), 1);
        assert_eq!(leaving[0], id);
        assert_eq!(pop.total(), 0);
    }
}
