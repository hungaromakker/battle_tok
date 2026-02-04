// ============================================================================
// OPTIMIZED MESH RENDERING (Vercidium Techniques)
// ============================================================================
// Performance optimizations applied:
// 1. Greedy meshing - merge adjacent quads into larger rectangles
// 2. Hidden face culling - skip faces between terrain cells
// 3. Back-face culling - GPU culls faces facing away from camera
// 4. Distance fog - hide popping at chunk boundaries
// 5. LOD-ready structure for future chunk streaming
// ============================================================================

struct MeshUniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    chunk_offset: vec3<f32>,
    chunk_scale: f32,
    water_level: f32,
    fog_start: f32,
    fog_end: f32,
    _pad2: f32,
}

// Standard vertex input (for non-packed mode)
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
}

// Packed vertex input (Vercidium optimization - 32 bits per vertex)
// Bits 0-9:   X position (0-1023, scaled by chunk_scale)
// Bits 10-19: Y position (0-1023, for height)
// Bits 20-29: Z position (0-1023, scaled by chunk_scale)
// Bits 30-31: Normal index (0-3 for top/side faces)
struct PackedVertexInput {
    @location(0) packed_pos_normal: u32,
    @location(1) packed_color_material: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_pos: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) depth: f32,
}

@group(0) @binding(0) var<uniform> uniforms: MeshUniforms;

// Normals for terrain (simplified - mostly up-facing with some slopes)
const TERRAIN_NORMALS: array<vec3<f32>, 4> = array<vec3<f32>, 4>(
    vec3<f32>(0.0, 1.0, 0.0),    // 0: Up (flat terrain)
    vec3<f32>(0.707, 0.707, 0.0), // 1: Slope +X
    vec3<f32>(0.0, 0.707, 0.707), // 2: Slope +Z
    vec3<f32>(-0.707, 0.707, 0.0), // 3: Slope -X
);

// Tropical material colors (vibrant)
const MATERIALS: array<vec3<f32>, 6> = array<vec3<f32>, 6>(
    vec3<f32>(0.18, 0.65, 0.12),  // 0: Grass (lush tropical green)
    vec3<f32>(0.95, 0.88, 0.65),  // 1: Sand (warm beach)
    vec3<f32>(0.30, 0.28, 0.25),  // 2: Rock (volcanic)
    vec3<f32>(0.98, 0.98, 1.0),   // 3: Snow
    vec3<f32>(0.0, 0.75, 0.85),   // 4: Water (turquoise)
    vec3<f32>(0.12, 0.45, 0.08),  // 5: Dark grass (jungle)
);

// Noise functions for water animation
fn hash21(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    let a = hash21(i);
    let b = hash21(i + vec2<f32>(1.0, 0.0));
    let c = hash21(i + vec2<f32>(0.0, 1.0));
    let d = hash21(i + vec2<f32>(1.0, 1.0));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    
    // Transform position with chunk offset
    var world_pos = in.position + uniforms.chunk_offset;
    let normal = in.normal;
    var color = in.color;
    
    // Detect water by blue channel
    let is_water = color.b > 0.5 && color.r < 0.2;
    if is_water {
        // Animated water waves
        let wave1 = sin(world_pos.x * 0.3 + uniforms.time * 1.5) * 0.15;
        let wave2 = sin(world_pos.z * 0.4 + uniforms.time * 1.2) * 0.12;
        let wave3 = sin((world_pos.x + world_pos.z) * 0.2 + uniforms.time * 0.8) * 0.1;
        world_pos.y += wave1 + wave2 + wave3;
    }
    
    out.clip_position = uniforms.view_proj * vec4<f32>(world_pos, 1.0);
    out.world_pos = world_pos;
    out.world_normal = normal;
    out.color = color;
    out.depth = length(world_pos - uniforms.camera_pos);
    
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    var color = in.color;
    
    // Check if this is water
    let is_water = color.b > 0.5 && color.r < 0.2;
    
    // Sun direction (tropical, high sun)
    let sun_dir = normalize(vec3<f32>(0.4, 0.85, 0.35));
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    
    if is_water {
        // === WATER SHADING ===
        let p = in.world_pos.xz;
        let time = uniforms.time;
        
        // Animated caustics
        let caustic1 = noise2d(p * 0.25 + vec2<f32>(time * 0.3, 0.0));
        let caustic2 = noise2d(p * 0.4 + vec2<f32>(0.0, time * 0.25));
        let caustics = (caustic1 + caustic2) * 0.5;
        
        // Depth-based color (turquoise shallow, deep blue deep)
        let shallow = vec3<f32>(0.0, 0.85, 0.85);
        let deep = vec3<f32>(0.0, 0.35, 0.55);
        let depth_factor = clamp((uniforms.water_level - in.world_pos.y + 5.0) / 15.0, 0.0, 1.0);
        color = mix(shallow, deep, depth_factor);
        
        // Add caustics
        color += caustics * 0.12;
        
        // Fresnel reflection
        let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 4.0);
        let sky_reflect = vec3<f32>(0.55, 0.75, 0.95);
        color = mix(color, sky_reflect, fresnel * 0.5);
        
        // Specular highlight (sun glitter)
        let half_vec = normalize(sun_dir + view_dir);
        let spec = pow(max(dot(normal, half_vec), 0.0), 128.0);
        color += vec3<f32>(1.0, 0.98, 0.9) * spec * 0.8;
        
        // Soft diffuse
        let ndotl = max(dot(normal, sun_dir), 0.0);
        color *= (0.5 + ndotl * 0.5);
        
    } else {
        // === TERRAIN SHADING ===
        // Diffuse lighting
        let ndotl = max(dot(normal, sun_dir), 0.0);
        let ambient = 0.35;
        let diffuse = ndotl * 0.65;
        
        // Hemisphere ambient (sky blue from above, ground brown from below)
        let sky_ambient = vec3<f32>(0.6, 0.75, 0.9) * 0.15;
        let ground_ambient = vec3<f32>(0.4, 0.35, 0.3) * 0.1;
        let hemisphere = mix(ground_ambient, sky_ambient, normal.y * 0.5 + 0.5);
        
        color = color * (ambient + diffuse) + hemisphere;
        
        // Subtle rim lighting for depth
        let rim = pow(1.0 - max(dot(normal, view_dir), 0.0), 3.0);
        color += vec3<f32>(0.3, 0.4, 0.5) * rim * 0.1;
    }
    
    // Distance fog (atmospheric perspective)
    let fog_start = uniforms.fog_start;
    let fog_end = uniforms.fog_end;
    let fog_factor = clamp((in.depth - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    let fog_color = vec3<f32>(0.55, 0.72, 0.92);  // Tropical sky blue
    color = mix(color, fog_color, fog_factor * fog_factor);  // Quadratic falloff
    
    // Slight desaturation in fog
    let gray = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    color = mix(color, vec3<f32>(gray), fog_factor * 0.3);
    
    return vec4<f32>(color, 1.0);
}

// Water-specific fragment shader
@fragment
fn fs_water(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal = normalize(in.world_normal);
    
    // Animated water color
    let time = uniforms.time;
    let p = in.world_pos.xz;
    
    // Caustics pattern
    let caustic1 = noise2d(p * 0.3 + vec2<f32>(time * 0.5, 0.0));
    let caustic2 = noise2d(p * 0.5 + vec2<f32>(0.0, time * 0.3));
    let caustics = (caustic1 + caustic2) * 0.5;
    
    // Base water colors (tropical)
    let shallow = vec3<f32>(0.0, 0.85, 0.8);  // Turquoise
    let deep = vec3<f32>(0.0, 0.3, 0.5);       // Deep blue
    
    // Depth-based color (using world Y as proxy for depth)
    let depth_factor = clamp((uniforms.water_level - in.world_pos.y) / 10.0, 0.0, 1.0);
    var water_color = mix(shallow, deep, depth_factor);
    
    // Add caustics
    water_color += caustics * 0.15;
    
    // Fresnel reflection
    let view_dir = normalize(uniforms.camera_pos - in.world_pos);
    let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 3.0);
    let sky_color = vec3<f32>(0.5, 0.7, 0.9);
    water_color = mix(water_color, sky_color, fresnel * 0.4);
    
    // Lighting
    let light_dir = normalize(vec3<f32>(0.5, 0.8, 0.3));
    let ndotl = max(dot(normal, light_dir), 0.0);
    water_color *= (0.4 + ndotl * 0.6);
    
    // Specular highlight
    let half_vec = normalize(light_dir + view_dir);
    let spec = pow(max(dot(normal, half_vec), 0.0), 64.0);
    water_color += vec3<f32>(1.0, 1.0, 0.9) * spec * 0.5;
    
    // Distance fog
    let fog_start = 50.0;
    let fog_end = 200.0;
    let fog_factor = clamp((in.depth - fog_start) / (fog_end - fog_start), 0.0, 1.0);
    let fog_color = vec3<f32>(0.5, 0.7, 0.9);
    water_color = mix(water_color, fog_color, fog_factor);
    
    return vec4<f32>(water_color, 0.85);  // Slight transparency
}
