//! Scene data: terrain, the rasterised map, textures and voxel buildings. Everything is plain
//! CPU data; nothing here touches the GPU, the engine or the network.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::ground::Ground;
use crate::palette::Rgb;

/// Kilometres per degree of latitude, and of longitude at the scenario's reference latitude.
pub const KM_PER_DEG_LAT: f32 = 110.57;
/// Nominal kilometres per voxel before a building is sized to its real footprint.
pub const VOXEL_KM: f32 = 2.4;
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

/// Whose building it is. The scenery (downtown blocks) is nobody's; the player's own buildings
/// are gold roofed and rivals keep their sector's colours.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Owner {
    #[default]
    Scenery,
    Player,
    Rival,
}

/// A building placed on the map. `yaw` turns it in degrees (0 faces south).
#[derive(Clone, Debug)]
pub struct Placement {
    pub model: String,
    pub lon: f32,
    pub lat: f32,
    pub yaw: f32,
    /// Multiplier of the scene's voxel size. Set from the building's real size in kilometres
    /// (`Placement::sized`), so a building is the same size on the ground at every zoom.
    pub scale: f32,
    pub owner: Owner,
}

impl Placement {
    /// The `scale` that makes `model` stand `size_km` across on its longest side.
    pub fn scale_for_km(voxel_km: f32, model: &VoxelModel, size_km: f32) -> f32 {
        size_km / (model.footprint[0].max(model.footprint[1]).max(1.0) * voxel_km)
    }
}

/// Road classes of [`Line`]: the 1.8 map's interstates, its main roads and its railways.
pub const LINE_INTERSTATE: u8 = 0;
pub const LINE_MAIN: u8 = 1;
pub const LINE_RAIL: u8 = 2;

/// A polyline of the map's own road and railway data, in degrees.
#[derive(Clone, Debug)]
pub struct Line {
    pub class: u8,
    pub points: Vec<(f32, f32)>,
}

/// A city of the map's own data, for the dense downtown ground and the downtown blocks.
#[derive(Clone, Debug)]
pub struct City {
    pub lon: f32,
    pub lat: f32,
    pub population: u64,
}

pub struct Scene {
    pub projection: Projection,
    pub heightmap: Heightmap,
    pub mask: Mask,
    pub earth: GeoImage,
    pub clouds: GeoImage,
    pub models: HashMap<String, VoxelModel>,
    pub buildings: Vec<Placement>,
    /// Interstates, main roads and railways from the map data the apps draw.
    pub lines: Vec<Line>,
    pub cities: Vec<City>,
    /// Kilometres per voxel of the buildings.
    pub voxel_km: f32,
    /// Vertical exaggeration of terrain, and the terrace step in metres.
    pub exaggeration: f32,
    pub terrace_m: f32,
    /// Shifts where the steps fall, so the common heights of the Midwest and the lake surfaces
    /// share one terrace and flat country gets as few walls as possible.
    pub terrace_offset_m: f32,
    /// Land cover as clean patches, and the smoothed terrace level of every heightmap cell
    /// (row major). Both are derived from the fields above by `finish`.
    pub ground: Ground,
    pub levels: Vec<i32>,
    /// Week of the year the snow is drawn for (1 to 52), and a counter bumped when it changes so
    /// the renderer repaints only the ground and the apron.
    pub week: u32,
    pub season_rev: u64,
    /// The scenario's regions in mask order: region `i` is painted `i + 1` in the mask's red
    /// channel. Hub positions are where a region's own buildings stand.
    pub regions: Vec<RegionInfo>,
    /// Bumped when the mask tints or the building placements change, so the renderer re-uploads
    /// only what moved.
    pub mask_rev: u64,
    pub buildings_rev: u64,
}

/// One playable region of a scenario.
#[derive(Clone, Debug)]
pub struct RegionInfo {
    pub code: String,
    pub lon: f32,
    pub lat: f32,
}

/// Tint classes of the mask's alpha channel: 1 play, 2 the player's holdings (gold), and
/// `TINT_HEAT + n` for map modes, where `n` is 0 to 3 (low to high).
pub const TINT_HEAT: u8 = 3;

impl Scene {
    /// Paint the regions' tint classes (region code to class). Codes not listed become `TINT_PLAY`.
    pub fn set_region_tints(&mut self, tints: &HashMap<String, u8>) {
        let mut lut = vec![TINT_PLAY; 256];
        for (i, r) in self.regions.iter().enumerate() {
            lut[i + 1] = tints.get(&r.code).copied().unwrap_or(TINT_PLAY);
        }
        for p in self.mask.rgba.chunks_exact_mut(4) {
            if p[0] > 0 {
                p[3] = lut[p[0] as usize];
            }
        }
        self.mask_rev += 1;
    }

    /// Set the season: snow follows the week of the year, by latitude and height.
    pub fn set_week(&mut self, week: u32) {
        let week = week.clamp(1, 52);
        if week != self.week {
            self.week = week;
            self.ground.set_week(week);
            self.season_rev += 1;
        }
    }

    /// Replace the placed buildings and mark them for upload.
    pub fn set_buildings(&mut self, buildings: Vec<Placement>) {
        self.buildings = buildings;
        self.buildings_rev += 1;
    }

    /// The region at a position, from the same mask the renderer draws.
    pub fn region_at(&self, lon: f32, lat: f32) -> Option<&RegionInfo> {
        let p = self.mask.at(lon, lat);
        if p[0] == 0 || p[2] == WATER_OCEAN || p[2] == WATER_LAKE {
            return None;
        }
        self.regions.get(p[0] as usize - 1)
    }

    /// Derive the ground classes and terrace levels. Call after the raw fields are filled.
    fn finish(mut self) -> Scene {
        self.levels = crate::mesh::build_levels(&self);
        self.ground = crate::ground::build(&self);
        self
    }

    /// Terrace level of a heightmap cell.
    pub fn level_at(&self, col: usize, row: usize) -> i32 {
        self.levels
            [row.min(self.heightmap.h - 1) * self.heightmap.w + col.min(self.heightmap.w - 1)]
    }
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
    hubs: HashMap<String, [i32; 2]>,
    #[serde(default)]
    roads: Vec<Vec<i32>>,
    #[serde(default)]
    interstates: Vec<RawLines>,
    #[serde(default)]
    railroads: Vec<RawLines>,
}
#[derive(Deserialize)]
struct RawLines {
    lines: Vec<Vec<i32>>,
    #[serde(default)]
    scenario: Option<String>,
}
#[derive(Deserialize)]
struct RawUrban {
    areas: Vec<RawRings>,
}

/// The eight states of the Great Lakes 2000 scenario.
pub const GREAT_LAKES_STATES: [&str; 8] = ["IL", "IN", "KY", "MI", "NY", "OH", "PA", "WV"];
/// Regions drawn as the player's holdings in the spike.
pub const PLAYER_REGIONS: [&str; 5] = ["MI-DET", "IL-CHI", "OH-CEN", "OH-SW", "PA-WES"];
/// Sector models for the hub buildings, picked by position in the region list. The large
/// models go to the big anchor cities.
const HUB_MODELS: [&str; 10] = [
    "manufacturing/2",
    "energy/1",
    "logistics/1",
    "technology/1",
    "retail/1",
    "chemical_industries/1",
    "healthcare/1",
    "media/1",
    "extraction/1",
    "agriculture/2",
];
const BIG_HUBS: [(&str, &str); 4] = [
    ("IL-CHI", "financial/2"),
    ("NY-NYC", "financial/2"),
    ("MI-DET", "manufacturing/2"),
    ("PA-PHL", "financial/1"),
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
pub fn rasterise_mask(
    map_json: &str,
    urban_json: Option<&str>,
    bounds: Bounds,
    ppd: f32,
    states: &[&str],
    player_regions: &[&str],
) -> Result<Mask, String> {
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
    for (i, r) in map
        .regions
        .iter()
        .filter(|r| states.contains(&r.state.as_str()))
        .enumerate()
    {
        let n = (i + 1) as u8;
        let tint = if player_regions.contains(&r.code.as_str()) {
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
    // Real built-up areas (Natural Earth urban areas), land only.
    if let Some(u) = urban_json {
        let urban: RawUrban = serde_json::from_str(u).map_err(|e| e.to_string())?;
        for a in urban
            .areas
            .iter()
            .filter(|a| ring_touches(&a.rings, &bounds))
        {
            fill_rings(&rings_to_px(&a.rings, &bounds, ppd), w, h, |x, y| {
                if px[idx(x, y) + 2] == WATER_NONE {
                    px[idx(x, y) + 2] = WATER_URBAN;
                }
            });
        }
    }
    // Cities and region hubs: a blobby fringe of urban ground sized from population, on land
    // only. The outline wobbles with the angle so footprints read as places, not discs.
    let (kx, ky) = (
        (KM_PER_DEG_LON_EQ * ((bounds.south + bounds.north) / 2.0).to_radians().cos()),
        KM_PER_DEG_LAT,
    );
    let mut sites: Vec<(f32, f32, f32)> = map
        .cities
        .iter()
        .map(|c| {
            (
                c.x as f32 / 1e4,
                c.y as f32 / 1e4,
                3.0 + 8.0 * (c.population as f32 / 1.0e6).sqrt(),
            )
        })
        .collect();
    for (code, p) in &map.hubs {
        if states.contains(&&code[..2]) {
            sites.push((p[0] as f32 / 1e4, p[1] as f32 / 1e4, 4.0));
        }
    }
    for (lon, lat, r_km) in sites {
        if lon < bounds.west || lon > bounds.east || lat < bounds.south || lat > bounds.north {
            continue;
        }
        let seed = (lon * 131.0 + lat * 71.0).sin() * 43758.5;
        let ph = [
            seed.fract() * std::f32::consts::TAU,
            (seed * 1.7).fract() * std::f32::consts::TAU,
            (seed * 2.3).fract() * std::f32::consts::TAU,
        ];
        let (cx, cy) = ((lon - bounds.west) * ppd, (bounds.north - lat) * ppd);
        let reach = r_km * 1.5;
        let (rx, ry) = (reach / kx * ppd, reach / ky * ppd);
        for y in (cy - ry).max(0.0) as usize..((cy + ry) as usize + 1).min(h) {
            for x in (cx - rx).max(0.0) as usize..((cx + rx) as usize + 1).min(w) {
                let (dx, dy) = ((x as f32 - cx) / ppd * kx, (y as f32 - cy) / ppd * ky);
                let a = dy.atan2(dx);
                let wob = 1.0
                    + 0.22 * (3.0 * a + ph[0]).sin()
                    + 0.14 * (5.0 * a + ph[1]).sin()
                    + 0.08 * (8.0 * a + ph[2]).sin();
                if (dx * dx + dy * dy).sqrt() <= r_km * wob && px[idx(x, y) + 2] == WATER_NONE {
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

/// Soften the cloud alpha so thresholding gives smooth, speckle free shapes.
fn blur_alpha(img: &mut GeoImage, radius: usize, passes: usize) {
    let (w, h) = (img.w as usize, img.h as usize);
    let mut a: Vec<f32> = img.rgba.chunks_exact(4).map(|p| p[3] as f32).collect();
    for _ in 0..passes {
        let mut tmp = vec![0.0f32; w * h];
        for y in 0..h {
            for x in 0..w {
                let (lo, hi) = (x.saturating_sub(radius), (x + radius).min(w - 1));
                let s: f32 = (lo..=hi).map(|i| a[y * w + i]).sum();
                tmp[y * w + x] = s / (hi - lo + 1) as f32;
            }
        }
        for y in 0..h {
            let (lo, hi) = (y.saturating_sub(radius), (y + radius).min(h - 1));
            for x in 0..w {
                let s: f32 = (lo..=hi).map(|j| tmp[j * w + x]).sum();
                a[y * w + x] = s / (hi - lo + 1) as f32;
            }
        }
    }
    for (p, v) in img.rgba.chunks_exact_mut(4).zip(a) {
        p[3] = v.round().clamp(0.0, 255.0) as u8;
    }
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

/// The map's roads and railways that touch `bounds`, tagged with their class. Interstates and
/// railways carry a scenario tag in the map file; untagged ones belong to the default scenario.
fn collect_lines(map: &RawMap, scenario: &str, b: &Bounds) -> Vec<Line> {
    let mut out = Vec::new();
    let mut push = |class: u8, flat: &[i32]| {
        let pts: Vec<(f32, f32)> = flat
            .chunks_exact(2)
            .map(|p| (p[0] as f32 / 1e4, p[1] as f32 / 1e4))
            .collect();
        let inside = pts
            .iter()
            .any(|p| p.0 >= b.west && p.0 <= b.east && p.1 >= b.south && p.1 <= b.north);
        if pts.len() >= 2 && inside {
            out.push(Line { class, points: pts });
        }
    };
    for r in &map.roads {
        push(LINE_MAIN, r);
    }
    for (class, set) in [
        (LINE_INTERSTATE, &map.interstates),
        (LINE_RAIL, &map.railroads),
    ] {
        for g in set {
            if g.scenario.as_deref().is_none_or(|s| s == scenario) {
                for l in &g.lines {
                    push(class, l);
                }
            }
        }
    }
    out
}

/// Footprint of one downtown block in kilometres: a real city block cluster, not a district.
const BLOCK_KM: f32 = 1.1;

/// A few small voxel blocks round the centre of every big city: more blocks for bigger cities,
/// each picked and turned from the city's own position so the same map always builds the same
/// skyline. The blocks stand inside the city's downtown, whose radius follows its population
/// (the same radius the dense downtown ground uses).
fn downtown_blocks(
    cities: &[City],
    models: &HashMap<String, VoxelModel>,
    lat: f32,
    voxel_km: f32,
) -> Vec<Placement> {
    const KINDS: [&str; 6] = [
        "financial/1",
        "city/1",
        "real_estate/2",
        "technology/1",
        "city/2",
        "telecommunications/1",
    ];
    let kx = KM_PER_DEG_LON_EQ * lat.to_radians().cos();
    let mut out = Vec::new();
    for c in cities.iter().filter(|c| c.population >= 600_000) {
        let n = (2.0 + (c.population as f32 / 1.0e6).sqrt()).min(7.0) as usize;
        let reach = 2.0 + 3.2 * (c.population as f32 / 1.0e6).sqrt();
        let seed = ((c.lon * 977.0 + c.lat * 313.0).sin() * 43758.5)
            .fract()
            .abs();
        for i in 0..n {
            let a = (seed + i as f32 / n as f32) * std::f32::consts::TAU;
            let ring = if i == 0 {
                0.0
            } else {
                reach * (0.3 + 0.12 * ((i * 7) % 5) as f32)
            };
            let kind = KINDS[((seed * 97.0) as usize + i * 5) % KINDS.len()];
            let Some(model) = models.get(kind) else {
                continue;
            };
            out.push(Placement {
                model: kind.into(),
                lon: c.lon + a.cos() * ring / kx,
                lat: c.lat + a.sin() * ring / KM_PER_DEG_LAT,
                yaw: (((seed * 31.0) as usize + i) % 4) as f32 * 90.0,
                scale: Placement::scale_for_km(voxel_km, model, BLOCK_KM),
                owner: Owner::Scenery,
            });
        }
    }
    out
}

fn empty_ground(bounds: Bounds) -> Ground {
    Ground {
        w: 0,
        h: 0,
        bounds,
        ppd: 1.0,
        classes: Vec::new(),
        base: Vec::new(),
        elev: Vec::new(),
        variant: Vec::new(),
    }
}

/// What differs between the four scenarios: where the terrain and satellite images sit and which
/// states are in play. The game's scenarios are listed in [`SCENARIOS`].
#[derive(Clone, Copy, Debug)]
pub struct ScenarioSpec {
    pub id: &'static str,
    pub states: &'static [&'static str],
    /// Box of the earth and cloud images (the same boxes the classic map uses).
    pub image_bounds: Bounds,
    /// Where the flat projection is anchored, and where the camera starts.
    pub centre: (f32, f32),
    pub start_width_km: f32,
}

const fn bounds(west: f32, south: f32, east: f32, north: f32) -> Bounds {
    Bounds {
        west,
        south,
        east,
        north,
    }
}

pub const SCENARIOS: [ScenarioSpec; 4] = [
    ScenarioSpec {
        id: "great-lakes-2000",
        states: &GREAT_LAKES_STATES,
        image_bounds: bounds(-122.0, 18.0, -42.0, 66.0),
        centre: (-82.0, 41.5),
        start_width_km: 2250.0,
    },
    ScenarioSpec {
        id: "texas-1981",
        states: &["LA", "OK", "TX"],
        image_bounds: bounds(-137.0, 8.0, -57.0, 56.0),
        centre: (-97.5, 31.0),
        start_width_km: 2300.0,
    },
    ScenarioSpec {
        id: "silicon-valley-1995",
        states: &["CA"],
        image_bounds: bounds(-160.0, 13.0, -80.0, 61.0),
        centre: (-119.5, 37.0),
        start_width_km: 1500.0,
    },
    ScenarioSpec {
        id: "ruhr-rhine-1991",
        states: &["BE", "DE", "FR", "LU", "NL"],
        image_bounds: bounds(-32.0, 28.0, 48.0, 76.0),
        centre: (8.0, 51.0),
        start_width_km: 1500.0,
    },
];

impl ScenarioSpec {
    pub fn find(id: &str) -> Option<&'static ScenarioSpec> {
        SCENARIOS.iter().find(|s| s.id == id)
    }
}

/// Which files to read. The repository keeps them in two places and the app ships them in one
/// folder, so both are given.
pub struct AssetDirs<'a> {
    /// world-map.json, earth-*.jpg, clouds-*.webp, voxel-models.json
    pub resources: &'a Path,
    /// heightmap-*.cwh and urban-*.json
    pub baked: &'a Path,
}

/// How a scene starts: which regions are drawn as the player's holdings, and whether every
/// region hub gets a building (the spike's look; the game places its own plants instead).
#[derive(Default)]
pub struct SceneOptions {
    pub player_regions: Vec<String>,
    pub hub_buildings: bool,
}

impl Scene {
    /// The Great Lakes 2000 scenario from the repository's own assets. `repo` is the
    /// repository root; the heightmap comes from crates/cw-render/assets.
    pub fn great_lakes_2000(repo: &Path) -> Result<Scene, String> {
        Scene::for_scenario(
            &AssetDirs {
                resources: &repo.join("apple/Resources"),
                baked: &repo.join("crates/cw-render/assets"),
            },
            &SCENARIOS[0],
            &SceneOptions {
                player_regions: PLAYER_REGIONS.iter().map(|s| s.to_string()).collect(),
                hub_buildings: true,
            },
        )
    }

    /// Build a scenario's scene: terrain, region mask, land cover, roads, cities.
    pub fn for_scenario(
        dirs: &AssetDirs,
        spec: &ScenarioSpec,
        options: &SceneOptions,
    ) -> Result<Scene, String> {
        let res = dirs.resources;
        let read = |p: &Path| std::fs::read(p).map_err(|e| format!("{}: {e}", p.display()));
        let heightmap = Heightmap::parse(&read(
            &dirs.baked.join(format!("heightmap-{}.cwh", spec.id)),
        )?)?;
        let map_json =
            String::from_utf8(read(&res.join("world-map.json"))?).map_err(|e| e.to_string())?;
        let urban_json =
            std::fs::read_to_string(dirs.baked.join(format!("urban-{}.json", spec.id))).ok();
        let player: Vec<&str> = options.player_regions.iter().map(|s| s.as_str()).collect();
        let mask = rasterise_mask(
            &map_json,
            urban_json.as_deref(),
            heightmap.bounds,
            120.0,
            spec.states,
            &player,
        )?;
        let region = spec.image_bounds;
        let earth = load_geo_image(&res.join(format!("earth-{}.jpg", spec.id)), region)?;
        let mut clouds = load_geo_image(&res.join(format!("clouds-{}.webp", spec.id)), region)?;
        blur_alpha(&mut clouds, 2, 2);
        let models = parse_models(
            &String::from_utf8(read(&res.join("voxel-models.json"))?).map_err(|e| e.to_string())?,
        )?;
        let map: RawMap = serde_json::from_str(&map_json).map_err(|e| e.to_string())?;
        let mut codes: Vec<&String> = map
            .hubs
            .keys()
            .filter(|c| spec.states.contains(&&c[..2]))
            .collect();
        codes.sort();
        let mut buildings: Vec<Placement> = if options.hub_buildings {
            codes
                .iter()
                .enumerate()
                .map(|(i, code)| {
                    let p = map.hubs[*code];
                    let model = BIG_HUBS
                        .iter()
                        .find(|(c, _)| c == code)
                        .map(|(_, m)| *m)
                        .unwrap_or(HUB_MODELS[i % HUB_MODELS.len()]);
                    Placement {
                        model: model.into(),
                        lon: p[0] as f32 / 1e4,
                        lat: p[1] as f32 / 1e4,
                        yaw: 0.0,
                        scale: models
                            .get(model)
                            .map_or(1.0, |m| Placement::scale_for_km(VOXEL_KM, m, 2.5)),
                        owner: Owner::Rival,
                    }
                })
                .collect()
        } else {
            Vec::new()
        };
        let regions: Vec<RegionInfo> = map
            .regions
            .iter()
            .filter(|r| spec.states.contains(&r.state.as_str()))
            .map(|r| {
                let h = map.hubs.get(&r.code).copied().unwrap_or([0, 0]);
                RegionInfo {
                    code: r.code.clone(),
                    lon: h[0] as f32 / 1e4,
                    lat: h[1] as f32 / 1e4,
                }
            })
            .collect();
        let hb = heightmap.bounds;
        let cities: Vec<City> = map
            .cities
            .iter()
            .map(|c| City {
                lon: c.x as f32 / 1e4,
                lat: c.y as f32 / 1e4,
                population: c.population,
            })
            .filter(|c| {
                c.lon >= hb.west && c.lon <= hb.east && c.lat >= hb.south && c.lat <= hb.north
            })
            .collect();
        let mid_lat = (hb.south + hb.north) / 2.0;
        buildings.extend(
            downtown_blocks(&cities, &models, mid_lat, VOXEL_KM)
                .into_iter()
                .filter(|b| {
                    let kind = mask.at(b.lon, b.lat)[2];
                    kind == WATER_NONE || kind == WATER_URBAN
                }),
        );
        let lines = collect_lines(&map, spec.id, &hb);

        Ok(Scene {
            projection: Projection {
                lon0: spec.centre.0,
                lat0: spec.centre.1,
            },
            heightmap,
            mask,
            earth,
            clouds,
            models,
            buildings,
            lines,
            cities,
            voxel_km: VOXEL_KM,
            exaggeration: 28.0,
            terrace_m: 200.0,
            terrace_offset_m: 60.0,
            ground: empty_ground(hb),
            levels: Vec::new(),
            week: 28,
            season_rev: 0,
            regions,
            mask_rev: 0,
            buildings_rev: 0,
        }
        .finish())
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
                rgba[i] = if x < 20 { 1 } else { 2 };
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
                scale: 1.0,
                owner: Owner::Rival,
            }],
            lines: vec![
                // An interstate across the ridge and its terrace walls, a main road and a railway.
                Line {
                    class: LINE_INTERSTATE,
                    points: vec![
                        (-83.95, 41.3),
                        (-83.4, 41.32),
                        (-82.9, 41.28),
                        (-82.1, 41.3),
                    ],
                },
                Line {
                    class: LINE_MAIN,
                    points: vec![(-83.9, 40.6), (-83.0, 40.62), (-82.2, 40.7)],
                },
                Line {
                    class: LINE_RAIL,
                    points: vec![(-83.9, 41.7), (-83.0, 41.66), (-82.2, 41.7)],
                },
            ],
            cities: vec![City {
                lon: -83.3,
                lat: 41.0,
                population: 1_500_000,
            }],
            voxel_km: 3.0,
            exaggeration: 10.0,
            terrace_m: 45.0,
            terrace_offset_m: 0.0,
            ground: empty_ground(bounds),
            levels: Vec::new(),
            week: 28,
            season_rev: 0,
            regions: vec![
                RegionInfo {
                    code: "AA-ONE".into(),
                    lon: -83.5,
                    lat: 41.0,
                },
                RegionInfo {
                    code: "AA-TWO".into(),
                    lon: -82.4,
                    lat: 41.0,
                },
            ],
            mask_rev: 0,
            buildings_rev: 0,
        }
        .finish()
    }
}
