# Phase 2: Visual Upgrade - Apocalyptic Battle Arena

## Problem Statement

The current battle_tök scene (as shown in the screenshot) has:
- **Washed-out colors** with low contrast
- **Flat lighting** without dramatic atmosphere
- **Missing lava glow** - the lava plane exists but doesn't emit light
- **Basic sky** - needs dramatic purple/orange storm clouds with lightning
- **No particle effects** - missing meteors, embers, ash, smoke
- **No castle materials** - building blocks use simple colors, not realistic stone
- **Missing environmental details** - no torch lights, no fog depth, no chain bridge shaders

**Target**: Match the concept art showing an apocalyptic battlefield with:
- Two castles on opposing hex islands
- Glowing lava rivers between them
- Chain bridge connecting the islands
- Purple/orange stormy sky with lightning strikes
- Falling meteors with fire trails
- Battle smoke and floating embers
- Dramatic rim lighting from lava glow

## Solution Overview

Implement a complete visual overhaul using modular WGSL shaders integrated one-by-one into the engine. Each shader will be:
1. A standalone `.wgsl` file in `shaders/`
2. A corresponding Rust module in `engine/src/render/` or `src/game/render/`
3. Integrated into `battle_arena.rs` render loop

**Technical Approach:**
- Forward rendering with HDR pipeline
- ACES tonemapping for cinematic look
- Depth-based fog post-pass (applies to everything)
- Point lights for torches with flickering
- Billboard particles for embers/ash
- Vertex animation for flag cloth physics

## Data Structures / Layouts

### Shader Uniform Layouts

All shaders follow this convention for uniform buffer alignment:

```wgsl
// Standard vertex outputs (shared across materials)
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) view_dir: vec3<f32>,  // or uv for some shaders
}

// Standard scene uniforms (bind group 0)
struct SceneUniforms {
    view_proj: mat4x4<f32>,      // 64 bytes
    camera_pos: vec3<f32>,        // 12 bytes
    time: f32,                    // 4 bytes
    sun_dir: vec3<f32>,           // 12 bytes
    fog_density: f32,             // 4 bytes
    fog_color: vec3<f32>,         // 12 bytes
    ambient: f32,                 // 4 bytes
    // Total: 112 bytes
}

// Material-specific uniforms (bind group 1)
// Each material shader defines its own params
```

### Point Light Structure (for torches)

```rust
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PointLight {
    pub position: [f32; 3],
    pub radius: f32,
    pub color: [f32; 3],
    pub intensity: f32,
}

// Max 16 point lights for performance
const MAX_POINT_LIGHTS: usize = 16;
```

### Particle System Structure

```rust
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct Particle {
    pub position: [f32; 3],
    pub lifetime: f32,
    pub velocity: [f32; 3],
    pub size: f32,
    pub color: [f32; 4],
}

// GPU instanced rendering, max 1024 particles
const MAX_PARTICLES: usize = 1024;
```

## What Changes vs What Stays

### Changed Files

| File | What Changes |
|------|-------------|
| `shaders/lava.wgsl` | Already exists - enhance with light contribution output |
| `shaders/stormy_sky.wgsl` | Already exists - integrate purple/orange battle_arena preset |
| `shaders/terrain_enhanced.wgsl` | Already exists - use apocalyptic color palette |
| `shaders/castle_stone.wgsl` | **NEW** - Medieval stone with brick pattern + grime |
| `shaders/chain_bridge.wgsl` | **NEW** - Metal chain + wood planks |
| `shaders/flag.wgsl` | **NEW** - Team-colored cloth with wind animation |
| `shaders/torch_light.wgsl` | **NEW** - Point light with flicker |
| `shaders/fog_post.wgsl` | Enhance existing - depth-based volumetric fog |
| `shaders/tonemap_aces.wgsl` | Already exists - ensure it's used |
| `shaders/ember_particle.wgsl` | **NEW** - Billboard ember/ash particles |
| `engine/src/render/point_lights.rs` | **NEW** - Point light manager |
| `engine/src/render/particles.rs` | **NEW** - GPU particle system |
| `engine/src/render/material_system.rs` | **NEW** - Material pipeline coordinator |
| `src/game/destruction.rs` | Enhance meteor system visual |
| `src/bin/battle_arena.rs` | Integrate all new systems into render loop |

### Unchanged Files

| File | Why Unchanged |
|------|--------------|
| `engine/src/render/stormy_sky.rs` | Config presets already include battle_arena() |
| `engine/src/render/building_blocks.rs` | Building system stays same, just new material |
| `engine/src/physics/*` | Physics unchanged |
| `src/game/building/*` | Building logic unchanged |
| `src/game/economy/*` | Economy system unchanged |
| `src/game/population/*` | Population system unchanged |
| `src/game/ui/*` | UI unchanged |

## Stories

### Story 1: Integrate Apocalyptic Sky Preset
**What:** Update `battle_arena.rs` to use `StormySkyConfig::battle_arena()` preset and verify it renders correctly with purple zenith, orange horizon fog, and active lightning.

**Files:** `src/bin/battle_arena.rs`

**Changes:**
1. Change `StormySky::new()` to `StormySky::with_config(device, format, StormySkyConfig::battle_arena())`
2. Add random lightning trigger every 3-8 seconds
3. Verify shader compiles and renders

**Acceptance:**
- Sky shows dark purple at zenith
- Orange-red fog at horizon
- Lightning flashes occur periodically
- `cargo check` passes
- No runtime errors

---

### Story 2: Enhance Lava Shader with HDR Emission
**What:** Update `shaders/lava.wgsl` to output brighter HDR values (2.0-3.0) for the molten cracks so they bloom properly after tonemapping.

**Files:** `shaders/lava.wgsl`

**Changes:**
1. Increase `core_color` to `vec3<f32>(3.0, 0.8, 0.1)` (HDR bright orange)
2. Increase `emissive_strength` multiplier to 2.5
3. Add pulsing animation to crack brightness
4. Reduce fog influence on lava (emissive cuts through)

**Acceptance:**
- Lava cracks appear bright orange/yellow
- Cracks pulse/animate over time
- Fog doesn't wash out lava
- `cargo check` passes

---

### Story 3: Create Castle Stone Material Shader
**What:** Create `shaders/castle_stone.wgsl` with procedural brick pattern, mortar lines, grime darkening near bottom, and warm torch bounce light.

**Files:**
- `shaders/castle_stone.wgsl` (new)
- `engine/src/render/castle_material.rs` (new)
- `engine/src/render/mod.rs` (add export)

**Shader Features:**
```wgsl
// Procedural brick pattern using world position
let block_uv = world_pos.xz * 1.2 + world_pos.y * 0.35;
let mortar = smoothstep(0.48, 0.52, abs(fract(block_uv.x) - 0.5));

// Grime darkening near bottom
let grime = clamp(1.0 - world_pos.y * 0.12, 0.0, 1.0);

// Torch bounce (warm orange from below)
let torch_flicker = 0.85 + 0.15 * sin(time * 12.0);
let torch = torch_color * torch_strength * torch_flicker;
```

**Acceptance:**
- Stone shows visible brick/block pattern
- Bottom of walls darker (grime)
- Warm orange glow visible on stone surfaces
- Shader compiles without errors
- Rust module creates pipeline correctly

---

### Story 4: Create Point Light System for Torches
**What:** Create `engine/src/render/point_lights.rs` that manages up to 16 point lights with position, color, radius, and flickering intensity.

**Files:**
- `engine/src/render/point_lights.rs` (new)
- `engine/src/render/mod.rs` (add export)

**Rust Structure:**
```rust
pub struct PointLightManager {
    lights: Vec<PointLight>,
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl PointLightManager {
    pub fn add_torch(&mut self, pos: Vec3, color: Vec3, radius: f32);
    pub fn update(&mut self, queue: &Queue, time: f32); // Apply flicker
    pub fn bind_group(&self) -> &BindGroup;
}
```

**Acceptance:**
- Can add/remove torches dynamically
- Torches flicker with sin(time * frequency + random_offset)
- Buffer updates correctly each frame
- `cargo check` passes

---

### Story 5: Integrate Point Lights into Castle Stone Shader
**What:** Update `castle_stone.wgsl` to sample point lights from a storage buffer and add their contribution to the final color.

**Files:**
- `shaders/castle_stone.wgsl`
- `engine/src/render/castle_material.rs`

**Shader Changes:**
```wgsl
@group(1) @binding(1) var<storage, read> point_lights: array<PointLight>;
@group(1) @binding(2) var<uniform> light_count: u32;

// In fragment:
for (var i = 0u; i < light_count; i++) {
    let light = point_lights[i];
    let light_vec = light.position - world_pos;
    let dist = length(light_vec);
    let attenuation = 1.0 / (1.0 + dist * dist / (light.radius * light.radius));
    let ndl = max(dot(normal, normalize(light_vec)), 0.0);
    color += light.color * light.intensity * attenuation * ndl;
}
```

**Acceptance:**
- Castle stone lit by nearby torches
- Light attenuates with distance squared
- Multiple torches combine correctly
- `cargo check` passes

---

### Story 6: Create Chain Bridge Material Shaders
**What:** Create two shaders: `shaders/wood_plank.wgsl` for bridge planks and `shaders/chain_metal.wgsl` for chains with metallic rim highlight.

**Files:**
- `shaders/wood_plank.wgsl` (new)
- `shaders/chain_metal.wgsl` (new)
- `engine/src/render/bridge_materials.rs` (new)

**Wood Plank Shader:**
```wgsl
let base = vec3<f32>(0.38, 0.26, 0.16); // Brown wood
// Add noise variation for grain
// Lambert lighting
```

**Chain Metal Shader:**
```wgsl
let steel = vec3<f32>(0.55, 0.58, 0.62);
// Fresnel rim highlight
let rim = pow(1.0 - dot(normal, view_dir), 4.0);
color += vec3<f32>(1.0) * rim * shine;
```

**Acceptance:**
- Wood planks show brown with subtle grain
- Chains show metallic with bright edges
- Both shaders compile
- `cargo check` passes

---

### Story 7: Create Team Flag Shader with Wind Animation
**What:** Create `shaders/flag.wgsl` with vertex animation for cloth wave and team color stripe pattern.

**Files:**
- `shaders/flag.wgsl` (new)
- `engine/src/render/flag_material.rs` (new)

**Vertex Shader:**
```wgsl
// Wave displacement based on UV.x position
let wave = sin(uv.x * 10.0 + time * 3.5) * (1.0 - uv.y) * wind_strength;
let displaced_pos = pos + vec3<f32>(0.0, wave, 0.0);
```

**Fragment Shader:**
```wgsl
// Horizontal stripe for emblem band
let stripe = smoothstep(0.45, 0.48, uv.y) - smoothstep(0.52, 0.55, uv.y);
let color = mix(team_color, stripe_color, stripe * 0.85);
```

**Acceptance:**
- Flag mesh deforms with wind wave
- Team color displays correctly
- Stripe band visible
- `cargo check` passes

---

### Story 8: Create Ember/Ash Particle System
**What:** Create GPU-instanced billboard particle system for floating embers and ash rising from lava.

**Files:**
- `shaders/ember_particle.wgsl` (new)
- `engine/src/render/particles.rs` (new)

**Particle Shader:**
```wgsl
// Billboard vertex positioning
let right = normalize(cross(up, view_dir));
let billboard_up = cross(view_dir, right);
let world_pos = particle_center + right * local.x * size + billboard_up * local.y * size;

// Fragment: soft circle with additive blend
let d = length(uv - 0.5);
let alpha = smoothstep(0.5, 0.2, d) * lifetime_fade;
let color = vec3<f32>(2.0, 0.6, 0.1); // Emissive orange
```

**Rust System:**
```rust
pub struct ParticleSystem {
    particles: Vec<Particle>,
    instance_buffer: wgpu::Buffer,
    spawner: ParticleSpawner,
}

impl ParticleSystem {
    pub fn spawn_ember(&mut self, pos: Vec3);
    pub fn update(&mut self, dt: f32); // Move particles, kill dead ones
    pub fn render(&self, pass: &mut RenderPass);
}
```

**Acceptance:**
- Embers spawn near lava
- Particles float upward with slight randomness
- Particles fade out over lifetime (2-4 seconds)
- Additive blending looks correct
- `cargo check` passes

---

### Story 9: Enhance Depth-Based Fog Post-Pass
**What:** Update `shaders/fog_post.wgsl` to use depth buffer for distance-based fog, with height-based density variation (thicker near ground).

**Files:**
- `shaders/fog_post.wgsl`
- `engine/src/render/fog_post.rs` (new or enhance)

**Fog Formula:**
```wgsl
// Reconstruct world position from depth
let depth = textureSample(depth_tex, depth_samp, uv).r;
let world_pos = reconstruct_world_pos(uv, depth, inv_view_proj);

// Distance fog
let dist = length(world_pos - camera_pos);
let fog_factor = 1.0 - exp(-dist * fog_density);

// Height fog (thicker near ground)
let height_fog = exp(-world_pos.y * 0.05);
fog_factor = mix(fog_factor, 1.0, height_fog * 0.3);

// Purple-brown stormy fog color
let fog_color = vec3<f32>(0.4, 0.3, 0.5);
```

**Acceptance:**
- Distant objects fade into fog
- Ground level foggier than elevated areas
- Fog color matches stormy atmosphere
- Depth buffer reads correctly
- `cargo check` passes

---

### Story 10: Integrate ACES Tonemapping Post-Pass
**What:** Ensure `shaders/tonemap_aces.wgsl` is integrated into the render pipeline as the final pass, converting HDR to LDR with cinematic contrast.

**Files:**
- `shaders/tonemap_aces.wgsl` (verify/enhance)
- `src/bin/battle_arena.rs` (add post-process pass)

**Tonemap Function:**
```wgsl
fn aces_tonemap(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3(0.0), vec3(1.0));
}

// Apply exposure, tonemap, then gamma
let exposed = hdr_color * exposure;
let tonemapped = aces_tonemap(exposed);
let gamma_corrected = pow(tonemapped, vec3(1.0 / 2.2));
```

**Acceptance:**
- HDR scene renders to intermediate texture
- Tonemapping pass converts to swapchain
- Lava and embers bloom correctly (bright HDR → visible bright)
- No banding artifacts
- `cargo check` passes

---

### Story 11: Enhance Meteor System with Fire Trail
**What:** Update `src/game/destruction.rs` meteor rendering to include particle trail and brighter HDR emission.

**Files:**
- `src/game/destruction.rs`
- (may use particle system from Story 8)

**Changes:**
1. Meteor uses emissive material with HDR values (3.0+ intensity)
2. Meteor spawns ember particles along its trajectory
3. On impact, spawn burst of debris particles
4. Tumbling rotation for visual interest

**Acceptance:**
- Meteors appear as bright fireballs
- Trail of particles follows each meteor
- Impact creates particle burst
- `cargo check` passes

---

### Story 12: Update Terrain Colors to Apocalyptic Palette
**What:** Update `shaders/terrain_enhanced.wgsl` and the TerrainParams in Rust to use scorched/volcanic colors.

**Files:**
- `shaders/terrain_enhanced.wgsl`
- Rust code that sets TerrainParams

**New Color Palette:**
```rust
TerrainParams {
    grass: Vec3::new(0.15, 0.18, 0.10),    // Scorched olive
    dirt: Vec3::new(0.28, 0.20, 0.14),     // Ashen brown
    rock: Vec3::new(0.25, 0.22, 0.24),     // Dark volcanic
    snow: Vec3::new(0.50, 0.48, 0.45),     // Ash/dust (not white)
}
```

**Acceptance:**
- Terrain appears dark and scorched
- No bright green grass
- Rock dominates steep areas
- `cargo check` passes

---

### Story 13: Create Material System Coordinator
**What:** Create `engine/src/render/material_system.rs` that manages all material pipelines and provides a unified interface for switching materials per-object.

**Files:**
- `engine/src/render/material_system.rs` (new)
- `engine/src/render/mod.rs`

**Structure:**
```rust
pub enum MaterialType {
    Terrain,
    CastleStone,
    WoodPlank,
    ChainMetal,
    Flag,
    Lava,
}

pub struct MaterialSystem {
    pipelines: HashMap<MaterialType, wgpu::RenderPipeline>,
    bind_groups: HashMap<MaterialType, wgpu::BindGroup>,
    // Scene uniforms shared across all materials
    scene_bind_group: wgpu::BindGroup,
}

impl MaterialSystem {
    pub fn render_with_material(
        &self,
        pass: &mut RenderPass,
        material: MaterialType,
        mesh: &Mesh,
    );
}
```

**Acceptance:**
- Can create all material pipelines
- Scene uniforms shared correctly
- Material switching works
- `cargo check` passes

---

### Story 14: Integrate All Systems into battle_arena.rs
**What:** Wire up all new systems (sky, materials, lights, particles, fog, tonemap) into the main game loop with correct render order.

**Files:**
- `src/bin/battle_arena.rs`

**Render Order:**
1. Clear HDR render target
2. Render stormy sky (no depth)
3. Render terrain with apocalyptic colors
4. Render lava planes (emissive, no depth write)
5. Render building blocks with castle_stone material
6. Render bridge with wood/chain materials
7. Render flags with wind animation
8. Render meteors with fire trail
9. Render particles (additive blend)
10. Apply fog post-pass
11. Apply ACES tonemap
12. Render UI overlay

**Acceptance:**
- All systems render in correct order
- No z-fighting or artifacts
- Performance maintains 60fps
- Visual matches reference concept
- `cargo check` passes
- `cargo run --bin battle_arena` works

## Technical Considerations

### Existing Patterns to Follow
- Uniform buffer layouts match existing `stormy_sky.rs` pattern (scalar fields for alignment)
- Shader loading via `include_str!()` macro
- Pipeline creation follows `StormySky::new()` pattern
- Bind group layouts use explicit BindGroupLayoutDescriptor

### Performance Targets
- 60 FPS minimum at 1080p
- Max 6 ray march steps for sky
- Max 1024 particles
- Max 16 point lights
- Fog pass is single fullscreen quad

### Dependencies
- All shaders must compile with wgpu's naga validator
- Rust modules use `bytemuck` for Pod/Zeroable
- `glam` for math types

## Non-Goals (This Phase)

- **PBR materials** - Keep it simple with Lambert + rim lighting
- **Shadow mapping** - Too complex for this phase
- **Screen-space reflections** - Not needed
- **Deferred rendering** - Stay with forward rendering
- **Complex particle physics** - Simple upward drift only
- **Terrain deformation** - Terrain mesh stays static
- **Water shader** - Lava only for this phase
- **Unit rendering** - Units are out of scope (no models yet)
