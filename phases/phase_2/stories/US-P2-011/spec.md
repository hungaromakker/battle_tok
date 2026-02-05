# US-P2-011: Enhance Meteor System with Fire Trail

## Description
Update the meteor rendering to use brighter HDR emission and integrate with the particle system for fire trails - this makes meteors dramatic fireballs instead of simple colored spheres.

## The Core Concept / Why This Matters
The reference art shows blazing meteors streaking across the sky. The current meteor system exists but needs enhancement:

1. **HDR emission** - Meteors should output values 3.0+ so they bloom after tonemapping
2. **Fire trail** - Spawn ember particles along the trajectory
3. **Impact burst** - Spawn particle explosion on ground hit
4. **Tumbling** - Rotation during flight for visual interest

Meteors are dramatic visual punctuation - they should demand attention.

## Goal
Update `src/game/destruction.rs` to make meteors visually dramatic with HDR values and particle trails.

## Files to Create/Modify
- `src/game/destruction.rs` — Enhance meteor rendering and particle integration

## Implementation Steps
1. Find the Meteor struct/rendering in `destruction.rs`

2. Update meteor material/color to HDR values:
   ```rust
   let meteor_color = Vec3::new(3.5, 1.0, 0.2);  // Bright HDR orange
   ```

3. In meteor update loop, spawn trail particles:
   ```rust
   // Every N frames, spawn ember at meteor position
   if frame % 3 == 0 {
       particle_system.spawn_ember(meteor.position);
   }
   ```

4. On meteor impact (when it hits ground):
   ```rust
   // Spawn burst of 20-50 particles
   for _ in 0..30 {
       let offset = random_unit_sphere() * 2.0;
       particle_system.spawn_ember(impact_pos + offset);
   }
   ```

5. Add tumbling rotation:
   ```rust
   meteor.rotation += meteor.angular_velocity * dt;
   ```

6. Run `cargo check`

## Code Patterns
Meteor with HDR color (if using instancing):
```rust
#[repr(C)]
pub struct MeteorInstance {
    pub position: [f32; 3],
    pub scale: f32,
    pub color: [f32; 3],  // HDR values > 1.0
    pub intensity: f32,    // Multiplier for emission
}

// When creating meteor:
MeteorInstance {
    color: [3.5, 1.0, 0.2],  // Bright HDR orange
    intensity: 2.5,
    // ...
}
```

Trail spawning:
```rust
impl MeteorSpawner {
    pub fn update(&mut self, dt: f32, particle_system: &mut ParticleSystem) {
        for meteor in &mut self.meteors {
            meteor.position += meteor.velocity * dt;

            // Spawn trail particles
            self.trail_timer += dt;
            if self.trail_timer > 0.05 {  // Every 50ms
                particle_system.spawn_ember(meteor.position.into());
                self.trail_timer = 0.0;
            }

            // Check for ground impact
            if meteor.position.y < 0.0 {
                self.spawn_impact_burst(meteor.position, particle_system);
                meteor.alive = false;
            }
        }
    }
}
```

## Acceptance Criteria
- [ ] Meteors appear as bright fireballs (HDR orange, not flat color)
- [ ] Trail of ember particles follows each meteor
- [ ] Impact on ground creates particle burst
- [ ] Meteors have tumbling rotation during flight
- [ ] `cargo check` passes

## Success Looks Like
When meteors fall:
- They're BRIGHT - clearly visible against the dark sky
- A stream of glowing particles trails behind each one
- When they hit the ground, there's a satisfying burst of sparks
- They tumble/rotate as they fall, not just translate
- The whole effect is dramatic and eye-catching

## Dependencies
- Depends on: US-P2-008 (particle system exists)
