# Visual Upgrade TODO

This document lists what remains to match `visualupgrade.md` and the reference image.

## Implemented (already in repo)
- Storm sky shaders: `shaders/stormy_sky.wgsl`, `shaders/sky_storm_procedural.wgsl` (wired in `src/bin/battle_arena.rs`)
- Fog post pass: `shaders/fog_post.wgsl`, `engine/src/render/fog_post.rs` (wired)
- Tonemap (ACES): `shaders/tonemap_aces.wgsl` (inline in `src/game/render/shader.rs`)
- Lava shader: `shaders/lava.wgsl`
- Terrain enhanced shader: `shaders/terrain_enhanced.wgsl`
- Castle stone shader: `shaders/castle_stone.wgsl` + `engine/src/render/castle_material.rs`
- Flag shader: `shaders/flag.wgsl` + `engine/src/render/flag_material.rs`
- Bridge materials: `shaders/wood_plank.wgsl`, `shaders/chain_metal.wgsl` + `engine/src/render/bridge_materials.rs`
- Ember particles: `shaders/ember_particle.wgsl` + `engine/src/render/particles.rs`

## Missing or not wired
- Water material/shader (lake/river water)
- Rock/cliff material/shader (island edges)
- Bloom pass (extract + blur + combine)
- Heat distortion (lava shimmer post)
- Ash wind overlay (fullscreen post)
- Smoke columns / battlefield haze
- Lightning flash overlay (global scene flash)
- Weapon glow / attack FX shader
- Outline post (ID mask or depth/normal)
- Destruction shader variants (burn/dissolve)

## Recommended implementation order (highest impact first)
1. Water shader + material + pipeline
2. Rock/cliff shader + material + pipeline
3. Bloom post chain
4. Heat distortion post
5. Ash wind overlay + smoke columns
6. Lightning flash overlay
7. Outline post
8. Weapon/attack FX
9. Destruction shader variants

## Integration notes
- Keep each material in its own WGSL file under `shaders/`.
- Mirror existing patterns: `engine/src/render/stormy_sky.rs`, `engine/src/render/castle_material.rs`, `engine/src/render/bridge_materials.rs`.
- Wire post passes in the render chain similar to `FogPostPass`.
