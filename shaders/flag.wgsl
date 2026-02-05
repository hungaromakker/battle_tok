// ============================================================================
// Team Flag Shader with Wind Animation (flag.wgsl)
// ============================================================================
// Vertex-based wind animation for waving flag effect with team colors.
// Features:
// - Wave displacement stronger at flag edge, weaker near pole
// - Configurable team color (red/blue)
// - Horizontal stripe band for visual interest
// - Smooth sin() based wind animation

// Uniforms - scalar fields to match Rust alignment
// Total size: 64 bytes (aligned to 16)
struct FlagUniforms {
    view_proj: mat4x4<f32>,           // 64 bytes (offset 0)
    time: f32,                        // 4 bytes (offset 64)
    team_color_r: f32,                // 4 bytes (offset 68)
    team_color_g: f32,                // 4 bytes (offset 72)
    team_color_b: f32,                // 4 bytes (offset 76)
    stripe_color_r: f32,              // 4 bytes (offset 80)
    stripe_color_g: f32,              // 4 bytes (offset 84)
    stripe_color_b: f32,              // 4 bytes (offset 88)
    wind_strength: f32,               // 4 bytes (offset 92) - 0.1..0.3 typical
    camera_pos_x: f32,                // 4 bytes (offset 96)
    camera_pos_y: f32,                // 4 bytes (offset 100)
    camera_pos_z: f32,                // 4 bytes (offset 104)
    ambient_strength: f32,            // 4 bytes (offset 108)
    _pad1: f32,                       // 4 bytes (offset 112) - padding
    _pad2: f32,                       // 4 bytes (offset 116) - padding
    _pad3: f32,                       // 4 bytes (offset 120) - padding
    _pad4: f32,                       // 4 bytes (offset 124) - align to 128
}

@group(0) @binding(0)
var<uniform> uniforms: FlagUniforms;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

fn get_team_color() -> vec3<f32> {
    return vec3<f32>(uniforms.team_color_r, uniforms.team_color_g, uniforms.team_color_b);
}

fn get_stripe_color() -> vec3<f32> {
    return vec3<f32>(uniforms.stripe_color_r, uniforms.stripe_color_g, uniforms.stripe_color_b);
}

fn get_camera_pos() -> vec3<f32> {
    return vec3<f32>(uniforms.camera_pos_x, uniforms.camera_pos_y, uniforms.camera_pos_z);
}

// ============================================================================
// VERTEX SHADER - Wind Wave Animation
// ============================================================================

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) normal: vec3<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // ========================================================================
    // WAVE DISPLACEMENT
    // ========================================================================
    // Wave increases toward flag edge (high uv.x = far from pole)
    // Wave decreases toward top of flag attachment (high uv.y = attached edge)
    // This simulates realistic cloth physics where:
    // - The pole (uv.x=0) is fixed
    // - The free edge (uv.x=1) waves most
    // - The top attachment (uv.y=1) moves less than the bottom (uv.y=0)

    let wave_amount = in.uv.x * (1.0 - in.uv.y * 0.5);

    // Main wave using sin() - creates smooth rippling motion
    // Multiple wave frequencies for more natural look
    let primary_wave = sin(in.uv.x * 10.0 + uniforms.time * 3.5);
    let secondary_wave = sin(in.uv.x * 15.0 + uniforms.time * 5.0) * 0.3;
    let combined_wave = (primary_wave + secondary_wave) * wave_amount * uniforms.wind_strength;

    // Apply displacement in multiple directions for realistic cloth motion
    // Main motion is perpendicular to flag face (Z), with slight Y wobble
    let displaced = in.position + vec3<f32>(
        combined_wave * 0.3,  // Slight X movement
        combined_wave,        // Main Y wave
        combined_wave * 0.1   // Slight Z movement
    );

    // Calculate approximate normal for displaced surface
    // This gives basic lighting variation across the wave
    let wave_derivative = cos(in.uv.x * 10.0 + uniforms.time * 3.5) * wave_amount * uniforms.wind_strength * 10.0;
    out.normal = normalize(vec3<f32>(-wave_derivative * 0.3, 1.0, -wave_derivative * 0.1));

    // Transform to clip space
    out.clip_position = uniforms.view_proj * vec4<f32>(displaced, 1.0);
    out.uv = in.uv;
    out.world_pos = displaced;

    return out;
}

// ============================================================================
// FRAGMENT SHADER - Team Color with Stripe Pattern
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    let normal = normalize(in.normal);

    // ========================================================================
    // TEAM COLOR AND STRIPE PATTERN
    // ========================================================================

    let team_color = get_team_color();
    let stripe_color = get_stripe_color();

    // Horizontal stripe band across the flag
    // Using smoothstep for anti-aliased edges
    // Stripe is positioned around y=0.5 (middle of flag)
    let stripe_start = smoothstep(0.45, 0.48, uv.y);
    let stripe_end = smoothstep(0.52, 0.55, uv.y);
    let stripe = stripe_start - stripe_end;

    // Mix team color with stripe color
    // Stripe is 85% of stripe color for visible but not overpowering effect
    var base_color = mix(team_color, stripe_color, stripe * 0.85);

    // ========================================================================
    // BASIC LIGHTING
    // ========================================================================

    // Simple directional light from above-right
    let light_dir = normalize(vec3<f32>(0.5, 0.8, 0.3));
    let n_dot_l = max(dot(normal, light_dir), 0.0);

    // Ambient plus diffuse
    let ambient = uniforms.ambient_strength;
    let diffuse = n_dot_l * (1.0 - ambient);
    let lighting = ambient + diffuse;

    // Add subtle edge darkening for cloth feel
    let edge_factor = 1.0 - abs(in.uv.x - 0.5) * 0.2;

    var final_color = base_color * lighting * edge_factor;

    // ========================================================================
    // OPTIONAL: Slight saturation boost for vibrant team colors
    // ========================================================================

    let luminance = dot(final_color, vec3<f32>(0.299, 0.587, 0.114));
    final_color = mix(vec3<f32>(luminance), final_color, 1.15);

    // Clamp to valid range
    final_color = clamp(final_color, vec3<f32>(0.0), vec3<f32>(1.0));

    // Gamma correction
    final_color = pow(final_color, vec3<f32>(1.0 / 2.2));

    return vec4<f32>(final_color, 1.0);
}
