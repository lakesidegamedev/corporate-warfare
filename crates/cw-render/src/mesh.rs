//! Geometry: stepped terrace terrain, voxel buildings and the cloud sheet.

use std::collections::{BTreeMap, HashMap};

use bytemuck::{Pod, Zeroable};

use crate::data::{Owner, Scene, VoxelModel, WATER_LAKE};

/// Vertex kinds, carried in the colour alpha byte.
pub const KIND_TOP: u8 = 0;
pub const KIND_WALL: u8 = 1;
pub const KIND_BUILDING: u8 = 2;
pub const KIND_PLAIN: u8 = 3;
pub const KIND_CLOUD: u8 = 4;
pub const KIND_ROAD: u8 = 5;
/// The land and sea beyond the scenario's terrain, at low detail. The ground class is carried in
/// the red byte (a `ground` class, or `APRON_WATER`).
pub const KIND_APRON_TOP: u8 = 6;
pub const KIND_APRON_WALL: u8 = 7;
pub const APRON_WATER: u8 = 7;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    /// Box colour and the kind in alpha.
    pub colour: [u8; 4],
    /// Position inside the model in voxels (windows and trim); unused for terrain.
    pub local: [f32; 3],
    /// Position on the face, 0 to 1 each way (building outlines); unused for terrain.
    pub uv: [f32; 2],
}

const UV: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

pub const VERTEX_BYTES: usize = std::mem::size_of::<Vertex>();

#[derive(Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    fn quad(&mut self, p: [[f32; 3]; 4], normal: [f32; 3], colour: [u8; 4], local: [[f32; 3]; 4]) {
        let base = self.vertices.len() as u32;
        for i in 0..4 {
            self.vertices.push(Vertex {
                pos: p[i],
                normal,
                colour,
                local: local[i],
                uv: UV[i],
            });
        }
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Surface height of the Great Lakes in metres (published chart datums, rounded). The
/// elevation grid holds the lake beds, so lake cells take their surface instead.
fn lake_surface_m(lon: f32, lat: f32) -> f32 {
    if lon > -79.9 && lat > 43.0 {
        75.0 // Ontario
    } else if lat > 46.0 && lon < -84.4 {
        183.0 // Superior
    } else if lat < 42.9 && lon > -83.6 && lon < -78.8 {
        174.0 // Erie
    } else {
        176.0 // Michigan, Huron, St Clair
    }
}

const SMOOTH_RADIUS: usize = 3;
const MODE_RADIUS: usize = 3;
/// Land patches of one level smaller than this many cells are absorbed by their neighbours, so
/// near flat country never gets a terrace wall around a handful of cells.
const MIN_PATCH_CELLS: usize = 90;

const RELIEF_RADIUS_DEG: f32 = 0.6;
/// Relief (metres) at which mountain damping starts and where it is complete, and the share of
/// the height above the valley floor that is taken off at full damping.
const MOUNTAIN_RELIEF_LO_M: f32 = 800.0;
const MOUNTAIN_RELIEF_HI_M: f32 = 2400.0;
const MOUNTAIN_DAMP: f32 = 0.6;
/// Land above this height with relief under the two limits gets a terrace step of 3 or 2 levels.
const PLATEAU_MIN_M: f32 = 600.0;
const FLAT_RELIEF_M: f32 = 350.0;
const GENTLE_RELIEF_M: f32 = 700.0;

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Separable box blur of a grid, clamped at the edges.
fn box_blur(src: &[f32], w: usize, h: usize, radius: usize) -> Vec<f32> {
    let mut tmp = vec![0.0f32; w * h];
    for r in 0..h {
        for c in 0..w {
            let (lo, hi) = (c.saturating_sub(radius), (c + radius).min(w - 1));
            tmp[r * w + c] = (lo..=hi).map(|k| src[r * w + k]).sum::<f32>() / (hi - lo + 1) as f32;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for r in 0..h {
        let (lo, hi) = (r.saturating_sub(radius), (r + radius).min(h - 1));
        for c in 0..w {
            out[r * w + c] = (lo..=hi).map(|k| tmp[k * w + c]).sum::<f32>() / (hi - lo + 1) as f32;
        }
    }
    out
}

/// Separable box maximum (or minimum) of a grid, clamped at the edges.
fn box_extreme(src: &[f32], w: usize, h: usize, radius: usize, max: bool) -> Vec<f32> {
    let pick = |a: f32, b: f32| if max { a.max(b) } else { a.min(b) };
    let start = if max { f32::MIN } else { f32::MAX };
    let mut tmp = vec![0.0f32; w * h];
    for r in 0..h {
        for c in 0..w {
            let (lo, hi) = (c.saturating_sub(radius), (c + radius).min(w - 1));
            tmp[r * w + c] = (lo..=hi).fold(start, |m, k| pick(m, src[r * w + k]));
        }
    }
    let mut out = vec![0.0f32; w * h];
    for r in 0..h {
        let (lo, hi) = (r.saturating_sub(radius), (r + radius).min(h - 1));
        for c in 0..w {
            out[r * w + c] = (lo..=hi).fold(start, |m, k| pick(m, tmp[k * w + c]));
        }
    }
    out
}

/// Terrace level of every heightmap cell. Water sits on its own surface (lake chart datum or
/// sea level). Land is smoothed before it is quantised, then cleaned with a mode filter and a
/// minimum patch area, so flat country gets one or two gentle steps and no staircase.
pub fn build_levels(scene: &Scene) -> Vec<i32> {
    let hm = &scene.heightmap;
    let (w, h) = (hm.w, hm.h);
    let (dx, dy) = hm.cell_degrees();
    let mut value = vec![0.0f32; w * h];
    let mut water = vec![false; w * h];
    for r in 0..h {
        for c in 0..w {
            let lon = hm.bounds.west + (c as f32 + 0.5) * dx;
            let lat = hm.bounds.north - (r as f32 + 0.5) * dy;
            let kind = scene.mask.at(lon, lat)[2];
            let i = r * w + c;
            value[i] = match kind {
                crate::data::WATER_OCEAN => 0.0,
                WATER_LAKE => lake_surface_m(lon, lat),
                _ => (hm.at(c, r) as f32).max(0.0),
            };
            water[i] = kind == crate::data::WATER_OCEAN || kind == WATER_LAKE;
        }
    }
    // Smooth the land heights (two box passes), keeping water where it is.
    let boxed = |src: &[f32]| box_blur(src, w, h, SMOOTH_RADIUS);
    let mut smooth = boxed(&boxed(&value));
    for i in 0..w * h {
        if water[i] {
            smooth[i] = value[i];
        }
    }

    // Relief: the spread of heights within about 65 km. Steep mountain fronts are drawn lower
    // (the height above the valley floor is scaled down by relief), so a 28 times exaggerated
    // range reads as mountains and not as a wall. Flat plateaus get a wider terrace step below.
    let rad = ((RELIEF_RADIUS_DEG / dx).round() as usize).clamp(2, 40);
    let hi = box_extreme(&smooth, w, h, rad, true);
    let lo = box_extreme(&smooth, w, h, rad, false);
    let relief: Vec<f32> = hi.iter().zip(&lo).map(|(a, b)| a - b).collect();
    let floor_m = box_blur(&lo, w, h, rad * 2);
    let mut plateau = vec![0u8; w * h];
    for i in 0..w * h {
        if water[i] {
            continue;
        }
        let k =
            1.0 - MOUNTAIN_DAMP * smoothstep(MOUNTAIN_RELIEF_LO_M, MOUNTAIN_RELIEF_HI_M, relief[i]);
        let base = floor_m[i].min(smooth[i]);
        smooth[i] = (base + (smooth[i] - base) * k).max(0.0);
        if smooth[i] > PLATEAU_MIN_M {
            plateau[i] = if relief[i] < FLAT_RELIEF_M {
                3
            } else if relief[i] < GENTLE_RELIEF_M {
                2
            } else {
                1
            };
        }
    }
    let mut level: Vec<i32> = smooth
        .iter()
        .zip(&plateau)
        .map(|(m, &step)| {
            let l = ((m + scene.terrace_offset_m) / scene.terrace_m).floor() as i32;
            if step > 1 {
                // Arid, low-relief country: a wider step, so flat country reads flat.
                (l as f32 / step as f32).round() as i32 * step as i32
            } else {
                l
            }
        })
        .collect();

    // Mode filter on land: a cell takes the most common level in its window.
    for _ in 0..2 {
        let prev = level.clone();
        for r in 0..h {
            for c in 0..w {
                if water[r * w + c] {
                    continue;
                }
                let mut counts: Vec<(i32, u32)> = Vec::new();
                for rr in r.saturating_sub(MODE_RADIUS)..=(r + MODE_RADIUS).min(h - 1) {
                    for cc in c.saturating_sub(MODE_RADIUS)..=(c + MODE_RADIUS).min(w - 1) {
                        let l = prev[rr * w + cc];
                        match counts.iter_mut().find(|e| e.0 == l) {
                            Some(e) => e.1 += 1,
                            None => counts.push((l, 1)),
                        }
                    }
                }
                let own = prev[r * w + c];
                let best = counts.iter().map(|e| e.1).max().unwrap_or(0);
                let own_n = counts.iter().find(|e| e.0 == own).map_or(0, |e| e.1);
                if own_n < best {
                    // Ties go to the level nearest the cell's own.
                    level[r * w + c] = counts
                        .iter()
                        .filter(|e| e.1 == best)
                        .min_by_key(|e| (e.0 - own).abs())
                        .map(|e| e.0)
                        .unwrap_or(own);
                }
            }
        }
    }

    // Absorb small patches into their most common neighbouring level.
    for _ in 0..3 {
        let mut seen = vec![false; w * h];
        let mut changed = false;
        for start in 0..w * h {
            if seen[start] {
                continue;
            }
            let l = level[start];
            let mut stack = vec![start];
            let mut cells = Vec::new();
            let mut has_water = false;
            seen[start] = true;
            while let Some(i) = stack.pop() {
                cells.push(i);
                has_water |= water[i];
                let (r, c) = (i / w, i % w);
                let mut push = |j: usize| {
                    if !seen[j] && level[j] == l {
                        seen[j] = true;
                        stack.push(j);
                    }
                };
                if c > 0 {
                    push(i - 1);
                }
                if c + 1 < w {
                    push(i + 1);
                }
                if r > 0 {
                    push(i - w);
                }
                if r + 1 < h {
                    push(i + w);
                }
            }
            if has_water || cells.len() >= MIN_PATCH_CELLS {
                continue;
            }
            let mut border: Vec<(i32, u32)> = Vec::new();
            for &i in &cells {
                let (r, c) = (i / w, i % w);
                let mut look = |j: usize| {
                    if level[j] != l {
                        match border.iter_mut().find(|e| e.0 == level[j]) {
                            Some(e) => e.1 += 1,
                            None => border.push((level[j], 1)),
                        }
                    }
                };
                if c > 0 {
                    look(i - 1);
                }
                if c + 1 < w {
                    look(i + 1);
                }
                if r > 0 {
                    look(i - w);
                }
                if r + 1 < h {
                    look(i + w);
                }
            }
            if let Some(&(to, _)) = border.iter().max_by_key(|e| e.1) {
                for &i in &cells {
                    level[i] = to;
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    level
}

pub fn level_z_km(scene: &Scene, level: i32) -> f32 {
    level as f32 * scene.terrace_m * scene.exaggeration / 1000.0
}

/// Horizontal run of a terrace wall: a steep slope (about 55 degrees), at most a cell wide, so
/// it is never thinner than a couple of pixels.
fn slope_run(scene: &Scene, rise_km: f32) -> f32 {
    let (dx, _) = scene.heightmap.cell_degrees();
    let cell_km = dx * scene.projection.km_per_deg_lon();
    (rise_km * 0.7).clamp(0.3 * cell_km, 0.9 * cell_km)
}

fn slope_normal(out: [f32; 2], rise: f32, run: f32) -> [f32; 3] {
    let l = (rise * rise + run * run).sqrt().max(1e-6);
    [out[0] * rise / l, out[1] * rise / l, run / l]
}

/// Greedy terrain mesh: coplanar cells of one level merge into big rectangles, and terrace
/// walls merge along their run. Ground class and water are read per pixel from textures, so
/// they never split geometry.
pub fn terrain(scene: &Scene) -> Mesh {
    let hm = &scene.heightmap;
    let (w, h) = (hm.w, hm.h);
    let (dx, dy) = hm.cell_degrees();
    let b = &hm.bounds;
    let lvl = |c: usize, r: usize| scene.levels[r * w + c];
    let km = |c: usize, r: usize| {
        // Corner of the grid (north west of cell c, r) in kilometres.
        scene
            .projection
            .to_km(b.west + c as f32 * dx, b.north - r as f32 * dy)
    };
    let mut m = Mesh::default();
    let top = [0, 0, 0, KIND_TOP];
    let wall = [0, 0, 0, KIND_WALL];
    let z0 = [0.0; 3];

    // Tops.
    let mut done = vec![false; w * h];
    for r in 0..h {
        for c in 0..w {
            if done[r * w + c] {
                continue;
            }
            let l = lvl(c, r);
            let mut c1 = c + 1;
            while c1 < w && !done[r * w + c1] && lvl(c1, r) == l {
                c1 += 1;
            }
            let mut r1 = r + 1;
            'grow: while r1 < h {
                for k in c..c1 {
                    if done[r1 * w + k] || lvl(k, r1) != l {
                        break 'grow;
                    }
                }
                r1 += 1;
            }
            for rr in r..r1 {
                for cc in c..c1 {
                    done[rr * w + cc] = true;
                }
            }
            let (x0, y1) = km(c, r);
            let (x1, y0) = km(c1, r1);
            let z = level_z_km(scene, l);
            m.quad(
                [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]],
                [0.0, 0.0, 1.0],
                top,
                [z0; 4],
            );
        }
    }

    // East walls: between column c and c + 1, merged down the rows.
    for c in 0..w.saturating_sub(1) {
        let mut r = 0;
        while r < h {
            let (h0, o0) = (lvl(c, r), lvl(c + 1, r));
            if h0 == o0 {
                r += 1;
                continue;
            }
            let mut r1 = r + 1;
            while r1 < h && lvl(c, r1) == h0 && lvl(c + 1, r1) == o0 {
                r1 += 1;
            }
            let (hi, lo) = if h0 > o0 { (h0, o0) } else { (o0, h0) };
            // Slope toward the lower side so the wall is never thinner than a pixel.
            let dir = if h0 > o0 { 1.0 } else { -1.0 };
            let (x, ya) = km(c + 1, r);
            let (_, yb) = km(c + 1, r1);
            let (za, zb) = (level_z_km(scene, lo), level_z_km(scene, hi));
            let run = slope_run(scene, zb - za) * dir;
            let n = slope_normal([dir, 0.0], zb - za, run.abs());
            m.quad(
                [
                    [x + run, yb, za],
                    [x + run, ya, za],
                    [x, ya, zb],
                    [x, yb, zb],
                ],
                n,
                wall,
                [z0; 4],
            );
            r = r1;
        }
    }
    // South walls: between row r and r + 1, merged along the columns.
    for r in 0..h.saturating_sub(1) {
        let mut c = 0;
        while c < w {
            let (h0, o0) = (lvl(c, r), lvl(c, r + 1));
            if h0 == o0 {
                c += 1;
                continue;
            }
            let mut c1 = c + 1;
            while c1 < w && lvl(c1, r) == h0 && lvl(c1, r + 1) == o0 {
                c1 += 1;
            }
            let (hi, lo) = if h0 > o0 { (h0, o0) } else { (o0, h0) };
            let dir = if h0 > o0 { -1.0 } else { 1.0 };
            let (xa, y) = km(c, r + 1);
            let (xb, _) = km(c1, r + 1);
            let (za, zb) = (level_z_km(scene, lo), level_z_km(scene, hi));
            let run = slope_run(scene, zb - za) * dir;
            let n = slope_normal([0.0, dir], zb - za, run.abs());
            m.quad(
                [
                    [xa, y + run, za],
                    [xb, y + run, za],
                    [xb, y, zb],
                    [xa, y, zb],
                ],
                n,
                wall,
                [z0; 4],
            );
            c = c1;
        }
    }
    m
}

fn box_faces(
    m: &mut Mesh,
    tf: &dyn Fn([f32; 3]) -> [f32; 3],
    rot: &dyn Fn([f32; 3]) -> [f32; 3],
    min: [f32; 3],
    max: [f32; 3],
    rgb: [u8; 3],
    kind: u8,
) {
    let c = [rgb[0], rgb[1], rgb[2], kind];
    let (a, b) = (min, max);
    // (corner order counter clockwise seen from outside, outward normal)
    let faces: [([[f32; 3]; 4], [f32; 3]); 6] = [
        (
            [
                [a[0], a[1], b[2]],
                [b[0], a[1], b[2]],
                [b[0], b[1], b[2]],
                [a[0], b[1], b[2]],
            ],
            [0.0, 0.0, 1.0],
        ),
        (
            [
                [a[0], a[1], a[2]],
                [a[0], b[1], a[2]],
                [b[0], b[1], a[2]],
                [b[0], a[1], a[2]],
            ],
            [0.0, 0.0, -1.0],
        ),
        (
            [
                [a[0], a[1], a[2]],
                [b[0], a[1], a[2]],
                [b[0], a[1], b[2]],
                [a[0], a[1], b[2]],
            ],
            [0.0, -1.0, 0.0],
        ),
        (
            [
                [b[0], b[1], a[2]],
                [a[0], b[1], a[2]],
                [a[0], b[1], b[2]],
                [b[0], b[1], b[2]],
            ],
            [0.0, 1.0, 0.0],
        ),
        (
            [
                [b[0], a[1], a[2]],
                [b[0], b[1], a[2]],
                [b[0], b[1], b[2]],
                [b[0], a[1], b[2]],
            ],
            [1.0, 0.0, 0.0],
        ),
        (
            [
                [a[0], b[1], a[2]],
                [a[0], a[1], a[2]],
                [a[0], a[1], b[2]],
                [a[0], b[1], b[2]],
            ],
            [-1.0, 0.0, 0.0],
        ),
    ];
    for (corners, n) in faces {
        m.quad(
            [
                tf(corners[0]),
                tf(corners[1]),
                tf(corners[2]),
                tf(corners[3]),
            ],
            rot(n),
            c,
            corners,
        );
    }
}

/// Voxel buildings standing on the terrain, each on a plinth down to the lowest ground under it.
pub fn buildings(scene: &Scene) -> Mesh {
    let mut m = Mesh::default();
    for p in &scene.buildings {
        let Some(model) = scene.models.get(&p.model) else {
            continue;
        };
        let (cx, cy) = scene.projection.to_km(p.lon, p.lat);
        let (hi, lo) = ground_range(scene, model, p.scale, cx, cy);
        let (z_top, z_low) = (level_z_km(scene, hi), level_z_km(scene, lo));
        let (s, c) = p.yaw.to_radians().sin_cos();
        let v = scene.voxel_km * p.scale;
        let rot = move |n: [f32; 3]| [n[0] * c - n[1] * s, n[0] * s + n[1] * c, n[2]];
        let tf = move |q: [f32; 3]| {
            [
                cx + (q[0] * c - q[1] * s) * v,
                cy + (q[0] * s + q[1] * c) * v,
                z_top + q[2] * v,
            ]
        };
        let half = [model.footprint[0] / 2.0, model.footprint[1] / 2.0];
        // Plinth: earth coloured, from the lowest ground under the footprint up to the base.
        let drop = (z_top - z_low) / v;
        if drop > 0.0 {
            box_faces(
                &mut m,
                &tf,
                &rot,
                [-half[0], -half[1], -drop],
                [half[0], half[1], 0.0],
                [0x8C, 0x75, 0x56],
                KIND_PLAIN,
            );
        }
        for b in &model.boxes {
            let tall = b.max[2] - b.min[2] >= 2.0 && b.min[2] >= 1.0;
            box_faces(
                &mut m,
                &tf,
                &rot,
                b.min,
                b.max,
                b.colour,
                if tall { KIND_BUILDING } else { KIND_PLAIN },
            );
        }
        if p.owner == Owner::Player {
            // Gold roofs: a thin gold slab over every top-most roof of the player's buildings.
            let top = model.boxes.iter().map(|b| b.max[2]).fold(0.0f32, f32::max);
            for b in &model.boxes {
                let wide = (b.max[0] - b.min[0]) * (b.max[1] - b.min[1]) >= 4.0;
                if b.max[2] >= top - 0.01 && wide {
                    box_faces(
                        &mut m,
                        &tf,
                        &rot,
                        [b.min[0], b.min[1], b.max[2]],
                        [b.max[0], b.max[1], b.max[2] + 0.7],
                        PLAYER_ROOF,
                        KIND_PLAIN,
                    );
                }
            }
        }
    }
    m
}

/// Roof colour of the player's own buildings (the palette's gold).
const PLAYER_ROOF: [u8; 3] = [0xE2, 0xB0, 0x4A];

/// A placed building's ground footprint and height, for the interface: it reserves the space on
/// the label planner and decides whether the building is big enough to show on screen.
#[derive(Clone, Debug)]
pub struct BuildingBox {
    pub lon: f32,
    pub lat: f32,
    /// Half the footprint's longest side, in kilometres.
    pub half_km: f32,
    pub height_km: f32,
    pub owner: Owner,
    pub model: String,
    /// The body colour of the sector's model.
    pub colour: [u8; 3],
}

pub fn building_boxes(scene: &Scene) -> Vec<BuildingBox> {
    scene
        .buildings
        .iter()
        .filter_map(|p| {
            let model = scene.models.get(&p.model)?;
            let v = scene.voxel_km * p.scale;
            let top = model.boxes.iter().map(|b| b.max[2]).fold(0.0f32, f32::max);
            // The sector colour is the colour of the biggest box above the plinth.
            let body = model
                .boxes
                .iter()
                .filter(|b| b.min[2] >= 1.0)
                .max_by(|a, b| {
                    let vol = |x: &crate::data::VoxelBox| {
                        (x.max[0] - x.min[0]) * (x.max[1] - x.min[1]) * (x.max[2] - x.min[2])
                    };
                    vol(a).total_cmp(&vol(b))
                })
                .map_or([0x80, 0x80, 0x80], |b| b.colour);
            Some(BuildingBox {
                lon: p.lon,
                lat: p.lat,
                half_km: model.footprint[0].max(model.footprint[1]) * v / 2.0,
                height_km: top * v,
                owner: p.owner,
                model: p.model.clone(),
                colour: body,
            })
        })
        .collect()
}

/// Highest and lowest terrace level under a building's footprint.
fn ground_range(scene: &Scene, model: &VoxelModel, scale: f32, cx: f32, cy: f32) -> (i32, i32) {
    let hm = &scene.heightmap;
    let r = model.footprint[0].max(model.footprint[1]) * scene.voxel_km * scale / 2.0;
    let (mut hi, mut lo) = (i32::MIN, i32::MAX);
    for (ox, oy) in [(-r, -r), (r, -r), (r, r), (-r, r), (0.0, 0.0)] {
        let (lon, lat) = scene.projection.to_lonlat(cx + ox, cy + oy);
        let (c, row) = hm.cell_of(lon, lat);
        let l = scene.level_at(c, row);
        hi = hi.max(l);
        lo = lo.min(l);
    }
    (hi, lo)
}

/// A flat sheet of cloud over the whole terrain extent.
pub fn cloud_sheet(scene: &Scene, altitude_km: f32) -> Mesh {
    let b = &scene.heightmap.bounds;
    let (x0, y0) = scene.projection.to_km(b.west, b.south);
    let (x1, y1) = scene.projection.to_km(b.east, b.north);
    let mut m = Mesh::default();
    let c = [255, 255, 255, KIND_CLOUD];
    let z = altitude_km;
    m.quad(
        [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]],
        [0.0, 0.0, 1.0],
        c,
        [[0.0; 3]; 4],
    );
    m
}

/// Height of the terrain mesh at a point, in kilometres: the terrace level of the cell, or, on
/// the low side of a terrace wall, the wall's ramp. This mirrors `terrain` exactly, so anything
/// laid along it sits on the ground the mesh draws.
pub fn ground_z(scene: &Scene, x: f32, y: f32) -> f32 {
    let hm = &scene.heightmap;
    let (lon, lat) = scene.projection.to_lonlat(x, y);
    let (dx, dy) = hm.cell_degrees();
    let (fc, fr) = ((lon - hm.bounds.west) / dx, (hm.bounds.north - lat) / dy);
    let c = (fc.floor().max(0.0) as usize).min(hm.w - 1);
    let r = (fr.floor().max(0.0) as usize).min(hm.h - 1);
    let own = scene.level_at(c, r);
    let za = level_z_km(scene, own);
    let (cell_w, cell_h) = (
        dx * scene.projection.km_per_deg_lon(),
        dy * crate::data::KM_PER_DEG_LAT,
    );
    let (fx, fy) = (
        (fc - c as f32).clamp(0.0, 1.0),
        (fr - r as f32).clamp(0.0, 1.0),
    );
    let mut z = za;
    let near = [
        (c.checked_sub(1).map(|k| (k, r)), fx * cell_w),
        ((c + 1 < hm.w).then_some((c + 1, r)), (1.0 - fx) * cell_w),
        (r.checked_sub(1).map(|k| (c, k)), fy * cell_h),
        ((r + 1 < hm.h).then_some((c, r + 1)), (1.0 - fy) * cell_h),
    ];
    for (n, d) in near {
        let Some((nc, nr)) = n else { continue };
        let lvl = scene.level_at(nc, nr);
        if lvl > own {
            let zb = level_z_km(scene, lvl);
            let run = slope_run(scene, zb - za);
            if d < run {
                z = z.max(zb - (zb - za) * d / run);
            }
        }
    }
    z
}

/// One end of a road segment ribbon: the vertex shader widens it in screen pixels.
pub struct RoadPoint {
    pub xyz: [f32; 3],
    /// Kilometres along the line, for dashes and sleepers cut on the ground.
    pub along: f32,
}

/// Height the roads sit above the terrain, in kilometres. The depth test does the rest.
const ROAD_LIFT_KM: f32 = 0.03;

/// Split a segment until a straight line between the points follows the ground to within
/// `tol` kilometres, so a road crosses a terrace wall as a short ramp and never cuts through it.
fn follow_ground(
    scene: &Scene,
    a: (f32, f32),
    b: (f32, f32),
    out: &mut Vec<(f32, f32)>,
    depth: u32,
) {
    let (za, zb) = (ground_z(scene, a.0, a.1), ground_z(scene, b.0, b.1));
    let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let mut bad = false;
    for t in [0.25f32, 0.5, 0.75] {
        let p = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        if (ground_z(scene, p.0, p.1) - (za + (zb - za) * t)).abs() > 0.03 {
            bad = true;
        }
    }
    if bad && len > 0.02 && depth < 14 {
        let m = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        follow_ground(scene, a, m, out, depth + 1);
        follow_ground(scene, m, b, out, depth + 1);
    } else {
        out.push(b);
    }
}

/// How much of the road network a zoom band draws and how hard it is simplified.
#[derive(Clone, Copy, Debug)]
pub struct RoadBand {
    /// Highest line class kept (0 interstates, 1 main roads, 2 railways).
    pub max_class: u8,
    /// Cell size in kilometres for merging near-parallel duplicate segments.
    pub merge_km: f32,
}

/// The three zoom bands: far (interstates), middle (interstates and rails) and closest (all).
pub const ROAD_BANDS: [RoadBand; 3] = [
    RoadBand {
        max_class: 0,
        merge_km: 4.0,
    },
    RoadBand {
        max_class: 2,
        merge_km: 1.8,
    },
    RoadBand {
        max_class: 2,
        merge_km: 0.6,
    },
];

/// Which road band a camera width in kilometres uses.
pub fn road_band(width_km: f32) -> usize {
    if width_km > 1500.0 {
        0
    } else if width_km > 450.0 {
        1
    } else {
        2
    }
}

/// Class priority for merging: interstates first, then railways, then main roads.
fn class_priority(class: u8) -> u8 {
    match class {
        0 => 0,
        2 => 1,
        _ => 2,
    }
}

/// Douglas-Peucker simplification of a polyline, keeping both ends.
fn douglas_peucker(pts: &[(f32, f32)], tol: f32) -> Vec<(f32, f32)> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0usize, pts.len() - 1)];
    while let Some((lo, hi)) = stack.pop() {
        let (a, b) = (pts[lo], pts[hi]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len2 = (dx * dx + dy * dy).max(1e-12);
        let mut worst = (0.0f32, lo);
        for (k, p) in pts.iter().enumerate().take(hi).skip(lo + 1) {
            let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
            let d = ((p.0 - a.0 - dx * t).powi(2) + (p.1 - a.1 - dy * t).powi(2)).sqrt();
            if d > worst.0 {
                worst = (d, k);
            }
        }
        if worst.0 > tol {
            keep[worst.1] = true;
            stack.push((lo, worst.1));
            stack.push((worst.1, hi));
        }
    }
    pts.iter()
        .zip(&keep)
        .filter(|(_, k)| **k)
        .map(|(p, _)| *p)
        .collect()
}

/// One map polyline cut at every heightmap cell edge it crosses and refined along terrace
/// walls, as points on the ground (kilometres, ground height).
fn ground_chain(scene: &Scene, km: &[(f32, f32)]) -> Vec<RoadPoint> {
    let hm = &scene.heightmap;
    let (dx, dy) = hm.cell_degrees();
    let mut flat: Vec<(f32, f32)> = vec![km[0]];
    for w in km.windows(2) {
        let (a, b) = (w[0], w[1]);
        // Parameters where the segment crosses a grid line of the heightmap.
        let (la, lb) = (line_lonlat(scene, a), line_lonlat(scene, b));
        let mut ts = vec![0.0f32, 1.0];
        let cross = |ua: f32, ub: f32, ts: &mut Vec<f32>| {
            let (lo, hi) = (ua.min(ub).floor() as i32, ua.max(ub).floor() as i32);
            for k in lo + 1..=hi {
                let t = (k as f32 - ua) / (ub - ua);
                if t > 0.0 && t < 1.0 {
                    ts.push(t);
                }
            }
        };
        cross(
            (la.0 - hm.bounds.west) / dx,
            (lb.0 - hm.bounds.west) / dx,
            &mut ts,
        );
        cross(
            (hm.bounds.north - la.1) / dy,
            (hm.bounds.north - lb.1) / dy,
            &mut ts,
        );
        ts.sort_by(|p, q| p.total_cmp(q));
        for pair in ts.windows(2) {
            let at = |t: f32| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            if pair[1] - pair[0] > 1e-6 {
                follow_ground(scene, at(pair[0]), at(pair[1]), &mut flat, 0);
            }
        }
    }
    let mut pts = Vec::with_capacity(flat.len());
    let mut along = 0.0f32;
    for (i, p) in flat.iter().enumerate() {
        if i > 0 {
            let q = flat[i - 1];
            along += ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
        }
        pts.push(RoadPoint {
            xyz: [p.0, p.1, ground_z(scene, p.0, p.1) + ROAD_LIFT_KM],
            along,
        });
    }
    pts
}

/// Roads and railways as chains of points on the ground, every map segment kept.
pub fn road_chains(scene: &Scene) -> Vec<(u8, Vec<RoadPoint>)> {
    scene
        .lines
        .iter()
        .map(|line| {
            let km: Vec<(f32, f32)> = line
                .points
                .iter()
                .map(|p| scene.projection.to_km(p.0, p.1))
                .collect();
            (line.class, ground_chain(scene, &km))
        })
        .collect()
}

/// The chains one zoom band draws: only its classes, Douglas-Peucker simplified, and with a
/// segment dropped when an interstate, a railway or another line of the same class already
/// runs within a cell of it (near-parallel duplicates merge into one corridor).
pub fn road_chains_for_band(scene: &Scene, band: &RoadBand) -> Vec<(u8, Vec<RoadPoint>)> {
    let s = band.merge_km;
    let mut order: Vec<usize> = (0..scene.lines.len())
        .filter(|i| scene.lines[*i].class <= band.max_class && scene.lines[*i].points.len() >= 2)
        .collect();
    order.sort_by_key(|i| (class_priority(scene.lines[*i].class), *i));
    // Cell occupancy: priority and line of whoever claimed the cell first.
    let mut cells: HashMap<(i32, i32), (u8, usize)> = HashMap::new();
    let mut out = Vec::new();
    for li in order {
        let line = &scene.lines[li];
        let prio = class_priority(line.class);
        let km: Vec<(f32, f32)> = line
            .points
            .iter()
            .map(|p| scene.projection.to_km(p.0, p.1))
            .collect();
        let km = douglas_peucker(&km, s * 0.6);
        let mut run: Vec<(f32, f32)> = Vec::new();
        let flush = |run: &mut Vec<(f32, f32)>, out: &mut Vec<(u8, Vec<RoadPoint>)>| {
            if run.len() >= 2 {
                out.push((line.class, ground_chain(scene, run)));
            }
            run.clear();
        };
        for w in km.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
            let n = (len / (s * 0.5)).ceil().max(1.0) as usize;
            let samples: Vec<(i32, i32)> = (0..=n)
                .map(|k| {
                    let t = k as f32 / n as f32;
                    (
                        ((a.0 + (b.0 - a.0) * t) / s).floor() as i32,
                        ((a.1 + (b.1 - a.1) * t) / s).floor() as i32,
                    )
                })
                .collect();
            let foreign = |c: (i32, i32)| {
                (-1..=1).any(|ox| {
                    (-1..=1).any(|oy| {
                        cells
                            .get(&(c.0 + ox, c.1 + oy))
                            .is_some_and(|(p, l)| *p < prio || (*p == prio && *l != li))
                    })
                })
            };
            if samples.iter().all(|c| foreign(*c)) {
                flush(&mut run, &mut out);
                continue;
            }
            for c in &samples {
                cells.entry(*c).or_insert((prio, li));
            }
            if run.is_empty() {
                run.push(a);
            }
            run.push(b);
        }
        flush(&mut run, &mut out);
    }
    out
}

fn line_lonlat(scene: &Scene, p: (f32, f32)) -> (f32, f32) {
    scene.projection.to_lonlat(p.0, p.1)
}

/// Edge of a road tile in kilometres. Roads are grouped by tile so a frame draws only the
/// tiles in view.
const ROAD_TILE_KM: f32 = 80.0;

/// A tile of road ribbons: its ground bounding box and its slice of the index list.
pub struct RoadTile {
    pub min: [f32; 2],
    pub max: [f32; 2],
    pub first: u32,
    pub count: u32,
}

/// The road ribbons of one zoom band, ordered by tile.
pub struct RoadMesh {
    pub mesh: Mesh,
    pub tiles: Vec<RoadTile>,
}

/// Roads and railways of a zoom band as one ribbon quad per unique segment. Each vertex
/// carries its own end, the far end and a side; the shader extrudes by a width in screen
/// pixels, so lines stay crisp at any zoom. Classes are drawn by priority through the depth
/// the shader writes.
pub fn roads(scene: &Scene, band: usize) -> RoadMesh {
    type Seg = (u8, [f32; 3], f32, [f32; 3], f32);
    let mut by_tile: BTreeMap<(i32, i32), Vec<Seg>> = BTreeMap::new();
    for (class, pts) in road_chains_for_band(scene, &ROAD_BANDS[band]) {
        for w in pts.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if (a.xyz[0] - b.xyz[0]).abs() + (a.xyz[1] - b.xyz[1]).abs() < 1e-5 {
                continue;
            }
            let key = (
                (((a.xyz[0] + b.xyz[0]) * 0.5) / ROAD_TILE_KM).floor() as i32,
                (((a.xyz[1] + b.xyz[1]) * 0.5) / ROAD_TILE_KM).floor() as i32,
            );
            by_tile
                .entry(key)
                .or_default()
                .push((class, a.xyz, a.along, b.xyz, b.along));
        }
    }
    let mut m = Mesh::default();
    let mut tiles = Vec::new();
    for segs in by_tile.values() {
        let first = m.indices.len() as u32;
        let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
        for (class, a, a_along, b, b_along) in segs {
            for p in [a, b] {
                for k in 0..2 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            let base = m.vertices.len() as u32;
            let colour = [*class, 0, 0, KIND_ROAD];
            // Corners: (this end, other end, side, along at this end).
            for (this, other, side, end, along, other_along) in [
                (a, b, -1.0f32, 0.0f32, *a_along, *b_along),
                (a, b, 1.0, 0.0, *a_along, *b_along),
                (b, a, 1.0, 1.0, *b_along, *a_along),
                (b, a, -1.0, 1.0, *b_along, *a_along),
            ] {
                m.vertices.push(Vertex {
                    pos: *this,
                    normal: *other,
                    colour,
                    local: [side, end, along],
                    uv: [other_along, 0.0],
                });
            }
            m.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        tiles.push(RoadTile {
            min: lo,
            max: hi,
            first,
            count: m.indices.len() as u32 - first,
        });
    }
    RoadMesh { mesh: m, tiles }
}

/// How far the horizon extends past the terrain, in degrees (east and west; north and south
/// use `APRON_REACH_NS_DEG`), how fast its cells grow with distance, and the biggest cell.
const APRON_REACH_DEG: f32 = 90.0;
const APRON_REACH_NS_DEG: f32 = 60.0;
const APRON_GROWTH: f32 = 1.2;
const APRON_MAX_CELL_DEG: f32 = 5.0;

/// Breakpoints along one axis, ascending: the terrain's own cell edges from `a` to `b` (`n`
/// cells), and outside them cells that start one terrain cell wide and grow by a fifth each
/// step, so detail steps down crisply with distance and the far horizon costs few cells.
fn apron_axis(a: f32, b: f32, n: usize, reach: f32) -> Vec<f32> {
    let cell = (b - a) / n as f32;
    let outward = |from: f32, sign: f32| {
        let mut out = Vec::new();
        let (mut x, mut step) = (from, cell);
        while (x - from).abs() < reach {
            x += sign * step;
            out.push(x);
            step = (step * APRON_GROWTH).min(APRON_MAX_CELL_DEG);
        }
        out
    };
    let mut t: Vec<f32> = outward(a, -1.0);
    t.reverse();
    t.push(a);
    t.extend((1..n).map(|k| a + cell * k as f32));
    t.push(b);
    t.extend(outward(b, 1.0));
    t
}

/// The land class the satellite picture shows at a point (no water test), with the season's
/// snow on top.
fn land_class(scene: &Scene, lon: f32, lat: f32) -> u8 {
    let e = &scene.earth;
    let u = ((lon - e.bounds.west) / (e.bounds.east - e.bounds.west) * e.w as f32)
        .clamp(0.0, e.w as f32 - 1.0) as usize;
    let v = ((e.bounds.north - lat) / (e.bounds.north - e.bounds.south) * e.h as f32)
        .clamp(0.0, e.h as f32 - 1.0) as usize;
    let p = (v * e.w as usize + u) * 4;
    let (r, g, b) = (e.rgba[p] as f32, e.rgba[p + 1] as f32, e.rgba[p + 2] as f32);
    if b > g * 1.05 && b > r * 1.25 {
        return APRON_WATER;
    }
    if crate::ground::snow_cover(scene.week, lat, lon, 0.0) > 0.2 {
        crate::ground::G_SNOW
    } else {
        crate::ground::classify([e.rgba[p], e.rgba[p + 1], e.rgba[p + 2]])
    }
}

/// What the picture shows over a cell of the apron: the most common class among a grid of
/// samples (water counts as a class), so a big far cell shows what most of it is.
fn apron_class(scene: &Scene, lon: [f32; 2], lat: [f32; 2], samples: usize) -> u8 {
    let mut votes = [0u32; 8];
    for j in 0..samples {
        for i in 0..samples {
            let f = |k: usize| (k as f32 + 0.5) / samples as f32;
            let class = land_class(
                scene,
                lon[0] + (lon[1] - lon[0]) * f(i),
                lat[0] + (lat[1] - lat[0]) * f(j),
            );
            votes[class as usize & 7] += 1;
        }
    }
    (0..8u8)
        .max_by_key(|&c| (votes[c as usize], c != APRON_WATER))
        .unwrap_or(0)
}

/// Class and level of every apron cell (`None` inside the terrain), with the cell edges.
struct ApronGrid {
    xs: Vec<f32>,
    ys: Vec<f32>,
    cells: Vec<Option<(u8, i32)>>,
}

fn apron_grid(scene: &Scene) -> ApronGrid {
    let hm = &scene.heightmap;
    let hb = hm.bounds;
    let (dx, dy) = hm.cell_degrees();
    let xs = apron_axis(hb.west, hb.east, hm.w, APRON_REACH_DEG);
    let ys = apron_axis(hb.south, hb.north, hm.h, APRON_REACH_NS_DEG);
    let sea = scene.levels.iter().copied().min().unwrap_or(0);
    // Land levels summed over every window of the terrain (summed area tables).
    let (w, h) = (hm.w, hm.h);
    let mut water = vec![false; w * h];
    let mut sum = vec![0.0f64; (w + 1) * (h + 1)];
    let mut cnt = vec![0i64; (w + 1) * (h + 1)];
    for r in 0..h {
        for c in 0..w {
            let lon = hb.west + (c as f32 + 0.5) * dx;
            let lat = hb.north - (r as f32 + 0.5) * dy;
            let kind = scene.mask.at(lon, lat)[2];
            let wet = kind == crate::data::WATER_OCEAN || kind == WATER_LAKE;
            water[r * w + c] = wet;
            let (l, n) = if wet {
                (0.0, 0i64)
            } else {
                (scene.levels[r * w + c] as f64, 1i64)
            };
            let at = |rr: usize, cc: usize| rr * (w + 1) + cc;
            sum[at(r + 1, c + 1)] = l + sum[at(r, c + 1)] + sum[at(r + 1, c)] - sum[at(r, c)];
            cnt[at(r + 1, c + 1)] = n + cnt[at(r, c + 1)] + cnt[at(r + 1, c)] - cnt[at(r, c)];
        }
    }
    let window = |c0: usize, c1: usize, r0: usize, r1: usize| -> Option<i32> {
        let at = |rr: usize, cc: usize| rr * (w + 1) + cc;
        let s =
            sum[at(r1 + 1, c1 + 1)] - sum[at(r0, c1 + 1)] - sum[at(r1 + 1, c0)] + sum[at(r0, c0)];
        let n =
            cnt[at(r1 + 1, c1 + 1)] - cnt[at(r0, c1 + 1)] - cnt[at(r1 + 1, c0)] + cnt[at(r0, c0)];
        (n > 0).then(|| (s / n as f64).round() as i32)
    };
    let inside =
        |lon: f32, lat: f32| lon > hb.west && lon < hb.east && lat > hb.south && lat < hb.north;
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);
    let mut cells: Vec<Option<(u8, i32)>> = vec![None; nx * ny];
    for j in 0..ny {
        for i in 0..nx {
            let (lon, lat) = ((xs[i] + xs[i + 1]) / 2.0, (ys[j] + ys[j + 1]) / 2.0);
            if inside(lon, lat) {
                continue;
            }
            // The terrain cell nearest this one, and how many terrain cells away it lies.
            let (px, py) = (lon.clamp(hb.west, hb.east), lat.clamp(hb.south, hb.north));
            let (c, r) = hm.cell_of(
                px.clamp(hb.west + dx * 0.5, hb.east - dx * 0.5),
                py.clamp(hb.south + dy * 0.5, hb.north - dy * 0.5),
            );
            let away = ((lon - px).abs() / dx).max((lat - py).abs() / dy);
            let own = r * w + c;
            if away < 1.1 {
                let class = if water[own] {
                    APRON_WATER
                } else {
                    match land_class(scene, px, py) {
                        APRON_WATER => crate::ground::G_FIELD,
                        class => class,
                    }
                };
                cells[j * nx + i] = Some((class, scene.levels[own]));
                continue;
            }
            // The picture is read over a patch about as wide as the cell is far from the
            // terrain, so a strip beside the edge settles into blocks rather than streaks.
            let (hx, hy) = (
                ((xs[i + 1] - xs[i]) / 2.0).max(0.35 * (away - 1.1) * dx),
                ((ys[j + 1] - ys[j]) / 2.0).max(0.35 * (away - 1.1) * dy),
            );
            let wide = hx > dx * 1.5 || hy > dy * 1.5;
            let class = apron_class(
                scene,
                [lon - hx, lon + hx],
                [lat - hy, lat + hy],
                if wide { 5 } else { 1 },
            );
            let level = if class == APRON_WATER {
                sea
            } else {
                let reach = (away - 1.1) * 0.8;
                let (rc, rr) = (reach.ceil() as usize, reach.ceil() as usize);
                window(
                    c.saturating_sub(rc),
                    (c + rc).min(w - 1),
                    r.saturating_sub(rr),
                    (r + rr).min(h - 1),
                )
                .unwrap_or(scene.levels[own])
            };
            cells[j * nx + i] = Some((class, level));
        }
    }
    ApronGrid { xs, ys, cells }
}

/// The ring of land and sea beyond the terrain's edge, so a tilted view ends in a horizon of
/// land and sea rather than a hard board edge. It is the terrain's own look at a coarser grain:
/// the same land classes, the same terrace look, and cells that begin one terrain cell wide and
/// grow with distance. The first ring copies the terrain's edge cells exactly (class, water and
/// level), so there is no wall at the seam. Further out the level is the mean of the terrain's
/// land over a window that widens with distance, so the far land settles to one flat level and
/// the horizon is a clean line.
pub fn apron(scene: &Scene) -> Mesh {
    let ApronGrid { xs, ys, cells } = apron_grid(scene);
    let (nx, ny) = (xs.len() - 1, ys.len() - 1);
    let mut m = Mesh::default();
    let km = |lon: f32, lat: f32| scene.projection.to_km(lon, lat);
    let top = |class: u8| [class, 0, 0, KIND_APRON_TOP];
    let wall_c = |class: u8| [class, 0, 0, KIND_APRON_WALL];
    // A sloped wall along the segment (a to b) in km, from the high level down to the low one,
    // leaning toward the low side `dir`.
    let wall = |m: &mut Mesh,
                a: (f32, f32),
                b: (f32, f32),
                dir: (f32, f32),
                lo: i32,
                hi: i32,
                class: u8| {
        let (za, zb) = (level_z_km(scene, lo), level_z_km(scene, hi));
        let run = slope_run(scene, zb - za);
        let n = slope_normal([dir.0, dir.1], zb - za, run);
        m.quad(
            [
                [a.0 + dir.0 * run, a.1 + dir.1 * run, za],
                [b.0 + dir.0 * run, b.1 + dir.1 * run, za],
                [b.0, b.1, zb],
                [a.0, a.1, zb],
            ],
            n,
            wall_c(class),
            [[0.0; 3]; 4],
        );
    };
    // Tops: cells of one class and level merge into rectangles, like the terrain's.
    let mut done = vec![false; nx * ny];
    for j in 0..ny {
        for i in 0..nx {
            let Some(v) = cells[j * nx + i] else { continue };
            if done[j * nx + i] {
                continue;
            }
            let mut i1 = i + 1;
            while i1 < nx && !done[j * nx + i1] && cells[j * nx + i1] == Some(v) {
                i1 += 1;
            }
            let mut j1 = j + 1;
            'grow: while j1 < ny {
                for k in i..i1 {
                    if done[j1 * nx + k] || cells[j1 * nx + k] != Some(v) {
                        break 'grow;
                    }
                }
                j1 += 1;
            }
            for jj in j..j1 {
                for ii in i..i1 {
                    done[jj * nx + ii] = true;
                }
            }
            let (x0, y0) = km(xs[i], ys[j]);
            let (x1, y1) = km(xs[i1], ys[j1]);
            let z = level_z_km(scene, v.1);
            m.quad(
                [[x0, y0, z], [x1, y0, z], [x1, y1, z], [x0, y1, z]],
                [0.0, 0.0, 1.0],
                top(v.0),
                [[0.0; 3]; 4],
            );
        }
    }
    // Walls between neighbouring cells of different levels (never at the terrain's own edge,
    // whose first ring matches it).
    for j in 0..ny {
        for i in 0..nx {
            let Some((class, level)) = cells[j * nx + i] else {
                continue;
            };
            let (x0, y0) = km(xs[i], ys[j]);
            let (x1, y1) = km(xs[i + 1], ys[j + 1]);
            if i + 1 < nx
                && let Some((c2, l2)) = cells[j * nx + i + 1]
                && l2 != level
            {
                let (lo, hi, dir, c) = if level > l2 {
                    (l2, level, (1.0, 0.0), class)
                } else {
                    (level, l2, (-1.0, 0.0), c2)
                };
                wall(&mut m, (x1, y0), (x1, y1), dir, lo, hi, c);
            }
            if j + 1 < ny
                && let Some((c2, l2)) = cells[(j + 1) * nx + i]
                && l2 != level
            {
                let (lo, hi, dir, c) = if level > l2 {
                    (l2, level, (0.0, 1.0), class)
                } else {
                    (level, l2, (0.0, -1.0), c2)
                };
                wall(&mut m, (x0, y1), (x1, y1), dir, lo, hi, c);
            }
        }
    }
    m
}

/// West, south, east and north extent of the horizon apron in kilometres, for tests.
pub fn apron_extent_km(scene: &Scene) -> [f32; 4] {
    let m = apron(scene);
    let mut e = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
    for v in &m.vertices {
        e = [
            e[0].min(v.pos[0]),
            e[1].min(v.pos[1]),
            e[2].max(v.pos[0]),
            e[3].max(v.pos[1]),
        ];
    }
    e
}

/// Height the camera looks at: the terrain's mean height round the target, in kilometres. The
/// terrain is drawn exaggerated, so a close view over a plateau would otherwise sit inside it.
/// Averaged over the view (not read at one point) so a pan does not jerk at each terrace.
pub fn camera_target_z(scene: &Scene, x: f32, y: f32, width_km: f32) -> f32 {
    let r = width_km * 0.2;
    let mut sum = 0.0;
    for (ox, oy) in [
        (0.0, 0.0),
        (-r, 0.0),
        (r, 0.0),
        (0.0, -r),
        (0.0, r),
        (-r, -r),
        (r, -r),
        (-r, r),
        (r, r),
    ] {
        sum += ground_z(scene, x + ox, y + oy);
    }
    sum / 9.0
}

/// Number of apron cells touching the terrain whose level differs from the terrain cell they
/// touch. Zero means there is no wall at the seam.
pub fn apron_seam_steps(scene: &Scene) -> usize {
    let hm = &scene.heightmap;
    let hb = hm.bounds;
    let (dx, dy) = hm.cell_degrees();
    let g = apron_grid(scene);
    let (nx, ny) = (g.xs.len() - 1, g.ys.len() - 1);
    let mut bad = 0;
    for j in 0..ny {
        for i in 0..nx {
            let Some((_, level)) = g.cells[j * nx + i] else {
                continue;
            };
            let (lon, lat) = ((g.xs[i] + g.xs[i + 1]) / 2.0, (g.ys[j] + g.ys[j + 1]) / 2.0);
            let (px, py) = (lon.clamp(hb.west, hb.east), lat.clamp(hb.south, hb.north));
            let away = ((lon - px).abs() / dx).max((lat - py).abs() / dy);
            if away >= 1.1 {
                continue;
            }
            let (c, r) = hm.cell_of(
                px.clamp(hb.west + dx * 0.5, hb.east - dx * 0.5),
                py.clamp(hb.south + dy * 0.5, hb.north - dy * 0.5),
            );
            if scene.level_at(c, r) != level {
                bad += 1;
            }
        }
    }
    bad
}
