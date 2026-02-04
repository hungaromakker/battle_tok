# Battle Sphere - Reference Materials

## Study These (Don't Depend On Them)

### Physics - Rapier

**Repository**: https://github.com/dimforge/rapier

Key files to study:
- `src/dynamics/rigid_body.rs` - Rigid body implementation
- `src/geometry/collider.rs` - Collision shapes
- `src/pipeline/physics_pipeline.rs` - Integration loop
- `src/dynamics/solver/` - Constraint solver (complex)

What to extract:
- Verlet/semi-implicit Euler integration
- GJK/EPA collision detection
- Broadphase spatial partitioning
- Island-based constraint solving

### Entity System - Bevy ECS

**Repository**: https://github.com/bevyengine/bevy

Key files to study:
- `crates/bevy_ecs/src/world/mod.rs` - World storage
- `crates/bevy_ecs/src/query/` - Query system
- `crates/bevy_ecs/src/schedule/` - System scheduling

What to extract:
- Archetype-based storage
- Sparse set components
- System parallelization

### Math - glam

**Repository**: https://github.com/bitshifter/glam-rs

Already using as dependency (acceptable - pure math, no runtime).

### Rendering Techniques

**Oscar Stalberg** (mesh-based terrain):
- GDC talks on procedural generation
- Townscaper/Bad North techniques

**Vercidium** (voxel optimization):
- YouTube: Voxel rendering at 10K FPS
- Instancing + LOD techniques

**Inigo Quilez** (SDF):
- https://iquilezles.org/articles/
- SDF primitives, smooth operations

---

## Implementation Order

### Phase 1: Minimal Custom Physics

```rust
// ballistics.rs - Start here
pub struct Projectile {
    position: Vec3,
    velocity: Vec3,
    mass: f32,
    drag: f32,
}

impl Projectile {
    pub fn integrate(&mut self, dt: f32, gravity: Vec3) {
        // Semi-implicit Euler (simple, stable)
        let drag_force = -self.velocity * self.drag;
        let acceleration = gravity + drag_force / self.mass;
        self.velocity += acceleration * dt;
        self.position += self.velocity * dt;
    }
}
```

### Phase 2: Ray-Hex Collision

Use existing hex mesh, implement ray intersection.

### Phase 3: Full Rigid Body (Later)

Study Rapier's constraint solver when needed.

---

## Notes

- **Don't copy code** - Understand algorithms, rewrite
- **Optimize for hex grid** - Generic physics libraries don't know about hex
- **GPU-first** - Put physics in compute shaders when possible
