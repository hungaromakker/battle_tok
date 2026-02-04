//! Game State
//!
//! Central state struct that holds all game systems together.

use glam::Vec3;

use crate::game::building::{DualGrid, DragBuilder, MeshCombiner, BlockLibrary, BuildEvent};
use crate::game::economy::{Resources, DayCycle, ResourceType};
use crate::game::population::{Population, JobAI, Morale};
use crate::game::ui::TopBar;

/// Region size for mesh combining (in blocks)
const MESH_REGION_SIZE: i32 = 16;

/// Central game state holding all systems
pub struct GameState {
    // === Building System ===
    /// Dual-grid building system (Stalberg-style)
    pub grid: DualGrid,
    /// Drag-to-build controller
    pub drag_builder: DragBuilder,
    /// Mesh combiner for optimization
    pub mesh_combiner: MeshCombiner,
    /// Block template library
    pub block_library: BlockLibrary,
    /// Current building material
    pub current_material: u32,

    // === Economy System ===
    /// Player resources
    pub resources: Resources,
    /// Day/night cycle
    pub day_cycle: DayCycle,

    // === Population System ===
    /// All villagers
    pub population: Population,
    /// Job assignment AI
    pub job_ai: JobAI,
    /// Population morale
    pub morale: Morale,

    // === UI ===
    /// Top bar UI
    pub top_bar: TopBar,

    // === Game Flags ===
    /// Is the game paused?
    pub paused: bool,
    /// Has the player lost their flag?
    pub flag_captured: bool,
    /// Player's territory hexagon count
    pub territory_count: u32,
}

impl Default for GameState {
    fn default() -> Self {
        Self::new()
    }
}

impl GameState {
    /// Create a new game state with starting values
    pub fn new() -> Self {
        let mut population = Population::new();
        // Start with 1 villager
        population.add_villager();

        Self {
            // Building
            grid: DualGrid::new(),
            drag_builder: DragBuilder::new(),
            mesh_combiner: MeshCombiner::new(MESH_REGION_SIZE),
            block_library: BlockLibrary::new(),
            current_material: 0,

            // Economy
            resources: Resources::new(),
            day_cycle: DayCycle::new(),

            // Population
            population,
            job_ai: JobAI::new(),
            morale: Morale::new(),

            // UI
            top_bar: TopBar::new(),

            // Flags
            paused: false,
            flag_captured: false,
            territory_count: 1, // Start with 1 hex
        }
    }

    /// Update game state each frame
    /// Returns true if a new day started
    pub fn update(&mut self, delta_seconds: f32) -> bool {
        if self.paused {
            return false;
        }

        // Update day cycle
        let new_day = self.day_cycle.update(delta_seconds);

        if new_day {
            self.process_day_end();
        }

        new_day
    }

    /// Process end of day: resources, population, morale
    pub fn process_day_end(&mut self) {
        // Calculate food expenses from population
        let food_consumption = self.population.total_food_consumption();
        self.resources.set_expenses(ResourceType::Food, food_consumption);

        // Calculate gold expenses from military upkeep
        let gold_upkeep = self.population.total_gold_upkeep();
        self.resources.set_expenses(ResourceType::Gold, gold_upkeep);

        // Process resources
        let report = self.resources.process_day_end();

        // Calculate morale modifiers
        let mut morale_mod = 0i32;

        // Flag captured = big morale hit
        if self.flag_captured {
            morale_mod -= 30;
        }

        // Food deficit = morale hit
        if report.has_deficit() {
            morale_mod -= 15;
        }

        // Surplus food = morale boost
        let food_net = self.resources.get_net(ResourceType::Food);
        if food_net > 0 {
            morale_mod += 5;
        }

        // Process population (villagers may leave)
        let food_available = !report.deficits().contains(&ResourceType::Food);
        let leaving = self.population.process_day_end(food_available, morale_mod);

        // Log if anyone left
        if !leaving.is_empty() {
            // In a real game, we'd show a notification
            eprintln!("{} villager(s) left due to low morale!", leaving.len());
        }

        // Note: Job AI auto_assign requires buildings array which we'll integrate later
        // For now, we analyze resources to update priorities
        self.job_ai.analyze_resources(&self.resources);
    }

    /// Toggle pause state
    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
        self.day_cycle.set_paused(self.paused);
    }

    /// Handle mouse drag start for building
    pub fn start_build(&mut self, world_pos: Vec3) -> BuildEvent {
        self.drag_builder.start_drag(world_pos)
    }

    /// Handle mouse drag update for building
    pub fn update_build(&mut self, world_pos: Vec3) -> Option<BuildEvent> {
        self.drag_builder.update_drag(world_pos)
    }

    /// Handle mouse drag end for building
    pub fn end_build(&mut self) -> BuildEvent {
        self.drag_builder.end_drag(&mut self.grid)
    }

    /// Check if player can afford a building cost
    pub fn can_afford(&self, costs: &[(ResourceType, i32)]) -> bool {
        self.resources.can_afford(costs)
    }

    /// Pay for a building
    pub fn pay_for_building(&mut self, costs: &[(ResourceType, i32)]) -> bool {
        self.resources.pay(costs)
    }

    /// Get current morale level description
    pub fn morale_description(&self) -> &'static str {
        let avg = self.population.average_morale();
        match avg {
            0..=20 => "Desperate",
            21..=40 => "Unhappy",
            41..=60 => "Content",
            61..=80 => "Happy",
            _ => "Thriving",
        }
    }

    /// Add a new villager (e.g., from immigration or birth)
    pub fn add_villager(&mut self) -> u32 {
        self.population.add_villager()
    }

    /// Set time scale (for debugging or fast-forward)
    pub fn set_time_scale(&mut self, scale: f32) {
        self.day_cycle.set_time_scale(scale);
    }

    /// Set current building material
    pub fn set_material(&mut self, material_id: u32) {
        self.current_material = material_id;
    }

    /// Get references for UI rendering
    pub fn ui_data(&self) -> (&Resources, &DayCycle, &Population) {
        (&self.resources, &self.day_cycle, &self.population)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_game_state() {
        let state = GameState::new();

        assert_eq!(state.population.total(), 1);
        assert_eq!(state.day_cycle.day(), 1);
        assert!(!state.paused);
        assert!(!state.flag_captured);
    }

    #[test]
    fn test_toggle_pause() {
        let mut state = GameState::new();

        assert!(!state.paused);
        state.toggle_pause();
        assert!(state.paused);
        state.toggle_pause();
        assert!(!state.paused);
    }

    #[test]
    fn test_update_advances_time() {
        let mut state = GameState::new();
        let initial_time = state.day_cycle.time();

        state.update(1.0); // 1 second

        assert!(state.day_cycle.time() > initial_time);
    }
}
