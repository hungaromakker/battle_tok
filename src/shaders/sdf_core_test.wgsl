// SDF Core Test Shader - Minimal isolated SDF renderer
// Purpose: Test equation-based graphics capabilities
// Includes: Human figure SDF, Unity-style infinite grid plane, Procedural Skybox
// Interactive object placement support
// Sky system adapted from bevy_sky_gradient (TanTanDev)

// Uniforms struct - 128 bytes total, must match Rust TestUniforms exactly
// WGSL std140 layout (no vec3 padding fields to avoid alignment complexity):
//   offset  0: camera_pos (vec3<f32>)    = 12 bytes
//   offset 12: time (f32)                = 4 bytes
//   offset 16: resolution (vec2<f32>)    = 8 bytes
//   offset 24: debug_mode (u32)          = 4 bytes
//   offset 28: human_visible (u32)       = 4 bytes
//   offset 32: camera_target (vec3<f32>) = 12 bytes (vec3 aligned to 16)
//   offset 44: grid_size (f32)           = 4 bytes
//   offset 48: volume_grid_visible (u32) = 4 bytes
//   offset 52: placement_height (f32)    = 4 bytes
//   offset 56: show_hud (u32)            = 4 bytes
//   offset 60: camera_pitch (f32)        = 4 bytes
//   offset 64: camera_mode (u32)         = 4 bytes
//   offset 68: show_perf_overlay (u32)   = 4 bytes
//   offset 72: perf_fps (f32)            = 4 bytes
//   offset 76: perf_frame_time_ms (f32)  = 4 bytes
//   offset 80: perf_entity_count (u32)   = 4 bytes
//   offset 84: perf_baked_sdf_count (u32)= 4 bytes
//   offset 88: perf_tile_buffer_kb (f32) = 4 bytes
//   offset 92: perf_gpu_memory_mb (f32)  = 4 bytes
//   offset 96: perf_active_tile_count (u32) = 4 bytes
//   offset 100: _pad0 (u32)              = 4 bytes
//   offset 104: _pad1 (u32)              = 4 bytes
//   offset 108: _pad2 (u32)              = 4 bytes (aligns player_position to 16)
//   offset 112: player_position (vec3<f32>) = 12 bytes (vec3 aligned to 16)
//   offset 124: _pad3 (u32)              = 4 bytes
//   Total: 128 bytes
struct Uniforms {
    camera_pos: vec3<f32>,
    time: f32,
    resolution: vec2<f32>,
    debug_mode: u32,
    human_visible: u32,           // Toggle human figure visibility
    camera_target: vec3<f32>,     // Pan target for look-at
    grid_size: f32,               // Small grid cell size
    volume_grid_visible: u32,     // 1 = show 3D volume grid for building
    placement_height: f32,        // Current height for placement preview
    show_hud: u32,                // 1 = show on-screen controls HUD
    camera_pitch: f32,            // Camera pitch in radians (negative = looking down)
    camera_mode: u32,             // 0 = third-person, 1 = first-person
    show_perf_overlay: u32,       // 1 = show performance metrics overlay (F12)
    perf_fps: f32,                // Current FPS
    perf_frame_time_ms: f32,      // Frame time in milliseconds
    perf_entity_count: u32,       // Number of placed entities
    perf_baked_sdf_count: u32,    // Number of allocated baked SDFs
    perf_tile_buffer_kb: f32,     // Tile buffer memory usage in KB
    perf_gpu_memory_mb: f32,      // Estimated GPU memory usage in MB
    perf_active_tile_count: u32,  // Number of active tiles
    use_sky_cubemap: u32,          // 1 = sample pre-baked cubemap, 0 = procedural sky
    _pad1: u32,                   // Padding scalar
    _pad2: u32,                   // Padding scalar (aligns player_position to 16)
    player_position: vec3<f32>,   // Player position for first-person body rendering
    _pad3: u32,                   // Final padding
}

// Sky settings - separate uniform block for easier updates
struct SkySettings {
    // Time settings
    time_of_day: f32,          // 0.0 = sunrise, 0.25 = noon, 0.5 = sunset, 0.75 = midnight
    cycle_speed: f32,          // Speed of day/night cycle (0 = paused)
    elapsed_time: f32,         // For animations
    _pad0: f32,

    // Sun
    sun_dir: vec3<f32>,
    sun_sharpness: f32,
    sun_color: vec4<f32>,
    sun_strength: f32,
    sun_enabled: u32,
    _pad1: f32,
    _pad2: f32,

    // Gradient colors (day/night palette)
    day_horizon: vec4<f32>,
    day_zenith: vec4<f32>,
    sunset_horizon: vec4<f32>,
    sunset_zenith: vec4<f32>,
    night_horizon: vec4<f32>,
    night_zenith: vec4<f32>,

    // Stars
    stars_enabled: u32,
    stars_threshold: f32,
    stars_blink_speed: f32,
    stars_density: f32,

    // Aurora
    aurora_enabled: u32,
    aurora_intensity: f32,
    aurora_speed: f32,
    aurora_height: f32,
    aurora_color_bottom: vec4<f32>,
    aurora_color_top: vec4<f32>,

    // Weather system
    weather_type: u32,
    cloud_coverage: f32,
    cloud_density: f32,
    cloud_speed: f32,

    cloud_height: f32,
    cloud_thickness: f32,
    cloud_scale: f32,
    cloud_sharpness: f32,

    season: u32,
    season_intensity: f32,
    _pad3: f32,
    _pad4: f32,

    temperature: f32,
    humidity: f32,
    wind_speed: f32,
    wind_direction: f32,

    rain_intensity: f32,
    rain_visibility: f32,
    lightning_intensity: f32,
    haze_enabled: u32,          // 0 = haze OFF, 1 = haze ON (K key to toggle)

    // Fog settings
    fog_density: f32,           // 0.005 = good visibility, ~50% fog at 200 units
    fog_start_distance: f32,    // Distance at which fog starts (default: 50.0)
    fog_enabled: u32,           // 0 = fog OFF, 1 = fog ON (L key to toggle)
    _pad_fog1: f32,

    // Moon system
    moon_enabled: u32,
    moon_phase: f32,          // 0.0 = new moon, 0.5 = full moon, 1.0 = new moon again
    lunar_day: f32,           // Current day in lunar cycle (0-29.5)
    moon_sharpness: f32,
    moon_color: vec4<f32>,
    moon_strength: f32,
    moon_size: f32,
    _pad6: f32,
    _pad7: f32,
}

// Dynamic entity for placed objects
// MUST match Rust struct layout EXACTLY (48 bytes total)
// Rust uses explicit padding for cross-platform GPU buffer compatibility
struct PlacedEntity {
    position: vec3<f32>,      // offset 0,  size 12
    _pad_after_pos: u32,      // offset 12, size 4  (PADDING to match Rust)
    entity_type: u32,         // offset 16, size 4
    _pad_before_scale_0: u32, // offset 20, size 4  (PADDING for vec3 alignment)
    _pad_before_scale_1: u32, // offset 24, size 4  (PADDING for vec3 alignment)
    _pad_before_scale_2: u32, // offset 28, size 4  (PADDING for vec3 alignment)
    scale: vec3<f32>,         // offset 32, size 12
    color_packed: u32,        // offset 44, size 4
    // Total: 48 bytes - MATCHES Rust PlacedEntity exactly!
}

// Entity buffer for placed objects
struct EntityBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    entities: array<PlacedEntity, 64>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var<storage, read> placed_entities: EntityBuffer;

@group(0) @binding(2)
var<uniform> sky: SkySettings;

// ============================================================================
// FROXEL + TILE CULLING DATA (bindings 3-5)
// ============================================================================

// Froxel grid constants (must match Rust froxel_config.rs)
const FROXEL_TILES_X: u32 = 16u;
const FROXEL_TILES_Y: u32 = 16u;
const FROXEL_DEPTH_SLICES: u32 = 24u;
const TOTAL_FROXELS: u32 = 6144u; // 16 * 16 * 24
const MAX_SDFS_PER_FROXEL: u32 = 64u;

// Tile culling constants (must match Rust culling.rs)
const TILE_SIZE: u32 = 16u;
const MAX_ENTITIES_PER_TILE: u32 = 32u;

// Per-froxel axis-aligned bounding box in world space (32 bytes)
struct FroxelBounds {
    min_x: f32,
    min_y: f32,
    min_z: f32,
    _pad0: u32,
    max_x: f32,
    max_y: f32,
    max_z: f32,
    _pad1: u32,
}

// Buffer of all froxel bounds
struct FroxelBoundsBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    bounds: array<FroxelBounds, 6144>,
}

// Per-froxel list of SDF entity indices (272 bytes)
struct FroxelSDFList {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    sdf_indices: array<u32, 64>,
}

// Buffer of all froxel SDF lists
struct FroxelSDFListBuffer {
    count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    lists: array<FroxelSDFList, 6144>,
}

// Per-tile entity list for screen-space culling (136 bytes)
struct TileData {
    entity_count: u32,
    _padding: u32,
    entity_indices: array<u32, 32>,
}

// Buffer of all tiles
struct TileBuffer {
    tiles_x: u32,
    tiles_y: u32,
    tile_size: u32,
    total_tiles: u32,
    tiles: array<TileData>,
}

@group(0) @binding(3)
var<storage, read> froxel_bounds: FroxelBoundsBuffer;

@group(0) @binding(4)
var<storage, read> froxel_sdf_lists: FroxelSDFListBuffer;

@group(0) @binding(5)
var<storage, read> tile_data: TileBuffer;

@group(0) @binding(8)
var sky_cubemap: texture_cube<f32>;

@group(0) @binding(9)
var sky_sampler: sampler;

// ============================================================================
// VERTEX SHADER - Fullscreen triangle
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
    return out;
}

// ============================================================================
// NOISE FUNCTIONS (for stars and aurora)
// ============================================================================

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

// ============================================================================
// SDF PRIMITIVES - Core equation-based shapes
// ============================================================================

fn sdf_sphere(p: vec3<f32>, radius: f32) -> f32 {
    return length(p) - radius;
}

fn sdf_ellipsoid(p: vec3<f32>, r: vec3<f32>) -> f32 {
    let k0 = length(p / r);
    let k1 = length(p / (r * r));
    return k0 * (k0 - 1.0) / k1;
}

fn sdf_box(p: vec3<f32>, half_size: vec3<f32>) -> f32 {
    let q = abs(p) - half_size;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0);
}

fn sdf_rounded_box(p: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let q = abs(p) - b;
    return length(max(q, vec3<f32>(0.0))) + min(max(q.x, max(q.y, q.z)), 0.0) - r;
}

fn sdf_capsule(p: vec3<f32>, a: vec3<f32>, b: vec3<f32>, r: f32) -> f32 {
    let pa = p - a;
    let ba = b - a;
    let h = clamp(dot(pa, ba) / dot(ba, ba), 0.0, 1.0);
    return length(pa - ba * h) - r;
}

fn sdf_cylinder(p: vec3<f32>, h: f32, r: f32) -> f32 {
    let d = abs(vec2<f32>(length(p.xz), p.y)) - vec2<f32>(r, h);
    return min(max(d.x, d.y), 0.0) + length(max(d, vec2<f32>(0.0)));
}

fn sdf_torus(p: vec3<f32>, t: vec2<f32>) -> f32 {
    let q = vec2<f32>(length(p.xz) - t.x, p.y);
    return length(q) - t.y;
}

// Smooth minimum for blending shapes (organic forms)
fn smin(a: f32, b: f32, k: f32) -> f32 {
    let h = max(k - abs(a - b), 0.0) / k;
    return min(a, b) - h * h * k * 0.25;
}

// Smooth maximum for intersection
fn smax(a: f32, b: f32, k: f32) -> f32 {
    return -smin(-a, -b, k);
}

// ============================================================================
// PROCEDURAL SKYBOX WITH DAY/NIGHT CYCLE
// ============================================================================

// Calculate sun direction based on time of day
fn get_sun_direction() -> vec3<f32> {
    // time_of_day: 0.0 = sunrise (east), 0.25 = noon (top), 0.5 = sunset (west), 0.75 = midnight (below)
    let angle = sky.time_of_day * 6.28318; // 2*PI
    return normalize(vec3<f32>(
        sin(angle),           // X: east to west
        -cos(angle),          // Y: up at noon, down at midnight
        0.3                   // Z: slight tilt
    ));
}

// Render sun disc and glow
fn render_sun(rd: vec3<f32>) -> vec4<f32> {
    if sky.sun_enabled == 0u {
        return vec4<f32>(0.0);
    }

    let sun_dir = get_sun_direction();
    let sun_dot = max(dot(rd, sun_dir), 0.0);
    let sun_factor = pow(sun_dot, sky.sun_sharpness);

    return sky.sun_color * sun_factor * sky.sun_strength;
}

// ============================================================================
// MOON SYSTEM - Lunar cycle with phases
// ============================================================================

// Calculate moon direction (opposite to sun with slight orbital offset)
fn get_moon_direction() -> vec3<f32> {
    // Moon travels across the sky dome, opposite to sun
    // angle: 0 = midnight (moon at zenith), 0.5 = noon (moon below horizon)
    let angle = sky.time_of_day * 6.28318 + 3.14159;
    let inclination = 0.09; // ~5 degrees orbital tilt
    let phase_offset = sky.moon_phase * 0.2;

    // Calculate moon position on sky dome
    // X: horizontal position across sky (east-west)
    // Y: vertical height (must be positive when visible, high = far away in sky)
    // Z: depth (north-south tilt for variation)
    let horizontal = sin(angle + phase_offset);
    let vertical = cos(angle + phase_offset); // Positive when above horizon
    let tilt = sin(sky.lunar_day * 0.21) * inclination;

    // Clamp vertical to ensure moon appears high in sky, not at horizon
    // Moon minimum elevation is 15 degrees above horizon when visible
    let min_elevation = 0.26; // sin(15 degrees)
    let adjusted_vertical = select(vertical, max(vertical, min_elevation), vertical > 0.0);

    return normalize(vec3<f32>(
        horizontal,
        adjusted_vertical + tilt,
        0.15 // Slight forward tilt for 3D effect
    ));
}

// Calculate the illuminated portion of the moon based on phase
// Returns a value from -1 (new moon, dark side facing) to 1 (full moon, fully lit)
fn get_moon_illumination() -> f32 {
    // moon_phase: 0.0 = new moon, 0.25 = first quarter, 0.5 = full moon, 0.75 = last quarter
    return cos(sky.moon_phase * 6.28318);
}

// Get moon phase name as a debug value (0-7 for 8 phases)
fn get_moon_phase_index() -> u32 {
    // 0: New Moon
    // 1: Waxing Crescent
    // 2: First Quarter
    // 3: Waxing Gibbous
    // 4: Full Moon
    // 5: Waning Gibbous
    // 6: Last Quarter
    // 7: Waning Crescent
    return u32(sky.moon_phase * 8.0) % 8u;
}

// Perlin noise glow for moon - smooth flowing rays
fn perlin_moon_glow(rd: vec3<f32>, moon_dir: vec3<f32>, moon_dot: f32) -> f32 {
    // Only calculate glow if looking somewhat toward moon
    if moon_dot < 0.9 {
        return 0.0;
    }

    // Create radial coordinates around moon center
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let moon_right = normalize(cross(up, moon_dir));
    let moon_up = cross(moon_dir, moon_right);

    // Project onto moon plane
    let diff = rd - moon_dir * moon_dot;
    let local_x = dot(diff, moon_right);
    let local_y = dot(diff, moon_up);

    // Distance from moon center
    let dist = sqrt(local_x * local_x + local_y * local_y);

    // Angle around moon (for ray direction variation)
    let angle = atan2(local_y, local_x);

    // Use Perlin noise for smooth ray variation
    // Multiple octaves for more detail
    let time_offset = sky.elapsed_time * 0.05;  // Slow rotation of rays

    // Sample noise at different scales for flowing rays
    let ray_noise1 = noise3d(vec3<f32>(angle * 3.0, dist * 20.0, time_offset)) * 0.5;
    let ray_noise2 = noise3d(vec3<f32>(angle * 7.0 + 1.5, dist * 40.0, time_offset * 0.7)) * 0.3;
    let ray_noise3 = noise3d(vec3<f32>(angle * 12.0 + 3.0, dist * 80.0, time_offset * 0.4)) * 0.2;

    let combined_noise = ray_noise1 + ray_noise2 + ray_noise3;

    // Create ray intensity - stronger near center, fading outward
    let ray_base = smoothstep(0.25, 0.05, dist);  // Fade from moon edge
    let ray_variation = 0.5 + combined_noise * 0.5;  // Noise modulates ray brightness

    // Additional radial rays using noise-based streaks
    let streak_angle = angle + time_offset * 0.3;
    let streak_noise = noise3d(vec3<f32>(streak_angle * 5.0, 0.0, time_offset * 0.2));
    let streaks = pow(streak_noise, 2.0) * smoothstep(0.3, 0.08, dist);

    // Combine base glow with ray variations
    let glow_intensity = ray_base * ray_variation + streaks * 0.4;

    // Smooth falloff
    let falloff = pow(max(moon_dot - 0.9, 0.0) / 0.1, 0.5);

    return glow_intensity * falloff * 0.6;
}

// Render the moon with proper phase illumination
fn render_moon(rd: vec3<f32>) -> vec4<f32> {
    if sky.moon_enabled == 0u {
        return vec4<f32>(0.0);
    }

    let moon_dir = get_moon_direction();

    // Check if ray is pointing toward moon
    let moon_dot = dot(rd, moon_dir);

    // Moon disc (smaller and sharper than sun)
    let moon_angular_size = sky.moon_size;
    let moon_disc = smoothstep(1.0 - moon_angular_size, 1.0 - moon_angular_size * 0.8, moon_dot);

    // Calculate Perlin-based glow (even if moon disc not visible for halo effect)
    let perlin_glow = perlin_moon_glow(rd, moon_dir, moon_dot);

    if moon_disc < 0.01 && perlin_glow < 0.01 {
        return vec4<f32>(0.0);
    }

    // Calculate phase illumination
    // We need to determine which part of the moon is illuminated
    let illumination = get_moon_illumination();

    // Create a local coordinate system on the moon's face
    let up = vec3<f32>(0.0, 1.0, 0.0);
    let moon_right = normalize(cross(up, moon_dir));
    let moon_up = cross(moon_dir, moon_right);

    // Project ray onto moon's local coordinates
    let local_x = dot(rd - moon_dir * moon_dot, moon_right);
    let local_y = dot(rd - moon_dir * moon_dot, moon_up);

    // Normalize to moon's apparent size
    let normalized_x = local_x / moon_angular_size * 2.0;
    let normalized_y = local_y / moon_angular_size * 2.0;

    // Distance from moon center (for disc shape)
    let dist_from_center = sqrt(normalized_x * normalized_x + normalized_y * normalized_y);

    // Create the phase shadow
    // The terminator (shadow line) moves across the moon
    // illumination: -1 = new moon (shadow on right), 0 = quarters, 1 = full moon (no shadow)

    var phase_shadow = 1.0;

    if abs(illumination) < 0.99 {
        // Calculate terminator position
        // At new moon (illumination = -1), shadow covers from x = -1 to x = 1 (full shadow)
        // At full moon (illumination = 1), no shadow
        // At first quarter (illumination = 0), shadow covers x > 0 (right half dark)

        // The terminator is an ellipse that changes shape with phase
        let terminator_x = -illumination;  // Moves from +1 (new) to -1 (full)

        if illumination < 0.0 {
            // Waxing phases (new moon to full moon) - shadow on the left
            let shadow_edge = terminator_x;
            // Elliptical terminator based on spherical projection
            let terminator_curve = sqrt(max(0.0, 1.0 - normalized_y * normalized_y)) * illumination;
            phase_shadow = smoothstep(terminator_curve - 0.1, terminator_curve + 0.1, normalized_x);
        } else {
            // Waning phases (full moon to new moon) - shadow on the right
            let shadow_edge = terminator_x;
            let terminator_curve = sqrt(max(0.0, 1.0 - normalized_y * normalized_y)) * illumination;
            phase_shadow = smoothstep(terminator_curve + 0.1, terminator_curve - 0.1, normalized_x);
        }
    }

    // Moon surface features (subtle crater-like variations)
    let crater_noise = noise3d(rd * 50.0) * 0.15 + 0.85;
    let mare_noise = noise3d(rd * 15.0 + vec3<f32>(100.0, 0.0, 0.0));
    let mare = smoothstep(0.4, 0.6, mare_noise) * 0.15;  // Dark "seas"

    // Combine moon features
    let surface_brightness = (crater_noise - mare) * phase_shadow;

    // Moon color varies slightly with phase (redder during eclipses, bluer at full)
    var moon_color = sky.moon_color.rgb;

    // Earthshine on the dark side (faint illumination from Earth reflection)
    let earthshine = (1.0 - phase_shadow) * 0.03;

    // Final moon brightness based on phase
    // Full moon is brightest, new moon is nearly invisible
    let phase_brightness = max(abs(illumination) * 0.8 + 0.2, 0.0) * sky.moon_strength;

    // Sharp disc edge
    let disc_edge = smoothstep(1.0, 0.95, dist_from_center);

    // Combine everything
    let moon_brightness = (surface_brightness * phase_brightness + earthshine) * disc_edge * moon_disc;

    // Perlin noise-based glow (smooth flowing rays instead of cheap circle)
    let glow_color = vec3<f32>(0.8, 0.85, 0.95);  // Slightly blue-white glow
    let glow = perlin_glow * sky.moon_strength * phase_brightness;

    return vec4<f32>(moon_color * moon_brightness + glow_color * glow, max(moon_disc, perlin_glow * 0.5));
}

// Interpolate sky colors based on time of day
fn get_sky_colors() -> array<vec4<f32>, 2> {
    var horizon: vec4<f32>;
    var zenith: vec4<f32>;

    // Blend between day/sunset/night based on time_of_day
    // 0.0-0.1: sunrise, 0.1-0.4: day, 0.4-0.6: sunset, 0.6-1.0: night

    if sky.time_of_day < 0.1 {
        // Sunrise (blend night -> day through sunset colors)
        let t = sky.time_of_day / 0.1;
        horizon = mix(sky.night_horizon, sky.sunset_horizon, t);
        zenith = mix(sky.night_zenith, sky.sunset_zenith, t);
    } else if sky.time_of_day < 0.15 {
        // Morning transition
        let t = (sky.time_of_day - 0.1) / 0.05;
        horizon = mix(sky.sunset_horizon, sky.day_horizon, t);
        zenith = mix(sky.sunset_zenith, sky.day_zenith, t);
    } else if sky.time_of_day < 0.4 {
        // Day
        horizon = sky.day_horizon;
        zenith = sky.day_zenith;
    } else if sky.time_of_day < 0.5 {
        // Sunset approach
        let t = (sky.time_of_day - 0.4) / 0.1;
        horizon = mix(sky.day_horizon, sky.sunset_horizon, t);
        zenith = mix(sky.day_zenith, sky.sunset_zenith, t);
    } else if sky.time_of_day < 0.6 {
        // Sunset to night
        let t = (sky.time_of_day - 0.5) / 0.1;
        horizon = mix(sky.sunset_horizon, sky.night_horizon, t);
        zenith = mix(sky.sunset_zenith, sky.night_zenith, t);
    } else {
        // Night
        horizon = sky.night_horizon;
        zenith = sky.night_zenith;
    }

    return array<vec4<f32>, 2>(horizon, zenith);
}

fn render_gradient(rd: vec3<f32>) -> vec3<f32> {
    let colors = get_sky_colors();
    let t = clamp(rd.y * 0.5 + 0.5, 0.0, 1.0);

    // Smooth gradient from horizon to zenith
    return mix(colors[0].rgb, colors[1].rgb, pow(t, 0.7));
}

// Render stars (visible at night)
fn render_stars(rd: vec3<f32>) -> f32 {
    if sky.stars_enabled == 0u {
        return 0.0;
    }

    // Rotate star field slowly
    let rotation = sky.elapsed_time * 0.01;
    let c = cos(rotation);
    let s = sin(rotation);
    let rotated_dir = vec3<f32>(
        rd.x * c - rd.z * s,
        rd.y,
        rd.x * s + rd.z * c
    );

    // Sample star noise using voronoi for sharp star points
    let star_sample = rotated_dir * sky.stars_density;
    var star_noise = 1.0 - voronoi3d(star_sample);

    // Mask with regular noise for variation
    let mask = noise3d(rotated_dir * sky.stars_density * 0.5);
    star_noise *= (1.0 - smoothstep(0.4, 1.0, mask));

    // Add twinkling
    let blink_offset = hash31(floor(star_sample));
    let blink = cos(sky.elapsed_time * sky.stars_blink_speed + blink_offset * 6.28) * 0.5 + 0.5;
    let blink_threshold = blink * 0.1;

    // Threshold to create discrete stars
    let star_intensity = smoothstep(sky.stars_threshold + blink_threshold, 1.0, star_noise);

    return star_intensity;
}

// Aurora stripe helper
fn make_aurora_stripe(x: f32, half_size: f32) -> f32 {
    let base_value = fract(x);
    let left = smoothstep(0.5 - half_size, 0.5, base_value);
    let right = smoothstep(0.5 + half_size, 0.5, base_value);
    return left * right;
}

// Render aurora borealis (visible at night, northern sky)
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

        // Flow distortion
        let flow_sample = world_pos * 0.1;
        let flow = vec2<f32>(
            noise3d(vec3<f32>(flow_sample.x, flow_sample.y, flow_time)),
            noise3d(vec3<f32>(flow_sample.x + 100.0, flow_sample.y, flow_time))
        );
        let flow_dir = normalize(flow);

        // Wiggle distortion
        let wiggle_sample = world_pos * 0.3;
        let wiggle = vec2<f32>(
            noise3d(vec3<f32>(wiggle_sample.x, wiggle_sample.y, flow_time * 2.0)),
            noise3d(vec3<f32>(wiggle_sample.x + 50.0, wiggle_sample.y, flow_time * 2.0))
        ) * 2.0;

        let warped_pos = world_pos + flow_dir * 3.0 + wiggle + vec2<f32>(flow_time * 0.5, 0.0);

        // Create bands
        let large_bands = make_aurora_stripe(warped_pos.x * 0.1, 0.2);
        let small_bands = make_aurora_stripe(warped_pos.x * 0.17, 0.1);
        let base_bands = pow(max(large_bands, small_bands), 3.0);

        // Vertical intensity (bright at bottom, fade at top)
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

    // Fade by view angle (only in upper hemisphere)
    let up_factor = smoothstep(0.05, 0.7, rd.y);
    let final_alpha = accumulated_alpha * up_factor * sky.aurora_intensity;

    return vec4<f32>(accumulated_color * up_factor * sky.aurora_intensity, final_alpha);
}

// ============================================================================
// PERLIN NOISE VOLUMETRIC CLOUDS - Using aurora-like ray marching technique
// ============================================================================

// Ken Perlin's classic permutation-based gradient noise
// Uses hash function to simulate the 256-element permutation table
fn perlin_hash(p: vec3<i32>) -> u32 {
    // Classic Perlin permutation table simulation using hashing
    let permutation = array<u32, 16>(
        151u, 160u, 137u, 91u, 90u, 15u, 131u, 13u,
        201u, 95u, 96u, 53u, 194u, 233u, 7u, 225u
    );
    var h = u32(p.x & 255);
    h = permutation[h & 15u] ^ u32(p.y & 255);
    h = permutation[h & 15u] ^ u32(p.z & 255);
    return h;
}

// Quintic fade curve for smooth interpolation (improved over cubic)
fn perlin_fade(t: f32) -> f32 {
    return t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
}

// 3D gradient selection based on hash
fn perlin_gradient(hash: u32, p: vec3<f32>) -> f32 {
    let h = hash & 15u;
    let u = select(p.y, p.x, h < 8u);
    let v = select(select(p.x, p.z, h == 12u || h == 14u), p.y, h < 4u);
    return select(-u, u, (h & 1u) == 0u) + select(-v, v, (h & 2u) == 0u);
}

// Classic 3D Perlin noise
fn perlin_noise_3d(p: vec3<f32>) -> f32 {
    // Integer and fractional parts
    let pi = vec3<i32>(floor(p));
    let pf = fract(p);

    // Quintic fade for smooth interpolation
    let u = vec3<f32>(perlin_fade(pf.x), perlin_fade(pf.y), perlin_fade(pf.z));

    // Hash coordinates of cube corners
    let n000 = perlin_gradient(perlin_hash(pi + vec3<i32>(0, 0, 0)), pf - vec3<f32>(0.0, 0.0, 0.0));
    let n100 = perlin_gradient(perlin_hash(pi + vec3<i32>(1, 0, 0)), pf - vec3<f32>(1.0, 0.0, 0.0));
    let n010 = perlin_gradient(perlin_hash(pi + vec3<i32>(0, 1, 0)), pf - vec3<f32>(0.0, 1.0, 0.0));
    let n110 = perlin_gradient(perlin_hash(pi + vec3<i32>(1, 1, 0)), pf - vec3<f32>(1.0, 1.0, 0.0));
    let n001 = perlin_gradient(perlin_hash(pi + vec3<i32>(0, 0, 1)), pf - vec3<f32>(0.0, 0.0, 1.0));
    let n101 = perlin_gradient(perlin_hash(pi + vec3<i32>(1, 0, 1)), pf - vec3<f32>(1.0, 0.0, 1.0));
    let n011 = perlin_gradient(perlin_hash(pi + vec3<i32>(0, 1, 1)), pf - vec3<f32>(0.0, 1.0, 1.0));
    let n111 = perlin_gradient(perlin_hash(pi + vec3<i32>(1, 1, 1)), pf - vec3<f32>(1.0, 1.0, 1.0));

    // Trilinear interpolation
    let nx00 = mix(n000, n100, u.x);
    let nx10 = mix(n010, n110, u.x);
    let nx01 = mix(n001, n101, u.x);
    let nx11 = mix(n011, n111, u.x);
    let nxy0 = mix(nx00, nx10, u.y);
    let nxy1 = mix(nx01, nx11, u.y);

    return mix(nxy0, nxy1, u.z);
}

// Fractal Brownian Motion (FBM) using Perlin noise for realistic cloud detail
fn cloud_fbm(p: vec3<f32>, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var total_amplitude = 0.0;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * perlin_noise_3d(p * frequency);
        total_amplitude += amplitude;
        amplitude *= 0.5;  // Persistence
        frequency *= 2.0;  // Lacunarity
    }

    return value / total_amplitude;
}

// Calculate night visibility (0 = day, 1 = full night)
// Defined before render_clouds because WGSL requires functions to be defined before use
fn get_night_visibility() -> f32 {
    // time_of_day: 0.0 = sunrise, 0.25 = noon, 0.5 = sunset, 0.75 = midnight
    if sky.time_of_day >= 0.55 || sky.time_of_day < 0.1 {
        // Night time
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

// Render volumetric clouds using ray marching (similar to aurora technique)
// US-025: Day/Night Cloud Color Variation
fn render_clouds(rd: vec3<f32>) -> vec4<f32> {
    // Skip if looking below horizon or cloud coverage is zero
    if rd.y < 0.02 || sky.cloud_coverage < 0.01 {
        return vec4<f32>(0.0);
    }

    // Cloud layer parameters
    let cloud_base_height = sky.cloud_height;
    let cloud_top_height = cloud_base_height + sky.cloud_thickness;

    // Wind animation
    let wind_time = sky.elapsed_time * sky.cloud_speed;
    let wind_offset = vec2<f32>(
        cos(sky.wind_direction * 6.28318) * wind_time,
        sin(sky.wind_direction * 6.28318) * wind_time
    );

    // Ray march through cloud layer
    let num_samples = 12;
    var accumulated_color = vec3<f32>(0.0);
    var accumulated_alpha = 0.0;

    // Get sun direction for lighting
    let sun_dir = get_sun_direction();
    let night_visibility = get_night_visibility();

    // ==========================================================================
    // DAY/NIGHT CLOUD COLOR VARIATION (US-025)
    // ==========================================================================
    // Calculate sun brightness factor for cloud illumination
    let sun_height = sun_dir.y;
    let sun_brightness = smoothstep(-0.2, 0.4, sun_height);

    // BASE CLOUD COLORS - varies by time of day
    // 1. DAYTIME CLOUDS: White/light gray with subtle sun-tinting
    let day_lit_color = vec3<f32>(1.0, 0.98, 0.96);
    let day_shadow_color = vec3<f32>(0.65, 0.70, 0.80);

    // 2. SUNSET/SUNRISE CLOUDS: Orange/pink dramatic coloring
    let sunset_lit_color = vec3<f32>(1.0, 0.65, 0.45);
    let sunset_shadow_color = vec3<f32>(0.85, 0.35, 0.35);

    // 3. NIGHT CLOUDS: Very faint, barely visible (don't obscure stars)
    let night_lit_color = vec3<f32>(0.12, 0.13, 0.18);
    let night_shadow_color = vec3<f32>(0.05, 0.05, 0.08);

    // TIME-BASED BLENDING - smooth transitions between phases
    let sunrise_factor = smoothstep(0.0, 0.05, sky.time_of_day)
                       * smoothstep(0.18, 0.08, sky.time_of_day);
    let day_factor = smoothstep(0.12, 0.18, sky.time_of_day)
                   * smoothstep(0.42, 0.38, sky.time_of_day);
    let sunset_factor = smoothstep(0.38, 0.44, sky.time_of_day)
                      * smoothstep(0.60, 0.52, sky.time_of_day);
    let night_early = smoothstep(0.55, 0.65, sky.time_of_day);
    let night_late = 1.0 - smoothstep(0.0, 0.08, sky.time_of_day);
    let night_factor = max(night_early, select(0.0, night_late, sky.time_of_day < 0.1));
    let golden_factor = max(sunrise_factor, sunset_factor);

    // BLEND CLOUD COLORS based on time phases
    var cloud_lit_color = day_lit_color;
    var cloud_shadow_color = day_shadow_color;

    // Apply sunset/sunrise golden hour coloring (orange/pink)
    cloud_lit_color = mix(cloud_lit_color, sunset_lit_color, golden_factor * 0.85);
    cloud_shadow_color = mix(cloud_shadow_color, sunset_shadow_color, golden_factor * 0.7);

    // Apply night coloring (very faint)
    cloud_lit_color = mix(cloud_lit_color, night_lit_color, night_factor);
    cloud_shadow_color = mix(cloud_shadow_color, night_shadow_color, night_factor);

    // SUN-TINTED HIGHLIGHTS for daytime clouds
    let sun_tint = vec3<f32>(1.0, 0.95, 0.88);
    let sun_tint_strength = day_factor * sun_brightness * 0.15;
    cloud_lit_color = mix(cloud_lit_color, cloud_lit_color * sun_tint, sun_tint_strength);

    // BRIGHTNESS SCALING based on sun position
    let brightness_scale = mix(0.08, 1.0, sun_brightness);
    cloud_lit_color *= brightness_scale;
    cloud_shadow_color *= mix(0.15, 1.0, sun_brightness);

    // Temperature and weather affect cloud color
    cloud_lit_color = mix(cloud_lit_color, vec3<f32>(0.95, 0.95, 1.0), max(-sky.temperature, 0.0) * 0.2);
    cloud_shadow_color = mix(cloud_shadow_color, vec3<f32>(0.4, 0.4, 0.5), f32(sky.weather_type) * 0.05);

    for (var i = 0; i < num_samples; i++) {
        let height_factor = f32(i) / f32(num_samples - 1);
        let sample_height = mix(cloud_base_height, cloud_top_height, height_factor);

        // Calculate ray intersection with this height plane
        let t = sample_height / max(rd.y, 0.001);
        let world_pos = rd * t;

        // Sample position with wind movement
        let sample_pos = vec3<f32>(
            world_pos.x * sky.cloud_scale + wind_offset.x,
            height_factor * 2.0,
            world_pos.z * sky.cloud_scale + wind_offset.y
        );

        // Multi-octave Perlin FBM for cloud shape
        let cloud_noise = cloud_fbm(sample_pos * 0.3, 5);

        // Add detail noise at different scales
        let detail_noise = cloud_fbm(sample_pos * 1.5 + vec3<f32>(100.0, 0.0, 0.0), 3) * 0.3;
        let wisp_noise = cloud_fbm(sample_pos * 3.0 + vec3<f32>(0.0, 50.0, 0.0), 2) * 0.15;

        // Combine noise layers
        var cloud_density = (cloud_noise + detail_noise + wisp_noise) * 0.5 + 0.5;

        // Apply coverage threshold with sharpness control
        let coverage_threshold = 1.0 - sky.cloud_coverage;
        cloud_density = smoothstep(coverage_threshold, coverage_threshold + sky.cloud_sharpness, cloud_density);

        // Density multiplier from settings
        cloud_density *= sky.cloud_density;

        // Vertical density profile (thicker in middle, thin at edges)
        let vertical_profile = smoothstep(0.0, 0.3, height_factor) * smoothstep(1.0, 0.7, height_factor);
        cloud_density *= vertical_profile;

        // Skip if no cloud here
        if cloud_density < 0.01 {
            continue;
        }

        // Simple self-shadowing using light direction
        let light_sample_pos = sample_pos + sun_dir.xzy * 0.5;
        let shadow_noise = cloud_fbm(light_sample_pos * 0.3, 3);
        let self_shadow = 1.0 - smoothstep(0.3, 0.7, shadow_noise) * 0.6;

        // Calculate cloud lighting (lit from sun direction)
        let light_dot = dot(normalize(vec3<f32>(rd.x, 0.0, rd.z)), sun_dir) * 0.5 + 0.5;
        let lit_factor = mix(0.3, 1.0, light_dot) * self_shadow;

        // Silver lining effect (bright edges when backlit) - only visible during day/sunset
        let backlit = pow(max(dot(rd, -sun_dir), 0.0), 4.0);
        let silver_lining = backlit * 0.4 * sun_brightness;

        // Combine lit and shadow colors
        var sample_color = mix(cloud_shadow_color, cloud_lit_color, lit_factor);
        sample_color += vec3<f32>(1.0, 0.95, 0.9) * silver_lining;

        // Accumulate using front-to-back compositing
        let sample_alpha = cloud_density * 0.15;
        let sample_weight = sample_alpha * (1.0 - accumulated_alpha);

        accumulated_color += sample_color * sample_weight;
        accumulated_alpha += sample_alpha * (1.0 - accumulated_alpha);

        // Early exit if nearly opaque
        if accumulated_alpha > 0.95 {
            break;
        }
    }

    // Fade clouds at horizon for natural look
    let horizon_fade = smoothstep(0.02, 0.15, rd.y);
    accumulated_alpha *= horizon_fade;

    // Distance fade (far clouds are more transparent)
    let distance_fade = smoothstep(0.05, 0.25, rd.y);
    accumulated_alpha *= distance_fade;

    // NIGHT CLOUD OPACITY REDUCTION (US-025)
    // Make night clouds very faint so they don't obscure stars
    let night_opacity_scale = mix(0.10, 1.0, sun_brightness);
    accumulated_alpha *= night_opacity_scale;

    return vec4<f32>(accumulated_color, accumulated_alpha);
}

fn render_skybox(rd: vec3<f32>) -> vec3<f32> {
    let night_visibility = get_night_visibility();
    let day_visibility = max(1.0 - night_visibility, 0.05);

    // Base gradient
    var color = render_gradient(rd);

    // Add sun (brighter during day)
    let sun = render_sun(rd);
    color += sun.rgb * day_visibility;

    // Add volumetric Perlin noise clouds (visible day and night)
    if rd.y > 0.0 {
        let clouds = render_clouds(rd);
        color = mix(color, clouds.rgb, clouds.a);
    }

    // Add moon (visible at night, but can also be visible during day near sunset/sunrise)
    // Moon is most visible at night but can be seen during twilight
    let moon_visibility = smoothstep(0.3, 0.7, night_visibility) + 0.1; // Some visibility even during day
    if rd.y > -0.1 {
        let moon = render_moon(rd);
        // Moon is less visible during day due to sky brightness
        let moon_day_fade = mix(0.15, 1.0, night_visibility);
        color += moon.rgb * moon_visibility * moon_day_fade;
    }

    // Add stars (visible at night only, and only above horizon)
    if rd.y > 0.0 {
        let stars = render_stars(rd);
        color += vec3<f32>(stars) * night_visibility;

        // Add aurora (visible at night, northern sky)
        let aurora = render_aurora(rd);
        color = mix(color, color + aurora.rgb, aurora.a * night_visibility);
    }

    // Horizon haze (subtle) - affected by humidity, can be toggled with K key
    if sky.haze_enabled == 1u {
        let haze_intensity = 0.3 + sky.humidity * 0.2;
        let horizon_haze = exp(-abs(rd.y) * 6.0);
        let haze_color = mix(vec3<f32>(0.7, 0.75, 0.8), vec3<f32>(0.15, 0.15, 0.2), night_visibility);
        color = mix(color, haze_color, horizon_haze * haze_intensity);
    }

    // Below horizon - ground reflection
    if rd.y < 0.0 {
        let ground_color = mix(vec3<f32>(0.3, 0.35, 0.3), vec3<f32>(0.1, 0.1, 0.15), night_visibility);
        let ground_blend = smoothstep(0.0, -0.2, rd.y);
        color = mix(color, ground_color, ground_blend);
    }

    return color;
}

// Sample sky: use pre-baked cubemap when available, otherwise fall back to procedural
fn sample_sky(rd: vec3<f32>) -> vec3<f32> {
    if uniforms.use_sky_cubemap == 1u {
        return textureSample(sky_cubemap, sky_sampler, rd).rgb;
    }
    return render_skybox(rd);
}

// ============================================================================
// DYNAMIC PLACED ENTITIES
// ============================================================================

fn unpack_color(packed: u32) -> vec3<f32> {
    let r = f32((packed >> 16u) & 0xFFu) / 255.0;
    let g = f32((packed >> 8u) & 0xFFu) / 255.0;
    let b = f32(packed & 0xFFu) / 255.0;
    return vec3<f32>(r, g, b);
}

fn sdf_placed_entity(p: vec3<f32>, entity: PlacedEntity) -> f32 {
    let local_p = (p - entity.position) / entity.scale;

    switch entity.entity_type {
        case 0u: {
            // Sphere
            return sdf_sphere(local_p, 1.0) * min(entity.scale.x, min(entity.scale.y, entity.scale.z));
        }
        case 1u: {
            // Box
            return sdf_box(local_p, vec3<f32>(1.0)) * min(entity.scale.x, min(entity.scale.y, entity.scale.z));
        }
        case 2u: {
            // Capsule (vertical)
            return sdf_capsule(local_p, vec3<f32>(0.0, -0.5, 0.0), vec3<f32>(0.0, 0.5, 0.0), 0.5) * min(entity.scale.x, min(entity.scale.y, entity.scale.z));
        }
        case 3u: {
            // Torus
            return sdf_torus(local_p, vec2<f32>(0.7, 0.3)) * min(entity.scale.x, min(entity.scale.y, entity.scale.z));
        }
        case 4u: {
            // Cylinder
            return sdf_cylinder(local_p, 1.0, 0.5) * min(entity.scale.x, min(entity.scale.y, entity.scale.z));
        }
        default: {
            return sdf_sphere(local_p, 1.0) * min(entity.scale.x, min(entity.scale.y, entity.scale.z));
        }
    }
}

// ============================================================================
// HUMAN FIGURE SDF - Built from primitives
// ============================================================================

fn sdf_human(p: vec3<f32>) -> f32 {
    // Human figure at origin, standing upright
    // Scale: ~1.7m tall human

    var d = 1000.0;

    // --- HEAD ---
    // Main head (ellipsoid)
    let head_pos = p - vec3<f32>(0.0, 1.55, 0.0);
    let head = sdf_ellipsoid(head_pos, vec3<f32>(0.1, 0.12, 0.1));
    d = head;

    // --- NECK ---
    let neck = sdf_capsule(p,
        vec3<f32>(0.0, 1.35, 0.0),
        vec3<f32>(0.0, 1.45, 0.0),
        0.04);
    d = smin(d, neck, 0.05);

    // --- TORSO ---
    // Upper torso (chest)
    let chest_pos = p - vec3<f32>(0.0, 1.15, 0.0);
    let chest = sdf_ellipsoid(chest_pos, vec3<f32>(0.18, 0.2, 0.1));
    d = smin(d, chest, 0.08);

    // Lower torso (abdomen)
    let abdomen_pos = p - vec3<f32>(0.0, 0.9, 0.0);
    let abdomen = sdf_ellipsoid(abdomen_pos, vec3<f32>(0.14, 0.12, 0.08));
    d = smin(d, abdomen, 0.1);

    // Hips
    let hips_pos = p - vec3<f32>(0.0, 0.75, 0.0);
    let hips = sdf_ellipsoid(hips_pos, vec3<f32>(0.16, 0.1, 0.1));
    d = smin(d, hips, 0.08);

    // --- ARMS ---
    // Left arm - upper
    let l_upper_arm = sdf_capsule(p,
        vec3<f32>(-0.22, 1.25, 0.0),
        vec3<f32>(-0.3, 0.95, 0.0),
        0.045);
    d = smin(d, l_upper_arm, 0.04);

    // Left arm - forearm
    let l_forearm = sdf_capsule(p,
        vec3<f32>(-0.3, 0.95, 0.0),
        vec3<f32>(-0.35, 0.65, 0.0),
        0.035);
    d = smin(d, l_forearm, 0.03);

    // Left hand
    let l_hand_pos = p - vec3<f32>(-0.37, 0.58, 0.0);
    let l_hand = sdf_ellipsoid(l_hand_pos, vec3<f32>(0.025, 0.045, 0.02));
    d = smin(d, l_hand, 0.02);

    // Right arm - upper
    let r_upper_arm = sdf_capsule(p,
        vec3<f32>(0.22, 1.25, 0.0),
        vec3<f32>(0.3, 0.95, 0.0),
        0.045);
    d = smin(d, r_upper_arm, 0.04);

    // Right arm - forearm
    let r_forearm = sdf_capsule(p,
        vec3<f32>(0.3, 0.95, 0.0),
        vec3<f32>(0.35, 0.65, 0.0),
        0.035);
    d = smin(d, r_forearm, 0.03);

    // Right hand
    let r_hand_pos = p - vec3<f32>(0.37, 0.58, 0.0);
    let r_hand = sdf_ellipsoid(r_hand_pos, vec3<f32>(0.025, 0.045, 0.02));
    d = smin(d, r_hand, 0.02);

    // --- LEGS ---
    // Left leg - thigh
    let l_thigh = sdf_capsule(p,
        vec3<f32>(-0.1, 0.72, 0.0),
        vec3<f32>(-0.12, 0.4, 0.0),
        0.065);
    d = smin(d, l_thigh, 0.05);

    // Left leg - shin
    let l_shin = sdf_capsule(p,
        vec3<f32>(-0.12, 0.4, 0.0),
        vec3<f32>(-0.12, 0.08, 0.0),
        0.045);
    d = smin(d, l_shin, 0.04);

    // Left foot
    let l_foot_pos = p - vec3<f32>(-0.12, 0.04, 0.04);
    let l_foot = sdf_rounded_box(l_foot_pos, vec3<f32>(0.04, 0.03, 0.09), 0.015);
    d = smin(d, l_foot, 0.02);

    // Right leg - thigh
    let r_thigh = sdf_capsule(p,
        vec3<f32>(0.1, 0.72, 0.0),
        vec3<f32>(0.12, 0.4, 0.0),
        0.065);
    d = smin(d, r_thigh, 0.05);

    // Right leg - shin
    let r_shin = sdf_capsule(p,
        vec3<f32>(0.12, 0.4, 0.0),
        vec3<f32>(0.12, 0.08, 0.0),
        0.045);
    d = smin(d, r_shin, 0.04);

    // Right foot
    let r_foot_pos = p - vec3<f32>(0.12, 0.04, 0.04);
    let r_foot = sdf_rounded_box(r_foot_pos, vec3<f32>(0.04, 0.03, 0.09), 0.015);
    d = smin(d, r_foot, 0.02);

    return d;
}

// ============================================================================
// FIRST-PERSON BODY SDF - Excludes head to avoid z-fighting with camera
// ============================================================================
// This is used when in first-person mode and looking down (pitch < -30°)
// Shows body, arms, and legs but NOT head/neck to avoid camera clipping

fn sdf_human_body_only(p: vec3<f32>) -> f32 {
    // Human figure at origin, standing upright
    // Scale: ~1.7m tall human
    // EXCLUDES: Head and neck (camera is at head position)

    var d = 1000.0;

    // --- TORSO ---
    // Upper torso (chest)
    let chest_pos = p - vec3<f32>(0.0, 1.15, 0.0);
    let chest = sdf_ellipsoid(chest_pos, vec3<f32>(0.18, 0.2, 0.1));
    d = chest;

    // Lower torso (abdomen)
    let abdomen_pos = p - vec3<f32>(0.0, 0.9, 0.0);
    let abdomen = sdf_ellipsoid(abdomen_pos, vec3<f32>(0.14, 0.12, 0.08));
    d = smin(d, abdomen, 0.1);

    // Hips
    let hips_pos = p - vec3<f32>(0.0, 0.75, 0.0);
    let hips = sdf_ellipsoid(hips_pos, vec3<f32>(0.16, 0.1, 0.1));
    d = smin(d, hips, 0.08);

    // --- ARMS ---
    // Left arm - upper
    let l_upper_arm = sdf_capsule(p,
        vec3<f32>(-0.22, 1.25, 0.0),
        vec3<f32>(-0.3, 0.95, 0.0),
        0.045);
    d = smin(d, l_upper_arm, 0.04);

    // Left arm - forearm
    let l_forearm = sdf_capsule(p,
        vec3<f32>(-0.3, 0.95, 0.0),
        vec3<f32>(-0.35, 0.65, 0.0),
        0.035);
    d = smin(d, l_forearm, 0.03);

    // Left hand
    let l_hand_pos = p - vec3<f32>(-0.37, 0.58, 0.0);
    let l_hand = sdf_ellipsoid(l_hand_pos, vec3<f32>(0.025, 0.045, 0.02));
    d = smin(d, l_hand, 0.02);

    // Right arm - upper
    let r_upper_arm = sdf_capsule(p,
        vec3<f32>(0.22, 1.25, 0.0),
        vec3<f32>(0.3, 0.95, 0.0),
        0.045);
    d = smin(d, r_upper_arm, 0.04);

    // Right arm - forearm
    let r_forearm = sdf_capsule(p,
        vec3<f32>(0.3, 0.95, 0.0),
        vec3<f32>(0.35, 0.65, 0.0),
        0.035);
    d = smin(d, r_forearm, 0.03);

    // Right hand
    let r_hand_pos = p - vec3<f32>(0.37, 0.58, 0.0);
    let r_hand = sdf_ellipsoid(r_hand_pos, vec3<f32>(0.025, 0.045, 0.02));
    d = smin(d, r_hand, 0.02);

    // --- LEGS ---
    // Left leg - thigh
    let l_thigh = sdf_capsule(p,
        vec3<f32>(-0.1, 0.72, 0.0),
        vec3<f32>(-0.12, 0.4, 0.0),
        0.065);
    d = smin(d, l_thigh, 0.05);

    // Left leg - shin
    let l_shin = sdf_capsule(p,
        vec3<f32>(-0.12, 0.4, 0.0),
        vec3<f32>(-0.12, 0.08, 0.0),
        0.045);
    d = smin(d, l_shin, 0.04);

    // Left foot
    let l_foot_pos = p - vec3<f32>(-0.12, 0.04, 0.04);
    let l_foot = sdf_rounded_box(l_foot_pos, vec3<f32>(0.04, 0.03, 0.09), 0.015);
    d = smin(d, l_foot, 0.02);

    // Right leg - thigh
    let r_thigh = sdf_capsule(p,
        vec3<f32>(0.1, 0.72, 0.0),
        vec3<f32>(0.12, 0.4, 0.0),
        0.065);
    d = smin(d, r_thigh, 0.05);

    // Right leg - shin
    let r_shin = sdf_capsule(p,
        vec3<f32>(0.12, 0.4, 0.0),
        vec3<f32>(0.12, 0.08, 0.0),
        0.045);
    d = smin(d, r_shin, 0.04);

    // Right foot
    let r_foot_pos = p - vec3<f32>(0.12, 0.04, 0.04);
    let r_foot = sdf_rounded_box(r_foot_pos, vec3<f32>(0.04, 0.03, 0.09), 0.015);
    d = smin(d, r_foot, 0.02);

    return d;
}

// Calculate first-person body visibility based on camera pitch
// Returns: visibility factor 0.0 (hidden) to 1.0 (fully visible)
// Body becomes visible when pitch < -30° with smooth fade-in
fn get_first_person_body_visibility() -> f32 {
    // Only visible in first-person mode
    if uniforms.camera_mode != 1u {
        return 0.0;
    }

    // Pitch threshold in radians: -30° = -0.5236 radians
    let PITCH_THRESHOLD: f32 = -0.5236;
    // Fade range: 15° = 0.2618 radians
    let FADE_RANGE: f32 = 0.2618;

    // Calculate visibility based on pitch
    // At -30° pitch: visibility starts (0.0)
    // At -45° pitch: fully visible (1.0)
    if uniforms.camera_pitch >= PITCH_THRESHOLD {
        return 0.0;
    }

    let fade_start = PITCH_THRESHOLD;
    let fade_end = PITCH_THRESHOLD - FADE_RANGE;

    // Smooth fade using smoothstep for natural transition
    return smoothstep(fade_start, fade_end, uniforms.camera_pitch);
}

// ============================================================================
// UNITY-STYLE INFINITE GRID
// ============================================================================

fn grid_pattern(p: vec2<f32>, grid_size: f32) -> f32 {
    let grid = abs(fract(p / grid_size - 0.5) - 0.5) * grid_size;
    return min(grid.x, grid.y);
}

// Calculate shadow on the ground plane from objects
fn ground_shadow(hit_point: vec3<f32>) -> f32 {
    let sun_dir = get_sun_direction();

    // Only calculate shadows if sun is above horizon
    if sun_dir.y <= 0.0 {
        return 1.0;
    }

    // Trace from ground point toward sun
    let shadow_origin = hit_point + vec3<f32>(0.0, 0.001, 0.0);  // Tiny offset above ground
    return soft_shadow(shadow_origin, sun_dir, 0.01, 50.0, 8.0);
}

fn unity_grid(p: vec3<f32>, rd: vec3<f32>, ro: vec3<f32>) -> vec4<f32> {
    // Intersect with Y=0 plane
    if abs(rd.y) < 0.0001 {
        return vec4<f32>(0.0); // Parallel to plane
    }

    let t = -ro.y / rd.y;
    if t < 0.0 {
        return vec4<f32>(0.0); // Behind camera
    }

    let hit_point = ro + rd * t;

    // Distance-based fade
    let dist_to_camera = length(hit_point - ro);
    let fade = 1.0 - clamp(dist_to_camera / 100.0, 0.0, 1.0);  // Longer fade distance

    if fade < 0.01 {
        return vec4<f32>(0.0);
    }

    // Determine if viewing from below (silhouette effect)
    let viewing_from_below = ro.y < 0.0;

    // Multi-scale grid (like Unity) - use dynamic grid size
    let grid_sz = max(uniforms.grid_size, 0.25);  // Minimum grid size
    let small_grid = grid_pattern(hit_point.xz, grid_sz);
    let large_grid = grid_pattern(hit_point.xz, grid_sz * 10.0);

    // Grid line thickness (thinner = smaller values)
    let small_line = 1.0 - smoothstep(0.0, 0.03, small_grid);
    let large_line = 1.0 - smoothstep(0.0, 0.05, large_grid);

    // Axis highlighting (X = red, Z = blue)
    let x_axis = 1.0 - smoothstep(0.0, 0.08, abs(hit_point.z));
    let z_axis = 1.0 - smoothstep(0.0, 0.08, abs(hit_point.x));

    // GROUND SHADOWS from objects!
    let shadow = ground_shadow(hit_point);
    let shadow_darkening = mix(0.3, 1.0, shadow);  // Shadows darken to 30%

    // Base ground color (affected by shadow and time of day)
    let night_visibility = get_night_visibility();
    let ground_base = mix(vec3<f32>(0.25, 0.28, 0.25), vec3<f32>(0.08, 0.08, 0.1), night_visibility);

    // Grid line colors
    var grid_color = vec3<f32>(0.4, 0.4, 0.4) * small_line * 0.4;
    grid_color = max(grid_color, vec3<f32>(0.6, 0.6, 0.6) * large_line * 0.6);
    grid_color = max(grid_color, vec3<f32>(0.9, 0.25, 0.25) * x_axis);
    grid_color = max(grid_color, vec3<f32>(0.25, 0.25, 0.9) * z_axis);

    // Combine base ground with grid, both affected by shadow
    var color = ground_base * shadow_darkening;
    color = max(color, grid_color * shadow_darkening);

    // When viewing from below, add a semi-transparent "underside" effect
    if viewing_from_below {
        // Darker color when viewing from below (silhouette)
        color = color * 0.5 + vec3<f32>(0.1, 0.05, 0.15);  // Slight purple tint
    }

    // Alpha calculation - include shadow visibility
    var alpha = max(small_line * 0.3, max(large_line * 0.5, max(x_axis, z_axis))) * fade;

    // Shadow areas should be more visible (darker ground shows through)
    if shadow < 0.9 {
        alpha = max(alpha, (1.0 - shadow) * 0.6 * fade);  // Shadow areas more opaque
    }

    // Increase alpha when viewing from below for better silhouette visibility
    if viewing_from_below {
        alpha = max(alpha, 0.3 * fade);  // Minimum 30% opacity when below ground
    }

    return vec4<f32>(color, alpha);
}

// ============================================================================
// 3D VOLUME GRID - For building in the air (like Minecraft creative mode)
// ============================================================================

fn volume_grid_3d(p: vec3<f32>, rd: vec3<f32>, ro: vec3<f32>) -> vec4<f32> {
    if uniforms.volume_grid_visible == 0u {
        return vec4<f32>(0.0);
    }

    let grid_sz = uniforms.grid_size;

    // Draw grid lines at placement height
    let height = uniforms.placement_height;

    // Intersect with Y=height plane
    if abs(rd.y) < 0.0001 {
        return vec4<f32>(0.0);
    }

    let t = (height - ro.y) / rd.y;
    if t < 0.0 {
        return vec4<f32>(0.0);
    }

    let hit_point = ro + rd * t;

    // Distance-based fade
    let dist_to_camera = length(hit_point - ro);
    let fade = 1.0 - clamp(dist_to_camera / 50.0, 0.0, 1.0);

    if fade < 0.01 {
        return vec4<f32>(0.0);
    }

    // Grid pattern at this height
    let grid = abs(fract(hit_point.xz / grid_sz - 0.5) - 0.5) * grid_sz;
    let grid_line = min(grid.x, grid.y);

    // Thinner, more transparent lines for volume grid
    let line_intensity = 1.0 - smoothstep(0.0, 0.02, grid_line);

    // Yellow-ish color for volume grid to distinguish from ground
    let grid_color = vec3<f32>(0.8, 0.7, 0.2);

    let alpha = line_intensity * 0.3 * fade;

    return vec4<f32>(grid_color, alpha);
}

// Placement height indicator - shows a faint plane at current placement height
fn placement_height_indicator(rd: vec3<f32>, ro: vec3<f32>) -> vec4<f32> {
    if uniforms.placement_height < 0.01 {
        return vec4<f32>(0.0); // Don't show when at ground level
    }

    let height = uniforms.placement_height;

    // Intersect with Y=height plane
    if abs(rd.y) < 0.0001 {
        return vec4<f32>(0.0);
    }

    let t = (height - ro.y) / rd.y;
    if t < 0.0 {
        return vec4<f32>(0.0);
    }

    let hit_point = ro + rd * t;

    // Circular fade around origin (placement area indicator)
    let dist_from_center = length(hit_point.xz);
    let area_fade = 1.0 - smoothstep(5.0, 15.0, dist_from_center);

    // Distance-based fade
    let dist_to_camera = length(hit_point - ro);
    let cam_fade = 1.0 - clamp(dist_to_camera / 30.0, 0.0, 1.0);

    // Subtle cyan color
    let indicator_color = vec3<f32>(0.3, 0.6, 0.8);

    let alpha = area_fade * cam_fade * 0.1;

    return vec4<f32>(indicator_color, alpha);
}

// ============================================================================
// FIRST-PERSON HANDS SDF (US-019)
// ============================================================================
//
// First-person hands are rendered as capsules positioned relative to the camera.
// They are only visible in first-person mode (camera_mode == 1).
//
// Hand positions in camera space (relative to camera looking direction):
// - Left hand:  (-0.3, -0.3, 0.5) - 30cm left, 30cm down, 50cm forward
// - Right hand: ( 0.3, -0.3, 0.5) - 30cm right, 30cm down, 50cm forward

// First-person hand constants
const FP_HAND_RADIUS: f32 = 0.05;        // Hand sphere radius (5cm - realistic palm width)
const FP_HAND_BOB_AMPLITUDE: f32 = 0.01; // Subtle bob amplitude (1cm)
const FP_HAND_BOB_SPEED: f32 = 2.0;      // Bob animation speed

// Left hand offset in camera space
const FP_LEFT_HAND_OFFSET: vec3<f32> = vec3<f32>(-0.3, -0.3, 0.5);
// Right hand offset in camera space
const FP_RIGHT_HAND_OFFSET: vec3<f32> = vec3<f32>(0.3, -0.3, 0.5);

/// Get camera orientation vectors (forward, right, up) from camera position and target.
fn get_camera_orientation_vectors() -> mat3x3<f32> {
    let forward = normalize(uniforms.camera_target - uniforms.camera_pos);
    let up_world = vec3<f32>(0.0, 1.0, 0.0);

    // Handle edge case when camera is directly above/below target
    var right: vec3<f32>;
    var up: vec3<f32>;

    if abs(dot(forward, up_world)) > 0.99 {
        // Looking straight down or up
        right = vec3<f32>(1.0, 0.0, 0.0);
        up = cross(forward, right);
    } else {
        right = normalize(cross(up_world, forward));
        up = cross(forward, right);
    }

    return mat3x3<f32>(right, up, forward);
}

/// Transform a position from camera space to world space.
fn camera_to_world_fp(camera_local: vec3<f32>) -> vec3<f32> {
    let orientation = get_camera_orientation_vectors();
    return uniforms.camera_pos
         + orientation[0] * camera_local.x  // right
         + orientation[1] * camera_local.y  // up
         + orientation[2] * camera_local.z; // forward
}

/// Calculate idle bob offset for hands.
fn hand_bob_offset_fp(time: f32, is_left: bool) -> f32 {
    let phase = select(0.0, 0.5, is_left);
    return sin((time + phase) * FP_HAND_BOB_SPEED) * FP_HAND_BOB_AMPLITUDE;
}

/// Evaluate first-person hands SDF.
/// Returns distance to nearest hand, or a large value if hands are not visible.
fn evaluate_first_person_hands_fp(p: vec3<f32>) -> f32 {
    // Only render hands in first-person mode
    if uniforms.camera_mode != 1u {
        return 1000.0;
    }

    // Calculate bob offset for subtle idle animation
    let left_bob = hand_bob_offset_fp(uniforms.time, true);
    let right_bob = hand_bob_offset_fp(uniforms.time, false);

    // Calculate hand positions in camera space with bob animation
    let left_cam = FP_LEFT_HAND_OFFSET + vec3<f32>(0.0, left_bob, 0.0);
    let right_cam = FP_RIGHT_HAND_OFFSET + vec3<f32>(0.0, right_bob, 0.0);

    // Transform to world space
    let left_world = camera_to_world_fp(left_cam);
    let right_world = camera_to_world_fp(right_cam);

    // Evaluate SDF for both hands (simple spheres)
    let d_left = length(p - left_world) - FP_HAND_RADIUS;
    let d_right = length(p - right_world) - FP_HAND_RADIUS;

    // Return minimum distance (closest hand)
    return min(d_left, d_right);
}

// ============================================================================
// FROXEL CULLING HELPERS
// ============================================================================

// Near/far planes for froxel depth distribution (must match Rust)
const FROXEL_NEAR: f32 = 0.1;
const FROXEL_FAR: f32 = 1000.0;

// FOV used for froxel projection (must match get_ray_direction call)
const FROXEL_FOV: f32 = 1.2; // ~69 degrees, same as raymarcher

// Compute froxel index for a world-space point. Returns -1 if outside grid.
fn get_froxel_index(p: vec3<f32>) -> i32 {
    // Compute camera orientation from position and target (same as get_camera_orientation)
    var cam_fwd = normalize(uniforms.camera_target - uniforms.camera_pos);
    let up_world = vec3<f32>(0.0, 1.0, 0.0);
    var cam_right: vec3<f32>;
    var cam_up: vec3<f32>;
    if abs(dot(cam_fwd, up_world)) > 0.99 {
        cam_right = vec3<f32>(1.0, 0.0, 0.0);
        cam_up = cross(cam_fwd, cam_right);
    } else {
        cam_right = normalize(cross(up_world, cam_fwd));
        cam_up = cross(cam_fwd, cam_right);
    }

    let rel = p - uniforms.camera_pos;
    // View-space: z = depth along -forward (camera convention), x = right, y = up
    let vz = -dot(rel, cam_fwd);
    let vx = dot(rel, cam_right);
    let vy = dot(rel, cam_up);

    // Check depth range
    if vz < FROXEL_NEAR || vz > FROXEL_FAR {
        return -1;
    }

    // Exponential depth slice: d = near * (far/near)^(slice/total_slices)
    let ratio = FROXEL_FAR / FROXEL_NEAR;
    let t = log(vz / FROXEL_NEAR) / log(ratio);
    let slice_z = u32(clamp(t * f32(FROXEL_DEPTH_SLICES), 0.0, f32(FROXEL_DEPTH_SLICES) - 1.0));

    // Project to NDC using FOV and aspect ratio
    let aspect = uniforms.resolution.x / uniforms.resolution.y;
    let half_fov = tan(FROXEL_FOV * 0.5);

    // In our ray setup: ray_cam = (ndc.x * aspect * half_fov, ndc.y * half_fov, -1.0)
    // So reverse: ndc.x = vx / (vz * aspect * half_fov), ndc.y = vy / (vz * half_fov)
    let ndc_x = vx / (vz * aspect * half_fov);
    let ndc_y = vy / (vz * half_fov);

    // NDC [-1, 1] to UV [0, 1]
    let uv_x = ndc_x * 0.5 + 0.5;
    let uv_y = -ndc_y * 0.5 + 0.5; // flip Y

    if uv_x < 0.0 || uv_x >= 1.0 || uv_y < 0.0 || uv_y >= 1.0 {
        return -1;
    }

    let tile_x = u32(uv_x * f32(FROXEL_TILES_X));
    let tile_y = u32(uv_y * f32(FROXEL_TILES_Y));

    let idx = slice_z * (FROXEL_TILES_X * FROXEL_TILES_Y) + tile_y * FROXEL_TILES_X + tile_x;
    if idx >= TOTAL_FROXELS {
        return -1;
    }
    return i32(idx);
}

// Get the tile index for a screen pixel coordinate
fn get_tile_index(pixel_x: u32, pixel_y: u32) -> i32 {
    let tx = pixel_x / TILE_SIZE;
    let ty = pixel_y / TILE_SIZE;
    if tx >= tile_data.tiles_x || ty >= tile_data.tiles_y {
        return -1;
    }
    return i32(ty * tile_data.tiles_x + tx);
}

// ============================================================================
// SCENE SDF
// ============================================================================

struct HitResult {
    dist: f32,
    material_id: u32,
}

fn scene_sdf(p: vec3<f32>) -> HitResult {
    var result: HitResult;
    result.dist = 1000.0;
    result.material_id = 0u;

    // Human figure visibility logic
    // In third-person mode (camera_mode == 0): show full human at origin
    // In first-person mode (camera_mode == 1): show body only when looking down
    if uniforms.camera_mode == 0u {
        // Third-person mode: show full human figure if visible
        if uniforms.human_visible == 1u {
            let human = sdf_human(p);
            if human < result.dist {
                result.dist = human;
                result.material_id = 10u; // Human material
            }
        }
    } else {
        // First-person mode: show body (no head) when looking down
        let body_visibility = get_first_person_body_visibility();
        if body_visibility > 0.01 {
            // Transform point to player local space
            let local_p = p - uniforms.player_position;
            let body = sdf_human_body_only(local_p);

            // Scale distance by visibility for smooth fade-in effect
            // When visibility is low, effectively push the body further away
            let effective_dist = body / max(body_visibility, 0.1);

            if effective_dist < result.dist {
                result.dist = effective_dist;
                result.material_id = 11u; // First-person body material (different from full human)
            }
        }

        // US-019: First-person hands - always visible in first-person mode
        // Hands are positioned relative to the camera with idle bob animation
        let hands_dist = evaluate_first_person_hands_fp(p);
        if hands_dist < result.dist {
            result.dist = hands_dist;
            result.material_id = 12u; // First-person hands material (skin tone)
        }
    }

    // Test primitives (always visible for reference)
    // Small red sphere at (-3, 0.5, 0)
    let sphere1 = sdf_sphere(p - vec3<f32>(-3.0, 0.5, 0.0), 0.5);
    if sphere1 < result.dist {
        result.dist = sphere1;
        result.material_id = 2u;
    }

    // Green box at (3, 0.5, 0)
    let box1 = sdf_box(p - vec3<f32>(3.0, 0.5, 0.0), vec3<f32>(0.4, 0.4, 0.4));
    if box1 < result.dist {
        result.dist = box1;
        result.material_id = 3u;
    }

    // Blue torus at (0, 0.3, 3)
    let torus_p = p - vec3<f32>(0.0, 0.3, 3.0);
    let torus1 = sdf_torus(torus_p, vec2<f32>(0.5, 0.15));
    if torus1 < result.dist {
        result.dist = torus1;
        result.material_id = 4u;
    }

    // Dynamic placed entities with froxel-based culling
    // Use froxel SDF list if available, otherwise fallback to full iteration
    let froxel_idx = get_froxel_index(p);
    if froxel_idx >= 0 {
        let fi = u32(froxel_idx);
        let list_count = min(froxel_sdf_lists.lists[fi].count, MAX_SDFS_PER_FROXEL);
        if list_count > 0u {
            // Froxel has entities - only evaluate those in this froxel's list
            for (var j = 0u; j < list_count; j++) {
                let i = froxel_sdf_lists.lists[fi].sdf_indices[j];
                if i < placed_entities.count {
                    let entity = placed_entities.entities[i];
                    let entity_dist = sdf_placed_entity(p, entity);
                    if entity_dist < result.dist {
                        result.dist = entity_dist;
                        result.material_id = 100u + i;
                    }
                }
            }
        } else {
            // Froxel list empty - fallback to full entity iteration
            let entity_count = min(placed_entities.count, 64u);
            for (var i = 0u; i < entity_count; i++) {
                let entity = placed_entities.entities[i];
                let entity_dist = sdf_placed_entity(p, entity);
                if entity_dist < result.dist {
                    result.dist = entity_dist;
                    result.material_id = 100u + i;
                }
            }
        }
    } else {
        // Outside froxel grid - fallback to full entity iteration
        let entity_count = min(placed_entities.count, 64u);
        for (var i = 0u; i < entity_count; i++) {
            let entity = placed_entities.entities[i];
            let entity_dist = sdf_placed_entity(p, entity);
            if entity_dist < result.dist {
                result.dist = entity_dist;
                result.material_id = 100u + i;
            }
        }
    }

    return result;
}

fn scene_sdf_dist(p: vec3<f32>) -> f32 {
    return scene_sdf(p).dist;
}

// ============================================================================
// RAY MARCHING
// ============================================================================

const MAX_STEPS: u32 = 200u;
const MAX_DIST: f32 = 100.0;
const SURF_DIST: f32 = 0.0005;

struct RayResult {
    hit: bool,
    position: vec3<f32>,
    distance: f32,
    steps: u32,
    material_id: u32,
}

// Evaluate only entities visible in a specific tile (coarse screen-space pass).
// Used as pre-filter before froxel fine pass in scene_sdf.
fn scene_sdf_tile_culled(p: vec3<f32>, tile_idx: i32) -> HitResult {
    var result: HitResult;
    result.dist = 1000.0;
    result.material_id = 0u;

    // Non-entity geometry (human, primitives) always evaluated
    // Human figure
    if uniforms.camera_mode == 0u {
        if uniforms.human_visible == 1u {
            let human = sdf_human(p);
            if human < result.dist {
                result.dist = human;
                result.material_id = 10u;
            }
        }
    } else {
        let body_visibility = get_first_person_body_visibility();
        if body_visibility > 0.01 {
            let local_p = p - uniforms.player_position;
            let body = sdf_human_body_only(local_p);
            let effective_dist = body / max(body_visibility, 0.1);
            if effective_dist < result.dist {
                result.dist = effective_dist;
                result.material_id = 11u;
            }
        }
        let hands_dist = evaluate_first_person_hands_fp(p);
        if hands_dist < result.dist {
            result.dist = hands_dist;
            result.material_id = 12u;
        }
    }

    // Test primitives
    let sphere1 = sdf_sphere(p - vec3<f32>(-3.0, 0.5, 0.0), 0.5);
    if sphere1 < result.dist { result.dist = sphere1; result.material_id = 2u; }
    let box1 = sdf_box(p - vec3<f32>(3.0, 0.5, 0.0), vec3<f32>(0.4, 0.4, 0.4));
    if box1 < result.dist { result.dist = box1; result.material_id = 3u; }
    let torus_p = p - vec3<f32>(0.0, 0.3, 3.0);
    let torus1 = sdf_torus(torus_p, vec2<f32>(0.5, 0.15));
    if torus1 < result.dist { result.dist = torus1; result.material_id = 4u; }

    // Entity culling: tile coarse pass -> froxel fine pass
    if tile_idx >= 0 {
        let ti = u32(tile_idx);
        let tile_entity_count = min(tile_data.tiles[ti].entity_count, MAX_ENTITIES_PER_TILE);
        if tile_entity_count > 0u {
            // Tile has entities - now refine with froxel lookup
            let froxel_idx = get_froxel_index(p);
            if froxel_idx >= 0 {
                let fi = u32(froxel_idx);
                let list_count = min(froxel_sdf_lists.lists[fi].count, MAX_SDFS_PER_FROXEL);
                if list_count > 0u {
                    // Froxel fine pass: only entities in both tile AND froxel
                    for (var j = 0u; j < list_count; j++) {
                        let i = froxel_sdf_lists.lists[fi].sdf_indices[j];
                        if i < placed_entities.count {
                            let entity = placed_entities.entities[i];
                            let entity_dist = sdf_placed_entity(p, entity);
                            if entity_dist < result.dist {
                                result.dist = entity_dist;
                                result.material_id = 100u + i;
                            }
                        }
                    }
                } else {
                    // Froxel empty but tile has entities - use tile list as fallback
                    for (var j = 0u; j < tile_entity_count; j++) {
                        let i = tile_data.tiles[ti].entity_indices[j];
                        if i < placed_entities.count {
                            let entity = placed_entities.entities[i];
                            let entity_dist = sdf_placed_entity(p, entity);
                            if entity_dist < result.dist {
                                result.dist = entity_dist;
                                result.material_id = 100u + i;
                            }
                        }
                    }
                }
            } else {
                // Outside froxel grid but in tile - use tile list
                for (var j = 0u; j < tile_entity_count; j++) {
                    let i = tile_data.tiles[ti].entity_indices[j];
                    if i < placed_entities.count {
                        let entity = placed_entities.entities[i];
                        let entity_dist = sdf_placed_entity(p, entity);
                        if entity_dist < result.dist {
                            result.dist = entity_dist;
                            result.material_id = 100u + i;
                        }
                    }
                }
            }
        }
        // else: tile has no entities, skip entity evaluation entirely
    } else {
        // No tile info - fallback to full entity iteration
        let entity_count = min(placed_entities.count, 64u);
        for (var i = 0u; i < entity_count; i++) {
            let entity = placed_entities.entities[i];
            let entity_dist = sdf_placed_entity(p, entity);
            if entity_dist < result.dist {
                result.dist = entity_dist;
                result.material_id = 100u + i;
            }
        }
    }

    return result;
}

fn ray_march(ro: vec3<f32>, rd: vec3<f32>) -> RayResult {
    var result: RayResult;
    result.hit = false;
    result.distance = 0.0;
    result.steps = 0u;
    result.material_id = 0u;

    var t = 0.0;
    var prev_froxel_idx: i32 = -2; // Track current froxel to detect boundary crossings

    for (var i = 0u; i < MAX_STEPS; i++) {
        result.steps = i;
        let p = ro + rd * t;

        // Track froxel boundary crossings (recalculate when entering new froxel)
        let cur_froxel = get_froxel_index(p);
        if cur_froxel != prev_froxel_idx {
            prev_froxel_idx = cur_froxel;
            // Froxel changed - scene_sdf will use the new froxel's entity list
        }

        let hit = scene_sdf(p);

        if hit.dist < SURF_DIST {
            result.hit = true;
            result.position = p;
            result.distance = t;
            result.material_id = hit.material_id;
            break;
        }

        if t > MAX_DIST {
            break;
        }

        // Adaptive step size based on distance from camera
        // Close: fine steps for detail. Far: larger steps for efficiency.
        let step_scale = select(0.9, 0.9 + 0.1 * clamp((t - 10.0) / 90.0, 0.0, 1.0), t > 10.0);
        t += hit.dist * step_scale;
    }

    result.distance = t;
    return result;
}

// ============================================================================
// NORMALS AND LIGHTING
// ============================================================================

fn calc_normal(p: vec3<f32>) -> vec3<f32> {
    let e = vec2<f32>(0.0005, 0.0);
    return normalize(vec3<f32>(
        scene_sdf_dist(p + e.xyy) - scene_sdf_dist(p - e.xyy),
        scene_sdf_dist(p + e.yxy) - scene_sdf_dist(p - e.yxy),
        scene_sdf_dist(p + e.yyx) - scene_sdf_dist(p - e.yyx)
    ));
}

fn get_material_color(material_id: u32) -> vec3<f32> {
    switch material_id {
        case 2u:  { return vec3<f32>(0.9, 0.2, 0.2); }   // Red sphere
        case 3u:  { return vec3<f32>(0.2, 0.9, 0.2); }   // Green box
        case 4u:  { return vec3<f32>(0.2, 0.2, 0.9); }   // Blue torus
        case 10u: { return vec3<f32>(0.9, 0.75, 0.6); }  // Human (skin tone)
        case 11u: { return vec3<f32>(0.9, 0.75, 0.6); }  // First-person body (skin tone)
        case 12u: { return vec3<f32>(0.9, 0.75, 0.6); }  // US-019: First-person hands (skin tone)
        default: {
            // Placed entities (100+)
            if material_id >= 100u {
                let entity_idx = material_id - 100u;
                if entity_idx < placed_entities.count {
                    return unpack_color(placed_entities.entities[entity_idx].color_packed);
                }
            }
            return vec3<f32>(1.0, 0.0, 1.0);   // Magenta error
        }
    }
}

// Hard raymarched shadows - for crisp sun shadows
fn hard_shadow(ro: vec3<f32>, rd: vec3<f32>, mint: f32, maxt: f32) -> f32 {
    var t = mint;
    for (var i = 0; i < 64; i++) {
        let p = ro + rd * t;
        let h = scene_sdf_dist(p);
        if h < 0.001 {
            return 0.0;  // In shadow
        }
        t += max(h, 0.01);  // Minimum step to avoid getting stuck
        if t > maxt {
            break;
        }
    }
    return 1.0;  // Lit
}

// Soft shadows for more realistic lighting (penumbra effect)
fn soft_shadow(ro: vec3<f32>, rd: vec3<f32>, mint: f32, maxt: f32, k: f32) -> f32 {
    var res = 1.0;
    var t = mint;
    var ph = 1e10;  // Previous hit distance for improved penumbra

    for (var i = 0; i < 64; i++) {
        let p = ro + rd * t;
        let h = scene_sdf_dist(p);

        if h < 0.0001 {
            return 0.0;  // Fully in shadow
        }

        // Improved soft shadow formula (from IQ)
        let y = h * h / (2.0 * ph);
        let d = sqrt(h * h - y * y);
        res = min(res, k * d / max(0.0, t - y));
        ph = h;

        t += max(h * 0.5, 0.005);  // Conservative stepping
        if t > maxt {
            break;
        }
    }
    return clamp(res, 0.0, 1.0);
}

// Ambient occlusion
fn calc_ao(p: vec3<f32>, n: vec3<f32>) -> f32 {
    var occ = 0.0;
    var sca = 1.0;
    for (var i = 0; i < 5; i++) {
        let h = 0.01 + 0.12 * f32(i) / 4.0;
        let d = scene_sdf_dist(p + h * n);
        occ += (h - d) * sca;
        sca *= 0.95;
    }
    return clamp(1.0 - 3.0 * occ, 0.0, 1.0);
}

fn shade(result: RayResult, rd: vec3<f32>, ro: vec3<f32>) -> vec3<f32> {
    if !result.hit {
        // Procedural skybox with day/night cycle
        return sample_sky(rd);
    }

    let p = result.position;
    let n = calc_normal(p);
    let base_color = get_material_color(result.material_id);

    // Get sun direction based on time of day
    let sun_dir = get_sun_direction();
    let sun_visible = sun_dir.y > -0.1;  // Sun can cast some light even slightly below horizon

    // Get moon direction and lighting
    let moon_dir = get_moon_direction();
    let moon_visible = moon_dir.y > 0.0 && sky.moon_enabled == 1u;

    // Calculate moon illumination for lighting intensity
    let moon_illumination = get_moon_illumination();
    let moon_brightness = max(abs(moon_illumination) * 0.8 + 0.2, 0.0) * sky.moon_strength;

    // Sun color varies with time
    let night_visibility = get_night_visibility();
    let sun_height_factor = clamp(sun_dir.y + 0.1, 0.0, 1.0);  // Smooth transition
    let sun_intensity = sun_height_factor * (1.0 - night_visibility * 0.8);

    // Sun color: warm at sunrise/sunset, bright white at noon
    let horizon_sun_color = vec3<f32>(1.0, 0.6, 0.3);  // Orange/warm
    let noon_sun_color = vec3<f32>(1.0, 0.98, 0.95);   // Slightly warm white
    let sun_color = mix(horizon_sun_color, noon_sun_color, clamp(sun_dir.y * 2.0, 0.0, 1.0));

    // Moon color: cool blue-white
    let moon_light_color = vec3<f32>(0.7, 0.75, 0.9);  // Cool blue moonlight

    // Key light (sun)
    let sun_diffuse = max(dot(n, sun_dir), 0.0);

    // Moon diffuse lighting
    let moon_diffuse = max(dot(n, moon_dir), 0.0);

    // RAYMARCHED SHADOWS - the key feature!
    // Offset from surface along normal to prevent self-shadowing
    let shadow_origin = p + n * 0.02;

    // Sun shadows
    var sun_shadow = 1.0;
    if sun_visible && sun_diffuse > 0.0 {
        sun_shadow = soft_shadow(shadow_origin, sun_dir, 0.02, 50.0, 12.0);
        sun_shadow = sun_shadow * sun_shadow;
    }

    // Moon shadows (softer, less defined than sun shadows)
    var moon_shadow = 1.0;
    if moon_visible && moon_diffuse > 0.0 && night_visibility > 0.3 {
        moon_shadow = soft_shadow(shadow_origin, moon_dir, 0.02, 30.0, 6.0);
        moon_shadow = moon_shadow * 0.7 + 0.3;  // Softer shadows for moonlight
    }

    // Ambient occlusion
    let ao = calc_ao(p, n);

    // Fill light (sky) - varies with day/night
    let sky_brightness = mix(vec3<f32>(0.4, 0.5, 0.7), vec3<f32>(0.08, 0.08, 0.15), night_visibility);
    let sky_light = max(0.0, 0.5 + 0.5 * n.y) * sky_brightness;

    // Ground bounce - subtle upward fill light
    let ground_brightness = mix(vec3<f32>(0.2, 0.18, 0.15), vec3<f32>(0.03, 0.03, 0.03), night_visibility);
    let ground_light = max(0.0, 0.5 - 0.5 * n.y) * ground_brightness;

    // Ambient base - minimum light level (slightly brighter during full moon)
    let moon_ambient_boost = moon_brightness * night_visibility * 0.05;
    let ambient = mix(vec3<f32>(0.15, 0.15, 0.18), vec3<f32>(0.04, 0.04, 0.08), night_visibility)
                  + vec3<f32>(moon_ambient_boost * 0.7, moon_ambient_boost * 0.8, moon_ambient_boost);

    // Specular highlight (Blinn-Phong) - Sun
    let view_dir = normalize(ro - p);
    let sun_half_dir = normalize(sun_dir + view_dir);
    let spec_power = 64.0;
    let sun_spec = pow(max(dot(n, sun_half_dir), 0.0), spec_power) * 0.6 * sun_intensity;

    // Specular highlight - Moon (weaker, broader)
    let moon_half_dir = normalize(moon_dir + view_dir);
    let moon_spec_power = 32.0;  // Broader highlight
    let moon_spec = pow(max(dot(n, moon_half_dir), 0.0), moon_spec_power) * 0.2 * moon_brightness * night_visibility;

    // Direct sunlight contribution (affected by shadow)
    let direct_sun_light = sun_color * sun_diffuse * sun_shadow * sun_intensity * 1.2;

    // Direct moonlight contribution (affected by shadow, only at night)
    let direct_moon_light = moon_light_color * moon_diffuse * moon_shadow * moon_brightness * night_visibility * 0.4;

    // Indirect lighting (not affected by shadow, only AO)
    let indirect_light = (sky_light + ground_light) * ao;

    // Combine all lighting
    var color = base_color * (ambient + direct_sun_light + direct_moon_light + indirect_light);

    // Add sun specular (affected by shadow)
    color += vec3<f32>(sun_spec) * sun_shadow * sun_color;

    // Add moon specular (softer, blue-tinted)
    color += moon_light_color * moon_spec * moon_shadow;

    // Rim lighting for better object separation (combines sun and moon)
    let rim = 1.0 - max(dot(view_dir, n), 0.0);
    let sun_rim_light = pow(rim, 4.0) * 0.15 * sun_intensity * (1.0 - sun_shadow * 0.5);
    let moon_rim_light = pow(rim, 5.0) * 0.08 * moon_brightness * night_visibility * (1.0 - moon_shadow * 0.5);
    color += sun_color * sun_rim_light + moon_light_color * moon_rim_light;

    return color;
}

// ============================================================================
// FOG SYSTEM
// ============================================================================

// Calculate fog amount based on distance
// Returns fog factor (0.0 = no fog, 1.0 = full fog)
// Formula: 1.0 - exp(-(distance - fog_start) * fog_density)
// Fog density interpolates based on sun position (less fog during day)
fn calculate_fog(distance: f32, rd: vec3<f32>) -> f32 {
    // Check if fog is disabled
    if sky.fog_enabled == 0u {
        return 0.0;
    }

    // Only apply fog beyond the start distance
    let effective_distance = max(distance - sky.fog_start_distance, 0.0);
    if effective_distance <= 0.0 {
        return 0.0;
    }

    // Get sun direction for day/night interpolation
    let sun_dir = get_sun_direction();

    // Day has less fog (0.5x density), night has more (1.5x density for atmosphere)
    // sun_dir.y: -1 = midnight, 0 = horizon, 1 = noon
    let day_factor = clamp(sun_dir.y + 0.1, 0.0, 1.0);  // 0 at night, 1 at noon
    let density_multiplier = mix(1.5, 0.5, day_factor);  // More fog at night

    let adjusted_density = sky.fog_density * density_multiplier;

    // Exponential fog formula
    let fog_amount = 1.0 - exp(-effective_distance * adjusted_density);

    return clamp(fog_amount, 0.0, 1.0);
}

// Apply fog to a scene color, blending towards the sky/horizon color
fn apply_fog(scene_color: vec3<f32>, distance: f32, rd: vec3<f32>) -> vec3<f32> {
    let fog_amount = calculate_fog(distance, rd);

    if fog_amount <= 0.0 {
        return scene_color;
    }

    // Fog color is based on the sky at the horizon
    let fog_color = sample_sky(vec3<f32>(rd.x, 0.05, rd.z));  // Look near horizon

    return mix(scene_color, fog_color, fog_amount);
}

// ============================================================================
// RAY GENERATION
// ============================================================================

fn get_ray_direction(uv: vec2<f32>, fov: f32) -> vec3<f32> {
    let ndc = uv * 2.0 - 1.0;
    let aspect = uniforms.resolution.x / uniforms.resolution.y;
    let half_fov = tan(fov * 0.5);

    let ray_cam = vec3<f32>(
        ndc.x * aspect * half_fov,
        ndc.y * half_fov,
        -1.0
    );

    // Camera looks at camera_target (not origin) - Unity-style panning
    var forward = normalize(uniforms.camera_target - uniforms.camera_pos);

    // Handle edge case when camera is directly above/below target
    let up_world = vec3<f32>(0.0, 1.0, 0.0);
    if abs(dot(forward, up_world)) > 0.99 {
        // Looking straight down or up
        let right = vec3<f32>(1.0, 0.0, 0.0);
        let up = cross(forward, right);
        return normalize(ray_cam.x * right + ray_cam.y * up + ray_cam.z * forward);
    }

    let right = normalize(cross(up_world, forward));
    let up = cross(forward, right);

    return normalize(ray_cam.x * right + ray_cam.y * up + ray_cam.z * forward);
}

// ============================================================================
// DEBUG MODES
// ============================================================================

fn debug_ray_direction(rd: vec3<f32>) -> vec3<f32> {
    return rd * 0.5 + 0.5;
}

fn debug_normals(result: RayResult) -> vec3<f32> {
    if !result.hit {
        return vec3<f32>(0.0);
    }
    let n = calc_normal(result.position);
    return n * 0.5 + 0.5;
}

fn debug_ao(result: RayResult) -> vec3<f32> {
    if !result.hit {
        return vec3<f32>(0.0);
    }
    let ao = calc_ao(result.position, calc_normal(result.position));
    return vec3<f32>(ao);
}

fn debug_steps(result: RayResult) -> vec3<f32> {
    let t = f32(result.steps) / f32(MAX_STEPS);
    // Heat map: blue -> green -> yellow -> red
    if t < 0.33 {
        return mix(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), t * 3.0);
    } else if t < 0.66 {
        return mix(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 1.0, 0.0), (t - 0.33) * 3.0);
    } else {
        return mix(vec3<f32>(1.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), (t - 0.66) * 3.0);
    }
}

// ============================================================================
// PERFORMANCE OVERLAY - 7-segment style digit rendering
// ============================================================================

// SDF for a horizontal segment (for 7-segment display)
fn sdf_h_segment(p: vec2<f32>, w: f32, h: f32) -> f32 {
    let d = abs(p) - vec2<f32>(w * 0.5, h * 0.5);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

// SDF for a vertical segment
fn sdf_v_segment(p: vec2<f32>, w: f32, h: f32) -> f32 {
    let d = abs(p) - vec2<f32>(w * 0.5, h * 0.5);
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0);
}

// Render a single 7-segment digit (0-9)
// Returns 1.0 if pixel is on the digit, 0.0 otherwise
fn render_digit(p: vec2<f32>, digit: u32, char_w: f32, char_h: f32) -> f32 {
    // Segment dimensions relative to character size
    let seg_w = char_w * 0.8;  // Horizontal segment width
    let seg_h = char_h * 0.12; // Segment thickness
    let v_seg_h = char_h * 0.35; // Vertical segment height

    // Center the digit
    let cp = p - vec2<f32>(char_w * 0.5, char_h * 0.5);

    // 7-segment layout:
    //   A
    //  F B
    //   G
    //  E C
    //   D

    // Segment positions
    let seg_a = cp - vec2<f32>(0.0, char_h * 0.42);  // Top
    let seg_d = cp - vec2<f32>(0.0, -char_h * 0.42); // Bottom
    let seg_g = cp;                                   // Middle
    let seg_b = cp - vec2<f32>(char_w * 0.3, char_h * 0.21);  // Top-right
    let seg_c = cp - vec2<f32>(char_w * 0.3, -char_h * 0.21); // Bottom-right
    let seg_e = cp - vec2<f32>(-char_w * 0.3, -char_h * 0.21); // Bottom-left
    let seg_f = cp - vec2<f32>(-char_w * 0.3, char_h * 0.21);  // Top-left

    // Which segments are lit for each digit (A B C D E F G)
    // 0: A B C D E F
    // 1: B C
    // 2: A B D E G
    // 3: A B C D G
    // 4: B C F G
    // 5: A C D F G
    // 6: A C D E F G
    // 7: A B C
    // 8: A B C D E F G
    // 9: A B C D F G

    var result = 0.0;
    let threshold = seg_h * 0.6;

    // Segment A (top)
    if digit == 0u || digit == 2u || digit == 3u || digit == 5u || digit == 6u || digit == 7u || digit == 8u || digit == 9u {
        let d = sdf_h_segment(seg_a, seg_w, seg_h);
        if d < threshold { result = 1.0; }
    }
    // Segment B (top-right)
    if digit == 0u || digit == 1u || digit == 2u || digit == 3u || digit == 4u || digit == 7u || digit == 8u || digit == 9u {
        let d = sdf_v_segment(seg_b, seg_h, v_seg_h);
        if d < threshold { result = 1.0; }
    }
    // Segment C (bottom-right)
    if digit == 0u || digit == 1u || digit == 3u || digit == 4u || digit == 5u || digit == 6u || digit == 7u || digit == 8u || digit == 9u {
        let d = sdf_v_segment(seg_c, seg_h, v_seg_h);
        if d < threshold { result = 1.0; }
    }
    // Segment D (bottom)
    if digit == 0u || digit == 2u || digit == 3u || digit == 5u || digit == 6u || digit == 8u || digit == 9u {
        let d = sdf_h_segment(seg_d, seg_w, seg_h);
        if d < threshold { result = 1.0; }
    }
    // Segment E (bottom-left)
    if digit == 0u || digit == 2u || digit == 6u || digit == 8u {
        let d = sdf_v_segment(seg_e, seg_h, v_seg_h);
        if d < threshold { result = 1.0; }
    }
    // Segment F (top-left)
    if digit == 0u || digit == 4u || digit == 5u || digit == 6u || digit == 8u || digit == 9u {
        let d = sdf_v_segment(seg_f, seg_h, v_seg_h);
        if d < threshold { result = 1.0; }
    }
    // Segment G (middle)
    if digit == 2u || digit == 3u || digit == 4u || digit == 5u || digit == 6u || digit == 8u || digit == 9u {
        let d = sdf_h_segment(seg_g, seg_w, seg_h);
        if d < threshold { result = 1.0; }
    }

    return result;
}

// Render a decimal point
fn render_decimal(p: vec2<f32>, char_w: f32, char_h: f32) -> f32 {
    let dot_pos = p - vec2<f32>(char_w * 0.5, char_h * 0.1);
    let d = length(dot_pos) - char_h * 0.08;
    if d < 0.0 { return 1.0; }
    return 0.0;
}

// Render a number as digits at a given position
// Returns the color contribution (0.0-1.0) for the overlay
fn render_number(screen_pos: vec2<f32>, value: f32, num_digits: u32, decimal_places: u32,
                 char_w: f32, char_h: f32, start_x: f32, start_y: f32) -> f32 {
    let local_pos = screen_pos - vec2<f32>(start_x, start_y);

    // Check if we're in the number area
    let total_chars = num_digits + select(0u, 1u, decimal_places > 0u); // +1 for decimal point
    let total_width = f32(total_chars) * char_w * 1.2;

    if local_pos.x < 0.0 || local_pos.x > total_width || local_pos.y < 0.0 || local_pos.y > char_h {
        return 0.0;
    }

    // Scale value for decimal places
    var scaled_value = value;
    for (var i = 0u; i < decimal_places; i++) {
        scaled_value = scaled_value * 10.0;
    }
    var int_value = u32(max(scaled_value, 0.0));

    // Calculate which character position we're at
    let char_idx = u32(local_pos.x / (char_w * 1.2));
    let char_local_x = local_pos.x - f32(char_idx) * char_w * 1.2;
    let char_pos = vec2<f32>(char_local_x, local_pos.y);

    // Determine decimal point position (from right)
    let decimal_pos = num_digits - decimal_places;

    // Check if this is the decimal point position
    if decimal_places > 0u && char_idx == decimal_pos {
        return render_decimal(char_pos, char_w, char_h);
    }

    // Adjust char_idx for positions after decimal point
    var digit_idx = char_idx;
    if decimal_places > 0u && char_idx > decimal_pos {
        digit_idx = char_idx - 1u;
    }

    // Extract the digit at this position
    let digits_from_right = num_digits - 1u - digit_idx;
    var divisor = 1u;
    for (var i = 0u; i < digits_from_right; i++) {
        divisor = divisor * 10u;
    }
    let digit = (int_value / divisor) % 10u;

    return render_digit(char_pos, digit, char_w, char_h);
}

// Simple text rendering using predefined character patterns
// Each letter is a 5-bit wide, 7-bit tall pattern encoded in a u32
fn get_char_pattern(c: u32) -> u32 {
    // Very simple 5x7 font patterns (encoded as bits)
    switch c {
        // Letters (uppercase)
        case 70u: { return 0x7C1F04u; }  // F: 01111100 00011111 00000100
        case 80u: { return 0x7C1F7Cu; }  // P: 01111100 00011111 01111100
        case 83u: { return 0x3E413Eu; }  // S: 00111110 01000001 00111110
        case 69u: { return 0x7F417Fu; }  // E: 01111111 01000001 01111111
        case 78u: { return 0x7F0808u; }  // N: 01111111 00001000 00001000 (partial)
        case 84u: { return 0x7F0808u; }  // T: 01111111 00001000 00001000 (partial)
        case 73u: { return 0x7F0000u; }  // I: 01111111
        case 89u: { return 0x070870u; }  // Y: partial
        case 66u: { return 0x7F497Fu; }  // B: 01111111 01001001 01111111 (partial)
        case 65u: { return 0x7F097Fu; }  // A: partial
        case 75u: { return 0x7F0836u; }  // K: partial
        case 68u: { return 0x7F413Eu; }  // D: partial
        case 71u: { return 0x3E517Eu; }  // G: partial
        case 77u: { return 0x7F0207u; }  // M: partial
        case 58u: { return 0x0024u; }    // : colon
        default: { return 0u; }
    }
}

// Render the performance overlay
// Returns vec4: rgb = overlay color, a = alpha (blend factor)
fn render_perf_overlay(uv: vec2<f32>) -> vec4<f32> {
    if uniforms.show_perf_overlay == 0u {
        return vec4<f32>(0.0);
    }

    let pixel = uv * uniforms.resolution;

    // Overlay dimensions (top-left corner)
    let overlay_x = 10.0;
    let overlay_y = uniforms.resolution.y - 10.0; // From top
    let overlay_width = 280.0;
    let overlay_height = 180.0;
    let char_w = 12.0;
    let char_h = 18.0;
    let line_height = 22.0;
    let padding = 8.0;

    // Convert to overlay-local coordinates (Y inverted for top-down)
    let local_x = pixel.x - overlay_x;
    let local_y = overlay_y - pixel.y;

    // Check if we're in the overlay background area
    if local_x >= 0.0 && local_x < overlay_width && local_y >= 0.0 && local_y < overlay_height {
        // Semi-transparent dark background
        let bg_color = vec3<f32>(0.0, 0.0, 0.0);
        let bg_alpha = 0.75;

        // Text content
        var text_alpha = 0.0;
        let text_color = vec3<f32>(0.0, 1.0, 0.3); // Green text like old terminals

        // Line positions (from top)
        let line1_y = overlay_height - padding - line_height;        // FPS
        let line2_y = overlay_height - padding - line_height * 2.0;  // Frame time
        let line3_y = overlay_height - padding - line_height * 3.0;  // Entities
        let line4_y = overlay_height - padding - line_height * 4.0;  // Baked SDFs
        let line5_y = overlay_height - padding - line_height * 5.0;  // Tile buffer
        let line6_y = overlay_height - padding - line_height * 6.0;  // GPU memory
        let line7_y = overlay_height - padding - line_height * 7.0;  // Active tiles

        // Labels are at x = padding, values at x = padding + 140
        let label_x = padding;
        let value_x = padding + 140.0;

        // Render FPS value (line 1)
        if local_y >= line1_y && local_y < line1_y + char_h {
            let digit_contrib = render_number(
                vec2<f32>(local_x, local_y - line1_y),
                uniforms.perf_fps, 4u, 0u, char_w, char_h, value_x, 0.0
            );
            text_alpha = max(text_alpha, digit_contrib);
        }

        // Render Frame Time value (line 2) - with 2 decimal places
        if local_y >= line2_y && local_y < line2_y + char_h {
            let digit_contrib = render_number(
                vec2<f32>(local_x, local_y - line2_y),
                uniforms.perf_frame_time_ms, 5u, 2u, char_w, char_h, value_x, 0.0
            );
            text_alpha = max(text_alpha, digit_contrib);
        }

        // Render Entity Count (line 3)
        if local_y >= line3_y && local_y < line3_y + char_h {
            let digit_contrib = render_number(
                vec2<f32>(local_x, local_y - line3_y),
                f32(uniforms.perf_entity_count), 3u, 0u, char_w, char_h, value_x, 0.0
            );
            text_alpha = max(text_alpha, digit_contrib);
        }

        // Render Baked SDF Count (line 4)
        if local_y >= line4_y && local_y < line4_y + char_h {
            let digit_contrib = render_number(
                vec2<f32>(local_x, local_y - line4_y),
                f32(uniforms.perf_baked_sdf_count), 3u, 0u, char_w, char_h, value_x, 0.0
            );
            text_alpha = max(text_alpha, digit_contrib);
        }

        // Render Tile Buffer KB (line 5) - with 1 decimal
        if local_y >= line5_y && local_y < line5_y + char_h {
            let digit_contrib = render_number(
                vec2<f32>(local_x, local_y - line5_y),
                uniforms.perf_tile_buffer_kb, 5u, 1u, char_w, char_h, value_x, 0.0
            );
            text_alpha = max(text_alpha, digit_contrib);
        }

        // Render GPU Memory MB (line 6) - with 2 decimals
        if local_y >= line6_y && local_y < line6_y + char_h {
            let digit_contrib = render_number(
                vec2<f32>(local_x, local_y - line6_y),
                uniforms.perf_gpu_memory_mb, 5u, 2u, char_w, char_h, value_x, 0.0
            );
            text_alpha = max(text_alpha, digit_contrib);
        }

        // Render Active Tile Count (line 7)
        if local_y >= line7_y && local_y < line7_y + char_h {
            let digit_contrib = render_number(
                vec2<f32>(local_x, local_y - line7_y),
                f32(uniforms.perf_active_tile_count), 5u, 0u, char_w, char_h, value_x, 0.0
            );
            text_alpha = max(text_alpha, digit_contrib);
        }

        // Combine background and text
        if text_alpha > 0.5 {
            return vec4<f32>(text_color, 1.0);
        } else {
            return vec4<f32>(bg_color, bg_alpha);
        }
    }

    return vec4<f32>(0.0);
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let ro = uniforms.camera_pos;
    let rd = get_ray_direction(uv, 1.2); // ~69 degree FOV

    var color: vec3<f32>;

    switch uniforms.debug_mode {
        case 0u: {
            // Normal rendering with grid
            let result = ray_march(ro, rd);
            var sdf_color = shade(result, rd, ro);

            // Apply fog to objects (not to sky)
            if result.hit {
                sdf_color = apply_fog(sdf_color, result.distance, rd);
            }

            // Blend grid underneath objects
            let grid = unity_grid(result.position, rd, ro);
            if !result.hit {
                // Sky + grid overlay
                color = sdf_color;
                let grid_floor = unity_grid(vec3<f32>(0.0), rd, ro);
                color = mix(color, grid_floor.rgb, grid_floor.a);

                // Add volume grid if enabled (for building in air)
                let vol_grid = volume_grid_3d(vec3<f32>(0.0), rd, ro);
                color = mix(color, vol_grid.rgb, vol_grid.a);

                // Add placement height indicator
                let height_ind = placement_height_indicator(rd, ro);
                color = mix(color, height_ind.rgb, height_ind.a);
            } else if result.position.y < 0.01 {
                // Object on/near ground - no grid blend needed
                color = sdf_color;
            } else {
                color = sdf_color;
            }
        }
        case 1u: {
            // Grid + objects (no human)
            // Still render placed entities and reference objects
            let result = ray_march(ro, rd);

            if result.hit && result.material_id != 10u {
                // Hit something other than human - render it with fog
                var obj_color = shade(result, rd, ro);
                obj_color = apply_fog(obj_color, result.distance, rd);
                color = obj_color;
            } else {
                // No hit or human hit - show grid + sky
                let grid = unity_grid(vec3<f32>(0.0), rd, ro);
                let sky_col = sample_sky(rd);
                color = mix(sky_col, grid.rgb, grid.a);

                // Add volume grid if enabled
                let vol_grid = volume_grid_3d(vec3<f32>(0.0), rd, ro);
                color = mix(color, vol_grid.rgb, vol_grid.a);
            }
        }
        case 2u: {
            // Normals
            let result = ray_march(ro, rd);
            color = debug_normals(result);
        }
        case 3u: {
            // Ambient occlusion
            let result = ray_march(ro, rd);
            color = debug_ao(result);
        }
        case 4u: {
            // Ray march steps (performance)
            let result = ray_march(ro, rd);
            color = debug_steps(result);
        }
        case 5u: {
            // Ray directions
            color = debug_ray_direction(rd);
        }
        default: {
            color = vec3<f32>(1.0, 0.0, 1.0);
        }
    }

    // Gamma correction
    color = pow(color, vec3<f32>(1.0 / 2.2));

    // Performance overlay (F12 to toggle)
    let perf_overlay = render_perf_overlay(uv);
    if perf_overlay.a > 0.0 {
        color = mix(color, perf_overlay.rgb, perf_overlay.a);
    }

    return vec4<f32>(color, 1.0);
}
