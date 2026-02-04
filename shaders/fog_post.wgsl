// ============================================================================
// Fog Post-Process Shader (fog_post.wgsl)
// ============================================================================
// Fullscreen post-process fog that applies to everything
// Distance-based exponential fog + height-based fog (collects in low areas)
// World position reconstruction from depth buffer
// ============================================================================

struct FogParams {
    fog_color: vec3<f32>,       // Stormy purple (0.55, 0.45, 0.70)
    density: f32,               // 0.015..0.04 (distance fog)
    height_fog_start: f32,      // Y level where height fog starts
    height_fog_density: f32,    // 0.05..0.15 (height fog strength)
    // Camera matrices for world pos reconstruction
    inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    _pad0: f32,
}

@group(0) @binding(0)
var<uniform> fog: FogParams;

@group(0) @binding(1)
var scene_texture: texture_2d<f32>;

@group(0) @binding(2)
var scene_sampler: sampler;

@group(0) @binding(3)
var depth_texture: texture_depth_2d;

// ============================================================================
// VERTEX SHADER (fullscreen triangle)
// ============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0)
    );
    
    var out: VertexOutput;
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    out.uv.y = 1.0 - out.uv.y; // Flip Y for texture sampling
    return out;
}

// ============================================================================
// WORLD POSITION RECONSTRUCTION
// ============================================================================

fn reconstruct_world_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    // Convert UV to NDC
    let ndc = vec2<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0);
    
    // Create clip-space position
    let clip = vec4<f32>(ndc, depth, 1.0);
    
    // Transform to world space
    let world = fog.inv_view_proj * clip;
    
    return world.xyz / world.w;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Sample scene color and depth
    let scene_color = textureSample(scene_texture, scene_sampler, uv).rgb;
    let depth = textureSample(depth_texture, scene_sampler, uv);
    
    // Skip fog for sky (depth at far plane)
    if (depth >= 0.9999) {
        return vec4<f32>(scene_color, 1.0);
    }
    
    // Reconstruct world position
    let world_pos = reconstruct_world_position(uv, depth);
    
    // ========================================
    // DISTANCE FOG (exponential)
    // ========================================
    let distance = length(world_pos - fog.camera_pos);
    let distance_fog = 1.0 - exp(-distance * fog.density);
    
    // ========================================
    // HEIGHT FOG (collects in low areas)
    // ========================================
    // Fog is denser below height_fog_start
    let height_diff = fog.height_fog_start - world_pos.y;
    let height_factor = max(height_diff, 0.0);
    let height_fog = 1.0 - exp(-height_factor * fog.height_fog_density);
    
    // ========================================
    // COMBINE FOG
    // ========================================
    // Use max of distance and height fog, slightly weighted toward distance
    let fog_amount = clamp(distance_fog * 0.7 + height_fog * 0.5, 0.0, 0.95);
    
    // Apply fog
    let fogged = mix(scene_color, fog.fog_color, fog_amount);
    
    return vec4<f32>(fogged, 1.0);
}

// ============================================================================
// SIMPLIFIED VERSION (no depth texture, uses built-in fog)
// ============================================================================
// Use this if you want to apply fog inline in other shaders:
//
// fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>, camera_pos: vec3<f32>) -> vec3<f32> {
//     let fog_color = vec3<f32>(0.55, 0.45, 0.70);
//     let dist = length(world_pos - camera_pos);
//     let fog = 1.0 - exp(-dist * 0.02);
//     return mix(color, fog_color, clamp(fog, 0.0, 1.0));
// }
