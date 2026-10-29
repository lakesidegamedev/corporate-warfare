// Stylized map shader. Every colour that leaves fs_main is one of the 48 palette entries:
// ground, water, light, cloud and buildings all snap to it. No blending, no filtering, no
// antialiasing: hard edges by construction. Constants RAMP_*, STEPS and PALETTE_LEN are
// prepended by the host from palette.rs.

struct Globals {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    sun: vec4<f32>,        // xyz toward the light, w = 1 at night
    proj: vec4<f32>,       // lon0, lat0, km per degree lon, km per degree lat
    earth: vec4<f32>,      // west, south, east, north of the earth and cloud images
    mask_b: vec4<f32>,     // west, south, east, north of the mask
    mask_dim: vec4<f32>,   // width, height, pixels per degree, dither spread
    cloud: vec4<f32>,      // threshold, core threshold, altitude km, enabled
    cloud2: vec4<f32>,     // shadow offset (degrees lon, lat), shadow texel km, unused
    palette: array<vec4<f32>, PALETTE_LEN>,
    day: array<vec4<f32>, PALETTE_LEN>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var earth_tex: texture_2d<f32>;
@group(0) @binding(2) var cloud_tex: texture_2d<f32>;
@group(0) @binding(3) var mask_tex: texture_2d<f32>;
@group(0) @binding(4) var shadow_tex: texture_depth_2d;
@group(0) @binding(5) var nearest: sampler;
@group(0) @binding(6) var shadow_cmp: sampler_comparison;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) colour: vec4<f32>,
    @location(3) local: vec3<f32>,
};

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) colour: vec3<f32>,
    @location(3) @interpolate(flat) kind: u32,
    @location(4) local: vec3<f32>,
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

fn earth_uv(ll: vec2<f32>) -> vec2<f32> {
    return vec2<f32>((ll.x - g.earth.x) / (g.earth.z - g.earth.x), (g.earth.w - ll.y) / (g.earth.w - g.earth.y));
}

fn cloud_alpha(ll: vec2<f32>) -> f32 {
    return textureSampleLevel(cloud_tex, nearest, earth_uv(ll), 0.0).a;
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

// Nearest of the ramp colours at step 2, with ordered dithering across ramp boundaries.
fn ground_ramp(tex: vec3<f32>, frag: vec2<f32>) -> u32 {
    // Stretch the dark summer image so land cover separates.
    let c = (tex - vec3<f32>(0.03, 0.05, 0.02)) * 2.6 + (bayer8(frag) - 0.5) * g.mask_dim.w;
    var best = RAMP_FOREST;
    var best_d = 1e9;
    for (var i = 0u; i < 5u; i++) {
        var proto = vec3<f32>(0.0);
        var ramp = RAMP_FOREST;
        if (i == 0u) { proto = vec3<f32>(0.17, 0.33, 0.12); ramp = RAMP_FOREST; }
        if (i == 1u) { proto = vec3<f32>(0.32, 0.46, 0.14); ramp = RAMP_FIELD; }
        if (i == 2u) { proto = vec3<f32>(0.52, 0.47, 0.30); ramp = RAMP_SOIL; }
        if (i == 3u) { proto = vec3<f32>(0.50, 0.50, 0.50); ramp = RAMP_ROCK; }
        if (i == 4u) { proto = vec3<f32>(0.90, 0.92, 0.95); ramp = RAMP_SNOW; }
        let e = c - proto;
        let d = dot(e * e, vec3<f32>(0.30, 0.59, 0.11));
        if (d < best_d) { best_d = d; best = ramp; }
    }
    return best;
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
    let dpx = dpdx(in.world);
    let dpy = dpdy(in.world);
    let n = normalize(in.normal);
    let sun = normalize(g.sun.xyz);
    let ndl = dot(n, sun);
    let night = g.sun.w > 0.5;
    var colour = vec3<f32>(0.0);

    if (in.kind == 4u) {
        // Cloud sheet: thresholded to hard shapes, two flat tones, a darker underside rim.
        let ll = lonlat(in.world.xy);
        let a = cloud_alpha(ll);
        let ar = cloud_alpha(lonlat(in.world.xy + dpx.xy));
        let ad = cloud_alpha(lonlat(in.world.xy + dpy.xy));
        let al = cloud_alpha(lonlat(in.world.xy - dpx.xy));
        let au = cloud_alpha(lonlat(in.world.xy - dpy.xy));
        if (a < g.cloud.x || g.cloud.w < 0.5) { discard; }
        var step = 3;
        if (a < g.cloud.y) { step = 2; }
        let edge = ar < g.cloud.x || ad < g.cloud.x || al < g.cloud.x || au < g.cloud.x;
        if (edge) { step = 1; }
        return vec4<f32>(pal(RAMP_CLOUD, step), 1.0);
    }

    // Light step 0..3: lit top 3, one-step cloud shadow 2, caster shadow 1.
    var level = 0;
    let sh = lit(in.world, n);
    if (in.kind == 0u || in.kind == 1u) {
        let ll = lonlat(in.world.xy);
        let m = mask_at(ll);
        let mr = mask_at(lonlat(in.world.xy + dpx.xy));
        let md = mask_at(lonlat(in.world.xy + dpy.xy));
        let ml = mask_at(lonlat(in.world.xy - dpx.xy));
        let mu = mask_at(lonlat(in.world.xy - dpy.xy));
        let is_water = m.b > 0.5 && m.b < 2.5;
        let in_cloud = g.cloud.w > 0.5 && cloud_alpha(ll + g.cloud2.xy) >= g.cloud.x;

        if (in.kind == 1u) {
            // Terrace wall: lit side, or dark.
            level = 0;
            if (ndl > 0.15 && sh > 0.5) { level = 2; }
            if (is_water) { return vec4<f32>(pal(RAMP_WATER, 0), 1.0); }
        } else {
            level = 3;
            if (in_cloud) { level = 2; }
            if (sh < 0.5 || ndl <= 0.0) { level = 1; }
        }

        // Water: flat ramp, shallow rim along the shore, one step darker in shadow.
        if (is_water) {
            let shore = !(mr.b > 0.5 && mr.b < 2.5) || !(md.b > 0.5 && md.b < 2.5)
                || !(ml.b > 0.5 && ml.b < 2.5) || !(mu.b > 0.5 && mu.b < 2.5);
            var w = 2;
            if (level < 3) { w = 1; }
            if (shore) { w = 3; }
            return vec4<f32>(pal(RAMP_WATER, w), 1.0);
        }

        // Region lines, one pixel: states bright, regions soft, the player's regions gold.
        let land_n = m.b < 0.5 || m.b > 2.5;
        let state_edge = (mr.g != m.g && !(mr.b > 0.5 && mr.b < 2.5)) || (md.g != m.g && !(md.b > 0.5 && md.b < 2.5));
        let region_edge = (mr.r != m.r && !(mr.b > 0.5 && mr.b < 2.5)) || (md.r != m.r && !(md.b > 0.5 && md.b < 2.5));
        if (in.kind == 0u && land_n) {
            if (state_edge && (m.a > 0.5 || mr.a > 0.5 || md.a > 0.5)) {
                return vec4<f32>(pal(RAMP_CLOUD, 3), 1.0);
            }
            if (region_edge) {
                if (m.a > 1.5) { return vec4<f32>(pal(RAMP_GOLD, 3), 1.0); }
                if (m.a > 0.5) { return vec4<f32>(pal(RAMP_CLOUD, 1), 1.0); }
            }
        }

        // Ground: land cover from the real satellite image, snapped to a ramp.
        let tex = textureSampleLevel(earth_tex, nearest, earth_uv(ll), 0.0).rgb;
        var ramp = ground_ramp(tex, frag);
        var step = level;
        if (m.b > 2.5) { ramp = RAMP_URBAN; }
        if (m.a > 1.5 && m.b < 2.5) {
            // The player's holdings: gold ground, texture detail as a stipple between steps.
            ramp = RAMP_GOLD;
            let lum = dot(tex, vec3<f32>(0.3, 0.59, 0.11)) * 2.6;
            if (lum + (bayer8(frag) - 0.5) * 0.5 < 0.30) { step = max(step - 1, 0); }
        } else if (m.a < 0.5) {
            // Neighbouring land, two steps down where lit.
            if (step == 3) { step = 1; } else { step = max(step - 1, 0); }
        }
        colour = pal(ramp, step);
        return vec4<f32>(colour, 1.0);
    }

    // Buildings and plinths: nearest palette colour, shifted by the light step.
    var shade = 1;
    if (n.z > 0.5) { shade = 3; }
    else if (ndl > 0.1) { shade = 2; }
    if (sh < 0.5) { shade = min(shade, 1); }
    let base = nearest_colour(in.colour);
    var step2 = clamp(base.y + shade - 3, 0, 3);
    var out_c = pal(u32(base.x), step2);
    if (night && in.kind == 2u && abs(n.z) < 0.5) {
        // Lit windows: a one voxel grid on the side faces, about half of them lit.
        var u = in.local.x;
        if (abs(n.x) > 0.5) { u = in.local.y; }
        let cell = vec3<f32>(floor(u * 0.5), floor(in.local.z * 0.5), floor(in.local.x + in.local.y));
        let inside = (fract(u * 0.5) < 0.5) && (fract(in.local.z * 0.5) > 0.5);
        if (inside && hash3(cell) > 0.45) { out_c = pal(RAMP_SIGNAL, 1); }
    }
    return vec4<f32>(out_c, 1.0);
}
