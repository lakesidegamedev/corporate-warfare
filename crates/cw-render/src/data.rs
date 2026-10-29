//! Scene data: terrain, the rasterised map, textures and voxel buildings. Everything is plain
//! CPU data; nothing here touches the GPU, the engine or the network.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::palette::Rgb;

/// Kilometres per degree of latitude, and of longitude at the scenario's reference latitude.
pub const KM_PER_DEG_LAT: f32 = 110.57;
pub const KM_PER_DEG_LON_EQ: f32 = 111.32;

/// Mask water kinds (blue channel).
pub const WATER_NONE: u8 = 0;
pub const WATER_OCEAN: u8 = 1;
pub const WATER_LAKE: u8 = 2;
pub const WATER_URBAN: u8 = 3;

/// Region tint classes (alpha channel); 0 means no region.
pub const TINT_PLAY: u8 = 1;
pub const TINT_PLAYER: u8 = 2;

/// The flat map projection: x east, y north, kilometres from the reference point.
#[derive(Clone, Copy, Debug)]
pub struct Projection {
    pub lon0: f32,
    pub lat0: f32,
}

impl Projection {
    pub fn km_per_deg_lon(&self) -> f32 {
        KM_PER_DEG_LON_EQ * self.lat0.to_radians().cos()
    }
    pub fn to_km(&self, lon: f32, lat: f32) -> (f32, f32) {
        (
            (lon - self.lon0) * self.km_per_deg_lon(),
            (lat - self.lat0) * KM_PER_DEG_LAT,
        )
    }
    pub fn to_lonlat(&self, x: f32, y: f32) -> (f32, f32) {
        (
            self.lon0 + x / self.km_per_deg_lon(),
            self.lat0 + y / KM_PER_DEG_LAT,
        )
    }
}

/// A geographic box in degrees.
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub west: f32,
    pub south: f32,
    pub east: f32,
    pub north: f32,
}

/// Elevation in metres on a regular lon/lat grid, north row first.
pub struct Heightmap {
    pub w: usize,
    pub h: usize,
    pub bounds: Bounds,
    pub metres: Vec<i16>,
}

impl Heightmap {
    pub fn parse(bytes: &[u8]) -> Result<Heightmap, String> {
        if bytes.len() < 16 || &bytes[0..4] != b"CWH1" {
            return Err("not a CWH1 heightmap".into());
        }
        let u = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
        let s = |i: usize| i16::from_le_bytes([bytes[i], bytes[i + 1]]) as f32;
        let (w, h) = (u(4), u(6));
        let bounds = Bounds {
            west: s(8),
            south: s(10),
            east: s(12),
            north: s(14),
        };
        let body = &bytes[16..];
        if body.len() != w * h * 2 {
            return Err("heightmap size mismatch".into());
        }
        let metres = body
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        Ok(Heightmap {
            w,
            h,
            bounds,
            metres,
        })
    }

    pub fn cell_degrees(&self) -> (f32, f32) {
        (
            (self.bounds.east - self.bounds.west) / self.w as f32,
            (self.bounds.north - self.bounds.south) / self.h as f32,
        )
    }

    pub fn at(&self, col: usize, row: usize) -> i16 {
        self.metres[row.min(self.h - 1) * self.w + col.min(self.w - 1)]
    }

    pub fn cell_of(&self, lon: f32, lat: f32) -> (usize, usize) {
        let (dx, dy) = self.cell_degrees();
        let c = ((lon - self.bounds.west) / dx)
            .floor()
            .clamp(0.0, self.w as f32 - 1.0) as usize;
        let r = ((self.bounds.north - lat) / dy)
            .floor()
            .clamp(0.0, self.h as f32 - 1.0) as usize;
        (c, r)
    }
}

/// A raster of the map's own data: RGBA8, R region, G state, B water kind, A tint class.
pub struct Mask {
    pub w: usize,
    pub h: usize,
    pub bounds: Bounds,
    pub rgba: Vec<u8>,
}

impl Mask {
    pub fn px_per_degree(&self) -> f32 {
        self.w as f32 / (self.bounds.east - self.bounds.west)
    }
    pub fn at(&self, lon: f32, lat: f32) -> [u8; 4] {
        let ppd = self.px_per_degree();
        let x = (((lon - self.bounds.west) * ppd) as isize).clamp(0, self.w as isize - 1) as usize;
        let y = (((self.bounds.north - lat) * ppd) as isize).clamp(0, self.h as isize - 1) as usize;
        let i = (y * self.w + x) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

/// An RGBA8 image that covers a geographic box (rows north first).
pub struct GeoImage {
    pub w: u32,
    pub h: u32,
    pub bounds: Bounds,
    pub rgba: Vec<u8>,
}

/// Axis aligned box of a voxel model: x0 y0 z0 x1 y1 z1 in voxels, plus a colour.
#[derive(Clone, Debug)]
pub struct VoxelBox {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub colour: Rgb,
}

#[derive(Clone, Debug)]
pub struct VoxelModel {
    pub boxes: Vec<VoxelBox>,
    pub footprint: [f32; 2],
}

/// A building placed on the map. `yaw` turns it in degrees (0 faces south).
#[derive(Clone, Debug)]
pub struct Placement {
    pub model: String,
    pub lon: f32,
    pub lat: f32,
    pub yaw: f32,
}

pub struct Scene {
    pub projection: Projection,
    pub heightmap: Heightmap,
    pub mask: Mask,
    pub earth: GeoImage,
    pub clouds: GeoImage,
    pub models: HashMap<String, VoxelModel>,
    pub buildings: Vec<Placement>,
    /// Kilometres per voxel of the buildings.
    pub voxel_km: f32,
    /// Vertical exaggeration of terrain, and the terrace step in metres.
    pub exaggeration: f32,
    pub terrace_m: f32,
}

#[derive(Deserialize)]
struct RawRegion {
    code: String,
    state: String,
    rings: Vec<Vec<i32>>,
}
#[derive(Deserialize)]
struct RawState {
    rings: Vec<Vec<i32>>,
}
#[derive(Deserialize)]
struct RawRings {
    rings: Vec<Vec<i32>>,
}
#[derive(Deserialize)]
struct RawCity {
    x: i32,
    y: i32,
    population: u64,
}
#[derive(Deserialize)]
struct RawMap {
    countries: Vec<RawRings>,
    states: Vec<RawState>,
    regions: Vec<RawRegion>,
    lakes: Vec<RawRings>,
    cities: Vec<RawCity>,
}

/// The eight states of the Great Lakes 2000 scenario.
pub const GREAT_LAKES_STATES: [&str; 8] = ["IL", "IN", "KY", "MI", "NY", "OH", "PA", "WV"];
/// Regions drawn as the player's holdings in the spike.
pub const PLAYER_REGIONS: [&str; 5] = ["MI-DET", "IL-CHI", "OH-CEN", "OH-SW", "PA-WES"];
/// Cities that get a test building: name-free, by position in thousandths of a degree.
const TEST_CITIES: [(f32, f32, &str, f32); 14] = [
    (-87.635, 41.848, "financial/2", 0.0),
    (-83.051, 42.334, "manufacturing/2", 0.0),
    (-81.697, 41.472, "manufacturing/1", 0.0),
    (-82.999, 39.982, "financial/1", 0.0),
    (-80.002, 40.432, "manufacturing/2", 0.0),
    (-84.459, 39.162, "energy/1", 0.0),
    (-86.172, 39.771, "logistics/1", 0.0),
    (-78.882, 42.886, "chemical_industries/1", 0.0),
    (-79.383, 43.653, "financial/2", 0.0),
    (-87.906, 43.042, "retail/1", 0.0),
    (-83.74, 42.28, "technology/1", 0.0),
    (-81.52, 41.08, "extraction/1", 0.0),
    (-75.16, 39.95, "financial/1", 0.0),
    (-87.0, 40.2, "agriculture/2", 0.0),
];

fn parse_models(json: &str) -> Result<HashMap<String, VoxelModel>, String> {
    #[derive(Deserialize)]
    struct Raw {
        models: HashMap<String, RawModel>,
    }
    #[derive(Deserialize)]
    struct RawModel {
        w: f32,
        d: f32,
        boxes: Vec<(f32, f32, f32, f32, f32, f32, String)>,
    }
    let raw: Raw = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let hex = |s: &str| -> Rgb {
        let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0x808080);
        [(v >> 16) as u8, (v >> 8) as u8, v as u8]
    };
    Ok(raw
        .models
        .into_iter()
        .map(|(k, m)| {
            let boxes = m
                .boxes
                .iter()
                .map(|b| VoxelBox {
                    min: [b.0, b.1, b.2],
                    max: [b.3, b.4, b.5],
                    colour: hex(&b.6),
                })
                .collect();
            (
                k,
                VoxelModel {
                    boxes,
                    footprint: [m.w, m.d],
                },
            )
        })
        .collect())
}

/// Scanline even-odd fill of a set of rings into `put(x, y)`, in pixel coordinates.
fn fill_rings(rings: &[Vec<(f32, f32)>], w: usize, h: usize, mut put: impl FnMut(usize, usize)) {
    let (mut y0, mut y1) = (f32::MAX, f32::MIN);
    for r in rings {
        for p in r {
            y0 = y0.min(p.1);
            y1 = y1.max(p.1);
        }
    }
    if y1 < 0.0 || y0 > h as f32 {
        return;
    }
    let ys = y0.max(0.0).floor() as usize;
    let ye = (y1.min(h as f32 - 1.0)).ceil() as usize;
    let mut xs: Vec<f32> = Vec::new();
    for y in ys..=ye {
        let cy = y as f32 + 0.5;
        xs.clear();
        for r in rings {
            for i in 0..r.len() {
                let (a, b) = (r[i], r[(i + 1) % r.len()]);
                if (a.1 <= cy) != (b.1 <= cy) {
                    xs.push(a.0 + (cy - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks_exact(2) {
            let xa = (pair[0] - 0.5).ceil().max(0.0) as isize;
            let xb = ((pair[1] - 0.5).floor() as isize).min(w as isize - 1);
            for x in xa..=xb {
                put(x as usize, y);
            }
        }
    }
}

fn rings_to_px(rings: &[Vec<i32>], b: &Bounds, ppd: f32) -> Vec<Vec<(f32, f32)>> {
    rings
        .iter()
        .map(|r| {
            r.chunks_exact(2)
                .map(|p| {
                    let (lon, lat) = (p[0] as f32 / 1e4, p[1] as f32 / 1e4);
                    ((lon - b.west) * ppd, (b.north - lat) * ppd)
                })
                .collect()
        })
        .collect()
}

fn ring_touches(rings: &[Vec<i32>], b: &Bounds) -> bool {
    let (mut x0, mut x1, mut y0, mut y1) = (i32::MAX, i32::MIN, i32::MAX, i32::MIN);
    for r in rings {
        for p in r.chunks_exact(2) {
            x0 = x0.min(p[0]);
            x1 = x1.max(p[0]);
            y0 = y0.min(p[1]);
            y1 = y1.max(p[1]);
        }
    }
    (x1 as f32 / 1e4) >= b.west
        && (x0 as f32 / 1e4) <= b.east
        && (y1 as f32 / 1e4) >= b.south
        && (y0 as f32 / 1e4) <= b.north
}

/// Rasterise the world map JSON into the mask for `bounds` at `ppd` pixels a degree.
pub fn rasterise_mask(map_json: &str, bounds: Bounds, ppd: f32) -> Result<Mask, String> {
    let map: RawMap = serde_json::from_str(map_json).map_err(|e| e.to_string())?;
    let w = ((bounds.east - bounds.west) * ppd) as usize;
    let h = ((bounds.north - bounds.south) * ppd) as usize;
    // Pixel layout: R region index, G state index, B water kind, A tint class.
    let mut px = vec![0u8; w * h * 4];
    for i in 0..w * h {
        px[i * 4 + 2] = WATER_OCEAN;
    }
    let idx = |x: usize, y: usize| (y * w + x) * 4;
    for c in map
        .countries
        .iter()
        .filter(|c| ring_touches(&c.rings, &bounds))
    {
        fill_rings(&rings_to_px(&c.rings, &bounds, ppd), w, h, |x, y| {
            px[idx(x, y) + 2] = WATER_NONE
        });
    }
    for l in map.lakes.iter().filter(|l| ring_touches(&l.rings, &bounds)) {
        fill_rings(&rings_to_px(&l.rings, &bounds, ppd), w, h, |x, y| {
            px[idx(x, y) + 2] = WATER_LAKE
        });
    }
    // States and regions only claim land, so a polygon that includes lake water cannot hide it.
    for (i, s) in map.states.iter().enumerate() {
        if !ring_touches(&s.rings, &bounds) {
            continue;
        }
        fill_rings(&rings_to_px(&s.rings, &bounds, ppd), w, h, |x, y| {
            if px[idx(x, y) + 2] != WATER_LAKE && px[idx(x, y) + 2] != WATER_OCEAN {
                px[idx(x, y) + 1] = (i + 1).min(255) as u8;
            }
        });
    }
    let mut n = 0u8;
    for r in map
        .regions
        .iter()
        .filter(|r| GREAT_LAKES_STATES.contains(&r.state.as_str()))
    {
        n += 1;
        let tint = if PLAYER_REGIONS.contains(&r.code.as_str()) {
            TINT_PLAYER
        } else {
            TINT_PLAY
        };
        fill_rings(&rings_to_px(&r.rings, &bounds, ppd), w, h, |x, y| {
            let k = idx(x, y);
            if px[k + 2] == WATER_NONE {
                px[k] = n;
                px[k + 3] = tint;
            }
        });
    }
    // Cities: a disc of urban ground sized from population, on land only.
    let (kx, ky) = (
        (KM_PER_DEG_LON_EQ * 41.5f32.to_radians().cos()),
        KM_PER_DEG_LAT,
    );
    for c in &map.cities {
        let (lon, lat) = (c.x as f32 / 1e4, c.y as f32 / 1e4);
        if lon < bounds.west || lon > bounds.east || lat < bounds.south || lat > bounds.north {
            continue;
        }
        let r_km = 3.0 + 7.0 * (c.population as f32 / 1.0e6).sqrt();
        let (cx, cy) = ((lon - bounds.west) * ppd, (bounds.north - lat) * ppd);
        let (rx, ry) = (r_km / kx * ppd, r_km / ky * ppd);
        for y in (cy - ry).max(0.0) as usize..((cy + ry) as usize + 1).min(h) {
            for x in (cx - rx).max(0.0) as usize..((cx + rx) as usize + 1).min(w) {
                let (dx, dy) = ((x as f32 - cx) / rx, (y as f32 - cy) / ry);
                if dx * dx + dy * dy <= 1.0 && px[idx(x, y) + 2] == WATER_NONE {
                    px[idx(x, y) + 2] = WATER_URBAN;
                }
            }
        }
    }
    Ok(Mask {
        w,
        h,
        bounds,
        rgba: px,
    })
}

fn load_geo_image(path: &Path, bounds: Bounds) -> Result<GeoImage, String> {
    let img = image::open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?
        .to_rgba8();
    Ok(GeoImage {
        w: img.width(),
        h: img.height(),
        bounds,
        rgba: img.into_raw(),
    })
}

impl Scene {
    /// The Great Lakes 2000 scenario from the repository's own assets. `repo` is the
    /// repository root; the heightmap comes from crates/cw-render/assets.
    pub fn great_lakes_2000(repo: &Path) -> Result<Scene, String> {
        let res = repo.join("apple/Resources");
        let read = |p: &Path| std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()));
        let heightmap = Heightmap::parse(&read(
            &repo.join("crates/cw-render/assets/heightmap-great-lakes-2000.cwh"),
        )?)?;
        let map_json =
            String::from_utf8(read(&res.join("world-map.json"))?).map_err(|e| e.to_string())?;
        let mask = rasterise_mask(&map_json, heightmap.bounds, 120.0)?;
        let region = Bounds {
            west: -122.0,
            south: 18.0,
            east: -42.0,
            north: 66.0,
        };
        let earth = load_geo_image(&res.join("earth-great-lakes-2000.jpg"), region)?;
        let clouds = load_geo_image(&res.join("clouds-great-lakes-2000.webp"), region)?;
        let models = parse_models(
            &String::from_utf8(read(&res.join("voxel-models.json"))?).map_err(|e| e.to_string())?,
        )?;
        let buildings = TEST_CITIES
            .iter()
            .map(|&(lon, lat, m, yaw)| Placement {
                model: m.into(),
                lon,
                lat,
                yaw,
            })
            .collect();
        Ok(Scene {
            projection: Projection {
                lon0: -82.0,
                lat0: 41.5,
            },
            heightmap,
            mask,
            earth,
            clouds,
            models,
            buildings,
            voxel_km: 2.2,
            exaggeration: 36.0,
            terrace_m: 60.0,
        })
    }

    /// A few kilobytes of synthetic data for tests: a ridge, a lake, one building.
    pub fn tiny() -> Scene {
        let bounds = Bounds {
            west: -84.0,
            south: 40.0,
            east: -82.0,
            north: 42.0,
        };
        let (w, h) = (16usize, 16usize);
        let mut metres = vec![0i16; w * h];
        for r in 0..h {
            for c in 0..w {
                metres[r * w + c] = if c < 6 { 200 + (c as i16) * 40 } else { 180 };
            }
        }
        let mut rgba = vec![0u8; 64 * 64 * 4];
        for y in 0..64 {
            for x in 0..64 {
                let i = (y * 64 + x) * 4;
                rgba[i + 2] = if x >= 44 { WATER_LAKE } else { WATER_NONE };
                rgba[i + 3] = if x < 20 { TINT_PLAYER } else { TINT_PLAY };
            }
        }
        let mask = Mask {
            w: 64,
            h: 64,
            bounds,
            rgba,
        };
        let flat = |c: [u8; 4]| GeoImage {
            w: 4,
            h: 4,
            bounds,
            rgba: c.repeat(16),
        };
        let mut models = HashMap::new();
        models.insert(
            "tiny".to_string(),
            VoxelModel {
                boxes: vec![
                    VoxelBox {
                        min: [-3.0, -3.0, 0.0],
                        max: [3.0, 3.0, 1.0],
                        colour: [0x6F, 0xA8, 0x3B],
                    },
                    VoxelBox {
                        min: [-2.0, -2.0, 1.0],
                        max: [2.0, 2.0, 6.0],
                        colour: [0xB2, 0x3A, 0x2E],
                    },
                ],
                footprint: [6.0, 6.0],
            },
        );
        Scene {
            projection: Projection {
                lon0: -83.0,
                lat0: 41.0,
            },
            heightmap: Heightmap {
                w,
                h,
                bounds,
                metres,
            },
            mask,
            earth: flat([40, 60, 22, 255]),
            clouds: flat([255, 255, 255, 0]),
            models,
            buildings: vec![Placement {
                model: "tiny".into(),
                lon: -83.2,
                lat: 41.0,
                yaw: 0.0,
            }],
            voxel_km: 3.0,
            exaggeration: 36.0,
            terrace_m: 45.0,
        }
    }
}
