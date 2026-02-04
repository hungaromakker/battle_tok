// ============================================================================
// ACES Tonemapping Shader (tonemap_aces.wgsl)
// ============================================================================
// Cinematic HDR to LDR conversion using ACES filmic curve
// Makes emissive effects (lava, magic) look correct without clipping
// Industry-standard tonemapping used in film and games
// ============================================================================

struct TonemapParams {
    exposure: f32,              // 1.0 default, increase for brighter
    gamma: f32,                 // 2.2 default (sRGB)
    saturation: f32,            // 1.0 default, <1 desaturated, >1 vivid
    contrast: f32,              // 1.0 default
}

@group(0) @binding(0)
var<uniform> tonemap: TonemapParams;

@group(0) @binding(1)
var hdr_texture: texture_2d<f32>;

@group(0) @binding(2)
var hdr_sampler: sampler;

// ============================================================================
// ACES FILMIC TONEMAPPING
// ============================================================================
// Reference: https://knarkowicz.wordpress.com/2016/01/06/aces-filmic-tone-mapping-curve/

fn aces_film(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

// Alternative: Uncharted 2 tonemapping (more filmic)
fn uncharted2_partial(x: vec3<f32>) -> vec3<f32> {
    let A = 0.15;
    let B = 0.50;
    let C = 0.10;
    let D = 0.20;
    let E = 0.02;
    let F = 0.30;
    return ((x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F)) - E / F;
}

fn uncharted2(x: vec3<f32>) -> vec3<f32> {
    let W = 11.2; // Linear white point
    let curr = uncharted2_partial(x);
    let white_scale = 1.0 / uncharted2_partial(vec3<f32>(W));
    return curr * white_scale;
}

// ============================================================================
// COLOR ADJUSTMENTS
// ============================================================================

fn adjust_saturation(color: vec3<f32>, saturation: f32) -> vec3<f32> {
    let luminance = dot(color, vec3<f32>(0.299, 0.587, 0.114));
    return mix(vec3<f32>(luminance), color, saturation);
}

fn adjust_contrast(color: vec3<f32>, contrast: f32) -> vec3<f32> {
    return (color - 0.5) * contrast + 0.5;
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
    out.position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
    out.uv = positions[vertex_index] * 0.5 + 0.5;
    out.uv.y = 1.0 - out.uv.y;
    return out;
}

// ============================================================================
// FRAGMENT SHADER
// ============================================================================

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.uv;
    
    // Sample HDR color
    var hdr = textureSample(hdr_texture, hdr_sampler, uv).rgb;
    
    // Apply exposure
    hdr *= tonemap.exposure;
    
    // Apply ACES tonemapping (HDR -> LDR)
    var ldr = aces_film(hdr);
    
    // Color adjustments (after tonemapping)
    ldr = adjust_saturation(ldr, tonemap.saturation);
    ldr = adjust_contrast(ldr, tonemap.contrast);
    
    // Gamma correction (linear -> sRGB)
    let gamma_inv = 1.0 / tonemap.gamma;
    ldr = pow(ldr, vec3<f32>(gamma_inv));
    
    return vec4<f32>(ldr, 1.0);
}

// ============================================================================
// COMBINED FOG + TONEMAP (single pass version)
// ============================================================================
// If you want to combine fog and tonemap in one pass for efficiency:

// @fragment
// fn fs_fog_tonemap(in: VertexOutput) -> @location(0) vec4<f32> {
//     let hdr = textureSample(hdr_texture, hdr_sampler, in.uv).rgb;
//     
//     // Apply fog first (in linear HDR space)
//     // ... fog calculations ...
//     
//     // Then tonemap
//     var ldr = aces_film(hdr * tonemap.exposure);
//     ldr = pow(ldr, vec3<f32>(1.0 / tonemap.gamma));
//     
//     return vec4<f32>(ldr, 1.0);
// }
