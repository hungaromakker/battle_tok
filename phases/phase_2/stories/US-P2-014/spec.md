# US-P2-014: Integrate All Systems into battle_arena.rs

## Description
Wire up all new visual systems (sky, materials, lights, particles, fog, tonemap) into the main game loop with correct render order - this is the final integration that brings everything together.

## The Core Concept / Why This Matters
All the individual pieces exist:
- Apocalyptic sky preset
- Enhanced lava with HDR
- Castle stone material
- Point light system
- Bridge materials
- Flag with wind
- Ember particles
- Depth fog
- ACES tonemapping
- Apocalyptic terrain
- Material system

But they need to be INTEGRATED in the correct order. Rendering order matters:
1. Sky first (no depth, background)
2. Opaque geometry (terrain, castles, bridge)
3. Emissive (lava, meteors)
4. Transparent/particles (embers with additive blend)
5. Post-processing (fog, then tonemap)
6. UI last

## Goal
Update `src/bin/battle_arena.rs` to use all new systems in correct render order.

## Files to Create/Modify
- `src/bin/battle_arena.rs` — Main integration file

## Implementation Steps
1. Add imports for all new modules:
   ```rust
   use engine::render::{
       stormy_sky::{StormySky, StormySkyConfig},
       point_lights::PointLightManager,
       particles::ParticleSystem,
       fog_post::FogPostPass,
       material_system::{MaterialSystem, MaterialType},
   };
   ```

2. Initialize all systems in setup:
   ```rust
   // Sky with battle_arena preset
   let stormy_sky = StormySky::with_config(&device, format, StormySkyConfig::battle_arena());

   // Point lights for torches
   let mut point_lights = PointLightManager::new(&device);
   point_lights.add_torch(castle1_pos + Vec3::Y * 2.0, Vec3::new(1.0, 0.6, 0.3), 10.0);
   // ... add more torches

   // Particle system
   let mut particles = ParticleSystem::new(&device);

   // Material system
   let material_system = MaterialSystem::new(&device, format);

   // Post-processing
   let fog_pass = FogPostPass::new(&device, format);
   let tonemap_pass = TonemapPass::new(&device, format);
   ```

3. Update loop - update all systems:
   ```rust
   // Lightning trigger
   lightning_timer -= dt;
   if lightning_timer <= 0.0 {
       stormy_sky.trigger_lightning();
       lightning_timer = random_range(3.0, 8.0);
   }

   // Update point lights (flicker)
   point_lights.update(&queue, time);

   // Update particles
   particles.update(dt);
   particles.spawn_embers_near_lava(&lava_positions);

   // Update meteors with particle trails
   meteors.update(dt, &mut particles);
   ```

4. Render loop - correct order:
   ```rust
   // 1. Render to HDR texture
   {
       let mut pass = encoder.begin_render_pass(&hdr_pass_desc);

       // Sky (first, no depth)
       stormy_sky.render(&mut pass);

       // Terrain
       material_system.render_with_material(&mut pass, MaterialType::Terrain, &terrain_mesh);

       // Lava (emissive)
       material_system.render_with_material(&mut pass, MaterialType::Lava, &lava_mesh);

       // Castle buildings
       material_system.render_with_material(&mut pass, MaterialType::CastleStone, &buildings_mesh);

       // Bridge
       material_system.render_with_material(&mut pass, MaterialType::WoodPlank, &planks_mesh);
       material_system.render_with_material(&mut pass, MaterialType::ChainMetal, &chains_mesh);

       // Flags
       material_system.render_with_material(&mut pass, MaterialType::Flag, &flags_mesh);

       // Meteors
       meteors.render(&mut pass);

       // Particles (additive blend)
       particles.render(&mut pass);
   }

   // 2. Fog post-pass
   fog_pass.render(&mut encoder, &hdr_view, &depth_view, &fog_output_view);

   // 3. Tonemap to swapchain
   tonemap_pass.render(&mut encoder, &fog_output_view, &swapchain_view);

   // 4. UI overlay
   ui.render(&mut encoder, &swapchain_view);
   ```

5. Run `cargo check` and `cargo run --bin battle_arena`

## Code Patterns
HDR render target setup:
```rust
let hdr_texture = device.create_texture(&wgpu::TextureDescriptor {
    format: wgpu::TextureFormat::Rgba16Float,  // HDR!
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
    // ...
});
```

Post-pass chain:
```rust
// Scene → HDR texture
// HDR texture → Fog post → Fog output texture
// Fog output → Tonemap → Swapchain
```

## Acceptance Criteria
- [ ] All systems initialized without panics
- [ ] Render order is correct (no z-fighting, particles visible)
- [ ] Sky shows apocalyptic colors
- [ ] Lava glows brightly (HDR bloom)
- [ ] Castle stone has brick pattern and torch lighting
- [ ] Embers float up from lava
- [ ] Fog adds depth to distant objects
- [ ] Tonemapping produces cinematic output
- [ ] Performance maintains 60fps
- [ ] `cargo check` passes
- [ ] `cargo run --bin battle_arena` runs without errors

## Success Looks Like
When running the game:
- The scene matches the concept art - dramatic and apocalyptic
- Purple stormy sky with periodic lightning
- Glowing orange lava rivers with floating embers
- Castle walls with realistic brick and torch lighting
- Fog fades distant objects into the atmosphere
- Everything has a cinematic, filmic quality (ACES tonemap)
- It's VISUALLY STUNNING and looks like a AAA game

## Dependencies
- Depends on: ALL previous stories (US-P2-001 through US-P2-013)
- This is the FINAL integration story
