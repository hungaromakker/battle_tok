// ============================================================================
// Dark and Stormy Skybox Shader (stormy_sky.wgsl)
// ============================================================================
// Optimized volumetric cloud shader with dramatic lighting
// Uses efficient noise and minimal texture lookups

// Uniforms - scalar fields to match Rust alignment
struct SkyUniforms {
    view_proj: mat4x4<f32>,       // 64 bytes (offset 0)
    inv_view_proj: mat4x4<f32>,   // 64 bytes (offset 64)
    camera_pos_x: f32,            // 4 bytes (offset 128)
    camera_pos_y: f32,            // 4 bytes (offset 132)
    camera_pos_z: f32,            // 4 bytes (offset 136)
    time: f32,                    // 4 bytes (offset 140)
    resolution_x: f32,            // 4 bytes (offset 144)
    resolution_y: f32,            // 4 bytes (offset 148)
    cloud_speed: f32,             // 4 bytes (offset 152)
    flow_speed: f32,              // 4 bytes (offset 156)
    flow_amount: f32,             // 4 bytes (offset 160)
    wave_amount: f32,             // 4 bytes (offset 164)
    wave_distort: f32,            // 4 bytes (offset 168)
    cloud_density: f32,           // 4 bytes (offset 172)
    cloud_scale: f32,             // 4 bytes (offset 176)
    cloud_bias: f32,              // 4 bytes (offset 180)
    bump_offset: f32,             // 4 bytes (offset 184)
    parallax_steps: f32,          // 4 bytes (offset 188)
    cloud_height: f32,            // 4 bytes (offset 192)
    world_scale: f32,             // 4 bytes (offset 196)
    light_spread_power1: f32,     // 4 bytes (offset 200)
    light_spread_factor1: f32,    // 4 bytes (offset 204)
    light_spread_power2: f32,     // 4 bytes (offset 208)
    light_spread_factor2: f32,    // 4 bytes (offset 212)
    sun_dir_x: f32,               // 4 bytes (offset 216)
    sun_dir_y: f32,               // 4 bytes (offset 220)
    sun_dir_z: f32,               // 4 bytes (offset 224)
    lightning_intensity: f32,     // 4 bytes (offset 228)
    cloud_color1_r: f32,          // 4 bytes (offset 232)
    cloud_color1_g: f32,          // 4 bytes (offset 236)
    cloud_color1_b: f32,          // 4 bytes (offset 240)
    cloud_color2_r: f32,          // 4 bytes (offset 244)
    cloud_color2_g: f32,          // 4 bytes (offset 248)
    cloud_color2_b: f32,          // 4 bytes (offset 252)
    upper_color_r: f32,           // 4 bytes (offset 256)
    upper_color_g: f32,           // 4 bytes (offset 260)
    upper_color_b: f32,           // 4 bytes (offset 264)
    fog_color_r: f32,             // 4 bytes (offset 268)
    fog_color_g: f32,             // 4 bytes (offset 272)
    fog_color_b: f32,             // 4 bytes (offset 276)
    fog_density: f32,             // 4 bytes (offset 280)
    _pad: f32,                    // 4 bytes (offset 284) - align to 288
}

@group(0) @binding(0)
var<uniform> uniforms: SkyUniforms;

// Helper functions
fn get_camera_pos() -> vec3<f32> {
    return vec3<f32>(uniforms.camera_pos_x, uniforms.camera_pos_y, uniforms.camera_pos_z);
}

fn get_sun_dir() -> vec3<f32> {
    return vec3<f32>(uniforms.sun_dir_x, uniforms.sun_dir_y, uniforms.sun_dir_z);
}

// Vertex output
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Fullscreen triangle vertex shader
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index) / 2) * 4.0 - 1.0;
    let y = f32(i32(vertex_index) % 2) * 4.0 - 1.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// ============================================================================
// OPTIMIZED NOISE FUNCTIONS
// ============================================================================

fn hash(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// Value noise - cheaper than gradient noise
fn noise(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    
    return mix(
        mix(mix(hash(i + vec3<f32>(0.0, 0.0, 0.0)), hash(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash(i + vec3<f32>(0.0, 1.0, 0.0)), hash(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y),
        mix(mix(hash(i + vec3<f32>(0.0, 0.0, 1.0)), hash(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash(i + vec3<f32>(0.0, 1.0, 1.0)), hash(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y),
        u.z
    );
}

// Optimized FBM - only 4 octaves
fn fbm(p: vec3<f32>) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;
    
    // 4 octaves is enough for clouds
    value += amplitude * noise(pos); pos *= 2.0; amplitude *= 0.5;
    value += amplitude * noise(pos); pos *= 2.0; amplitude *= 0.5;
    value += amplitude * noise(pos); pos *= 2.0; amplitude *= 0.5;
    value += amplitude * noise(pos);
    
    return value;
}

// ============================================================================
// CLOUD DENSITY FUNCTION
// ============================================================================

fn cloudDensity(p: vec3<f32>, time: f32) -> f32 {
    // Base cloud shape - large billowing forms
    let base_pos = p * 0.3 + vec3<f32>(time * 0.02, 0.0, time * 0.01);
    var density = fbm(base_pos);
    
    // Add detail at smaller scale
    let detail_pos = p * 0.8 + vec3<f32>(time * 0.05, time * 0.02, 0.0);
    density += fbm(detail_pos) * 0.4;
    
    // Height-based density falloff - clouds thicker in middle
    let height_factor = 1.0 - abs(p.y * 0.5);
    density *= max(height_factor, 0.0);
    
    // Shape clouds - threshold and contrast
    density = smoothstep(0.3, 0.7, density);
    
    return density;
}

// ============================================================================
// MAIN FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Get ray direction
    let camera_pos = get_camera_pos();
    let ndc = vec4<f32>(in.uv.x * 2.0 - 1.0, (1.0 - in.uv.y) * 2.0 - 1.0, 1.0, 1.0);
    let world_pos = uniforms.inv_view_proj * ndc;
    let ray_dir = normalize(world_pos.xyz / world_pos.w - camera_pos);
    
    let sun_dir = get_sun_dir();
    let time = uniforms.time * uniforms.cloud_speed;
    
    // ========================================================================
    // BACKGROUND GRADIENT - Dark stormy atmosphere
    // ========================================================================
    
    // Vertical gradient for base sky
    let up = ray_dir.y;
    
    // Deep storm colors
    let zenith_color = vec3<f32>(0.02, 0.02, 0.04);      // Almost black at top
    let mid_color = vec3<f32>(0.08, 0.08, 0.12);         // Dark blue-gray
    let horizon_color = vec3<f32>(0.15, 0.12, 0.1);      // Murky brown horizon
    let ground_color = vec3<f32>(0.03, 0.03, 0.03);      // Near black ground
    
    var sky_color: vec3<f32>;
    if (up > 0.3) {
        // Upper sky
        sky_color = mix(mid_color, zenith_color, smoothstep(0.3, 0.8, up));
    } else if (up > 0.0) {
        // Horizon band
        sky_color = mix(horizon_color, mid_color, smoothstep(0.0, 0.3, up));
    } else {
        // Below horizon
        sky_color = mix(horizon_color, ground_color, smoothstep(0.0, -0.3, up));
    }
    
    // ========================================================================
    // VOLUMETRIC CLOUDS - Ray march through cloud layer
    // ========================================================================
    
    // Cloud layer bounds
    let cloud_base = 50.0;
    let cloud_top = 150.0;
    
    // Calculate ray entry/exit for cloud layer (only if looking up)
    var cloud_color = vec3<f32>(0.0);
    var cloud_alpha = 0.0;
    
    if (ray_dir.y > 0.01) {
        // Find intersection with cloud layer
        let t_base = (cloud_base - camera_pos.y) / ray_dir.y;
        let t_top = (cloud_top - camera_pos.y) / ray_dir.y;
        
        let t_start = max(t_base, 0.0);
        let t_end = max(t_top, 0.0);
        
        if (t_end > t_start) {
            // Ray march through clouds - only 6 steps for performance
            let steps = 6;
            let step_size = (t_end - t_start) / f32(steps);
            
            var accumulated_color = vec3<f32>(0.0);
            var accumulated_alpha = 0.0;
            var t = t_start;
            
            for (var i = 0; i < steps; i = i + 1) {
                if (accumulated_alpha > 0.95) { break; }
                
                let pos = camera_pos + ray_dir * t;
                
                // Normalize position for noise lookup
                let cloud_pos = vec3<f32>(pos.x * 0.01, (pos.y - cloud_base) / (cloud_top - cloud_base), pos.z * 0.01);
                
                // Get cloud density
                let density = cloudDensity(cloud_pos, time);
                
                if (density > 0.01) {
                    // Cloud lighting
                    let light_dot = dot(ray_dir, sun_dir) * 0.5 + 0.5;
                    
                    // Dark cloud base color
                    let dark_cloud = vec3<f32>(0.05, 0.05, 0.07);
                    
                    // Bright edge where light breaks through
                    let lit_cloud = vec3<f32>(0.4, 0.35, 0.3);
                    
                    // Red/orange rim lighting
                    let rim_color = vec3<f32>(0.6, 0.25, 0.1);
                    
                    // Mix based on density and light direction
                    let light_penetration = pow(1.0 - density, 2.0) * light_dot;
                    var sample_color = mix(dark_cloud, lit_cloud, light_penetration);
                    
                    // Add rim lighting at cloud edges
                    let edge_factor = pow(1.0 - density, 3.0);
                    let rim_intensity = edge_factor * pow(light_dot, 2.0);
                    sample_color = sample_color + rim_color * rim_intensity * 2.0;
                    
                    // Height-based color variation
                    let height_factor = cloud_pos.y;
                    sample_color = sample_color * (0.7 + height_factor * 0.5);
                    
                    // Accumulate with front-to-back blending
                    let sample_alpha = density * 0.3;
                    accumulated_color = accumulated_color + sample_color * sample_alpha * (1.0 - accumulated_alpha);
                    accumulated_alpha = accumulated_alpha + sample_alpha * (1.0 - accumulated_alpha);
                }
                
                t = t + step_size;
            }
            
            cloud_color = accumulated_color;
            cloud_alpha = accumulated_alpha;
        }
    }
    
    // ========================================================================
    // LIGHT RAYS - God rays breaking through clouds
    // ========================================================================
    
    let sun_dot = max(dot(ray_dir, sun_dir), 0.0);
    
    // Soft sun glow
    let sun_glow = pow(sun_dot, 4.0) * 0.3;
    let glow_color = vec3<f32>(0.8, 0.6, 0.4);
    
    // Bright sun core (visible through cloud gaps)
    let sun_core = pow(sun_dot, 32.0) * (1.0 - cloud_alpha * 0.8);
    let core_color = vec3<f32>(1.0, 0.9, 0.7);
    
    // ========================================================================
    // LIGHTNING FLASH
    // ========================================================================
    
    var lightning = vec3<f32>(0.0);
    if (uniforms.lightning_intensity > 0.0) {
        let flash = uniforms.lightning_intensity * (1.0 - cloud_alpha * 0.5);
        lightning = vec3<f32>(0.8, 0.85, 1.0) * flash * 2.0;
    }
    
    // ========================================================================
    // FINAL COMPOSITION
    // ========================================================================
    
    // Start with sky gradient
    var final_color = sky_color;
    
    // Add sun glow to sky
    final_color = final_color + glow_color * sun_glow;
    
    // Blend in clouds
    final_color = mix(final_color, cloud_color, cloud_alpha);
    
    // Add sun core (visible through gaps)
    final_color = final_color + core_color * sun_core;
    
    // Add lightning
    final_color = final_color + lightning;
    
    // Horizon fog
    let fog_factor = pow(1.0 - abs(ray_dir.y), 8.0);
    let fog_color = vec3<f32>(0.12, 0.1, 0.08);
    final_color = mix(final_color, fog_color, fog_factor * 0.6);
    
    // ========================================================================
    // POST-PROCESSING
    // ========================================================================
    
    // Reinhard tonemapping
    final_color = final_color / (final_color + 1.0);
    
    // Slight desaturation for stormy mood
    let luma = dot(final_color, vec3<f32>(0.299, 0.587, 0.114));
    final_color = mix(vec3<f32>(luma), final_color, 0.8);
    
    // Contrast boost
    final_color = pow(final_color, vec3<f32>(1.1));
    
    // Gamma correction
    final_color = pow(final_color, vec3<f32>(1.0 / 2.2));
    
    return vec4<f32>(final_color, 1.0);
}
