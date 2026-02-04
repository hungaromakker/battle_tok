// Sketch Engine - Noise Utility Shader
// Hash, noise, and FBM functions for procedural generation

// --- Hash Functions ---

// 3D to 1D hash function
fn hash(p: vec3f) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += dot(p3, p3.zyx + 31.32);
    return fract((p3.x + p3.y) * p3.z);
}

// 3D to 3D hash function for gradient noise
fn hash33(p: vec3f) -> vec3f {
    var p3 = fract(p * vec3f(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yxz + 33.33);
    return fract((p3.xxy + p3.yxx) * p3.zyx);
}

// 2D to 1D hash function
fn hash21(p: vec2f) -> f32 {
    var p3 = fract(vec3f(p.xyx) * 0.1031);
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

// 2D to 2D hash function
fn hash22(p: vec2f) -> vec2f {
    var p3 = fract(vec3f(p.xyx) * vec3f(0.1031, 0.1030, 0.0973));
    p3 += dot(p3, p3.yzx + 33.33);
    return fract((p3.xx + p3.yz) * p3.zy);
}

// 1D to 1D hash function
fn hash11(p: f32) -> f32 {
    var p3 = fract(p * 0.1031);
    p3 += p3 * (p3 + 33.33);
    return fract(p3 * p3);
}

// --- Gradient Noise (Perlin-like) ---

// 3D Gradient noise with smoothstep interpolation
fn noise(p: vec3f) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Smoothstep interpolation (3t^2 - 2t^3)
    let u = f * f * (3.0 - 2.0 * f);

    // Sample gradients at 8 corners and compute dot products
    return mix(
        mix(
            mix(dot(hash33(i + vec3f(0.0, 0.0, 0.0)) * 2.0 - 1.0, f - vec3f(0.0, 0.0, 0.0)),
                dot(hash33(i + vec3f(1.0, 0.0, 0.0)) * 2.0 - 1.0, f - vec3f(1.0, 0.0, 0.0)), u.x),
            mix(dot(hash33(i + vec3f(0.0, 1.0, 0.0)) * 2.0 - 1.0, f - vec3f(0.0, 1.0, 0.0)),
                dot(hash33(i + vec3f(1.0, 1.0, 0.0)) * 2.0 - 1.0, f - vec3f(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(dot(hash33(i + vec3f(0.0, 0.0, 1.0)) * 2.0 - 1.0, f - vec3f(0.0, 0.0, 1.0)),
                dot(hash33(i + vec3f(1.0, 0.0, 1.0)) * 2.0 - 1.0, f - vec3f(1.0, 0.0, 1.0)), u.x),
            mix(dot(hash33(i + vec3f(0.0, 1.0, 1.0)) * 2.0 - 1.0, f - vec3f(0.0, 1.0, 1.0)),
                dot(hash33(i + vec3f(1.0, 1.0, 1.0)) * 2.0 - 1.0, f - vec3f(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

// 2D Value Noise
fn noise2d(p: vec2f) -> f32 {
    let i = floor(p);
    let f = fract(p);

    let u = f * f * (3.0 - 2.0 * f);

    return mix(
        mix(hash21(i + vec2f(0.0, 0.0)), hash21(i + vec2f(1.0, 0.0)), u.x),
        mix(hash21(i + vec2f(0.0, 1.0)), hash21(i + vec2f(1.0, 1.0)), u.x),
        u.y
    );
}

// 3D Gradient noise with quintic interpolation (smoother but more expensive)
fn gradient_noise_quintic(p: vec3f) -> f32 {
    let i = floor(p);
    let f = fract(p);

    // Quintic interpolation for smoother results
    let u = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);

    return mix(
        mix(
            mix(dot(hash33(i + vec3f(0.0, 0.0, 0.0)) * 2.0 - 1.0, f - vec3f(0.0, 0.0, 0.0)),
                dot(hash33(i + vec3f(1.0, 0.0, 0.0)) * 2.0 - 1.0, f - vec3f(1.0, 0.0, 0.0)), u.x),
            mix(dot(hash33(i + vec3f(0.0, 1.0, 0.0)) * 2.0 - 1.0, f - vec3f(0.0, 1.0, 0.0)),
                dot(hash33(i + vec3f(1.0, 1.0, 0.0)) * 2.0 - 1.0, f - vec3f(1.0, 1.0, 0.0)), u.x),
            u.y
        ),
        mix(
            mix(dot(hash33(i + vec3f(0.0, 0.0, 1.0)) * 2.0 - 1.0, f - vec3f(0.0, 0.0, 1.0)),
                dot(hash33(i + vec3f(1.0, 0.0, 1.0)) * 2.0 - 1.0, f - vec3f(1.0, 0.0, 1.0)), u.x),
            mix(dot(hash33(i + vec3f(0.0, 1.0, 1.0)) * 2.0 - 1.0, f - vec3f(0.0, 1.0, 1.0)),
                dot(hash33(i + vec3f(1.0, 1.0, 1.0)) * 2.0 - 1.0, f - vec3f(1.0, 1.0, 1.0)), u.x),
            u.y
        ),
        u.z
    );
}

// --- Fractal Brownian Motion (FBM) ---

// Standard FBM with configurable octaves
fn fbm(p: vec3f, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var pos = p;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise(pos * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    return value;
}

// FBM with custom parameters
fn fbm_params(p: vec3f, octaves: i32, lacunarity: f32, gain: f32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * noise(p * frequency);
        amplitude *= gain;
        frequency *= lacunarity;
    }

    return value;
}

// FBM with quintic interpolation for smoother results
fn fbm_quintic(p: vec3f, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * gradient_noise_quintic(p * frequency);
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    return value;
}

// Ridged FBM for mountain-like terrain
fn fbm_ridged(p: vec3f, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;
    var prev = 1.0;

    for (var i = 0; i < octaves; i++) {
        let n = abs(noise(p * frequency) * 2.0 - 1.0);
        let ridge = 1.0 - n;
        let weighted = ridge * ridge * prev;
        value += weighted * amplitude;
        prev = ridge;
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    return value;
}

// Turbulence (absolute value noise)
fn turbulence(p: vec3f, octaves: i32) -> f32 {
    var value = 0.0;
    var amplitude = 0.5;
    var frequency = 1.0;

    for (var i = 0; i < octaves; i++) {
        value += amplitude * abs(noise(p * frequency) * 2.0 - 1.0);
        amplitude *= 0.5;
        frequency *= 2.0;
    }

    return value;
}

// --- Voronoi / Cellular Noise ---

// Basic Voronoi returning distance to nearest cell
fn voronoi(p: vec3f) -> f32 {
    let i = floor(p);
    let f = fract(p);

    var min_dist = 1.0;

    for (var z = -1; z <= 1; z++) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let neighbor = vec3f(f32(x), f32(y), f32(z));
                let cell_pos = hash33(i + neighbor);
                let diff = neighbor + cell_pos - f;
                let dist = length(diff);
                min_dist = min(min_dist, dist);
            }
        }
    }

    return min_dist;
}

// Voronoi returning both F1 (nearest) and F2 (second nearest) distances
fn voronoi_f1f2(p: vec3f) -> vec2f {
    let i = floor(p);
    let f = fract(p);

    var f1 = 1.0;
    var f2 = 1.0;

    for (var z = -1; z <= 1; z++) {
        for (var y = -1; y <= 1; y++) {
            for (var x = -1; x <= 1; x++) {
                let neighbor = vec3f(f32(x), f32(y), f32(z));
                let cell_pos = hash33(i + neighbor);
                let diff = neighbor + cell_pos - f;
                let dist = length(diff);

                if (dist < f1) {
                    f2 = f1;
                    f1 = dist;
                } else if (dist < f2) {
                    f2 = dist;
                }
            }
        }
    }

    return vec2f(f1, f2);
}

// --- Domain Warping ---

// Simple domain warp
fn domain_warp(p: vec3f, strength: f32) -> vec3f {
    let offset = vec3f(
        noise(p),
        noise(p + vec3f(5.2, 1.3, 2.8)),
        noise(p + vec3f(2.1, 7.3, 4.5))
    );
    return p + offset * strength;
}

// FBM-based domain warp for more complex warping
fn domain_warp_fbm(p: vec3f, strength: f32, octaves: i32) -> vec3f {
    let offset = vec3f(
        fbm(p, octaves),
        fbm(p + vec3f(5.2, 1.3, 2.8), octaves),
        fbm(p + vec3f(2.1, 7.3, 4.5), octaves)
    );
    return p + offset * strength;
}
