// ============================================================================
// Lava Shader (lava.wgsl)
// ============================================================================
// Animated flowing lava with emissive cracks, crust, and Fresnel edge glow
// Domain-warped FBM for sheet flow motion
// HDR emissive output for bloom compatibility
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

struct LavaParams {
    time: f32,
    emissive_strength: f32,  // 0.8..2.5 (HDR)
    scale: f32,              // 0.15..0.6 (noise scale)
    speed: f32,              // 0.1..0.6 (flow speed)
    crack_sharpness: f32,    // 0.78..0.95 (how defined cracks are)
    normal_strength: f32,    // 0.3..1.0 (procedural bump)
    // Colors
    core_color: vec3<f32>,   // Bright molten (2.4, 0.65, 0.08) HDR
    _pad0: f32,
    crust_color: vec3<f32>,  // Dark cooled crust (0.05, 0.01, 0.01)
    _pad1: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<uniform> lava: LavaParams;

// ============================================================================
// NOISE FUNCTIONS
// ============================================================================

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
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

// Domain warping for more organic flow
fn warp_domain(p: vec2<f32>, t: f32) -> vec2<f32> {
    let offset = vec2<f32>(
        fbm(p + vec2<f32>(t * 0.3, 0.0), 2),
        fbm(p + vec2<f32>(0.0, t * 0.2), 2)
    );
    return p + offset * 0.5;
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
    let normal = normalize(in.world_normal);
    let view_dir = normalize(uniforms.camera_pos - world_pos);

    // ========================================
    // ANIMATED LAVA FLOW - Enhanced for dramatic look
    // ========================================
    let uv = world_pos.xz * lava.scale;
    let t = lava.time * lava.speed;

    // Domain-warped flow for organic motion - more turbulent
    let flow1 = vec2<f32>(t * 0.5, t * 0.35);
    let flow2 = vec2<f32>(-t * 0.4, t * 0.2);
    let flow3 = vec2<f32>(t * 0.15, -t * 0.3);

    // Warp the domain for more interesting patterns
    let warped_uv = warp_domain(uv, t);

    // Multiple noise octaves at different scales - more detail
    let n1 = fbm(warped_uv + flow1, 5);
    let n2 = fbm(warped_uv * 2.5 - flow2, 4);
    let n3 = fbm(warped_uv * 0.4 + flow3, 3);
    let n4 = fbm(warped_uv * 4.0 + flow1 * 0.3, 2);  // Fine detail

    // Combine noise layers - create river-like channels
    var combined = n1 * 0.45 + n2 * 0.3 + n3 * 0.15 + n4 * 0.1;

    // Create lava river channels by enhancing bright areas
    let river_pattern = smoothstep(0.35, 0.65, combined);
    combined = combined * 0.5 + river_pattern * 0.5;

    // ========================================
    // CRACK PATTERN (thin bright bands) - Enhanced
    // ========================================
    // Sharp transition creates crack-like bright lines
    let crack = smoothstep(lava.crack_sharpness, 0.96, combined);

    // Secondary finer cracks
    let fine_cracks = smoothstep(0.7, 0.85, n4) * 0.4;

    // Heat value combines general flow with crack brightness
    let heat = clamp(combined * 0.5 + crack * 0.7 + fine_cracks, 0.0, 1.0);

    // ========================================
    // COLOR MIXING (HDR values for proper bloom) - MUCH brighter
    // ========================================
    // Very bright HDR core for intense glow matching reference
    let hdr_core = vec3<f32>(4.5, 1.2, 0.15);     // Intense HDR orange-yellow
    let hdr_mid = vec3<f32>(2.5, 0.5, 0.08);      // Mid-bright orange
    let dark_crust = vec3<f32>(0.08, 0.02, 0.01); // Near-black crust

    // Three-way mix for more natural lava look
    var color: vec3<f32>;
    if (heat > 0.6) {
        // Hot zones - brightest
        color = mix(hdr_mid, hdr_core, (heat - 0.6) * 2.5);
    } else {
        // Cooler zones - dark crust to mid-bright
        color = mix(dark_crust, hdr_mid, heat * 1.67);
    }

    // ========================================
    // FRESNEL EDGE GLOW - Enhanced
    // ========================================
    // Edges glow brighter (molten material visible at grazing angles)
    let fresnel = pow(1.0 - max(dot(normal, view_dir), 0.0), 2.5);
    let edge_glow = fresnel * 0.7;

    // ========================================
    // EMISSIVE OUTPUT (HDR with pulse animation) - More intense
    // ========================================
    // Subtle pulsing animation makes lava feel alive
    let pulse = 0.95 + 0.05 * sin(lava.time * 2.5);
    let slow_pulse = 0.9 + 0.1 * sin(lava.time * 0.8 + world_pos.x * 0.1);
    let emissive = lava.emissive_strength * 3.0 * (heat + edge_glow) * pulse * slow_pulse;
    color = color * emissive;

    // Add bright yellow-orange rim at edges
    color += vec3<f32>(1.5, 0.5, 0.08) * fresnel * heat * lava.emissive_strength;

    // Add hot white core in brightest areas
    if (heat > 0.8) {
        let white_amount = (heat - 0.8) * 5.0;
        color += vec3<f32>(2.0, 1.5, 0.8) * white_amount * pulse;
    }

    // ========================================
    // REDUCED FOG (emissive cuts through)
    // ========================================
    let distance = length(uniforms.camera_pos - world_pos);
    let fog_amount = (1.0 - exp(-distance * uniforms.fog_density)) * 0.05;  // Less fog on lava
    let final_color = mix(color, uniforms.fog_color * 0.3, fog_amount);

    return vec4<f32>(final_color, 1.0);
}
