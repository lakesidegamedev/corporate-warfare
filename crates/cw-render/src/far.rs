//! The world beyond the scenario's detailed terrain: where there is land and what it looks like,
//! read from the same land outlines and satellite picture the classic map's Earth layer uses.
//! The horizon apron is built from this, never from the scenario picture past its own edge.

use std::path::Path;

use crate::data::Bounds;
use crate::ground;

/// Degrees a block of the far land cover covers.
const CLASS_DEG: f32 = 0.25;

/// Unknown cell in the land cover while it is being filled in.
const NONE: u8 = 255;

/// A grey picture (0 sea, 255 land) over a box of the world.
struct Mask8 {
    w: usize,
    h: usize,
    a: Vec<u8>,
}

impl Mask8 {
    fn at(&self, u: f32, v: f32) -> bool {
        let x = (u * self.w as f32).floor();
        let y = (v * self.h as f32).floor();
        if y < 0.0 || y >= self.h as f32 {
            return false;
        }
        let x = (x as i64).rem_euclid(self.w as i64) as usize;
        self.a[y as usize * self.w + x] > 127
    }
}

pub struct FarWorld {
    /// The whole world, longitude -180 to 180 and latitude 90 to -90.
    world: Mask8,
    /// A finer outline of the scenario's own surroundings, with the box it covers.
    region: Option<(Mask8, Bounds)>,
    /// The land class of the world in `CLASS_DEG` blocks, the same classes the terrain uses.
    classes: Vec<u8>,
    cw: usize,
    ch: usize,
}

fn load_mask(path: &Path) -> Result<Mask8, String> {
    let img = image::open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .to_rgba8();
    let (w, h) = (img.width() as usize, img.height() as usize);
    let a = img.into_raw().chunks_exact(4).map(|p| p[3]).collect();
    Ok(Mask8 { w, h, a })
}

impl FarWorld {
    /// Read the world picture and the land outlines from the resources folder. The scenario's
    /// finer outline is optional.
    pub fn load(res: &Path, scenario: &str, region: Bounds) -> Result<FarWorld, String> {
        let world = load_mask(&res.join("earth-land.png"))?;
        let region = load_mask(&res.join(format!("earth-land-{scenario}.png")))
            .ok()
            .map(|m| (m, region));
        let earth = image::open(res.join("earth.jpg"))
            .map_err(|e| format!("earth.jpg: {e}"))?
            .to_rgb8();
        let (cw, ch) = ((360.0 / CLASS_DEG) as usize, (180.0 / CLASS_DEG) as usize);
        let (ew, eh) = (earth.width() as usize, earth.height() as usize);
        let mut classes = vec![NONE; cw * ch];
        for by in 0..ch {
            for bx in 0..cw {
                let mut votes = [0u32; 8];
                let (x0, x1) = (bx * ew / cw, ((bx + 1) * ew / cw).max(bx * ew / cw + 1));
                let (y0, y1) = (by * eh / ch, ((by + 1) * eh / ch).max(by * eh / ch + 1));
                for y in y0..y1.min(eh) {
                    for x in x0..x1.min(ew) {
                        let (u, v) = ((x as f32 + 0.5) / ew as f32, (y as f32 + 0.5) / eh as f32);
                        if world.at(u, v) {
                            let p = earth.get_pixel(x as u32, y as u32).0;
                            votes[ground::classify(p) as usize & 7] += 1;
                        }
                    }
                }
                if let Some((k, n)) = votes.iter().enumerate().max_by_key(|(_, n)| **n)
                    && *n > 0
                {
                    classes[by * cw + bx] = k as u8;
                }
            }
        }
        // Blocks of open sea take the nearest land's class, so a coastal cell never reads
        // nothing.
        for _ in 0..64 {
            let mut changed = false;
            let prev = classes.clone();
            for y in 0..ch {
                for x in 0..cw {
                    if prev[y * cw + x] != NONE {
                        continue;
                    }
                    let nb = [
                        (x + cw - 1) % cw + y * cw,
                        (x + 1) % cw + y * cw,
                        x + y.saturating_sub(1) * cw,
                        x + (y + 1).min(ch - 1) * cw,
                    ];
                    if let Some(c) = nb.iter().map(|&i| prev[i]).find(|&c| c != NONE) {
                        classes[y * cw + x] = c;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        for c in &mut classes {
            if *c == NONE {
                *c = ground::G_FIELD;
            }
        }
        Ok(FarWorld {
            world,
            region,
            classes,
            cw,
            ch,
        })
    }

    /// A world that is all one flat land, for the tiny synthetic test scene.
    pub fn flat_land() -> FarWorld {
        FarWorld {
            world: Mask8 {
                w: 1,
                h: 1,
                a: vec![255],
            },
            region: None,
            classes: vec![ground::G_FIELD],
            cw: 1,
            ch: 1,
        }
    }

    /// Is there land at this point. Longitude wraps; beyond the poles is sea.
    pub fn land(&self, lon: f32, lat: f32) -> bool {
        if let Some((m, b)) = &self.region
            && lon >= b.west
            && lon < b.east
            && lat > b.south
            && lat <= b.north
        {
            return m.at(
                (lon - b.west) / (b.east - b.west),
                (b.north - lat) / (b.north - b.south),
            );
        }
        self.world.at((lon + 180.0) / 360.0, (90.0 - lat) / 180.0)
    }

    /// How much of the square of half width `half` degrees round a point is land, 0 to 1.
    /// Averaged over a small grid so the outline reads smooth rather than in picture pixels.
    pub fn cover(&self, lon: f32, lat: f32, half: f32) -> f32 {
        let mut n = 0;
        for j in -1..=1 {
            for i in -1..=1 {
                if self.land(lon + half * i as f32, lat + half * j as f32) {
                    n += 1;
                }
            }
        }
        n as f32 / 9.0
    }

    /// The land class of the world at a point (the picture's, ignoring where the sea is).
    pub fn class(&self, lon: f32, lat: f32) -> u8 {
        let x = (((lon + 180.0) / CLASS_DEG).floor() as i64).rem_euclid(self.cw as i64) as usize;
        let y = (((90.0 - lat) / CLASS_DEG).floor().max(0.0) as usize).min(self.ch - 1);
        self.classes[y * self.cw + x]
    }
}
