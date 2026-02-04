// ============================================================================
// Enhanced Terrain Shader (terrain_enhanced.wgsl)
// ============================================================================
// Height-based material bands + slope detection + procedural noise variation
// Creates "designed" terrain with grass, dirt, rock, and snow transitions
// ============================================================================

struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    sun_dir: vec3<f32>,
    fog_density: f32,
    fog_color: vec3<f32>,
    ambient: f32,
}

struct TerrainParams {
    // Material colors (can be tuned from Rust)
    grass: vec3<f32>,
    _pad0: f32,
    dirt: vec3<f32>,
    _pad1: f32,
    rock: vec3<f32>,
    _pad2: f32,
    snow: vec3<f32>,
    _pad3: f32,
    // Height band thresholds (world units)
    dirt_start: f32,
    dirt_end: f32,
    rock_start: f32,
    rock_end: f32,
    snow_start: f32,
    snow_end: f32,
    // Additional params
    noise_scale: f32,
    noise_strength: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<uniform> terrain: TerrainParams;

// ============================================================================
// NOISE FUNCTIONS
// ============================================================================

fn hash(p: vec2<f32>) -> f32 {
    let h = dot(p, vec2<f32>(127.1, 311.7));
    return fract(sin(h) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    let u = f * f * (3.0 - 2.0 * f);
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;
    
    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise(pos * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }
    return value;
}

// ============================================================================
// LIGHTING
// ============================================================================

fn lambert(n: vec3<f32>, l: vec3<f32>) -> f32 {
    return max(dot(normalize(n), normalize(l)), 0.0);
}

// ============================================================================
// VERTEX SHADER
// ============================================================================

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) world_normal: vec3<f32>,
    @location(2) vertex_color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.world_position = in.position;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.world_normal = in.normal;
    out.vertex_color = in.color;
    return out;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let world_pos = in.world_position;
    let world_nrm = normalize(in.world_normal);
    let view_pos = uniforms.camera_pos;
    let light_dir = normalize(uniforms.sun_dir);
    
    let up = vec3<f32>(0.0, 1.0, 0.0);
    
    // ========================================
    // SLOPE DETECTION
    // ========================================
    // How much the surface faces away from vertical (0 = flat, 1 = cliff)
    let slope = 1.0 - clamp(dot(world_nrm, up), 0.0, 1.0);
    
    // ========================================
    // HEIGHT-BASED MATERIAL BANDS
    // ========================================
    let h = world_pos.y;
    
    // Smooth transitions between materials
    let dirt_t = smoothstep(terrain.dirt_start, terrain.dirt_end, h);
    let rock_t = smoothstep(terrain.rock_start, terrain.rock_end, h);
    let snow_t = smoothstep(terrain.snow_start, terrain.snow_end, h);
    
    // Blend materials by height
    var col = mix(terrain.grass, terrain.dirt, dirt_t);
    col = mix(col, terrain.rock, rock_t);
    col = mix(col, terrain.snow, snow_t);
    
    // ========================================
    // SLOPE PUSHES TOWARD ROCK (CLIFFS)
    // ========================================
    // Steep areas become rocky regardless of height
    col = mix(col, terrain.rock, slope * 0.85);
    
    // ========================================
    // PROCEDURAL NOISE VARIATION
    // ========================================
    // Subtle color variation to break up uniformity
    let noise_val = fbm(world_pos.xz * terrain.noise_scale, 3) * terrain.noise_strength - (terrain.noise_strength * 0.5);
    col = col + vec3<f32>(noise_val, noise_val * 0.5, noise_val * 0.3);
    
    // ========================================
    // LIGHTING
    // ========================================
    let ndl = lambert(world_nrm, light_dir);
    let ambient = uniforms.ambient + 0.08; // Slightly boost ambient
    
    // Hemisphere ambient (sky contribution)
    let sky_factor = (world_nrm.y + 1.0) * 0.5;
    let hemisphere = mix(0.15, 0.35, sky_factor);
    
    var lit = col * (ambient + ndl * 0.9 + hemisphere * 0.3);
    
    // ========================================
    // RIM LIGHTING (subtle edge highlight)
    // ========================================
    let view_dir = normalize(view_pos - world_pos);
    let rim = pow(1.0 - max(dot(view_dir, world_nrm), 0.0), 3.0);
    lit += vec3<f32>(0.08, 0.12, 0.15) * rim * 0.4;
    
    // ========================================
    // DISTANCE FOG
    // ========================================
    let distance = length(view_pos - world_pos);
    let fog_amount = 1.0 - exp(-distance * uniforms.fog_density);
    let final_color = mix(lit, uniforms.fog_color, fog_amount);
    
    return vec4<f32>(final_color, 1.0);
}
