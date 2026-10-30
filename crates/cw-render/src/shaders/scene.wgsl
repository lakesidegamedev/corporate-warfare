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
    cloud2: vec4<f32>,     // shadow offset (degrees lon, lat), shadow texel km, unused
    road: vec4<f32>,       // interstate, main road and railway widths in pixels, detail on (0 or 1)
    tex: vec4<f32>,        // texture texel in km, texture on (0 or 1), render width and height in pixels
    ground_b: vec4<f32>,   // west, south, east, north of the ground class raster
    ground_dim: vec4<f32>, // width, height, texels per degree, unused
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

fn cloud_alpha(ll: vec2<f32>) -> f32 {
    return textureSampleLevel(cloud_tex, linear, earth_uv(ll), 0.0).a;
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
const TILE_URBAN = array<u32, 8>(0x11111111u, 0x10000000u, 0x10000000u, 0x10000000u, 0x10000000u, 0x10000000u, 0x10000000u, 0x10000000u);
const TILE_CORE = array<u32, 8>(0x11111111u, 0x10002221u, 0x10002221u, 0x10002221u, 0x11111111u, 0x12220001u, 0x12220001u, 0x12220001u);
const TILE_WAVE = array<u32, 8>(0x00000000u, 0x02220000u, 0x00000000u, 0x00000000u, 0x00000000u, 0x00000222u, 0x00000000u, 0x00000000u);

fn nib(row: u32, tx: i32) -> u32 {
    return (row >> (28u - 4u * u32(tx & 7))) & 15u;
}

fn tile_value(t: array<u32, 8>, tx: i32, ty: i32) -> u32 {
    return nib(t[u32(ty & 7)], tx);
}

// Class texture at a world texel. Field rows run in a direction picked per patch.
fn class_texel(cls: u32, variant: u32, tx: i32, ty: i32) -> i32 {
    var v = 0u;
    if (cls == 0u) { v = tile_value(TILE_FOREST, tx, ty); }
    else if (cls == 1u) {
        // Survey grid fields: rows run east-west or north-south by patch, one dark row in six,
        // and half the patches stay plain. Never diagonal.
        if (variant == 0u) { if (((ty % 6) + 6) % 6 == 0) { v = 1u; } }
        else if (variant == 1u) { if (((tx % 6) + 6) % 6 == 0) { v = 1u; } }
    }
    else if (cls == 2u) { v = tile_value(TILE_SOIL, tx, ty); }
    else if (cls == 3u) { v = tile_value(TILE_ROCK, tx, ty); }
    else if (cls == 4u) { v = tile_value(TILE_SNOW, tx, ty); }
    else if (cls == 5u) { v = tile_value(TILE_URBAN, tx, ty); }
    else { v = tile_value(TILE_CORE, tx, ty); }
    if (v == 1u) { return -1; }
    if (v == 2u) { return 1; }
    return 0;
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
    let n = normalize(in.normal);
    let sun = normalize(g.sun.xyz);
    let ndl = dot(n, sun);
    let night = g.sun.w > 0.5;
    let px_km = max(length(dpx.xy), length(dpy.xy));

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
                // Wave dashes in world space (static for now, ready to move later), only in
                // open water away from any shore and only while the camera is close enough
                // for a texel to be a few pixels wide.
                let tx = i32(floor(in.world.x / g.tex.x));
                let ty = i32(floor(in.world.y / g.tex.x));
                let shift = i32(hash3(vec3<f32>(f32(ty >> 3), 3.0, 1.0)) * 8.0);
                let far = !is_water(mask_at(lonlat(in.world.xy + dpx.xy * 8.0)))
                    || !is_water(mask_at(lonlat(in.world.xy - dpx.xy * 8.0)))
                    || !is_water(mask_at(lonlat(in.world.xy + dpy.xy * 8.0)))
                    || !is_water(mask_at(lonlat(in.world.xy - dpy.xy * 8.0)));
                if (tile_value(TILE_WAVE, tx + shift, ty) == 2u && !far) { w = min(w + 1, 3); }
            }
            return vec4<f32>(pal(RAMP_WATER, w), 1.0);
        }

        let player = m.a > 1.5;
        let cls = ground_class(tcf, fw, frag, select(g.mask_dim.w, 0.0, in.kind == 1u));
        var ramp = ramp_of(cls);
        var base = 2;
        if (player) { ramp = RAMP_GOLD; base = gold_base(cls); }

        let tc = vec2<i32>(floor(tcf));
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
                let own = o.a < 0.5 || (i < 2 && (o.a < 1.5 || player)) || (player && o.a < 1.5);
                if (diff_r && own) {
                    edge = true;
                    if (o.g != m.g) { state_edge = true; }
                }
            }
            if (edge) {
                if (player) { return vec4<f32>(pal(RAMP_SIGNAL, 1), 1.0); }
                if (state_edge) { return vec4<f32>(pal(RAMP_CLOUD, 3), 1.0); }
                return vec4<f32>(pal(RAMP_CLOUD, 2), 1.0);
            }
        }

        // Flat ground colour; lit 2, shadow or cloud shadow one lower, both two lower.
        var step = base;
        if (sh < 0.5 || ndl <= 0.0) { step = step - 1; }
        step = step - shade;
        // Pixel texture of the class, in world space, at a texel that is always a few screen
        // pixels wide; gone (flat class colour) once the camera is too far out.
        if (g.tex.y > 0.5) {
            let tx = i32(floor(in.world.x / g.tex.x));
            let ty = i32(floor(in.world.y / g.tex.x));
            let tv = class_texel(cls, variant_at(tc), tx, ty);
            if (player) {
                // Gold holdings: every other textured texel shows the land through.
                if (tv != 0 && cls < 5u && ((tx + ty) & 1) == 0) { return vec4<f32>(pal(ramp_of(cls), 2 - shade), 1.0); }
                step = step + tv;
            } else {
                step = step + tv;
            }
        }
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
    if (night && in.kind == 2u && abs(n.z) < 0.5) {
        // Lit windows: a one voxel grid on the side faces, about half of them lit.
        var u = in.local.x;
        if (abs(n.x) > 0.5) { u = in.local.y; }
        let cell = vec3<f32>(floor(u * 0.5), floor(in.local.z * 0.5), floor(in.local.x + in.local.y));
        let inside = (fract(u * 0.5) < 0.5) && (fract(in.local.z * 0.5) > 0.5);
        if (inside && hash3(cell) > 0.45) { out_c = pal(RAMP_SIGNAL, 1); }
    }
    let ex = min(in.uv.x, 1.0 - in.uv.x) / max(fuv.x, 1e-5);
    let ey = min(in.uv.y, 1.0 - in.uv.y) / max(fuv.y, 1e-5);
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

fn road_width(cls: u32) -> f32 {
    if (cls == 0u) { return g.road.x; }
    if (cls == 1u) { return g.road.y; }
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
    if (this_c.w < 1e-3 || other_c.w < 1e-3 || road_width(cls) < 0.5) {
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
    let end_sign = select(-1.0, 1.0, v.local.y > 0.5);
    let p = p_this + nrm * v.local.x * ext + d * end_sign * ext;
    let ndc = vec2<f32>(p.x / g.tex.z * 2.0 - 1.0, 1.0 - p.y / g.tex.w * 2.0);
    o.clip = vec4<f32>(ndc * this_c.w, this_c.z, this_c.w);
    o.across = v.local.x * ext;
    return o;
}

struct RFOut {
    @location(0) colour: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_road(in: ROut) -> RFOut {
    let ll = lonlat(in.world.xy);
    let m = mask_at(ll);
    let w = road_width(in.kind);
    let hw = w * 0.5;
    let d = abs(in.across);
    if (is_water(m) || d >= hw) { discard; }
    var c = pal(RAMP_ROCK, 2);
    let detail = g.road.w > 0.5;
    if (in.kind == 0u) {
        // Interstate: mid grey asphalt, a slightly darker one pixel edge and, up close, a gold
        // dashed centre line.
        if (w > 2.5 && d >= hw - 1.0) { c = pal(RAMP_ROCK, 1); }
        else if (detail && w > 4.5 && d < 0.75 && fract(in.along / 7.0) < 0.5) { c = pal(RAMP_GOLD, 3); }
    } else if (in.kind == 1u) {
        // Main road: one step lighter than the ground it crosses, so it reads as a faint line.
        let cls = class_at(vec2<i32>(floor(ground_tc(ll))));
        var ramp = ramp_of(cls);
        var base = 2;
        if (m.a > 1.5) { ramp = RAMP_GOLD; base = gold_base(cls); }
        c = pal(ramp, base + 1);
    } else {
        // Railway: a thin light grey-brown line, with sleepers across it up close.
        c = pal(RAMP_SOIL, 3);
        if (detail && d >= 0.75) {
            if (fract(in.along / 3.5) < 0.43) { c = pal(RAMP_SOIL, 2); } else { discard; }
        }
    }
    var out: RFOut;
    out.colour = vec4<f32>(c, 1.0);
    out.depth = in.clip.z - 4.0e-5 - 1.0e-5 * f32(in.kind) - 5.0e-6 * d / max(hw, 0.5);
    return out;
}
