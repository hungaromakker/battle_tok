# Battle Sphere - Summary

## Game Concept

Turn-based/real-time strategy game on a hex-tiled spherical planet. Players build fortifications, craft siege weapons, and battle for territory.

## Core Philosophy

**Build everything from scratch** - No external game engines (Bevy) or physics libraries (Rapier). Study their code, understand the algorithms, implement our own optimized versions.

## Technical Stack

| Component | Approach |
|-----------|----------|
| Engine | Magic Engine (custom Rust + wgpu) |
| Physics | Custom implementation (study Rapier) |
| ECS | Custom entity system |
| Rendering | Hybrid Hex-SDF |

## Rendering Strategy (FINAL)

| Element | Method |
|---------|--------|
| Hex Planet | Triangle mesh + LOD (existing `hex_planet.rs`) |
| Buildings/Walls | Hexagonal prism voxels |
| Siege Weapons | SDF (fallback: pre-baked mesh) |
| **Characters/Units** | **Pre-baked mesh + GPU instancing** |
| Projectiles | SDF |
| Destruction | SDF particles |
| Hit Effects | SDF |

### Unit Pipeline
1. Design in SDF (infinite detail)
2. Bake to optimized mesh (offline)
3. Create LOD variants (high/med/low)
4. Instance at runtime (1 draw call per unit type)

## Why Hex-Prism Voxels (Not Cubes)

- Match hex planet grid naturally
- No visible cube artifacts
- 6-way symmetry looks organic
- Micro-voxels (0.1-0.5 units) appear smooth

## Performance Target

**10,000+ FPS** (< 0.1ms frame time)

## Phase 1 Focus

1. Single hex terrain tile
2. Hex-prism wall building (first-person)
3. One siege weapon (cannon) using SDF
4. Custom projectile ballistics
5. Target destruction

## Key Files

- Existing: `src/bin/hex_planet.rs` (1823 lines, working prototype)
- Existing: `shaders/hex_terrain.wgsl` (needs visual polish)
- TODO: Custom physics in `src/physics/`
- TODO: Hex-prism voxel system
- TODO: SDF siege weapons
- TODO: GPU instancing system for units
- TODO: SDF-to-mesh baker (offline tool)

## Visual Style

Realistic strategy game (AoE2/Total War inspired), not cartoon. Hex tiles visible but integrated aesthetically.

---

## Phase 1 Status (Completed)

### What Was Built

Phase 1 established the foundational architecture for Battle Sphere's combat prototype:

#### Physics Module (`engine/src/physics/`)

| File | Purpose | Status |
|------|---------|--------|
| `mod.rs` | Module exports and re-exports | Complete |
| `types.rs` | Vec3/Quat re-exports from glam | Complete |
| `ballistics.rs` | Projectile physics types with gravity/drag | Complete |
| `collision.rs` | Ray-AABB collision detection | Complete |

**Ballistics types:**
- **Projectile struct**: position, velocity, mass, drag_coefficient, radius, active
- **BallisticsConfig**: gravity vector, air_density
- **ProjectileState**: Flying, Hit(position, normal), Expired

**Collision types:**
- **HitInfo struct**: position, normal, prism_coord, distance
- **ray_aabb_intersect()**: Slab method for ray-box intersection
- **aabb_surface_normal()**: Compute outward normal at hit point

**Unit tests**: Default values, state transitions, collision tests

#### Rendering Shaders (`shaders/`)

| Shader | Purpose | Status |
|--------|---------|--------|
| `hex_terrain.wgsl` | Mesh terrain with lighting, fog, magma/crystal effects | Complete |
| `hex_prism.wgsl` | Hex-prism voxel walls with Lambert lighting | Complete |
| `raymarcher.wgsl` | Core SDF raymarching (existing engine) | Stable |

#### Key Architecture Decisions

1. **No External Physics**: Custom ballistics without Rapier dependency
2. **SI Units**: 1 unit = 1 meter throughout codebase
3. **Hybrid Rendering**: Mesh terrain + hex-prism voxels + SDF objects
4. **Matching Lighting**: Both terrain and prism shaders use same sun direction and fog

### FPS Benchmarks

| Configuration | Platform | FPS |
|---------------|----------|-----|
| Hex terrain only | Software renderer (WSL2) | ~60 FPS (vsync) |
| Hex terrain only | GPU | 300+ FPS |
| Raymarcher (complex SDF) | GPU | 60-300 FPS (scene dependent) |

*Note: Phase 1 focused on architecture, not optimization. Phase 2 adds tile-based culling for 1000+ FPS.*

### Known Limitations

1. **No battle_arena.rs binary yet** - Arena integration pending
2. **Ballistics integration not implemented** - Types defined, no `integrate()` method
3. **No hex-prism mesh generation** - Data structures need `generate_mesh()`
4. **No HexPrismGrid::ray_cast()** - Needs to use ray_aabb_intersect()
5. **No cannon SDF model** - `SdfObject` framework not created
6. **FPS below target** - Optimization pass (US-019) not complete

### Phase 2 Improvements

1. **Tile-Based Culling** (PRD US-010, US-017)
   - Pre-pass compute shader projects entity bounds
   - 16x16 pixel tiles get entity lists
   - Expected: O(pixels × rays × ~10) vs O(pixels × rays × entities)

2. **Battle Arena Binary**
   - Two floating hex tiles (attacker/defender)
   - Camera system from hex_planet.rs
   - Combined render passes: terrain + voxels + SDF

3. **Complete Physics Pipeline**
   - Semi-implicit Euler integration
   - Quadratic air drag: F = -0.5 × ρ × Cd × A × v² × v̂
   - Ray-AABB collision (implemented: `ray_aabb_intersect()`)
   - Connect HexPrismGrid to collision system

4. **SDF Objects Framework**
   - `SdfPrimitive` enum (Sphere, Cylinder, Box, RoundedBox)
   - `SdfOperation` enum (Union, Intersection, Subtraction, SmoothUnion)
   - Cannon model as primitive composition

---

## Ready for Phase 2

Phase 1 foundations in place:
- Physics module structure
- Ballistics types with proper physics units
- Hex-prism shader ready for mesh data
- Hex terrain shader with emissive effects
- Architecture documented for new developers

Next: Implement ballistics integration (US-007) and hex-prism mesh generation (US-008).
