//! Countryside props: trees, farmsteads and suburban houses as real 3D boxes standing on the
//! ground classes, in the close-zoom band. Placement is world fixed and deterministic: a hash of
//! the lattice cell decides every prop, so the same spot always gets the same prop and nothing
//! slides when the camera moves. The map is cut into square chunks; a chunk's mesh depends only
//! on the scene (never the camera), so the renderer builds each one once and keeps it.
//!
//! Zoom steps the density, never the opacity: a wider view uses every n-th lattice cell (a
//! subset of the closer view, so props never move) and draws each prop a little bigger so it
//! keeps a minimum on-screen size. Beyond [`MAX_WIDTH_KM`] there are no props.

use std::collections::HashMap;

use crate::data::{
    LINE_INTERSTATE, LINE_MAIN, LINE_RAIL, LINE_RIVER, Scene, WATER_LAKE, WATER_OCEAN,
};
use crate::ground::{
    G_CORE, G_FIELD, G_FOREST, G_ROCK, G_SNOW, G_SOIL, G_URBAN, ZONE_INDUSTRIAL, ZONE_OUTER,
};
use crate::mesh::{self, KIND_PROP, Mesh, Vertex};
use crate::palette::{Palette, Rgb, ramp};

/// Props are drawn only when the camera sees at most this many kilometres across.
pub const MAX_WIDTH_KM: f32 = 60.0;
/// Hard limit on the props one chunk may hold (a safety net; real chunks stay far below it).
pub const CHUNK_MAX_PROPS: usize = 1400;
/// Edge of a chunk in kilometres at stride 1; wider bands use proportionally bigger chunks so a
/// chunk always holds about the same number of lattice cells.
const CHUNK_KM: f32 = 4.0;
/// Lattice of the trees, and of the houses in the outer residential ground.
const TREE_CELL_KM: f32 = 0.26;
/// Trees are drawn this much bigger than the models are built, so a stand of them reads at close zoom.
const TREE_SCALE: f32 = 1.5;
/// Farmsteads: one chance per cell of this size.
const FARM_CELL_KM: f32 = 3.0;
/// The city's street lattice: one cell is 31.25 m, the cell the ground shader lays its streets and
/// blocks out in (`city_texel` in scene.wgsl), so a city building stands on the block it is drawn on.
const CELL_KM: f32 = 0.03125;
/// City buildings stand on lots of this many cells a side, one building (or none) a lot.
const LOT_CELLS: i32 = 3;
/// City buildings show only when the camera sees at most this many kilometres across; further
/// out they thin by the band's stride (fewer of them, never smaller or softer) and then stop.
pub const CITY_MAX_WIDTH_KM: f32 = 36.0;
/// Hard limit on the city buildings one chunk may hold.
pub const CHUNK_MAX_CITY: usize = 2600;
/// Clear ground kept round a road, a river and a building, in km.
const ROAD_CLEAR_KM: f32 = 0.1;
const RIVER_CLEAR_KM: f32 = 0.13;
const BUILDING_CLEAR_KM: f32 = 0.12;

/// Density step of a camera width: 1 is every lattice cell, 2 every second one and so on.
pub fn stride_for(width_km: f32) -> Option<u8> {
    if width_km > MAX_WIDTH_KM || width_km.is_nan() {
        return None;
    }
    Some(((width_km / 12.0).ceil() as u8).clamp(1, 5))
}

/// Drawing scale of a stride: sparser bands draw their props bigger for a minimum on-screen size.
fn scale_for(stride: u8) -> f32 {
    1.0 + 0.3 * (stride as f32 - 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChunkKey {
    pub ix: i32,
    pub iy: i32,
    pub stride: u8,
}

fn chunk_km(stride: u8) -> f32 {
    CHUNK_KM * stride as f32
}

/// The chunks a camera sees: a disc of 0.85 of the width round the target (a turned, tilted
/// view always fits inside it), as chunk keys at the width's stride.
pub fn visible_chunks(target: (f32, f32), width_km: f32) -> Vec<ChunkKey> {
    let Some(stride) = stride_for(width_km) else {
        return Vec::new();
    };
    chunks_around(target, 0.85 * width_km, stride)
}

/// The chunks of a band (`stride`) within `reach` km of the target: the renderer uses it to build
/// the chunks just past the view and those of the neighbouring bands before the camera gets there.
pub fn chunks_around(target: (f32, f32), reach: f32, stride: u8) -> Vec<ChunkKey> {
    let ck = chunk_km(stride);
    let (x0, x1) = (
        ((target.0 - reach) / ck).floor() as i32,
        ((target.0 + reach) / ck).floor() as i32,
    );
    let (y0, y1) = (
        ((target.1 - reach) / ck).floor() as i32,
        ((target.1 + reach) / ck).floor() as i32,
    );
    let mut out = Vec::new();
    for iy in y0..=y1 {
        for ix in x0..=x1 {
            // Nearest point of the chunk to the target must be inside the disc.
            let nx = target.0.clamp(ix as f32 * ck, (ix + 1) as f32 * ck);
            let ny = target.1.clamp(iy as f32 * ck, (iy + 1) as f32 * ck);
            if (nx - target.0).hypot(ny - target.1) <= reach {
                out.push(ChunkKey { ix, iy, stride });
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// What must stay clear: roads, rails, rivers and the placed buildings.

#[derive(Clone, Copy)]
enum Obstacle {
    Road([f32; 2], [f32; 2]),
    River([f32; 2], [f32; 2]),
    /// Centre and half side of a building's footprint.
    Lot([f32; 2], f32),
}

const INDEX_CELL_KM: f32 = 2.0;

/// The scene's roads, rivers and building footprints in a coarse hash, so a prop can ask what is
/// near it. Built once per scene and rebuilt when the buildings change.
pub struct PropIndex {
    cells: HashMap<(i32, i32), Vec<Obstacle>>,
}

fn cell_of(x: f32, y: f32) -> (i32, i32) {
    (
        (x / INDEX_CELL_KM).floor() as i32,
        (y / INDEX_CELL_KM).floor() as i32,
    )
}

fn seg_dist(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - (a[0] + t * dx)).hypot(p[1] - (a[1] + t * dy))
}

impl PropIndex {
    pub fn build(scene: &Scene) -> PropIndex {
        let mut cells: HashMap<(i32, i32), Vec<Obstacle>> = HashMap::new();
        let add = |cells: &mut HashMap<(i32, i32), Vec<Obstacle>>,
                   o: Obstacle,
                   x: f32,
                   y: f32,
                   reach: f32| {
            let (c0, c1) = (cell_of(x - reach, y - reach), cell_of(x + reach, y + reach));
            for cy in c0.1..=c1.1 {
                for cx in c0.0..=c1.0 {
                    cells.entry((cx, cy)).or_default().push(o);
                }
            }
        };
        for line in &scene.lines {
            let km: Vec<[f32; 2]> = line
                .points
                .iter()
                .map(|&(lon, lat)| {
                    let (x, y) = scene.projection.to_km(lon, lat);
                    [x, y]
                })
                .collect();
            for w in km.windows(2) {
                let o = match line.class {
                    LINE_RIVER => Obstacle::River(w[0], w[1]),
                    LINE_INTERSTATE | LINE_MAIN | LINE_RAIL => Obstacle::Road(w[0], w[1]),
                    _ => continue,
                };
                // A segment goes into every cell its stretch passes through.
                let len = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
                let steps = (len / (INDEX_CELL_KM * 0.5)).ceil().max(1.0) as usize;
                let mut seen: Vec<(i32, i32)> = Vec::new();
                for k in 0..=steps {
                    let t = k as f32 / steps as f32;
                    let (x, y) = (
                        w[0][0] + (w[1][0] - w[0][0]) * t,
                        w[0][1] + (w[1][1] - w[0][1]) * t,
                    );
                    let (c0, c1) = (cell_of(x - 0.5, y - 0.5), cell_of(x + 0.5, y + 0.5));
                    for cy in c0.1..=c1.1 {
                        for cx in c0.0..=c1.0 {
                            if !seen.contains(&(cx, cy)) {
                                seen.push((cx, cy));
                                cells.entry((cx, cy)).or_default().push(o);
                            }
                        }
                    }
                }
            }
        }
        for p in &scene.buildings {
            let Some(model) = scene.models.get(&p.model) else {
                continue;
            };
            let (x, y) = scene.projection.to_km(p.lon, p.lat);
            let half = model.footprint[0].max(model.footprint[1]) * scene.voxel_km * p.scale / 2.0;
            add(&mut cells, Obstacle::Lot([x, y], half), x, y, half + 0.3);
        }
        PropIndex { cells }
    }

    /// Distance to the nearest road or rail, to the nearest river (both capped at 2 km), and
    /// whether the point is inside a building's footprint plus `margin`.
    fn near(&self, x: f32, y: f32) -> (f32, f32, bool) {
        let (mut road, mut river, mut lot) = (2.0f32, 2.0f32, false);
        let c = cell_of(x, y);
        for cy in c.1 - 1..=c.1 + 1 {
            for cx in c.0 - 1..=c.0 + 1 {
                let Some(list) = self.cells.get(&(cx, cy)) else {
                    continue;
                };
                for o in list {
                    match *o {
                        Obstacle::Road(a, b) => road = road.min(seg_dist([x, y], a, b)),
                        Obstacle::River(a, b) => river = river.min(seg_dist([x, y], a, b)),
                        Obstacle::Lot(m, half) => {
                            let r = half + BUILDING_CLEAR_KM;
                            if (x - m[0]).abs() < r && (y - m[1]).abs() < r {
                                lot = true;
                            }
                        }
                    }
                }
            }
        }
        (road, river, lot)
    }
}

// ---------------------------------------------------------------------------------------------
// Hash and ground lookups.

fn hash(a: i32, b: i32, salt: u32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1)
        ^ (b as u32).wrapping_mul(0x85EB_CA6B)
        ^ salt.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

/// Uniform 0 to 1 from a hash.
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

struct Ground {
    base: u8,
    cls: u8,
    zone: u8,
    elev_m: f32,
    lat: f32,
}

fn ground_at(scene: &Scene, x: f32, y: f32) -> Option<Ground> {
    let g = &scene.ground;
    let (lon, lat) = scene.projection.to_lonlat(x, y);
    let (fx, fy) = (
        (lon - g.bounds.west) * g.ppd,
        (g.bounds.north - lat) * g.ppd,
    );
    if fx < 0.0 || fy < 0.0 || fx >= g.w as f32 || fy >= g.h as f32 {
        return None;
    }
    let i = fy as usize * g.w + fx as usize;
    Some(Ground {
        base: g.base[i],
        cls: g.classes[i],
        zone: g.variant[i] & 3,
        elev_m: g.elev[i] as f32,
        lat,
    })
}

fn is_water(scene: &Scene, x: f32, y: f32) -> bool {
    let (lon, lat) = scene.projection.to_lonlat(x, y);
    let k = scene.mask.at(lon, lat)[2];
    if k == WATER_OCEAN || k == WATER_LAKE {
        return true;
    }
    // A city's outline can cover a bay: the mask says urban but the ground is at or under sea
    // level, and the terrain draws it as water.
    let (c, r) = scene.heightmap.cell_of(lon, lat);
    scene.heightmap.at(c, r) <= 0
}

/// Land all round a footprint of half side `r`, with the ground level everywhere under it
/// (within a hair). Returns the base height, or None when the spot is wet or on a terrace step.
fn standing_z(scene: &Scene, x: f32, y: f32, r: f32) -> Option<(f32, f32)> {
    let mut lowest = f32::MAX;
    let centre = mesh::ground_z(scene, x, y);
    for (ox, oy) in [(0.0, 0.0), (-r, -r), (r, -r), (r, r), (-r, r)] {
        if is_water(scene, x + ox, y + oy) {
            return None;
        }
        lowest = lowest.min(mesh::ground_z(scene, x + ox, y + oy));
    }
    // A prop on a step would hang in the air on one side: allow a small drop (the trunk or the
    // footing covers it), refuse a real wall.
    if centre - lowest > 0.03 {
        return None;
    }
    Some((centre, lowest))
}

// ---------------------------------------------------------------------------------------------
// The city block pattern, ported from the ground shader so houses stand on blocks and trees on
// the texture's own tree texels.

fn ihash(a: i32, b: i32) -> u32 {
    let mut h = (a as u32).wrapping_mul(0x9E37_79B1) ^ (b as u32).wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^ (h >> 15)
}

fn imod(a: i32, m: i32) -> i32 {
    ((a % m) + m) % m
}

fn axis_street(
    t: i32,
    band: i32,
    salt: i32,
    span: i32,
    lo: i32,
    rng: u32,
    width: i32,
) -> (bool, i32) {
    let k = (t - imod(t, span)) / span;
    let off = k * span + lo + (ihash(k + salt, band) % rng) as i32;
    let on = t >= off && t < off + width;
    (on, if t >= off { k } else { k - 1 })
}

/// (tone, green) of a city cell at close zoom: the ground shader's `city_texel` with its avenues
/// two cells wide. Tone below 0 is an avenue or a street, 0 or 1 a block; green 0 none, 1 lawn,
/// 2 a tree, 3 the dark lawn of a park. `core` is downtown ground, `zone` the city zone.
fn city_cell(core: bool, zone: u8, tx: i32, ty: i32) -> (i32, i32) {
    let (span, lo, rng, av) = if core {
        (6, 2, 2u32, 24)
    } else if zone == ZONE_OUTER {
        (12, 3, 6, 48)
    } else if zone == ZONE_INDUSTRIAL {
        (20, 4, 10, 64)
    } else {
        (8, 2, 4, 32)
    };
    let avx = imod(tx - (ihash(tx >> 6, 91) % 5) as i32, av);
    let avy = imod(ty - (ihash(ty >> 6, 92) % 5) as i32, av);
    if avx < 2 || avy < 2 {
        return (-2, 0);
    }
    let sx = axis_street(tx, ty >> 6, 11, span, lo, rng, 1);
    let sy = axis_street(ty, tx >> 6, 23, span, lo, rng, 1);
    if sx.0 || sy.0 {
        return (-1, 0);
    }
    let bh = ihash(sx.1 * 7 + 3, sy.1 * 13 + 5) % 100;
    let th = ihash(tx + 17, ty + 29) % 100;
    if core {
        return (i32::from(bh < 72), 0);
    }
    if zone == ZONE_OUTER {
        if bh < 30 {
            return (0, if th < 14 { 2 } else { 1 });
        }
        if bh < 38 {
            return (1, 0);
        }
        if th < 12 {
            return (0, 1);
        }
        if th < 16 {
            return (0, 2);
        }
        return (0, 0);
    }
    if zone == ZONE_INDUSTRIAL {
        return (i32::from(bh < 22), 0);
    }
    if bh < 7 {
        return (0, if th < 10 { 2 } else { 3 });
    }
    (i32::from(bh < 30), 0)
}

// ---------------------------------------------------------------------------------------------
// Colours and seasons.

fn pc(r: usize, s: usize) -> Rgb {
    Palette::day().ramps[r][s]
}

/// What the broadleaf trees of a latitude look like in a week of the year.
#[derive(Clone, Copy, PartialEq)]
enum Leaves {
    Green,
    /// Some trees have turned: the share of turned trees, 0 to 1.
    Turning(f32),
    Autumn,
    Bare,
}

fn leaves(week: u32, lat: f32) -> Leaves {
    let a = lat.abs();
    let s = if lat >= 0.0 { week } else { (week + 26) % 52 } as i32;
    if a < 28.0 {
        return Leaves::Green;
    }
    // Further north the cold comes earlier and stays longer, by up to four weeks.
    let shift = ((45.0 - a) / 15.0).clamp(0.0, 1.0) * 4.0;
    let s = s as f32;
    let bare_from = 46.0 + shift;
    let bare_to = 12.0 - shift;
    if s >= bare_from || s <= bare_to {
        Leaves::Bare
    } else if s >= 40.0 + shift * 0.5 {
        Leaves::Autumn
    } else if s >= 35.0 + shift * 0.5 {
        Leaves::Turning((s - 35.0 - shift * 0.5) / 5.0)
    } else {
        Leaves::Green
    }
}

// ---------------------------------------------------------------------------------------------
// Box builder: top and four sides only (nothing sees a bottom), turned by a yaw.

struct Build<'a> {
    m: &'a mut Mesh,
    at: [f32; 3],
    s: f32,
    sn: f32,
    cs: f32,
}

impl<'a> Build<'a> {
    fn new(m: &'a mut Mesh, at: [f32; 3], scale: f32, yaw_deg: f32) -> Build<'a> {
        let (sn, cs) = yaw_deg.to_radians().sin_cos();
        Build {
            m,
            at,
            s: scale,
            sn,
            cs,
        }
    }

    fn p(&self, q: [f32; 3]) -> [f32; 3] {
        let (x, y, z) = (q[0] * self.s, q[1] * self.s, q[2] * self.s);
        [
            self.at[0] + x * self.cs - y * self.sn,
            self.at[1] + x * self.sn + y * self.cs,
            self.at[2] + z,
        ]
    }

    fn n(&self, n: [f32; 3]) -> [f32; 3] {
        [
            n[0] * self.cs - n[1] * self.sn,
            n[0] * self.sn + n[1] * self.cs,
            n[2],
        ]
    }

    fn quad(&mut self, c: [[f32; 3]; 4], n: [f32; 3], rgb: Rgb) {
        const UV: [[f32; 2]; 4] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let base = self.m.vertices.len() as u32;
        let normal = self.n(n);
        for i in 0..4 {
            self.m.vertices.push(Vertex {
                pos: self.p(c[i]),
                normal,
                colour: [rgb[0], rgb[1], rgb[2], KIND_PROP],
                local: [0.0; 3],
                uv: UV[i],
                anchor: [0.0; 4],
            });
        }
        self.m
            .indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A box from `a` to `b` (km, before the scale), in a frame whose x runs ahead.
    fn put(&mut self, a: [f32; 3], b: [f32; 3], rgb: Rgb) {
        self.quad(
            [
                [a[0], a[1], b[2]],
                [b[0], a[1], b[2]],
                [b[0], b[1], b[2]],
                [a[0], b[1], b[2]],
            ],
            [0.0, 0.0, 1.0],
            rgb,
        );
        self.quad(
            [
                [a[0], a[1], a[2]],
                [b[0], a[1], a[2]],
                [b[0], a[1], b[2]],
                [a[0], a[1], b[2]],
            ],
            [0.0, -1.0, 0.0],
            rgb,
        );
        self.quad(
            [
                [b[0], b[1], a[2]],
                [a[0], b[1], a[2]],
                [a[0], b[1], b[2]],
                [b[0], b[1], b[2]],
            ],
            [0.0, 1.0, 0.0],
            rgb,
        );
        self.quad(
            [
                [b[0], a[1], a[2]],
                [b[0], b[1], a[2]],
                [b[0], b[1], b[2]],
                [b[0], a[1], b[2]],
            ],
            [1.0, 0.0, 0.0],
            rgb,
        );
        self.quad(
            [
                [a[0], b[1], a[2]],
                [a[0], a[1], a[2]],
                [a[0], a[1], b[2]],
                [a[0], b[1], b[2]],
            ],
            [-1.0, 0.0, 0.0],
            rgb,
        );
    }

    /// A square block centred on the origin: half side `h`, from `z0` to `z1`.
    fn block(&mut self, h: f32, z0: f32, z1: f32, rgb: Rgb) {
        self.put([-h, -h, z0], [h, h, z1], rgb);
    }
}

// ---------------------------------------------------------------------------------------------
// The props.

/// Trunk colour, and the trunk's drop to the lowest ground under the prop.
fn trunk(b: &mut Build, drop: f32, top: f32) {
    b.block(0.008, -drop, top, pc(ramp::SOIL, 0));
}

fn conifer(b: &mut Build, drop: f32, snow: bool, r: u32) {
    trunk(b, drop, 0.03);
    let tall = 1.0 + 0.25 * unit(r);
    let tiers: [(f32, f32, f32, usize); 4] = [
        (0.046, 0.03, 0.082, 0),
        (0.034, 0.082, 0.126, 1),
        (0.022, 0.126, 0.164, 0),
        (0.010, 0.164, 0.2, 1),
    ];
    for (half, z0, z1, step) in tiers {
        let (z0, z1) = (z0 * tall, z1 * tall);
        b.block(half, z0, z1, pc(ramp::FOREST, step));
        if snow {
            b.block(half - 0.004, z1, z1 + 0.004, pc(ramp::SNOW, 2));
        }
    }
}

fn broadleaf(b: &mut Build, drop: f32, leaves: Leaves, snow: bool, r: u32) {
    let big = unit(r >> 3) > 0.5;
    let crown = if big { 0.05 } else { 0.042 };
    trunk(b, drop, 0.05);
    let turned = match leaves {
        Leaves::Turning(share) => unit(r >> 7) < share,
        Leaves::Autumn => true,
        _ => false,
    };
    if leaves == Leaves::Bare {
        // Bare branches: a thin trunk, three short crossed limbs and a twig on top.
        let limb = pc(ramp::SOIL, 1);
        b.block(0.006, 0.05, 0.1, pc(ramp::SOIL, 0));
        b.put([-0.034, -0.005, 0.07], [0.034, 0.005, 0.077], limb);
        b.put([-0.005, -0.034, 0.088], [0.005, 0.034, 0.095], limb);
        b.put([-0.024, -0.005, 0.1], [0.024, 0.005, 0.107], limb);
        if snow {
            b.put(
                [-0.034, -0.005, 0.077],
                [0.034, 0.005, 0.081],
                pc(ramp::SNOW, 2),
            );
        }
        return;
    }
    let (lower, upper) = if turned {
        match (r >> 11) % 6 {
            0 | 1 => (pc(ramp::GOLD, 1), pc(ramp::GOLD, 2)),
            2 | 3 => (pc(ramp::BRICK, 1), pc(ramp::BRICK, 2)),
            4 => (pc(ramp::FIELD, 1), pc(ramp::FIELD, 2)),
            _ => (pc(ramp::SOIL, 2), pc(ramp::GOLD, 1)),
        }
    } else {
        (pc(ramp::FOREST, 0), pc(ramp::FOREST, 2))
    };
    b.block(crown, 0.05, 0.1, lower);
    b.block(crown * 0.62, 0.1, 0.135, upper);
    if snow {
        b.block(crown * 0.62 - 0.004, 0.135, 0.139, pc(ramp::SNOW, 2));
    }
}

fn scrub(b: &mut Build, drop: f32, r: u32) {
    let (lo, hi) = if r.is_multiple_of(3) {
        (pc(ramp::FOREST, 0), pc(ramp::FOREST, 1))
    } else {
        (pc(ramp::SOIL, 1), pc(ramp::FIELD, 0))
    };
    b.block(0.022, -drop, 0.024, lo);
    b.block(0.014, 0.024, 0.036, hi);
}

fn farmstead(b: &mut Build, snow: bool, r: u32) {
    // Dirt yard, barn with a stepped roof, two silos, the farmhouse and a windbreak.
    b.put([-0.09, -0.07, 0.0], [0.09, 0.07, 0.003], pc(ramp::SOIL, 1));
    let barn = if r & 1 == 0 {
        pc(ramp::BRICK, 1)
    } else {
        pc(ramp::BRICK, 2)
    };
    b.put([-0.05, -0.022, 0.003], [0.03, 0.022, 0.03], barn);
    let roof = pc(ramp::ROCK, 2);
    b.put([-0.054, -0.026, 0.03], [0.034, 0.026, 0.037], roof);
    b.put([-0.054, -0.016, 0.037], [0.034, 0.016, 0.044], roof);
    if snow {
        b.put(
            [-0.05, -0.012, 0.044],
            [0.03, 0.012, 0.048],
            pc(ramp::SNOW, 2),
        );
    }
    for (k, y) in [-0.016f32, 0.01].iter().enumerate() {
        let h = if k == 0 { 0.07 } else { 0.056 };
        b.put(
            [-0.078, y - 0.007, 0.003],
            [-0.064, y + 0.007, h],
            pc(ramp::ROCK, 3),
        );
        b.put(
            [-0.078, y - 0.007, h],
            [-0.064, y + 0.007, h + 0.008],
            pc(ramp::ROCK, 1),
        );
    }
    b.put(
        [0.05, -0.016, 0.003],
        [0.08, 0.016, 0.022],
        pc(ramp::CLOUD, 3),
    );
    b.put(
        [0.047, -0.019, 0.022],
        [0.083, 0.019, 0.03],
        pc(ramp::BRICK, 1),
    );
    b.put(
        [0.056, -0.012, 0.03],
        [0.074, 0.012, 0.036],
        pc(ramp::BRICK, 1),
    );
    if snow {
        b.put(
            [0.056, -0.012, 0.036],
            [0.074, 0.012, 0.04],
            pc(ramp::SNOW, 2),
        );
    }
}

// ---------------------------------------------------------------------------------------------
// Placement.

/// A conifer or a broadleaf at a position, by latitude, height and the stand's patch noise.
fn plant_tree(b: &mut Build, g: &Ground, patch: f32, week: u32, drop: f32, r: u32) {
    let snow = g.cls == G_SNOW;
    let a = g.lat.abs();
    let north = ((a - 40.5) / 7.0).clamp(0.0, 1.0);
    let high = ((g.elev_m - 900.0) / 500.0).clamp(0.0, 1.0) * 0.6;
    let south_pine = if a < 36.0 { 0.2 } else { 0.0 };
    let p_conifer = (north * north * (3.0 - 2.0 * north) + high + south_pine + (patch - 0.5) * 0.5)
        .clamp(0.0, 1.0);
    if unit(r >> 5) < p_conifer {
        conifer(b, drop, snow, r);
    } else {
        broadleaf(b, drop, leaves(week, g.lat), snow, r);
    }
}

/// Everything standing in one chunk, as a mesh in world kilometres.
/// What a chunk holds, for tests and the interface: each prop's kind and position (km).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub kind: PropKind,
    pub x: f32,
    pub y: f32,
    /// Half the footprint east and north in km, for a city building (0 for the others).
    pub half: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropKind {
    Tree,
    Scrub,
    House,
    Farm,
    /// A city building on a block: a mid-rise, an industrial shed or a downtown tower.
    Building,
}

/// The mesh of one chunk.
pub fn chunk_mesh(scene: &Scene, index: &PropIndex, key: ChunkKey) -> Mesh {
    build_chunk(scene, index, key).0
}

/// The props of one chunk, without building their geometry's meaning: positions only.
pub fn chunk_placed(scene: &Scene, index: &PropIndex, key: ChunkKey) -> Vec<Placed> {
    build_chunk(scene, index, key).1
}

fn build_chunk(scene: &Scene, index: &PropIndex, key: ChunkKey) -> (Mesh, Vec<Placed>) {
    let mut m = Mesh::default();
    let mut placed: Vec<Placed> = Vec::new();
    let stride = key.stride as i32;
    let scale = scale_for(key.stride);
    let ck = chunk_km(key.stride);
    let (x0, y0) = (key.ix as f32 * ck, key.iy as f32 * ck);
    let (x1, y1) = (x0 + ck, y0 + ck);
    let mut count = 0usize;

    // Lattice cells whose centre lies in this chunk and whose indices are multiples of the stride.
    let cells = |cell: f32| -> Vec<(i32, i32)> {
        let (i0, i1) = ((x0 / cell).floor() as i32, (x1 / cell).floor() as i32);
        let (j0, j1) = ((y0 / cell).floor() as i32, (y1 / cell).floor() as i32);
        let mut v = Vec::new();
        for j in j0..=j1 {
            if j.rem_euclid(stride) != 0 {
                continue;
            }
            for i in i0..=i1 {
                if i.rem_euclid(stride) != 0 {
                    continue;
                }
                let (cx, cy) = ((i as f32 + 0.5) * cell, (j as f32 + 0.5) * cell);
                if cx >= x0 && cx < x1 && cy >= y0 && cy < y1 {
                    v.push((i, j));
                }
            }
        }
        v
    };
    let clear = |x: f32, y: f32, r: f32| -> Option<(f32, f32, f32, f32, f32)> {
        let (road, river, lot) = index.near(x, y);
        if lot || road < ROAD_CLEAR_KM + r * 0.3 || river < RIVER_CLEAR_KM + r * 0.3 {
            return None;
        }
        let (z, low) = standing_z(scene, x, y, r)?;
        Some((z, low, road, river, 0.0))
    };

    // Trees: forest stands, field edges, road and river sides, scrub, and the city's tree texels.
    for (i, j) in cells(TREE_CELL_KM) {
        if count >= CHUNK_MAX_PROPS {
            break;
        }
        let h = hash(i, j, 0x7A);
        let x = (i as f32 + 0.5 + (unit(h) - 0.5) * 0.7) * TREE_CELL_KM;
        let y = (j as f32 + 0.5 + (unit(h >> 9) - 0.5) * 0.7) * TREE_CELL_KM;
        let Some(g) = ground_at(scene, x, y) else {
            continue;
        };
        let roll = unit(hash(i, j, 0x7B));
        // Stand noise: cells of about 1.4 km share a density and a species lean.
        let patch = unit(hash(
            (x / 1.4).floor() as i32,
            (y / 1.4).floor() as i32,
            0x31,
        ));
        let wanted = match g.base {
            G_FOREST => {
                let density = if patch < 0.08 {
                    0.0
                } else {
                    0.45 + 0.5 * patch
                };
                roll < density
            }
            G_FIELD => {
                let edge = [(0.45, 0.0), (-0.45, 0.0), (0.0, 0.45), (0.0, -0.45)]
                    .iter()
                    .any(|(ox, oy)| {
                        ground_at(scene, x + ox, y + oy)
                            .is_some_and(|n| n.base != G_FIELD && n.base != G_URBAN)
                    });
                let (road, river, _) = index.near(x, y);
                let p = if river > RIVER_CLEAR_KM && river < 0.45 {
                    0.55
                } else if road > ROAD_CLEAR_KM && road < 0.24 {
                    0.3
                } else if edge {
                    0.4
                } else {
                    0.008
                };
                roll < p
            }
            G_SOIL => roll < 0.1,
            G_URBAN if g.zone == ZONE_OUTER || g.zone == 1 => false,
            _ => false,
        };
        if !wanted || g.base == G_ROCK {
            continue;
        }
        let ts = scale * TREE_SCALE;
        let r = 0.05 * ts;
        let Some((z, low, ..)) = clear(x, y, r) else {
            continue;
        };
        let mut b = Build::new(&mut m, [x, y, z], ts, 0.0);
        let rh = hash(i, j, 0x7C);
        if g.base == G_SOIL {
            scrub(&mut b, (z - low) / ts, rh);
        } else {
            plant_tree(&mut b, &g, patch, scene.week, (z - low) / ts, rh);
        }
        placed.push(Placed {
            kind: if g.base == G_SOIL {
                PropKind::Scrub
            } else {
                PropKind::Tree
            },
            x,
            y,
            half: [0.0; 2],
        });
        count += 1;
    }

    // Cities: one building (or none) on each lot of the street lattice, by the zone it stands in:
    // houses in the outer residential ground, mid-rise blocks in the inner city, sheds in the
    // industrial ground and towers downtown, at their fixed size. A wider band keeps every n-th lot.
    if city_band(key.stride) {
        let lot_km = LOT_CELLS as f32 * CELL_KM;
        let mut city = 0usize;
        for (i, j) in cells(lot_km) {
            if city >= CHUNK_MAX_CITY {
                break;
            }
            if let Some(p) = city_lot(scene, &mut m, &clear, i, j) {
                placed.push(p);
                city += 1;
            }
        }
    }

    // Farmsteads: one chance per big cell, on farmland only.
    let fk = FARM_CELL_KM;
    let (i0, i1) = ((x0 / fk).floor() as i32, ((x1 - 1e-4) / fk).floor() as i32);
    let (j0, j1) = ((y0 / fk).floor() as i32, ((y1 - 1e-4) / fk).floor() as i32);
    for j in j0..=j1 {
        for i in i0..=i1 {
            let h = hash(i, j, 0xFA);
            if unit(h) >= 0.5 {
                continue;
            }
            let x = (i as f32 + 0.15 + 0.7 * unit(h >> 8)) * fk;
            let y = (j as f32 + 0.15 + 0.7 * unit(h >> 16)) * fk;
            if x < x0 || x >= x1 || y < y0 || y >= y1 {
                continue;
            }
            let Some(g) = ground_at(scene, x, y) else {
                continue;
            };
            if g.base != G_FIELD {
                continue;
            }
            // Fields all round: look a few hundred metres out so a farm is not on a field's rim.
            let inside = [(0.3, 0.0), (-0.3, 0.0), (0.0, 0.3), (0.0, -0.3)]
                .iter()
                .all(|(ox, oy)| {
                    ground_at(scene, x + ox, y + oy).is_some_and(|n| n.base == G_FIELD)
                });
            if !inside {
                continue;
            }
            let fs = scale * 1.5;
            let r = 0.1 * fs;
            let Some((z, low, ..)) = clear(x, y, r) else {
                continue;
            };
            if z - low > 0.004 {
                continue;
            }
            let yaw = (h >> 4 & 3) as f32 * 90.0;
            let mut b = Build::new(&mut m, [x, y, z], fs, yaw);
            farmstead(&mut b, g.cls == G_SNOW, h);
            // A windbreak of trees on the yard's north side (in the farm's own frame).
            for k in 0..3 {
                let local = [-0.09 + 0.09 * k as f32, 0.1];
                let (s, c) = yaw.to_radians().sin_cos();
                let (tx, ty) = (
                    x + (local[0] * c - local[1] * s) * fs,
                    y + (local[0] * s + local[1] * c) * fs,
                );
                if let Some((tz, tlow, ..)) = clear(tx, ty, 0.05 * scale * TREE_SCALE) {
                    let mut tb = Build::new(&mut m, [tx, ty, tz], scale * TREE_SCALE, 0.0);
                    let rh = hash(i * 4 + k, j, 0xFB);
                    conifer(
                        &mut tb,
                        (tz - tlow) / (scale * TREE_SCALE),
                        g.cls == G_SNOW,
                        rh,
                    );
                }
            }
            placed.push(Placed {
                kind: PropKind::Farm,
                x,
                y,
                half: [0.0; 2],
            });
        }
    }
    (m, placed)
}

/// Whether a point is a street or an avenue of the city as the ground shader draws it up close
/// (false off city ground), so tests can check that no building stands on one.
pub fn city_street_at(scene: &Scene, x: f32, y: f32) -> bool {
    let Some(g) = ground_at(scene, x, y) else {
        return false;
    };
    let core = g.base == G_CORE;
    if g.base != G_URBAN && !core {
        return false;
    }
    let (cx, cy) = ((x / CELL_KM).floor() as i32, (y / CELL_KM).floor() as i32);
    city_cell(core, g.zone, cx, cy).0 < 0
}

/// The prop placement's check that a spot is free: (x, y, half side) to the ground height under it,
/// the lowest ground round it, and the distances to the nearest road and river.
type Clear<'a> = dyn Fn(f32, f32, f32) -> Option<(f32, f32, f32, f32, f32)> + 'a;

/// Whether a band (`stride`) draws the city's buildings.
fn city_band(stride: u8) -> bool {
    stride as f32 * 12.0 <= CITY_MAX_WIDTH_KM
}

#[derive(Clone, Copy, PartialEq)]
enum CityKind {
    House,
    MidRise,
    Shed,
    Tower,
}

/// The building (or tree) of one lot of the street lattice, built into `m`; None for an empty lot.
/// Its footprint is a whole number of lattice cells inside the lot, every one of them block ground
/// of the city (never a street, an avenue, a park or another zone's street), and the spot must be
/// clear of roads, rivers, water and the placed buildings like every prop.
fn city_lot(scene: &Scene, m: &mut Mesh, clear: &Clear, i: i32, j: i32) -> Option<Placed> {
    let (lx, ly) = (i * LOT_CELLS, j * LOT_CELLS);
    let km = |c: i32| (c as f32 + 0.5) * CELL_KM;
    let mid = LOT_CELLS / 2;
    let g = ground_at(scene, km(lx + mid), km(ly + mid))?;
    let core = g.base == G_CORE;
    if g.base != G_URBAN && !core {
        return None;
    }
    let kind = if core {
        CityKind::Tower
    } else if g.zone == ZONE_OUTER {
        CityKind::House
    } else if g.zone == ZONE_INDUSTRIAL {
        CityKind::Shed
    } else {
        CityKind::MidRise
    };
    let h = hash(i, j, 0xC17);
    let roll = unit(h) * 100.0;
    // The texture's own tree cell in the middle of the lot gets a tree, as the old suburbs did.
    let (_, green) = city_cell(core, g.zone, lx + mid, ly + mid);
    if green == 2 {
        let (x, y) = (km(lx + mid), km(ly + mid));
        let r = 0.045 * 1.2;
        let (z, low, ..) = clear(x, y, r)?;
        let mut b = Build::new(m, [x, y, z], 1.2, 0.0);
        plant_tree(
            &mut b,
            &g,
            0.3,
            scene.week,
            (z - low) / 1.2,
            hash(i, j, 0x92),
        );
        return Some(Placed {
            kind: PropKind::Tree,
            x,
            y,
            half: [0.0; 2],
        });
    }
    let wanted = match kind {
        CityKind::House => {
            let (lon, lat) = scene.projection.to_lonlat(km(lx), km(ly));
            house_limit(scene, lon, lat)
        }
        CityKind::MidRise => 82.0,
        CityKind::Shed => 70.0,
        CityKind::Tower => 88.0,
    };
    if roll >= wanted {
        return None;
    }
    let bit = |k: u32| (h >> k) & 1 == 1;
    let (fw, fh) = match kind {
        CityKind::House => {
            if bit(9) {
                (2, 1)
            } else {
                (1, 2)
            }
        }
        CityKind::MidRise | CityKind::Tower => (2 + bit(9) as i32, 2 + bit(10) as i32),
        CityKind::Shed => {
            if bit(9) {
                (3, 2)
            } else {
                (2, 3)
            }
        }
    };
    let ox = (hash(i, j, 0xC18) % (LOT_CELLS - fw + 1) as u32) as i32;
    let oy = (hash(i, j, 0xC19) % (LOT_CELLS - fh + 1) as u32) as i32;
    let (cx0, cy0) = (lx + ox, ly + oy);
    // Every cell under the footprint is block ground of the city. The shader reads the zone (and
    // so the street layout) per pixel, and a ground texel's edge can cross a cell, so each cell is
    // read at its middle and near its four corners with the zone found there: any part of the cell
    // that some zone draws as a street holds at least one of those points.
    const AT: [(f32, f32); 5] = [
        (0.5, 0.5),
        (0.001, 0.001),
        (0.999, 0.001),
        (0.001, 0.999),
        (0.999, 0.999),
    ];
    for cy in cy0..cy0 + fh {
        for cx in cx0..cx0 + fw {
            for (fx, fy) in AT {
                let (px, py) = ((cx as f32 + fx) * CELL_KM, (cy as f32 + fy) * CELL_KM);
                let gc = ground_at(scene, px, py)?;
                let c_core = gc.base == G_CORE;
                if gc.base != G_URBAN && !c_core {
                    return None;
                }
                let (tone, green) = city_cell(c_core, gc.zone, cx, cy);
                if tone < 0 || green != 0 {
                    return None;
                }
            }
        }
    }
    // A sliver of the block stays open round the walls, so no wall sits on the street's edge.
    let gap = 0.004;
    let hw = fw as f32 * CELL_KM / 2.0 - gap;
    let hd = fh as f32 * CELL_KM / 2.0 - gap;
    let x = cx0 as f32 * CELL_KM + fw as f32 * CELL_KM / 2.0;
    let y = cy0 as f32 * CELL_KM + fh as f32 * CELL_KM / 2.0;
    let (z, low, ..) = clear(x, y, hw.max(hd))?;
    let mut b = Build::new(m, [x, y, z], 1.0, 0.0);
    if z - low > 1e-4 {
        b.put([-hw, -hd, -(z - low)], [hw, hd, 0.0], pc(ramp::ROCK, 1));
    }
    let r = hash(i, j, 0xC1A);
    let snow = snowy(scene, x, y);
    match kind {
        CityKind::House => city_house(&mut b, hw, hd, snow, r),
        CityKind::MidRise => mid_rise(&mut b, hw, hd, snow, r),
        CityKind::Shed => shed(&mut b, hw, hd, snow, r),
        CityKind::Tower => tower(&mut b, hw, hd, r),
    }
    Some(Placed {
        kind: if kind == CityKind::House {
            PropKind::House
        } else {
            PropKind::Building
        },
        x,
        y,
        half: [hw, hd],
    })
}

/// A low house with a pitched roof: the roof steps in toward a ridge along the long side.
fn city_house(b: &mut Build, hw: f32, hd: f32, snow: bool, r: u32) {
    let walls = [
        pc(ramp::CLOUD, 2),
        pc(ramp::SOIL, 3),
        pc(ramp::BRICK, 2),
        pc(ramp::URBAN, 3),
        pc(ramp::CLOUD, 1),
    ];
    let roofs = [
        pc(ramp::ROCK, 1),
        pc(ramp::BRICK, 1),
        pc(ramp::SOIL, 1),
        pc(ramp::ROCK, 0),
        pc(ramp::WATER, 1),
    ];
    let wall_h = 0.007;
    b.put(
        [-hw, -hd, 0.0],
        [hw, hd, wall_h],
        walls[(r >> 3) as usize % walls.len()],
    );
    let roof = if snow {
        pc(ramp::SNOW, 2)
    } else {
        roofs[(r >> 7) as usize % roofs.len()]
    };
    // Three steps up to the ridge, each narrower across the short side.
    let along_x = hw >= hd;
    for k in 0..3 {
        let f = 1.0 - k as f32 / 3.0;
        let (ex, ey) = if along_x {
            (hw + 0.002, (hd + 0.002) * f)
        } else {
            ((hw + 0.002) * f, hd + 0.002)
        };
        let z0 = wall_h + k as f32 * 0.003;
        b.put([-ex, -ey, z0], [ex, ey, z0 + 0.003], roof);
    }
}

/// An inner city block: three to seven floors, a darker shop floor, a roof edge and a stair house.
fn mid_rise(b: &mut Build, hw: f32, hd: f32, snow: bool, r: u32) {
    let bodies = [
        pc(ramp::BRICK, 2),
        pc(ramp::URBAN, 3),
        pc(ramp::CLOUD, 2),
        pc(ramp::BRICK, 1),
        pc(ramp::SOIL, 3),
        pc(ramp::ROCK, 2),
    ];
    let floors = 3 + (r >> 4) % 5;
    let top = 0.006 * floors as f32;
    let body = bodies[(r >> 9) as usize % bodies.len()];
    b.put([-hw, -hd, 0.0], [hw, hd, 0.005], pc(ramp::ROCK, 1));
    b.put([-hw, -hd, 0.005], [hw, hd, top], body);
    let roof = if snow {
        pc(ramp::SNOW, 2)
    } else {
        pc(ramp::ROCK, 1)
    };
    b.put(
        [-hw - 0.001, -hd - 0.001, top],
        [hw + 0.001, hd + 0.001, top + 0.002],
        roof,
    );
    let (sx, sy) = (hw * 0.3, hd * 0.3);
    b.put(
        [-sx, -sy, top + 0.002],
        [sx, sy, top + 0.006],
        pc(ramp::ROCK, 2),
    );
}

/// An industrial shed: low and wide, with a sawtooth roof of three ridges.
fn shed(b: &mut Build, hw: f32, hd: f32, snow: bool, r: u32) {
    let walls = [pc(ramp::CLOUD, 2), pc(ramp::ROCK, 2), pc(ramp::URBAN, 2)];
    let top = 0.008 + 0.004 * unit(r);
    b.put(
        [-hw, -hd, 0.0],
        [hw, hd, top],
        walls[(r >> 11) as usize % walls.len()],
    );
    let roof = if snow {
        pc(ramp::SNOW, 2)
    } else {
        pc(ramp::ROCK, 1)
    };
    let along_x = hw >= hd;
    for k in 0..3 {
        let t = -1.0 + (2 * k + 1) as f32 / 3.0;
        let (a, c) = if along_x {
            (
                [t * hw - hw / 6.0, -hd, top],
                [t * hw + hw / 12.0, hd, top + 0.003],
            )
        } else {
            (
                [-hw, t * hd - hd / 6.0, top],
                [hw, t * hd + hd / 12.0, top + 0.003],
            )
        };
        b.put(a, c, roof);
    }
    if (r >> 15).is_multiple_of(4) {
        let s = 0.003;
        b.put(
            [hw * 0.6 - s, -s, top],
            [hw * 0.6 + s, s, top + 0.02],
            pc(ramp::BRICK, 1),
        );
    }
}

/// A downtown tower: a body in two or three setbacks, glass or stone, with a crown.
fn tower(b: &mut Build, hw: f32, hd: f32, r: u32) {
    let skins = [
        pc(ramp::WATER, 2),
        pc(ramp::URBAN, 3),
        pc(ramp::CLOUD, 2),
        pc(ramp::WATER, 3),
        pc(ramp::ROCK, 2),
    ];
    let u = unit(r);
    let top = 0.04 + 0.16 * u * u;
    let skin = skins[(r >> 5) as usize % skins.len()];
    let tiers = 2 + (r >> 9) % 2;
    let mut z = 0.0;
    for k in 0..tiers {
        let f = 1.0 - 0.22 * k as f32;
        let z1 = top * (k + 1) as f32 / tiers as f32;
        b.put([-hw * f, -hd * f, z], [hw * f, hd * f, z1], skin);
        z = z1;
    }
    let f = 1.0 - 0.22 * tiers as f32;
    b.put(
        [-hw * f, -hd * f, top],
        [hw * f, hd * f, top + 0.006],
        pc(ramp::ROCK, 1),
    );
}

/// Rank a house needs to stand: steady at 45, 55 percent more (or fewer) at two percent net
/// migration a year or more, like the classic map's `houseLimit`.
fn house_limit(scene: &Scene, lon: f32, lat: f32) -> f32 {
    let m = scene.mask.at(lon, lat)[0] as usize;
    let bps = if m > 0 {
        scene.region_trend_bps.get(m - 1).copied().unwrap_or(0)
    } else {
        0
    };
    let gain = (bps as f32 / 200.0).clamp(-1.0, 1.0);
    45.0 * (1.0 + 0.55 * gain)
}

/// Whether snow lies at a position this week (city ground never takes the ground's own snow
/// class, so the roofs ask the season directly).
fn snowy(scene: &Scene, x: f32, y: f32) -> bool {
    let (lon, lat) = scene.projection.to_lonlat(x, y);
    let (c, r) = scene.heightmap.cell_of(lon, lat);
    let m = scene.heightmap.at(c, r) as f32;
    crate::ground::snow_cover(scene.week, lat, lon, m) > 0.2
}
