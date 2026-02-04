//! Morale System
//!
//! Morale affects villager productivity and whether they stay.
//! Key morale factors:
//! - Flag visibility (if enemy captures flag, morale drops)
//! - Food availability
//! - Housing
//! - Military strength

/// Morale state levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoraleState {
    /// 0-25: Very unhappy, will leave
    Desperate,
    /// 25-50: Unhappy, reduced productivity
    Low,
    /// 50-75: Content, normal productivity
    Normal,
    /// 75-90: Happy, increased productivity
    High,
    /// 90-100: Ecstatic, maximum productivity
    Ecstatic,
}

impl MoraleState {
    /// Get state from morale value
    pub fn from_value(morale: u32) -> Self {
        match morale {
            0..=24 => MoraleState::Desperate,
            25..=49 => MoraleState::Low,
            50..=74 => MoraleState::Normal,
            75..=89 => MoraleState::High,
            _ => MoraleState::Ecstatic,
        }
    }

    /// Productivity multiplier
    pub fn productivity_multiplier(&self) -> f32 {
        match self {
            MoraleState::Desperate => 0.25,
            MoraleState::Low => 0.5,
            MoraleState::Normal => 1.0,
            MoraleState::High => 1.25,
            MoraleState::Ecstatic => 1.5,
        }
    }

    /// Display name
    pub fn name(&self) -> &'static str {
        match self {
            MoraleState::Desperate => "Desperate",
            MoraleState::Low => "Unhappy",
            MoraleState::Normal => "Content",
            MoraleState::High => "Happy",
            MoraleState::Ecstatic => "Ecstatic",
        }
    }

    /// Color for UI
    pub fn color(&self) -> [u8; 3] {
        match self {
            MoraleState::Desperate => [255, 0, 0],     // Red
            MoraleState::Low => [255, 165, 0],        // Orange
            MoraleState::Normal => [255, 255, 0],     // Yellow
            MoraleState::High => [144, 238, 144],     // Light green
            MoraleState::Ecstatic => [0, 255, 0],     // Green
        }
    }
}

/// Types of morale modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoraleModifier {
    /// Flag is visible and safe
    FlagSafe,
    /// Flag was captured by enemy
    FlagCaptured,
    /// Enemy captured flag is in your base
    EnemyFlagCaptured,
    /// Lost a battle
    BattleLost,
    /// Won a battle
    BattleWon,
    /// Food shortage
    FoodShortage,
    /// Plenty of food
    FoodSurplus,
    /// Not enough housing
    HousingShortage,
    /// Comfortable housing
    HousingSurplus,
    /// Strong military
    StrongMilitary,
    /// Weak military
    WeakMilitary,
    /// Tax rate high
    HighTax,
    /// Tax rate low
    LowTax,
}

impl MoraleModifier {
    /// Daily morale change from this modifier
    pub fn daily_effect(&self) -> i32 {
        match self {
            MoraleModifier::FlagSafe => 5,
            MoraleModifier::FlagCaptured => -30,
            MoraleModifier::EnemyFlagCaptured => 10,
            MoraleModifier::BattleLost => -15,
            MoraleModifier::BattleWon => 10,
            MoraleModifier::FoodShortage => -20,
            MoraleModifier::FoodSurplus => 5,
            MoraleModifier::HousingShortage => -10,
            MoraleModifier::HousingSurplus => 3,
            MoraleModifier::StrongMilitary => 5,
            MoraleModifier::WeakMilitary => -5,
            MoraleModifier::HighTax => -10,
            MoraleModifier::LowTax => 3,
        }
    }

    /// Description for tooltip
    pub fn description(&self) -> &'static str {
        match self {
            MoraleModifier::FlagSafe => "Your flag is safe",
            MoraleModifier::FlagCaptured => "The enemy has your flag!",
            MoraleModifier::EnemyFlagCaptured => "You have the enemy's flag!",
            MoraleModifier::BattleLost => "Lost a recent battle",
            MoraleModifier::BattleWon => "Won a recent battle",
            MoraleModifier::FoodShortage => "Not enough food",
            MoraleModifier::FoodSurplus => "Plenty of food",
            MoraleModifier::HousingShortage => "Not enough housing",
            MoraleModifier::HousingSurplus => "Comfortable housing",
            MoraleModifier::StrongMilitary => "Strong military presence",
            MoraleModifier::WeakMilitary => "Weak military",
            MoraleModifier::HighTax => "High taxes",
            MoraleModifier::LowTax => "Low taxes",
        }
    }
}

/// Kingdom-wide morale tracker
#[derive(Debug, Clone)]
pub struct Morale {
    /// Base morale value (50 = neutral)
    base: u32,
    /// Active modifiers
    modifiers: Vec<MoraleModifier>,
    /// Current computed morale
    current: u32,
    /// Morale trend (positive = improving)
    trend: i32,
}

impl Default for Morale {
    fn default() -> Self {
        Self::new()
    }
}

impl Morale {
    pub fn new() -> Self {
        Self {
            base: 75, // Start with decent morale
            modifiers: vec![MoraleModifier::FlagSafe],
            current: 75,
            trend: 0,
        }
    }

    /// Add a modifier
    pub fn add_modifier(&mut self, modifier: MoraleModifier) {
        // Remove opposite modifiers
        match modifier {
            MoraleModifier::FlagSafe => {
                self.modifiers.retain(|m| !matches!(m, MoraleModifier::FlagCaptured));
            }
            MoraleModifier::FlagCaptured => {
                self.modifiers.retain(|m| !matches!(m, MoraleModifier::FlagSafe));
            }
            MoraleModifier::FoodShortage => {
                self.modifiers.retain(|m| !matches!(m, MoraleModifier::FoodSurplus));
            }
            MoraleModifier::FoodSurplus => {
                self.modifiers.retain(|m| !matches!(m, MoraleModifier::FoodShortage));
            }
            MoraleModifier::HousingShortage => {
                self.modifiers.retain(|m| !matches!(m, MoraleModifier::HousingSurplus));
            }
            MoraleModifier::HousingSurplus => {
                self.modifiers.retain(|m| !matches!(m, MoraleModifier::HousingShortage));
            }
            _ => {}
        }

        if !self.modifiers.contains(&modifier) {
            self.modifiers.push(modifier);
        }

        self.recalculate();
    }

    /// Remove a modifier
    pub fn remove_modifier(&mut self, modifier: MoraleModifier) {
        self.modifiers.retain(|m| *m != modifier);
        self.recalculate();
    }

    /// Clear temporary modifiers (battle results)
    pub fn clear_temporary(&mut self) {
        self.modifiers.retain(|m| !matches!(m,
            MoraleModifier::BattleLost | MoraleModifier::BattleWon
        ));
        self.recalculate();
    }

    /// Recalculate morale from modifiers
    fn recalculate(&mut self) {
        let total_effect: i32 = self.modifiers.iter().map(|m| m.daily_effect()).sum();
        self.trend = total_effect;
    }

    /// Process end of day - apply morale changes
    pub fn process_day_end(&mut self) -> i32 {
        let change = self.trend / 5; // Dampen daily changes
        self.current = (self.current as i32 + change).clamp(0, 100) as u32;
        change
    }

    /// Get current morale
    pub fn value(&self) -> u32 {
        self.current
    }

    /// Get morale state
    pub fn state(&self) -> MoraleState {
        MoraleState::from_value(self.current)
    }

    /// Get trend direction
    pub fn trend(&self) -> i32 {
        self.trend
    }

    /// Get active modifiers
    pub fn modifiers(&self) -> &[MoraleModifier] {
        &self.modifiers
    }

    /// Set morale directly (for game events)
    pub fn set(&mut self, value: u32) {
        self.current = value.min(100);
    }

    /// Apply a one-time morale change
    pub fn apply_change(&mut self, amount: i32) {
        self.current = (self.current as i32 + amount).clamp(0, 100) as u32;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_morale_state() {
        assert_eq!(MoraleState::from_value(0), MoraleState::Desperate);
        assert_eq!(MoraleState::from_value(50), MoraleState::Normal);
        assert_eq!(MoraleState::from_value(100), MoraleState::Ecstatic);
    }

    #[test]
    fn test_modifiers() {
        let mut morale = Morale::new();

        morale.add_modifier(MoraleModifier::FoodShortage);
        assert!(morale.trend() < 0);

        morale.add_modifier(MoraleModifier::FoodSurplus);
        // Should replace shortage
        assert!(!morale.modifiers().contains(&MoraleModifier::FoodShortage));
    }

    #[test]
    fn test_flag_capture() {
        let mut morale = Morale::new();
        let initial = morale.value();

        morale.add_modifier(MoraleModifier::FlagCaptured);

        // Process several days
        for _ in 0..5 {
            morale.process_day_end();
        }

        assert!(morale.value() < initial);
    }
}
