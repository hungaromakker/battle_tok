//! SDF Operations Module (Phase 3)
//!
//! Provides operations for merging, intersecting, and subtracting SDF shapes.
//! Also includes the merge workflow system for double-click merging of building blocks.
//!
//! # Merge Workflow
//!
//! 1. Place blocks normally (renders as individual meshes)
//! 2. Double-click on a block to start merge selection
//! 3. System detects connected group via flood-fill
//! 4. Evaluate combined SDF with smooth union
//! 5. Run Marching Cubes to generate triangle mesh
//! 6. Replace blocks with single mesh (removes from block list, adds to static geometry)

use glam::Vec3;
use std::collections::HashSet;
use std::time::Instant;

use super::building_blocks::{BuildingBlock, BuildingBlockManager, BlockVertex, AABB};
use super::marching_cubes::generate_merged_mesh;

// ============================================================================
// SDF OPERATIONS (already in building_blocks, but re-exported for convenience)
// ============================================================================

/// Smooth union of two SDFs - creates smooth blend between shapes
pub fn smooth_union(d1: f32, d2: f32, k: f32) -> f32 {
    let h = (0.5 + 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
    d2 + (d1 - d2) * h - k * h * (1.0 - h)
}

/// Hard union (minimum of two SDFs)
pub fn union(d1: f32, d2: f32) -> f32 {
    d1.min(d2)
}

/// Hard intersection (maximum of two SDFs)
pub fn intersection(d1: f32, d2: f32) -> f32 {
    d1.max(d2)
}

/// Subtraction (d1 minus d2)
pub fn subtraction(d1: f32, d2: f32) -> f32 {
    d1.max(-d2)
}

/// Smooth subtraction
pub fn smooth_subtraction(d1: f32, d2: f32, k: f32) -> f32 {
    let h = (0.5 - 0.5 * (d2 + d1) / k).clamp(0.0, 1.0);
    d1 + ((-d2) - d1) * h + k * h * (1.0 - h)
}

/// Smooth intersection
pub fn smooth_intersection(d1: f32, d2: f32, k: f32) -> f32 {
    let h = (0.5 - 0.5 * (d2 - d1) / k).clamp(0.0, 1.0);
    d2 + (d1 - d2) * h + k * h * (1.0 - h)
}

// ============================================================================
// MERGE STATE MACHINE
// ============================================================================

/// State of the merge workflow
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MergeState {
    /// Idle - no merge in progress
    Idle,
    /// Waiting for first click (selecting first block)
    SelectingFirst,
    /// First block selected, waiting for second click or double-click
    FirstSelected { block_id: u32 },
    /// Merge in progress (computing)
    Merging,
}

impl Default for MergeState {
    fn default() -> Self {
        Self::Idle
    }
}

/// A merged mesh that was created from multiple building blocks
#[derive(Debug)]
pub struct MergedMesh {
    /// Unique ID for this merged mesh
    pub id: u32,
    /// Vertices of the mesh
    pub vertices: Vec<BlockVertex>,
    /// Indices of the mesh
    pub indices: Vec<u32>,
    /// IDs of the blocks that were merged to create this
    pub source_block_ids: Vec<u32>,
    /// AABB of the merged mesh
    pub aabb: AABB,
}

// ============================================================================
// MERGE WORKFLOW MANAGER
// ============================================================================

/// Double-click detection helper
#[derive(Default)]
pub struct DoubleClickDetector {
    /// Last click time
    last_click_time: Option<Instant>,
    /// Last clicked block ID
    last_clicked_block: Option<u32>,
    /// Double-click threshold in seconds
    threshold_secs: f32,
}

impl DoubleClickDetector {
    /// Create a new double-click detector
    pub fn new(threshold_secs: f32) -> Self {
        Self {
            last_click_time: None,
            last_clicked_block: None,
            threshold_secs,
        }
    }
    
    /// Register a click. Returns true if this is a double-click on the same block.
    pub fn click(&mut self, block_id: u32) -> bool {
        let now = Instant::now();
        
        let is_double_click = if let (Some(last_time), Some(last_block)) = 
            (self.last_click_time, self.last_clicked_block) 
        {
            let elapsed = now.duration_since(last_time).as_secs_f32();
            last_block == block_id && elapsed < self.threshold_secs
        } else {
            false
        };
        
        self.last_click_time = Some(now);
        self.last_clicked_block = Some(block_id);
        
        is_double_click
    }
    
    /// Reset the detector
    pub fn reset(&mut self) {
        self.last_click_time = None;
        self.last_clicked_block = None;
    }
}

/// Manager for the merge workflow
pub struct MergeWorkflowManager {
    /// Current state
    state: MergeState,
    /// Double-click detector
    double_click: DoubleClickDetector,
    /// Selected blocks for merging
    selected_blocks: HashSet<u32>,
    /// Smoothness factor for SDF merging
    smoothness: f32,
    /// Resolution for Marching Cubes
    resolution: u32,
    /// All merged meshes
    merged_meshes: Vec<MergedMesh>,
    /// Next merged mesh ID
    next_mesh_id: u32,
}

impl Default for MergeWorkflowManager {
    fn default() -> Self {
        Self::new()
    }
}

impl MergeWorkflowManager {
    /// Create a new merge workflow manager
    pub fn new() -> Self {
        Self {
            state: MergeState::Idle,
            double_click: DoubleClickDetector::new(0.4), // 400ms threshold
            selected_blocks: HashSet::new(),
            smoothness: 0.15,
            resolution: 48, // 48x48x48 grid for marching cubes
            merged_meshes: Vec::new(),
            next_mesh_id: 1,
        }
    }
    
    /// Set the smoothness factor for SDF merging
    pub fn set_smoothness(&mut self, smoothness: f32) {
        self.smoothness = smoothness.max(0.01);
    }
    
    /// Set the resolution for Marching Cubes
    pub fn set_resolution(&mut self, resolution: u32) {
        self.resolution = resolution.clamp(16, 128);
    }
    
    /// Get current state
    pub fn state(&self) -> MergeState {
        self.state
    }
    
    /// Get selected block IDs
    pub fn selected_blocks(&self) -> &HashSet<u32> {
        &self.selected_blocks
    }
    
    /// Get all merged meshes
    pub fn merged_meshes(&self) -> &[MergedMesh] {
        &self.merged_meshes
    }
    
    /// Handle a click on a block
    ///
    /// Returns Some(block_ids) if a merge should be performed
    pub fn on_block_click(&mut self, block_id: u32, block_manager: &BuildingBlockManager) -> Option<Vec<u32>> {
        // Check for double-click
        if self.double_click.click(block_id) {
            // Double-click detected! Find all connected blocks and merge
            let connected = self.find_connected_blocks(block_id, block_manager);
            
            if connected.len() > 1 {
                // Multiple blocks connected - merge them
                return Some(connected);
            } else {
                // Single block - just select it
                self.selected_blocks.clear();
                self.selected_blocks.insert(block_id);
                self.state = MergeState::FirstSelected { block_id };
            }
        } else {
            // Single click - toggle selection
            if self.selected_blocks.contains(&block_id) {
                self.selected_blocks.remove(&block_id);
            } else {
                self.selected_blocks.insert(block_id);
            }
        }
        
        None
    }
    
    /// Find all blocks connected to the given block (via AABB overlap)
    fn find_connected_blocks(&self, start_id: u32, manager: &BuildingBlockManager) -> Vec<u32> {
        let mut connected = HashSet::new();
        let mut to_check = vec![start_id];
        
        // Flood-fill algorithm
        while let Some(current_id) = to_check.pop() {
            if connected.contains(&current_id) {
                continue;
            }
            
            connected.insert(current_id);
            
            // Get the current block's AABB
            if let Some(current_block) = manager.get_block(current_id) {
                let current_aabb = current_block.aabb();
                
                // Expand AABB slightly to find adjacent blocks
                let expanded_aabb = AABB {
                    min: current_aabb.min - Vec3::splat(0.1),
                    max: current_aabb.max + Vec3::splat(0.1),
                };
                
                // Check all other blocks for intersection
                for block in manager.blocks() {
                    if !connected.contains(&block.id) && expanded_aabb.intersects(&block.aabb()) {
                        to_check.push(block.id);
                    }
                }
            }
        }
        
        connected.into_iter().collect()
    }
    
    /// Perform merge operation on selected blocks
    ///
    /// Returns the created MergedMesh if successful
    pub fn merge_blocks(&mut self, block_ids: &[u32], manager: &mut BuildingBlockManager, color: [f32; 4]) -> Option<MergedMesh> {
        if block_ids.len() < 2 {
            return None;
        }
        
        self.state = MergeState::Merging;
        
        // Collect the blocks to merge
        let blocks: Vec<BuildingBlock> = block_ids
            .iter()
            .filter_map(|id| manager.get_block(*id).cloned())
            .collect();
        
        if blocks.len() < 2 {
            self.state = MergeState::Idle;
            return None;
        }
        
        // Generate merged mesh using Marching Cubes
        let (vertices, indices) = generate_merged_mesh(&blocks, self.smoothness, self.resolution, color);
        
        if vertices.is_empty() {
            self.state = MergeState::Idle;
            return None;
        }
        
        // Calculate AABB
        let mut aabb = AABB {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN),
        };
        for vertex in &vertices {
            aabb.expand(Vec3::from_array(vertex.position));
        }
        
        // Create merged mesh
        let merged = MergedMesh {
            id: self.next_mesh_id,
            vertices,
            indices,
            source_block_ids: block_ids.to_vec(),
            aabb,
        };
        self.next_mesh_id += 1;
        
        // Remove source blocks from manager
        for id in block_ids {
            manager.remove_block(*id);
        }
        
        // Clear selection and reset state
        self.selected_blocks.clear();
        self.state = MergeState::Idle;
        
        // Store and return the merged mesh
        let mesh_clone = MergedMesh {
            id: merged.id,
            vertices: merged.vertices.clone(),
            indices: merged.indices.clone(),
            source_block_ids: merged.source_block_ids.clone(),
            aabb: merged.aabb,
        };
        
        self.merged_meshes.push(merged);
        
        Some(mesh_clone)
    }
    
    /// Merge currently selected blocks
    pub fn merge_selected(&mut self, manager: &mut BuildingBlockManager, color: [f32; 4]) -> Option<MergedMesh> {
        let ids: Vec<u32> = self.selected_blocks.iter().copied().collect();
        self.merge_blocks(&ids, manager, color)
    }
    
    /// Cancel current selection
    pub fn cancel(&mut self) {
        self.selected_blocks.clear();
        self.state = MergeState::Idle;
        self.double_click.reset();
    }
    
    /// Check if a block is selected
    pub fn is_selected(&self, block_id: u32) -> bool {
        self.selected_blocks.contains(&block_id)
    }
    
    /// Get number of selected blocks
    pub fn selection_count(&self) -> usize {
        self.selected_blocks.len()
    }
    
    /// Remove a merged mesh by ID
    pub fn remove_merged_mesh(&mut self, mesh_id: u32) -> Option<MergedMesh> {
        if let Some(pos) = self.merged_meshes.iter().position(|m| m.id == mesh_id) {
            Some(self.merged_meshes.remove(pos))
        } else {
            None
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::building_blocks::BuildingBlockShape;
    
    #[test]
    fn test_smooth_union() {
        let d1 = 0.5;
        let d2 = 0.5;
        let k = 0.2;
        
        let result = smooth_union(d1, d2, k);
        // Smooth union should be less than hard union
        assert!(result < d1.min(d2));
    }
    
    #[test]
    fn test_double_click_detection() {
        let mut detector = DoubleClickDetector::new(0.5);
        
        // First click
        assert!(!detector.click(1));
        
        // Second click on same block (simulated immediately) - should be double-click
        assert!(detector.click(1));
        
        // Third click on different block
        assert!(!detector.click(2));
    }
    
    #[test]
    fn test_find_connected_blocks() {
        let mut manager = BuildingBlockManager::new();
        
        // Add two touching blocks
        let block1 = BuildingBlock::new(
            BuildingBlockShape::Cube { half_extents: Vec3::splat(0.5) },
            Vec3::new(0.0, 0.0, 0.0),
            0
        );
        let id1 = manager.add_block(block1);
        
        let block2 = BuildingBlock::new(
            BuildingBlockShape::Cube { half_extents: Vec3::splat(0.5) },
            Vec3::new(1.0, 0.0, 0.0), // Adjacent to block1
            0
        );
        let id2 = manager.add_block(block2);
        
        // Add a separate block
        let block3 = BuildingBlock::new(
            BuildingBlockShape::Cube { half_extents: Vec3::splat(0.5) },
            Vec3::new(10.0, 0.0, 0.0), // Far from others
            0
        );
        manager.add_block(block3);
        
        let workflow = MergeWorkflowManager::new();
        let connected = workflow.find_connected_blocks(id1, &manager);
        
        // Should find block1 and block2, but not block3
        assert_eq!(connected.len(), 2);
        assert!(connected.contains(&id1));
        assert!(connected.contains(&id2));
    }
}
