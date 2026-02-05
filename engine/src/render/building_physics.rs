//! Building Physics System
//!
//! Handles physics simulation for building blocks:
//! - Gravity and falling for unsupported blocks
//! - Structural integrity checks
//! - Cascade collapse when support is removed
//! - Disintegration of unstable structures

use glam::Vec3;
use std::collections::{HashMap, HashSet};

use super::building_blocks::{BuildingBlockManager, BuildingBlockShape, AABB};

/// Physics state for a building block
#[derive(Debug, Clone)]
pub struct BlockPhysicsState {
    /// Current velocity (m/s)
    pub velocity: Vec3,
    /// Is the block resting on something solid?
    pub grounded: bool,
    /// Is this block being held by structural integrity?
    pub structurally_supported: bool,
    /// Time since block started falling (for disintegration timer)
    pub fall_time: f32,
    /// Should this block disintegrate?
    pub should_disintegrate: bool,
    /// Angular velocity for tumbling during fall (rad/s)
    pub angular_velocity: Vec3,
    /// Accumulated rotation during fall (radians)
    pub fall_rotation: Vec3,
    
    // === New force-based physics fields ===
    
    /// Accumulated force this frame (Newtons) - reset each frame after applying
    pub accumulated_force: Vec3,
    /// Highest impact force received (Newtons) - used for break threshold check
    pub peak_impact: f32,
    /// Is the block detached from structure and can be picked up?
    pub is_loose: bool,
    /// Block mass in kg (calculated from volume * material density)
    pub mass: f32,
    /// Current roll axis for spheres/cylinders (None = sliding, not rolling)
    pub rolling_axis: Option<Vec3>,
    /// Progress through a tumble for cubes (0.0-1.0, triggers 90° rotation)
    pub tumble_progress: f32,
    /// Material index for physics properties lookup
    pub material_index: u8,
}

impl Default for BlockPhysicsState {
    fn default() -> Self {
        Self {
            velocity: Vec3::ZERO,
            grounded: true,
            structurally_supported: true,
            fall_time: 0.0,
            should_disintegrate: false,
            angular_velocity: Vec3::ZERO,
            fall_rotation: Vec3::ZERO,
            // New fields
            accumulated_force: Vec3::ZERO,
            peak_impact: 0.0,
            is_loose: false,
            mass: 10.0, // Default mass, will be recalculated on register
            rolling_axis: None,
            tumble_progress: 0.0,
            material_index: 0,
        }
    }
}

impl BlockPhysicsState {
    /// Apply an impulse (instantaneous force) to the block
    /// impulse = force * dt, so velocity change = impulse / mass
    pub fn apply_impulse(&mut self, impulse: Vec3) {
        if self.mass > 0.0 {
            self.velocity += impulse / self.mass;
        }
    }
    
    /// Apply a continuous force (will be integrated over dt in update)
    pub fn apply_force(&mut self, force: Vec3) {
        self.accumulated_force += force;
    }
    
    /// Record an impact force for break threshold checking
    pub fn record_impact(&mut self, force_magnitude: f32) {
        self.peak_impact = self.peak_impact.max(force_magnitude);
    }
    
    /// Reset per-frame values (call at end of physics update)
    pub fn reset_frame(&mut self) {
        self.accumulated_force = Vec3::ZERO;
    }
}

/// Configuration for the physics simulation
#[derive(Debug, Clone)]
pub struct PhysicsConfig {
    /// Gravity acceleration (m/s²)
    pub gravity: f32,
    /// Ground level Y coordinate
    pub ground_level: f32,
    /// Maximum number of horizontal neighbors required for structural support
    pub min_neighbors_for_support: u32,
    /// Maximum cantilever distance (blocks can extend this far horizontally without support below)
    pub max_cantilever: u32,
    /// Time before an unsupported block starts to fall (seconds)
    pub support_check_delay: f32,
    /// Time a falling block takes to disintegrate (seconds)
    pub disintegration_time: f32,
    /// Velocity damping factor when hitting ground (restitution)
    pub bounce_damping: f32,
    /// Minimum velocity to stop bouncing
    pub velocity_threshold: f32,
    /// Default static friction coefficient (used if material not specified)
    pub default_friction_static: f32,
    /// Default dynamic friction coefficient
    pub default_friction_dynamic: f32,
    /// Air resistance coefficient
    pub air_drag: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            gravity: 9.81,
            ground_level: 0.0,
            min_neighbors_for_support: 2,
            max_cantilever: 2,
            support_check_delay: 0.1,
            disintegration_time: 2.0,
            bounce_damping: 0.3,
            velocity_threshold: 0.1,
            default_friction_static: 0.6,
            default_friction_dynamic: 0.4,
            air_drag: 0.01,
        }
    }
}

/// Get friction coefficients for a material index
/// Returns (static_friction, dynamic_friction)
pub fn get_friction_coefficients(material_index: u8) -> (f32, f32) {
    // Material friction values (indexed by u8 material)
    // 0=Stone, 1=Wood, 2=StoneDark, 3=Sandstone, 4=Slate, 5=Brick, 6=Moss, 7=Metal, 8=Marble, 9=Obsidian
    match material_index {
        0 => (0.70, 0.50), // Stone Gray - high friction
        1 => (0.50, 0.40), // Wood Brown - medium friction
        2 => (0.75, 0.55), // Stone Dark - very high friction
        3 => (0.60, 0.45), // Sandstone - medium-high
        4 => (0.55, 0.40), // Slate - medium
        5 => (0.65, 0.50), // Brick Red - high
        6 => (0.80, 0.60), // Moss Green - very high (soft)
        7 => (0.30, 0.20), // Metal Gray - low friction (slippery)
        8 => (0.40, 0.30), // Marble White - low-medium
        9 => (0.35, 0.25), // Obsidian - low (smooth glass)
        _ => (0.60, 0.40), // Default
    }
}

/// Get break threshold for a material (Newtons)
pub fn get_break_threshold(material_index: u8) -> f32 {
    match material_index {
        0 => 5000.0,  // Stone Gray
        1 => 1500.0,  // Wood Brown - weakest
        2 => 6000.0,  // Stone Dark - stronger
        3 => 2500.0,  // Sandstone - weak stone
        4 => 3500.0,  // Slate
        5 => 3000.0,  // Brick Red
        6 => 800.0,   // Moss Green - very weak
        7 => 10000.0, // Metal Gray - strongest
        8 => 4000.0,  // Marble White
        9 => 2000.0,  // Obsidian - brittle
        _ => 3000.0,  // Default
    }
}

/// Get material density (kg/m³)
pub fn get_material_density(material_index: u8) -> f32 {
    match material_index {
        0 => 2500.0, // Stone Gray
        1 => 600.0,  // Wood Brown - light
        2 => 2700.0, // Stone Dark
        3 => 2200.0, // Sandstone
        4 => 2800.0, // Slate - dense
        5 => 1900.0, // Brick Red
        6 => 500.0,  // Moss Green - very light
        7 => 7800.0, // Metal Gray - heavy
        8 => 2700.0, // Marble White
        9 => 2400.0, // Obsidian
        _ => 2000.0, // Default
    }
}

/// Building physics simulation system
pub struct BuildingPhysics {
    /// Physics state per block (indexed by block ID)
    states: HashMap<u32, BlockPhysicsState>,
    /// Configuration
    pub config: PhysicsConfig,
    /// Blocks pending support check (ID, time_remaining)
    pending_checks: HashMap<u32, f32>,
    /// Blocks that need to be removed after physics update
    blocks_to_remove: Vec<u32>,
    /// Cached support graph (which blocks support which)
    support_graph: HashMap<u32, Vec<u32>>,
    /// Is the support graph dirty?
    graph_dirty: bool,
}

impl Default for BuildingPhysics {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildingPhysics {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
            config: PhysicsConfig::default(),
            pending_checks: HashMap::new(),
            blocks_to_remove: Vec::new(),
            support_graph: HashMap::new(),
            graph_dirty: true,
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: PhysicsConfig) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }

    /// Register a new block in the physics system
    /// Block starts as unsupported - support check will determine actual state
    pub fn register_block(&mut self, block_id: u32) {
        let mut state = BlockPhysicsState::default();
        // New blocks start as NOT supported - the support check will verify
        state.grounded = false;
        state.structurally_supported = false;
        self.states.insert(block_id, state);
        self.graph_dirty = true;
        // Schedule initial support check (very short delay to verify support)
        self.pending_checks.insert(block_id, 0.05); // Check support quickly
    }
    
    /// Register a new block with material and volume for mass calculation
    /// Block starts as unsupported - support check will determine actual state
    pub fn register_block_with_physics(&mut self, block_id: u32, material_index: u8, volume: f32, density: f32) {
        let mut state = BlockPhysicsState::default();
        state.material_index = material_index;
        state.mass = volume * density;
        // New blocks start as NOT supported - the support check will verify
        state.grounded = false;
        state.structurally_supported = false;
        self.states.insert(block_id, state);
        self.graph_dirty = true;
        // Schedule initial support check (very short delay to verify support)
        self.pending_checks.insert(block_id, 0.05);
    }
    
    /// Register a block that is known to be at ground level (grounded immediately)
    pub fn register_grounded_block(&mut self, block_id: u32) {
        let mut state = BlockPhysicsState::default();
        state.grounded = true;
        state.structurally_supported = true;
        self.states.insert(block_id, state);
        self.graph_dirty = true;
    }
    
    /// Apply an impulse to a block (e.g., from projectile hit or player push)
    pub fn apply_impulse(&mut self, block_id: u32, impulse: Vec3) {
        if let Some(state) = self.states.get_mut(&block_id) {
            state.apply_impulse(impulse);
            // Record impact magnitude
            let impact_force = impulse.length() / 0.016; // Approximate force from impulse (assuming ~60fps)
            state.record_impact(impact_force);
        }
    }
    
    /// Apply a continuous force to a block
    pub fn apply_force(&mut self, block_id: u32, force: Vec3) {
        if let Some(state) = self.states.get_mut(&block_id) {
            state.apply_force(force);
        }
    }
    
    /// Check if a block is loose (detached and pickable)
    pub fn is_loose(&self, block_id: u32) -> bool {
        self.states.get(&block_id).map(|s| s.is_loose).unwrap_or(false)
    }
    
    /// Get peak impact force for a block
    pub fn get_peak_impact(&self, block_id: u32) -> f32 {
        self.states.get(&block_id).map(|s| s.peak_impact).unwrap_or(0.0)
    }

    /// Unregister a block from the physics system
    pub fn unregister_block(&mut self, block_id: u32) {
        self.states.remove(&block_id);
        self.pending_checks.remove(&block_id);
        self.graph_dirty = true;
    }

    /// Get physics state for a block
    pub fn get_state(&self, block_id: u32) -> Option<&BlockPhysicsState> {
        self.states.get(&block_id)
    }

    /// Get mutable physics state for a block
    pub fn get_state_mut(&mut self, block_id: u32) -> Option<&mut BlockPhysicsState> {
        self.states.get_mut(&block_id)
    }

    /// Mark the support graph as needing recalculation
    pub fn invalidate_support_graph(&mut self) {
        self.graph_dirty = true;
    }
    
    /// Schedule immediate support check for a block (e.g., after collision/impact)
    pub fn trigger_support_check(&mut self, block_id: u32) {
        // Schedule immediate check and invalidate graph
        self.pending_checks.insert(block_id, 0.0);
        self.graph_dirty = true;
    }
    
    /// Schedule support checks for multiple blocks (e.g., area of impact)
    pub fn trigger_area_support_check(&mut self, block_ids: &[u32]) {
        self.graph_dirty = true;
        for &id in block_ids {
            self.pending_checks.insert(id, 0.0);
        }
    }

    /// Update the support graph based on current block positions
    fn update_support_graph(&mut self, manager: &BuildingBlockManager) {
        if !self.graph_dirty {
            return;
        }

        self.support_graph.clear();
        let blocks = manager.blocks();

        // For each block, find which blocks it's resting on
        for block in blocks {
            let aabb = block.aabb();
            let mut supporters: Vec<u32> = Vec::new();

            // Check for blocks directly below this one
            let check_aabb = AABB::new(
                Vec3::new(aabb.min.x + 0.01, aabb.min.y - 0.1, aabb.min.z + 0.01),
                Vec3::new(aabb.max.x - 0.01, aabb.min.y, aabb.max.z - 0.01),
            );

            for other in blocks {
                if other.id != block.id {
                    let other_aabb = other.aabb();
                    if check_aabb.intersects(&other_aabb) {
                        supporters.push(other.id);
                    }
                }
            }

            self.support_graph.insert(block.id, supporters);
        }

        self.graph_dirty = false;
    }

    /// Check if a block has structural support
    fn has_support(&self, block_id: u32, manager: &BuildingBlockManager) -> bool {
        let Some(block) = manager.get_block(block_id) else {
            return false;
        };

        let aabb = block.aabb();

        // Ground check - if block is at or below ground level, it's supported
        if aabb.min.y <= self.config.ground_level + 0.01 {
            return true;
        }

        // Check direct support from below (from support graph)
        if let Some(supporters) = self.support_graph.get(&block_id) {
            if !supporters.is_empty() {
                // Has something below, check if those supports are valid
                for &supporter_id in supporters {
                    if let Some(state) = self.states.get(&supporter_id) {
                        if state.grounded || state.structurally_supported {
                            return true;
                        }
                    }
                }
            }
        }

        // Check horizontal neighbors for structural integrity
        let blocks = manager.blocks();
        let mut neighbor_count = 0;
        let mut supported_neighbors = 0;

        for other in blocks {
            if other.id == block_id {
                continue;
            }

            let other_aabb = other.aabb();

            // Check if horizontally adjacent (not above/below)
            let h_overlap_x = aabb.max.x > other_aabb.min.x && aabb.min.x < other_aabb.max.x;
            let h_overlap_z = aabb.max.z > other_aabb.min.z && aabb.min.z < other_aabb.max.z;
            let v_overlap = aabb.max.y > other_aabb.min.y + 0.01 && aabb.min.y < other_aabb.max.y - 0.01;

            // Check if touching horizontally
            let touching_x = (aabb.max.x - other_aabb.min.x).abs() < 0.05
                || (aabb.min.x - other_aabb.max.x).abs() < 0.05;
            let touching_z = (aabb.max.z - other_aabb.min.z).abs() < 0.05
                || (aabb.min.z - other_aabb.max.z).abs() < 0.05;

            let is_horizontal_neighbor = v_overlap
                && ((touching_x && h_overlap_z) || (touching_z && h_overlap_x));

            if is_horizontal_neighbor {
                neighbor_count += 1;

                if let Some(state) = self.states.get(&other.id) {
                    if state.grounded || state.structurally_supported {
                        supported_neighbors += 1;
                    }
                }
            }
        }

        // Structural integrity: need enough supported neighbors
        supported_neighbors >= self.config.min_neighbors_for_support as i32
    }

    /// Perform cascade support check - when one block loses support, check dependents
    fn cascade_support_check(&mut self, starting_block_id: u32, manager: &BuildingBlockManager) {
        let mut to_check: Vec<u32> = vec![starting_block_id];
        let mut checked: HashSet<u32> = HashSet::new();

        while let Some(block_id) = to_check.pop() {
            if checked.contains(&block_id) {
                continue;
            }
            checked.insert(block_id);

            let has_support = self.has_support(block_id, manager);

            if let Some(state) = self.states.get_mut(&block_id) {
                let was_supported = state.grounded || state.structurally_supported;
                state.structurally_supported = has_support;

                if was_supported && !has_support {
                    state.grounded = false;
                    // This block lost support - check all blocks that might depend on it
                    for (other_id, supporters) in self.support_graph.iter() {
                        if supporters.contains(&block_id) && !checked.contains(other_id) {
                            to_check.push(*other_id);
                        }
                    }
                }
            }
        }
    }

    /// Update physics simulation
    pub fn update(&mut self, dt: f32, manager: &mut BuildingBlockManager) {
        // Update support graph if needed
        self.update_support_graph(manager);

        // Process pending support checks
        let mut completed_checks: Vec<u32> = Vec::new();
        for (block_id, time_remaining) in self.pending_checks.iter_mut() {
            *time_remaining -= dt;
            if *time_remaining <= 0.0 {
                completed_checks.push(*block_id);
            }
        }

        for block_id in completed_checks {
            self.pending_checks.remove(&block_id);
            self.cascade_support_check(block_id, manager);
        }

        // Collect blocks that need physics update
        let block_ids: Vec<u32> = self.states.keys().copied().collect();
        
        // Collect position updates to apply after state processing
        let mut position_updates: Vec<(u32, Vec3)> = Vec::new();
        let mut blocks_for_collision_check: Vec<(u32, BuildingBlockShape)> = Vec::new();

        for block_id in block_ids {
            // Get block data we need (copy to avoid borrow issues)
            let (position, aabb, shape) = {
                let Some(block) = manager.get_block(block_id) else {
                    continue;
                };
                (block.position, block.aabb(), block.shape)
            };

            // Process state updates in a scope to release the borrow
            let (new_position, should_check_collisions) = {
                // Get current state
                let state = match self.states.get_mut(&block_id) {
                    Some(s) => s,
                    None => continue,
                };
                
                // Get material-specific friction
                let (friction_static, friction_dynamic) = get_friction_coefficients(state.material_index);

                // === FORCE INTEGRATION ===
                // Apply accumulated forces: F = ma → a = F/m → v += a*dt
                if state.mass > 0.0 && state.accumulated_force.length_squared() > 0.001 {
                    let acceleration = state.accumulated_force / state.mass;
                    state.velocity += acceleration * dt;
                }
                
                // === FRICTION CALCULATION ===
                // When grounded, apply friction opposing horizontal motion
                if state.grounded {
                    let horizontal_velocity = Vec3::new(state.velocity.x, 0.0, state.velocity.z);
                    let speed = horizontal_velocity.length();
                    
                    if speed > 0.001 {
                        // Normal force = mass * gravity (on flat ground)
                        let normal_force = state.mass * self.config.gravity;
                        
                        // Use static friction if nearly stationary, dynamic otherwise
                        let friction_coeff = if speed < 0.1 {
                            friction_static
                        } else {
                            friction_dynamic
                        };
                        
                        // Friction force magnitude = μ * N
                        let friction_magnitude = friction_coeff * normal_force;
                        
                        // Friction acceleration = F / m = μ * g
                        let friction_accel = friction_magnitude / state.mass;
                        
                        // Friction opposes motion direction
                        let friction_dir = -horizontal_velocity.normalize();
                        
                        // Calculate velocity reduction (capped to not reverse direction)
                        let velocity_reduction = friction_accel * dt;
                        
                        if velocity_reduction >= speed {
                            // Friction stops the block completely
                            state.velocity.x = 0.0;
                            state.velocity.z = 0.0;
                        } else {
                            // Apply friction deceleration
                            state.velocity += friction_dir * velocity_reduction;
                        }
                    }
                }
                
                // === AIR DRAG ===
                // Apply air resistance (quadratic drag: F = -kv²)
                if !state.grounded {
                    let speed_sq = state.velocity.length_squared();
                    if speed_sq > 0.01 {
                        let drag_force = self.config.air_drag * speed_sq;
                        let drag_accel = drag_force / state.mass.max(1.0);
                        let drag_dir = -state.velocity.normalize();
                        state.velocity += drag_dir * drag_accel * dt;
                    }
                }

                // Skip further processing if grounded and stable (after friction applied)
                if state.grounded && state.velocity.length() < self.config.velocity_threshold {
                    state.velocity = Vec3::ZERO;
                    state.reset_frame();
                    continue;
                }

                // Check support - block starts falling if not supported
                if !state.grounded && !state.structurally_supported {
                    // Apply gravity
                    state.velocity.y -= self.config.gravity * dt;

                    // Add slight angular velocity for tumbling effect
                    if state.fall_time < 0.1 {
                        state.angular_velocity = Vec3::new(
                            (block_id as f32 * 0.1).sin() * 2.0,
                            (block_id as f32 * 0.2).cos() * 1.0,
                            (block_id as f32 * 0.15).sin() * 2.0,
                        );
                    }

                    state.fall_rotation += state.angular_velocity * dt;
                    state.fall_time += dt;

                    // Check for disintegration
                    if state.fall_time > self.config.disintegration_time {
                        state.should_disintegrate = true;
                        self.blocks_to_remove.push(block_id);
                    }
                }
                
                // Apply velocity to get new position
                let new_position = position + state.velocity * dt;

                // Ground collision
                let block_bottom = match shape {
                    BuildingBlockShape::Cube { half_extents } => new_position.y - half_extents.y,
                    BuildingBlockShape::Sphere { radius } => new_position.y - radius,
                    _ => aabb.min.y + (new_position.y - position.y),
                };

                let final_position = if block_bottom <= self.config.ground_level {
                    // Calculate impact force: F = m * Δv / Δt
                    let impact_velocity = state.velocity.y.abs();
                    let impact_force = state.mass * impact_velocity / dt.max(0.001);
                    state.record_impact(impact_force);
                    
                    // Mark as loose if it was falling (detached from structure)
                    if !state.structurally_supported && state.fall_time > 0.1 {
                        state.is_loose = true;
                    }
                    
                    // Hit ground
                    state.grounded = true;
                    state.structurally_supported = true;
                    
                    // Bounce
                    state.velocity.y = -state.velocity.y * self.config.bounce_damping;
                    
                    // Apply friction to horizontal velocity
                    state.velocity.x *= 1.0 - friction_dynamic;
                    state.velocity.z *= 1.0 - friction_dynamic;
                    state.angular_velocity *= 0.5;

                    // Snap to ground if velocity is low
                    if state.velocity.length() < self.config.velocity_threshold {
                        state.velocity = Vec3::ZERO;
                        state.angular_velocity = Vec3::ZERO;
                        state.fall_time = 0.0;
                    }

                    // Adjust position to be on ground
                    let ground_offset = self.config.ground_level - block_bottom;
                    Vec3::new(new_position.x, new_position.y + ground_offset, new_position.z)
                } else {
                    new_position
                };
                
                // Reset frame-specific values
                state.reset_frame();
                
                (final_position, true)
            };
            
            // Queue position update
            position_updates.push((block_id, new_position));
            
            // Queue collision check
            if should_check_collisions {
                blocks_for_collision_check.push((block_id, shape));
            }
        }
        
        // Apply position updates to manager
        for (block_id, new_pos) in position_updates {
            if let Some(block_mut) = manager.get_block_mut(block_id) {
                block_mut.position = new_pos;
            }
        }
        
        // Perform collision checks and rolling updates
        for (block_id, shape) in blocks_for_collision_check {
            self.check_block_collisions(block_id, manager);
            self.update_rolling_behavior(block_id, &shape, dt);
        }

        // Invalidate graph since positions may have changed
        if !self.blocks_to_remove.is_empty() {
            self.graph_dirty = true;
        }
    }

    /// Check and resolve collisions between falling blocks and static ones
    fn check_block_collisions(&mut self, block_id: u32, manager: &mut BuildingBlockManager) {
        let Some(block) = manager.get_block(block_id) else {
            return;
        };

        let aabb = block.aabb();
        let position = block.position;

        let state = match self.states.get(&block_id) {
            Some(s) => s.clone(),
            None => return,
        };

        // Only check if this block is falling
        if state.grounded || state.velocity.length_squared() < 0.001 {
            return;
        }

        let blocks = manager.blocks();

        for other in blocks {
            if other.id == block_id {
                continue;
            }

            // Only collide with grounded/supported blocks
            if let Some(other_state) = self.states.get(&other.id) {
                if !other_state.grounded && !other_state.structurally_supported {
                    continue;
                }
            }

            let other_aabb = other.aabb();

            if aabb.intersects(&other_aabb) {
                // Collision detected - resolve by moving block up
                let overlap_y = aabb.min.y - other_aabb.max.y;

                if overlap_y < 0.0 && overlap_y > -0.5 {
                    // Landing on top of another block
                    if let Some(block_mut) = manager.get_block_mut(block_id) {
                        block_mut.position.y -= overlap_y + 0.01;
                    }

                    if let Some(state_mut) = self.states.get_mut(&block_id) {
                        state_mut.grounded = true;
                        state_mut.structurally_supported = true;
                        state_mut.velocity = Vec3::ZERO;
                        state_mut.angular_velocity = Vec3::ZERO;
                        state_mut.fall_time = 0.0;
                    }

                    // Trigger cascade check for blocks that might now have support
                    self.graph_dirty = true;
                    break;
                }
            }
        }
    }
    
    /// Check if a block should disintegrate based on impact force vs material strength
    /// Returns: (should_disintegrate, particle_count) or None if no action needed
    fn check_impact_threshold(&mut self, block_id: u32) -> Option<(bool, usize)> {
        let state = self.states.get_mut(&block_id)?;
        
        if state.peak_impact <= 0.0 {
            return None;
        }
        
        let break_threshold = get_break_threshold(state.material_index);
        
        if state.peak_impact > break_threshold {
            // Force exceeds threshold - disintegrate!
            state.should_disintegrate = true;
            
            // Calculate particle count based on how much force exceeded threshold
            let force_ratio = state.peak_impact / break_threshold;
            let particle_count = ((force_ratio * 8.0) as usize).clamp(4, 32);
            
            self.blocks_to_remove.push(block_id);
            
            Some((true, particle_count))
        } else if state.peak_impact > break_threshold * 0.3 {
            // Partial impact - knock loose but don't destroy
            if !state.is_loose {
                state.is_loose = true;
                state.structurally_supported = false;
                // Give it a small upward bounce from the impact
                state.velocity.y += (state.peak_impact / state.mass.max(1.0)) * 0.01;
            }
            
            Some((false, 0))
        } else {
            None
        }
    }
    
    /// Check all blocks for impact threshold and return blocks that disintegrated
    /// Returns: Vec of (block_id, particle_count, impact_velocity)
    pub fn check_all_impact_thresholds(&mut self) -> Vec<(u32, usize, Vec3)> {
        let block_ids: Vec<u32> = self.states.keys().copied().collect();
        let mut disintegrated = Vec::new();
        
        for block_id in block_ids {
            // Get velocity before potential removal
            let velocity = self.states.get(&block_id)
                .map(|s| s.velocity)
                .unwrap_or(Vec3::ZERO);
            
            if let Some((should_disintegrate, particle_count)) = self.check_impact_threshold(block_id) {
                if should_disintegrate {
                    disintegrated.push((block_id, particle_count, velocity));
                }
            }
        }
        
        disintegrated
    }

    /// Update shape-specific rolling/sliding behavior
    /// Spheres roll, cylinders can roll or slide, cubes slide and tumble
    fn update_rolling_behavior(&mut self, block_id: u32, shape: &BuildingBlockShape, dt: f32) {
        let state = match self.states.get_mut(&block_id) {
            Some(s) => s,
            None => return,
        };
        
        // Only apply rolling physics when grounded and moving
        if !state.grounded {
            return;
        }
        
        let horizontal_speed = Vec3::new(state.velocity.x, 0.0, state.velocity.z).length();
        if horizontal_speed < 0.01 {
            state.rolling_axis = None;
            state.tumble_progress = 0.0;
            return;
        }
        
        match shape {
            BuildingBlockShape::Sphere { radius } => {
                // True rolling: ω = v / r (no-slip condition)
                // Rolling axis is perpendicular to velocity direction
                let velocity_dir = Vec3::new(state.velocity.x, 0.0, state.velocity.z).normalize();
                let roll_axis = Vec3::new(-velocity_dir.z, 0.0, velocity_dir.x); // Cross with Y-up
                
                state.rolling_axis = Some(roll_axis);
                
                // Angular velocity for rolling without slip
                let angular_speed = horizontal_speed / radius;
                state.angular_velocity = roll_axis * angular_speed;
                state.fall_rotation += state.angular_velocity * dt;
            }
            
            BuildingBlockShape::Cylinder { radius, .. } => {
                // Cylinders roll when moving perpendicular to their axis
                // Assume cylinder axis is vertical (Y-up) for simplicity
                let velocity_dir = Vec3::new(state.velocity.x, 0.0, state.velocity.z).normalize();
                let roll_axis = Vec3::new(-velocity_dir.z, 0.0, velocity_dir.x);
                
                state.rolling_axis = Some(roll_axis);
                
                let angular_speed = horizontal_speed / radius;
                state.angular_velocity = roll_axis * angular_speed;
                state.fall_rotation += state.angular_velocity * dt;
            }
            
            BuildingBlockShape::Cube { half_extents } => {
                // Cubes slide, but can tumble at edges when enough momentum
                // Tumble threshold: kinetic energy > potential energy to tip
                // Simplified: if speed > sqrt(2 * g * h) where h is half-height
                
                let tumble_threshold = (2.0 * 9.81 * half_extents.y).sqrt();
                
                if horizontal_speed > tumble_threshold * 0.8 {
                    // Start or continue tumble
                    state.tumble_progress += dt * (horizontal_speed / tumble_threshold);
                    
                    if state.tumble_progress >= 1.0 {
                        // Complete tumble - snap to 90 degree rotation
                        let velocity_dir = Vec3::new(state.velocity.x, 0.0, state.velocity.z).normalize();
                        let tumble_axis = Vec3::new(-velocity_dir.z, 0.0, velocity_dir.x);
                        
                        // Add 90 degrees (π/2) rotation around tumble axis
                        state.fall_rotation += tumble_axis * std::f32::consts::FRAC_PI_2;
                        state.tumble_progress = 0.0;
                        
                        // Reduce velocity from tumble energy loss
                        state.velocity.x *= 0.7;
                        state.velocity.z *= 0.7;
                    }
                    
                    state.rolling_axis = None; // Cubes don't roll, they slide/tumble
                } else {
                    // Just sliding
                    state.rolling_axis = None;
                    state.tumble_progress = 0.0;
                    
                    // Apply angular damping when sliding
                    state.angular_velocity *= 0.9;
                }
            }
            
            BuildingBlockShape::Wedge { size } => {
                // Wedges slide, rarely tumble
                // Use the smallest dimension for tumble threshold
                let base = size.x.min(size.z);
                let tumble_threshold = (2.0_f32 * 9.81 * base * 0.3).sqrt();
                
                if horizontal_speed > tumble_threshold {
                    state.tumble_progress += dt * 0.5;
                    if state.tumble_progress >= 1.0 {
                        state.tumble_progress = 0.0;
                        state.velocity *= 0.6;
                    }
                }
                state.rolling_axis = None;
            }
            
            _ => {
                // Other shapes: just slide
                state.rolling_axis = None;
                state.angular_velocity *= 0.95; // Gradual damping
            }
        }
    }

    /// Get blocks that should be removed (disintegrated)
    pub fn take_blocks_to_remove(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.blocks_to_remove)
    }

    /// Manually trigger a block to start falling
    pub fn trigger_fall(&mut self, block_id: u32) {
        if let Some(state) = self.states.get_mut(&block_id) {
            state.grounded = false;
            state.structurally_supported = false;
        }
        self.graph_dirty = true;
    }

    /// Get fall progress (0.0 = just started, 1.0 = about to disintegrate)
    pub fn get_fall_progress(&self, block_id: u32) -> f32 {
        self.states
            .get(&block_id)
            .map(|s| (s.fall_time / self.config.disintegration_time).min(1.0))
            .unwrap_or(0.0)
    }

    /// Get the fall rotation for rendering tumbling effect
    pub fn get_fall_rotation(&self, block_id: u32) -> Vec3 {
        self.states
            .get(&block_id)
            .map(|s| s.fall_rotation)
            .unwrap_or(Vec3::ZERO)
    }

    /// Check if a block is currently falling
    pub fn is_falling(&self, block_id: u32) -> bool {
        self.states
            .get(&block_id)
            .map(|s| !s.grounded && !s.structurally_supported)
            .unwrap_or(false)
    }

    /// Remove support from beneath a block (for testing/gameplay)
    pub fn remove_support_below(&mut self, block_id: u32, manager: &BuildingBlockManager) {
        self.cascade_support_check(block_id, manager);
    }

    /// Get number of tracked blocks
    pub fn block_count(&self) -> usize {
        self.states.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_physics_state_default() {
        let state = BlockPhysicsState::default();
        assert!(state.grounded);
        assert!(state.structurally_supported);
        assert_eq!(state.velocity, Vec3::ZERO);
    }

    #[test]
    fn test_physics_config_default() {
        let config = PhysicsConfig::default();
        assert!(config.gravity > 0.0);
        assert!(config.disintegration_time > 0.0);
    }

    #[test]
    fn test_register_block() {
        let mut physics = BuildingPhysics::new();
        physics.register_block(1);
        assert!(physics.get_state(1).is_some());
    }

    #[test]
    fn test_unregister_block() {
        let mut physics = BuildingPhysics::new();
        physics.register_block(1);
        physics.unregister_block(1);
        assert!(physics.get_state(1).is_none());
    }

    #[test]
    fn test_trigger_fall() {
        let mut physics = BuildingPhysics::new();
        physics.register_block(1);
        physics.trigger_fall(1);

        let state = physics.get_state(1).unwrap();
        assert!(!state.grounded);
        assert!(!state.structurally_supported);
    }
}
