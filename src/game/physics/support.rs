//! Support Checking
//!
//! Physics support validation for hex prism structures.

/// Hex coordinate neighbors in axial coordinates
pub const HEX_NEIGHBORS: [(i32, i32); 6] = [
    (1, 0),     // E
    (-1, 0),    // W
    (0, 1),     // SE
    (0, -1),    // NW
    (1, -1),    // NE
    (-1, 1),    // SW
];

/// Check if a prism at given coordinates has structural support
///
/// A prism has support if:
/// - It's at ground level (level <= 0)
/// - There's a prism directly below it
/// - It has at least 2 supports from neighbors (horizontal or diagonal)
///
/// # Arguments
/// * `q` - Axial Q coordinate
/// * `r` - Axial R coordinate
/// * `level` - Vertical level
/// * `grid_contains` - Function to check if grid contains prism at coord
///
/// # Returns
/// true if the prism has structural support
pub fn has_support<F>(q: i32, r: i32, level: i32, grid_contains: F) -> bool
where
    F: Fn(i32, i32, i32) -> bool,
{
    // Ground level always has support
    if level <= 0 {
        return true;
    }
    
    // Check if there's a prism directly below
    if grid_contains(q, r, level - 1) {
        return true;
    }
    
    // Check for diagonal support from neighbors (structural support)
    // A prism can be supported if at least 2 adjacent neighbors at same level exist
    let mut neighbor_support = 0;
    for (dq, dr) in HEX_NEIGHBORS {
        let nq = q + dq;
        let nr = r + dr;
        
        // Check neighbor at same level (horizontal structural support)
        if grid_contains(nq, nr, level) {
            neighbor_support += 1;
        }
        // Check neighbor below (diagonal support)
        if grid_contains(nq, nr, level - 1) {
            neighbor_support += 1;
        }
    }
    
    // Need at least 2 supports to stay standing (structural integrity)
    neighbor_support >= 2
}

/// Find all prisms that would lose support if a prism at given coordinates is destroyed
///
/// # Arguments
/// * `destroyed_coord` - Coordinates of the destroyed prism (q, r, level)
/// * `grid_contains` - Function to check if grid contains prism at coord
/// * `max_cascade_levels` - Maximum levels above to check for cascade (default: 3)
///
/// # Returns
/// Vec of coordinates that would lose support
pub fn find_unsupported_cascade<F>(
    destroyed_coord: (i32, i32, i32),
    grid_contains: F,
    max_cascade_levels: i32,
) -> Vec<(i32, i32, i32)>
where
    F: Fn(i32, i32, i32) -> bool,
{
    let (q, r, level) = destroyed_coord;
    let mut prisms_to_fall = Vec::new();
    
    // Check prism directly above
    if grid_contains(q, r, level + 1) {
        if !has_support(q, r, level + 1, &grid_contains) {
            prisms_to_fall.push((q, r, level + 1));
        }
    }
    
    // Check prisms in adjacent columns that might have lost support
    for (dq, dr) in HEX_NEIGHBORS {
        let nq = q + dq;
        let nr = r + dr;
        
        // Check if neighbor at same level or above has lost support
        for check_level in level..=(level + max_cascade_levels) {
            if grid_contains(nq, nr, check_level) {
                if !has_support(nq, nr, check_level, &grid_contains) {
                    prisms_to_fall.push((nq, nr, check_level));
                }
            }
        }
    }
    
    prisms_to_fall
}

/// Check if a falling prism collides with any nearby prisms in the grid
///
/// # Arguments
/// * `approx_q` - Approximate Q coordinate of falling prism
/// * `approx_r` - Approximate R coordinate of falling prism
/// * `approx_level` - Approximate level of falling prism
/// * `grid_contains` - Function to check if grid contains prism at coord
///
/// # Returns
/// Option with the coordinate of the collided prism, if any
pub fn check_falling_prism_collision<F>(
    approx_q: i32,
    approx_r: i32,
    approx_level: i32,
    grid_contains: F,
) -> Option<(i32, i32, i32)>
where
    F: Fn(i32, i32, i32) -> bool,
{
    for dq in -1..=1 {
        for dr in -1..=1 {
            for dl in -1..=1 {
                let check_coord = (approx_q + dq, approx_r + dr, approx_level + dl);
                if grid_contains(check_coord.0, check_coord.1, check_coord.2) {
                    return Some(check_coord);
                }
            }
        }
    }
    None
}
