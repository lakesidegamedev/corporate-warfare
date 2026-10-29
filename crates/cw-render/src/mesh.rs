//! Geometry: stepped terrace terrain, voxel buildings and the cloud sheet.

use bytemuck::{Pod, Zeroable};

use crate::data::{Scene, VoxelModel, WATER_LAKE};

/// Vertex kinds, carried in the colour alpha byte.
pub const KIND_TOP: u8 = 0;
pub const KIND_WALL: u8 = 1;
pub const KIND_BUILDING: u8 = 2;
pub const KIND_PLAIN: u8 = 3;
pub const KIND_CLOUD: u8 = 4;

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
    let boxed = |src: &[f32]| -> Vec<f32> {
        let mut tmp = vec![0.0f32; w * h];
        for r in 0..h {
            for c in 0..w {
                let (lo, hi) = (
                    c.saturating_sub(SMOOTH_RADIUS),
                    (c + SMOOTH_RADIUS).min(w - 1),
                );
                tmp[r * w + c] =
                    (lo..=hi).map(|k| src[r * w + k]).sum::<f32>() / (hi - lo + 1) as f32;
            }
        }
        let mut out = vec![0.0f32; w * h];
        for r in 0..h {
            let (lo, hi) = (
                r.saturating_sub(SMOOTH_RADIUS),
                (r + SMOOTH_RADIUS).min(h - 1),
            );
            for c in 0..w {
                out[r * w + c] =
                    (lo..=hi).map(|k| tmp[k * w + c]).sum::<f32>() / (hi - lo + 1) as f32;
            }
        }
        out
    };
    let mut smooth = boxed(&boxed(&value));
    for i in 0..w * h {
        if water[i] {
            smooth[i] = value[i];
        }
    }
    let mut level: Vec<i32> = smooth
        .iter()
        .map(|m| ((m + scene.terrace_offset_m) / scene.terrace_m).floor() as i32)
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
        let (hi, lo) = ground_range(scene, model, cx, cy);
        let (z_top, z_low) = (level_z_km(scene, hi), level_z_km(scene, lo));
        let (s, c) = p.yaw.to_radians().sin_cos();
        let v = scene.voxel_km;
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
    }
    m
}

/// Highest and lowest terrace level under a building's footprint.
fn ground_range(scene: &Scene, model: &VoxelModel, cx: f32, cy: f32) -> (i32, i32) {
    let hm = &scene.heightmap;
    let r = model.footprint[0].max(model.footprint[1]) * scene.voxel_km / 2.0;
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
