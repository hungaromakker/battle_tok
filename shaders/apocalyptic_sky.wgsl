// ============================================================================
// Apocalyptic Sky Shader (apocalyptic_sky.wgsl)
// ============================================================================
// High-fidelity volumetric skybox with:
// - Dramatic stormy clouds with purple/orange lighting
// - Nebula effects (space dust, stars)
// - Molten planet visible in sky
// - Floating asteroid debris
// - Lightning strikes
// - HDR output for bloom compatibility
// ============================================================================

struct SkyUniforms {
    view_proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    camera_pos_x: f32,
    camera_pos_y: f32,
    camera_pos_z: f32,
    time: f32,
    resolution_x: f32,
    resolution_y: f32,
    // Cloud parameters
    cloud_speed: f32,
    cloud_density: f32,
    cloud_scale: f32,
    cloud_coverage: f32,
    // Sky colors
    zenith_r: f32,
    zenith_g: f32,
    zenith_b: f32,
    horizon_r: f32,
    horizon_g: f32,
    horizon_b: f32,
    // Lava glow from below
    lava_glow_r: f32,
    lava_glow_g: f32,
    lava_glow_b: f32,
    lava_glow_strength: f32,
    // Sun/moon
    sun_dir_x: f32,
    sun_dir_y: f32,
    sun_dir_z: f32,
    sun_intensity: f32,
    // Lightning
    lightning_intensity: f32,
    lightning_pos_x: f32,
    lightning_pos_z: f32,
    _pad: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: SkyUniforms;

// Vertex output
struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// Fullscreen triangle vertex shader - single oversized triangle covers entire viewport
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Standard fullscreen triangle: vertices at (-1,-1), (3,-1), (-1,3)
    // This single triangle covers the entire [-1,1] NDC range
    let x = f32(i32(vertex_index & 1u) * 4) - 1.0;
    let y = f32(i32((vertex_index >> 1u) & 1u) * 4) - 1.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

// ============================================================================
// NOISE FUNCTIONS
// ============================================================================

fn hash2d(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn hash3d(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 = p3 + dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(hash2d(i), hash2d(i + vec2<f32>(1.0, 0.0)), u.x),
        mix(hash2d(i + vec2<f32>(0.0, 1.0)), hash2d(i + vec2<f32>(1.0, 1.0)), u.x),
        u.y
    );
}

fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(mix(hash3d(i), hash3d(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash3d(i + vec3<f32>(0.0, 1.0, 0.0)), hash3d(i + vec3<f32>(1.0, 1.0, 0.0)), u.x), u.y),
        mix(mix(hash3d(i + vec3<f32>(0.0, 0.0, 1.0)), hash3d(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash3d(i + vec3<f32>(0.0, 1.0, 1.0)), hash3d(i + vec3<f32>(1.0, 1.0, 1.0)), u.x), u.y),
        u.z
    );
}

fn fbm2d(p: vec2<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise2d(pos);
        pos *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

fn fbm3d(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var pos = p;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise3d(pos);
        pos *= 2.0;
        amplitude *= 0.5;
    }
    return value;
}

// ============================================================================
// VOLUMETRIC CLOUD FUNCTIONS
// ============================================================================

fn cloud_density(p: vec3<f32>, time: f32) -> f32 {
    // Animate cloud position with more turbulent motion
    let flow = vec3<f32>(time * 0.03, time * 0.01, time * 0.025);
    let pos = p * uniforms.cloud_scale + flow;

    // Multi-scale noise for billowing clouds - more octaves for detail
    var density = fbm3d(pos * 0.25, 6);

    // Add turbulent detail at multiple scales for that dramatic storm look
    density += fbm3d(pos * 0.6 + flow * 0.5, 4) * 0.45;
    density += fbm3d(pos * 1.2 - flow * 0.3, 3) * 0.25;

    // Add swirling motion for storm effect
    let swirl = sin(pos.x * 0.5 + time * 0.1) * cos(pos.z * 0.5 + time * 0.08) * 0.15;
    density += swirl;

    // Shape clouds - sharper threshold for more defined edges
    let threshold = 1.0 - uniforms.cloud_coverage;
    density = smoothstep(threshold - 0.1, threshold + 0.2, density);

    // Height falloff - allow clouds to be thicker overall
    let height_factor = 1.0 - pow(abs(p.y - 0.5) * 1.5, 1.5);
    density *= max(height_factor, 0.0);

    return density * uniforms.cloud_density;
}

// Dramatic cloud layer for the storm
fn storm_cloud_layer(p: vec3<f32>, time: f32) -> f32 {
    let flow = vec3<f32>(time * 0.04, 0.0, time * 0.03);
    let pos = p + flow;

    // Large-scale storm structure
    var density = fbm3d(pos * 0.15, 4) * 1.5;

    // Add dramatic billowing
    let billow = pow(fbm3d(pos * 0.4, 3), 2.0) * 0.8;
    density += billow;

    return max(density - 0.3, 0.0);
}

// ============================================================================
// NEBULA / SPACE EFFECTS
// ============================================================================

fn nebula_color(dir: vec3<f32>, time: f32) -> vec3<f32> {
    // Create swirling nebula patterns - matching fiery broken atmosphere reference
    let nebula_pos = dir * 3.0 + vec3<f32>(time * 0.01, time * 0.005, 0.0);

    let n1 = fbm3d(nebula_pos, 4);
    let n2 = fbm3d(nebula_pos * 1.5 + 5.0, 3);
    let n3 = fbm3d(nebula_pos * 0.8 - 3.0, 3);

    // Fiery nebula: deep crimson, dark orange, ember sparks
    let crimson = vec3<f32>(1.2, 0.12, 0.04) * n1 * 2.5;
    let dark_orange = vec3<f32>(0.8, 0.25, 0.05) * n2 * 2.0;
    let ember = vec3<f32>(1.5, 0.6, 0.1) * pow(n1 * n2, 2.0) * 4.0;
    // Subtle dark purple for depth
    let deep_void = vec3<f32>(0.15, 0.03, 0.12) * n3 * 1.0;

    return crimson + dark_orange + ember + deep_void;
}

fn stars(dir: vec3<f32>) -> f32 {
    // Random star field
    let grid = floor(dir * 200.0);
    let star_hash = hash3d(grid);

    // Only show some stars (sparse)
    if (star_hash > 0.995) {
        // Star twinkle
        let twinkle = sin(uniforms.time * 2.0 + star_hash * 100.0) * 0.3 + 0.7;
        return pow(star_hash, 20.0) * twinkle * 2.0;
    }
    return 0.0;
}

// ============================================================================
// MOLTEN PLANET IN SKY
// ============================================================================

fn molten_planet(dir: vec3<f32>, planet_dir: vec3<f32>, planet_radius: f32, time: f32) -> vec3<f32> {
    let dot_val = dot(dir, planet_dir);
    let angular_dist = acos(clamp(dot_val, -1.0, 1.0));

    // Check if ray hits planet or its atmosphere
    if (angular_dist > planet_radius * 2.5) {
        return vec3<f32>(0.0);
    }

    let t = angular_dist / planet_radius;

    // Outer atmospheric haze - wide red/orange glow
    if (t > 1.2) {
        let atmo_t = (t - 1.2) / 1.3;
        let atmo_glow = exp(-atmo_t * 2.0);
        return vec3<f32>(1.0, 0.2, 0.05) * atmo_glow * 0.5;
    }

    // Inner atmosphere - bright corona ring
    if (t > 1.0) {
        let corona_t = (t - 1.0) / 0.2;
        let corona = exp(-corona_t * 4.0);
        // Bright white-orange corona like in the reference
        return vec3<f32>(3.0, 1.2, 0.4) * corona;
    }

    // Planet surface with lava cracks - matching reference molten planet
    let surface_uv = vec2<f32>(
        atan2(dir.x - planet_dir.x, dir.z - planet_dir.z),
        t
    );

    // Multiple lava crack scales for detail
    let lava_large = fbm2d(surface_uv * 6.0 + time * 0.05, 4);
    let lava_fine = fbm2d(surface_uv * 15.0 + time * 0.08, 3);
    let cracks = smoothstep(0.35, 0.55, lava_large) + smoothstep(0.5, 0.7, lava_fine) * 0.4;
    let crack_val = clamp(cracks, 0.0, 1.0);

    // Very dark crust with bright molten cracks
    let crust = vec3<f32>(0.06, 0.02, 0.01);       // Near-black rock
    let lava_hot = vec3<f32>(4.0, 1.0, 0.12);       // Bright HDR molten
    let lava_warm = vec3<f32>(1.5, 0.3, 0.05);      // Dimmer lava

    let surface = mix(crust, mix(lava_warm, lava_hot, crack_val), crack_val);

    // Limb darkening with bright rim
    let limb = pow(1.0 - t, 0.4);
    let rim = pow(t, 3.0) * 0.3; // Subtle bright rim at edge

    return surface * limb + vec3<f32>(1.5, 0.4, 0.1) * rim;
}

// ============================================================================
// LIGHTNING
// ============================================================================

fn lightning_bolt(uv: vec2<f32>, time: f32) -> f32 {
    if (uniforms.lightning_intensity < 0.01) {
        return 0.0;
    }

    // Lightning position
    let bolt_x = uniforms.lightning_pos_x;
    let bolt_z = uniforms.lightning_pos_z;

    // Distance from bolt center
    let dist = length(uv - vec2<f32>(bolt_x, bolt_z));

    // Bolt shape - jagged line
    let bolt_width = 0.02 + fbm2d(vec2<f32>(uv.y * 20.0, time * 100.0), 3) * 0.03;

    if (dist < bolt_width) {
        return uniforms.lightning_intensity * (1.0 - dist / bolt_width);
    }

    // Glow around bolt
    let glow = exp(-dist * 10.0) * uniforms.lightning_intensity * 0.5;
    return glow;
}

// ============================================================================
// MAIN FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Reconstruct ray direction
    let camera_pos = vec3<f32>(uniforms.camera_pos_x, uniforms.camera_pos_y, uniforms.camera_pos_z);
    let ndc = vec4<f32>(in.uv.x * 2.0 - 1.0, (1.0 - in.uv.y) * 2.0 - 1.0, 1.0, 1.0);
    let world_pos = uniforms.inv_view_proj * ndc;
    let ray_dir = normalize(world_pos.xyz / world_pos.w - camera_pos);

    let time = uniforms.time;
    let up = ray_dir.y;

    // ========================================================================
    // BASE SKY GRADIENT
    // ========================================================================

    let zenith = vec3<f32>(uniforms.zenith_r, uniforms.zenith_g, uniforms.zenith_b);
    let horizon = vec3<f32>(uniforms.horizon_r, uniforms.horizon_g, uniforms.horizon_b);

    // Purple zenith, orange horizon gradient
    var sky_color: vec3<f32>;
    if (up > 0.0) {
        // Above horizon
        let t = pow(up, 0.5);
        sky_color = mix(horizon, zenith, t);
    } else {
        // Below horizon - lava glow
        let lava_glow = vec3<f32>(uniforms.lava_glow_r, uniforms.lava_glow_g, uniforms.lava_glow_b);
        let t = pow(-up, 0.7);
        sky_color = mix(horizon, lava_glow * uniforms.lava_glow_strength, t);
    }

    // ========================================================================
    // NEBULA & STARS (upper sky only)
    // ========================================================================

    if (up > 0.2) {
        let nebula_blend = smoothstep(0.2, 0.6, up);
        let nebula = nebula_color(ray_dir, time);
        sky_color = sky_color + nebula * nebula_blend * 0.3;

        // Stars
        let star = stars(ray_dir);
        sky_color = sky_color + vec3<f32>(star);
    }

    // ========================================================================
    // MOLTEN PLANET (visible in upper sky)
    // ========================================================================

    // Large molten planet dominating the sky (matching reference image)
    let planet_dir = normalize(vec3<f32>(0.3, 0.35, -0.8));
    let planet_radius = 0.22; // Much larger - key feature of the skybox
    let planet = molten_planet(ray_dir, planet_dir, planet_radius, time);
    if (length(planet) > 0.01) {
        sky_color = sky_color + planet;
    }

    // ========================================================================
    // VOLUMETRIC CLOUDS (ray march) - Enhanced dramatic storm
    // ========================================================================

    if (up > -0.1) {  // Extend slightly below horizon for dramatic effect
        let cloud_base = 40.0;
        let cloud_top = 250.0;

        // Calculate ray intersection with cloud layer
        let t_base = (cloud_base - camera_pos.y) / max(ray_dir.y, 0.001);
        let t_top = (cloud_top - camera_pos.y) / max(ray_dir.y, 0.001);

        let t_start = max(t_base, 0.0);
        let t_end = max(t_top, t_start);

        if (t_end > t_start) {
            // Ray march with 12 steps for better quality
            let steps = 12;
            let step_size = (t_end - t_start) / f32(steps);

            var cloud_accum = vec3<f32>(0.0);
            var cloud_alpha = 0.0;
            var t = t_start;

            for (var i = 0; i < steps; i++) {
                if (cloud_alpha > 0.95) { break; }

                let pos = camera_pos + ray_dir * t;
                let cloud_pos = vec3<f32>(
                    pos.x * 0.008,
                    (pos.y - cloud_base) / (cloud_top - cloud_base),
                    pos.z * 0.008
                );

                let density = cloud_density(cloud_pos, time);
                let storm_density = storm_cloud_layer(cloud_pos, time);
                let total_density = density + storm_density * 0.5;

                if (total_density > 0.01) {
                    // Cloud lighting - enhanced dramatic colors
                    let sun_dir = normalize(vec3<f32>(uniforms.sun_dir_x, uniforms.sun_dir_y, uniforms.sun_dir_z));
                    let sun_dot = dot(ray_dir, sun_dir) * 0.5 + 0.5;

                    // Dark cloud core (deep red-black, not purple)
                    let dark_cloud = vec3<f32>(0.06, 0.01, 0.01);
                    // Fiery orange-red lit cloud edges
                    let lit_cloud = vec3<f32>(1.0, 0.35, 0.08);
                    // Hot orange-yellow rim lighting (fire atmosphere)
                    let rim_color = vec3<f32>(1.5, 0.5, 0.12);

                    // Mix based on density and light - more dramatic contrast
                    let light_penetration = pow(1.0 - total_density, 1.5) * sun_dot;
                    var sample_color = mix(dark_cloud, lit_cloud, light_penetration);

                    // Add intense rim lighting at cloud edges
                    let edge_factor = pow(1.0 - total_density, 2.5);
                    sample_color = sample_color + rim_color * edge_factor * sun_dot * 2.0;

                    // Strong lava glow from below - key for the fiery atmosphere look
                    let bottom_glow = pow(1.0 - cloud_pos.y, 1.5) * uniforms.lava_glow_strength * 0.8;
                    let lava_light = vec3<f32>(1.5, 0.4, 0.08) * bottom_glow * (1.0 - total_density * 0.5);
                    sample_color = sample_color + lava_light;

                    // Dark red-black top (void of space)
                    let top_glow = pow(cloud_pos.y, 2.0) * 0.15;
                    sample_color = sample_color + vec3<f32>(0.15, 0.02, 0.02) * top_glow;

                    // Accumulate with enhanced alpha
                    let sample_alpha = total_density * 0.3;
                    cloud_accum = cloud_accum + sample_color * sample_alpha * (1.0 - cloud_alpha);
                    cloud_alpha = cloud_alpha + sample_alpha * (1.0 - cloud_alpha);
                }

                t = t + step_size;
            }

            // Blend clouds more strongly
            sky_color = mix(sky_color, cloud_accum / max(cloud_alpha, 0.001), cloud_alpha * 1.2);
        }
    }

    // ========================================================================
    // LIGHTNING - Enhanced dramatic bolts
    // ========================================================================

    if (uniforms.lightning_intensity > 0.0) {
        let lightning = lightning_bolt(in.uv, time);
        // Blue-white lightning with pink tinge
        let lightning_color = vec3<f32>(0.85, 0.9, 1.0) + vec3<f32>(0.3, 0.1, 0.2) * uniforms.lightning_intensity;
        sky_color = sky_color + lightning_color * lightning * 4.0;

        // Flash illuminates clouds dramatically
        sky_color = sky_color * (1.0 + uniforms.lightning_intensity * 0.8);

        // Add purple flash to entire sky during lightning
        sky_color = sky_color + vec3<f32>(0.15, 0.05, 0.2) * uniforms.lightning_intensity;
    }

    // ========================================================================
    // HORIZON FOG
    // ========================================================================

    // Fiery horizon haze - warm orange glow at the horizon line
    let fog_factor = pow(1.0 - abs(up), 5.0);
    let fog_color = vec3<f32>(0.6, 0.18, 0.05);
    sky_color = mix(sky_color, fog_color, fog_factor * 0.5);

    // ========================================================================
    // POST PROCESSING
    // ========================================================================

    // ACES tonemapping (HDR -> LDR) - keeps bright lava/emissive under control
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e_val = 0.14;
    sky_color = clamp((sky_color * (a * sky_color + b)) / (sky_color * (c * sky_color + d) + e_val), vec3<f32>(0.0), vec3<f32>(1.0));

    // NOTE: No manual gamma correction - sRGB surface format handles it automatically.
    // Manual pow(1/2.2) on sRGB surface = double gamma = washed out/pale colors.

    return vec4<f32>(sky_color, 1.0);
}
