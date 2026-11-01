// Stylized map shader. Every colour that leaves fs_main is one of the 48 palette entries:
// ground, water, light, cloud and buildings all snap to it. No blending, no antialiasing: hard
// edges by construction. Ground is flat palette colour per class patch; dither exists only in
// a narrow band across the border between two classes. Constants RAMP_*, STEPS and PALETTE_LEN are
// prepended by the host from palette.rs.

struct Globals {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    sun: vec4<f32>,        // xyz toward the light, w = 1 at night
    proj: vec4<f32>,       // lon0, lat0, km per degree lon, km per degree lat
    earth: vec4<f32>,      // west, south, east, north of the earth and cloud images
    mask_b: vec4<f32>,     // west, south, east, north of the mask
    mask_dim: vec4<f32>,   // width, height, pixels per degree, dither band half width in pixels
    cloud: vec4<f32>,      // threshold, core threshold, altitude km, enabled
    cloud2: vec4<f32>,     // shadow offset (degrees lon, lat), shadow texel km, map mode ramp
    road: vec4<f32>,       // interstate, main road and railway widths in pixels, detail on (0 or 1)
    tex: vec4<f32>,        // texture texel in km, texture on (0 or 1), render width and height in pixels
    ground_b: vec4<f32>,   // west, south, east, north of the ground class raster
    ground_dim: vec4<f32>, // width, height, texels per degree, unused
    palette: array<vec4<f32>, PALETTE_LEN>,
    day: array<vec4<f32>, PALETTE_LEN>,
    anim: vec4<f32>,       // animation clock in seconds (wrapped), cloud drift (image u, v), unused
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var earth_tex: texture_2d<f32>;
@group(0) @binding(2) var cloud_tex: texture_2d<f32>;
@group(0) @binding(3) var mask_tex: texture_2d<f32>;
@group(0) @binding(4) var shadow_tex: texture_depth_2d;
@group(0) @binding(5) var nearest: sampler;
@group(0) @binding(6) var shadow_cmp: sampler_comparison;
@group(0) @binding(7) var linear: sampler;
@group(0) @binding(8) var ground_tex: texture_2d<f32>;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) colour: vec4<f32>,
    @location(3) local: vec3<f32>,
    @location(4) uv: vec2<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) colour: vec3<f32>,
    @location(3) @interpolate(flat) kind: u32,
    @location(4) local: vec3<f32>,
    @location(5) uv: vec2<f32>,
};

@vertex
fn vs_main(v: VIn) -> VOut {
    var o: VOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.normal = v.normal;
    o.colour = v.colour.rgb;
    o.kind = u32(round(v.colour.a * 255.0));
    o.local = v.local;
    o.uv = v.uv;
    return o;
}

@vertex
fn vs_shadow(v: VIn) -> @builtin(position) vec4<f32> {
    return g.light_vp * vec4<f32>(v.pos, 1.0);
}

fn pal(ramp: u32, step: i32) -> vec3<f32> {
    return g.palette[ramp * STEPS + u32(clamp(step, 0, i32(STEPS) - 1))].rgb;
}

// 8x8 Bayer threshold in [0, 1).
fn bayer8(p: vec2<f32>) -> f32 {
    let x = u32(p.x) & 7u;
    let y = (u32(p.y) & 7u) ^ x;
    let i = ((y & 1u) << 5u) | ((x & 1u) << 4u) | ((y & 2u) << 2u) | ((x & 2u) << 1u) | ((y & 4u) >> 1u) | ((x & 4u) >> 2u);
    return (f32(i) + 0.5) / 64.0;
}

fn lonlat(xy: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(g.proj.x + xy.x / g.proj.z, g.proj.y + xy.y / g.proj.w);
}

fn mask_at(ll: vec2<f32>) -> vec4<f32> {
    let ppd = g.mask_dim.z;
    let x = clamp(i32(floor((ll.x - g.mask_b.x) * ppd)), 0, i32(g.mask_dim.x) - 1);
    let y = clamp(i32(floor((g.mask_b.w - ll.y) * ppd)), 0, i32(g.mask_dim.y) - 1);
    return textureLoad(mask_tex, vec2<i32>(x, y), 0) * 255.0;
}

fn is_water(m: vec4<f32>) -> bool {
    return m.b > 0.5 && m.b < 2.5;
}

fn earth_uv(ll: vec2<f32>) -> vec2<f32> {
    return vec2<f32>((ll.x - g.earth.x) / (g.earth.z - g.earth.x), (g.earth.w - ll.y) / (g.earth.w - g.earth.y));
}

// A coordinate folded back and forth into 0..1 (period 2), so the image is read as if mirrored
// at its edges: the drifting clouds never meet a seam.
fn mirror01(x: f32) -> f32 {
    let m = x - 2.0 * floor(x * 0.5);
    return 1.0 - abs(m - 1.0);
}

fn cloud_alpha(ll: vec2<f32>) -> f32 {
    let uv = earth_uv(ll) - g.anim.yz;
    return textureSampleLevel(cloud_tex, linear, vec2<f32>(mirror01(uv.x), mirror01(uv.y)), 0.0).a;
}

// 1 when the point is lit by the shadow-casting light, 0 when a caster is in front of it.
fn lit(world: vec3<f32>, normal: vec3<f32>) -> f32 {
    let p = world + normal * g.cloud2.z * 1.5;
    let c = g.light_vp * vec4<f32>(p, 1.0);
    let uv = vec2<f32>(c.x * 0.5 + 0.5, 0.5 - c.y * 0.5);
    if (c.z > 1.0 || c.z < 0.0 || uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return 1.0;
    }
    return textureSampleCompareLevel(shadow_tex, shadow_cmp, uv, c.z - 0.0008);
}

fn ground_tc(ll: vec2<f32>) -> vec2<f32> {
    let ppd = g.ground_dim.z;
    return vec2<f32>((ll.x - g.ground_b.x) * ppd, (g.ground_b.w - ll.y) * ppd);
}

fn class_at(tc: vec2<i32>) -> u32 {
    let c = clamp(tc, vec2<i32>(0, 0), vec2<i32>(i32(g.ground_dim.x) - 1, i32(g.ground_dim.y) - 1));
    return u32(round(textureLoad(ground_tex, c, 0).r * 255.0));
}

// Ground class at a point. With `band` > 0 the pixels within `band` screen pixels of a border
// to another class pick that class with a probability that falls from one half at the border
// to zero at the band edge (ordered dither), so a transition is a few pixels of stipple and
// the middle of a patch is always one flat colour.
fn ground_class(tcf: vec2<f32>, fw: vec2<f32>, frag: vec2<f32>, band: f32) -> u32 {
    let tc = vec2<i32>(floor(tcf));
    let f = tcf - floor(tcf);
    let own = class_at(tc);
    if (band <= 0.0) { return own; }
    let rate = max(fw, vec2<f32>(1e-4, 1e-4));
    var best_d = 1e9;
    var other = own;
    let dist = array<f32, 4>((1.0 - f.x) / rate.x, f.x / rate.x, (1.0 - f.y) / rate.y, f.y / rate.y);
    let step = array<vec2<i32>, 4>(vec2<i32>(1, 0), vec2<i32>(-1, 0), vec2<i32>(0, 1), vec2<i32>(0, -1));
    for (var i = 0; i < 4; i++) {
        let n = class_at(tc + step[i]);
        if (n != own && dist[i] < best_d) { best_d = dist[i]; other = n; }
    }
    if (best_d < band) {
        let p = 0.5 * (1.0 - best_d / band);
        if (bayer8(frag) < p) { return other; }
    }
    return own;
}

fn variant_at(tc: vec2<i32>) -> u32 {
    let c = clamp(tc, vec2<i32>(0, 0), vec2<i32>(i32(g.ground_dim.x) - 1, i32(g.ground_dim.y) - 1));
    return u32(round(textureLoad(ground_tex, c, 0).g * 255.0));
}

// Hand authored 8x8 pixel tiles, one nibble a texel, a row a word. 0 is the class colour, 1 one
// step darker, 2 one step lighter. They tile in world space, so they never swim with the camera.
const TILE_FOREST = array<u32, 8>(0x02200000u, 0x02210000u, 0x00110220u, 0x00000221u, 0x00000011u, 0x22000000u, 0x22102200u, 0x01102210u);
const TILE_ROCK = array<u32, 8>(0x00100002u, 0x00002000u, 0x10000010u, 0x00200000u, 0x00001000u, 0x02000002u, 0x00010000u, 0x00000200u);
const TILE_SOIL = array<u32, 8>(0u, 0u, 0u, 0u, 0u, 0u, 0u, 0u);
const TILE_SNOW = array<u32, 8>(0x00000000u, 0x01110000u, 0x00000000u, 0x00000000u, 0x00000001u, 0x00000000u, 0x11100000u, 0x00000000u);
const TILE_WAVE = array<u32, 8>(0x00000000u, 0x02220000u, 0x00000000u, 0x00000000u, 0x00000000u, 0x00000222u, 0x00000000u, 0x00000000u);

fn nib(row: u32, tx: i32) -> u32 {
    return (row >> (28u - 4u * u32(tx & 7))) & 15u;
}

fn tile_value(t: array<u32, 8>, tx: i32, ty: i32) -> u32 {
    return nib(t[u32(ty & 7)], tx);
}

fn ihash(a: i32, b: i32) -> u32 {
    var h = u32(a) * 0x9E3779B1u ^ u32(b) * 0x85EBCA6Bu;
    h = h ^ (h >> 15u);
    h = h * 0x2C1B3C6Du;
    h = h ^ (h >> 12u);
    h = h * 0x297A2D39u;
    return h ^ (h >> 15u);
}

// A tile value with the tile mirrored and turned per 8x8 block of the world, so one hand drawn
// tile gives eight looks and the repeat is not visible over kilometres.
fn tile_oriented(t: array<u32, 8>, tx: i32, ty: i32, h: u32) -> u32 {
    var lx = tx & 7;
    var ly = ty & 7;
    if ((h & 1u) != 0u) { lx = 7 - lx; }
    if ((h & 2u) != 0u) { ly = 7 - ly; }
    if ((h & 4u) != 0u) { let t0 = lx; lx = ly; ly = t0; }
    return nib(t[u32(ly)], lx);
}

const TILE_FOREST2 = array<u32, 8>(0x00022000u, 0x00022100u, 0x01000110u, 0x02200000u, 0x02210002u, 0x00110022u, 0x00000021u, 0x10000001u);
const TILE_ROCK2 = array<u32, 8>(0x00000200u, 0x01000000u, 0x00002001u, 0x00000000u, 0x20010000u, 0x00000020u, 0x00100100u, 0x02000000u);

fn imod(a: i32, m: i32) -> i32 {
    return ((a % m) + m) % m;
}

// Crop of a field patch this season: 0 plain green, 1 light (ripe, or young and pale), 2 dark
// (ploughed, or dense), 3 pasture. The patch's own crop number picks a column, the season a
// row. Light and dark patches come in equal numbers each season, so the mean colour of the
// fields stays put when the crops drop out a few zoom steps further out.
fn crop_state(variant: u32) -> u32 {
    let c = (variant >> 2u) & 3u;
    let season = i32(round(g.ground_dim.w));
    if (c == 3u) { return 3u; }
    if (season == 2) { if (c == 2u) { return 1u; } if (c == 1u) { return 2u; } return 0u; }
    if (season == 3) { if (c == 0u) { return 1u; } if (c == 1u) { return 2u; } return 0u; }
    if (c == 2u) { return 2u; }
    if (c == 1u) { return 1u; }
    return 0u;
}

fn crown(tx: i32, ty: i32) -> bool {
    // Crowns are a 2x2 texel block, the odd rows of blocks shifted by one texel so they are not a grid.
    let by = ty >> 1;
    let sx = tx + select(0, 1, (by & 1) == 1);
    return ihash(sx >> 1, by) % 16u < 5u;
}

// Class texture at a world texel: 0 the class colour, -1 one step darker, 1 one step lighter.
// Every pattern keeps about the same share of dark and light at any texel size, so the mean
// colour does not jump when the texel doubles as the camera pulls back.
fn class_texel(cls: u32, variant: u32, tx: i32, ty: i32) -> i32 {
    var v = 0u;
    let h8 = ihash(tx >> 3, ty >> 3);
    if (cls == 0u) {
        // Canopy: crowns two texels across on a hashed lattice, each casting a one texel
        // shadow down and to the left of the sun side.
        if (crown(tx, ty)) { v = 2u; }
        else if (crown(tx + 1, ty - 1)) { v = 1u; }
    }
    else if (cls == 1u) {
        // Rows run east-west or north-south by patch (never diagonal), one dark row in four or
        // five. Pasture is sparse speckle instead.
        let crop = crop_state(variant);
        let along = select(ty, tx, (variant & 1u) == 1u);
        let period = select(4, 5, (variant & 2u) != 0u);
        if (crop == 3u) {
            let r = ihash(tx, ty + 311) % 24u;
            if (r == 0u) { v = 1u; } else if (r == 1u) { v = 2u; }
        } else if (crop == 2u) {
            if (imod(along, 3) == 0) { v = 1u; }
        } else {
            if (imod(along, period) == 0) { v = 1u; }
        }
    }
    else if (cls == 2u) {
        // Scrub and desert: bare, with a sparse scatter of darker brush and pale sand.
        let r = ihash(tx + 57, ty) % 36u;
        if (r == 0u) { v = 1u; } else if (r == 1u) { v = 2u; }
    }
    else if (cls == 3u) {
        if (((h8 >> 3u) & 1u) == 1u) { v = tile_oriented(TILE_ROCK2, tx, ty, h8); }
        else { v = tile_oriented(TILE_ROCK, tx, ty, h8); }
    }
    else if (cls == 4u) {
        v = tile_oriented(TILE_SNOW, tx, ty, h8);
    }
    // City ground (classes 5 and 6) is drawn by city_texel.
    if (v == 1u) { return -1; }
    if (v == 2u) { return 1; }
    return 0;
}

// ---- City ground ---------------------------------------------------------------------------
// A city is not one repeating grid. The raster says which zone a texel is in (class 6 downtown,
// class 5 with a zone number: 0 outer residential, 1 inner city, 2 industrial and port). Streets
// are placed per axis from a hash of the street's own index, so every street runs unbroken
// across tiles, while the gaps between them differ block to block. A second, wider avenue grid
// is independent of the first, and the offsets shift every 64 texels so no 5 km piece repeats.

// Position of one axis' street: x = 1 on a street texel, y = the index of the block between streets.
fn axis_street(t: i32, band: i32, salt: i32, span: i32, lo: i32, rng: u32, width: i32) -> vec2<i32> {
    let k = (t - imod(t, span)) / span;
    let off = k * span + lo + i32(ihash(k + salt, band) % rng);
    let on = select(0, 1, t >= off && t < off + width);
    return vec2<i32>(on, select(k - 1, k, t >= off));
}

// Returns (tone, green): tone -2 avenue, -1 street, 0 block, 1 light block; green 0 none, 1 lawn,
// 2 tree, 3 dark lawn of a park.
fn city_texel(cls: u32, zone: u32, tx: i32, ty: i32) -> vec2<i32> {
    var span = 8;
    var lo = 2;
    var rng = 4u;
    var av = 32;
    if (cls == 6u) { span = 6; lo = 2; rng = 2u; av = 24; }
    else if (zone == 0u) { span = 12; lo = 3; rng = 6u; av = 48; }
    else if (zone == 2u) { span = 20; lo = 4; rng = 10u; av = 64; }
    // Avenues: two texels wide, continuous over the whole map.
    let avx = imod(tx - i32(ihash(tx >> 6, 91) % 5u), av);
    let avy = imod(ty - i32(ihash(ty >> 6, 92) % 5u), av);
    if (avx < 2 || avy < 2) { return vec2<i32>(select(-2, -1, cls == 5u && zone != 1u), 0); }
    let sx = axis_street(tx, ty >> 6, 11, span, lo, rng, 1);
    let sy = axis_street(ty, tx >> 6, 23, span, lo, rng, 1);
    if (sx.x == 1 || sy.x == 1) { return vec2<i32>(-1, 0); }
    let bh = ihash(sx.y * 7 + 3, sy.y * 13 + 5) % 100u;
    let th = ihash(tx + 17, ty + 29) % 100u;
    if (cls == 6u) {
        // Downtown: tight blocks, most of them the lightest.
        if (bh < 72u) { return vec2<i32>(1, 0); }
        return vec2<i32>(0, 0);
    }
    if (zone == 0u) {
        // Outer residential: lawns between the houses, whole green blocks, scattered trees.
        if (bh < 30u) { return vec2<i32>(0, select(1, 2, th < 14u)); }
        if (bh < 38u) { return vec2<i32>(1, 0); }
        if (th < 12u) { return vec2<i32>(0, 1); }
        if (th < 16u) { return vec2<i32>(0, 2); }
        return vec2<i32>(0, 0);
    }
    if (zone == 2u) {
        // Industrial and port: big grey lots, now and then a lighter roof.
        if (bh < 22u) { return vec2<i32>(1, 0); }
        return vec2<i32>(0, 0);
    }
    // Inner city: medium blocks, a few light ones and the odd park.
    if (bh < 7u) { return vec2<i32>(0, select(3, 2, th < 10u)); }
    if (bh < 30u) { return vec2<i32>(1, 0); }
    return vec2<i32>(0, 0);
}

// A lit pixel on city ground at night: a block texel with a lamp or a window behind it. How many
// depends on the zone (per mille: downtown 140, inner city 70, outer residential 28, industrial and
// port 8), times `boost` where the ground is too coarse to show blocks. Returns the step of the signal ramp, or -1 for none. A texel flickers off for a couple
// of seconds now and then, like the windows on the buildings.
fn city_light(cls: u32, zone: u32, tx: i32, ty: i32, boost: u32) -> i32 {
    var per_mille = 70u;
    if (cls == 6u) { per_mille = 140u; }
    else if (zone == 0u) { per_mille = 28u; }
    else if (zone == 2u) { per_mille = 8u; }
    per_mille = per_mille * boost;
    let h = ihash(tx * 3 + 101, ty * 5 + 7);
    if (h % 1000u >= per_mille) { return -1; }
    let flick = ihash(tx + 977, ty + i32(floor(g.anim.x * 0.5))) % 160u == 0u;
    if (flick) { return -1; }
    // One lamp in four is the warmer copper step.
    return select(1, 3, ((h >> 12u) & 3u) == 0u);
}

fn ramp_of(cls: u32) -> u32 {
    if (cls == 0u) { return RAMP_FOREST; }
    if (cls == 1u) { return RAMP_FIELD; }
    if (cls == 2u) { return RAMP_SOIL; }
    if (cls == 3u) { return RAMP_ROCK; }
    if (cls == 4u) { return RAMP_SNOW; }
    return RAMP_URBAN;
}

// Step of the gold ramp for each ground class: the ground stays readable under the gold.
fn gold_base(cls: u32) -> i32 {
    if (cls == 0u) { return 1; }
    if (cls == 3u) { return 1; }
    if (cls == 5u || cls == 6u) { return 3; }
    return 2;
}

fn nearest_colour(c: vec3<f32>) -> vec2<i32> {
    var best = 0u;
    var best_d = 1e9;
    for (var i = 0u; i < PALETTE_LEN; i++) {
        let e = c - g.day[i].rgb;
        let d = dot(e * e, vec3<f32>(0.30, 0.59, 0.11));
        if (d < best_d) { best_d = d; best = i; }
    }
    return vec2<i32>(i32(best / STEPS), i32(best % STEPS));
}

fn hash3(p: vec3<f32>) -> f32 {
    return fract(sin(dot(p, vec3<f32>(12.9898, 78.233, 37.719))) * 43758.5453);
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    let frag = in.clip.xy;
    // Derivatives first, while control flow is uniform.
    let dpx = dpdx(in.world);
    let dpy = dpdy(in.world);
    let ll = lonlat(in.world.xy);
    let tcf = ground_tc(ll);
    let fw = fwidth(tcf);
    let fuv = fwidth(in.uv);
    let flo = fwidth(in.local);
    let n = normalize(in.normal);
    let sun = normalize(g.sun.xyz);
    let ndl = dot(n, sun);
    let night = g.sun.w > 0.5;
    let px_km = max(length(dpx.xy), length(dpy.xy));

    if (in.kind == 15u) {
        // A lamp (vehicle light, beacon): flat in its step of the signal ramp, day and night.
        return vec4<f32>(pal(RAMP_SIGNAL, i32(round(in.colour.r * 255.0))), 1.0);
    }

    if (in.kind == 4u) {
        // Cloud sheet: smooth hard-edged shapes. Sun side rim bright, body mid, a three pixel
        // underside band darker with a one pixel dark outline on the shadow side.
        let thr = g.cloud.x;
        let a = cloud_alpha(ll);
        if (a < thr || g.cloud.w < 0.5) { discard; }
        let away = -normalize(g.sun.xy);
        let outline = cloud_alpha(lonlat(in.world.xy + away * px_km * 2.6)) < thr
            || cloud_alpha(lonlat(in.world.xy - away * px_km * 1.6)) < thr;
        let band = cloud_alpha(lonlat(in.world.xy + away * px_km * 9.0)) < thr;
        let rim = cloud_alpha(lonlat(in.world.xy - away * px_km * 2.5)) < thr;
        var step = 2;
        if (a >= g.cloud.y || rim) { step = 3; }
        if (band) { step = 0; }
        if (outline) { return vec4<f32>(pal(RAMP_URBAN, 1), 1.0); }
        return vec4<f32>(pal(RAMP_CLOUD, step), 1.0);
    }

    if (in.kind == 6u || in.kind == 7u) {
        // The land and sea beyond the terrain: flat class colour per coarse cell, no texture,
        // no region lines. Class 7 is water, anything else a ground class.
        let acls = u32(round(in.colour.r * 255.0));
        let ash = lit(in.world, n);
        let in_shade = ash < 0.5 || ndl <= 0.0 || (g.cloud.w > 0.5 && cloud_alpha(ll + g.cloud2.xy) >= g.cloud.x);
        let wall = in.kind == 7u;
        let hi = wall && (1.0 - in.uv.y) / max(fuv.y, 1e-5) < 1.0;
        if (acls == 7u) {
            var wstep = 2;
            if (in_shade) { wstep = 1; }
            if (wall) { wstep = 0; }
            return vec4<f32>(pal(RAMP_WATER, wstep), 1.0);
        }
        var astep = 2;
        if (wall) { astep = 1; }
        if (hi) { astep = 3; }
        if (in_shade && !wall) { astep = 1; }
        return vec4<f32>(pal(ramp_of(acls), astep), 1.0);
    }

    if (in.kind == 0u || in.kind == 1u) {
        let m = mask_at(ll);
        let mr = mask_at(lonlat(in.world.xy + dpx.xy));
        let md = mask_at(lonlat(in.world.xy + dpy.xy));
        let ml = mask_at(lonlat(in.world.xy - dpx.xy));
        let mu = mask_at(lonlat(in.world.xy - dpy.xy));
        let sh = lit(in.world, n);
        let in_cloud = g.cloud.w > 0.5 && cloud_alpha(ll + g.cloud2.xy) >= g.cloud.x;
        var shade = 0;
        if (in_cloud) { shade = 2; }

        if (is_water(m)) {
            if (in.kind == 1u) { return vec4<f32>(pal(RAMP_WATER, 0), 1.0); }
            var w = 2;
            if (sh < 0.5 || ndl <= 0.0 || in_cloud) { w = 1; }
            // Shoreline: a one pixel lighter rim where the next pixel is land.
            if (!is_water(mr) || !is_water(md) || !is_water(ml) || !is_water(mu)) { w = 3; }
            else if (g.tex.y > 0.5) {
                // Shallows: the water within two texels of land is one step lighter, tested from
                // the texel centre so the band's edge is a clean staircase of whole texels.
                let q = (floor(in.world.xy / g.tex.x) + 0.5) * g.tex.x;
                let r2 = g.tex.x * 2.0;
                let shallow = !is_water(mask_at(lonlat(q + vec2<f32>(r2, 0.0))))
                    || !is_water(mask_at(lonlat(q - vec2<f32>(r2, 0.0))))
                    || !is_water(mask_at(lonlat(q + vec2<f32>(0.0, r2))))
                    || !is_water(mask_at(lonlat(q - vec2<f32>(0.0, r2))))
                    || !is_water(mask_at(lonlat(q + vec2<f32>(r2, r2) * 0.7)))
                    || !is_water(mask_at(lonlat(q - vec2<f32>(r2, r2) * 0.7)))
                    || !is_water(mask_at(lonlat(q + vec2<f32>(r2, -r2) * 0.7)))
                    || !is_water(mask_at(lonlat(q + vec2<f32>(-r2, r2) * 0.7)));
                if (shallow) { w = min(w + 1, 3); }
                // Open water: a sparse static scatter of wave texels, each block of rows
                // shifted so the dashes do not line up. Not in the shallows or near any shore.
                let tx = i32(floor(in.world.x / g.tex.x));
                let ty = i32(floor(in.world.y / g.tex.x));
                let hb = ihash(tx >> 3, ty >> 3);
                // The swell steps along: a texel at a time, every 1.5 s at sea and every 4.5 s on
                // a lake, each block of the world its own way, and a row now and then. The
                // periods divide the clock wrap by a multiple of 8, so the wrap is invisible.
                let lake = m.b > 1.5;
                let ws = i32(floor(g.anim.x / select(1.5, 4.5, lake)));
                let wy = i32(floor(g.anim.x / select(9.0, 18.0, lake)));
                let shift = i32(hb & 7u) + ws * select(1, -1, ((hb >> 6u) & 1u) == 1u);
                let far = !is_water(mask_at(lonlat(in.world.xy + dpx.xy * 8.0)))
                    || !is_water(mask_at(lonlat(in.world.xy - dpx.xy * 8.0)))
                    || !is_water(mask_at(lonlat(in.world.xy + dpy.xy * 8.0)))
                    || !is_water(mask_at(lonlat(in.world.xy - dpy.xy * 8.0)));
                let wv = tile_value(TILE_WAVE, tx + shift, ty + i32((hb >> 3u) & 3u) * 2 + wy);
                if (wv == 2u && !far && !shallow) { w = min(w + 1, 3); }
            }
            return vec4<f32>(pal(RAMP_WATER, w), 1.0);
        }

        // Holdings (class 2) are gold. Map modes (class 3 and up) paint the whole region in the
        // mode's ramp, one step per level. Both count as held for the region lines.
        let held = m.a > 1.5;
        let player = m.a > 1.5 && m.a < 2.5;
        let heat = m.a > 2.5;
        let cls = ground_class(tcf, fw, frag, select(g.mask_dim.w, 0.0, in.kind == 1u));
        var ramp = ramp_of(cls);
        var base = 2;
        if (player) { ramp = RAMP_GOLD; base = gold_base(cls); }
        if (heat) { ramp = u32(g.cloud2.w); base = clamp(i32(round(m.a)) - 2, 1, 3); }

        let tc = vec2<i32>(floor(tcf));
        if (cls == 1u && !player && !heat && g.tex.y > 0.5 && g.tex.x <= 1.0) {
            // Crops by the season, only while the texel is small: far out the fields are one green.
            let crop = crop_state(variant_at(tc));
            if (crop == 1u) { base = 3; }
            if (crop == 2u) { base = 1; }
        }
        if (in.kind == 1u) {
            // Terrace wall: one shade darker than the ground above, with a one pixel highlight
            // along its top edge. Crisp, flat.
            if ((1.0 - in.uv.y) / max(fuv.y, 1e-5) < 1.0) { return vec4<f32>(pal(ramp, base + 1 - shade), 1.0); }
            return vec4<f32>(pal(ramp, base - 1 - shade), 1.0);
        }

        // Region lines, exactly one pixel. Each edge belongs to one pixel only. Land only.
        if (m.a > 0.5) {
            var edge = false;
            var state_edge = false;
            let ms = array<vec4<f32>, 4>(mr, md, ml, mu);
            for (var i = 0; i < 4; i++) {
                let o = ms[i];
                let land = !is_water(o);
                let diff_r = land && o.r != m.r;
                let own = o.a < 0.5 || (i < 2 && (o.a < 1.5 || held)) || (held && o.a < 1.5);
                if (diff_r && own) {
                    edge = true;
                    if (o.g != m.g) { state_edge = true; }
                }
            }
            if (edge) {
                if (held) { return vec4<f32>(pal(RAMP_SIGNAL, 1), 1.0); }
                if (state_edge) { return vec4<f32>(pal(RAMP_CLOUD, 3), 1.0); }
                return vec4<f32>(pal(RAMP_CLOUD, 2), 1.0);
            }
        }

        // Flat ground colour; lit 2, shadow or cloud shadow one lower, both two lower.
        var step = base;
        if (sh < 0.5 || ndl <= 0.0) { step = step - 1; }
        step = step - shade;
        // Far out the city is a scatter of lit texels on its flat colour, three times as many as up
        // close where the streets and blocks take their share.
        if (night && cls >= 5u && !heat && g.tex.y < 0.5) {
            let lx = i32(floor(in.world.x / g.tex.x));
            let ly = i32(floor(in.world.y / g.tex.x));
            let lamp = city_light(cls, variant_at(tc) & 3u, lx, ly, 3u);
            if (lamp >= 0) { return vec4<f32>(pal(RAMP_SIGNAL, lamp), 1.0); }
        }
        // Pixel texture of the class, in world space, at a texel that is always a few screen
        // pixels wide; gone (flat class colour) once the camera is too far out.
        if (g.tex.y > 0.5 && !heat) {
            let tx = i32(floor(in.world.x / g.tex.x));
            let ty = i32(floor(in.world.y / g.tex.x));
            var tv = 0;
            if (cls >= 5u) {
                let ct = city_texel(cls, variant_at(tc) & 3u, tx, ty);
                tv = clamp(ct.x, -2, 1);
                if (night && ct.y == 0 && ct.x >= 0) {
                    let lamp = city_light(cls, variant_at(tc) & 3u, tx, ty, 1u);
                    if (lamp >= 0) { return vec4<f32>(pal(RAMP_SIGNAL, lamp), 1.0); }
                }
                if (ct.y != 0 && !player) {
                    if (ct.y == 2) { return vec4<f32>(pal(RAMP_FOREST, step - 1), 1.0); }
                    if (ct.y == 3) { return vec4<f32>(pal(RAMP_FOREST, step), 1.0); }
                    return vec4<f32>(pal(RAMP_FIELD, step - 1), 1.0);
                }
            } else {
                tv = class_texel(cls, variant_at(tc), tx, ty);
            }
            if (player) {
                // Gold holdings: every other textured texel shows the land through.
                if (tv != 0 && cls < 5u && ((tx + ty) & 1) == 0) { return vec4<f32>(pal(ramp_of(cls), 2 - shade), 1.0); }
                step = step + tv;
            } else {
                step = step + tv;
            }
        }
        // The player's gold ground never reaches the roof gold (step 3), so their buildings'
        // roofs are always the lightest thing on it.
        if (player) { step = min(step, 2); }
        return vec4<f32>(pal(ramp, step), 1.0);
    }

    // Buildings and plinths: nearest palette colour, shifted by the light step, with a one
    // pixel ink outline along every face edge so they pop off the ground.
    let sh = lit(in.world, n);
    var shade = 1;
    if (n.z > 0.5) { shade = 3; }
    else if (ndl > 0.1) { shade = 2; }
    if (sh < 0.5) { shade = min(shade, 1); }
    let base = nearest_colour(in.colour);
    var step2 = clamp(base.y + shade - 1, 0, 3);
    var out_c = pal(u32(base.x), step2);
    if (in.kind == 2u && abs(n.z) < 0.5) {
        // Window rows on the side faces: a one voxel texel in every two by two cells, dark by
        // day; at night about half of them are lit and the rest stay dark.
        var u = in.local.x;
        if (abs(n.x) > 0.5) { u = in.local.y; }
        let cell = vec3<f32>(floor(u * 0.5), floor(in.local.z * 0.5), floor(in.local.x + in.local.y));
        let inside = (fract(u * 0.5) < 0.5) && (fract(in.local.z * 0.5) > 0.5);
        if (inside) {
            // A lit window flips now and then (about one in a hundred, for two seconds at a time).
            let flick = hash3(cell + vec3<f32>(17.0, floor(g.anim.x * 0.5), 3.0)) > 0.992;
            let lit_now = (hash3(cell) > 0.45) != flick;
            if (night && lit_now) { out_c = pal(RAMP_SIGNAL, 1); }
            else if (flo.z < 0.34) { out_c = pal(RAMP_INK, 2 - select(0, 1, sh < 0.5)); }
        }
    }
    // A face under four and a half pixels across keeps no outline along that axis, so a distant
    // downtown block stays a lit shape instead of a black blot; the neighbouring faces still
    // carry the silhouette.
    var ex = 9.0;
    var ey = 9.0;
    if (1.0 / max(fuv.x, 1e-5) >= 4.5) { ex = min(in.uv.x, 1.0 - in.uv.x) / max(fuv.x, 1e-5); }
    if (1.0 / max(fuv.y, 1e-5) >= 4.5) { ey = min(in.uv.y, 1.0 - in.uv.y) / max(fuv.y, 1e-5); }
    if (min(ex, ey) < 1.0) { out_c = pal(RAMP_INK, 0); }
    return vec4<f32>(out_c, 1.0);
}


// Roads and railways. Each segment is a quad; the vertex shader widens it by a whole number of
// pixels in screen space and every pixel is either fully line or fully ground (no blending), so
// lines are crisp at every zoom. The fragment shader writes its own depth so the line closest to a pixel's centre wins
// where segments overlap, and the line always sits just in front of the ground it follows.
struct ROut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) @interpolate(flat) kind: u32,
    @location(2) @interpolate(linear) across: f32,
    @location(3) along: f32,
};

fn screen_of(c: vec4<f32>) -> vec2<f32> {
    return vec2<f32>((c.x / c.w * 0.5 + 0.5) * g.tex.z, (0.5 - c.y / c.w * 0.5) * g.tex.w);
}

// River ribbon width in pixels: the interstate's width, wider up close.
fn river_width() -> f32 {
    return select(g.road.x, g.road.x + 4.0, g.road.x > 6.5);
}

fn road_width(cls: u32) -> f32 {
    if (cls == 3u) { return river_width(); }
    // Bridge decks (4 + the carried road's class) are as wide as their road.
    let c = select(cls, cls - 4u, cls >= 4u);
    if (c == 0u) { return g.road.x; }
    if (c == 1u) { return g.road.y; }
    return g.road.z;
}

@vertex
fn vs_road(v: VIn) -> ROut {
    var o: ROut;
    let cls = u32(round(v.colour.x * 255.0));
    let this_c = g.view_proj * vec4<f32>(v.pos, 1.0);
    let other_c = g.view_proj * vec4<f32>(v.normal, 1.0);
    o.world = v.pos;
    o.kind = cls;
    o.across = 0.0;
    o.along = v.local.z;
    let bridge_deck = cls >= 4u;
    if (this_c.w < 1e-3 || other_c.w < 1e-3 || road_width(cls) < 0.5 || (bridge_deck && river_width() < 0.5)) {
        o.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        return o;
    }
    let p_this = screen_of(this_c);
    let p_other = screen_of(other_c);
    var d = p_other - p_this;
    if (v.local.y > 0.5) { d = -d; }   // always point from the first end to the second
    if (length(d) < 1e-3) { d = vec2<f32>(1.0, 0.0); }
    d = normalize(d);
    let nrm = vec2<f32>(-d.y, d.x);
    let ext = road_width(cls) * 0.5 + 1.0;
    // A deck runs along its road for the river's width plus two pixels of bank each side.
    let ext_along = select(ext, river_width() * 0.5 * v.local.z + 2.0, bridge_deck);
    let end_sign = select(-1.0, 1.0, v.local.y > 0.5);
    let p = p_this + nrm * v.local.x * ext + d * end_sign * ext_along;
    let ndc = vec2<f32>(p.x / g.tex.z * 2.0 - 1.0, 1.0 - p.y / g.tex.w * 2.0);
    o.clip = vec4<f32>(ndc * this_c.w, this_c.z, this_c.w);
    o.across = v.local.x * ext;
    return o;
}

struct RFOut {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

// Narrow water between two shores, tested at about 8 km each way: a road or rail line crossing
// it is a bridge, drawn over the water. Open lake and sea stay clear of lines.
fn narrow_water(p: vec2<f32>) -> bool {
    let r = 8.0;
    let e = !is_water(mask_at(lonlat(p + vec2<f32>(r, 0.0))));
    let w = !is_water(mask_at(lonlat(p - vec2<f32>(r, 0.0))));
    let n = !is_water(mask_at(lonlat(p + vec2<f32>(0.0, r))));
    let s = !is_water(mask_at(lonlat(p - vec2<f32>(0.0, r))));
    let ne = !is_water(mask_at(lonlat(p + vec2<f32>(r, r) * 0.7)));
    let sw = !is_water(mask_at(lonlat(p - vec2<f32>(r, r) * 0.7)));
    let nw = !is_water(mask_at(lonlat(p + vec2<f32>(-r, r) * 0.7)));
    let se = !is_water(mask_at(lonlat(p + vec2<f32>(r, -r) * 0.7)));
    return (e && w) || (n && s) || (ne && sw) || (nw && se);
}

@fragment
fn fs_road(in: ROut) -> RFOut {
    let fwa = fwidth(in.along);
    let ll = lonlat(in.world.xy);
    let m = mask_at(ll);
    let w = road_width(in.kind);
    let hw = w * 0.5;
    let d = abs(in.across);
    var bridge = false;
    if (in.kind == 3u) {
        // River: water body with a lighter rim on each bank once it is wide enough; the open
        // lake and sea keep their own water.
        if (d >= hw || is_water(m)) { discard; }
        var rc = pal(RAMP_WATER, 2);
        if (w > 4.5 && d >= hw - 1.0) { rc = pal(RAMP_WATER, 3); }
        var rout: RFOut;
        rout.colour = vec4<f32>(rc, 1.0);
        rout.depth = in.clip.z - 2.0e-5;
        return rout;
    }
    if (in.kind >= 4u) {
        // Bridge deck over a river: ink kerb, light deck.
        if (d >= hw) { discard; }
        var dc = pal(RAMP_ROCK, 3);
        if (w > 2.5 && d >= hw - 1.0) { dc = pal(RAMP_INK, 3); }
        var dout: RFOut;
        dout.colour = vec4<f32>(dc, 1.0);
        dout.depth = in.clip.z - 4.0e-4;
        return dout;
    }
    if (is_water(m)) {
        if (d >= hw || !narrow_water(in.world.xy)) { discard; }
        bridge = true;
    }
    if (d >= hw) { discard; }
    var c = pal(RAMP_ROCK, 2);
    let detail = g.road.w > 0.5;
    if (in.kind == 0u) {
        // Interstate: mid grey asphalt with a darker one pixel kerb. Up close a light edge line
        // inside each kerb and a gold dashed centre line.
        if (w > 2.5 && d >= hw - 1.0) { c = pal(RAMP_ROCK, 1); }
        else if (detail && w > 6.5 && d >= hw - 2.0) { c = pal(RAMP_CLOUD, 1); }
        else if (detail && w > 4.5 && d < 0.75 && fract(in.along / 7.0) < 0.5) { c = pal(RAMP_GOLD, 3); }
    } else if (in.kind == 1u) {
        // Main road: one step lighter than the ground it crosses, so it reads as a faint line.
        // Up close it widens to a grey road with a darker kerb.
        let cls = class_at(vec2<i32>(floor(ground_tc(ll))));
        var ramp = ramp_of(cls);
        var base = 2;
        if (m.a > 1.5 && m.a < 2.5) { ramp = RAMP_GOLD; base = gold_base(cls); }
        c = pal(ramp, base + 1);
        if (w > 2.5) {
            c = pal(RAMP_ROCK, 3);
            if (d >= hw - 1.0) { c = pal(RAMP_ROCK, 1); }
        }
    } else {
        // Railway: a thin light grey-brown line, with sleepers across it up close.
        c = pal(RAMP_SOIL, 3);
        if (detail && d >= 0.75) {
            if (fract(in.along / 3.5) < 0.43) { c = pal(RAMP_SOIL, 2); } else { discard; }
        }
    }
    if (bridge) {
        // Bridge deck: the kerb is ink, the deck a step lighter than the road, so it reads as a
        // structure crossing the water.
        if (in.kind != 2u && w > 2.5) {
            if (d >= hw - 1.0) { c = pal(RAMP_INK, 3); } else { c = pal(RAMP_URBAN, 3); }
        } else if (in.kind == 2u) {
            c = pal(RAMP_URBAN, 3);
        }
    }
    if (g.sun.w > 0.5 && detail && g.road.x > 6.5 && !bridge && in.kind <= 1u && w > 2.5 && d >= hw - 1.0) {
        // Streetlights at night: one lit pixel on a kerb every so often, alternating sides.
        // Dense in town, sparse along an interstate in the country, none on a country road.
        let urban = class_at(vec2<i32>(floor(ground_tc(ll)))) >= 5u;
        var spacing = 0.0;
        if (urban) { spacing = select(0.22, 0.3, in.kind == 0u); }
        else if (in.kind == 0u) { spacing = 2.0; }
        if (spacing > 0.0) {
            let idx = i32(floor(in.along / spacing));
            if (fract(in.along / spacing) < fwa / spacing && ((idx & 1) == 0) == (in.across > 0.0)) {
                c = pal(RAMP_SIGNAL, 1);
            }
        }
    }
    var out: RFOut;
    out.colour = vec4<f32>(c, 1.0);
    out.depth = in.clip.z - 4.0e-5 - 1.0e-5 * f32(in.kind) - 5.0e-6 * d / max(hw, 0.5);
    return out;
}
