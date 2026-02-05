// Battle Tok Engine - Ember/Ash Particle Shader
// GPU-instanced billboard particles for floating embers rising from lava
//
// This shader renders particles as camera-facing billboards with soft circular
// shapes and additive blending for a glowing ember effect.

// ============================================================================
// UNIFORM BINDINGS
// ============================================================================

// Particle uniforms (64 bytes total):
// - view: mat4x4<f32> @ offset 0 (64 bytes) - View matrix for billboard calculation
struct ParticleUniforms {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: ParticleUniforms;

// ============================================================================
// PARTICLE INSTANCE DATA (matches Rust GpuParticle struct)
// ============================================================================
//
// GpuParticle layout (32 bytes total):
// Row 0 (offset 0-15):  position_x, position_y, position_z, lifetime
// Row 1 (offset 16-31): size, color_r, color_g, color_b
//
// Note: We use scalar fields for position to match Rust [f32; 3] alignment.

struct GpuParticle {
    // Row 0: position (3 floats) + lifetime = 16 bytes
    position_x: f32,      // offset 0
    position_y: f32,      // offset 4
    position_z: f32,      // offset 8
    lifetime: f32,        // offset 12 (0.0 = dead, 1.0 = just spawned)
    // Row 1: size + color (3 floats) = 16 bytes
    size: f32,            // offset 16
    color_r: f32,         // offset 20
    color_g: f32,         // offset 24
    color_b: f32,         // offset 28
}

// Storage buffer containing all active particles
@group(0) @binding(1)
var<storage, read> particles: array<GpuParticle>;

// ============================================================================
// VERTEX SHADER
// ============================================================================

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) lifetime: f32,
}

// Quad vertices for a single particle (two triangles forming a quad)
// Vertex indices: 0, 1, 2, 0, 2, 3 form two triangles
// Positions: (-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)
fn get_quad_position(vertex_index: u32) -> vec2<f32> {
    // Generate quad corners from vertex index (0-5 for two triangles)
    // Triangle 1: 0, 1, 2 (bottom-left, bottom-right, top-right)
    // Triangle 2: 0, 2, 3 (bottom-left, top-right, top-left)
    let corner_index = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u)[vertex_index];
    let corners = array<vec2<f32>, 4>(
        vec2<f32>(-0.5, -0.5),  // bottom-left
        vec2<f32>(0.5, -0.5),   // bottom-right
        vec2<f32>(0.5, 0.5),    // top-right
        vec2<f32>(-0.5, 0.5),   // top-left
    );
    return corners[corner_index];
}

fn get_quad_uv(vertex_index: u32) -> vec2<f32> {
    let corner_index = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u)[vertex_index];
    let uvs = array<vec2<f32>, 4>(
        vec2<f32>(0.0, 0.0),  // bottom-left
        vec2<f32>(1.0, 0.0),  // bottom-right
        vec2<f32>(1.0, 1.0),  // top-right
        vec2<f32>(0.0, 1.0),  // top-left
    );
    return uvs[corner_index];
}

@vertex
fn vs_particle(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    let particle = particles[instance_index];

    // Get local quad position (-0.5 to 0.5)
    let local_pos = get_quad_position(vertex_index % 6u);

    // Billboard: extract right and up vectors from the view matrix
    // The view matrix transforms from world to view space
    // Its columns (before transpose) contain the camera's axes in world space
    let right = vec3<f32>(uniforms.view[0][0], uniforms.view[1][0], uniforms.view[2][0]);
    let up = vec3<f32>(uniforms.view[0][1], uniforms.view[1][1], uniforms.view[2][1]);

    // Particle world position
    let particle_pos = vec3<f32>(particle.position_x, particle.position_y, particle.position_z);

    // Offset from particle center to create billboard quad
    let world_pos = particle_pos
        + right * local_pos.x * particle.size
        + up * local_pos.y * particle.size;

    // Transform to clip space
    let view_pos = uniforms.view * vec4<f32>(world_pos, 1.0);
    let clip_pos = uniforms.proj * view_pos;

    var output: VertexOutput;
    output.clip_position = clip_pos;
    output.uv = get_quad_uv(vertex_index % 6u);
    output.color = vec3<f32>(particle.color_r, particle.color_g, particle.color_b);
    output.lifetime = particle.lifetime;

    return output;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_particle(in: VertexOutput) -> @location(0) vec4<f32> {
    // Calculate distance from center for soft circle
    let center = vec2<f32>(0.5, 0.5);
    let d = length(in.uv - center);

    // Soft circle falloff: smoothstep from edge (0.5) to center (0.0)
    // This creates a soft, glowing appearance
    let circle_alpha = smoothstep(0.5, 0.1, d);

    // Fade out as lifetime decreases
    // Lifetime goes from 1.0 (just spawned) to 0.0 (dead)
    let fade = in.lifetime;

    // Final alpha combines circle shape and lifetime fade
    let alpha = circle_alpha * fade;

    // HDR emissive color (values > 1.0 will bloom if post-processing is enabled)
    // Multiply by a glow factor for extra brightness
    let glow_factor = 2.0;
    let emissive_color = in.color * glow_factor;

    // Return with alpha for additive blending
    // In additive mode, the alpha modulates how much is added to the background
    return vec4<f32>(emissive_color * alpha, alpha);
}
