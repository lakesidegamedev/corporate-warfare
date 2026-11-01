//! The animated layer of the map: smoke from stacks, construction stages and cranes, the growth
//! of a new building, blinking beacons at night, and trucks, trains and ships on the roads, rails
//! and sea lanes.
//!
//! Everything here is plain geometry built on the CPU from the scene and one clock value, then
//! drawn with the same pipeline, palette and camera as the buildings. The same time always gives
//! the same mesh. Nothing moves with the camera: every object stands at a world position and has
//! a world size, apart from a minimum size in screen pixels so a puff or a truck never shrinks to
//! nothing at a distance.

use crate::data::{Owner, Placement, Scene, VoxelModel, WATER_LAKE, WATER_OCEAN};
use crate::game::SiteStatus;
use crate::mesh::{self, KIND_PLAIN, Mesh, box_faces};
use crate::palette::{Palette, Rgb, ramp};

mod night;

/// Wind on the map, a unit vector in world kilometres (x east, y north): smoke drifts along it.
/// The sun comes from the north west, so the smoke leans away from it.
pub const WIND: [f32; 2] = [0.94, 0.34];

/// The clock is wrapped at this many seconds so f32 keeps its precision over a long session.
/// Every period below divides it, so the wrap is never seen.
pub const CLOCK_WRAP_S: f64 = 3600.0;

/// A new building grows over this long, in this many whole steps.
pub const GROW_S: f32 = 1.0;
const GROW_STEPS: f32 = 5.0;

/// Furthest zoom (camera width in km) at which smoke, vehicles and beacons are drawn. Beyond it
/// they would be a pixel or less.
pub const ANIM_MAX_KM: f32 = 420.0;

/// The effective clock for a frame: zero in still mode, otherwise the host's seconds wrapped.
pub fn clock(time_s: f64, animate: bool) -> f32 {
    if animate {
        time_s.rem_euclid(CLOCK_WRAP_S) as f32
    } else {
        0.0
    }
}

/// How fast the clouds drift along [`WIND`], in kilometres a second.
pub const CLOUD_SPEED_KM_S: f64 = 0.03;

/// Where the cloud image is read from, as an offset in image units (u east, v south), at host
/// time `time_s`. The shader samples the image at `uv - drift` and mirrors it at its edges, so
/// the picture is continuous however far the clouds have gone, and the offset is kept below one
/// mirror period so f32 stays exact. Zero when animations are off.
pub fn cloud_drift_uv(
    time_s: f64,
    animate: bool,
    image_deg: [f32; 2],
    km_per_deg: [f32; 2],
) -> [f32; 2] {
    if !animate {
        return [0.0, 0.0];
    }
    let km = time_s * CLOUD_SPEED_KM_S;
    let u = km * WIND[0] as f64 / km_per_deg[0] as f64 / image_deg[0] as f64;
    let v = -km * WIND[1] as f64 / km_per_deg[1] as f64 / image_deg[1] as f64;
    [u.rem_euclid(2.0) as f32, v.rem_euclid(2.0) as f32]
}

/// A period near `seconds` that divides the wrap exactly.
fn period(seconds: f32) -> f32 {
    let n = (CLOCK_WRAP_S as f32 / seconds).round().max(1.0);
    CLOCK_WRAP_S as f32 / n
}

fn hash(a: u32, b: u32) -> f32 {
    let mut h = a.wrapping_mul(0x9E37_79B1) ^ b.wrapping_mul(0x85EB_CA6B) ^ 0xC2B2_AE35;
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    (h & 0xFFFF) as f32 / 65536.0
}

fn colour(ramp: usize, step: usize) -> Rgb {
    Palette::day().ramps[ramp][step]
}

fn put(m: &mut Mesh, min: [f32; 3], max: [f32; 3], rgb: Rgb) {
    box_faces(m, &|q| q, &|n| n, min, max, rgb, KIND_PLAIN);
}

/// A box in a frame turned by `yaw` about the vertical through `at` (local axes: x ahead).
fn put_turned(m: &mut Mesh, at: [f32; 3], yaw: f32, min: [f32; 3], max: [f32; 3], rgb: Rgb) {
    let (s, c) = yaw.sin_cos();
    let tf = move |q: [f32; 3]| {
        [
            at[0] + q[0] * c - q[1] * s,
            at[1] + q[0] * s + q[1] * c,
            at[2] + q[2],
        ]
    };
    let rot = move |n: [f32; 3]| [n[0] * c - n[1] * s, n[0] * s + n[1] * c, n[2]];
    box_faces(m, &tf, &rot, min, max, rgb, KIND_PLAIN);
}

/// Industries whose plants smoke.
fn smokes(industry: &str) -> bool {
    matches!(
        industry,
        "energy" | "chemical_industries" | "manufacturing" | "extraction"
    )
}

fn industry_of(model: &str) -> &str {
    model.split('/').next().unwrap_or("")
}

/// A point on a model that gives off smoke or carries a beacon, in model voxels.
#[derive(Clone, Copy, Debug)]
struct Spot {
    at: [f32; 3],
}

/// The chimneys of a model, found from its boxes: slim, tall boxes that stand in the upper half.
/// A smoking sector without any gets two roof vents. The tallest first, at most four.
fn stacks_of(model: &VoxelModel, industry: &str) -> Vec<Spot> {
    let top = model.boxes.iter().map(|b| b.max[2]).fold(0.0f32, f32::max);
    let mut found: Vec<Spot> = model
        .boxes
        .iter()
        .filter(|b| {
            let (w, d, h) = (
                b.max[0] - b.min[0],
                b.max[1] - b.min[1],
                b.max[2] - b.min[2],
            );
            w <= 3.5 && d <= 3.5 && h >= 4.0 && b.max[2] >= top * 0.5
        })
        .map(|b| Spot {
            at: [
                (b.min[0] + b.max[0]) / 2.0,
                (b.min[1] + b.max[1]) / 2.0,
                b.max[2],
            ],
        })
        .collect();
    found.sort_by(|a, b| b.at[2].total_cmp(&a.at[2]));
    let mut out: Vec<Spot> = Vec::new();
    for s in found {
        if out
            .iter()
            .all(|o| (o.at[0] - s.at[0]).abs() + (o.at[1] - s.at[1]).abs() >= 1.5)
        {
            out.push(s);
        }
        if out.len() == 4 {
            break;
        }
    }
    if out.is_empty()
        && smokes(industry)
        && let Some(roof) = model
            .boxes
            .iter()
            .filter(|b| b.min[2] >= 0.4)
            .max_by(|a, b| a.max[2].total_cmp(&b.max[2]))
    {
        let (cx, cy) = (
            (roof.min[0] + roof.max[0]) / 2.0,
            (roof.min[1] + roof.max[1]) / 2.0,
        );
        let off = (roof.max[0] - roof.min[0]) * 0.25;
        for sign in [-1.0f32, 1.0] {
            out.push(Spot {
                at: [cx + sign * off, cy, roof.max[2]],
            });
        }
    }
    out
}

/// The tip of a tower that should carry a beacon: the highest box of a tall model.
fn tower_of(model: &VoxelModel) -> Option<Spot> {
    let b = model
        .boxes
        .iter()
        .max_by(|a, b| a.max[2].total_cmp(&b.max[2]))?;
    (b.max[2] >= 17.0).then(|| Spot {
        at: [
            (b.min[0] + b.max[0]) / 2.0,
            (b.min[1] + b.max[1]) / 2.0,
            b.max[2],
        ],
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Truck,
    Train,
    Ship,
}

/// A path vehicles run to and fro along, in world kilometres.
struct Route {
    kind: Kind,
    pts: Vec<[f32; 3]>,
    /// Distance from the start to each point.
    cum: Vec<f32>,
    speed_km_s: f32,
    /// How many vehicles share it and the phase offset of the first.
    count: u32,
    seed: u32,
}

impl Route {
    fn new(kind: Kind, pts: Vec<[f32; 3]>, count: u32, seed: u32) -> Option<Route> {
        if pts.len() < 2 {
            return None;
        }
        let mut cum = vec![0.0f32];
        for w in pts.windows(2) {
            let d = ((w[1][0] - w[0][0]).powi(2) + (w[1][1] - w[0][1]).powi(2)).sqrt();
            cum.push(cum.last().copied().unwrap_or(0.0) + d);
        }
        let len = *cum.last()?;
        (len >= 1.0).then_some(Route {
            kind,
            pts,
            cum,
            speed_km_s: match kind {
                Kind::Truck => 0.5,
                Kind::Train => 0.8,
                Kind::Ship => 0.3,
            },
            count,
            seed,
        })
    }

    fn length(&self) -> f32 {
        *self.cum.last().unwrap_or(&0.0)
    }

    /// Position and heading (radians) at `s` kilometres from the start.
    fn at(&self, s: f32) -> ([f32; 3], f32) {
        let s = s.clamp(0.0, self.length());
        let i = self
            .cum
            .partition_point(|&c| c <= s)
            .clamp(1, self.pts.len() - 1);
        let (a, b) = (self.pts[i - 1], self.pts[i]);
        let span = (self.cum[i] - self.cum[i - 1]).max(1e-6);
        let t = (s - self.cum[i - 1]) / span;
        (
            [
                a[0] + (b[0] - a[0]) * t,
                a[1] + (b[1] - a[1]) * t,
                a[2] + (b[2] - a[2]) * t,
            ],
            (b[1] - a[1]).atan2(b[0] - a[0]),
        )
    }

    /// Distance along the route of vehicle `k` at clock `t`, going out and coming back, and the
    /// direction it faces (+1 out, -1 back).
    fn state(&self, k: u32, t: f32) -> (f32, f32) {
        let len = self.length();
        let lap = period(2.0 * len / self.speed_km_s);
        let phase = (t / lap + k as f32 / self.count.max(1) as f32 + hash(self.seed, 7)).fract();
        if phase < 0.5 {
            (phase * 2.0 * len, 1.0)
        } else {
            ((1.0 - phase) * 2.0 * len, -1.0)
        }
    }
}

/// What the animated layer needs to know about a scene's buildings, rebuilt when they change.
pub struct Plan {
    /// Smoke and beacon spots per building index.
    stacks: Vec<Vec<Spot>>,
    towers: Vec<Option<Spot>>,
    routes: Vec<Route>,
    routes_built: bool,
}

impl Plan {
    pub fn build(scene: &Scene) -> Plan {
        let mut stacks = Vec::new();
        let mut towers = Vec::new();
        for p in &scene.buildings {
            let model = scene.models.get(&p.model);
            let industry = industry_of(&p.model);
            let lit = p.activity.status == Some(SiteStatus::Running);
            stacks.push(match model {
                Some(m) if lit && smokes(industry) => stacks_of(m, industry),
                _ => Vec::new(),
            });
            towers.push(model.and_then(tower_of));
        }
        Plan {
            stacks,
            towers,
            routes: Vec::new(),
            routes_built: false,
        }
    }

    /// Lay the vehicle routes. Apart from the buildings this reads the road network, so it is
    /// done the first time an animated frame needs it.
    pub fn build_routes(&mut self, scene: &Scene) {
        if self.routes_built {
            return;
        }
        self.routes_built = true;
        self.routes = routes_for(scene);
    }

    /// True when anything of the plan moves at all.
    pub fn has_motion(&self, scene: &Scene) -> bool {
        self.stacks.iter().any(|s| !s.is_empty())
            || scene
                .buildings
                .iter()
                .any(|p| p.activity.status == Some(SiteStatus::Site) || p.activity.born.is_some())
            || !self.routes.is_empty()
            || !self.routes_built
    }
}

/// True when the scene has anything that moves on its own at clock `t`: a running or building
/// plant (smoke, vehicles, a crane) or a building still growing. The shell uses it to decide
/// whether frames must keep coming.
pub fn scene_animates(scene: &Scene, t: f32) -> bool {
    scene.buildings.iter().any(|p| {
        matches!(
            p.activity.status,
            Some(SiteStatus::Running) | Some(SiteStatus::Site)
        ) || growth(p, t).is_some_and(|g| g < 1.0)
    })
}

/// Whether the static building mesh leaves building `index` out because the animated layer draws
/// it: construction sites always, and a building while it is growing.
pub fn drawn_by_anim(p: &Placement, t: f32) -> bool {
    p.activity.status == Some(SiteStatus::Site) || growth(p, t).is_some_and(|g| g < 1.0)
}

/// How grown a new building is at clock `t` (whole steps, 0 to 1); None when it is not new.
pub fn growth(p: &Placement, t: f32) -> Option<f32> {
    let born = p.activity.born?;
    let age = (t - born).rem_euclid(CLOCK_WRAP_S as f32);
    if age >= GROW_S {
        return Some(1.0);
    }
    Some(((age / GROW_S * GROW_STEPS).floor() + 1.0).min(GROW_STEPS) / GROW_STEPS)
}

fn km_of(scene: &Scene, p: (f32, f32)) -> (f32, f32) {
    scene.projection.to_km(p.0, p.1)
}

fn routes_for(scene: &Scene) -> Vec<Route> {
    let chains = mesh::road_chains_for_band(scene, &mesh::ROAD_BANDS[2]);
    let cities: Vec<(f32, f32)> = scene
        .cities
        .iter()
        .map(|c| km_of(scene, (c.lon, c.lat)))
        .collect();
    let mut out = Vec::new();
    for (bi, p) in scene.buildings.iter().enumerate() {
        if p.owner == Owner::Scenery || p.activity.status != Some(SiteStatus::Running) {
            continue;
        }
        let (cx, cy) = km_of(scene, (p.lon, p.lat));
        let util = p.activity.utilisation;
        let n = 1 + (util * 2.0).round() as u32;
        let seed = bi as u32 * 31 + 5;
        // Trucks: the nearest road, followed toward the nearest city.
        let city = cities
            .iter()
            .copied()
            .filter(|c| dist(*c, (cx, cy)) > 4.0)
            .min_by(|a, b| dist(*a, (cx, cy)).total_cmp(&dist(*b, (cx, cy))));
        if let Some(path) = chain_path(&chains, (cx, cy), 0..=1, 9.0, 28.0, city)
            && let Some(r) = Route::new(Kind::Truck, path, n, seed)
        {
            out.push(r);
        }
        // Trains: the nearest railway, both ways from the plant, for the bigger plants.
        if p.activity.utilisation > 0.2
            && let Some(path) = chain_path(&chains, (cx, cy), 2..=2, 22.0, 34.0, None)
            && let Some(r) = Route::new(Kind::Train, path, 1, seed + 1)
        {
            out.push(r);
        }
        // Ships: a plant on a coast sends one along a straight lane over open water.
        if matches!(
            industry_of(&p.model),
            "energy" | "extraction" | "logistics" | "chemical_industries" | "agriculture"
        ) && let Some(path) = sea_lane(scene, (cx, cy))
            && let Some(r) = Route::new(Kind::Ship, path, 2, seed + 2)
        {
            out.push(r);
        }
    }
    out
}

/// How many truck, train and ship routes the scene's buildings get (for diagnostics and tests).
pub fn route_counts(scene: &Scene) -> [usize; 3] {
    let mut n = [0; 3];
    for r in routes_for(scene) {
        n[r.kind as usize] += 1;
    }
    n
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// The stretch of the nearest chain of the given classes within `reach` km of `from`, walked
/// away from the plant for up to `max_len` km (toward `city` when given, else both ways).
fn chain_path(
    chains: &[(u8, Vec<mesh::RoadPoint>)],
    from: (f32, f32),
    classes: std::ops::RangeInclusive<u8>,
    reach: f32,
    max_len: f32,
    city: Option<(f32, f32)>,
) -> Option<Vec<[f32; 3]>> {
    let mut best: Option<(usize, usize, f32)> = None;
    for (ci, (class, pts)) in chains.iter().enumerate() {
        if !classes.contains(class) {
            continue;
        }
        for (pi, q) in pts.iter().enumerate() {
            let d = dist((q.xyz[0], q.xyz[1]), from);
            if d <= reach && best.is_none_or(|b| d < b.2) {
                best = Some((ci, pi, d));
            }
        }
    }
    let (ci, pi, _) = best?;
    let pts = &chains[ci].1;
    let walk = |dir: isize, limit: f32| -> Vec<[f32; 3]> {
        let mut out = vec![pts[pi].xyz];
        let (mut i, mut len) = (pi as isize, 0.0f32);
        loop {
            let j = i + dir;
            if j < 0 || j >= pts.len() as isize || len >= limit {
                break;
            }
            let (a, b) = (pts[i as usize].xyz, pts[j as usize].xyz);
            len += dist((a[0], a[1]), (b[0], b[1]));
            out.push(b);
            i = j;
            if let Some(c) = city
                && dist((b[0], b[1]), c) < 2.5
            {
                break;
            }
        }
        out
    };
    let path = match city {
        Some(c) => {
            let (fwd, back) = (walk(1, max_len), walk(-1, max_len));
            let gap = |p: &Vec<[f32; 3]>| {
                let e = p[p.len() - 1];
                dist((e[0], e[1]), c)
            };
            if gap(&fwd) <= gap(&back) { fwd } else { back }
        }
        None => {
            let mut back = walk(-1, max_len / 2.0);
            back.reverse();
            let fwd = walk(1, max_len / 2.0);
            back.extend(fwd.into_iter().skip(1));
            back
        }
    };
    (path.len() >= 2).then_some(path)
}

/// A straight lane over water starting near the shore of a coastal plant: the longest of sixteen
/// directions that reach water within 8 km of it.
fn sea_lane(scene: &Scene, from: (f32, f32)) -> Option<Vec<[f32; 3]>> {
    let water = |x: f32, y: f32| {
        let (lon, lat) = scene.projection.to_lonlat(x, y);
        let w = scene.mask.at(lon, lat)[2];
        w == WATER_OCEAN || w == WATER_LAKE
    };
    let mut best: Option<(f32, [f32; 2], f32)> = None;
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let dir = [a.cos(), a.sin()];
        let at = |d: f32| (from.0 + dir[0] * d, from.1 + dir[1] * d);
        let Some(start) = (1..=8).map(|i| i as f32).find(|&d| {
            let (x, y) = at(d);
            water(x, y)
        }) else {
            continue;
        };
        let mut end = start;
        while end < start + 30.0 && {
            let (x, y) = at(end + 1.0);
            water(x, y)
        } {
            end += 1.0;
        }
        if end - start >= 12.0 && best.is_none_or(|b| end - start > b.2 - b.0) {
            best = Some((start + 2.0, dir, end - 2.0));
        }
    }
    let (a, dir, b) = best?;
    let pt = |d: f32| {
        let (x, y) = (from.0 + dir[0] * d, from.1 + dir[1] * d);
        [x, y, mesh::ground_z(scene, x, y) + 0.012]
    };
    Some(vec![pt(a), pt(b)])
}

/// Inputs of one animated frame.
pub struct FrameView {
    /// The camera target in kilometres and the width it sees.
    pub target: (f32, f32),
    pub width_km: f32,
    /// Kilometres one render pixel covers at the target.
    pub px_km: f32,
    pub night: bool,
    pub t: f32,
    /// Animations on: smoke, beacons and vehicles are drawn only then. Construction stages and
    /// grown buildings are state, so they always are.
    pub animated: bool,
}

/// The animated geometry of one frame.
pub fn frame_mesh(scene: &Scene, plan: &Plan, v: &FrameView) -> Mesh {
    let mut m = Mesh::default();
    // How far from the target things are drawn: the whole view and a margin.
    let reach = v.width_km * 1.3;
    let near = |x: f32, y: f32| (x - v.target.0).abs() < reach && (y - v.target.1).abs() < reach;
    for (bi, p) in scene.buildings.iter().enumerate() {
        let Some(frame) = mesh::placement_frame(scene, p) else {
            continue;
        };
        let [cx, cy, z_top, _] = frame;
        if !near(cx, cy) {
            continue;
        }
        let Some(model) = scene.models.get(&p.model) else {
            continue;
        };
        let vx = scene.voxel_km * p.scale;
        match p.activity.status {
            Some(SiteStatus::Site) => construction(&mut m, p, model, frame, vx, v, bi),
            _ => {
                if let Some(g) = growth(p, v.t).filter(|g| *g < 1.0) {
                    grow(&mut m, scene, bi, [cx, cy, z_top], g);
                }
            }
        }
        if v.width_km > ANIM_MAX_KM || !v.animated {
            continue;
        }
        let place = |s: &Spot| {
            let (sn, cs) = p.yaw.to_radians().sin_cos();
            [
                cx + (s.at[0] * cs - s.at[1] * sn) * vx,
                cy + (s.at[0] * sn + s.at[1] * cs) * vx,
                z_top + s.at[2] * vx,
            ]
        };
        let grown = growth(p, v.t).unwrap_or(1.0);
        if grown < 1.0 {
            continue;
        }
        // Smoke.
        let puff_unit = (1.6 * vx).max(3.6 * v.px_km);
        for (si, s) in plan.stacks.get(bi).into_iter().flatten().enumerate() {
            let top = place(s);
            smoke(
                &mut m,
                top,
                puff_unit,
                p.activity.utilisation,
                v.t,
                (bi * 7 + si) as u32,
            );
        }
        // Beacons.
        if v.night {
            let size = (0.9 * vx).max(4.5 * v.px_km);
            let mut spots: Vec<Spot> = plan.towers.get(bi).copied().flatten().into_iter().collect();
            if p.activity.status == Some(SiteStatus::Running)
                && let Some(s) = plan.stacks.get(bi).and_then(|s| s.first())
                && s.at[2] >= 10.0
            {
                spots.push(*s);
            }
            for (k, s) in spots.iter().enumerate() {
                let ph = hash(bi as u32, k as u32 + 11);
                let on = (v.t / period(1.6) + ph).fract() < 0.45;
                if on {
                    night::beacon(&mut m, place(s), size);
                }
            }
        }
    }
    if v.width_km <= ANIM_MAX_KM && v.animated {
        for r in &plan.routes {
            for k in 0..r.count {
                let (s, dir) = r.state(k, v.t);
                vehicle(&mut m, r, s, dir, v, &near);
            }
        }
    }
    m
}

/// Puffs of smoke rising from `top` and drifting with the wind. Each puff is a cube in palette
/// greys; it fades by stepping down to smaller cubes, never by blending.
fn smoke(m: &mut Mesh, top: [f32; 3], unit: f32, utilisation: f32, t: f32, seed: u32) {
    const SIZES: [f32; 8] = [0.7, 1.1, 1.5, 1.7, 1.5, 1.2, 0.9, 0.65];
    let count = 2 + (utilisation.clamp(0.0, 1.0) * 4.0).round() as u32;
    let life = period(6.0);
    for i in 0..count {
        let age = (t / life + i as f32 / count as f32 + hash(seed, 3)).fract();
        let step = ((age * SIZES.len() as f32) as usize).min(SIZES.len() - 1);
        let size = unit * SIZES[step];
        let rise = age * unit * 7.0;
        let drift = age * unit * 5.0;
        let c = [
            top[0] + WIND[0] * drift,
            top[1] + WIND[1] * drift,
            top[2] + unit * 0.4 + rise,
        ];
        let h = size / 2.0;
        let rgb = if age < 0.45 {
            colour(ramp::ROCK, 1)
        } else {
            colour(ramp::ROCK, 2)
        };
        put(
            m,
            [c[0] - h, c[1] - h, c[2] - h],
            [c[0] + h, c[1] + h, c[2] + h],
            rgb,
        );
    }
}

/// A site at its stage: foundation, frame, then cladding, with a crane whose jib turns.
fn construction(
    m: &mut Mesh,
    p: &Placement,
    model: &VoxelModel,
    frame: [f32; 4],
    vx: f32,
    v: &FrameView,
    bi: usize,
) {
    let (px_km, t) = (v.px_km, v.t);
    let [cx, cy, z_top, z_low] = frame;
    let (hx, hy) = (model.footprint[0] / 2.0 * vx, model.footprint[1] / 2.0 * vx);
    let progress = p.activity.progress.clamp(0.0, 1.0);
    let concrete = colour(ramp::ROCK, 2);
    let steel = colour(ramp::SIGNAL, 1);
    let wall = colour(ramp::URBAN, 2);
    let earth: Rgb = [0x8C, 0x75, 0x56];
    if z_top > z_low {
        put(
            m,
            [cx - hx, cy - hy, z_low],
            [cx + hx, cy + hy, z_top],
            earth,
        );
    }
    // The pad: a flat slab, always.
    let slab = 0.5 * vx;
    put(
        m,
        [cx - hx, cy - hy, z_top],
        [cx + hx, cy + hy, z_top + slab],
        concrete,
    );
    let height = 8.0 * vx;
    let frame_share = ((progress - 0.34) / 0.33).clamp(0.0, 1.0);
    let wall_share = ((progress - 0.67) / 0.33).clamp(0.0, 1.0);
    // Stage one: footings and a stack of material.
    if progress < 0.34 {
        let foot = 0.7 * vx;
        for (sx, sy) in [(-0.6f32, -0.6f32), (0.6, -0.6), (0.6, 0.6), (-0.6, 0.6)] {
            let (x, y) = (cx + sx * (hx - foot), cy + sy * (hy - foot));
            put(
                m,
                [x - foot / 2.0, y - foot / 2.0, z_top + slab],
                [
                    x + foot / 2.0,
                    y + foot / 2.0,
                    z_top + slab + foot * (0.5 + progress * 3.0),
                ],
                concrete,
            );
        }
    } else {
        // Stage two: columns and floor plates, rising floor by floor (three floors).
        let floors = (frame_share * 3.0).ceil().max(1.0) as u32;
        let col = 0.55 * vx;
        let storey = height / 3.0;
        for (sx, sy) in [
            (-1.0f32, -1.0f32),
            (1.0, -1.0),
            (1.0, 1.0),
            (-1.0, 1.0),
            (0.0, -1.0),
            (0.0, 1.0),
        ] {
            let (x, y) = (cx + sx * (hx - col), cy + sy * (hy - col));
            put(
                m,
                [x - col / 2.0, y - col / 2.0, z_top + slab],
                [
                    x + col / 2.0,
                    y + col / 2.0,
                    z_top + slab + storey * floors as f32,
                ],
                steel,
            );
        }
        for f in 1..=floors {
            let z = z_top + slab + storey * f as f32;
            put(
                m,
                [cx - hx, cy - hy, z - 0.35 * vx],
                [cx + hx, cy + hy, z],
                concrete,
            );
        }
        // Stage three: cladding rises up the frame.
        if wall_share > 0.0 {
            let wh = height * wall_share;
            let th = 0.5 * vx;
            let z0 = z_top + slab;
            put(
                m,
                [cx - hx, cy - hy, z0],
                [cx + hx, cy - hy + th, z0 + wh],
                wall,
            );
            put(
                m,
                [cx - hx, cy + hy - th, z0],
                [cx + hx, cy + hy, z0 + wh],
                wall,
            );
            put(
                m,
                [cx - hx, cy - hy + th, z0],
                [cx - hx + th, cy + hy - th, z0 + wh],
                wall,
            );
            put(
                m,
                [cx + hx - th, cy - hy + th, z0],
                [cx + hx, cy + hy - th, z0 + wh],
                wall,
            );
        }
    }
    // The crane: a mast at one corner, a jib that turns slowly in whole steps, a counterweight.
    let mast = [cx - hx * 0.8, cy - hy * 0.8];
    let mh = height + 5.0 * vx;
    let mw = (0.5 * vx).max(2.0 * px_km);
    put(
        m,
        [mast[0] - mw / 2.0, mast[1] - mw / 2.0, z_top + slab],
        [mast[0] + mw / 2.0, mast[1] + mw / 2.0, z_top + slab + mh],
        steel,
    );
    let turn = period(48.0);
    let step = ((t / turn + hash(bi as u32, 5)).fract() * 32.0).floor();
    let yaw = step / 32.0 * std::f32::consts::TAU;
    let top = [mast[0], mast[1], z_top + slab + mh];
    put_turned(
        m,
        top,
        yaw,
        [-hx * 0.5, -mw / 2.0, 0.0],
        [hx * 1.9, mw / 2.0, mw],
        steel,
    );
    put_turned(
        m,
        top,
        yaw,
        [-hx * 0.5, -mw, 0.0],
        [-hx * 0.2, mw, mw * 1.6],
        concrete,
    );
    // The hook block hangs from the jib at a fixed spot.
    put_turned(
        m,
        top,
        yaw,
        [hx * 1.4, -mw / 4.0, -3.0 * vx],
        [hx * 1.4 + mw / 2.0, mw / 4.0, 0.0],
        concrete,
    );
}

/// The building scaled up from its base to `g` of its size.
fn grow(m: &mut Mesh, scene: &Scene, index: usize, base: [f32; 3], g: f32) {
    let one = mesh::buildings_where(scene, |i, _| i == index);
    let first = m.vertices.len() as u32;
    for mut v in one.vertices {
        for (pos, b) in v.pos.iter_mut().zip(base) {
            *pos = b + (*pos - b) * g;
        }
        m.vertices.push(v);
    }
    m.indices.extend(one.indices.iter().map(|i| i + first));
}

/// One vehicle at `s` kilometres along its route, facing `dir`.
fn vehicle(
    m: &mut Mesh,
    r: &Route,
    s: f32,
    dir: f32,
    v: &FrameView,
    near: &dyn Fn(f32, f32) -> bool,
) {
    let (at, heading) = r.at(s);
    if !near(at[0], at[1]) {
        return;
    }
    let yaw = if dir > 0.0 {
        heading
    } else {
        heading + std::f32::consts::PI
    };
    let u = match r.kind {
        Kind::Truck => 0.035f32,
        Kind::Train => 0.05,
        Kind::Ship => 0.09,
    }
    .max(
        match r.kind {
            Kind::Truck => 2.2,
            Kind::Train => 2.4,
            Kind::Ship => 3.2,
        } * v.px_km,
    );
    let white = colour(ramp::CLOUD, 3);
    let dark = colour(ramp::ROCK, 1);
    let red = colour(ramp::BRICK, 2);
    match r.kind {
        Kind::Truck => {
            put_turned(
                m,
                at,
                yaw,
                [-1.6 * u, -0.75 * u, 0.15 * u],
                [0.5 * u, 0.75 * u, 1.5 * u],
                white,
            );
            put_turned(
                m,
                at,
                yaw,
                [0.6 * u, -0.75 * u, 0.15 * u],
                [1.4 * u, 0.75 * u, 1.2 * u],
                red,
            );
        }
        Kind::Train => {
            // A locomotive and three wagons, each following the rail at its own distance.
            for car in 0..4 {
                let back = car as f32 * 2.3 * u * -dir;
                let (p, h) = r.at((s + back).clamp(0.0, r.length()));
                let y = if dir > 0.0 {
                    h
                } else {
                    h + std::f32::consts::PI
                };
                let rgb = if car == 0 {
                    red
                } else if car % 2 == 1 {
                    colour(ramp::ROCK, 2)
                } else {
                    dark
                };
                put_turned(
                    m,
                    p,
                    y,
                    [-u, -0.7 * u, 0.1 * u],
                    [1.0 * u, 0.7 * u, 1.3 * u],
                    rgb,
                );
            }
        }
        Kind::Ship => {
            put_turned(
                m,
                at,
                yaw,
                [-1.8 * u, -0.8 * u, 0.0],
                [1.8 * u, 0.8 * u, 0.7 * u],
                dark,
            );
            put_turned(
                m,
                at,
                yaw,
                [-1.4 * u, -0.7 * u, 0.7 * u],
                [0.9 * u, 0.7 * u, 1.3 * u],
                red,
            );
            put_turned(
                m,
                at,
                yaw,
                [-1.1 * u, -0.5 * u, 1.3 * u],
                [-0.4 * u, 0.5 * u, 1.9 * u],
                white,
            );
        }
    }
    if v.night {
        night::vehicle_lights(m, r, s, dir, night::Pose { at, yaw }, u, v.px_km);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placement(born: Option<f32>) -> Placement {
        Placement {
            activity: crate::data::Activity {
                born,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn growth_goes_up_in_whole_steps_and_settles() {
        let p = placement(Some(100.0));
        assert_eq!(growth(&placement(None), 5.0), None);
        let steps: Vec<f32> = [100.0, 100.25, 100.45, 100.65, 100.85, 101.0, 500.0]
            .iter()
            .map(|t| growth(&p, *t).unwrap())
            .collect();
        assert_eq!(steps, vec![0.2, 0.4, 0.6, 0.8, 1.0, 1.0, 1.0]);
        assert!(drawn_by_anim(&p, 100.1));
        assert!(!drawn_by_anim(&p, 102.0));
    }

    #[test]
    fn the_clock_wraps_and_stills() {
        assert_eq!(clock(5.0, false), 0.0);
        assert_eq!(clock(3600.0 + 5.0, true), 5.0);
        // Every period divides the wrap, so the wrap never shows.
        for p in [1.6f32, 6.0, 48.0, 91.7] {
            let q = period(p);
            let n = CLOCK_WRAP_S as f32 / q;
            assert!((n - n.round()).abs() < 1e-3, "{p} gave {q}");
        }
    }

    #[test]
    fn vehicles_go_out_and_come_back_along_their_route() {
        let r = Route::new(
            Kind::Truck,
            vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 10.0, 0.0]],
            2,
            1,
        )
        .unwrap();
        assert!((r.length() - 20.0).abs() < 1e-4);
        let (s, dir) = r.state(0, 0.0);
        let (s2, _) = r.state(0, period(2.0 * r.length() / r.speed_km_s));
        // After one full lap it is back where it started.
        assert!((s - s2).abs() < 1e-3 && dir > 0.0 || dir < 0.0);
        let (p, _) = r.at(15.0);
        assert!((p[0] - 10.0).abs() < 1e-4 && (p[1] - 5.0).abs() < 1e-4);
        for k in 0..40 {
            let (s, _) = r.state(1, k as f32 * 3.7);
            assert!((0.0..=r.length()).contains(&s));
        }
    }

    #[test]
    fn chimneys_are_found_from_the_boxes() {
        let model = VoxelModel {
            boxes: vec![
                crate::data::VoxelBox {
                    min: [-8.0, -8.0, 0.0],
                    max: [8.0, 8.0, 5.0],
                    colour: [1, 2, 3],
                    trim: false,
                    windows: None,
                },
                crate::data::VoxelBox {
                    min: [3.0, 3.0, 5.0],
                    max: [5.0, 5.0, 14.0],
                    colour: [1, 2, 3],
                    trim: false,
                    windows: None,
                },
            ],
            footprint: [16.0, 16.0],
        };
        let s = stacks_of(&model, "energy");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].at, [4.0, 4.0, 14.0]);
        // A smoking sector with no chimney gets roof vents; a quiet one gets nothing.
        let mut body = model.boxes[0].clone();
        body.min[2] = 0.5;
        let flat = VoxelModel {
            boxes: vec![body],
            footprint: [16.0, 16.0],
        };
        assert_eq!(stacks_of(&flat, "manufacturing").len(), 2);
        assert!(stacks_of(&flat, "retail").is_empty());
    }
}
