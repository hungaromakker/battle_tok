// ============================================================================
// Procedural Storm Sky Shader (sky_storm_procedural.wgsl)
// ============================================================================
// Multi-color roguelike palette with swirling clouds and lightning bolts
// Vertical gradient: horizon (orange-red) -> mid (purple) -> top (dark)
// Domain-warped FBM for turbulent cloud motion
// ============================================================================

struct SkyParams {
    time: f32,
    cloud_speed: f32,           // 0.02..0.1
    cloud_scale: f32,           // 2.0..6.0
    cloud_density: f32,         // 0.3..0.8
    lightning_intensity: f32,   // 0.0..1.0
    lightning_frequency: f32,   // 5.0..10.0 (seconds between flashes)
    // Colors
    col_top: vec3<f32>,         // Dark sky (0.08, 0.06, 0.14)
    _pad0: f32,
    col_mid: vec3<f32>,         // Purple mid (0.22, 0.12, 0.30)
    _pad1: f32,
    col_horizon: vec3<f32>,     // Orange-red horizon (0.70, 0.22, 0.18)
    _pad2: f32,
    col_magic: vec3<f32>,       // Magic veins (0.25, 0.45, 0.95)
    _pad3: f32,
}

@group(0) @binding(0)
var<uniform> sky: SkyParams;

// ============================================================================
// NOISE FUNCTIONS
// ============================================================================

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn hash2(p: vec2<f32>) -> vec2<f32> {
    let h = vec2<f32>(
        dot(p, vec2<f32>(127.1, 311.7)),
        dot(p, vec2<f32>(269.5, 183.3))
    );
    return fract(sin(h) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    
    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise(pos);
        pos *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

// Domain warping for swirling effect
fn warp(p: vec2<f32>, t: f32) -> vec2<f32> {
    let warp1 = fbm(p + vec2<f32>(t * 0.1, 0.0), 2);
    let warp2 = fbm(p + vec2<f32>(0.0, t * 0.08), 2);
    return p + vec2<f32>(warp1, warp2) * 0.8;
}

// ============================================================================
// LIGHTNING FUNCTIONS
// ============================================================================

fn lightning_bolt(uv: vec2<f32>, t: f32, seed: f32) -> f32 {
    // Bolt parameters
    let bolt_x = hash(vec2<f32>(seed, floor(t))) * 0.6 + 0.2; // Random X position
    let bolt_active = step(0.92, hash(vec2<f32>(seed + 1.0, floor(t * 0.5)))); // Sparse activation
    
    // Main bolt spine
    var bolt = 0.0;
    var y = uv.y;
    var x = bolt_x;
    
    // Zigzag path
    for (var i = 0; i < 8; i++) {
        let segment_hash = hash(vec2<f32>(f32(i), seed + floor(t)));
        x += (segment_hash - 0.5) * 0.15;
        
        let dist = abs(uv.x - x);
        let segment = smoothstep(0.02, 0.0, dist) * smoothstep(f32(i) / 8.0 - 0.05, f32(i) / 8.0 + 0.1, uv.y);
        bolt += segment;
    }
    
    // Flash decay
    let flash_time = fract(t);
    let flash = exp(-flash_time * 8.0);
    
    return bolt * flash * bolt_active;
}

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
    out.position = vec4<f32>(positions[vertex_index], 0.9999, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    out.uv.y = 1.0 - out.uv.y; // Flip Y
    return out;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let t = sky.time;
    
    // ========================================
    // VERTICAL GRADIENT (horizon to top)
    // ========================================
    // Non-linear gradient for more dramatic horizon
    let gradient_y = pow(uv.y, 0.7);
    
    // Three-way blend: horizon -> mid -> top
    let mid_blend = smoothstep(0.0, 0.4, gradient_y);
    let top_blend = smoothstep(0.4, 0.9, gradient_y);
    
    var base_color = mix(sky.col_horizon, sky.col_mid, mid_blend);
    base_color = mix(base_color, sky.col_top, top_blend);
    
    // ========================================
    // SWIRLING CLOUDS
    // ========================================
    let cloud_uv = uv * sky.cloud_scale;
    let warped = warp(cloud_uv, t * sky.cloud_speed);
    
    // Multiple cloud layers
    let cloud1 = fbm(warped + vec2<f32>(t * sky.cloud_speed, 0.0), 4);
    let cloud2 = fbm(warped * 2.0 - vec2<f32>(t * sky.cloud_speed * 0.5, t * sky.cloud_speed * 0.3), 3);
    
    let clouds = cloud1 * 0.6 + cloud2 * 0.4;
    
    // Cloud color variation
    let cloud_bright = smoothstep(0.3, 0.7, clouds);
    let cloud_color = mix(base_color * 0.6, base_color * 1.3, cloud_bright);
    
    var color = mix(base_color, cloud_color, sky.cloud_density);
    
    // ========================================
    // MAGIC COLOR VEINS
    // ========================================
    // Thin bright bands of magic color
    let magic_noise = fbm(warped * 3.0 + vec2<f32>(t * 0.05, 0.0), 3);
    let magic_bands = smoothstep(0.65, 0.72, magic_noise);
    color = mix(color, sky.col_magic * 1.5, magic_bands * 0.4);
    
    // ========================================
    // LIGHTNING
    // ========================================
    if (sky.lightning_intensity > 0.01) {
        // Multiple bolt sources
        let bolt_time = t / sky.lightning_frequency;
        let bolt1 = lightning_bolt(uv, bolt_time, 0.0);
        let bolt2 = lightning_bolt(uv, bolt_time + 0.3, 100.0);
        
        let lightning = (bolt1 + bolt2) * sky.lightning_intensity;
        
        // Lightning illuminates clouds
        let flash = max(bolt1, bolt2) * sky.lightning_intensity;
        color += vec3<f32>(0.9, 0.95, 1.0) * lightning * 2.0;
        color += vec3<f32>(0.3, 0.3, 0.4) * flash * 0.5; // Ambient flash
    }
    
    // ========================================
    // SUBTLE ANIMATION
    // ========================================
    // Slow color pulse
    let pulse = sin(t * 0.3) * 0.05 + 1.0;
    color *= pulse;
    
    return vec4<f32>(color, 1.0);
}
