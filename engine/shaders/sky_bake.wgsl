// Sky Bake Shader - Renders sky to a single cubemap face (US-0S03)
//
// This shader renders the full sky (gradient, sun, stars, aurora, moon, clouds)
// to one cubemap face. It is invoked once per face with `face_index` selecting
// the face orientation (+X, -X, +Y, -Y, +Z, -Z).
//
// The sky rendering logic reuses functions from sky_gradient.wgsl.

// =============================================================================
// SKY SETTINGS UNIFORM (352 bytes, matches sky_gradient.wgsl / Rust SkySettings)
// =============================================================================

struct SkySettings {
    // Time settings (16 bytes)
    time_of_day: f32,
    cycle_speed: f32,
    elapsed_time: f32,
    _pad0: f32,

    // Sun (48 bytes)
    sun_dir: vec3<f32>,
    sun_sharpness: f32,
    sun_color: vec4<f32>,
    sun_strength: f32,
    sun_enabled: u32,
    _pad1: f32,
    _pad2: f32,

    // Gradient colors (96 bytes)
    day_horizon: vec4<f32>,
    day_zenith: vec4<f32>,
    sunset_horizon: vec4<f32>,
    sunset_zenith: vec4<f32>,
    night_horizon: vec4<f32>,
    night_zenith: vec4<f32>,

    // Stars (16 bytes)
    stars_enabled: u32,
    stars_threshold: f32,
    stars_blink_speed: f32,
    stars_density: f32,

    // Aurora (48 bytes)
    aurora_enabled: u32,
    aurora_intensity: f32,
    aurora_speed: f32,
    aurora_height: f32,
    aurora_color_bottom: vec4<f32>,
    aurora_color_top: vec4<f32>,

    // Weather (16 bytes)
    weather_type: u32,
    cloud_coverage: f32,
    cloud_density: f32,
    cloud_speed: f32,

    // Cloud appearance (16 bytes)
    cloud_height: f32,
    cloud_thickness: f32,
    cloud_scale: f32,
    cloud_sharpness: f32,

    // Season (16 bytes)
    season: u32,
    season_intensity: f32,
    _pad3: f32,
    _pad4: f32,

    // Temperature (16 bytes)
    temperature: f32,
    humidity: f32,
    wind_speed: f32,
    wind_direction: f32,

    // Rain (16 bytes)
    rain_intensity: f32,
    rain_visibility: f32,
    lightning_intensity: f32,
    _pad5: f32,

    // Moon (48 bytes)
    moon_enabled: u32,
    moon_phase: f32,
    lunar_day: f32,
    moon_sharpness: f32,
    moon_color: vec4<f32>,
    moon_strength: f32,
    moon_size: f32,
    _pad6: f32,
    _pad7: f32,
}

struct BakeParams {
    face_index: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0)
var<uniform> sky: SkySettings;

@group(0) @binding(1)
var<uniform> params: BakeParams;

// =============================================================================
// VERTEX SHADER - fullscreen triangle
// =============================================================================

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle: 3 vertices cover the entire screen
    var out: VertexOutput;
    let x = f32(i32(vertex_index) / 2) * 4.0 - 1.0;
    let y = f32(i32(vertex_index) % 2) * 4.0 - 1.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    // UV in [0,1] range
    out.uv = vec2<f32>(x * 0.5 + 0.5, 1.0 - (y * 0.5 + 0.5));
    return out;
}

// =============================================================================
// CUBEMAP FACE RAY DIRECTION
// =============================================================================

/// Convert pixel UV [0,1]^2 and face index to a world-space ray direction.
/// Face order: 0=+X, 1=-X, 2=+Y, 3=-Y, 4=+Z, 5=-Z
fn cubemap_ray_direction(uv: vec2<f32>, face: u32) -> vec3<f32> {
    // Map UV from [0,1] to [-1,1]
    let u = uv.x * 2.0 - 1.0;
    let v = uv.y * 2.0 - 1.0;

    var dir: vec3<f32>;
    switch face {
        case 0u { // +X
            dir = vec3<f32>(1.0, -v, -u);
        }
        case 1u { // -X
            dir = vec3<f32>(-1.0, -v, u);
        }
        case 2u { // +Y
            dir = vec3<f32>(u, 1.0, v);
        }
        case 3u { // -Y
            dir = vec3<f32>(u, -1.0, -v);
        }
        case 4u { // +Z
            dir = vec3<f32>(u, -v, 1.0);
        }
        default { // -Z (case 5u)
            dir = vec3<f32>(-u, -v, -1.0);
        }
    }
    return normalize(dir);
}

// =============================================================================
// NOISE FUNCTIONS (from sky_gradient.wgsl)
// =============================================================================

fn hash31(p: vec3<f32>) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

fn hash33(p: vec3<f32>) -> vec3<f32> {
    var p3 = fract(p * vec3<f32>(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

fn noise3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(
            mix(hash31(i + vec3<f32>(0.0, 0.0, 0.0)), hash31(i + vec3<f32>(1.0, 0.0, 0.0)), u.x),
            mix(hash31(i + vec3<f32>(0.0, 1.0, 0.0)), hash31(i + vec3<f32>(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(hash31(i + vec3<f32>(0.0, 0.0, 1.0)), hash31(i + vec3<f32>(1.0, 0.0, 1.0)), u.x),
            mix(hash31(i + vec3<f32>(0.0, 1.0, 1.0)), hash31(i + vec3<f32>(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

fn voronoi3d(p: vec3<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);

    var min_dist = 1.0;
    for (var z = -1; z <= 1; z++) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let neighbor = vec3<f32>(f32(x), f32(y), f32(z));
                let point = hash33(i + neighbor);
                let diff = neighbor + point - f;
                let dist = length(diff);
                min_dist = min(min_dist, dist);
            }
        }
    }
    return min_dist;
}

// =============================================================================
// PERLIN NOISE (for clouds)
// =============================================================================

fn perlin_hash(p: vec3<i32>) -> u32 {
    let permutation = array<u32, 16>(
        151u, 160u, 137u, 91u, 90u, 15u, 131u, 13u,
        201u, 95u, 96u, 53u, 194u, 233u, 7u, 225u
    );
    var h = u32(p.x & 255);
    h = permutation[h & 15u] ^ u32(p.y & 255);
    h = permutation[h & 15u] ^ u32(p.z & 255);
    return h;
}

fn perlin_fade(t: f32) -> f32 {
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
}

fn perlin_gradient(hash: u32, p: vec3<f32>) -> f32 {
    let h = hash & 15u;
    let u = select(p.y, p.x, h < 8u);
    let v = select(select(p.x, p.z, h == 12u || h == 14u), p.y, h < 4u);
    return select(-u, u, (h & 1u) == 0u) + select(-v, v, (h & 2u) == 0u);
}

fn perlin_noise_3d(p: vec3<f32>) -> f32 {
    let pi = vec3<i32>(floor(p));
    let pf = fract(p);
    let u = vec3<f32>(perlin_fade(pf.x), perlin_fade(pf.y), perlin_fade(pf.z));

    let n000 = perlin_gradient(perlin_hash(pi + vec3<i32>(0, 0, 0)), pf - vec3<f32>(0.0, 0.0, 0.0));
    let n100 = perlin_gradient(perlin_hash(pi + vec3<i32>(1, 0, 0)), pf - vec3<f32>(1.0, 0.0, 0.0));
    let n010 = perlin_gradient(perlin_hash(pi + vec3<i32>(0, 1, 0)), pf - vec3<f32>(0.0, 1.0, 0.0));
    let n110 = perlin_gradient(perlin_hash(pi + vec3<i32>(1, 1, 0)), pf - vec3<f32>(1.0, 1.0, 0.0));
    let n001 = perlin_gradient(perlin_hash(pi + vec3<i32>(0, 0, 1)), pf - vec3<f32>(0.0, 0.0, 1.0));
    let n101 = perlin_gradient(perlin_hash(pi + vec3<i32>(1, 0, 1)), pf - vec3<f32>(1.0, 0.0, 1.0));
    let n011 = perlin_gradient(perlin_hash(pi + vec3<i32>(0, 1, 1)), pf - vec3<f32>(0.0, 1.0, 1.0));
    let n111 = perlin_gradient(perlin_hash(pi + vec3<i32>(1, 1, 1)), pf - vec3<f32>(1.0, 1.0, 1.0));

    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);
    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);

    return mix(nxy0, nxy1, u.z);
}

fn cloud_fbm(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var total_amplitude = 0.0;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * perlin_noise_3d(p * frequency);
        total_amplitude += amplitude;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    return value / total_amplitude;
}

// =============================================================================
// SUN
// =============================================================================

fn get_sun_direction() -> vec3<f32> {
    let angle = sky.time_of_day * 6.28318;
    return normalize(vec3<f32>(sin(angle), -cos(angle), 0.3));
}

fn render_sun(view_dir: vec3<f32>) -> vec4<f32> {
    if sky.sun_enabled == 0u {
        return vec4<f32>(0.0);
    }
    let sun_dir = get_sun_direction();
    let sun_dot = max(dot(view_dir, sun_dir), 0.0);
    let sun_factor = pow(sun_dot, sky.sun_sharpness);
    return sky.sun_color * sun_factor * sky.sun_strength;
}

// =============================================================================
// MOON
// =============================================================================

fn get_moon_direction() -> vec3<f32> {
    let angle = sky.time_of_day * 6.28318 + 3.14159;
    let inclination = 0.09;
    let phase_offset = sky.moon_phase * 0.2;
    let horizontal = sin(angle + phase_offset);
    let vertical = cos(angle + phase_offset);
    let tilt = sin(sky.lunar_day * 0.21) * inclination;
    let min_elevation = 0.26;
    let adjusted_vertical = select(vertical, max(vertical, min_elevation), vertical > 0.0);
    return normalize(vec3<f32>(horizontal, adjusted_vertical + tilt, 0.15));
}

fn get_moon_illumination() -> f32 {
    return cos(sky.moon_phase * 6.28318);
}

fn perlin_moon_glow(rd: vec3<f32>, moon_dir: vec3<f32>, moon_dot: f32) -> f32 {
    if moon_dot < 0.9 {
        return 0.0;
    }
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let moon_right = normalize(cross(up, moon_dir));
    let moon_up = cross(moon_dir, moon_right);
    let diff = rd - moon_dir * moon_dot;
    let local_x = dot(diff, moon_right);
    let local_y = dot(diff, moon_up);
    let dist = sqrt(local_x * local_x + local_y * local_y);
    let angle = atan2(local_y, local_x);
    let time_offset = sky.elapsed_time * 0.05;
    let ray_noise1 = noise3d(vec3<f32>(angle * 3.0, dist * 20.0, time_offset)) * 0.5;
    let ray_noise2 = noise3d(vec3<f32>(angle * 7.0 + 1.5, dist * 40.0, time_offset * 0.7)) * 0.3;
    let ray_noise3 = noise3d(vec3<f32>(angle * 12.0 + 3.0, dist * 80.0, time_offset * 0.4)) * 0.2;
    let combined_noise = ray_noise1 + ray_noise2 + ray_noise3;
    let ray_base = smoothstep(0.25, 0.05, dist);
    let ray_variation = 0.5 + combined_noise * 0.5;
    let streak_angle = angle + time_offset * 0.3;
    let streak_noise = noise3d(vec3<f32>(streak_angle * 5.0, 0.0, time_offset * 0.2));
    let streaks = pow(streak_noise, 2.0) * smoothstep(0.3, 0.08, dist);
    let glow_intensity = ray_base * ray_variation + streaks * 0.4;
    let falloff = pow(max(moon_dot - 0.9, 0.0) / 0.1, 0.5);
    return glow_intensity * falloff * 0.6;
}

fn render_moon(rd: vec3<f32>) -> vec4<f32> {
    if sky.moon_enabled == 0u {
        return vec4<f32>(0.0);
    }
    let moon_dir = get_moon_direction();
    let moon_dot = dot(rd, moon_dir);
    let moon_angular_size = sky.moon_size;
    let moon_disc = smoothstep(1.0 - moon_angular_size, 1.0 - moon_angular_size * 0.8, moon_dot);
    let perlin_glow = perlin_moon_glow(rd, moon_dir, moon_dot);
    if moon_disc < 0.01 && perlin_glow < 0.01 {
        return vec4<f32>(0.0);
    }
    let illumination = get_moon_illumination();
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let moon_right = normalize(cross(up, moon_dir));
    let moon_up = cross(moon_dir, moon_right);
    let local_x = dot(rd - moon_dir * moon_dot, moon_right);
    let local_y = dot(rd - moon_dir * moon_dot, moon_up);
    let normalized_x = local_x / moon_angular_size * 2.0;
    let normalized_y = local_y / moon_angular_size * 2.0;
    let dist_from_center = sqrt(normalized_x * normalized_x + normalized_y * normalized_y);

    var phase_shadow = 1.0;
    if abs(illumination) < 0.99 {
        if illumination < 0.0 {
            let terminator_curve = sqrt(max(0.0, 1.0 - normalized_y * normalized_y)) * illumination;
            phase_shadow = smoothstep(terminator_curve - 0.1, terminator_curve + 0.1, normalized_x);
        } else {
            let terminator_curve = sqrt(max(0.0, 1.0 - normalized_y * normalized_y)) * illumination;
            phase_shadow = smoothstep(terminator_curve + 0.1, terminator_curve - 0.1, normalized_x);
        }
    }

    let crater_noise = noise3d(rd * 50.0) * 0.15 + 0.85;
    let mare_noise = noise3d(rd * 15.0 + vec3<f32>(100.0, 0.0, 0.0));
    let mare = smoothstep(0.4, 0.6, mare_noise) * 0.15;
    let surface_brightness = (crater_noise - mare) * phase_shadow;

    var moon_color = sky.moon_color.rgb;
    let earthshine = (1.0 - phase_shadow) * 0.03;
    let phase_brightness = max(abs(illumination) * 0.8 + 0.2, 0.0) * sky.moon_strength;
    let disc_edge = smoothstep(1.0, 0.95, dist_from_center);
    let moon_brightness = (surface_brightness * phase_brightness + earthshine) * disc_edge * moon_disc;

    let glow_color = vec3<f32>(0.8, 0.85, 0.95);
    let glow = perlin_glow * sky.moon_strength * phase_brightness;

    return vec4<f32>(moon_color * moon_brightness + glow_color * glow, max(moon_disc, perlin_glow * 0.5));
}

// =============================================================================
// SKY GRADIENT
// =============================================================================

fn get_sky_colors() -> array<vec4<f32>, 2> {
    var horizon: vec4<f32>;
    var zenith: vec4<f32>;

    if sky.time_of_day < 0.1 {
        let t = sky.time_of_day / 0.1;
        horizon = mix(sky.night_horizon, sky.sunset_horizon, t);
        zenith = mix(sky.night_zenith, sky.sunset_zenith, t);
    } else if sky.time_of_day < 0.15 {
        let t = (sky.time_of_day - 0.1) / 0.05;
        horizon = mix(sky.sunset_horizon, sky.day_horizon, t);
        zenith = mix(sky.sunset_zenith, sky.day_zenith, t);
    } else if sky.time_of_day < 0.4 {
        horizon = sky.day_horizon;
        zenith = sky.day_zenith;
    } else if sky.time_of_day < 0.5 {
        let t = (sky.time_of_day - 0.4) / 0.1;
        horizon = mix(sky.day_horizon, sky.sunset_horizon, t);
        zenith = mix(sky.day_zenith, sky.sunset_zenith, t);
    } else if sky.time_of_day < 0.6 {
        let t = (sky.time_of_day - 0.5) / 0.1;
        horizon = mix(sky.sunset_horizon, sky.night_horizon, t);
        zenith = mix(sky.sunset_zenith, sky.night_zenith, t);
    } else {
        horizon = sky.night_horizon;
        zenith = sky.night_zenith;
    }

    return array<vec4<f32>, 2>(horizon, zenith);
}

fn render_gradient(rd: vec3<f32>) -> vec3<f32> {
    let colors = get_sky_colors();
    let t = clamp(rd.y * 0.5 + 0.5, 0.0, 1.0);
    return mix(colors[0].rgb, colors[1].rgb, pow(t, 0.7));
}

// =============================================================================
// STARS
// =============================================================================

fn render_stars(rd: vec3<f32>) -> f32 {
    if sky.stars_enabled == 0u {
        return 0.0;
    }
    let rotation = sky.elapsed_time * 0.01;
    let c = cos(rotation);
    let s = sin(rotation);
    let rotated_dir = vec3<f32>(
        rd.x * c - rd.z * s,
        rd.y,
        rd.x * s + rd.z * c
    );
    let star_sample = rotated_dir * sky.stars_density;
    var star_noise = 1.0 - voronoi3d(star_sample);
    let mask = noise3d(rotated_dir * sky.stars_density * 0.5);
    star_noise *= (1.0 - smoothstep(0.4, 1.0, mask));
    let blink_offset = hash31(floor(star_sample));
    let blink = cos(sky.elapsed_time * sky.stars_blink_speed + blink_offset * 6.28) * 0.5 + 0.5;
    let blink_threshold = blink * 0.1;
    let star_intensity = smoothstep(sky.stars_threshold + blink_threshold, 1.0, star_noise);
    return star_intensity;
}

// =============================================================================
// AURORA
// =============================================================================

fn make_aurora_stripe(x: f32, half_size: f32) -> f32 {
    let base_value = fract(x);
    let left = smoothstep(0.5 - half_size, 0.5, base_value);
    let right = smoothstep(0.5 + half_size, 0.5, base_value);
    return left * right;
}

fn render_aurora(rd: vec3<f32>) -> vec4<f32> {
    if sky.aurora_enabled == 0u || rd.y < 0.01 {
        return vec4<f32>(0.0);
    }
    let num_samples = 8;
    var accumulated_color = vec3<f32>(0.0);
    var accumulated_alpha = 0.0;
    let flow_time = sky.elapsed_time * sky.aurora_speed;

    for (var i = 0; i < num_samples; i++) {
        let height_factor = f32(i) / f32(num_samples - 1);
        let height = sky.aurora_height * (0.5 + height_factor * 0.5);
        let t = height / max(rd.y, 0.01);
        let p = rd * t;
        let world_pos = p.xz;
        let flow_sample = world_pos * 0.1;
        let flow = vec2<f32>(
            noise3d(vec3<f32>(flow_sample.x, flow_sample.y, flow_time)),
            noise3d(vec3<f32>(flow_sample.x + 100.0, flow_sample.y, flow_time))
        );
        let flow_dir = normalize(flow);
        let wiggle_sample = world_pos * 0.3;
        let wiggle = vec2<f32>(
            noise3d(vec3<f32>(wiggle_sample.x, wiggle_sample.y, flow_time * 2.0)),
            noise3d(vec3<f32>(wiggle_sample.x + 50.0, wiggle_sample.y, flow_time * 2.0))
        ) * 2.0;
        let warped_pos = world_pos + flow_dir * 3.0 + wiggle + vec2<f32>(flow_time * 0.5, 0.0);
        let large_bands = make_aurora_stripe(warped_pos.x * 0.1, 0.2);
        let small_bands = make_aurora_stripe(warped_pos.x * 0.17, 0.1);
        let base_bands = pow(max(large_bands, small_bands), 3.0);
        let vertical_intensity = smoothstep(0.0, 0.15, height_factor) *
                                 smoothstep(1.0, 0.5, height_factor);
        let curtain = base_bands * vertical_intensity;
        let sample_alpha = curtain * 0.15;
        let sample_weight = sample_alpha * (1.0 - accumulated_alpha);
        let selected_color = mix(sky.aurora_color_bottom.rgb, sky.aurora_color_top.rgb, height_factor);
        accumulated_color += selected_color * curtain * sample_weight;
        accumulated_alpha += sample_alpha * (1.0 - accumulated_alpha);
        if accumulated_alpha > 0.95 {
            break;
        }
    }
    let up_factor = smoothstep(0.05, 0.7, rd.y);
    let final_alpha = accumulated_alpha * up_factor * sky.aurora_intensity;
    return vec4<f32>(accumulated_color * up_factor * sky.aurora_intensity, final_alpha);
}

// =============================================================================
// PROCEDURAL CLOUDS (no texture sampling - cubemap bake uses procedural noise)
// =============================================================================

fn get_night_visibility() -> f32 {
    if sky.time_of_day >= 0.55 || sky.time_of_day < 0.1 {
        if sky.time_of_day >= 0.55 && sky.time_of_day < 0.65 {
            return smoothstep(0.55, 0.65, sky.time_of_day);
        } else if sky.time_of_day >= 0.05 && sky.time_of_day < 0.1 {
            return 1.0 - smoothstep(0.05, 0.1, sky.time_of_day);
        } else if sky.time_of_day < 0.05 {
            return 1.0;
        } else {
            return 1.0;
        }
    }
    return 0.0;
}

fn bake_cloud_noise(rd: vec3<f32>, height_factor: f32) -> f32 {
    // Procedural FBM clouds for cubemap bake (no texture dependency)
    let wind_drift = sky.elapsed_time * 0.01;
    let wind_dir = vec2<f32>(
        cos(sky.wind_direction * 6.28318),
        sin(sky.wind_direction * 6.28318)
    );

    let horizontal_length = length(rd.xz);
    if horizontal_length < 0.001 {
        return 0.0;
    }
    let elevation_factor = 1.0 - rd.y * 0.5;
    let angle = atan2(rd.z, rd.x);
    var uv = vec2<f32>(
        (angle / 6.28318 + 0.5) * elevation_factor,
        horizontal_length * elevation_factor
    );
    uv += wind_dir * wind_drift;
    uv += vec2<f32>(height_factor * 0.1, height_factor * 0.05);

    let base = cloud_fbm(vec3<f32>(uv * sky.cloud_scale * 0.3, 0.0), 4);
    let detail = cloud_fbm(vec3<f32>((uv * sky.cloud_scale * 0.8 + vec2<f32>(0.37, 0.73)), 1.0), 3);
    let wisp = cloud_fbm(vec3<f32>((uv * sky.cloud_scale * 2.0 + vec2<f32>(0.91, 0.23)), 2.0), 2);

    return base * 0.6 + detail * 0.3 + wisp * 0.1;
}

fn render_clouds_bake(rd: vec3<f32>) -> vec4<f32> {
    if rd.y < 0.02 || sky.cloud_coverage < 0.01 {
        return vec4<f32>(0.0);
    }

    let sun_dir = get_sun_direction();
    let sun_height = sun_dir.y;
    let sun_brightness = smoothstep(-0.2, 0.4, sun_height);

    let num_samples = 6;
    var accumulated_color = vec3<f32>(0.0);
    var accumulated_alpha = 0.0;

    // Cloud colors by time of day
    let day_lit_color = vec3<f32>(1.0, 0.98, 0.96);
    let day_shadow_color = vec3<f32>(0.65, 0.70, 0.80);
    let sunset_lit_color = vec3<f32>(1.0, 0.65, 0.45);
    let sunset_shadow_color = vec3<f32>(0.85, 0.35, 0.35);
    let night_lit_color = vec3<f32>(0.12, 0.13, 0.18);
    let night_shadow_color = vec3<f32>(0.05, 0.05, 0.08);

    let sunrise_factor = smoothstep(0.0, 0.05, sky.time_of_day) * smoothstep(0.18, 0.08, sky.time_of_day);
    let day_factor = smoothstep(0.12, 0.18, sky.time_of_day) * smoothstep(0.42, 0.38, sky.time_of_day);
    let sunset_factor = smoothstep(0.38, 0.44, sky.time_of_day) * smoothstep(0.60, 0.52, sky.time_of_day);
    let night_early = smoothstep(0.55, 0.65, sky.time_of_day);
    let night_late = 1.0 - smoothstep(0.0, 0.08, sky.time_of_day);
    let night_factor = max(night_early, select(0.0, night_late, sky.time_of_day < 0.1));
    let golden_factor = max(sunrise_factor, sunset_factor);

    var cloud_lit_color = day_lit_color;
    var cloud_shadow_color = day_shadow_color;
    cloud_lit_color = mix(cloud_lit_color, sunset_lit_color, golden_factor * 0.85);
    cloud_shadow_color = mix(cloud_shadow_color, sunset_shadow_color, golden_factor * 0.7);
    cloud_lit_color = mix(cloud_lit_color, night_lit_color, night_factor);
    cloud_shadow_color = mix(cloud_shadow_color, night_shadow_color, night_factor);

    let sun_tint = vec3<f32>(1.0, 0.95, 0.88);
    let sun_tint_strength = day_factor * sun_brightness * 0.15;
    cloud_lit_color = mix(cloud_lit_color, cloud_lit_color * sun_tint, sun_tint_strength);

    let brightness_scale = mix(0.08, 1.0, sun_brightness);
    cloud_lit_color *= brightness_scale;
    cloud_shadow_color *= mix(0.15, 1.0, sun_brightness);

    for (var i = 0; i < num_samples; i++) {
        let height_factor = f32(i) / f32(num_samples - 1);
        let cloud_noise = bake_cloud_noise(rd, height_factor);
        let coverage_threshold = 1.0 - sky.cloud_coverage;
        var cloud_density = smoothstep(coverage_threshold, coverage_threshold + sky.cloud_sharpness, cloud_noise);
        cloud_density *= sky.cloud_density;
        let vertical_profile = smoothstep(0.0, 0.3, height_factor) * smoothstep(1.0, 0.7, height_factor);
        cloud_density *= vertical_profile;
        if cloud_density < 0.01 {
            continue;
        }

        let shadow_offset = 0.1;
        var shadow_rd = rd + sun_dir * shadow_offset;
        shadow_rd = normalize(shadow_rd);
        let shadow_noise = bake_cloud_noise(shadow_rd, height_factor);
        let self_shadow = 1.0 - smoothstep(0.3, 0.7, shadow_noise) * 0.5;

        let light_dot = dot(normalize(vec3<f32>(rd.x, 0.0, rd.z)), sun_dir) * 0.5 + 0.5;
        let lit_factor = mix(0.3, 1.0, light_dot) * self_shadow;
        let backlit = pow(max(dot(rd, -sun_dir), 0.0), 4.0);
        let silver_lining = backlit * 0.4 * sun_brightness;

        var sample_color = mix(cloud_shadow_color, cloud_lit_color, lit_factor);
        sample_color += vec3<f32>(1.0, 0.95, 0.9) * silver_lining;

        let base_alpha = cloud_density * 0.2;
        let sample_weight = base_alpha * (1.0 - accumulated_alpha);
        accumulated_color += sample_color * sample_weight;
        accumulated_alpha += base_alpha * (1.0 - accumulated_alpha);
        if accumulated_alpha > 0.95 {
            break;
        }
    }

    let horizon_fade = smoothstep(0.02, 0.15, rd.y);
    accumulated_alpha *= horizon_fade;
    let distance_fade = smoothstep(0.05, 0.25, rd.y);
    accumulated_alpha *= distance_fade;
    let night_opacity_scale = mix(0.10, 1.0, sun_brightness);
    accumulated_alpha *= night_opacity_scale;

    return vec4<f32>(accumulated_color, accumulated_alpha);
}

// =============================================================================
// MAIN SKY RENDER (for cubemap bake)
// =============================================================================

fn render_sky_bake(view_dir: vec3<f32>) -> vec3<f32> {
    let rd = normalize(view_dir);
    let night_visibility = get_night_visibility();
    let day_visibility = max(1.0 - night_visibility, 0.05);

    var color = render_gradient(rd);

    // Sun
    let sun = render_sun(rd);
    color += sun.rgb * day_visibility;

    // Clouds
    if rd.y > 0.0 {
        let clouds = render_clouds_bake(rd);
        color = mix(color, clouds.rgb, clouds.a);
    }

    // Moon
    let moon_visibility = smoothstep(0.3, 0.7, night_visibility) + 0.1;
    if rd.y > -0.1 {
        let moon = render_moon(rd);
        let moon_day_fade = mix(0.15, 1.0, night_visibility);
        color += moon.rgb * moon_visibility * moon_day_fade;
    }

    // Stars and aurora (night, above horizon)
    if rd.y > 0.0 {
        let stars = render_stars(rd);
        color += vec3<f32>(stars) * night_visibility;

        let aurora = render_aurora(rd);
        color = mix(color, color + aurora.rgb, aurora.a * night_visibility);
    }

    // Horizon haze
    let haze_intensity = 0.3 + sky.humidity * 0.2;
    let horizon_haze = exp(-abs(rd.y) * 6.0);
    let haze_color = mix(vec3<f32>(0.7, 0.75, 0.8), vec3<f32>(0.15, 0.15, 0.2), night_visibility);
    color = mix(color, haze_color, horizon_haze * haze_intensity);

    // Below horizon - ground reflection
    if rd.y < 0.0 {
        let ground_color = mix(vec3<f32>(0.3, 0.35, 0.3), vec3<f32>(0.1, 0.1, 0.15), night_visibility);
        let ground_blend = smoothstep(0.0, -0.2, rd.y);
        color = mix(color, ground_color, ground_blend);
    }

    return color;
}

// =============================================================================
// FRAGMENT SHADER
// =============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let rd = cubemap_ray_direction(in.uv, params.face_index);
    let color = render_sky_bake(rd);
    return vec4<f32>(color, 1.0);
}
