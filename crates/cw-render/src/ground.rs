//! Ground classes: the land cover of the map as clean patches. The satellite image is read once
//! on the CPU, each texel snapped to one class, and a majority filter (a box blur of each
//! class, then the strongest class wins) turns speckle into forest blocks and field blocks with
//! clean edges. The shader never classifies; it only reads this raster.

use crate::data::{Bounds, Scene, WATER_LAKE, WATER_OCEAN, WATER_URBAN};

pub const G_FOREST: u8 = 0;
pub const G_FIELD: u8 = 1;
pub const G_SOIL: u8 = 2;
pub const G_ROCK: u8 = 3;
pub const G_SNOW: u8 = 4;
pub const G_URBAN: u8 = 5;
/// Downtown of the bigger cities: the densest block grid.
pub const G_CORE: u8 = 6;
const CLASSES: usize = 6;
/// Field patches are this many texels across (about 17 km); each gets its own crop row
/// direction. Every other row of patches is shifted half a patch so the quilt has no grid.
const PATCH_TEXELS: usize = 14;

/// Texels per degree of the class raster (about a kilometre).
pub const GROUND_PPD: f32 = 80.0;
/// Radius of the majority filter in texels, and the number of passes.
const FILTER_RADIUS: usize = 4;
const FILTER_PASSES: usize = 3;
/// Land above this height is bare rock.
const ROCK_M: i16 = 1150;

pub struct Ground {
    pub w: usize,
    pub h: usize,
    pub bounds: Bounds,
    pub ppd: f32,
    pub classes: Vec<u8>,
    /// Per texel patch variant 0 to 3 (crop row direction), read beside the class.
    pub variant: Vec<u8>,
}

/// One class per satellite colour. The image is dark summer imagery, so it is stretched first.
fn classify(rgb: [u8; 3]) -> u8 {
    let c = [
        (rgb[0] as f32 / 255.0 - 0.03) * 2.6,
        (rgb[1] as f32 / 255.0 - 0.05) * 2.6,
        (rgb[2] as f32 / 255.0 - 0.02) * 2.6,
    ];
    let protos: [([f32; 3], u8); 5] = [
        ([0.17, 0.33, 0.12], G_FOREST),
        ([0.32, 0.46, 0.14], G_FIELD),
        ([0.52, 0.47, 0.30], G_SOIL),
        ([0.50, 0.50, 0.50], G_SOIL),
        ([0.90, 0.92, 0.95], G_SNOW),
    ];
    let mut best = (f32::MAX, G_FOREST);
    for (p, k) in protos {
        let e = [c[0] - p[0], c[1] - p[1], c[2] - p[2]];
        let d = 0.30 * e[0] * e[0] + 0.59 * e[1] * e[1] + 0.11 * e[2] * e[2];
        if d < best.0 {
            best = (d, k);
        }
    }
    best.1
}

/// Separable box blur of one plane, radius `r`, clamped at the edges.
fn box_blur(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let norm = 1.0 / (2 * r + 1) as f32;
    let mut tmp = vec![0.0f32; w * h];
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let mut acc = 0.0;
        acc += row[..=r.min(w - 1)].iter().sum::<f32>();
        acc += row[0] * r as f32; // clamp left
        for x in 0..w {
            tmp[y * w + x] = acc * norm;
            let add = row[(x + r + 1).min(w - 1)];
            let sub = row[x.saturating_sub(r)];
            acc += add - sub;
        }
    }
    let mut out = vec![0.0f32; w * h];
    for x in 0..w {
        let mut acc = 0.0;
        for i in 0..=r.min(h - 1) {
            acc += tmp[i * w + x];
        }
        acc += tmp[x] * r as f32;
        for y in 0..h {
            out[y * w + x] = acc * norm;
            let add = tmp[(y + r + 1).min(h - 1) * w + x];
            let sub = tmp[y.saturating_sub(r) * w + x];
            acc += add - sub;
        }
    }
    out
}

pub fn build(scene: &Scene) -> Ground {
    let b = scene.heightmap.bounds;
    let ppd = GROUND_PPD;
    let w = ((b.east - b.west) * ppd) as usize;
    let h = ((b.north - b.south) * ppd) as usize;
    let earth = &scene.earth;
    let mut raw = vec![G_FOREST; w * h];
    let mut votes = vec![0.0f32; w * h]; // 1 where the texel is land and counts
    let mut urban = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let lon = b.west + (x as f32 + 0.5) / ppd;
            let lat = b.north - (y as f32 + 0.5) / ppd;
            let i = y * w + x;
            let kind = scene.mask.at(lon, lat)[2];
            if kind == WATER_OCEAN || kind == WATER_LAKE {
                continue;
            }
            urban[i] = kind == WATER_URBAN;
            let u = ((lon - earth.bounds.west) / (earth.bounds.east - earth.bounds.west)
                * earth.w as f32)
                .clamp(0.0, earth.w as f32 - 1.0) as usize;
            let v = ((earth.bounds.north - lat) / (earth.bounds.north - earth.bounds.south)
                * earth.h as f32)
                .clamp(0.0, earth.h as f32 - 1.0) as usize;
            let p = (v * earth.w as usize + u) * 4;
            raw[i] = classify([earth.rgba[p], earth.rgba[p + 1], earth.rgba[p + 2]]);
            votes[i] = 1.0;
            let m = scene.heightmap.at(
                scene.heightmap.cell_of(lon, lat).0,
                scene.heightmap.cell_of(lon, lat).1,
            );
            if m >= ROCK_M {
                raw[i] = G_ROCK;
            }
        }
    }
    // Majority filter: blur each class's land votes, repeat, and let the strongest win.
    let mut planes: Vec<Vec<f32>> = (0..CLASSES - 1)
        .map(|k| {
            (0..w * h)
                .map(|i| if raw[i] == k as u8 { votes[i] } else { 0.0 })
                .collect()
        })
        .collect();
    for _ in 0..FILTER_PASSES {
        for p in planes.iter_mut() {
            *p = box_blur(p, w, h, FILTER_RADIUS);
        }
    }
    let mut classes = vec![G_FOREST; w * h];
    for i in 0..w * h {
        let mut best = (-1.0f32, G_FOREST);
        for (k, p) in planes.iter().enumerate() {
            // Snow and rock need a clear majority so they never speckle.
            let bias = match k as u8 {
                G_SNOW | G_ROCK => 0.8,
                G_SOIL => 0.45,
                _ => 1.0,
            };
            if p[i] * bias > best.0 {
                best = (p[i] * bias, k as u8);
            }
        }
        classes[i] = best.1;
        if urban[i] {
            classes[i] = G_URBAN;
        }
    }
    // Downtown cores of the bigger cities, from the map's own city list.
    let kx = crate::data::KM_PER_DEG_LON_EQ * scene.projection.lat0.to_radians().cos();
    for c in scene.cities.iter().filter(|c| c.population >= 250_000) {
        let r_km = 2.0 + 3.2 * (c.population as f32 / 1.0e6).sqrt();
        let (cx, cy) = ((c.lon - b.west) * ppd, (b.north - c.lat) * ppd);
        let (rx, ry) = (r_km / kx * ppd, r_km / crate::data::KM_PER_DEG_LAT * ppd);
        for y in (cy - ry).max(0.0) as usize..((cy + ry) as usize + 1).min(h) {
            for x in (cx - rx).max(0.0) as usize..((cx + rx) as usize + 1).min(w) {
                let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 && classes[y * w + x] == G_URBAN {
                    classes[y * w + x] = G_CORE;
                }
            }
        }
    }
    let mut variant = vec![0u8; w * h];
    for y in 0..h {
        let by = y / PATCH_TEXELS;
        let shift = (by % 2) * PATCH_TEXELS / 2;
        for x in 0..w {
            let bx = (x + shift) / PATCH_TEXELS;
            let hsh = (bx as u32)
                .wrapping_mul(0x9E37_79B1)
                .wrapping_add((by as u32).wrapping_mul(0x85EB_CA6B))
                .rotate_left(13)
                .wrapping_mul(0xC2B2_AE35);
            variant[y * w + x] = ((hsh >> 24) & 3) as u8;
        }
    }
    Ground {
        w,
        h,
        bounds: b,
        ppd,
        classes,
        variant,
    }
}
