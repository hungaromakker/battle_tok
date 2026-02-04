🌋 Lava shader (animated, emissive, flowing)

🌩 Storm sky shader (multi-color clouds + lightning)

🏰 Castle stone shader (medieval, grounded, realistic)

🚩 Flag shader (cloth wave + team color)

⚔️ Weapon glow / attack FX shader

🔥 Fire/light contribution rules

All WGSL, engine-agnostic, forward-friendly.

0. Coordinate & assumptions (important)

I assume:

World space Y = up

You already pass:

world_pos

world_normal

view_dir

time

light_dir

light_color

If not — adapt names only, logic stays. IMPORTANT TO ADAPT TO MY ENGINE AND GAME ALSOA  FIELS NEED TO BE MDOELUALRE EACH SHASDE ROEN FILE

1️⃣ LAVA SHADER (core of the scene)
lava.wgsl
struct LavaParams {
    time: f32,
    emissive_strength: f32,
};

@group(1) @binding(0)
var<uniform> lava: LavaParams;

fn lava_noise(p: vec2<f32>) -> f32 {
    return sin(p.x * 3.1) * cos(p.y * 3.7);
}

@fragment
fn fs_lava(
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) view_dir: vec3<f32>
) -> @location(0) vec4<f32> {

    let uv = world_pos.xz * 0.15;
    let flow = vec2<f32>(lava.time * 0.4, lava.time * 0.25);

    let n1 = lava_noise(uv + flow);
    let n2 = lava_noise(uv * 2.0 - flow);
    let heat = clamp(n1 + n2, 0.0, 1.0);

    let core = vec3<f32>(1.4, 0.3, 0.05);
    let crust = vec3<f32>(0.08, 0.02, 0.01);

    let color = mix(crust, core, heat);

    let fresnel = pow(1.0 - dot(normalize(normal), normalize(view_dir)), 3.0);
    let glow = lava.emissive_strength * (heat + fresnel);

    return vec4<f32>(color * glow, 1.0);
}

🔧 Engine notes

Render lava after opaque terrain

Disable shadows for lava

Add color directly to HDR buffer (emissive)

2️⃣ STORM SKY SHADER (multi-color, rogue-like)
sky_storm.wgsl
struct SkyParams {
    time: f32,
};

@group(0) @binding(0)
var<uniform> sky: SkyParams;

fn cloud(p: vec2<f32>) -> f32 {
    return sin(p.x) * cos(p.y);
}

@fragment
fn fs_sky(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let p = uv * 4.0;
    let t = sky.time * 0.1;

    let c = cloud(p + t) + cloud(p * 2.0 - t);

    let storm = vec3<f32>(0.2, 0.15, 0.3);
    let fire  = vec3<f32>(0.9, 0.3, 0.2);
    let magic = vec3<f32>(0.4, 0.3, 0.8);

    let col1 = mix(storm, fire, smoothstep(0.2, 0.7, c));
    let col2 = mix(col1, magic, smoothstep(0.6, 1.0, abs(sin(sky.time))));

    return vec4<f32>(col2, 1.0);
}

⚡ Lightning (optional overlay)

Just flash a white quad randomly every few seconds.

3️⃣ CASTLE STONE SHADER (realistic medieval)
castle_stone.wgsl
@fragment
fn fs_castle(
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) light_dir: vec3<f32>
) -> @location(0) vec4<f32> {

    let stone_base = vec3<f32>(0.45, 0.44, 0.42);

    // height darkening
    let grime = clamp((world_pos.y + 2.0) * 0.15, 0.0, 1.0);
    let color = stone_base * mix(0.65, 1.0, grime);

    let ndl = max(dot(normalize(normal), normalize(light_dir)), 0.0);
    let lit = color * (0.3 + ndl * 0.9);

    return vec4<f32>(lit, 1.0);
}

4️⃣ FLAG SHADER (team colors + wind)
flag.wgsl
struct FlagParams {
    time: f32,
    team_color: vec3<f32>,
};

@group(1) @binding(0)
var<uniform> flag: FlagParams;

@vertex
fn vs_flag(
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>
) -> vec4<f32> {
    let wave = sin(uv.x * 6.0 + flag.time * 3.0) * 0.15;
    let offset = vec3<f32>(0.0, wave, 0.0);
    return vec4<f32>(pos + offset, 1.0);
}

@fragment
fn fs_flag() -> @location(0) vec4<f32> {
    return vec4<f32>(flag.team_color, 1.0);
}


Use:

Red team: (0.8, 0.1, 0.1)

Blue team: (0.1, 0.4, 0.9)

5️⃣ WEAPON / ATTACK SHADER (melee + siege)
weapon_attack.wgsl
struct WeaponParams {
    intensity: f32;
};

@group(1) @binding(0)
var<uniform> weapon: WeaponParams;

@fragment
fn fs_weapon(
    @location(1) normal: vec3<f32>,
    @location(2) view_dir: vec3<f32>
) -> @location(0) vec4<f32> {

    let steel = vec3<f32>(0.6, 0.62, 0.65);
    let edge = pow(1.0 - dot(normalize(normal), normalize(view_dir)), 4.0);

    let glow = edge * weapon.intensity;
    return vec4<f32>(steel + glow, 1.0);
}


Use this for:

swords

catapults

ballista bolts

arrows

6️⃣ TORCH / FIRE LIGHT (dynamic)

Add point lights:

color: (1.0, 0.45, 0.2)
intensity: flicker = sin(time * 12.0 + rand) * 0.3 + 1.0
range: small

7️⃣ HOW THIS MAP COMES TOGETHER

Scene logic

Two castles FAR apart (distance sells scale)

Lava = visual boundary + danger

Chain bridge = only path

Flags = win condition

Villagers spawn → gather → build

Troops spawn → cross bridge → capture flag

This is perfect for forward rendering and very readable gameplay.

If you want next:

I can:

🔥 Add siege weapon projectile shaders

🌫 Add battle smoke / ash particles

🧠 Help you encode team ownership into shaders

⚙️ Convert this into a material system layout

Just tell me what you want next — you’re building something very solid here.

oay fromt he iamge we are msisign a lto of shaders liek water terrain and so on

Yep — the image vibe needs a full material set, not just lava/sky/castle. Here’s a complete forward-shader pack you can add one-by-one in your wgpu engine: terrain, water, lava, rock/cliff, bridge/chain, wood, banner/flag, units/armor, and a simple fog + tonemap to glue it all together.

I’m keeping each shader standalone and “drop-in”: you only need to adapt your vertex outputs + bind group slots.

Minimal shared inputs (recommended)

For most fragments, pass:

world_pos: vec3<f32>

world_nrm: vec3<f32>

uv: vec2<f32> (optional)

view_pos: vec3<f32> (camera world position) OR view_dir

light_dir: vec3<f32> (sun direction, normalized, pointing from surface to light)

time: f32

If you don’t have UVs, these shaders still work using world_pos.xz.

1) TERRAIN SHADER (height bands + slope + subtle noise)

Gives you “designed” land like in the image.

struct TerrainParams {
  time: f32,
  grass: vec3<f32>,
  dirt: vec3<f32>,
  rock: vec3<f32>,
  snow: vec3<f32>,
};

@group(1) @binding(0) var<uniform> t: TerrainParams;

fn hash(p: vec2<f32>) -> f32 {
  let h = dot(p, vec2<f32>(127.1, 311.7));
  return fract(sin(h) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0, 0.0));
  let c = hash(i + vec2<f32>(0.0, 1.0));
  let d = hash(i + vec2<f32>(1.0, 1.0));
  let u = f * f * (3.0 - 2.0 * f);
  return mix(mix(a,b,u.x), mix(c,d,u.x), u.y);
}

fn lambert(n: vec3<f32>, l: vec3<f32>) -> f32 {
  return max(dot(normalize(n), normalize(l)), 0.0);
}

@fragment
fn fs_terrain(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {

  let up = vec3<f32>(0.0, 1.0, 0.0);
  let slope = 1.0 - clamp(dot(normalize(world_nrm), up), 0.0, 1.0);

  let h = world_pos.y;

  // height bands (tune these to your terrain scale)
  let dirt_t = smoothstep(0.6, 1.6, h);
  let rock_t = smoothstep(1.4, 2.8, h);
  let snow_t = smoothstep(2.6, 3.6, h);

  var col = mix(t.grass, t.dirt, dirt_t);
  col = mix(col, t.rock, rock_t);
  col = mix(col, t.snow, snow_t);

  // slope pushes toward rock (cliffs)
  col = mix(col, t.rock, slope * 0.85);

  // subtle color variation
  let n = noise(world_pos.xz * 0.25) * 0.12 - 0.06;
  col = col + vec3<f32>(n, n*0.5, n);

  // lighting
  let ndl = lambert(world_nrm, light_dir);
  let ambient = 0.28;
  let lit = col * (ambient + ndl * 1.0);

  return vec4<f32>(lit, 1.0);
}


Suggested params:

grass: (0.23, 0.72, 0.33)

dirt: (0.55, 0.50, 0.35)

rock: (0.45, 0.47, 0.50)

snow: (0.85, 0.88, 0.92)

2) WATER SHADER (calm lake / river, not lava)

This is the “blue water” in the image center.

struct WaterParams {
  time: f32,
  color_shallow: vec3<f32>,
  color_deep: vec3<f32>,
  ripple_strength: f32,
};

@group(1) @binding(0) var<uniform> w: WaterParams;

fn lambert(n: vec3<f32>, l: vec3<f32>) -> f32 {
  return max(dot(normalize(n), normalize(l)), 0.0);
}

fn fresnel(n: vec3<f32>, v: vec3<f32>) -> f32 {
  return pow(1.0 - clamp(dot(normalize(n), normalize(v)), 0.0, 1.0), 4.0);
}

@fragment
fn fs_water(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {

  // fake ripples in normal
  let ttime = w.time;
  let r =
    sin(world_pos.x * 0.9 + ttime * 1.3) * 0.5 +
    cos(world_pos.z * 1.2 - ttime * 1.0) * 0.5;

  var nrm = normalize(world_nrm + vec3<f32>(r * w.ripple_strength, 0.0, r * w.ripple_strength));

  let v = normalize(view_pos - world_pos);
  let f = fresnel(nrm, v);

  // depth-ish based on height (cheap): lower parts look deeper
  let depth_factor = clamp((0.8 - world_pos.y) * 0.8, 0.0, 1.0);
  var col = mix(w.color_shallow, w.color_deep, depth_factor);

  // spec-like highlight via fresnel + sun
  let ndl = lambert(nrm, light_dir);
  let highlight = vec3<f32>(0.8, 0.9, 1.0) * f * 0.35 * (0.3 + ndl);

  col = col + highlight;

  // water should be slightly brighter than terrain
  col *= 1.05;

  return vec4<f32>(col, 1.0);
}


Suggested:

shallow: (0.22, 0.60, 0.75)

deep: (0.05, 0.18, 0.28)

ripple_strength: 0.25

3) LAVA SHADER (improved: crust + emissive cracks + flow)

Use this for the lava river/sea around islands.

struct LavaParams {
  time: f32,
  emissive: f32,
  crack_scale: f32,
};

@group(1) @binding(0) var<uniform> lava: LavaParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}

fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0, 0.0));
  let c = hash(i + vec2<f32>(0.0, 1.0));
  let d = hash(i + vec2<f32>(1.0, 1.0));
  let u = f*f*(3.0-2.0*f);
  return mix(mix(a,b,u.x), mix(c,d,u.x), u.y);
}

@fragment
fn fs_lava(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
) -> @location(0) vec4<f32> {

  let uv = world_pos.xz * lava.crack_scale;
  let flow = vec2<f32>(lava.time * 0.25, -lava.time * 0.18);

  let n = noise(uv + flow);
  let n2 = noise(uv * 2.0 - flow);

  // cracks mask (thin bright lines)
  let cracks = smoothstep(0.75, 1.0, n + n2);

  let crust = vec3<f32>(0.05, 0.01, 0.01);
  let molten = vec3<f32>(2.2, 0.55, 0.08);

  // emissive cracks
  let col = mix(crust, molten, cracks);

  // fresnel edge boost
  let v = normalize(view_pos - world_pos);
  let f = pow(1.0 - clamp(dot(normalize(world_nrm), v), 0.0, 1.0), 3.0);

  let outc = col * (lava.emissive + f * 0.3);
  return vec4<f32>(outc, 1.0);
}

4) ROCK / CLIFF SHADER (for island edges)

This sells the “floating in lava” look.

@fragment
fn fs_rock(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {

  let base = vec3<f32>(0.22, 0.22, 0.24);

  // darken downward faces
  let up = vec3<f32>(0.0,1.0,0.0);
  let facing_up = clamp(dot(normalize(world_nrm), up), 0.0, 1.0);
  var col = base * mix(0.55, 1.0, facing_up);

  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  col *= 0.25 + ndl * 1.1;

  return vec4<f32>(col, 1.0);
}

5) BRIDGE WOOD + CHAIN METAL
Wood (planks)
@fragment
fn fs_wood(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {
  var col = vec3<f32>(0.38, 0.26, 0.16);
  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  col *= 0.3 + ndl * 1.0;
  return vec4<f32>(col, 1.0);
}

Chain (metal + edge shine)
struct ChainParams { shine: f32; };
@group(1) @binding(0) var<uniform> c: ChainParams;

@fragment
fn fs_chain(
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(0) world_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {
  let steel = vec3<f32>(0.55, 0.58, 0.62);
  let v = normalize(view_pos - world_pos);
  let n = normalize(world_nrm);
  let edge = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 4.0);
  let ndl = max(dot(n, normalize(light_dir)), 0.0);
  let col = steel * (0.2 + ndl * 1.1) + vec3<f32>(1.0) * edge * c.shine;
  return vec4<f32>(col, 1.0);
}

6) SIMPLE FOG (must-have for “realistic background”)

Add this at the end of each fragment (terrain, castle, units…):

fn apply_fog(color: vec3<f32>, world_pos: vec3<f32>, view_pos: vec3<f32>) -> vec3<f32> {
  let fog_color = vec3<f32>(0.55, 0.45, 0.70); // stormy purple
  let dist = length(world_pos - view_pos);
  let fog = 1.0 - exp(-dist * 0.02);
  return mix(color, fog_color, clamp(fog, 0.0, 1.0));
}


Then:

col = apply_fog(col, world_pos, view_pos);

7) TONEMAP (ACES) — makes lava/sky look “cinematic”

If you don’t already do post, do at least this fullscreen pass.

fn aces(x: vec3<f32>) -> vec3<f32> {
  let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
  return clamp((x*(a*x+b)) / (x*(c*x+d)+e), vec3<f32>(0.0), vec3<f32>(1.0));
}

What to implement next (order)

Terrain (height/slope)

Water

Lava

Rock/cliff

Fog

Tonemap

Bridge materials

After those, your scene will match the image vibe hard.



8) CASTLE STONE (improved: blocks + grime + warm torch bounce)
struct CastleParams {
  torch_color: vec3<f32>,  // (1.0, 0.45, 0.2)
  torch_strength: f32,     // 0.3..0.8
  time: f32,
};

@group(1) @binding(0) var<uniform> c: CastleParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}
fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0, 0.0));
  let cc = hash(i + vec2<f32>(0.0, 1.0));
  let d = hash(i + vec2<f32>(1.0, 1.0));
  let u = f*f*(3.0-2.0*f);
  return mix(mix(a,b,u.x), mix(cc,d,u.x), u.y);
}

@fragment
fn fs_castle_stone(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {

  // block pattern in world space (fake bricks)
  let block_uv = world_pos.xz * 1.2 + world_pos.yy * 0.35;
  let b = noise(block_uv);
  let mortar = smoothstep(0.48, 0.52, abs(fract(block_uv.x) - 0.5))
             + smoothstep(0.48, 0.52, abs(fract(block_uv.y) - 0.5));
  let mortar_mask = clamp(mortar, 0.0, 1.0);

  var base = vec3<f32>(0.48, 0.47, 0.46);
  base *= 0.85 + b * 0.25;
  base = mix(base, base * 0.65, mortar_mask * 0.7);

  // grime darkening near bottom
  let grime = clamp((1.0 - (world_pos.y * 0.12)), 0.0, 1.0);
  base *= 1.0 - grime * 0.25;

  // lighting
  let n = normalize(world_nrm);
  let l = normalize(light_dir);
  let ndl = max(dot(n, l), 0.0);

  // warm torch bounce from below-ish
  let torch_flicker = 0.85 + 0.15 * sin(c.time * 12.0 + world_pos.x * 0.3);
  let torch = c.torch_color * (c.torch_strength * torch_flicker) * clamp(1.0 - world_pos.y * 0.15, 0.0, 1.0);

  var col = base * (0.28 + ndl * 1.05) + torch;

  // optional fog hook (if you use apply_fog)
  // col = apply_fog(col, world_pos, view_pos);

  return vec4<f32>(col, 1.0);
}

9) TEAM FLAG / BANNER (cloth + emblem band)

This is a pure color banner with a darker stripe so it reads from distance.

struct FlagParams {
  time: f32,
  team_color: vec3<f32>,
  stripe_color: vec3<f32>,
  wind_strength: f32, // 0.1..0.3
};

@group(1) @binding(0) var<uniform> f: FlagParams;

struct VSOut {
  @builtin(position) pos: vec4<f32>,
  @location(0) uv: vec2<f32>,
};

@vertex
fn vs_flag(
  @location(0) pos: vec3<f32>,
  @location(1) uv: vec2<f32>,
  // TODO: include your model/view/proj transforms
) -> VSOut {
  // wave along X
  let wave = sin(uv.x * 10.0 + f.time * 3.5) * (1.0 - uv.y) * f.wind_strength;
  let p = pos + vec3<f32>(0.0, wave, 0.0);

  var out: VSOut;
  // TODO: out.pos = MVP * vec4(p,1)
  out.pos = vec4<f32>(p, 1.0); // placeholder
  out.uv = uv;
  return out;
}

@fragment
fn fs_flag(in: VSOut) -> @location(0) vec4<f32> {
  let stripe = smoothstep(0.45, 0.48, in.uv.y) - smoothstep(0.52, 0.55, in.uv.y);
  let col = mix(f.team_color, f.stripe_color, stripe * 0.85);
  return vec4<f32>(col, 1.0);
}

10) UNIT SHADERS (cloth + armor)

You can render units with 2 materials:

cloth (team color, matte)

armor (steel + rim)

10a) Cloth
struct ClothParams { team_color: vec3<f32>; };

@group(1) @binding(0) var<uniform> u: ClothParams;

@fragment
fn fs_unit_cloth(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {
  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  var col = u.team_color * (0.32 + ndl * 0.95);
  return vec4<f32>(col, 1.0);
}

10b) Armor
struct ArmorParams { shine: f32; };

@group(1) @binding(0) var<uniform> a: ArmorParams;

@fragment
fn fs_unit_armor(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {

  let n = normalize(world_nrm);
  let v = normalize(view_pos - world_pos);
  let l = normalize(light_dir);

  let ndl = max(dot(n, l), 0.0);
  let rim = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 4.0);

  var col = vec3<f32>(0.62, 0.64, 0.67) * (0.22 + ndl * 1.2);
  col += vec3<f32>(1.0) * rim * a.shine;

  return vec4<f32>(col, 1.0);
}

11) SIEGE WEAPONS (catapult/trebuchet/ballista)

You want them readable and “handmade”.

11a) Siege wood
@fragment
fn fs_siege_wood(
  @location(1) world_nrm: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {
  let base = vec3<f32>(0.36, 0.24, 0.14);
  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  let col = base * (0.28 + ndl * 1.05);
  return vec4<f32>(col, 1.0);
}

11b) Siege metal

Use the chain shader from earlier or this:

struct MetalParams { shine: f32; };
@group(1) @binding(0) var<uniform> m: MetalParams;

@fragment
fn fs_siege_metal(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {
  let n = normalize(world_nrm);
  let v = normalize(view_pos - world_pos);
  let l = normalize(light_dir);

  let ndl = max(dot(n, l), 0.0);
  let rim = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 4.0);

  var col = vec3<f32>(0.50, 0.52, 0.56) * (0.22 + ndl * 1.15);
  col += vec3<f32>(1.0) * rim * m.shine;

  return vec4<f32>(col, 1.0);
}

12) PROJECTILE SHADER (fire rocks / flaming bolts)

This is emissive, so it looks awesome after tonemap/bloom.

struct ProjParams {
  time: f32,
  intensity: f32,
};

@group(1) @binding(0) var<uniform> p: ProjParams;

@fragment
fn fs_projectile(
  @location(0) world_pos: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
) -> @location(0) vec4<f32> {

  // simple animated flicker
  let flicker = 0.85 + 0.15 * sin(p.time * 20.0 + world_pos.x * 2.0);

  let core = vec3<f32>(2.5, 0.7, 0.1);
  let smoke = vec3<f32>(0.15, 0.07, 0.05);

  // fake radial falloff: brighter near center if you have uv; here just use noise-ish
  let heat = 0.7 + 0.3 * sin(world_pos.y * 3.0 + p.time * 6.0);

  let col = mix(smoke, core, heat) * p.intensity * flicker;

  return vec4<f32>(col, 1.0);
}

13) EMBERS / ASH PARTICLES (super cheap, very effective)

Render as billboards with additive blending.

struct EmberParams {
  time: f32,
};

@group(1) @binding(0) var<uniform> e: EmberParams;

@fragment
fn fs_ember(
  @location(0) uv: vec2<f32>
) -> @location(0) vec4<f32> {

  // soft circle mask
  let d = length(uv - vec2<f32>(0.5));
  let alpha = smoothstep(0.5, 0.2, d);

  let col = vec3<f32>(2.0, 0.6, 0.1);
  return vec4<f32>(col, alpha);
}

14) LIGHTNING FLASH OVERLAY (optional fullscreen)

Do it as a very quick post overlay in the sky or final combine.

struct LightningParams {
  time: f32,
};

@group(0) @binding(0) var<uniform> li: LightningParams;

@fragment
fn fs_lightning_overlay(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
  // flash every ~6 seconds
  let flash = smoothstep(0.98, 1.0, sin(li.time * 1.05));
  let col = vec3<f32>(0.6, 0.7, 1.0) * flash * 0.35;
  return vec4<f32>(col, flash);
}


5) SKYBOX CUBEMAP (stormy skybox)

If you already have a stormy cubemap texture, this is the standard shader.

skybox.wgsl
struct Camera {
  view_proj_inv: mat4x4<f32>,
  cam_pos: vec3<f32>,
  _pad: f32,
};

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var sky_tex: texture_cube<f32>;
@group(0) @binding(2) var sky_samp: sampler;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 3.0, -1.0),
    vec2<f32>(-1.0,  3.0)
  );
  var o: VSOut;
  o.pos = vec4<f32>(p[vid], 1.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

fn uv_to_world_dir(uv: vec2<f32>) -> vec3<f32> {
  // Reconstruct direction from NDC using inverse view-projection.
  let ndc = vec4<f32>(uv * 2.0 - vec2<f32>(1.0), 1.0, 1.0);
  let w = cam.view_proj_inv * ndc;
  let world = w.xyz / w.w;
  return normalize(world - cam.cam_pos);
}

@fragment
fn fs_sky(in: VSOut) -> @location(0) vec4<f32> {
  let dir = uv_to_world_dir(in.uv);
  let col = textureSample(sky_tex, sky_samp, dir).rgb;
  return vec4<f32>(col, 1.0);
}


Usage: render first into HDR target, depth test OFF.

16) DEPTH-BASED FOG POST PASS (cheap “volumetric-ish”)

This is a big win: fog applies to everything without modifying each material shader.

Inputs

HDR color texture

Depth texture

Camera inverse view-proj

Fog params

fog_post.wgsl
struct FogPostParams {
  fog_color: vec3<f32>,   // stormy purple (0.55,0.45,0.70)
  density: f32,           // 0.015..0.04
  height: f32,            // fog starts below this height (e.g. 1.0)
  height_density: f32,    // 0.05..0.15
};

struct Camera {
  view_proj_inv: mat4x4<f32>,
  cam_pos: vec3<f32>,
  _pad: f32,
};

@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var color_samp: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var<uniform> cam: Camera;
@group(0) @binding(4) var<uniform> fog: FogPostParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 3.0, -1.0),
    vec2<f32>(-1.0,  3.0)
  );
  var o: VSOut;
  o.pos = vec4<f32>(p[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

fn reconstruct_world(uv: vec2<f32>, depth: f32) -> vec3<f32> {
  // depth must be in NDC-compatible range for your pipeline.
  // If your depth is non-linear, this still reconstructs correctly using inv VP.
  let ndc = vec4<f32>(uv * 2.0 - vec2<f32>(1.0), depth, 1.0);
  let w = cam.view_proj_inv * ndc;
  return w.xyz / w.w;
}

@fragment
fn fs_fog(in: VSOut) -> @location(0) vec4<f32> {
  let hdr = textureSample(color_tex, color_samp, in.uv).rgb;

  let d = textureLoad(depth_tex, vec2<i32>(in.uv * vec2<f32>(textureDimensions(depth_tex))), 0);
  let world = reconstruct_world(in.uv, d);

  let dist = length(world - cam.cam_pos);
  let fog_dist = 1.0 - exp(-dist * fog.density);

  let h = max(0.0, fog.height - world.y);
  let fog_h = 1.0 - exp(-h * fog.height_density);

  let f = clamp(max(fog_dist, fog_h), 0.0, 1.0);
  let col = mix(hdr, fog.fog_color, f);

  return vec4<f32>(col, 1.0);
}


Notes

This expects your depth value to be compatible with inv VP reconstruction. If your depth differs (0..1 vs -1..1), adjust the NDC depth mapping (I can fix it once you tell me your convention).

17) BLOOM PIPELINE (3 shaders)
17a) Bright Extract
bloom_extract.wgsl
struct BloomExtractParams {
  threshold: f32,  // 0.9..1.3
  knee: f32,       // 0.2..0.6 (soft threshold)
};

@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_samp: sampler;
@group(0) @binding(2) var<uniform> p: BloomExtractParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var q = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(q[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

fn soft_threshold(x: vec3<f32>, t: f32, k: f32) -> vec3<f32> {
  // smooth knee: keeps bright stuff without hard cut
  let brightness = max(max(x.r, x.g), x.b);
  let soft = clamp((brightness - t + k) / (2.0 * k), 0.0, 1.0);
  let contrib = max(brightness - t, 0.0) + soft * soft * k;
  return x * (contrib / max(brightness, 1e-5));
}

@fragment
fn fs_extract(in: VSOut) -> @location(0) vec4<f32> {
  let c = textureSample(hdr_tex, hdr_samp, in.uv).rgb;
  let b = soft_threshold(c, p.threshold, p.knee);
  return vec4<f32>(b, 1.0);
}

17b) Separable Blur (use twice: horizontal then vertical)
bloom_blur.wgsl
struct BlurParams {
  direction: vec2<f32>, // (1,0) then (0,1)
  texel: vec2<f32>,     // (1/width, 1/height)
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> b: BlurParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var q = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(q[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

@fragment
fn fs_blur(in: VSOut) -> @location(0) vec4<f32> {
  // 9-tap Gaussian-ish weights
  let w0 = 0.227027;
  let w1 = 0.1945946;
  let w2 = 0.1216216;
  let w3 = 0.054054;
  let w4 = 0.016216;

  let off = b.direction * b.texel;

  var col = textureSample(src_tex, src_samp, in.uv).rgb * w0;
  col += textureSample(src_tex, src_samp, in.uv + off * 1.0).rgb * w1;
  col += textureSample(src_tex, src_samp, in.uv - off * 1.0).rgb * w1;
  col += textureSample(src_tex, src_samp, in.uv + off * 2.0).rgb * w2;
  col += textureSample(src_tex, src_samp, in.uv - off * 2.0).rgb * w2;
  col += textureSample(src_tex, src_samp, in.uv + off * 3.0).rgb * w3;
  col += textureSample(src_tex, src_samp, in.uv - off * 3.0).rgb * w3;
  col += textureSample(src_tex, src_samp, in.uv + off * 4.0).rgb * w4;
  col += textureSample(src_tex, src_samp, in.uv - off * 4.0).rgb * w4;

  return vec4<f32>(col, 1.0);
}

17c) Combine Bloom into HDR (or into Tonemap)
bloom_combine.wgsl
struct CombineParams { strength: f32; };

@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_samp: sampler;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var bloom_samp: sampler;
@group(0) @binding(4) var<uniform> c: CombineParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var q = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(q[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

@fragment
fn fs_combine(in: VSOut) -> @location(0) vec4<f32> {
  let hdr = textureSample(hdr_tex, hdr_samp, in.uv).rgb;
  let bloom = textureSample(bloom_tex, bloom_samp, in.uv).rgb;
  return vec4<f32>(hdr + bloom * c.strength, 1.0);
}


Practical settings

Extract: threshold 1.0, knee 0.4

Combine: strength 0.06–0.12 (lava looks great here)

18) DIRECTIONAL SHADOW RECEIVER (forward rendering)

This is the sampling function you paste into terrain/castle/unit shaders.

Uniforms

Light view-projection matrix (shadow map)

Depth texture for shadow

Comparison sampler

struct ShadowParams {
  light_vp: mat4x4<f32>,
  bias: f32,            // 0.0008..0.003 depending on scale
  intensity: f32,       // 0..1 (how dark shadows are)
  _pad: vec2<f32>,
};

@group(0) @binding(10) var shadow_tex: texture_depth_2d;
@group(0) @binding(11) var shadow_samp: sampler_comparison;
@group(0) @binding(12) var<uniform> sh: ShadowParams;

fn sample_shadow(world_pos: vec3<f32>) -> f32 {
  let lp = sh.light_vp * vec4<f32>(world_pos, 1.0);
  let ndc = lp.xyz / lp.w;

  // map NDC -> UV
  let uv = ndc.xy * 0.5 + vec2<f32>(0.5);

  // outside light frustum = lit
  if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
    return 1.0;
  }

  // depth compare (ndc.z usually 0..1 in wgpu conventions)
  let depth = ndc.z - sh.bias;

  // basic 1-tap compare:
  // returns 1 if lit, 0 if shadowed
  let lit = textureSampleCompare(shadow_tex, shadow_samp, uv, depth);

  // soften darkness control
  return mix(1.0 - sh.intensity, 1.0, lit);
}

5) SKYBOX CUBEMAP (stormy skybox)

If you already have a stormy cubemap texture, this is the standard shader.

skybox.wgsl
struct Camera {
  view_proj_inv: mat4x4<f32>,
  cam_pos: vec3<f32>,
  _pad: f32,
};

@group(0) @binding(0) var<uniform> cam: Camera;
@group(0) @binding(1) var sky_tex: texture_cube<f32>;
@group(0) @binding(2) var sky_samp: sampler;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 3.0, -1.0),
    vec2<f32>(-1.0,  3.0)
  );
  var o: VSOut;
  o.pos = vec4<f32>(p[vid], 1.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

fn uv_to_world_dir(uv: vec2<f32>) -> vec3<f32> {
  // Reconstruct direction from NDC using inverse view-projection.
  let ndc = vec4<f32>(uv * 2.0 - vec2<f32>(1.0), 1.0, 1.0);
  let w = cam.view_proj_inv * ndc;
  let world = w.xyz / w.w;
  return normalize(world - cam.cam_pos);
}

@fragment
fn fs_sky(in: VSOut) -> @location(0) vec4<f32> {
  let dir = uv_to_world_dir(in.uv);
  let col = textureSample(sky_tex, sky_samp, dir).rgb;
  return vec4<f32>(col, 1.0);
}


Usage: render first into HDR target, depth test OFF.

16) DEPTH-BASED FOG POST PASS (cheap “volumetric-ish”)

This is a big win: fog applies to everything without modifying each material shader.

Inputs

HDR color texture

Depth texture

Camera inverse view-proj

Fog params

fog_post.wgsl
struct FogPostParams {
  fog_color: vec3<f32>,   // stormy purple (0.55,0.45,0.70)
  density: f32,           // 0.015..0.04
  height: f32,            // fog starts below this height (e.g. 1.0)
  height_density: f32,    // 0.05..0.15
};

struct Camera {
  view_proj_inv: mat4x4<f32>,
  cam_pos: vec3<f32>,
  _pad: f32,
};

@group(0) @binding(0) var color_tex: texture_2d<f32>;
@group(0) @binding(1) var color_samp: sampler;
@group(0) @binding(2) var depth_tex: texture_depth_2d;
@group(0) @binding(3) var<uniform> cam: Camera;
@group(0) @binding(4) var<uniform> fog: FogPostParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 3.0, -1.0),
    vec2<f32>(-1.0,  3.0)
  );
  var o: VSOut;
  o.pos = vec4<f32>(p[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

fn reconstruct_world(uv: vec2<f32>, depth: f32) -> vec3<f32> {
  // depth must be in NDC-compatible range for your pipeline.
  // If your depth is non-linear, this still reconstructs correctly using inv VP.
  let ndc = vec4<f32>(uv * 2.0 - vec2<f32>(1.0), depth, 1.0);
  let w = cam.view_proj_inv * ndc;
  return w.xyz / w.w;
}

@fragment
fn fs_fog(in: VSOut) -> @location(0) vec4<f32> {
  let hdr = textureSample(color_tex, color_samp, in.uv).rgb;

  let d = textureLoad(depth_tex, vec2<i32>(in.uv * vec2<f32>(textureDimensions(depth_tex))), 0);
  let world = reconstruct_world(in.uv, d);

  let dist = length(world - cam.cam_pos);
  let fog_dist = 1.0 - exp(-dist * fog.density);

  let h = max(0.0, fog.height - world.y);
  let fog_h = 1.0 - exp(-h * fog.height_density);

  let f = clamp(max(fog_dist, fog_h), 0.0, 1.0);
  let col = mix(hdr, fog.fog_color, f);

  return vec4<f32>(col, 1.0);
}


Notes

This expects your depth value to be compatible with inv VP reconstruction. If your depth differs (0..1 vs -1..1), adjust the NDC depth mapping (I can fix it once you tell me your convention).

17) BLOOM PIPELINE (3 shaders)
17a) Bright Extract
bloom_extract.wgsl
struct BloomExtractParams {
  threshold: f32,  // 0.9..1.3
  knee: f32,       // 0.2..0.6 (soft threshold)
};

@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_samp: sampler;
@group(0) @binding(2) var<uniform> p: BloomExtractParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var q = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(q[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

fn soft_threshold(x: vec3<f32>, t: f32, k: f32) -> vec3<f32> {
  // smooth knee: keeps bright stuff without hard cut
  let brightness = max(max(x.r, x.g), x.b);
  let soft = clamp((brightness - t + k) / (2.0 * k), 0.0, 1.0);
  let contrib = max(brightness - t, 0.0) + soft * soft * k;
  return x * (contrib / max(brightness, 1e-5));
}

@fragment
fn fs_extract(in: VSOut) -> @location(0) vec4<f32> {
  let c = textureSample(hdr_tex, hdr_samp, in.uv).rgb;
  let b = soft_threshold(c, p.threshold, p.knee);
  return vec4<f32>(b, 1.0);
}

17b) Separable Blur (use twice: horizontal then vertical)
bloom_blur.wgsl
struct BlurParams {
  direction: vec2<f32>, // (1,0) then (0,1)
  texel: vec2<f32>,     // (1/width, 1/height)
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;
@group(0) @binding(2) var<uniform> b: BlurParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var q = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(q[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

@fragment
fn fs_blur(in: VSOut) -> @location(0) vec4<f32> {
  // 9-tap Gaussian-ish weights
  let w0 = 0.227027;
  let w1 = 0.1945946;
  let w2 = 0.1216216;
  let w3 = 0.054054;
  let w4 = 0.016216;

  let off = b.direction * b.texel;

  var col = textureSample(src_tex, src_samp, in.uv).rgb * w0;
  col += textureSample(src_tex, src_samp, in.uv + off * 1.0).rgb * w1;
  col += textureSample(src_tex, src_samp, in.uv - off * 1.0).rgb * w1;
  col += textureSample(src_tex, src_samp, in.uv + off * 2.0).rgb * w2;
  col += textureSample(src_tex, src_samp, in.uv - off * 2.0).rgb * w2;
  col += textureSample(src_tex, src_samp, in.uv + off * 3.0).rgb * w3;
  col += textureSample(src_tex, src_samp, in.uv - off * 3.0).rgb * w3;
  col += textureSample(src_tex, src_samp, in.uv + off * 4.0).rgb * w4;
  col += textureSample(src_tex, src_samp, in.uv - off * 4.0).rgb * w4;

  return vec4<f32>(col, 1.0);
}

17c) Combine Bloom into HDR (or into Tonemap)
bloom_combine.wgsl
struct CombineParams { strength: f32; };

@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_samp: sampler;
@group(0) @binding(2) var bloom_tex: texture_2d<f32>;
@group(0) @binding(3) var bloom_samp: sampler;
@group(0) @binding(4) var<uniform> c: CombineParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var q = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(q[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

@fragment
fn fs_combine(in: VSOut) -> @location(0) vec4<f32> {
  let hdr = textureSample(hdr_tex, hdr_samp, in.uv).rgb;
  let bloom = textureSample(bloom_tex, bloom_samp, in.uv).rgb;
  return vec4<f32>(hdr + bloom * c.strength, 1.0);
}


Practical settings

Extract: threshold 1.0, knee 0.4

Combine: strength 0.06–0.12 (lava looks great here)

18) DIRECTIONAL SHADOW RECEIVER (forward rendering)

This is the sampling function you paste into terrain/castle/unit shaders.

Uniforms

Light view-projection matrix (shadow map)

Depth texture for shadow

Comparison sampler

struct ShadowParams {
  light_vp: mat4x4<f32>,
  bias: f32,            // 0.0008..0.003 depending on scale
  intensity: f32,       // 0..1 (how dark shadows are)
  _pad: vec2<f32>,
};

@group(0) @binding(10) var shadow_tex: texture_depth_2d;
@group(0) @binding(11) var shadow_samp: sampler_comparison;
@group(0) @binding(12) var<uniform> sh: ShadowParams;

fn sample_shadow(world_pos: vec3<f32>) -> f32 {
  let lp = sh.light_vp * vec4<f32>(world_pos, 1.0);
  let ndc = lp.xyz / lp.w;

  // map NDC -> UV
  let uv = ndc.xy * 0.5 + vec2<f32>(0.5);

  // outside light frustum = lit
  if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
    return 1.0;
  }

  // depth compare (ndc.z usually 0..1 in wgpu conventions)
  let depth = ndc.z - sh.bias;

  // basic 1-tap compare:
  // returns 1 if lit, 0 if shadowed
  let lit = textureSampleCompare(shadow_tex, shadow_samp, uv, depth);

  // soften darkness control
  return mix(1.0 - sh.intensity, 1.0, lit);
}

24) Procedural storm sky (no skybox texture)

Render as fullscreen triangle into HDR target before your opaque pass.

24a) sky_storm_procedural.wgsl
struct SkyParams {
  time: f32,
  // palette (roguelike multi-color)
  col_top: vec3<f32>,      // e.g. (0.10, 0.08, 0.18)
  col_mid: vec3<f32>,      // e.g. (0.25, 0.12, 0.35)
  col_horizon: vec3<f32>,  // e.g. (0.75, 0.25, 0.20)
  col_magic: vec3<f32>,    // e.g. (0.25, 0.40, 0.95)
  lightning_strength: f32, // 0..1
};

@group(0) @binding(0) var<uniform> sky: SkyParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(
    vec2<f32>(-1.0, -1.0),
    vec2<f32>( 3.0, -1.0),
    vec2<f32>(-1.0,  3.0)
  );
  var o: VSOut;
  o.pos = vec4<f32>(p[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

// hash + noise + fbm
fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}
fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0, 0.0));
  let c = hash(i + vec2<f32>(0.0, 1.0));
  let d = hash(i + vec2<f32>(1.0, 1.0));
  let u = f*f*(3.0 - 2.0*f);
  return mix(mix(a,b,u.x), mix(c,d,u.x), u.y);
}
fn fbm(p: vec2<f32>) -> f32 {
  var v = 0.0;
  var a = 0.5;
  var f = 1.0;
  for (var i = 0; i < 5; i = i + 1) {
    v += a * noise(p * f);
    f *= 2.0;
    a *= 0.5;
  }
  return v;
}

// lightning: rare flashes + thin branches
fn lightning(uv: vec2<f32>, t: f32) -> f32 {
  // flashes roughly every 5-7 seconds
  let flash = smoothstep(0.985, 1.0, sin(t * 1.05 + 1.7));
  if (flash <= 0.0) { return 0.0; }

  // bolt “spine”
  let x = uv.x + (fbm(vec2<f32>(uv.y * 6.0, t * 0.5)) - 0.5) * 0.18;
  let center = 0.52 + sin(t * 0.25) * 0.08;

  let spine = smoothstep(0.02, 0.0, abs(x - center)); // thin

  // branch noise
  let br = smoothstep(0.75, 1.0, fbm(uv * 8.0 + vec2<f32>(t, -t)));
  return spine * br * flash;
}

@fragment
fn fs_sky(in: VSOut) -> @location(0) vec4<f32> {
  let uv = in.uv;
  let t = sky.time;

  // vertical gradient base
  let y = clamp(uv.y, 0.0, 1.0);
  var base = mix(sky.col_horizon, sky.col_mid, smoothstep(0.10, 0.65, y));
  base = mix(base, sky.col_top, smoothstep(0.55, 1.0, y));

  // swirling clouds (domain warp)
  let p = uv * vec2<f32>(3.0, 1.6);
  let warp = vec2<f32>(
    fbm(p + vec2<f32>(t * 0.03, 0.0)),
    fbm(p + vec2<f32>(0.0, t * 0.03))
  );
  let c = fbm(p * 2.2 + warp * 1.6 + vec2<f32>(t * 0.06, -t * 0.04));

  // cloud mask: darker blobs
  let cloud = smoothstep(0.35, 0.85, c);
  let cloud_dark = base * (0.55 + 0.25 * (1.0 - cloud));

  // rogue-like “magic” color veins
  let magic_band = smoothstep(0.65, 0.95, fbm(p * 5.0 + vec2<f32>(-t * 0.08, t * 0.05)));
  var col = mix(cloud_dark, sky.col_magic, magic_band * 0.35);

  // add lightning flash
  let bolt = lightning(uv, t) * sky.lightning_strength;
  col += vec3<f32>(0.8, 0.9, 1.0) * bolt * 1.2;

  return vec4<f32>(col, 1.0);
}


Recommended params

col_top: (0.08, 0.06, 0.14)

col_mid: (0.22, 0.12, 0.30)

col_horizon: (0.70, 0.22, 0.18)

col_magic: (0.25, 0.45, 0.95)

lightning_strength: 0.8

25) Smoke columns & battlefield haze (billboards)

Use for:

siege impacts

volcano/lava vents

castle damage

ambient haze near lava edges

25a) smoke_billboard.wgsl

Assumes your CPU spawns quads that always face camera (or you do billboard in vertex).

struct SmokeParams {
  time: f32,
  color: vec3<f32>,        // e.g. (0.25, 0.22, 0.30)
  alpha: f32,              // 0.5..0.9
  scroll: vec2<f32>,       // e.g. (0.02, 0.05)
};

@group(1) @binding(0) var<uniform> s: SmokeParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(12.9898, 78.233))) * 43758.5453);
}
fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0,0.0));
  let c = hash(i + vec2<f32>(0.0,1.0));
  let d = hash(i + vec2<f32>(1.0,1.0));
  let u = f*f*(3.0-2.0*f);
  return mix(mix(a,b,u.x), mix(c,d,u.x), u.y);
}
fn fbm(p: vec2<f32>) -> f32 {
  var v = 0.0;
  var a = 0.5;
  var f = 1.0;
  for (var i=0; i<5; i=i+1) {
    v += a * noise(p*f);
    f *= 2.0;
    a *= 0.5;
  }
  return v;
}

struct FSIn {
  @location(0) uv: vec2<f32>,       // 0..1 across quad
  @location(1) world_pos: vec3<f32> // optional if you have it
};

@fragment
fn fs_smoke(in: FSIn) -> @location(0) vec4<f32> {
  // soft circle mask
  let d = distance(in.uv, vec2<f32>(0.5));
  let mask = smoothstep(0.52, 0.15, d);

  // scrolling noise
  let uv = in.uv * 2.6 + s.scroll * s.time;
  let n = fbm(uv * 2.0) * 0.75 + fbm(uv * 4.0) * 0.25;

  // smoke shape
  let puff = smoothstep(0.35, 0.85, n);
  let a = mask * puff * s.alpha;

  // slightly brighter center
  let center = smoothstep(0.5, 0.0, d);
  let col = s.color * (0.75 + center * 0.35);

  return vec4<f32>(col, a);
}


Blend mode

For smoke: alpha blending (src_alpha, one_minus_src_alpha)

For ash/embers: additive (one, one)

26) Heat distortion above lava (screen-space refraction)

This is the “shimmer” that screams lava heat.

Render order

Render scene into HDR color scene_tex

Render a heat mask (only in lava zones) into heat_mask_tex OR reuse lava ID mask

Fullscreen pass distorts scene_tex using noise where mask > 0

26a) heat_distort.wgsl
struct HeatParams {
  time: f32,
  strength: f32,   // 0.002..0.01 (UV units)
  speed: f32,      // 0.3..1.5
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_samp: sampler;

@group(0) @binding(2) var heat_tex: texture_2d<f32>; // R8 or R16F mask, 0..1
@group(0) @binding(3) var heat_samp: sampler;

@group(0) @binding(4) var<uniform> h: HeatParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1,311.7))) * 43758.5453);
}
fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0,0.0));
  let c = hash(i + vec2<f32>(0.0,1.0));
  let d = hash(i + vec2<f32>(1.0,1.0));
  let u = f*f*(3.0-2.0*f);
  return mix(mix(a,b,u.x), mix(c,d,u.x), u.y);
}

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(p[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

@fragment
fn fs_heat(in: VSOut) -> @location(0) vec4<f32> {
  let mask = textureSample(heat_tex, heat_samp, in.uv).r;
  if (mask <= 0.001) {
    let c = textureSample(scene_tex, scene_samp, in.uv).rgb;
    return vec4<f32>(c, 1.0);
  }

  // animated distortion field
  let p = in.uv * 18.0;
  let t = h.time * h.speed;

  let n1 = noise(p + vec2<f32>(t, -t));
  let n2 = noise(p * 1.7 + vec2<f32>(-t*0.6, t*0.8));

  let dx = (n1 - 0.5);
  let dy = (n2 - 0.5);

  let offset = vec2<f32>(dx, dy) * h.strength * mask;

  // sample distorted
  let c = textureSample(scene_tex, scene_samp, in.uv + offset).rgb;
  return vec4<f32>(c, 1.0);
}


How to build the heat mask

easiest: render the lava mesh again into an R8 target with solid 1.0

or render a fullscreen mask from lava world positions (if you have an ID buffer)

27) Enemy flag “objective highlight” (outline + pulse ping)

This is super useful gameplay readability: you always see the enemy flag.

Two approaches:

A) outline via object ID mask (recommended)

B) outline via depth edge detect (works but less specific)

I’ll give you A) (ID mask) because you’re building game systems anyway.

Pipeline

Render a small ID mask texture id_mask (R8Uint or R16Uint).

Write 1 for enemy flag pixels, 0 otherwise.

Post pass reads id_mask and adds outline/glow to scene.

27a) flag_outline_glow.wgsl
struct FlagFxParams {
  time: f32,
  outline_px: f32,     // 1..3
  glow_strength: f32,  // 0.3..1.2
  pulse_speed: f32,    // 1..4
  color: vec3<f32>,    // enemy highlight, e.g. (1.0, 0.25, 0.12)
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_samp: sampler;

@group(0) @binding(2) var id_tex: texture_2d<u32>;  // 0 or 1
@group(0) @binding(3) var<uniform> fx: FlagFxParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(p[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

fn id_at(uv: vec2<f32>) -> u32 {
  let dim = vec2<f32>(textureDimensions(id_tex));
  let p = vec2<i32>(uv * dim);
  return textureLoad(id_tex, p, 0).r;
}

@fragment
fn fs_flag_fx(in: VSOut) -> @location(0) vec4<f32> {
  var col = textureSample(scene_tex, scene_samp, in.uv).rgb;

  let center = id_at(in.uv);
  let dim = vec2<f32>(textureDimensions(id_tex));
  let texel = 1.0 / dim;

  // outline detection: if center is 0 but neighbors contain 1 => edge
  var edge = 0.0;
  let r = i32(fx.outline_px);

  if (center == 0u) {
    for (var y = -r; y <= r; y = y + 1) {
      for (var x = -r; x <= r; x = x + 1) {
        let u = in.uv + vec2<f32>(f32(x), f32(y)) * texel;
        if (id_at(u) == 1u) { edge = 1.0; }
      }
    }
  }

  // glow inside the flag too
  let inside = select(0.0, 1.0, center == 1u);

  // pulse ping
  let pulse = 0.6 + 0.4 * sin(fx.time * fx.pulse_speed);

  // additive highlight
  let outline = fx.color * edge * fx.glow_strength * pulse;
  let fill = fx.color * inside * (fx.glow_strength * 0.35) * (0.7 + 0.3 * pulse);

  col += outline + fill;

  return vec4<f32>(col, 1.0);
}


ID mask rendering tip

Use a tiny pipeline that writes to R32Uint or R8Uint target:

fragment outputs 1u for enemy flag geometry

all else 0u

Clear to 0 each frame

Recommended “storm/lava image” settings (quick presets)

Fog post: density 0.02, height 1.2, height_density 0.10, color (0.55,0.45,0.70)

Bloom: threshold 1.0, knee 0.4, strength 0.08

Heat distortion: strength 0.006, speed 1.0

Flag outline: outline_px 2, glow_strength 0.9, pulse_speed 2.3, color (1.0,0.25,0.12)





28) Volcanic ash + wind streaks (fullscreen overlay)

This is a post pass you add after fog (or before tonemap). It adds moving specks + streaks that scream “storm + ash”.

ash_wind_overlay.wgsl
struct AshParams {
  time: f32,
  strength: f32,     // 0.05..0.25
  wind_dir: vec2<f32>, // normalized (e.g. (0.9, 0.2))
  wind_speed: f32,   // 0.05..0.25
  streaks: f32,      // 0..1 (more streaky)
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_samp: sampler;
@group(0) @binding(2) var<uniform> a: AshParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(p[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

// cheap “particle field”
fn ash_field(uv: vec2<f32>, t: f32) -> f32 {
  let grid = uv * 220.0;
  let cell = floor(grid);
  let f = fract(grid);

  let rnd = hash(cell);
  // tiny speck near random position
  let px = fract(rnd * 13.13);
  let py = fract(rnd * 91.71);
  let d = distance(f, vec2<f32>(px, py));
  let speck = smoothstep(0.06, 0.0, d);

  // animate fade
  let flicker = 0.5 + 0.5 * sin(t * 2.0 + rnd * 6.28);
  return speck * flicker;
}

@fragment
fn fs_ash(in: VSOut) -> @location(0) vec4<f32> {
  var col = textureSample(scene_tex, scene_samp, in.uv).rgb;

  let t = a.time;
  let flow = a.wind_dir * (t * a.wind_speed);
  let uv = in.uv + flow;

  // specks
  let ash = ash_field(uv, t) * a.strength;

  // streaks: stretch along wind direction by sampling ahead
  let ahead = ash_field(uv + a.wind_dir * 0.01, t) * a.strength;
  let streak = mix(ash, max(ash, ahead), a.streaks);

  // tint (ashen purple/gray)
  let tint = vec3<f32>(0.45, 0.42, 0.55);
  col = col + tint * streak;

  return vec4<f32>(col, 1.0);
}


Preset

strength 0.12

wind_dir (0.95, 0.20)

wind_speed 0.12

streaks 0.65

29) Ground contact darkening (SSAO-lite)

This fakes “contact shadows” without SSAO. Best for troops/props/bridge posts.

Idea

In your opaque shaders, compute a darkening term based on:

how downward the normal points

how close the object is to ground height (or to a “contact plane”)

If you have a heightmap for terrain: sample it.
If not, use a simple “near y=0” approach or per-tile plane.

contact_darkening.wgsl helper
struct ContactParams {
  ground_y: f32,        // base terrain level, or per-island
  strength: f32,        // 0.1..0.5
  radius: f32,          // 0.3..1.5 world units
};

@group(1) @binding(6) var<uniform> contact: ContactParams;

fn contact_shadow(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
  // how close to ground
  let h = abs(world_pos.y - contact.ground_y);
  let near = clamp(1.0 - h / contact.radius, 0.0, 1.0);

  // stronger when surface faces down-ish (undersides, feet areas)
  let down = clamp(dot(normalize(normal), vec3<f32>(0.0, -1.0, 0.0)), 0.0, 1.0);

  // darken around contact
  return 1.0 - (near * (0.3 + down * 0.7) * contact.strength);
}


Use:

col *= contact_shadow(world_pos, world_nrm);


Preset

strength 0.35

radius 0.8

ground_y = your island base

30) Rain wetness/spec pass (storm mood on everything)

This is a material modifier you can apply to terrain/castles/bridge.

wetness_spec.wgsl helper
struct WetParams {
  time: f32,
  amount: f32,          // 0..1
  spec_strength: f32,   // 0.2..1.2
};

@group(1) @binding(7) var<uniform> wet: WetParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(12.9898,78.233))) * 43758.5453);
}
fn rain_mask(world_pos: vec3<f32>) -> f32 {
  // raindrop streak noise in world space
  let p = world_pos.xz * 3.0 + vec2<f32>(wet.time * 0.6, -wet.time * 1.2);
  return smoothstep(0.85, 1.0, hash(floor(p)));
}

fn apply_wetness(col: vec3<f32>, world_pos: vec3<f32>, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec3<f32> {
  if (wet.amount <= 0.001) { return col; }

  let N = normalize(n);
  let V = normalize(v);
  let L = normalize(l);

  // “glancing angle” wet sheen
  let fres = pow(1.0 - clamp(dot(N, V), 0.0, 1.0), 3.0);

  // fake spec highlight
  let H = normalize(V + L);
  let spec = pow(max(dot(N, H), 0.0), 48.0) * wet.spec_strength;

  // rain mask adds variation
  let r = rain_mask(world_pos);

  let darken = 1.0 - wet.amount * 0.08; // wet surfaces slightly darker
  var outc = col * darken;

  // add sheen
  outc += vec3<f32>(1.0) * (spec * wet.amount * (0.4 + 0.6*r));
  outc += vec3<f32>(0.25, 0.30, 0.40) * (fres * wet.amount * 0.15);

  return outc;
}


Use:

let v = (view_pos - world_pos);
col = apply_wetness(col, world_pos, world_nrm, v, light_dir);


Preset

amount 0.65

spec_strength 0.8

31) Projectile trails (ribbon/billboard shader)

Render projectile trails as a camera-facing ribbon mesh (or quads along path).
Use additive blending.

projectile_trail.wgsl
struct TrailParams {
  time: f32,
  color: vec3<f32>,    // (1.0,0.5,0.1) fire or (0.2,0.5,1.0) magic
  intensity: f32,      // 0.8..2.5
};

@group(1) @binding(0) var<uniform> tr: TrailParams;

struct FSIn {
  @location(0) uv: vec2<f32>, // x along length, y across width 0..1
};

@fragment
fn fs_trail(in: FSIn) -> @location(0) vec4<f32> {
  // soft width falloff
  let d = abs(in.uv.y - 0.5) * 2.0;
  let width = smoothstep(1.0, 0.0, d);

  // fade tail
  let tail = smoothstep(1.0, 0.0, in.uv.x);

  // animated flicker
  let flicker = 0.85 + 0.15 * sin(tr.time * 18.0 + in.uv.x * 10.0);

  let a = width * tail;
  let col = tr.color * tr.intensity * flicker;

  return vec4<f32>(col, a);
}


Blend

Additive: src=One, dst=One

Or premult alpha additive: src=SrcAlpha, dst=One

32) Impact shockwave ring (siege hit effect)

Spawn a quad aligned to ground at impact point. This shader creates a ring that expands.

shockwave_ring.wgsl
struct ShockParams {
  time: f32,
  start_time: f32,
  duration: f32,      // 0.6..1.2
  color: vec3<f32>,   // (1.0,0.45,0.2)
  intensity: f32,     // 0.6..2.0
};

@group(1) @binding(0) var<uniform> sh: ShockParams;

struct FSIn { @location(0) uv: vec2<f32> };

@fragment
fn fs_shock(in: FSIn) -> @location(0) vec4<f32> {
  let age = (sh.time - sh.start_time) / sh.duration;
  if (age < 0.0 || age > 1.0) { discard; }

  // uv centered
  let p = (in.uv - vec2<f32>(0.5)) * 2.0;
  let r = length(p);

  let ring_radius = age;           // expands from 0 to 1
  let thickness = 0.08;

  let ring = smoothstep(thickness, 0.0, abs(r - ring_radius));
  let fade = smoothstep(1.0, 0.0, age);

  let a = ring * fade;
  let col = sh.color * sh.intensity;

  return vec4<f32>(col, a);
}


Blend

Additive looks best for lava/impact.

For dust ring, use alpha blending and darker color.

Recommended order to integrate Batch 5

Wetness helper (apply to castle + bridge + terrain)

Lava heat distortion (from earlier batch)

Ash wind overlay (post)

Projectile trails

Shockwave rings

Contact darkening (units + props)


33) Screen-space outlines (ID-mask based, best for roguelike)

Best method: render an ID mask buffer where:

0 = nothing

1 = friendly units/buildings

2 = enemy units/buildings

3 = objective flag
(or any scheme)

Then outline by detecting edges in the mask.

33a) Outline post shader
outline_id_post.wgsl
struct OutlineParams {
  time: f32,
  thickness_px: f32,      // 1..3
  strength: f32,          // 0.2..1.2
  friendly: vec3<f32>,    // e.g. (0.15,0.50,1.00)
  enemy: vec3<f32>,       // e.g. (1.00,0.20,0.12)
  objective: vec3<f32>,   // e.g. (1.00,0.75,0.15)
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_samp: sampler;

@group(0) @binding(2) var id_tex: texture_2d<u32>; // R32Uint or R8Uint promoted
@group(0) @binding(3) var<uniform> o: OutlineParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var p = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var out: VSOut;
  out.pos = vec4<f32>(p[vid], 0.0, 1.0);
  out.uv = (out.pos.xy * 0.5) + vec2<f32>(0.5);
  return out;
}

fn id_at(uv: vec2<f32>) -> u32 {
  let dim = vec2<f32>(textureDimensions(id_tex));
  let p = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.9999)) * dim);
  return textureLoad(id_tex, p, 0).r;
}

fn color_for_id(id: u32) -> vec3<f32> {
  if (id == 1u) { return o.friendly; }
  if (id == 2u) { return o.enemy; }
  if (id == 3u) { return o.objective; }
  return vec3<f32>(0.0);
}

@fragment
fn fs_outline(in: VSOut) -> @location(0) vec4<f32> {
  var col = textureSample(scene_tex, scene_samp, in.uv).rgb;

  let center = id_at(in.uv);
  let dim = vec2<f32>(textureDimensions(id_tex));
  let texel = 1.0 / dim;
  let r = i32(o.thickness_px);

  // Edge if center differs from any neighbor
  var edge = 0.0;
  var edge_id = center;

  for (var y = -r; y <= r; y = y + 1) {
    for (var x = -r; x <= r; x = x + 1) {
      let n_id = id_at(in.uv + vec2<f32>(f32(x), f32(y)) * texel);
      if (n_id != center) {
        edge = 1.0;
        // prefer non-zero ids for outline color
        if (edge_id == 0u && n_id != 0u) { edge_id = n_id; }
        if (center != 0u) { edge_id = center; }
      }
    }
  }

  if (edge > 0.0 && edge_id != 0u) {
    // pulse for objective
    let pulse = if (edge_id == 3u) { 0.65 + 0.35 * sin(o.time * 2.3) } else { 1.0 };
    col += color_for_id(edge_id) * o.strength * pulse;
  }

  return vec4<f32>(col, 1.0);
}


ID render pass tip

Attach a second color target during your main opaque pass:

scene_hdr (Rgba16Float)

id_mask (R32Uint)

Each draw writes its ID (0/1/2/3) in the fragment.

34) Territory tint for hex ownership (tile shader)

If your world is made of hex tiles, tint each tile by owner while keeping material detail visible.

This shader assumes:

you can supply owner_id per tile (instance attribute or storage buffer)

you use the terrain base color logic (height/slope) and then overlay tint.

34a) hex_territory_tint.wgsl
struct TeamColors {
  neutral: vec3<f32>,
  friendly: vec3<f32>,
  enemy: vec3<f32>,
};

@group(1) @binding(0) var<uniform> team: TeamColors;

fn tint_for_owner(owner: u32) -> vec3<f32> {
  if (owner == 1u) { return team.friendly; }
  if (owner == 2u) { return team.enemy; }
  return team.neutral;
}

@fragment
fn fs_hex_tile(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
  @location(4) owner_id: u32, // from instance buffer / vertex attrib
) -> @location(0) vec4<f32> {

  // Base terrain color (simple grass)
  var base = vec3<f32>(0.22, 0.72, 0.33);

  // Add slope darkening for depth
  let up = vec3<f32>(0.0,1.0,0.0);
  let slope = 1.0 - clamp(dot(normalize(world_nrm), up), 0.0, 1.0);
  base = mix(base, vec3<f32>(0.45,0.47,0.50), slope * 0.55);

  // Lighting
  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  var col = base * (0.28 + ndl * 1.0);

  // Territory tint overlay (keeps detail visible)
  let tint = tint_for_owner(owner_id);

  // Stronger tint near tile edges (helps readability):
  // If you have tile-local UV (0..1), use that. Otherwise approximate using world_pos fractional.
  let edge_hint = smoothstep(0.0, 0.2, abs(fract(world_pos.x * 0.08) - 0.5))
                + smoothstep(0.0, 0.2, abs(fract(world_pos.z * 0.08) - 0.5));
  let edge = clamp(edge_hint, 0.0, 1.0);

  let tint_strength = 0.10 + edge * 0.12; // subtle in center, stronger at edges
  col = mix(col, col * tint, tint_strength);

  return vec4<f32>(col, 1.0);
}


Suggested team colors

neutral: (0.85, 0.85, 0.85) (almost no tint)

friendly: (0.25, 0.55, 1.00)

enemy: (1.00, 0.20, 0.15)

35) Lava shoreline glow (where lava meets rock)

This makes the border look “hot” and adds realism.

Method: in your rock/cliff shader, compute distance to lava plane/level and add a hot rim.

35a) lava_shore_glow.wgsl helper
struct ShoreParams {
  lava_y: f32,
  glow_color: vec3<f32>, // (1.0,0.35,0.08)
  glow_strength: f32,    // 0.2..1.5
  band: f32,             // 0.3..1.5 how thick the glow region is
};

@group(1) @binding(8) var<uniform> shore: ShoreParams;

fn lava_shore(world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
  // how close to lava level
  let d = abs(world_pos.y - shore.lava_y);
  let near = clamp(1.0 - d / shore.band, 0.0, 1.0);

  // emphasize edges (grazing angle)
  let up = vec3<f32>(0.0,1.0,0.0);
  let slope = 1.0 - clamp(dot(normalize(normal), up), 0.0, 1.0);

  let g = near * (0.3 + slope * 0.7);
  return shore.glow_color * shore.glow_strength * g;
}


Use in rock shader:

col += lava_shore(world_pos, world_nrm);


Preset

band 0.9

glow_strength 0.9

36) Objective beam (pillar of light over enemy flag)

This is perfect for “capture enemy flag” gameplay. Render it as:

a vertical cylinder mesh (or billboard column)

additive blending

animated swirl

36a) objective_beam.wgsl
struct BeamParams {
  time: f32,
  color: vec3<f32>,     // (1.0,0.7,0.2) or enemy color
  intensity: f32,       // 0.5..3.0
  swirl: f32,           // 2..8
};

@group(1) @binding(0) var<uniform> b: BeamParams;

struct FSIn {
  @location(0) uv: vec2<f32>, // uv.y = height (0..1), uv.x = around (0..1)
};

@fragment
fn fs_beam(in: FSIn) -> @location(0) vec4<f32> {
  let y = clamp(in.uv.y, 0.0, 1.0);

  // soft edges
  let edge = smoothstep(0.0, 0.15, in.uv.x) * smoothstep(1.0, 0.85, in.uv.x);
  // if you don't have radial uv, treat uv.x as "width"

  // swirl pattern
  let s = 0.5 + 0.5 * sin((in.uv.x * 6.28 * b.swirl) + b.time * 3.0 + y * 6.0);

  // pulse
  let pulse = 0.7 + 0.3 * sin(b.time * 2.0);

  // fade top
  let top_fade = smoothstep(1.0, 0.6, y);

  let a = edge * s * pulse * top_fade;
  let col = b.color * b.intensity;

  return vec4<f32>(col, a);
}


Blend

Additive: One, One

Render after opaque, before tonemap (HDR).

Quick “how to wire this into your engine”

New buffers/textures you’ll want

id_mask (R32Uint) for outlines + objective

hdr_scene (Rgba16Float) for emissive/bloom

optional post chain: fog → ash → outlines → tonemap




37) Lava “sheet flow” + better normals (no textures)

This upgrades lava from “noise glow” to “moving molten layers”.

lava_flow_normals.wgsl
struct LavaFlowParams {
  time: f32,
  emissive: f32,        // 0.8..2.5
  scale: f32,           // 0.15..0.6
  speed: f32,           // 0.1..0.6
  normal_strength: f32, // 0.2..0.8
};

@group(1) @binding(0) var<uniform> lf: LavaFlowParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}
fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0,0.0));
  let c = hash(i + vec2<f32>(0.0,1.0));
  let d = hash(i + vec2<f32>(1.0,1.0));
  let u = f*f*(3.0-2.0*f);
  return mix(mix(a,b,u.x), mix(c,d,u.x), u.y);
}
fn fbm(p: vec2<f32>) -> f32 {
  var v = 0.0;
  var a = 0.5;
  var f = 1.0;
  for (var i = 0; i < 5; i = i + 1) {
    v += a * noise(p*f);
    f *= 2.0;
    a *= 0.5;
  }
  return v;
}

fn fresnel(n: vec3<f32>, v: vec3<f32>) -> f32 {
  return pow(1.0 - clamp(dot(normalize(n), normalize(v)), 0.0, 1.0), 3.0);
}

@fragment
fn fs_lava_flow(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
) -> @location(0) vec4<f32> {

  let uv = world_pos.xz * lf.scale;
  let t = lf.time * lf.speed;

  // domain warp for “sheet” motion
  let warp = vec2<f32>(
    fbm(uv * 1.4 + vec2<f32>(t, -t)),
    fbm(uv * 1.4 + vec2<f32>(-t, t))
  ) - vec2<f32>(0.5);

  let f1 = fbm(uv * 2.0 + warp * 1.2 + vec2<f32>(t, 0.0));
  let f2 = fbm(uv * 4.0 - warp * 0.8 + vec2<f32>(0.0, -t));
  let heat = clamp(f1 * 0.7 + f2 * 0.5, 0.0, 1.0);

  // crack bands (thin bright lines)
  let cracks = smoothstep(0.78, 0.95, f2);

  // approximate normals from heightfield derivatives
  let eps = 0.03;
  let hx = fbm((uv + vec2<f32>(eps, 0.0)) * 2.0 + warp) - f1;
  let hz = fbm((uv + vec2<f32>(0.0, eps)) * 2.0 + warp) - f1;

  var n = normalize(world_nrm + vec3<f32>(hx, 0.0, hz) * lf.normal_strength);

  let v = normalize(view_pos - world_pos);
  let fr = fresnel(n, v);

  let crust = vec3<f32>(0.05, 0.01, 0.01);
  let molten = vec3<f32>(2.4, 0.65, 0.08);

  var col = mix(crust, molten, heat);
  col = mix(col, molten * 1.4, cracks);

  // emissive response (HDR)
  let outc = col * (lf.emissive + fr * 0.35);
  return vec4<f32>(outc, 1.0);
}

38) Water screen-space reflections (SSR-lite)

True SSR is heavy; this version is cheap and good enough:

reflect view ray on water normal

sample scene color at a small offset in screen space

mix with Fresnel

water_ssr_lite.wgsl
struct WaterSSRParams {
  time: f32,
  ripple: f32,        // 0.1..0.35
  reflect_strength: f32, // 0.1..0.5
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_samp: sampler;
@group(1) @binding(0) var<uniform> ws: WaterSSRParams;

fn fresnel(n: vec3<f32>, v: vec3<f32>) -> f32 {
  return pow(1.0 - clamp(dot(normalize(n), normalize(v)), 0.0, 1.0), 4.0);
}

@fragment
fn fs_water_ssr(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(4) screen_uv: vec2<f32>, // IMPORTANT: pass from vertex (0..1)
) -> @location(0) vec4<f32> {

  // ripple normal
  let r = sin(world_pos.x * 0.9 + ws.time * 1.2) * 0.5 +
          cos(world_pos.z * 1.2 - ws.time * 1.0) * 0.5;
  let n = normalize(world_nrm + vec3<f32>(r * ws.ripple, 0.0, r * ws.ripple));

  let v = normalize(view_pos - world_pos);
  let f = fresnel(n, v);

  // screen-space reflection sample offset (hacky but pretty)
  let refl_dir = reflect(-v, n);
  let offset = refl_dir.xz * 0.03;  // tune
  let refl = textureSample(scene_tex, scene_samp, screen_uv + offset).rgb;

  // base water
  let base = vec3<f32>(0.06, 0.18, 0.28);

  var col = mix(base, refl, f * ws.reflect_strength);
  return vec4<f32>(col, 1.0);
}


Note: you must pass screen_uv from vertex: clip.xy/clip.w * 0.5 + 0.5.

39) Bridge sway + chain sag (vertex animation)

Add life to the bridge. Do two materials:

bridge planks sway slightly

chains sag & wobble

39a) Bridge sway (vertex)
struct BridgeAnim {
  time: f32,
  sway_strength: f32, // 0.02..0.12
  sway_speed: f32,    // 0.8..2.0
};

@group(1) @binding(0) var<uniform> b: BridgeAnim;

@vertex
fn vs_bridge(
  @location(0) pos: vec3<f32>,
  @location(1) uv: vec2<f32>,
  // TODO: your model/view/proj
) -> /* your VSOut */ vec4<f32> {

  // uv.x along bridge length
  let phase = uv.x * 6.28;
  let sway = sin(phase + b.time * b.sway_speed) * b.sway_strength;

  // stronger in the middle
  let mid = 1.0 - abs(uv.x * 2.0 - 1.0);
  let yoff = sway * (0.2 + 0.8 * mid);

  let p = pos + vec3<f32>(0.0, yoff, 0.0);

  // TODO: return MVP * vec4(p,1)
  return vec4<f32>(p, 1.0);
}

39b) Chain sag (vertex)

Assumes chain mesh has uv.x along length.

struct ChainAnim {
  time: f32,
  sag: f32,          // 0.05..0.25
  wobble: f32,       // 0.01..0.06
};

@group(1) @binding(0) var<uniform> ca: ChainAnim;

@vertex
fn vs_chain(
  @location(0) pos: vec3<f32>,
  @location(1) uv: vec2<f32>,
) -> vec4<f32> {
  let x = uv.x; // 0..1 along chain
  let sag_curve = (x * (1.0 - x)) * 4.0; // parabola peak at center
  let wob = sin(ca.time * 2.1 + x * 12.0) * ca.wobble;

  let p = pos + vec3<f32>(0.0, -sag_curve * ca.sag + wob, 0.0);
  return vec4<f32>(p, 1.0);
}

40) Destruction / “obliteration” shader (burn + cracks + dissolve)

Use this on buildings when they are being destroyed.

building_destruction.wgsl
struct DestroyParams {
  time: f32,
  start_time: f32,   // when destruction begins
  duration: f32,     // 1.0..4.0
  burn_color: vec3<f32>, // (1.0,0.35,0.08)
  soot_color: vec3<f32>, // (0.05,0.03,0.03)
};

@group(1) @binding(0) var<uniform> d: DestroyParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1,311.7))) * 43758.5453);
}
fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0,0.0));
  let c = hash(i + vec2<f32>(0.0,1.0));
  let dd = hash(i + vec2<f32>(1.0,1.0));
  let u = f*f*(3.0-2.0*f);
  return mix(mix(a,b,u.x), mix(c,dd,u.x), u.y);
}
fn fbm(p: vec2<f32>) -> f32 {
  var v = 0.0;
  var a = 0.5;
  var f = 1.0;
  for (var i=0; i<5; i=i+1) {
    v += a * noise(p*f);
    f *= 2.0;
    a *= 0.5;
  }
  return v;
}

@fragment
fn fs_destroy(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {

  // base building color (stone)
  var base = vec3<f32>(0.48, 0.47, 0.46);

  let age = clamp((d.time - d.start_time) / d.duration, 0.0, 1.0);

  // dissolve mask (noise field)
  let n = fbm(world_pos.xz * 2.2 + vec2<f32>(age * 2.0, -age));
  let cut = smoothstep(age - 0.15, age + 0.15, n);

  // burn edge (where dissolving happens)
  let edge = smoothstep(0.45, 0.55, cut) * (1.0 - smoothstep(0.55, 0.65, cut));

  // soot coverage grows with age
  base = mix(base, d.soot_color, age * 0.75);

  // emissive burn line (HDR)
  let burn = d.burn_color * edge * (1.5 + 2.0 * (1.0 - age));

  // lighting
  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  var col = base * (0.25 + ndl * 1.0) + burn;

  // actually remove pixels as it dissolves
  if (cut < 0.15) { discard; }

  return vec4<f32>(col, 1.0);
}

41) Destruction particles (debris + sparks + dust) — practical pipeline

You asked: “particles when the building part is destruction… physics and sf”.

The simplest robust approach:

CPU spawns particles when building enters “destructing”

GPU draws them as:

debris chunks = instanced cubes (opaque) or billboards

sparks = additive billboards

dust = alpha billboards

41a) Particle data layout (Rust side idea)

Use a storage buffer of particles:

pos (vec3) + vel (vec3) + life (f32) + kind (u32) + seed (u32)

41b) GPU update (compute) OR CPU update

If you don’t have compute yet: update on CPU each frame (still fine for 1k–10k particles).

Debris physics (CPU or compute)

gravity

simple ground bounce

drag

random angular spin if you want

Pseudo rules

vel.y -= 9.8 * dt

pos += vel * dt

if pos.y < ground_y: pos.y = ground_y; vel *= 0.35; vel.y = abs(vel.y)*0.25

vel *= exp(-drag*dt)

41c) Debris render shader (instanced mesh)
debris_instanced.wgsl
struct DebrisParams {
  time: f32,
  base_color: vec3<f32>, // stone/wood mix
};

@group(1) @binding(0) var<uniform> dp: DebrisParams;

@fragment
fn fs_debris(
  @location(1) world_nrm: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {
  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  let col = dp.base_color * (0.22 + ndl * 1.05);
  return vec4<f32>(col, 1.0);
}

41d) Spark billboards (additive)
sparks.wgsl
struct SparkParams {
  time: f32,
  color: vec3<f32>,   // (2.0,0.7,0.2) HDR
  intensity: f32,     // 0.6..2.5
};

@group(1) @binding(0) var<uniform> sp: SparkParams;

struct FSIn { @location(0) uv: vec2<f32>, @location(1) life01: f32 };

@fragment
fn fs_spark(in: FSIn) -> @location(0) vec4<f32> {
  let d = distance(in.uv, vec2<f32>(0.5));
  let a = smoothstep(0.5, 0.0, d) * in.life01; // fade with life
  let col = sp.color * sp.intensity;
  return vec4<f32>(col, a);
}

41e) Dust (alpha)

Same as smoke shader from Batch 4, but color more gray and softer.

42) Explosion impulse + outward chunk spray (looks “physics”)

When a building piece breaks:

compute center C

for each spawned debris particle:

direction = normalize(randVec3() + (pos - C))

speed = random(3..12)

vel = direction * speed + Vec3(0, random(2..7), 0)

Optional: add “shock ring” from Batch 5 + sparks.

Quick integration recipe for “building destroyed”

When destruction triggers:

Set building material to fs_destroy with start_time = now

Spawn:

80–200 debris chunks (stone/wood)

60–140 sparks (additive)

8–20 smoke puffs (alpha)

Add a shockwave ring at base

Optional: camera shake / screen flash (tiny)

43) Fire propagation on buildings (burn spreads + embers)

Use this for castles, towers, wooden structures. It’s a material variant you apply when “on fire”.

fire_propagation_building.wgsl
struct FireParams {
  time: f32,
  ignited_at: f32,
  spread_speed: f32,     // 0.2..1.5
  burn_strength: f32,    // 0.2..1.0
  flame_intensity: f32,  // 0.8..3.0 (HDR)
  soot_color: vec3<f32>, // (0.05,0.03,0.03)
  flame_color: vec3<f32>,// (2.2,0.75,0.15)
};

@group(1) @binding(0) var<uniform> fp: FireParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1,311.7))) * 43758.5453);
}
fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0,0.0));
  let c = hash(i + vec2<f32>(0.0,1.0));
  let d = hash(i + vec2<f32>(1.0,1.0));
  let u = f*f*(3.0-2.0*f);
  return mix(mix(a,b,u.x), mix(c,d,u.x), u.y);
}
fn fbm(p: vec2<f32>) -> f32 {
  var v = 0.0;
  var a = 0.5;
  var f = 1.0;
  for (var i=0; i<5; i=i+1) {
    v += a * noise(p*f);
    f *= 2.0;
    a *= 0.5;
  }
  return v;
}

@fragment
fn fs_fire_building(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {

  // base stone/wood color (you can feed per-material)
  var base = vec3<f32>(0.48, 0.47, 0.46);

  let age = max(fp.time - fp.ignited_at, 0.0);

  // fire spread mask: grows from bottom to top + noisy tongues
  let climb = clamp(age * fp.spread_speed - world_pos.y * 0.35, 0.0, 1.0);
  let flick = fbm(world_pos.xz * 3.5 + vec2<f32>(fp.time * 0.6, -fp.time * 0.4));
  let fire_mask = clamp(climb + (flick - 0.5) * 0.25, 0.0, 1.0);

  // soot darkening where burned
  base = mix(base, fp.soot_color, fire_mask * fp.burn_strength);

  // emissive flames along edges/top surfaces
  let up = vec3<f32>(0.0,1.0,0.0);
  let facing_up = clamp(dot(normalize(world_nrm), up), 0.0, 1.0);
  let flame = fp.flame_color * fp.flame_intensity * fire_mask * (0.25 + facing_up * 0.75);

  // lighting
  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  var col = base * (0.25 + ndl * 1.0) + flame;

  return vec4<f32>(col, 1.0);
}


Gameplay hook

ignited_at = when hit by fire projectile

Increase spread_speed if oil/lava nearby

44) Decals: scorch marks (terrain/walls) — simple and robust

Best approach in custom engines:

Render decals as projected quads with alpha blending.

You don’t need true deferred decals; you can do forward-projected decals if you have depth.

44a) Screen-space decal (depth reconstruct) – scorch stamp

This is a post pass that draws scorch where you want by sampling a “decal list” (most engines do CPU-driven decals).
If you don’t want a list, easiest is: render a few decal meshes in world space.

Option A (mesh decal): scorch_decal_mesh.wgsl
struct DecalParams {
  color: vec3<f32>,   // (0.08,0.05,0.05)
  strength: f32,      // 0.2..0.8
};

@group(1) @binding(0) var<uniform> d: DecalParams;

@fragment
fn fs_scorch_decal(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
  // radial falloff
  let p = uv - vec2<f32>(0.5);
  let r = length(p);

  // irregular edge via cheap noise-ish
  let edge = smoothstep(0.48, 0.15, r);
  let a = edge * d.strength;

  return vec4<f32>(d.color, a);
}


Blend: alpha over.
Place: align decal quad to ground or wall at impact point.

45) Unit hit shader (flash + rim + micro-dissolve)

Use when a troop is hit — makes combat readable.

unit_hit_flash.wgsl
struct HitParams {
  time: f32,
  hit_time: f32,
  duration: f32,     // 0.08..0.18
  flash_color: vec3<f32>, // (1.0,0.95,0.8)
  rim_color: vec3<f32>,   // team color or white
  strength: f32,     // 0.5..2.0
};

@group(1) @binding(0) var<uniform> h: HitParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(12.9898,78.233))) * 43758.5453);
}

@fragment
fn fs_unit_hit(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {

  // base (pretend cloth)
  var col = vec3<f32>(0.25, 0.65, 0.35);

  // lighting
  let n = normalize(world_nrm);
  let l = normalize(light_dir);
  let ndl = max(dot(n, l), 0.0);
  col *= (0.30 + ndl * 1.0);

  // hit pulse
  let dt = (h.time - h.hit_time) / h.duration;
  let pulse = clamp(1.0 - dt, 0.0, 1.0);

  // rim highlight
  let v = normalize(view_pos - world_pos);
  let rim = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 4.0);

  col += h.flash_color * pulse * h.strength;
  col += h.rim_color * rim * pulse * (h.strength * 0.6);

  // tiny “chip” dissolve on hit (optional)
  let chip = hash(world_pos.xz * 50.0);
  if (chip > (0.98 + (1.0 - pulse) * 0.02)) { discard; }

  return vec4<f32>(col, 1.0);
}

46) Conquest capture wave (hex takeover animation)

When you capture the enemy flag, you want a visible wave sweeping across enemy tiles to show conquest.

Approach

Each hex tile has owner_id and capture_t (0..1 progress) or captured_at timestamp.

Shader blends from enemy tint → friendly tint with a wavefront.

hex_capture_wave.wgsl
struct CaptureParams {
  time: f32,
  wave_speed: f32,      // 2..12 world units per second
  wave_width: f32,      // 1..6 world units
  friendly: vec3<f32>,
  enemy: vec3<f32>,
  neutral: vec3<f32>,
  wave_color: vec3<f32>, // highlight band (1.0,0.85,0.25)
};

@group(1) @binding(0) var<uniform> cap: CaptureParams;

fn owner_color(owner: u32) -> vec3<f32> {
  if (owner == 1u) { return cap.friendly; }
  if (owner == 2u) { return cap.enemy; }
  return cap.neutral;
}

@fragment
fn fs_hex_capture(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
  @location(4) owner_id: u32,
  @location(5) prev_owner: u32,     // what it was before wave
  @location(6) captured_at: f32,    // timestamp when wave started for this tile (or global)
  @location(7) wave_origin: vec2<f32>, // world xz origin of capture (flag position)
) -> @location(0) vec4<f32> {

  // Base terrain shading
  var base = vec3<f32>(0.22, 0.72, 0.33);
  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  base *= (0.28 + ndl);

  // Wave distance in world space
  let d = distance(world_pos.xz, wave_origin);

  // wave front position over time
  let t = (cap.time - captured_at) * cap.wave_speed;

  // blend zone around wavefront
  let w = cap.wave_width;
  let front = smoothstep(t - w, t, d) - smoothstep(t, t + w, d); // band

  // captured progress (behind the wave is captured)
  let progress = smoothstep(t - w, t, d); // 0 ahead, 1 behind

  let from = owner_color(prev_owner);
  let to = owner_color(owner_id);

  // tint the tile
  var tint = mix(from, to, progress);

  // add a bright band at the front
  tint += cap.wave_color * front * 0.35;

  // apply tint subtly to preserve terrain
  let col = base * mix(vec3<f32>(1.0), tint, 0.18);

  return vec4<f32>(col, 1.0);
}


How to drive it

On victory: set wave_origin = enemy_flag_pos.xz, set captured_at = now for all tiles (or per tile staggered)

prev_owner = 2 (enemy), owner_id = 1 (player) once wave passes (or just animate the blend and then swap ownership)

Destruction particles tied to shader (requested)

Connect Batch 7 destruction to this batch:

When building enters fs_destroy or fs_fire_building, spawn particles:

sparks (additive)

debris chunks (instanced)

smoke puffs (alpha)

When capture wave runs, spawn subtle “victory motes” around tiles

If you want, I can give you a single unified “Event → spawn particles” struct layout and a GPU compute updater for particles in wgpu 0.27 next.


47) Screen-space outlines (depth + normal edge detect)

This outlines silhouettes/edges without ID masks. Best used subtly (roguelike readability).

Inputs

scene_tex (HDR or LDR)

depth_tex

normal_tex (optional but recommended). If you don’t have it, just do depth edges.

47a) outline_depth_normal.wgsl
struct OutlineDNParams {
  thickness_px: f32,     // 1..2
  depth_strength: f32,   // 0.6..2.0
  normal_strength: f32,  // 0.3..1.5
  color: vec3<f32>,      // (0.0,0.0,0.0) or team tint
};

@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_samp: sampler;

@group(0) @binding(2) var depth_tex: texture_depth_2d;

// Optional normal buffer (Rgba16Float or Rgba8Snorm etc)
@group(0) @binding(3) var normal_tex: texture_2d<f32>;
@group(0) @binding(4) var normal_samp: sampler;

@group(0) @binding(5) var<uniform> p: OutlineDNParams;

struct VSOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VSOut {
  var q = array<vec2<f32>, 3>(vec2<f32>(-1.0,-1.0), vec2<f32>(3.0,-1.0), vec2<f32>(-1.0,3.0));
  var o: VSOut;
  o.pos = vec4<f32>(q[vid], 0.0, 1.0);
  o.uv = (o.pos.xy * 0.5) + vec2<f32>(0.5);
  return o;
}

fn depth_at(uv: vec2<f32>) -> f32 {
  let dim = vec2<f32>(textureDimensions(depth_tex));
  let pxy = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.9999)) * dim);
  return textureLoad(depth_tex, pxy, 0);
}

fn normal_at(uv: vec2<f32>) -> vec3<f32> {
  // assuming stored as 0..1, remap to -1..1
  let n = textureSample(normal_tex, normal_samp, uv).xyz * 2.0 - vec3<f32>(1.0);
  return normalize(n);
}

@fragment
fn fs_outline(in: VSOut) -> @location(0) vec4<f32> {
  var col = textureSample(scene_tex, scene_samp, in.uv).rgb;

  let dim = vec2<f32>(textureDimensions(depth_tex));
  let texel = 1.0 / dim;
  let r = p.thickness_px;

  let d0 = depth_at(in.uv);

  // depth edge: compare against 4 neighbors
  let d1 = depth_at(in.uv + vec2<f32>( r, 0.0) * texel);
  let d2 = depth_at(in.uv + vec2<f32>(-r, 0.0) * texel);
  let d3 = depth_at(in.uv + vec2<f32>(0.0,  r) * texel);
  let d4 = depth_at(in.uv + vec2<f32>(0.0, -r) * texel);

  let dd = abs(d0 - d1) + abs(d0 - d2) + abs(d0 - d3) + abs(d0 - d4);
  let depth_edge = smoothstep(0.002, 0.01, dd) * p.depth_strength;

  // normal edge (optional): large change in normals
  let n0 = normal_at(in.uv);
  let n1 = normal_at(in.uv + vec2<f32>( r, 0.0) * texel);
  let n2 = normal_at(in.uv + vec2<f32>(-r, 0.0) * texel);
  let n3 = normal_at(in.uv + vec2<f32>(0.0,  r) * texel);
  let n4 = normal_at(in.uv + vec2<f32>(0.0, -r) * texel);

  let nd = (1.0 - dot(n0, n1)) + (1.0 - dot(n0, n2)) + (1.0 - dot(n0, n3)) + (1.0 - dot(n0, n4));
  let normal_edge = smoothstep(0.05, 0.20, nd) * p.normal_strength;

  let edge = clamp(depth_edge + normal_edge, 0.0, 1.0);

  // dark outline
  col = mix(col, col * 0.7 + p.color * 0.3, edge);

  return vec4<f32>(col, 1.0);
}


Tip: Keep subtle. Too strong makes it look “cartoony” unless you want that.

48) Rain streak particles + puddle darkening

Two parts:

Rain streaks: additive/alpha billboards falling

Puddles: darken + increase spec on flat surfaces

48a) Rain streak particle shader

Render as long thin quads aligned to velocity (or simple vertical quads).

rain_streaks.wgsl
struct RainParams {
  time: f32,
  color: vec3<f32>,     // (0.7,0.8,1.0)
  intensity: f32,       // 0.1..0.6
};

@group(1) @binding(0) var<uniform> r: RainParams;

struct FSIn { @location(0) uv: vec2<f32>, @location(1) life01: f32 };

@fragment
fn fs_rain(in: FSIn) -> @location(0) vec4<f32> {
  // uv.x across width, uv.y along length
  let width = smoothstep(0.5, 0.0, abs(in.uv.x - 0.5) * 2.0);
  let head = smoothstep(0.0, 0.2, in.uv.y);
  let tail = smoothstep(1.0, 0.6, in.uv.y);
  let a = width * head * tail * in.life01 * r.intensity;

  return vec4<f32>(r.color, a);
}


Blend: alpha or additive (alpha looks more realistic).

48b) Puddle darkening + wet spec (material helper)

Add to terrain/castle shaders in storm:

struct PuddleParams {
  amount: f32,          // 0..1
  puddle_strength: f32, // 0.05..0.25
};

@group(1) @binding(9) var<uniform> pd: PuddleParams;

fn puddle_mask(world_pos: vec3<f32>, normal: vec3<f32>) -> f32 {
  // puddles collect on flat areas; add variation by world noise
  let flat = pow(clamp(dot(normalize(normal), vec3<f32>(0.0,1.0,0.0)), 0.0, 1.0), 3.0);
  // cheap variation
  let v = fract(sin(dot(world_pos.xz, vec2<f32>(12.3, 45.6))) * 9999.0);
  return flat * smoothstep(0.4, 0.9, v) * pd.amount;
}

fn apply_puddles(col: vec3<f32>, world_pos: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
  let m = puddle_mask(world_pos, normal);
  // slightly darker & cooler
  let cool = vec3<f32>(0.92, 0.95, 1.05);
  return mix(col, col * (1.0 - pd.puddle_strength) * cool, m);
}


Use:

col = apply_puddles(col, world_pos, world_nrm);

49) Lightning that lights the entire scene (global flash)

This is a global light term you add to every fragment (or as a post multiply if you prefer).

You compute lightning intensity on CPU (or in a global uniform) and pass it in.

49a) Lightning uniform + helper
struct LightningGlobal {
  intensity: f32,       // 0..1 (spike quickly)
  color: vec3<f32>,     // (0.75,0.85,1.0)
  direction: vec3<f32>, // optional: where lightning comes from
};

@group(0) @binding(30) var<uniform> lg: LightningGlobal;

fn apply_lightning(col: vec3<f32>, normal: vec3<f32>) -> vec3<f32> {
  let n = normalize(normal);
  let l = normalize(lg.direction);
  let ndl = max(dot(n, l), 0.0);
  // lightning is mostly ambient flash + some directionality
  return col + lg.color * lg.intensity * (0.35 + ndl * 0.65);
}


Use:

col = apply_lightning(col, world_nrm);

CPU timing (simple, good-looking)

Every few seconds:

ramp intensity up in 0.05s

decay in 0.2–0.6s

Optional: during lightning, temporarily increase bloom strength a bit.

50) Castle breach shader (holes + interior glow + crumbling edge)

This makes “conquering” feel real: holes appear, edges glow, interior is dark/smoky.

Technique: a world-space noise field controls where pixels “break away”.

breach progresses with damage (0..1)

breach edges glow

interior shows “hot” or “dark” depending on style

castle_breach.wgsl
struct BreachParams {
  damage: f32,          // 0..1
  time: f32,
  edge_glow: f32,       // 0.5..2.5 (HDR)
  glow_color: vec3<f32>,// (2.0,0.6,0.15) lava-like
  interior_color: vec3<f32>, // (0.08,0.06,0.08)
};

@group(1) @binding(0) var<uniform> br: BreachParams;

fn hash(p: vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1,311.7))) * 43758.5453);
}
fn noise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0,0.0));
  let c = hash(i + vec2<f32>(0.0,1.0));
  let d = hash(i + vec2<f32>(1.0,1.0));
  let u = f*f*(3.0-2.0*f);
  return mix(mix(a,b,u.x), mix(c,d,u.x), u.y);
}
fn fbm(p: vec2<f32>) -> f32 {
  var v = 0.0;
  var a = 0.5;
  var f = 1.0;
  for (var i=0; i<5; i=i+1) {
    v += a * noise(p*f);
    f *= 2.0;
    a *= 0.5;
  }
  return v;
}

@fragment
fn fs_castle_breach(
  @location(0) world_pos: vec3<f32>,
  @location(1) world_nrm: vec3<f32>,
  @location(2) view_pos: vec3<f32>,
  @location(3) light_dir: vec3<f32>,
) -> @location(0) vec4<f32> {

  // base stone
  var stone = vec3<f32>(0.48, 0.47, 0.46);

  // breach mask: more damage => more holes
  let n = fbm(world_pos.xz * 2.4 + vec2<f32>(0.0, br.time * 0.05));
  let threshold = 1.0 - br.damage;           // damage raises cutout
  let cut = smoothstep(threshold - 0.12, threshold + 0.12, n);

  // hole region
  if (cut < 0.10) {
    // interior is dark; faint glowing embers
    let ember = smoothstep(0.0, 1.0, fbm(world_pos.xz * 8.0 + br.time)) * 0.08;
    let inside = br.interior_color + br.glow_color * ember;
    return vec4<f32>(inside, 1.0);
  }

  // crumbling edge glow near the cut boundary
  let edge = smoothstep(0.10, 0.18, cut) * (1.0 - smoothstep(0.18, 0.28, cut));

  // lighting
  let ndl = max(dot(normalize(world_nrm), normalize(light_dir)), 0.0);
  var col = stone * (0.25 + ndl * 1.05);

  // soot with damage
  col = mix(col, vec3<f32>(0.08,0.06,0.06), br.damage * 0.55);

  // edge glow (HDR)
  col += br.glow_color * edge * br.edge_glow;

  return vec4<f32>(col, 1.0);
}


Drive it

damage from building HP percentage inverted: damage = 1 - hp/hp_max


Quick recommended post chain for your scene

If you implement most batches, this order looks amazing:

Sky procedural

Opaque forward pass → HDR + Depth (+ optional normal)

Lava heat distortion (mask-based)

Fog post (depth-based)

Ash wind overlay

Outlines (ID or depth/normal)

Bloom extract → blur → combine

Tonemap (ACES)

Final vignette + grain