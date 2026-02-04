# Battle Sphere - Technical Constraints

## Core Philosophy: Build Everything From Scratch

**NO EXTERNAL DEPENDENCIES** - We study reference implementations (Bevy, Rapier, etc.) and build our own versions from first principles.

### Why Build Our Own
- Full control over performance optimization
- Deep understanding of every system
- No dependency bloat
- Tailor-made for our specific use case (10,000+ FPS target)
- No license concerns

### Reference Code (Study Only)
| System | Reference | Our Implementation |
|--------|-----------|-------------------|
| Physics | Rapier | Custom physics engine |
| ECS | Bevy ECS | Custom entity system |
| Math | glam internals | SIMD-optimized math |
| Collision | Rapier broadphase | Custom spatial partitioning |

---

## Rendering Strategy

### Hybrid Hex-SDF-Instanced Architecture (FINAL)

| Element | Method | Reason |
|---------|--------|--------|
| **Hex Planet terrain** | Triangle mesh + LOD | Already working in `hex_planet.rs` |
| **Buildings/Walls** | Hexagonal prism voxels | Matches hex grid, stackable, organic |
| **Siege weapons** | SDF (fallback: pre-baked mesh) | Complex curves, Magic Engine native |
| **Characters/Units** | **Pre-baked mesh + GPU instancing** | Many units on screen, 10K FPS target |
| **Projectiles** | SDF | Real-time physics deformation |
| **Destruction/Debris** | SDF particles | Procedural, no pre-made meshes |
| **Hit effects** | SDF | Native strength |

### Unit Rendering Pipeline (NEW)

```
Design Phase:              Runtime:
┌──────────────┐           ┌─────────────────────────┐
│ SDF Model    │           │ Single draw call        │
│ (infinite    │ ──bake──► │ per unit type           │
│  detail)     │           │                         │
└──────────────┘           │ Instance buffer:        │
                           │ - position (vec3)       │
Create LOD variants:       │ - rotation (quat)       │
- LOD0: 5000 tris         │ - animation_frame (u32) │
- LOD1: 1000 tris         │ - team_color (u32)      │
- LOD2: 200 tris          │ - health_state (u32)    │
                           └─────────────────────────┘
```

**Why pre-baked + instancing for units:**
- Strategy games have 100s-1000s of units on screen
- SDF ray marching per unit = too expensive at scale
- GPU instancing: 1 draw call renders ALL units of same type
- Bake from SDF preserves quality during design phase

### Hexagonal Prism Voxels (Not Cubes)

```
Side view:              Top view (matches planet):
   ⬡                         ⬡ ⬡ ⬡
   ⬡                        ⬡ ⬡ ⬡ ⬡
   ⬡  ← stackable            ⬡ ⬡ ⬡
   ⬡     layers
  ━━━ (hex tile base)
```

Benefits:
- Walls naturally align to hex terrain edges
- No visible cube grid artifacts
- 6-way symmetry looks deliberate, not glitchy
- Easy neighbor queries (same as hex planet grid)
- Micro-voxels (0.1-0.5 units) appear smooth

### SDF Fallback Strategy

If SDF performance is too heavy:
1. First try: SDF with LOD (simpler SDF at distance)
2. Fallback: Pre-bake mesh from SDF (offline conversion)
3. Last resort: Hand-made low-poly mesh

---

## Performance Target

### 10,000+ FPS Goal

| Metric | Target |
|--------|--------|
| Frame time | < 0.1ms |
| Draw calls | < 50 (instancing reduces dramatically) |
| GPU memory | < 500MB |
| CPU physics | < 0.05ms |
| Unit count | 10,000+ with instancing |

### Optimization Techniques

1. **GPU Instancing** - Single draw call per:
   - All hex prisms (buildings)
   - All units of same type (soldiers, archers, etc.)
   - All projectiles of same type
2. **Octree Culling** - Only render visible hexes
3. **LOD System** - Distance-based mesh simplification
   - Units: LOD0/LOD1/LOD2 based on camera distance
   - Hex terrain: subdivision level based on view
4. **Compute Shaders** - Physics, collision on GPU
5. **Memory Pooling** - No runtime allocations
6. **Batch SDF Objects** - Group similar SDF shapes

---

## Custom Physics Engine

### Built From First Principles

Study Rapier's architecture, implement our own:

```rust
// Our physics - no rapier dependency
pub struct PhysicsWorld {
    bodies: Vec<RigidBody>,
    colliders: Vec<Collider>,
    gravity: Vec3,
    // Custom spatial partitioning for hex grid
    hex_grid: HexSpatialHash,
}

pub trait RigidBody {
    fn position(&self) -> Vec3;
    fn velocity(&self) -> Vec3;
    fn apply_force(&mut self, force: Vec3);
    fn integrate(&mut self, dt: f32);
}
```

### Key Systems to Implement

| System | Reference | Complexity |
|--------|-----------|------------|
| Rigid body dynamics | Rapier RigidBody | Medium |
| Collision detection | Rapier Collider | High |
| Broadphase | Rapier BroadPhase | Medium |
| Constraint solver | Rapier Island solver | Very High |
| Projectile trajectories | Basic physics | Low |

### Phase 1 Scope (Simplified)

For Phase 1, implement only:
- Projectile ballistics (gravity + air resistance)
- Ray-hex collision (already have hex mesh)
- Impact point calculation

Full physics (rigid body stacking, constraints) comes later.

---

## Siege Weapon Systems

### Pártázány Ágyú (Siege Cannon)

| Property | Value |
|----------|-------|
| Rendering | SDF (smooth barrel curves) |
| Projectile | SDF sphere/custom shape |
| Trajectory | Custom ballistics (no Rapier) |
| Reload | State machine |

### Trebuchet

| Property | Value |
|----------|-------|
| Rendering | SDF (arm, sling, frame) |
| Physics | Custom constraint solver |
| Projectile crafting | Player-defined shapes |

---

## Visual Style

### Target: Realistic Strategy (AoE2 / Total War feel)

- NOT Fortnite cartoon
- Stylized realism
- Detailed siege weapons
- Atmospheric lighting
- Hex tiles visible but integrated

### Shader Requirements

Current `hex_terrain.wgsl` needs:
- [ ] PBR materials (specular, roughness, metallic)
- [ ] Edge softening for hex boundaries
- [ ] Atmospheric scattering
- [ ] Shadow mapping
- [ ] Ambient occlusion

---

## File Organization

```
magic_engine/
├── src/
│   ├── physics/           # Custom physics (study Rapier)
│   │   ├── mod.rs
│   │   ├── rigid_body.rs
│   │   ├── collider.rs
│   │   ├── broadphase.rs
│   │   └── ballistics.rs  # Projectile trajectories
│   ├── rendering/
│   │   ├── hex_prism.rs   # Hex voxel system
│   │   ├── sdf_objects.rs # SDF siege weapons
│   │   ├── instancing.rs  # GPU instancing for units
│   │   └── sdf_baker.rs   # SDF → mesh conversion (offline)
│   └── game/
│       └── battle_sphere/
│           ├── siege_weapons.rs
│           ├── projectiles.rs
│           └── units.rs    # Unit types, instance data
├── shaders/
│   ├── hex_terrain.wgsl   # Existing (needs polish)
│   ├── hex_prism.wgsl     # New - voxel buildings
│   ├── sdf_objects.wgsl   # New - siege weapons
│   └── instanced_mesh.wgsl # New - instanced units
└── assets/
    └── units/             # Pre-baked unit meshes
        ├── soldier_lod0.mesh
        ├── soldier_lod1.mesh
        └── soldier_lod2.mesh
```

---

## Development Phases

### Phase 1: Combat Prototype
- Single hex tile terrain
- Basic hex-prism walls (player-built)
- Single siege weapon (cannon, SDF)
- Projectile with custom ballistics
- Target destruction (hex-prism removal)

### Phase 2: Full Planet
- Complete icosphere hex planet
- Multiple siege weapons
- Units (SDF characters)
- Full physics (built from scratch)

### Phase 3: Multiplayer
- Network code (custom, no external libs)
- State synchronization
- Latency compensation
