//! World Placement System
//!
//! Places saved assets into the game world with ghost preview, single placement,
//! scatter brush (Poisson disk sampling), rotation/scale controls, ground conforming,
//! and conversion to `CreatureInstance` for GPU instanced rendering.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use super::variety::{generate_variety, seed_from_position, SimpleRng, VarietyParams};
use crate::render::instancing::CreatureInstance;

// ============================================================================
// TYPES
// ============================================================================

/// A placed asset instance in the world. Stores minimal data because the
/// Variety System regenerates all visual variation deterministically from the seed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlacedAsset {
    /// ID of the asset in the library (matches `AssetEntry::id`).
    pub asset_id: String,
    /// World position where the asset is placed.
    pub position: Vec3,
    /// Variety seed derived from world position for deterministic variation.
    pub variety_seed: u32,
    /// Manual Y-axis rotation applied by the user (radians).
    pub manual_rotation: f32,
    /// Manual uniform scale applied by the user.
    pub manual_scale: f32,
}

/// Manages world placement of assets: ghost preview, single/scatter placement,
/// rotation/scale controls, and instance generation.
pub struct PlacementSystem {
    /// Currently selected asset ID from the library (None = placement inactive).
    pub selected_asset: Option<String>,
    /// Ghost preview position (follows cursor on terrain).
    pub ghost_position: Vec3,
    /// Ghost preview rotation (adjusted with R key).
    pub ghost_rotation: f32,
    /// Ghost preview scale (adjusted with [ / ] keys).
    pub ghost_scale: f32,
    /// All placed asset instances in the world.
    pub placed_instances: Vec<PlacedAsset>,
    /// Whether scatter brush mode is active.
    pub scatter_mode: bool,
    /// Scatter brush radius in world units.
    pub scatter_radius: f32,
    /// Scatter brush density (unused placeholder for future tuning).
    pub scatter_density: f32,
    /// Minimum spacing between scattered assets (Poisson disk `r`).
    pub scatter_min_spacing: f32,
}

/// Rotation increment per R key press (15 degrees in radians).
const ROTATION_STEP: f32 = 15.0 * (std::f32::consts::PI / 180.0);

/// Scale increment per [ / ] key press.
const SCALE_STEP: f32 = 0.1;

/// Minimum allowed scale.
const SCALE_MIN: f32 = 0.1;

/// Maximum allowed scale.
const SCALE_MAX: f32 = 5.0;

/// Maximum candidate attempts per active point in Poisson disk sampling.
const POISSON_MAX_ATTEMPTS: u32 = 30;

// ============================================================================
// PLACEMENT SYSTEM
// ============================================================================

impl PlacementSystem {
    /// Create a new placement system with default settings.
    pub fn new() -> Self {
        Self {
            selected_asset: None,
            ghost_position: Vec3::ZERO,
            ghost_rotation: 0.0,
            ghost_scale: 1.0,
            placed_instances: Vec::new(),
            scatter_mode: false,
            scatter_radius: 5.0,
            scatter_density: 0.3,
            scatter_min_spacing: 2.0,
        }
    }

    /// Update ghost preview position to follow the cursor on terrain.
    pub fn update_ghost(&mut self, cursor_world_pos: Vec3) {
        self.ghost_position = cursor_world_pos;
    }

    /// Rotate the ghost preview by `delta` radians, wrapping within [0, TAU).
    pub fn rotate_ghost(&mut self, delta: f32) {
        self.ghost_rotation = (self.ghost_rotation + delta).rem_euclid(std::f32::consts::TAU);
    }

    /// Scale the ghost preview by `delta`, clamped to [SCALE_MIN, SCALE_MAX].
    pub fn scale_ghost(&mut self, delta: f32) {
        self.ghost_scale = (self.ghost_scale + delta).clamp(SCALE_MIN, SCALE_MAX);
    }

    /// Place a single asset at the ghost position. Returns the placed asset
    /// or None if no asset is selected.
    pub fn place(&mut self) -> Option<PlacedAsset> {
        let asset_id = self.selected_asset.as_ref()?.clone();
        let seed = seed_from_position(
            self.ghost_position.x,
            self.ghost_position.y,
            self.ghost_position.z,
        );
        let placed = PlacedAsset {
            asset_id,
            position: self.ghost_position,
            variety_seed: seed,
            manual_rotation: self.ghost_rotation,
            manual_scale: self.ghost_scale,
        };
        self.placed_instances.push(placed.clone());
        Some(placed)
    }

    /// Place a single asset with ground conforming via raycast.
    /// The ghost position's Y is replaced by the raycast result.
    /// Falls back to ghost_position.y if raycast returns None.
    pub fn place_with_raycast(
        &mut self,
        ground_raycast: &dyn Fn(f32, f32) -> Option<f32>,
    ) -> Option<PlacedAsset> {
        let ground_y = ground_raycast(self.ghost_position.x, self.ghost_position.z)
            .unwrap_or(self.ghost_position.y);
        self.ghost_position.y = ground_y;
        self.place()
    }

    /// Remove the last placed asset (undo). Returns the removed asset or None.
    pub fn undo_last(&mut self) -> Option<PlacedAsset> {
        let removed = self.placed_instances.pop();
        if let Some(ref pa) = removed {
            println!(
                "Placement: undid '{}' at ({:.1}, {:.1}, {:.1})",
                pa.asset_id, pa.position.x, pa.position.y, pa.position.z,
            );
        }
        removed
    }

    /// Return the number of placed instances.
    pub fn instance_count(&self) -> usize {
        self.placed_instances.len()
    }

    /// Scatter-place assets in a circle around the ghost position using Poisson
    /// disk sampling. Each point is ground-conformed via the provided raycast.
    ///
    /// `ground_raycast(x, z)` returns `Some(y)` if terrain exists at (x, z).
    pub fn scatter(
        &mut self,
        ground_raycast: &dyn Fn(f32, f32) -> Option<f32>,
    ) -> Vec<PlacedAsset> {
        let asset_id = match &self.selected_asset {
            Some(id) => id.clone(),
            None => return Vec::new(),
        };

        let center_seed =
            seed_from_position(self.ghost_position.x, 0.0, self.ghost_position.z);

        let sample_points = poisson_disk_sample(
            [self.ghost_position.x, self.ghost_position.z],
            self.scatter_radius,
            self.scatter_min_spacing,
            POISSON_MAX_ATTEMPTS,
            center_seed,
        );

        // Use a separate RNG for per-asset rotation jitter so each scattered
        // asset faces a slightly different direction (looks natural in forests/fields).
        let mut jitter_rng = SimpleRng::new(center_seed.wrapping_add(7));

        let mut newly_placed = Vec::new();
        for pt in &sample_points {
            let ground_y = ground_raycast(pt[0], pt[1]).unwrap_or(0.0);
            let position = Vec3::new(pt[0], ground_y, pt[1]);
            let seed = seed_from_position(position.x, position.y, position.z);
            // Random rotation jitter: base rotation + random offset in [0, TAU)
            let rotation_jitter = jitter_rng.next_f32() * std::f32::consts::TAU;
            let placed = PlacedAsset {
                asset_id: asset_id.clone(),
                position,
                variety_seed: seed,
                manual_rotation: (self.ghost_rotation + rotation_jitter)
                    .rem_euclid(std::f32::consts::TAU),
                manual_scale: self.ghost_scale,
            };
            self.placed_instances.push(placed.clone());
            newly_placed.push(placed);
        }
        newly_placed
    }

    /// Convert all placed instances to `CreatureInstance` for GPU rendering.
    /// Applies variety system variation on top of manual rotation/scale.
    pub fn generate_instances(&self, variety_params: &VarietyParams) -> Vec<CreatureInstance> {
        self.placed_instances
            .iter()
            .map(|pa| {
                let variety = generate_variety(variety_params, pa.variety_seed);
                let total_scale = pa.manual_scale * variety.scale.x;
                let total_rotation_y = pa.manual_rotation + variety.rotation_y;
                let rotation = glam::Quat::from_rotation_y(total_rotation_y);
                let rot_arr: [f32; 4] = [rotation.x, rotation.y, rotation.z, rotation.w];
                CreatureInstance::new(
                    pa.position.into(),
                    rot_arr,
                    total_scale,
                    0,
                    0,
                    0xFFFFFFFF,
                )
            })
            .collect()
    }

    /// Handle R key: rotate ghost by 15 degrees.
    pub fn handle_rotate(&mut self) {
        self.rotate_ghost(ROTATION_STEP);
        println!("Placement: rotation = {:.0}°", self.ghost_rotation.to_degrees());
    }

    /// Handle [ key: decrease ghost scale.
    pub fn handle_scale_down(&mut self) {
        self.scale_ghost(-SCALE_STEP);
        println!("Placement: scale = {:.1}", self.ghost_scale);
    }

    /// Handle ] key: increase ghost scale.
    pub fn handle_scale_up(&mut self) {
        self.scale_ghost(SCALE_STEP);
        println!("Placement: scale = {:.1}", self.ghost_scale);
    }

    /// Handle click: place a single asset at the ghost position.
    pub fn handle_click(&mut self) {
        if let Some(placed) = self.place() {
            println!(
                "Placed '{}' at ({:.1}, {:.1}, {:.1}) seed={}",
                placed.asset_id,
                placed.position.x,
                placed.position.y,
                placed.position.z,
                placed.variety_seed,
            );
        }
    }

    /// Handle Ctrl+click: scatter-place assets around ghost position.
    pub fn handle_scatter_click(
        &mut self,
        ground_raycast: &dyn Fn(f32, f32) -> Option<f32>,
    ) {
        let placed = self.scatter(ground_raycast);
        if !placed.is_empty() {
            println!(
                "Scatter: placed {} assets around ({:.1}, {:.1})",
                placed.len(),
                self.ghost_position.x,
                self.ghost_position.z,
            );
        }
    }

    /// Handle X key: remove the nearest placed asset to the ghost position.
    pub fn handle_delete(&mut self) {
        if let Some(removed) = self.remove_nearest(self.ghost_position, 5.0) {
            println!(
                "Placement: removed '{}' at ({:.1}, {:.1}, {:.1})",
                removed.asset_id, removed.position.x, removed.position.y, removed.position.z,
            );
        } else {
            println!("Placement: no asset within range to remove");
        }
    }

    /// Remove the placed asset nearest to `position` within `max_distance`.
    /// Returns the removed asset, or None if nothing was close enough.
    pub fn remove_nearest(&mut self, position: Vec3, max_distance: f32) -> Option<PlacedAsset> {
        if self.placed_instances.is_empty() {
            return None;
        }

        let max_dist_sq = max_distance * max_distance;
        let mut best_idx = None;
        let mut best_dist_sq = max_dist_sq;

        for (i, pa) in self.placed_instances.iter().enumerate() {
            let dist_sq = (pa.position - position).length_squared();
            if dist_sq < best_dist_sq {
                best_dist_sq = dist_sq;
                best_idx = Some(i);
            }
        }

        best_idx.map(|idx| self.placed_instances.remove(idx))
    }

    /// Select an asset for placement (called when library selection changes).
    pub fn select_asset(&mut self, asset_id: Option<String>) {
        self.selected_asset = asset_id;
    }

    /// Returns true if an asset is selected and ready for placement.
    pub fn is_active(&self) -> bool {
        self.selected_asset.is_some()
    }

    /// Adjust scatter brush radius, clamped to [1.0, 50.0].
    pub fn adjust_scatter_radius(&mut self, delta: f32) {
        self.scatter_radius = (self.scatter_radius + delta).clamp(1.0, 50.0);
        println!("Placement: scatter radius = {:.1}", self.scatter_radius);
    }

    /// Adjust scatter minimum spacing, clamped to [0.5, scatter_radius].
    pub fn adjust_scatter_spacing(&mut self, delta: f32) {
        self.scatter_min_spacing =
            (self.scatter_min_spacing + delta).clamp(0.5, self.scatter_radius);
        println!(
            "Placement: scatter spacing = {:.1}",
            self.scatter_min_spacing
        );
    }

    /// Clear all placed instances.
    pub fn clear(&mut self) {
        self.placed_instances.clear();
        println!("Placement: cleared all instances");
    }

    /// Save placements to a JSON file.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
        }
        let json =
            serde_json::to_string_pretty(&self.placed_instances).map_err(|e| format!("{e}"))?;
        std::fs::write(path, json).map_err(|e| format!("{e}"))?;
        println!("Placement: saved {} instances to {}", self.placed_instances.len(), path.display());
        Ok(())
    }

    /// Load placements from a JSON file.
    pub fn load(&mut self, path: &std::path::Path) -> Result<(), String> {
        let json = std::fs::read_to_string(path).map_err(|e| format!("{e}"))?;
        let loaded: Vec<PlacedAsset> =
            serde_json::from_str(&json).map_err(|e| format!("{e}"))?;
        let count = loaded.len();
        self.placed_instances = loaded;
        println!("Placement: loaded {} instances from {}", count, path.display());
        Ok(())
    }
}

impl Default for PlacementSystem {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// POISSON DISK SAMPLING (Bridson's Algorithm)
// ============================================================================

/// Generate points using Bridson's Poisson disk sampling algorithm.
///
/// Produces a set of 2D points within a circle of `radius` centered at `center`,
/// where all points are at least `min_dist` apart. This creates a natural-looking
/// "blue noise" distribution ideal for scatter placement.
///
/// # Arguments
/// * `center` - Center of the sampling circle `[x, z]`
/// * `radius` - Radius of the sampling region
/// * `min_dist` - Minimum distance between any two points
/// * `max_attempts` - Max candidates to try per active point before deactivation
/// * `seed` - RNG seed for deterministic results
pub fn poisson_disk_sample(
    center: [f32; 2],
    radius: f32,
    min_dist: f32,
    max_attempts: u32,
    seed: u32,
) -> Vec<[f32; 2]> {
    if min_dist <= 0.0 || radius <= 0.0 {
        return Vec::new();
    }

    let mut rng = SimpleRng::new(seed);
    let cell_size = min_dist / std::f32::consts::SQRT_2;
    let grid_side = (2.0 * radius / cell_size).ceil() as usize + 1;

    // Spatial grid for O(1) neighbor lookups. Each cell stores an optional point index.
    let mut grid: Vec<Option<usize>> = vec![None; grid_side * grid_side];
    let mut points: Vec<[f32; 2]> = Vec::new();
    let mut active: Vec<usize> = Vec::new();

    // Helper: convert world coords to grid cell
    let to_grid = |px: f32, pz: f32| -> (usize, usize) {
        let gx = ((px - center[0] + radius) / cell_size) as usize;
        let gz = ((pz - center[1] + radius) / cell_size) as usize;
        (gx.min(grid_side - 1), gz.min(grid_side - 1))
    };

    // Initialize with center point
    points.push(center);
    active.push(0);
    let (gx, gz) = to_grid(center[0], center[1]);
    if gx < grid_side && gz < grid_side {
        grid[gz * grid_side + gx] = Some(0);
    }

    while !active.is_empty() {
        // Pick a random active point
        let active_idx = (rng.next_u32() as usize) % active.len();
        let point_idx = active[active_idx];
        let point = points[point_idx];
        let mut found = false;

        for _ in 0..max_attempts {
            // Generate candidate in annulus [min_dist, 2 * min_dist]
            let angle = rng.next_f32() * std::f32::consts::TAU;
            let dist = min_dist + rng.next_f32() * min_dist;
            let cx = point[0] + angle.cos() * dist;
            let cz = point[1] + angle.sin() * dist;

            // Check if candidate is within the sampling circle
            let dx = cx - center[0];
            let dz = cz - center[1];
            if dx * dx + dz * dz > radius * radius {
                continue;
            }

            let (gx, gz) = to_grid(cx, cz);
            if gx >= grid_side || gz >= grid_side {
                continue;
            }

            // Check neighboring grid cells for minimum distance violations
            let mut too_close = false;
            let search_range = 2usize;
            let gx_min = gx.saturating_sub(search_range);
            let gz_min = gz.saturating_sub(search_range);
            let gx_max = (gx + search_range + 1).min(grid_side);
            let gz_max = (gz + search_range + 1).min(grid_side);

            'outer: for nz in gz_min..gz_max {
                for nx in gx_min..gx_max {
                    if let Some(neighbor_idx) = grid[nz * grid_side + nx] {
                        let neighbor = points[neighbor_idx];
                        let ndx = cx - neighbor[0];
                        let ndz = cz - neighbor[1];
                        if ndx * ndx + ndz * ndz < min_dist * min_dist {
                            too_close = true;
                            break 'outer;
                        }
                    }
                }
            }

            if !too_close {
                let new_idx = points.len();
                points.push([cx, cz]);
                active.push(new_idx);
                grid[gz * grid_side + gx] = Some(new_idx);
                found = true;
                break;
            }
        }

        if !found {
            // Deactivate this point (swap-remove for efficiency)
            active.swap_remove(active_idx);
        }
    }

    points
}

// ============================================================================
// SCREEN-TO-GROUND RAYCAST
// ============================================================================

/// Cast a ray from screen coordinates through the camera and intersect with
/// the Y=0 ground plane. Returns the world-space hit point, or `None` if
/// the ray is parallel to the ground plane (looking straight up).
///
/// * `screen_x`, `screen_y` - cursor position in pixels
/// * `screen_w`, `screen_h` - viewport dimensions in pixels
/// * `inv_vp` - inverse of the view-projection matrix (`Mat4`)
pub fn screen_to_ground(
    screen_x: f32,
    screen_y: f32,
    screen_w: f32,
    screen_h: f32,
    inv_vp: glam::Mat4,
) -> Option<Vec3> {
    // Convert screen coords to NDC [-1, 1]
    let ndc_x = (2.0 * screen_x / screen_w) - 1.0;
    let ndc_y = 1.0 - (2.0 * screen_y / screen_h);

    // Unproject near and far points
    let near_ndc = glam::Vec4::new(ndc_x, ndc_y, -1.0, 1.0);
    let far_ndc = glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);

    let near_world = inv_vp * near_ndc;
    let far_world = inv_vp * far_ndc;

    if near_world.w.abs() < 1e-8 || far_world.w.abs() < 1e-8 {
        return None;
    }

    let near = Vec3::new(
        near_world.x / near_world.w,
        near_world.y / near_world.w,
        near_world.z / near_world.w,
    );
    let far = Vec3::new(
        far_world.x / far_world.w,
        far_world.y / far_world.w,
        far_world.z / far_world.w,
    );

    // Ray-plane intersection with Y=0
    let dir = far - near;
    if dir.y.abs() < 1e-8 {
        return None; // Ray parallel to ground
    }

    let t = -near.y / dir.y;
    if t < 0.0 {
        return None; // Intersection behind camera
    }

    Some(near + dir * t)
}

// ============================================================================
// PLACEMENTS FILE PATH
// ============================================================================

/// Default path for world placements file.
pub fn placements_path() -> std::path::PathBuf {
    std::path::PathBuf::from("assets/world/placements.json")
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placement_system_new() {
        let ps = PlacementSystem::new();
        assert!(ps.selected_asset.is_none());
        assert_eq!(ps.ghost_position, Vec3::ZERO);
        assert_eq!(ps.ghost_rotation, 0.0);
        assert_eq!(ps.ghost_scale, 1.0);
        assert!(ps.placed_instances.is_empty());
    }

    #[test]
    fn test_update_ghost() {
        let mut ps = PlacementSystem::new();
        ps.update_ghost(Vec3::new(10.0, 5.0, -3.0));
        assert_eq!(ps.ghost_position, Vec3::new(10.0, 5.0, -3.0));
    }

    #[test]
    fn test_rotate_ghost() {
        let mut ps = PlacementSystem::new();
        ps.rotate_ghost(ROTATION_STEP);
        let expected = 15.0_f32.to_radians();
        assert!((ps.ghost_rotation - expected).abs() < 1e-5);
    }

    #[test]
    fn test_rotate_ghost_wraps() {
        let mut ps = PlacementSystem::new();
        // Rotate 24 times * 15° = 360° -> should wrap to 0
        for _ in 0..24 {
            ps.rotate_ghost(ROTATION_STEP);
        }
        assert!(ps.ghost_rotation.abs() < 0.01 || (ps.ghost_rotation - std::f32::consts::TAU).abs() < 0.01);
    }

    #[test]
    fn test_scale_ghost_clamp() {
        let mut ps = PlacementSystem::new();
        // Scale down past minimum
        for _ in 0..20 {
            ps.scale_ghost(-SCALE_STEP);
        }
        assert!((ps.ghost_scale - SCALE_MIN).abs() < 1e-5);

        // Scale up past maximum
        for _ in 0..100 {
            ps.scale_ghost(SCALE_STEP);
        }
        assert!((ps.ghost_scale - SCALE_MAX).abs() < 1e-5);
    }

    #[test]
    fn test_place_without_selection() {
        let mut ps = PlacementSystem::new();
        assert!(ps.place().is_none());
    }

    #[test]
    fn test_place_with_selection() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("oak_tree".to_string());
        ps.ghost_position = Vec3::new(5.0, 0.0, 10.0);
        ps.ghost_rotation = 1.0;
        ps.ghost_scale = 1.5;

        let placed = ps.place().unwrap();
        assert_eq!(placed.asset_id, "oak_tree");
        assert_eq!(placed.position, Vec3::new(5.0, 0.0, 10.0));
        assert_eq!(placed.manual_rotation, 1.0);
        assert_eq!(placed.manual_scale, 1.5);
        assert_ne!(placed.variety_seed, 0);
        assert_eq!(ps.placed_instances.len(), 1);
    }

    #[test]
    fn test_poisson_disk_sample_respects_min_distance() {
        let points = poisson_disk_sample([0.0, 0.0], 10.0, 2.0, 30, 42);
        assert!(!points.is_empty());

        // Verify minimum distance between all pairs
        for i in 0..points.len() {
            for j in (i + 1)..points.len() {
                let dx = points[i][0] - points[j][0];
                let dz = points[i][1] - points[j][1];
                let dist = (dx * dx + dz * dz).sqrt();
                assert!(
                    dist >= 2.0 - 0.01,
                    "Points {} and {} too close: dist={:.3}",
                    i,
                    j,
                    dist
                );
            }
        }
    }

    #[test]
    fn test_poisson_disk_sample_within_radius() {
        let center = [5.0, 5.0];
        let radius = 8.0;
        let points = poisson_disk_sample(center, radius, 1.5, 30, 123);
        for pt in &points {
            let dx = pt[0] - center[0];
            let dz = pt[1] - center[1];
            let dist = (dx * dx + dz * dz).sqrt();
            assert!(dist <= radius + 0.01, "Point outside radius: dist={:.3}", dist);
        }
    }

    #[test]
    fn test_poisson_disk_deterministic() {
        let p1 = poisson_disk_sample([0.0, 0.0], 5.0, 1.0, 30, 42);
        let p2 = poisson_disk_sample([0.0, 0.0], 5.0, 1.0, 30, 42);
        assert_eq!(p1.len(), p2.len());
        for (a, b) in p1.iter().zip(p2.iter()) {
            assert_eq!(a, b);
        }
    }

    #[test]
    fn test_scatter_with_ground_raycast() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("rock".to_string());
        ps.ghost_position = Vec3::new(0.0, 0.0, 0.0);

        // Flat ground at y=3.0
        let raycast = |_x: f32, _z: f32| -> Option<f32> { Some(3.0) };
        let placed = ps.scatter(&raycast);

        assert!(!placed.is_empty());
        for p in &placed {
            assert_eq!(p.position.y, 3.0, "Asset should conform to ground");
        }
    }

    #[test]
    fn test_generate_instances() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("tree".to_string());
        ps.ghost_position = Vec3::new(1.0, 0.0, 2.0);
        ps.place();

        let params = VarietyParams::default();
        let instances = ps.generate_instances(&params);
        assert_eq!(instances.len(), 1);
        assert!(instances[0].scale > 0.0);
    }

    #[test]
    fn test_placed_asset_serialize() {
        let pa = PlacedAsset {
            asset_id: "test".to_string(),
            position: Vec3::new(1.0, 2.0, 3.0),
            variety_seed: 42,
            manual_rotation: 0.5,
            manual_scale: 1.2,
        };
        let json = serde_json::to_string(&pa).unwrap();
        let deserialized: PlacedAsset = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.asset_id, "test");
        assert_eq!(deserialized.variety_seed, 42);
    }

    #[test]
    fn test_screen_to_ground_center() {
        use glam::Mat4;
        // Camera at (0, 10, 0) looking straight down -> center of screen hits (0, 0, 0)
        let view = Mat4::look_at_rh(
            Vec3::new(0.0, 10.0, 10.0),
            Vec3::ZERO,
            Vec3::Y,
        );
        let proj = Mat4::perspective_rh(45.0_f32.to_radians(), 1.0, 0.1, 100.0);
        let vp = proj * view;
        let inv_vp = vp.inverse();
        // Center of screen
        let result = screen_to_ground(640.0, 400.0, 1280.0, 800.0, inv_vp);
        assert!(result.is_some(), "Should hit ground plane");
        let hit = result.unwrap();
        // Y should be approximately 0 (ground plane)
        assert!(hit.y.abs() < 0.1, "Y should be near 0, got {}", hit.y);
    }

    #[test]
    fn test_undo_last() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("tree".to_string());
        ps.ghost_position = Vec3::new(1.0, 0.0, 2.0);
        ps.place();
        assert_eq!(ps.placed_instances.len(), 1);

        let removed = ps.undo_last();
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().asset_id, "tree");
        assert!(ps.placed_instances.is_empty());
    }

    #[test]
    fn test_place_with_raycast() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("rock".to_string());
        ps.ghost_position = Vec3::new(5.0, 0.0, 3.0);

        // Hilly terrain: y = 7.0 at all positions
        let raycast = |_x: f32, _z: f32| -> Option<f32> { Some(7.0) };
        let placed = ps.place_with_raycast(&raycast).unwrap();
        assert_eq!(placed.position.y, 7.0, "Should conform to terrain height");
    }

    #[test]
    fn test_instance_count() {
        let mut ps = PlacementSystem::new();
        assert_eq!(ps.instance_count(), 0);
        ps.selected_asset = Some("a".to_string());
        ps.place();
        assert_eq!(ps.instance_count(), 1);
        ps.place();
        assert_eq!(ps.instance_count(), 2);
    }

    #[test]
    fn test_poisson_invalid_params() {
        assert!(poisson_disk_sample([0.0, 0.0], 0.0, 1.0, 30, 1).is_empty());
        assert!(poisson_disk_sample([0.0, 0.0], 5.0, 0.0, 30, 1).is_empty());
    }

    #[test]
    fn test_place_with_raycast_fallback() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("bush".to_string());
        ps.ghost_position = Vec3::new(5.0, 3.0, 5.0);

        // Raycast returns None (no terrain), should fallback to ghost Y
        let raycast = |_x: f32, _z: f32| -> Option<f32> { None };
        let placed = ps.place_with_raycast(&raycast).unwrap();
        assert_eq!(placed.position.y, 3.0);
    }

    #[test]
    fn test_screen_to_ground_basic() {
        // Camera looking straight down at the origin
        let view = glam::Mat4::look_at_rh(
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::ZERO,
            Vec3::NEG_Z,
        );
        let proj = glam::Mat4::perspective_rh(
            std::f32::consts::FRAC_PI_4,
            1.0,
            0.1,
            100.0,
        );
        let inv_vp = (proj * view).inverse();

        // Center of screen should hit near origin on the ground plane
        let hit = screen_to_ground(400.0, 300.0, 800.0, 600.0, inv_vp);
        assert!(hit.is_some());
        let p = hit.unwrap();
        assert!(p.y.abs() < 0.01, "Ground hit Y should be ~0, got {}", p.y);
    }

    #[test]
    fn test_screen_to_ground_no_intersection() {
        // Camera looking straight up -- ray won't intersect Y=0 going forward
        let view = glam::Mat4::look_at_rh(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            Vec3::Z,
        );
        let proj = glam::Mat4::perspective_rh(
            std::f32::consts::FRAC_PI_4,
            1.0,
            0.1,
            100.0,
        );
        let inv_vp = (proj * view).inverse();
        let hit = screen_to_ground(400.0, 300.0, 800.0, 600.0, inv_vp);
        assert!(hit.is_none());
    }

    #[test]
    fn test_save_load_roundtrip() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("flower".to_string());
        ps.ghost_position = Vec3::new(1.0, 2.0, 3.0);
        ps.ghost_rotation = 0.5;
        ps.ghost_scale = 1.8;
        ps.place();
        ps.ghost_position = Vec3::new(4.0, 5.0, 6.0);
        ps.place();

        let dir = std::env::temp_dir().join("battle_tok_test_placement");
        let path = dir.join("test_placements.json");

        ps.save(&path).unwrap();
        assert!(path.exists());

        let mut ps2 = PlacementSystem::new();
        ps2.load(&path).unwrap();
        assert_eq!(ps2.placed_instances.len(), 2);
        assert_eq!(ps2.placed_instances[0].asset_id, "flower");
        assert_eq!(ps2.placed_instances[0].position, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(ps2.placed_instances[0].manual_rotation, 0.5);
        assert_eq!(ps2.placed_instances[0].manual_scale, 1.8);
        assert_eq!(ps2.placed_instances[1].position, Vec3::new(4.0, 5.0, 6.0));

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_scatter_with_sloped_ground() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("tree".to_string());
        ps.ghost_position = Vec3::new(0.0, 0.0, 0.0);

        // Sloped ground: y = 0.5 * x
        let raycast = |x: f32, _z: f32| -> Option<f32> { Some(x * 0.5) };
        let placed = ps.scatter(&raycast);

        assert!(!placed.is_empty());
        for p in &placed {
            let expected_y = p.position.x * 0.5;
            assert!(
                (p.position.y - expected_y).abs() < 0.01,
                "Asset at x={:.1} should have y={:.1}, got y={:.1}",
                p.position.x,
                expected_y,
                p.position.y,
            );
        }
    }

    #[test]
    fn test_remove_nearest() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("bush".to_string());

        // Place three assets at known positions
        ps.ghost_position = Vec3::new(0.0, 0.0, 0.0);
        ps.place();
        ps.ghost_position = Vec3::new(10.0, 0.0, 0.0);
        ps.place();
        ps.ghost_position = Vec3::new(20.0, 0.0, 0.0);
        ps.place();
        assert_eq!(ps.instance_count(), 3);

        // Remove nearest to (9.5, 0, 0) within max_distance=5 -> should find (10, 0, 0)
        let removed = ps.remove_nearest(Vec3::new(9.5, 0.0, 0.0), 5.0);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().position.x, 10.0);
        assert_eq!(ps.instance_count(), 2);

        // Try to remove with nothing in range
        let removed = ps.remove_nearest(Vec3::new(100.0, 0.0, 0.0), 2.0);
        assert!(removed.is_none());
        assert_eq!(ps.instance_count(), 2);
    }

    #[test]
    fn test_remove_nearest_empty() {
        let mut ps = PlacementSystem::new();
        assert!(ps.remove_nearest(Vec3::ZERO, 10.0).is_none());
    }

    #[test]
    fn test_scatter_rotation_jitter() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("grass".to_string());
        ps.ghost_position = Vec3::ZERO;
        ps.ghost_rotation = 0.0;

        let raycast = |_x: f32, _z: f32| -> Option<f32> { Some(0.0) };
        let placed = ps.scatter(&raycast);

        // With rotation jitter, not all assets should have the same rotation
        if placed.len() >= 2 {
            let rotations: Vec<f32> = placed.iter().map(|p| p.manual_rotation).collect();
            let all_same = rotations.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-5);
            assert!(
                !all_same,
                "Scattered assets should have varied rotations from jitter"
            );
        }
    }

    #[test]
    fn test_adjust_scatter_radius() {
        let mut ps = PlacementSystem::new();
        assert_eq!(ps.scatter_radius, 5.0);
        ps.adjust_scatter_radius(3.0);
        assert_eq!(ps.scatter_radius, 8.0);
        // Clamp at max
        ps.adjust_scatter_radius(100.0);
        assert_eq!(ps.scatter_radius, 50.0);
        // Clamp at min
        ps.adjust_scatter_radius(-100.0);
        assert_eq!(ps.scatter_radius, 1.0);
    }

    #[test]
    fn test_adjust_scatter_spacing() {
        let mut ps = PlacementSystem::new();
        assert_eq!(ps.scatter_min_spacing, 2.0);
        ps.adjust_scatter_spacing(1.0);
        assert_eq!(ps.scatter_min_spacing, 3.0);
        // Clamp at min
        ps.adjust_scatter_spacing(-10.0);
        assert_eq!(ps.scatter_min_spacing, 0.5);
    }

    #[test]
    fn test_scatter_without_selection() {
        let mut ps = PlacementSystem::new();
        let raycast = |_x: f32, _z: f32| -> Option<f32> { Some(0.0) };
        let placed = ps.scatter(&raycast);
        assert!(placed.is_empty());
    }

    #[test]
    fn test_scatter_preserves_scale() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("tree".to_string());
        ps.ghost_scale = 2.5;
        ps.ghost_position = Vec3::ZERO;

        let raycast = |_x: f32, _z: f32| -> Option<f32> { Some(0.0) };
        let placed = ps.scatter(&raycast);

        for p in &placed {
            assert_eq!(p.manual_scale, 2.5);
        }
    }

    #[test]
    fn test_generate_instances_empty() {
        let ps = PlacementSystem::new();
        let params = VarietyParams::default();
        let instances = ps.generate_instances(&params);
        assert!(instances.is_empty());
    }

    #[test]
    fn test_select_deselect() {
        let mut ps = PlacementSystem::new();
        assert!(!ps.is_active());
        ps.select_asset(Some("oak".to_string()));
        assert!(ps.is_active());
        ps.select_asset(None);
        assert!(!ps.is_active());
    }

    #[test]
    fn test_clear() {
        let mut ps = PlacementSystem::new();
        ps.selected_asset = Some("tree".to_string());
        ps.place();
        ps.place();
        assert_eq!(ps.instance_count(), 2);
        ps.clear();
        assert_eq!(ps.instance_count(), 0);
    }

    #[test]
    fn test_poisson_negative_radius() {
        assert!(poisson_disk_sample([0.0, 0.0], -5.0, 1.0, 30, 1).is_empty());
    }

    #[test]
    fn test_poisson_min_dist_larger_than_radius() {
        let points = poisson_disk_sample([0.0, 0.0], 1.0, 5.0, 30, 42);
        assert_eq!(points.len(), 1);
        assert_eq!(points[0], [0.0, 0.0]);
    }

    #[test]
    fn test_default_impl() {
        let ps = PlacementSystem::default();
        assert_eq!(ps.ghost_scale, 1.0);
        assert!(ps.placed_instances.is_empty());
    }

    #[test]
    fn test_placements_path() {
        let path = placements_path();
        assert!(path.ends_with("placements.json"));
    }
}
