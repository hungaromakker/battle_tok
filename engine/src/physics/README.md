# Physics Module Architecture

Custom physics implementation for Magic Engine / Battle Sphere. Built from scratch without external physics libraries (Rapier, etc.).

## Philosophy

Study reference implementations, understand the algorithms, build our own optimized versions. This gives us:
- Full control over performance
- No external dependency bloat
- Deep understanding of physics math
- Custom optimizations for SDF-based collision

## Module Structure

```
engine/src/physics/
├── mod.rs          # Module exports, re-exports common types
├── types.rs        # Math type re-exports (Vec3, Quat from glam)
├── ballistics.rs   # Projectile trajectory simulation
├── collision.rs    # Ray-AABB collision detection
└── README.md       # This file
```

## Types Overview

### Projectile (ballistics.rs)

Represents a projectile being simulated through the air.

```rust
pub struct Projectile {
    pub position: Vec3,           // Current position (meters)
    pub velocity: Vec3,           // Velocity vector (m/s)
    pub mass: f32,                // Mass (kg)
    pub drag_coefficient: f32,    // Cd, typically 0.1-1.0
    pub radius: f32,              // Radius for collision (m)
    pub active: bool,             // Still being simulated?
}
```

**Default values:**
- mass: 1.0 kg
- drag_coefficient: 0.47 (sphere)
- radius: 0.05 m (10cm diameter cannonball)

### BallisticsConfig (ballistics.rs)

Global physics environment configuration.

```rust
pub struct BallisticsConfig {
    pub gravity: Vec3,      // Gravity acceleration (m/s²)
    pub air_density: f32,   // Air density (kg/m³)
}
```

**Default values (Earth sea level):**
- gravity: Vec3(0, -9.81, 0)
- air_density: 1.225 kg/m³

### ProjectileState (ballistics.rs)

Tracks the current state of a projectile.

```rust
pub enum ProjectileState {
    Flying,                                    // In flight
    Hit { position: Vec3, normal: Vec3 },     // Impacted something
    Expired,                                   // Exceeded lifetime/bounds
}
```

## Physics Math (Future Implementation)

### Ballistics Integration (US-007)

Semi-implicit Euler integration (simple and stable):

```
acceleration = gravity + (drag_force / mass)
velocity += acceleration * dt
position += velocity * dt
```

### Air Drag Formula

Quadratic drag for high-speed projectiles:

```
F_drag = -0.5 * air_density * Cd * A * |v|² * normalize(v)

Where:
- air_density = 1.225 kg/m³ (sea level)
- Cd = drag_coefficient (0.47 for sphere)
- A = π * radius² (cross-sectional area)
- v = velocity vector
```

### Collision Detection (collision.rs)

Ray-AABB intersection using the slab method:

```rust
pub fn ray_aabb_intersect(
    ray_origin: Vec3,
    ray_dir: Vec3,     // Must be normalized
    aabb_min: Vec3,
    aabb_max: Vec3,
) -> Option<f32>  // Returns hit distance or None

pub fn aabb_surface_normal(
    point: Vec3,
    aabb_min: Vec3,
    aabb_max: Vec3,
) -> Vec3  // Returns outward normal

pub struct HitInfo {
    pub position: Vec3,
    pub normal: Vec3,
    pub prism_coord: (i32, i32, i32),
    pub distance: f32,
}
```

**Algorithm**: The slab method computes intersection times for each axis-aligned plane pair, then finds the overlap to determine if/where the ray enters the box.

## Unit System

**1 unit = 1 meter** (SI units throughout)

| Object | Typical Size |
|--------|--------------|
| Cannonball | 0.1 m diameter |
| Player height | 1.8 m |
| Hex tile | ~50 m radius |
| Cannon barrel | 4 m length |

## Performance Targets

| Metric | Target |
|--------|--------|
| Projectile updates/frame | 1000+ at 60 FPS |
| Collision checks/frame | O(n × m) with spatial partitioning |
| Integration timestep | Variable dt (frame-independent) |

## Dependencies

- `glam` - Math library (Vec3, Quat, Mat4)
- No external physics libraries

## Usage

```rust
use magic_engine::physics::{Projectile, BallisticsConfig, ProjectileState};
use glam::Vec3;

// Create environment config
let config = BallisticsConfig::default();

// Spawn a projectile
let mut projectile = Projectile {
    position: Vec3::new(0.0, 10.0, 0.0),
    velocity: Vec3::new(50.0, 30.0, 0.0),  // 50 m/s forward, 30 m/s up
    mass: 5.0,
    ..Default::default()
};

// Future: integrate in game loop
// projectile.integrate(&config, delta_time);
```

## Phase 1 Status

| Component | Status |
|-----------|--------|
| Projectile type definitions | Complete |
| BallisticsConfig | Complete |
| ProjectileState | Complete |
| Ray-AABB collision | Complete |
| HitInfo struct | Complete |
| Surface normal calculation | Complete |
| Unit tests | Complete |
| Ballistics integration | Pending (US-007) |
| HexPrismGrid integration | Pending |
| Spatial partitioning | Phase 2 |

## Phase 2 Roadmap

1. **US-007**: Implement `Projectile::integrate()` with drag
2. Create `HexPrismGrid::ray_cast()` using `ray_aabb_intersect()`
3. **US-016**: Connect collision to hex-prism destruction
4. Spatial partitioning for O(log n) collision queries
5. Rigid body physics for debris/destruction effects
