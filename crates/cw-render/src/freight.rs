//! Where vehicles run: routes laid from the game's freight lanes along the drawn roads, railways,
//! rivers and sea lanes between the lanes' regions, the harbours at the coastal and river ends of
//! sea and river freight, and the older plant-based routes used when the game has sent no lanes.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use crate::anim::{Kind, Route};
use crate::data::{Lane, Owner, Scene, WATER_LAKE, WATER_OCEAN};
use crate::fleet::{Harbour, cargo_of_goods, cargo_of_industry, ship_of};
use crate::game::SiteStatus;
use crate::mesh;

/// Everything vehicles need from a scene: their routes and the harbours they berth at.
#[derive(Default)]
pub struct Fleet {
    pub routes: Vec<Route>,
    pub harbours: Vec<Harbour>,
}

/// At most this many lanes are laid, the busiest first. Vehicles outside the view cost one
/// distance test each, so there is no budget on them.
const MAX_LANES: usize = 260;

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

/// A network of drawn lines that routes can be found on. Lines meet where their points lie
/// within a kilometre and a half of each other.
pub struct Network {
    pts: Vec<[f32; 3]>,
    adj: Vec<Vec<(u32, f32)>>,
    chain: Vec<u32>,
    grid: HashMap<(i32, i32), Vec<u32>>,
}

const CELL: f32 = 2.0;
const LINK_KM: f32 = 1.5;

fn cell_of(x: f32, y: f32) -> (i32, i32) {
    ((x / CELL).floor() as i32, (y / CELL).floor() as i32)
}

impl Network {
    pub fn new(chains: &[Vec<[f32; 3]>]) -> Network {
        let mut n = Network {
            pts: Vec::new(),
            adj: Vec::new(),
            chain: Vec::new(),
            grid: HashMap::new(),
        };
        for (ci, c) in chains.iter().enumerate() {
            let mut prev: Option<u32> = None;
            for p in c {
                let id = n.pts.len() as u32;
                n.pts.push(*p);
                n.adj.push(Vec::new());
                n.chain.push(ci as u32);
                n.grid.entry(cell_of(p[0], p[1])).or_default().push(id);
                if let Some(q) = prev {
                    let d = dist((p[0], p[1]), (n.pts[q as usize][0], n.pts[q as usize][1]));
                    n.adj[q as usize].push((id, d));
                    n.adj[id as usize].push((q, d));
                }
                prev = Some(id);
            }
        }
        let mut links: Vec<(u32, u32, f32)> = Vec::new();
        for i in 0..n.pts.len() {
            let (cx, cy) = cell_of(n.pts[i][0], n.pts[i][1]);
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for &j in n.grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                        if (j as usize) <= i || n.chain[j as usize] == n.chain[i] {
                            continue;
                        }
                        let d = dist(
                            (n.pts[i][0], n.pts[i][1]),
                            (n.pts[j as usize][0], n.pts[j as usize][1]),
                        );
                        if d <= LINK_KM {
                            links.push((i as u32, j, d));
                        }
                    }
                }
            }
        }
        for (a, b, d) in links {
            n.adj[a as usize].push((b, d));
            n.adj[b as usize].push((a, d));
        }
        n
    }

    /// The node nearest `p` within `reach` km.
    pub fn nearest(&self, p: (f32, f32), reach: f32) -> Option<u32> {
        let rings = (reach / CELL).ceil() as i32 + 1;
        let (cx, cy) = cell_of(p.0, p.1);
        let mut best: Option<(u32, f32)> = None;
        for dx in -rings..=rings {
            for dy in -rings..=rings {
                for &j in self.grid.get(&(cx + dx, cy + dy)).into_iter().flatten() {
                    let q = self.pts[j as usize];
                    let d = dist((q[0], q[1]), p);
                    if d <= reach && best.is_none_or(|b| d < b.1) {
                        best = Some((j, d));
                    }
                }
            }
        }
        best.map(|b| b.0)
    }

    pub fn point(&self, id: u32) -> [f32; 3] {
        self.pts[id as usize]
    }

    /// The shortest way from node `a` to node `b` no longer than `max_km`, as points.
    pub fn path(&self, a: u32, b: u32, max_km: f32) -> Option<Vec<[f32; 3]>> {
        let goal = self.pts[b as usize];
        let h = |i: u32| {
            let p = self.pts[i as usize];
            dist((p[0], p[1]), (goal[0], goal[1]))
        };
        let mut g = vec![f32::INFINITY; self.pts.len()];
        let mut from = vec![u32::MAX; self.pts.len()];
        let mut heap = BinaryHeap::new();
        g[a as usize] = 0.0;
        heap.push(Reverse(((h(a) * 100.0) as u32, a)));
        while let Some(Reverse((_, i))) = heap.pop() {
            if i == b {
                let mut out = vec![self.pts[b as usize]];
                let mut at = b;
                while at != a {
                    at = from[at as usize];
                    out.push(self.pts[at as usize]);
                }
                out.reverse();
                return Some(out);
            }
            for &(j, d) in &self.adj[i as usize] {
                let ng = g[i as usize] + d;
                if ng < g[j as usize] && ng <= max_km {
                    g[j as usize] = ng;
                    from[j as usize] = i;
                    heap.push(Reverse((((ng + h(j)) * 100.0) as u32, j)));
                }
            }
        }
        None
    }
}

fn is_water(scene: &Scene, x: f32, y: f32) -> bool {
    let (lon, lat) = scene.projection.to_lonlat(x, y);
    let w = scene.mask.at(lon, lat)[2];
    w == WATER_OCEAN || w == WATER_LAKE
}

/// Distance from `from` along `dir` to the last land before water that goes on for at least
/// 3 km, searched in 0.1 km steps out to `max` km.
fn shore_toward(scene: &Scene, from: (f32, f32), dir: [f32; 2], max: f32) -> Option<f32> {
    let at = |d: f32| (from.0 + dir[0] * d, from.1 + dir[1] * d);
    let mut d = 0.3;
    while d <= max {
        let (x, y) = at(d);
        if is_water(scene, x, y) {
            let open = (1..=6).all(|k| {
                let (x, y) = at(d + k as f32 * 0.5);
                is_water(scene, x, y)
            });
            return (open && d > 0.4).then_some(d - 0.1);
        }
        d += 0.1;
    }
    None
}

/// The shore near `hub` where a harbour can stand: the shore point and the direction from the
/// land out over the water. The nearest shore wins, but a spot with other buildings standing on
/// it (a downtown) costs as much as a few kilometres, so the quays are not built into towers.
pub fn coast_near(scene: &Scene, hub: (f32, f32), reach: f32) -> Option<((f32, f32), [f32; 2])> {
    if is_water(scene, hub.0, hub.1) {
        return None;
    }
    let crowd = |p: (f32, f32)| {
        scene
            .buildings
            .iter()
            .filter(|b| {
                let q = scene.projection.to_km(b.lon, b.lat);
                dist(q, p) < 1.2
            })
            .count() as f32
    };
    let mut best: Option<(f32, f32, [f32; 2])> = None;
    for k in 0..48 {
        let a = k as f32 * std::f32::consts::TAU / 48.0;
        let dir = [a.cos(), a.sin()];
        if let Some(d) = shore_toward(scene, hub, dir, reach) {
            let at = (hub.0 + dir[0] * d, hub.1 + dir[1] * d);
            let cost = d + 0.5 * crowd(at).min(40.0);
            if best.is_none_or(|b| cost < b.0) {
                best = Some((cost, d, dir));
            }
        }
    }
    let (_, d, dir) = best?;
    Some(((hub.0 + dir[0] * d, hub.1 + dir[1] * d), dir))
}

fn water_z(scene: &Scene, x: f32, y: f32) -> f32 {
    mesh::ground_z(scene, x, y) + 0.012
}

/// Is the straight way between two points all water?
fn clear_water(scene: &Scene, a: (f32, f32), b: (f32, f32)) -> bool {
    let n = (dist(a, b) / 0.8).ceil().max(1.0) as usize;
    (0..=n).all(|k| {
        let t = k as f32 / n as f32;
        is_water(scene, a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    })
}

/// A way over water between two points: straight when it can be, else found on a coarse grid
/// of water cells and pulled straight again where the view allows.
pub fn water_path(scene: &Scene, a: (f32, f32), b: (f32, f32)) -> Option<Vec<(f32, f32)>> {
    if clear_water(scene, a, b) {
        return Some(vec![a, b]);
    }
    let cell = 3.0f32;
    let margin = 45.0f32;
    let (x0, x1) = (a.0.min(b.0) - margin, a.0.max(b.0) + margin);
    let (y0, y1) = (a.1.min(b.1) - margin, a.1.max(b.1) + margin);
    let (w, h) = (
        ((x1 - x0) / cell).ceil() as usize + 1,
        ((y1 - y0) / cell).ceil() as usize + 1,
    );
    if w * h > 160_000 {
        return None;
    }
    let centre =
        |ix: usize, iy: usize| (x0 + (ix as f32 + 0.5) * cell, y0 + (iy as f32 + 0.5) * cell);
    let index = |p: (f32, f32)| {
        (
            (((p.0 - x0) / cell) as usize).min(w - 1),
            (((p.1 - y0) / cell) as usize).min(h - 1),
        )
    };
    let wet: Vec<bool> = (0..w * h)
        .map(|i| {
            let c = centre(i % w, i / w);
            is_water(scene, c.0, c.1)
        })
        .collect();
    let (sa, sb) = (index(a), index(b));
    let (start, goal) = (sa.1 * w + sa.0, sb.1 * w + sb.0);
    let mut g = vec![f32::INFINITY; w * h];
    let mut from = vec![usize::MAX; w * h];
    let mut heap = BinaryHeap::new();
    g[start] = 0.0;
    heap.push(Reverse((0u32, start)));
    let gp = centre(sb.0, sb.1);
    while let Some(Reverse((_, i))) = heap.pop() {
        if i == goal {
            break;
        }
        let (ix, iy) = ((i % w) as i32, (i / w) as i32);
        for (dx, dy) in [
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, -1),
            (-1, 1),
            (1, 1),
        ] {
            let (jx, jy) = (ix + dx, iy + dy);
            if jx < 0 || jy < 0 || jx >= w as i32 || jy >= h as i32 {
                continue;
            }
            let j = jy as usize * w + jx as usize;
            if !wet[j] && j != goal {
                continue;
            }
            let step = if dx != 0 && dy != 0 { 1.414 } else { 1.0 } * cell;
            let ng = g[i] + step;
            if ng < g[j] {
                g[j] = ng;
                from[j] = i;
                let c = centre(jx as usize, jy as usize);
                heap.push(Reverse((((ng + dist(c, gp)) * 10.0) as u32, j)));
            }
        }
    }
    if !g[goal].is_finite() {
        return None;
    }
    let mut cells = vec![goal];
    let mut at = goal;
    while at != start {
        at = from[at];
        cells.push(at);
    }
    cells.reverse();
    let mut pts: Vec<(f32, f32)> = vec![a];
    pts.extend(cells.iter().map(|c| centre(c % w, c / w)));
    pts.push(b);
    // Pull it straight: from each kept point jump to the furthest point still in clear water.
    let mut out = vec![pts[0]];
    let mut i = 0;
    while i < pts.len() - 1 {
        let mut j = pts.len() - 1;
        while j > i + 1 && !clear_water(scene, pts[i], pts[j]) {
            j -= 1;
        }
        out.push(pts[j]);
        i = j;
    }
    Some(out)
}

/// A harbour on a shore: `shore` is the last land point and `dir` points out over the water.
fn harbour_at(scene: &Scene, shore: (f32, f32), dir: [f32; 2], volume: f32, seed: u32) -> Harbour {
    let yaw = dir[1].atan2(dir[0]);
    let z = water_z(scene, shore.0, shore.1).max(mesh::ground_z(
        scene,
        shore.0 - dir[0] * 0.2,
        shore.1 - dir[1] * 0.2,
    )) + 0.004;
    Harbour {
        at: [shore.0, shore.1, z],
        yaw,
        len: 0.55 + 0.45 * volume.clamp(0.0, 1.0),
        cranes: 2 + (volume > 0.5) as u32 + (volume > 0.85) as u32,
        seed,
    }
}

/// How many vehicles a route of `len` km holds: one per `spacing` km, at least one.
fn count_for(len: f32, spacing: f32) -> u32 {
    ((len / spacing).round() as u32).clamp(1, 40)
}

/// The vehicles' spacing along a lane in km, closer on a busier lane.
fn spacing(volume: f32, base: f32) -> f32 {
    base / (0.35 + 1.4 * volume.clamp(0.0, 1.0))
}

/// Lay the routes and harbours of a scene.
pub fn build(scene: &Scene) -> Fleet {
    let mut fleet = if scene.lanes.is_empty() {
        Fleet::default()
    } else {
        from_lanes(scene)
    };
    if fleet.routes.is_empty() {
        let plants = from_plants(scene);
        fleet.routes = plants.routes;
        fleet.harbours = plants.harbours;
    }
    fleet
}

/// The cargo of a lane: from the goods when the game names them, else from the sector of the
/// plants standing round the lane's start (the most common one).
fn lane_cargo(scene: &Scene, lane: &Lane, from: (f32, f32)) -> crate::fleet::Cargo {
    if !lane.goods.is_empty() {
        return cargo_of_goods(&lane.goods);
    }
    let mut votes: HashMap<&str, u32> = HashMap::new();
    for p in &scene.buildings {
        if p.owner == Owner::Scenery {
            continue;
        }
        if dist(scene.projection.to_km(p.lon, p.lat), from) < 25.0 {
            *votes
                .entry(p.model.split('/').next().unwrap_or(""))
                .or_default() += 1;
        }
    }
    votes
        .into_iter()
        .max_by_key(|(name, n)| (*n, std::cmp::Reverse(*name)))
        .map_or(crate::fleet::Cargo::Box, |(name, _)| {
            cargo_of_industry(name)
        })
}

fn region_hub(scene: &Scene, code: &str) -> Option<(f32, f32)> {
    scene
        .regions
        .iter()
        .find(|r| r.code == code)
        .map(|r| scene.projection.to_km(r.lon, r.lat))
}

/// Chains of a line class group (ground points), for a network.
fn chains_of(scene: &Scene, classes: std::ops::RangeInclusive<u8>) -> Vec<Vec<[f32; 3]>> {
    mesh::road_chains_for_band(scene, &mesh::ROAD_BANDS[2])
        .into_iter()
        .filter(|(c, _)| classes.contains(c))
        .map(|(_, pts)| pts.into_iter().map(|p| p.xyz).collect())
        .collect()
}

fn river_network(scene: &Scene) -> Network {
    Network::new(
        &mesh::river_chains(scene, 2)
            .into_iter()
            .map(|c| {
                c.into_iter()
                    .map(|p| [p.xyz[0], p.xyz[1], p.xyz[2] + 0.012])
                    .collect()
            })
            .collect::<Vec<_>>(),
    )
}

/// A harbour on a river at node `at`: the quay lies on the bank to the left of the way the
/// river runs and faces the stream.
fn river_harbour(net: &Network, at: u32, path: &[[f32; 3]], volume: f32, seed: u32) -> Harbour {
    let p = net.point(at);
    let next = path.get(1).copied().unwrap_or([p[0] + 1.0, p[1], p[2]]);
    let (tx, ty) = (next[0] - p[0], next[1] - p[1]);
    let l = (tx * tx + ty * ty).sqrt().max(1e-6);
    // The normal pointing from the bank into the stream.
    let dir = [ty / l, -tx / l];
    let yaw = dir[1].atan2(dir[0]);
    Harbour {
        at: [p[0] - dir[0] * 0.10, p[1] - dir[1] * 0.10, p[2] + 0.004],
        yaw,
        len: 0.55 + 0.4 * volume.clamp(0.0, 1.0),
        cranes: 2 + (volume > 0.6) as u32,
        seed,
    }
}

/// Add a harbour to the fleet or reuse one standing within 4 km; returns its index.
fn add_harbour(harbours: &mut Vec<Harbour>, h: Harbour) -> usize {
    if let Some(i) = harbours
        .iter()
        .position(|o| dist((o.at[0], o.at[1]), (h.at[0], h.at[1])) < 4.0)
    {
        harbours[i].cranes = harbours[i].cranes.max(h.cranes);
        harbours[i].len = harbours[i].len.max(h.len);
        return i;
    }
    harbours.push(h);
    harbours.len() - 1
}

fn from_lanes(scene: &Scene) -> Fleet {
    let mut lanes: Vec<&Lane> = scene.lanes.iter().filter(|l| l.volume > 0.0).collect();
    lanes.sort_by(|a, b| b.volume.total_cmp(&a.volume));
    lanes.truncate(MAX_LANES);
    let mut fleet = Fleet::default();
    let needs = |modes: &[&str]| lanes.iter().any(|l| modes.contains(&l.mode.as_str()));
    let roads = needs(&["truck", "road"]).then(|| Network::new(&chains_of(scene, 0..=1)));
    let rails = needs(&["rail"]).then(|| Network::new(&chains_of(scene, 2..=2)));
    let rivers = needs(&["sea", "river"]).then(|| river_network(scene));
    for (li, lane) in lanes.iter().enumerate() {
        let (Some(a), Some(b)) = (region_hub(scene, &lane.from), region_hub(scene, &lane.to))
        else {
            continue;
        };
        let straight = dist(a, b);
        if straight < 3.0 {
            continue;
        }
        let seed = li as u32 * 17 + 3;
        let cargo = lane_cargo(scene, lane, a);
        let by_net = |net: &Network, reach: f32| -> Option<Vec<[f32; 3]>> {
            let (na, nb) = (net.nearest(a, reach), net.nearest(b, reach));
            net.path(na?, nb?, straight * 2.2 + 30.0)
        };
        match lane.mode.as_str() {
            "truck" | "road" => {
                if let Some(path) = roads.as_ref().and_then(|n| by_net(n, 30.0))
                    && let Some(mut r) = Route::new(Kind::Truck, path, 1, seed)
                {
                    r.cargo = cargo;
                    r.count = count_for(r.length(), spacing(lane.volume, 14.0));
                    fleet.routes.push(r);
                }
            }
            "rail" => {
                if let Some(path) = rails.as_ref().and_then(|n| by_net(n, 30.0))
                    && let Some(mut r) = Route::new(Kind::Train, path, 1, seed)
                {
                    r.cargo = cargo;
                    r.wagons = 4 + (lane.volume.clamp(0.0, 1.0) * 4.0).round() as u32;
                    r.count = count_for(r.length(), spacing(lane.volume, 70.0));
                    fleet.routes.push(r);
                }
            }
            "sea" | "river" => {
                // On a river when both regions lie by one and a way along it exists; else over
                // open water between two coastal harbours.
                let river = rivers.as_ref().and_then(|net| {
                    let (na, nb) = (net.nearest(a, 18.0)?, net.nearest(b, 18.0)?);
                    let path = net.path(na, nb, straight * 2.0 + 30.0)?;
                    let mut back = path.clone();
                    back.reverse();
                    let ha = river_harbour(net, na, &path, lane.volume, seed);
                    let hb = river_harbour(net, nb, &back, lane.volume, seed + 1);
                    Some((path, ha, hb))
                });
                if let Some((path, ha, hb)) = river {
                    let (ia, ib) = (
                        add_harbour(&mut fleet.harbours, ha),
                        add_harbour(&mut fleet.harbours, hb),
                    );
                    let (pa, ya) = fleet.harbours[ia].berth();
                    let (pb, yb) = fleet.harbours[ib].berth();
                    let mut pts = vec![pa];
                    pts.extend(path);
                    pts.push(pb);
                    if let Some(mut r) = Route::new(Kind::Ship, pts, 1, seed) {
                        r.ship = ship_of(cargo, true);
                        r.ends = [Some(ya), Some(yb)];
                        r.dwell = 40.0;
                        r.count = count_for(r.length(), spacing(lane.volume, 30.0)).min(8);
                        fleet.routes.push(r);
                    }
                } else if let (Some((sa, da)), Some((sb, db))) =
                    (coast_near(scene, a, 60.0), coast_near(scene, b, 60.0))
                {
                    let ia = add_harbour(
                        &mut fleet.harbours,
                        harbour_at(scene, sa, da, lane.volume, seed),
                    );
                    let ib = add_harbour(
                        &mut fleet.harbours,
                        harbour_at(scene, sb, db, lane.volume, seed + 1),
                    );
                    let (pa, ya) = fleet.harbours[ia].berth();
                    let (pb, yb) = fleet.harbours[ib].berth();
                    if let Some(way) = water_path(scene, (pa[0], pa[1]), (pb[0], pb[1])) {
                        let mut pts: Vec<[f32; 3]> = way
                            .iter()
                            .map(|p| [p.0, p.1, water_z(scene, p.0, p.1)])
                            .collect();
                        pts[0][2] = pa[2];
                        let last = pts.len() - 1;
                        pts[last][2] = pb[2];
                        if let Some(mut r) = Route::new(Kind::Ship, pts, 1, seed) {
                            r.ship = ship_of(cargo, false);
                            r.ends = [Some(ya), Some(yb)];
                            r.dwell = 40.0;
                            r.count = count_for(r.length(), spacing(lane.volume, 60.0)).min(8);
                            fleet.routes.push(r);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    fleet
}

/// The routes laid from the plants alone, for a game that has sent no lanes: trucks toward the
/// nearest city, trains along the nearest railway and, for coastal plants, a ship from a harbour
/// out over the water.
fn from_plants(scene: &Scene) -> Fleet {
    let chains = mesh::road_chains_for_band(scene, &mesh::ROAD_BANDS[2]);
    let cities: Vec<(f32, f32)> = scene
        .cities
        .iter()
        .map(|c| scene.projection.to_km(c.lon, c.lat))
        .collect();
    let mut fleet = Fleet::default();
    for (bi, p) in scene.buildings.iter().enumerate() {
        if p.owner == Owner::Scenery || p.activity.status != Some(SiteStatus::Running) {
            continue;
        }
        let (cx, cy) = scene.projection.to_km(p.lon, p.lat);
        let util = p.activity.utilisation;
        let n = 1 + (util * 2.0).round() as u32;
        let seed = bi as u32 * 31 + 5;
        let industry = p.model.split('/').next().unwrap_or("");
        let cargo = cargo_of_industry(industry);
        let city = cities
            .iter()
            .copied()
            .filter(|c| dist(*c, (cx, cy)) > 4.0)
            .min_by(|a, b| dist(*a, (cx, cy)).total_cmp(&dist(*b, (cx, cy))));
        if let Some(path) = chain_path(&chains, (cx, cy), 0..=1, 9.0, 28.0, city)
            && let Some(mut r) = Route::new(Kind::Truck, path, n, seed)
        {
            r.cargo = cargo;
            fleet.routes.push(r);
        }
        if util > 0.2
            && let Some(path) = chain_path(&chains, (cx, cy), 2..=2, 22.0, 34.0, None)
            && let Some(mut r) = Route::new(Kind::Train, path, 1, seed + 1)
        {
            r.cargo = cargo;
            r.wagons = 4 + (util * 4.0).round() as u32;
            fleet.routes.push(r);
        }
        if matches!(
            industry,
            "energy" | "extraction" | "logistics" | "chemical_industries" | "agriculture"
        ) && let Some((shore, dir)) = coast_near(scene, (cx, cy), 8.0)
        {
            let h = harbour_at(scene, shore, dir, util, seed);
            let hi = add_harbour(&mut fleet.harbours, h);
            let (pa, ya) = fleet.harbours[hi].berth();
            // Out over the water as far as it stays water, up to 30 km.
            let mut far = 1.0f32;
            while far < 30.0
                && is_water(
                    scene,
                    shore.0 + dir[0] * (far + 1.0),
                    shore.1 + dir[1] * (far + 1.0),
                )
            {
                far += 1.0;
            }
            if far >= 12.0 {
                let end = (
                    shore.0 + dir[0] * (far - 2.0),
                    shore.1 + dir[1] * (far - 2.0),
                );
                let pts = vec![pa, [end.0, end.1, water_z(scene, end.0, end.1)]];
                if let Some(mut r) = Route::new(Kind::Ship, pts, 2, seed + 2) {
                    r.ship = ship_of(cargo, false);
                    r.ends = [Some(ya), None];
                    r.dwell = 30.0;
                    fleet.routes.push(r);
                }
            }
        }
    }
    fleet
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

/// How many vehicles of each kind and how many harbours the scene's fleet has (for tests and
/// diagnostics).
pub fn summary(scene: &Scene) -> ([usize; 3], usize) {
    let f = build(scene);
    let mut n = [0; 3];
    for r in &f.routes {
        n[r.kind as usize] += 1;
    }
    (n, f.harbours.len())
}

/// Where the scene's harbours stand, as (longitude, latitude) of the middle of each quay.
pub fn harbour_sites(scene: &Scene) -> Vec<(f32, f32)> {
    build(scene)
        .harbours
        .iter()
        .map(|h| scene.projection.to_lonlat(h.at[0], h.at[1]))
        .collect()
}

/// Every vehicle of the scene's fleet at clock `t`, for pictures and tests: its kind (0 truck,
/// 1 train, 2 ship), cargo (0 tanker, 1 flatbed, 2 box, 3 hopper), ship kind (0 bulk, 1 tanker,
/// 2 container, 3 barge) and position.
pub fn vehicles_at(scene: &Scene, t: f32) -> Vec<(u8, u8, u8, f32, f32)> {
    let fleet = build(scene);
    let mut out = Vec::new();
    for r in &fleet.routes {
        for k in 0..r.count {
            let (s, _) = r.state(k, t);
            let (p, _) = r.at(s);
            let (lon, lat) = scene.projection.to_lonlat(p[0], p[1]);
            out.push((r.kind as u8, r.cargo as u8, r.ship as u8, lon, lat));
        }
    }
    out
}
