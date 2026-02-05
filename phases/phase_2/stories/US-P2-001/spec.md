# US-P2-001: Integrate Apocalyptic Sky Preset

## Description
Update battle_arena.rs to use the pre-built `StormySkyConfig::battle_arena()` preset and add periodic lightning triggers - this sets the dramatic atmospheric foundation for the entire visual upgrade.

## The Core Concept / Why This Matters
The sky is the first thing players see and sets the entire mood of the apocalyptic battle arena. Currently the sky may be using default settings that don't match our concept art showing a dramatic purple/orange stormy atmosphere with lightning. The `StormySkyConfig::battle_arena()` preset already exists in `engine/src/render/stormy_sky.rs` with the correct colors (purple zenith, orange-red fog, active lightning). This story simply wires it up and adds the periodic lightning trigger system.

This is the FOUNDATION story - all other visual improvements will look better against a properly dramatic sky backdrop.

## Goal
Make the sky match the concept art by using the existing battle_arena preset and triggering lightning every 3-8 seconds.

## Files to Create/Modify
- `src/bin/battle_arena.rs` — Change sky initialization and add lightning trigger logic

## Implementation Steps
1. Find where `StormySky::new()` is called in battle_arena.rs
2. Change it to `StormySky::with_config(device, format, StormySkyConfig::battle_arena())`
3. Add a `lightning_timer: f32` field to track time since last lightning
4. In the update loop, decrement timer and call `stormy_sky.trigger_lightning()` when it reaches 0
5. Reset timer to random value between 3.0 and 8.0 seconds
6. Import `StormySkyConfig` at the top of the file
7. Run `cargo check` to verify compilation

## Code Patterns
From `engine/src/render/stormy_sky.rs`:
```rust
// The preset we want to use:
pub fn battle_arena() -> Self {
    Self {
        cloud_speed: 0.12,
        lightning_intensity: 0.8,
        cloud_color1: Vec3::new(0.85, 0.45, 0.25),  // Orange-lit cloud edges
        cloud_color2: Vec3::new(0.12, 0.08, 0.18),  // Deep purple shadows
        upper_color: Vec3::new(0.15, 0.08, 0.25),   // Dark purple zenith
        fog_color: Vec3::new(0.55, 0.25, 0.15),     // Warm orange-red
        ...
    }
}

// How to trigger lightning:
pub fn trigger_lightning(&mut self) {
    self.config.lightning_intensity = 1.0;
}
```

## Acceptance Criteria
- [ ] Sky shows dark purple at zenith (top of sky)
- [ ] Orange-red fog visible at horizon
- [ ] Lightning flashes occur every 3-8 seconds (random interval)
- [ ] `cargo check` passes without errors
- [ ] `cargo run --bin battle_arena` starts without panics

## Success Looks Like
When you run the game, you should immediately notice:
- The sky is dramatically darker with purple tones at the top
- The horizon glows with warm orange-red colors (like distant lava)
- Every few seconds, the entire sky briefly flashes white/blue (lightning)
- The atmosphere feels apocalyptic and stormy, matching the concept art

## Dependencies
- Depends on: None (this is the first story)
