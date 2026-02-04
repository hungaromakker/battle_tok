// ============================================================================
// Hexagonal Terrain Mesh Shader (hex_terrain.wgsl)
// ============================================================================
//
// RENDERING PIPELINE OVERVIEW
// ---------------------------
// Battle Sphere uses a hybrid rendering approach:
//
//   1. Hex Terrain (THIS SHADER)
//      - Triangle mesh for hex planet tiles
//      - Geodesic icosphere subdivided into hex faces
//      - Per-vertex colors for biome variation
//      - Emissive effects: magma (flowing lava), crystals (pulsing)
//      - LOD support planned for distant tiles
//
//   2. Hex-Prism Voxels (hex_prism.wgsl)
//      - Building blocks for walls, fortifications
//      - Matches lighting with this shader
//
//   3. SDF Objects (raymarcher.wgsl)
//      - Mathematical objects (cannon, projectiles, effects)
//
// LIGHTING MODEL
// --------------
// All mesh shaders in Battle Sphere share this lighting model:
// - Lambert diffuse: max(dot(N, L), 0.0) * 0.7
// - Hemisphere ambient: sky contribution based on normal.y
// - Rim lighting: view-dependent edge highlight
// - Distance fog: exponential fog for atmosphere
//
// EMISSIVE MATERIALS
// ------------------
// Detected by vertex color thresholds:
// - Magma: R > 0.7, G < 0.5, B < 0.15 (orange/red hot)
// - Crystals: R > 0.9, B < 0.3
// Emissive materials:
// - Ignore normal lighting (self-illuminated)
// - Reduced fog (visible through atmosphere)
// - Extra rim glow
// - Animated pulsing/flowing effects
//
// SKY SHADER
// ----------
// Includes vs_sky/fs_sky for procedural starfield background.
// Full-screen triangle with hash-based stars.
//
// VERTEX FORMAT
// -------------
// Location 0: position (vec3) - world space
// Location 1: normal (vec3)   - world space
// Location 2: color (vec4)    - biome/material color
//
// PERFORMANCE
// -----------
// - Mesh-based: fixed vertex count per tile
// - No ray marching overhead
// - Batched draw calls planned
//
// ============================================================================

// Uniforms for camera and lighting
struct Uniforms {
    view_proj: mat4x4<f32>,      // Combined view-projection matrix
    camera_pos: vec3<f32>,       // Camera position for fog calculation
    time: f32,                   // Animation time
    sun_dir: vec3<f32>,          // Directional light direction (normalized)
    fog_density: f32,            // Fog density factor
    fog_color: vec3<f32>,        // Fog/sky color
    ambient: f32,                // Ambient light strength
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

// Vertex input from mesh
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

// Vertex output to fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    out.world_position = in.position;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.normal = in.normal;
    out.color = in.color;
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.normal);
    let sun_dir = normalize(uniforms.sun_dir);
    
    // Detect emissive materials:
    // - Magma: R > 0.7, G < 0.5, B < 0.1 (orange/red hot)
    // - Crystals: R > 0.9, B < 0.3
    let is_magma = in.color.r > 0.7 && in.color.g < 0.5 && in.color.b < 0.15;
    let is_crystal = in.color.r > 0.9 && in.color.b < 0.3 && !is_magma;
    let is_emissive = is_magma || is_crystal;
    
    // Basic Lambert diffuse lighting
    let ndotl = max(dot(normal, sun_dir), 0.0);
    let diffuse = ndotl * 0.7;
    
    // Ambient lighting
    let ambient = uniforms.ambient;
    
    // Simple hemisphere ambient (sky contribution from above)
    let sky_factor = (normal.y + 1.0) * 0.5;
    let hemisphere_ambient = mix(0.2, 0.4, sky_factor);
    
    // Combined lighting
    let lighting = ambient + diffuse + hemisphere_ambient * 0.3;
    
    // Apply lighting to vertex color
    var lit_color = in.color.rgb * lighting;
    
    // Magma glow effect - flowing, pulsing lava
    if (is_magma) {
        // Slow pulsing glow
        let pulse = 0.85 + sin(uniforms.time * 1.5) * 0.15;
        // Flowing effect based on position
        let flow = sin(in.world_position.x * 0.01 + uniforms.time * 0.5) * 
                   cos(in.world_position.z * 0.01 + uniforms.time * 0.3);
        let intensity = pulse + flow * 0.1;
        
        lit_color = in.color.rgb * intensity * 1.8;
        lit_color += vec3<f32>(0.3, 0.05, 0.0); // Extra orange glow
    }
    // Crystal glow (existing effect)
    else if (is_crystal) {
        let pulse = 0.8 + sin(uniforms.time * 3.0) * 0.2;
        lit_color = in.color.rgb * pulse * 1.5;
        lit_color += in.color.rgb * 0.3;
    }
    
    // Add slight rim lighting for depth (non-emissive only)
    let view_dir = normalize(uniforms.camera_pos - in.world_position);
    let rim = pow(1.0 - max(dot(view_dir, normal), 0.0), 3.0);
    if (!is_emissive) {
        lit_color += vec3<f32>(0.1, 0.15, 0.2) * rim * 0.5;
    } else {
        // Emissive rim glow - extra bright for magma
        let rim_intensity = select(0.8, 1.2, is_magma);
        lit_color += in.color.rgb * rim * rim_intensity;
    }
    
    // Distance fog (reduced for emissive)
    let distance = length(uniforms.camera_pos - in.world_position);
    var fog_amount = 1.0 - exp(-distance * uniforms.fog_density);
    if (is_emissive) {
        fog_amount *= 0.2; // Emissive cuts through fog even more
    }
    let final_color = mix(lit_color, uniforms.fog_color, fog_amount);
    
    return vec4<f32>(final_color, in.color.a);
}

// Sky background shader (separate pass or can be combined)
struct SkyVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_sky(@builtin(vertex_index) vertex_index: u32) -> SkyVertexOutput {
    // Full-screen triangle
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    
    var out: SkyVertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.9999, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    return out;
}

@fragment
fn fs_sky(in: SkyVertexOutput) -> @location(0) vec4<f32> {
    // Space background with stars
    let uv = in.uv;
    
    // Dark space color
    var space_color = vec3<f32>(0.01, 0.01, 0.03);
    
    // Add procedural stars using hash function
    let star_uv = uv * 200.0; // Scale for star density
    let cell = floor(star_uv);
    let local = fract(star_uv) - 0.5;
    
    // Simple hash for star positions
    let hash = fract(sin(dot(cell, vec2<f32>(127.1, 311.7))) * 43758.5453);
    
    // Star probability and brightness
    if (hash > 0.97) {
        let star_dist = length(local);
        let star_brightness = smoothstep(0.3, 0.0, star_dist) * (hash - 0.97) * 30.0;
        space_color += vec3<f32>(star_brightness);
    }
    
    return vec4<f32>(space_color, 1.0);
}
