// ============================================================================
// Hex Prism Shader (hex_prism.wgsl)
// ============================================================================
//
// RENDERING PIPELINE OVERVIEW
// ---------------------------
// Battle Sphere uses a hybrid rendering approach:
//
//   1. Hex Terrain (hex_terrain.wgsl)
//      - Triangle mesh for hex planet tiles
//      - LOD support planned
//      - Emissive effects for magma/crystals
//
//   2. Hex-Prism Voxels (THIS SHADER)
//      - Building blocks for walls, fortifications
//      - Each prism: 6-sided hexagonal base, extruded vertically
//      - GPU mesh generated from HexPrismGrid data structure
//      - Supports instancing via model matrix
//
//   3. SDF Objects (raymarcher.wgsl)
//      - Mathematical objects (cannon, projectiles, effects)
//      - Ray marched in fragment shader
//      - Infinite detail, smooth curves
//
// WHY HEX-PRISMS (NOT CUBES)
// --------------------------
// - Match hex planet grid naturally
// - 6-way symmetry looks organic, not blocky
// - No visible cube artifacts
// - Micro-voxels (0.1-0.5m) appear smooth
//
// LIGHTING MODEL
// --------------
// Must match hex_terrain.wgsl for visual consistency:
// - Lambert diffuse: max(dot(N, L), 0.0) * 0.7
// - Hemisphere ambient: sky contribution from above
// - Rim lighting: depth/edge highlight
// - Distance fog: atmospheric depth
//
// VERTEX FORMAT
// -------------
// Location 0: position (vec3) - local space
// Location 1: normal (vec3)   - local space
// Location 2: color (vec4)    - material color (see presets below)
//
// USAGE
// -----
// 1. Generate mesh from HexPrismGrid::generate_combined_mesh()
// 2. Create vertex/index buffers
// 3. Set uniforms (view_proj, camera_pos, sun_dir, fog)
// 4. Set model_uniforms (model matrix, normal matrix)
// 5. Draw indexed
//
// ============================================================================

// Uniforms for camera and lighting
// Matches hex_terrain.wgsl for consistent lighting across shaders
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

// Optional: Model matrix for instanced rendering or per-object transforms
// When not using instancing, this can be set to identity
struct ModelUniforms {
    model: mat4x4<f32>,          // Model transform matrix
    normal_matrix: mat3x3<f32>,  // Normal transform (inverse transpose of model 3x3)
}

@group(0) @binding(1)
var<uniform> model_uniforms: ModelUniforms;

// Vertex input from hex-prism mesh
struct VertexInput {
    @location(0) position: vec3<f32>,  // Local position
    @location(1) normal: vec3<f32>,    // Local normal
    @location(2) color: vec4<f32>,     // Vertex color (for material variation)
}

// Vertex output to fragment shader
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

// Vertex shader: MVP transform, pass normal and color to fragment
@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transform position: local -> world -> clip space
    let world_pos = model_uniforms.model * vec4<f32>(in.position, 1.0);
    out.world_position = world_pos.xyz;
    out.clip_position = uniforms.view_proj * world_pos;

    // Transform normal to world space using normal matrix
    out.world_normal = model_uniforms.normal_matrix * in.normal;

    // Pass through vertex color for material variation
    out.color = in.color;

    return out;
}

// Fragment shader: Lambert diffuse + ambient lighting
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Normalize interpolated normal
    let normal = normalize(in.world_normal);
    let sun_dir = normalize(uniforms.sun_dir);

    // Lambert diffuse lighting
    // Same formula as hex_terrain.wgsl: max(dot(normal, sun_dir), 0.0)
    let ndotl = max(dot(normal, sun_dir), 0.0);
    let diffuse = ndotl * 0.7;  // 70% diffuse contribution

    // Ambient lighting from uniforms (base illumination in shadows)
    let ambient = uniforms.ambient;

    // Simple hemisphere ambient (sky contribution from above)
    // Upward-facing surfaces get more ambient light
    let sky_factor = (normal.y + 1.0) * 0.5;
    let hemisphere_ambient = mix(0.2, 0.4, sky_factor);

    // Combined lighting
    let lighting = ambient + diffuse + hemisphere_ambient * 0.3;

    // Apply lighting to vertex color (stone gray, wood brown, etc.)
    var lit_color = in.color.rgb * lighting;

    // Subtle rim lighting for depth (helps prisms pop against each other)
    let view_dir = normalize(uniforms.camera_pos - in.world_position);
    let rim = pow(1.0 - max(dot(view_dir, normal), 0.0), 3.0);
    lit_color += vec3<f32>(0.08, 0.1, 0.12) * rim * 0.4;

    // Distance fog for atmospheric depth
    let distance = length(uniforms.camera_pos - in.world_position);
    let fog_amount = 1.0 - exp(-distance * uniforms.fog_density);
    let final_color = mix(lit_color, uniforms.fog_color, fog_amount);

    return vec4<f32>(final_color, in.color.a);
}

// Alternative entry point for simple rendering without model matrix
// Uses identity transforms (world space == local space)
@vertex
fn vs_main_simple(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Position is already in world space
    out.world_position = in.position;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);

    // Normal is already in world space
    out.world_normal = in.normal;

    // Pass through vertex color
    out.color = in.color;

    return out;
}

// ============================================================================
// GHOST PREVIEW SHADER (Builder Mode)
// ============================================================================
// Renders a semi-transparent pulsing preview of where the block will be placed.
// Uses green tint with animated alpha for visibility.

@fragment
fn fs_ghost(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    let view_dir = normalize(uniforms.camera_pos - in.world_position);
    
    // Base ghost color - bright green for visibility
    let base_color = vec3<f32>(0.3, 0.9, 0.4);
    
    // Pulsing alpha animation (0.4 to 0.7)
    let pulse = 0.55 + sin(uniforms.time * 4.0) * 0.15;
    
    // Rim glow for edge visibility
    let rim = pow(1.0 - max(dot(view_dir, normal), 0.0), 2.0);
    let rim_color = vec3<f32>(0.5, 1.0, 0.6) * rim * 0.8;
    
    // Simple lighting
    let sun_dir = normalize(uniforms.sun_dir);
    let ndotl = max(dot(normal, sun_dir), 0.0) * 0.5 + 0.5;
    
    let final_color = base_color * ndotl + rim_color;
    
    return vec4<f32>(final_color, pulse);
}

// ============================================================================
// GRID OVERLAY SHADER (Builder Mode)
// ============================================================================
// Renders thin lines for the hex grid overlay

@fragment
fn fs_grid(in: VertexOutput) -> @location(0) vec4<f32> {
    // Cyan grid lines, semi-transparent
    let line_color = vec3<f32>(0.3, 0.8, 1.0);
    let alpha = 0.5;
    
    return vec4<f32>(line_color, alpha);
}

// Material preset colors for convenience
// Stone gray - weathered fortress walls
const STONE_GRAY: vec4<f32> = vec4<f32>(0.45, 0.42, 0.40, 1.0);
// Stone gray (light) - newer/cleaner stone
const STONE_LIGHT: vec4<f32> = vec4<f32>(0.55, 0.52, 0.50, 1.0);
// Stone gray (dark) - aged/mossy stone
const STONE_DARK: vec4<f32> = vec4<f32>(0.32, 0.30, 0.28, 1.0);
// Wood brown - wooden palisades
const WOOD_BROWN: vec4<f32> = vec4<f32>(0.45, 0.32, 0.20, 1.0);
// Wood brown (light) - fresh lumber
const WOOD_LIGHT: vec4<f32> = vec4<f32>(0.55, 0.40, 0.25, 1.0);
// Wood brown (dark) - weathered wood
const WOOD_DARK: vec4<f32> = vec4<f32>(0.30, 0.22, 0.15, 1.0);
// Metal (iron) - reinforced sections
const METAL_IRON: vec4<f32> = vec4<f32>(0.35, 0.35, 0.38, 1.0);
// Metal (bronze) - decorative elements
const METAL_BRONZE: vec4<f32> = vec4<f32>(0.55, 0.42, 0.25, 1.0);
