// Sketch Engine - Lighting Utility Shader
// Directional lighting calculations for SDF ray marching
//
// Note: This file is designed to be included after a main shader that defines sdf_scene().
// The stub function below allows standalone validation while the real sdf_scene()
// is provided by the main shader file that includes this utility.

// Stub sdf_scene for standalone validation - overridden when included by main shader
fn sdf_scene(p: vec3f) -> f32 {
    return length(p) - 1.0;  // Simple sphere placeholder
}

// Calculate normal from SDF gradient using central differences
// Requires sdf_scene() to be defined in the main shader
fn calc_normal(p: vec3f) -> vec3f {
    let e = vec2f(0.001, 0.0);
    return normalize(vec3f(
        sdf_scene(p + e.xyy) - sdf_scene(p - e.xyy),
        sdf_scene(p + e.yxy) - sdf_scene(p - e.yxy),
        sdf_scene(p + e.yyx) - sdf_scene(p - e.yyx)
    ));
}

// Calculate normal with configurable epsilon for precision vs performance tradeoff
fn calc_normal_eps(p: vec3f, eps: f32) -> vec3f {
    let e = vec2f(eps, 0.0);
    return normalize(vec3f(
        sdf_scene(p + e.xyy) - sdf_scene(p - e.xyy),
        sdf_scene(p + e.yxy) - sdf_scene(p - e.yxy),
        sdf_scene(p + e.yyx) - sdf_scene(p - e.yyx)
    ));
}

// Directional light calculation
// Returns diffuse intensity (0.0 - 1.0)
fn directional_light(normal: vec3f, light_dir: vec3f) -> f32 {
    return max(dot(normal, normalize(light_dir)), 0.0);
}

// Directional light with ambient term
// ambient: base lighting level (0.0 - 1.0)
// Returns combined intensity (ambient - 1.0)
fn directional_light_ambient(normal: vec3f, light_dir: vec3f, ambient: f32) -> f32 {
    let diffuse = max(dot(normal, normalize(light_dir)), 0.0);
    return ambient + (1.0 - ambient) * diffuse;
}

// Half-Lambert lighting for softer falloff
// Wraps diffuse lighting to avoid harsh shadows
fn half_lambert(normal: vec3f, light_dir: vec3f) -> f32 {
    let ndotl = dot(normal, normalize(light_dir));
    return ndotl * 0.5 + 0.5;
}

// Blinn-Phong specular highlight
// view_dir: direction from surface to camera
// shininess: specular exponent (higher = sharper highlight)
fn specular_blinn_phong(normal: vec3f, light_dir: vec3f, view_dir: vec3f, shininess: f32) -> f32 {
    let l = normalize(light_dir);
    let v = normalize(view_dir);
    let h = normalize(l + v);  // Half vector
    return pow(max(dot(normal, h), 0.0), shininess);
}

// Full directional light with diffuse and specular
struct LightResult {
    diffuse: f32,
    specular: f32,
}

fn directional_light_full(
    normal: vec3f,
    light_dir: vec3f,
    view_dir: vec3f,
    shininess: f32
) -> LightResult {
    let l = normalize(light_dir);
    let v = normalize(view_dir);
    let h = normalize(l + v);

    let diffuse = max(dot(normal, l), 0.0);
    let specular = pow(max(dot(normal, h), 0.0), shininess);

    return LightResult(diffuse, specular);
}

// Apply lighting to a base color
fn apply_lighting(
    base_color: vec3f,
    normal: vec3f,
    light_dir: vec3f,
    light_color: vec3f,
    ambient_color: vec3f
) -> vec3f {
    let diffuse = max(dot(normal, normalize(light_dir)), 0.0);
    return base_color * (ambient_color + light_color * diffuse);
}

// Soft shadow calculation using sphere tracing
// Returns shadow factor (0.0 = full shadow, 1.0 = no shadow)
// ro: ray origin (surface point + small offset along normal)
// rd: direction toward light
// mint: minimum distance to start marching
// maxt: maximum distance to light
// k: softness factor (higher = softer shadows, 8-32 typical)
fn soft_shadow(ro: vec3f, rd: vec3f, mint: f32, maxt: f32, k: f32) -> f32 {
    var res = 1.0;
    var t = mint;

    for (var i = 0; i < 64; i++) {
        let h = sdf_scene(ro + rd * t);
        if (h < 0.001) {
            return 0.0;  // In shadow
        }
        res = min(res, k * h / t);
        t += h;
        if (t > maxt) {
            break;
        }
    }

    return clamp(res, 0.0, 1.0);
}

// Ambient occlusion using sphere marching
// p: surface point
// n: surface normal
// Returns AO factor (0.0 = fully occluded, 1.0 = no occlusion)
fn ambient_occlusion(p: vec3f, n: vec3f) -> f32 {
    var occ = 0.0;
    var sca = 1.0;

    for (var i = 0; i < 5; i++) {
        let h = 0.01 + 0.12 * f32(i);
        let d = sdf_scene(p + n * h);
        occ += (h - d) * sca;
        sca *= 0.95;
    }

    return clamp(1.0 - 3.0 * occ, 0.0, 1.0);
}

// Fresnel effect for rim lighting
// normal: surface normal
// view_dir: direction from surface to camera
// power: fresnel exponent (higher = narrower rim)
fn fresnel(normal: vec3f, view_dir: vec3f, power: f32) -> f32 {
    return pow(1.0 - max(dot(normal, normalize(view_dir)), 0.0), power);
}

// Sky/environment light contribution (simple hemisphere)
// normal: surface normal
// sky_color: color of sky (up direction)
// ground_color: color of ground bounce
fn hemisphere_light(normal: vec3f, sky_color: vec3f, ground_color: vec3f) -> vec3f {
    let blend = normal.y * 0.5 + 0.5;
    return mix(ground_color, sky_color, blend);
}
