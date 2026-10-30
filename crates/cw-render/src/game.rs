//! Turning game facts into scene data: where each firm's plants stand and how the regions are
//! tinted. Plain inputs only, so the renderer stays free of the engine crates; the FFI layer
//! converts the session's plants into these.

use std::collections::HashMap;

use crate::data::{KM_PER_DEG_LAT, Owner, Placement, Scene, TINT_HEAT, TINT_PLAY, TINT_PLAYER};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteStatus {
    /// Still being built: a construction site.
    Site,
    Running,
    Mothballed,
}

/// A firm's plants of one industry in one region, which the map draws as one building.
#[derive(Clone, Debug)]
pub struct PlantSite {
    pub region: String,
    pub industry: String,
    pub firm: u64,
    pub plants: u32,
    pub player: bool,
    pub status: SiteStatus,
}

/// Small up to three plants, medium up to eight, large beyond (the classic map's rule).
fn size_of(plants: u32) -> u32 {
    match plants {
        0..=3 => 0,
        4..=8 => 1,
        _ => 2,
    }
}

/// At most this many buildings stand at one region's hub: the player's first, then the biggest.
const PER_REGION: usize = 6;
/// Distance of the ring the buildings stand on, in km, round the hub building.
const RING_KM: f32 = 4.0;
/// Clear ground kept round a building's footprint, and between neighbours, in km.
const MARGIN_KM: f32 = 0.15;

/// Footprint of a plant in kilometres. A real plant is a few hundred metres to two km across:
/// the sector sets the base (a refinery or a mill is bigger than an office tower), each further
/// plant of the division adds to it, and a construction site is a small yard. The most any
/// building gets is 2 km.
pub fn plant_size_km(industry: &str, plants: u32, site: bool) -> f32 {
    if site {
        return 0.35;
    }
    let base = match industry {
        "energy" | "chemical_industries" | "manufacturing" | "extraction" | "agriculture" => 0.8,
        "logistics" | "construction" | "defense" => 0.7,
        _ => 0.55,
    };
    (base + 0.14 * plants.saturating_sub(1).min(8) as f32).min(2.0)
}

/// Where a region's land is centred, for buildings that must be pulled off the water.
fn land_centroids(scene: &Scene) -> Vec<Option<(f32, f32)>> {
    let m = &scene.mask;
    let mut sums = vec![(0.0f64, 0.0f64, 0u64); scene.regions.len()];
    for y in 0..m.h {
        for x in 0..m.w {
            let i = (y * m.w + x) * 4;
            let r = m.rgba[i] as usize;
            if r == 0
                || r > sums.len()
                || m.rgba[i + 2] == crate::data::WATER_OCEAN
                || m.rgba[i + 2] == crate::data::WATER_LAKE
            {
                continue;
            }
            let s = &mut sums[r - 1];
            s.0 += x as f64;
            s.1 += y as f64;
            s.2 += 1;
        }
    }
    let ppd = m.px_per_degree() as f64;
    sums.iter()
        .map(|&(sx, sy, n)| {
            (n > 0).then(|| {
                (
                    m.bounds.west + (sx / n as f64 / ppd) as f32,
                    m.bounds.north - (sy / n as f64 / ppd) as f32,
                )
            })
        })
        .collect()
}

/// A candidate standing place for a building `size_km` across: all land, clear of the buildings
/// already placed in the region (`taken`: centre in km offsets from the hub and size).
#[allow(clippy::too_many_arguments)]
fn fits(
    scene: &Scene,
    region_index: usize,
    lon: f32,
    lat: f32,
    size_km: f32,
    hub: (f32, f32),
    taken: &[(f32, f32, f32)],
    inside: bool,
) -> bool {
    let kx = scene.projection.km_per_deg_lon();
    let half = size_km / 2.0 + MARGIN_KM;
    if !scene.mask.footprint_on_land(lon, lat, half, kx) {
        return false;
    }
    if inside && scene.mask.at(lon, lat)[0] as usize != region_index + 1 {
        return false;
    }
    let (x, y) = ((lon - hub.0) * kx, (lat - hub.1) * KM_PER_DEG_LAT);
    taken
        .iter()
        .all(|&(tx, ty, ts)| (x - tx).abs().max((y - ty).abs()) >= (size_km + ts) / 2.0 + MARGIN_KM)
}

/// The buildings for `sites`, each beside its region's hub and always on land. The same sites
/// always give the same layout, so a rebuild each week moves only what changed. A building whose
/// ideal spot is water or taken slides round the ring, then inward, then outward from the hub,
/// then toward the region's land centre, until its whole footprint is dry and clear.
pub fn layout_plants(scene: &Scene, sites: &[PlantSite]) -> Vec<Placement> {
    let mut by_region: HashMap<&str, Vec<&PlantSite>> = HashMap::new();
    for s in sites {
        by_region.entry(s.region.as_str()).or_default().push(s);
    }
    let mut regions: Vec<&&str> = by_region.keys().collect();
    regions.sort();
    let kx = scene.projection.km_per_deg_lon();
    let centroids = land_centroids(scene);
    let mut out = Vec::new();
    for code in regions {
        let Some(region_index) = scene.regions.iter().position(|r| r.code == **code) else {
            continue;
        };
        let info = &scene.regions[region_index];
        let hub = (info.lon, info.lat);
        let mut list = by_region[*code].clone();
        list.sort_by(|a, b| {
            b.player
                .cmp(&a.player)
                .then(b.plants.cmp(&a.plants))
                .then(a.firm.cmp(&b.firm))
                .then(a.industry.cmp(&b.industry))
        });
        let mut taken: Vec<(f32, f32, f32)> = Vec::new();
        for (i, s) in list.into_iter().take(PER_REGION).enumerate() {
            let size = size_of(s.plants);
            let kind = if s.status == SiteStatus::Site {
                "site"
            } else {
                s.industry.as_str()
            };
            let mut model = format!("{kind}/{size}");
            if !scene.models.contains_key(&model) {
                model = format!("city/{size}");
            }
            let size_km = plant_size_km(&s.industry, s.plants, s.status == SiteStatus::Site);
            let angle = if i == 0 {
                0.0
            } else {
                (i as f32 - 1.0) * std::f32::consts::TAU / 5.0 + 0.4
            };
            let ring = if i == 0 { 0.0 } else { RING_KM };
            let spot = |a: f32, r: f32| {
                (
                    hub.0 + a.cos() * r / kx,
                    hub.1 + a.sin() * r / KM_PER_DEG_LAT,
                )
            };
            // Candidates in a fixed order: the ideal spot, round the ring and inward, outward
            // from the hub, then along the line to the region's land centre.
            let mut candidates = vec![spot(angle, ring)];
            for r in [ring, ring * 0.6, ring * 0.3, ring * 1.6, ring * 2.4] {
                if r == 0.0 {
                    continue;
                }
                for k in 1..=12 {
                    let off = (k as f32 + 1.0).div_euclid(2.0) * std::f32::consts::TAU / 24.0
                        * if k % 2 == 1 { 1.0 } else { -1.0 };
                    candidates.push(spot(angle + off, r));
                }
            }
            for r in 1..=20 {
                for k in 0..16 {
                    candidates.push(spot(
                        k as f32 * std::f32::consts::TAU / 16.0,
                        r as f32 * 1.5,
                    ));
                }
            }
            if let Some(c) = centroids[region_index] {
                for t in 0..=10 {
                    let f = t as f32 / 10.0;
                    candidates.push((hub.0 + (c.0 - hub.0) * f, hub.1 + (c.1 - hub.1) * f));
                }
            }
            let pick = |inside: bool| {
                candidates.iter().copied().find(|&(lon, lat)| {
                    fits(scene, region_index, lon, lat, size_km, hub, &taken, inside)
                })
            };
            let (lon, lat) = pick(true)
                .or_else(|| pick(false))
                .or_else(|| centroids[region_index])
                .unwrap_or(candidates[0]);
            taken.push(((lon - hub.0) * kx, (lat - hub.1) * KM_PER_DEG_LAT, size_km));
            let scale = scene
                .models
                .get(&model)
                .map_or(1.0, |m| Placement::scale_for_km(scene.voxel_km, m, size_km));
            out.push(Placement {
                model,
                lon,
                lat,
                yaw: 0.0,
                scale,
                owner: if s.player {
                    Owner::Player
                } else {
                    Owner::Rival
                },
            });
        }
    }
    out
}

/// The ramp names the interface may ask for a map mode's colours.
pub fn ramp_by_name(name: &str) -> usize {
    use crate::palette::ramp;
    match name {
        "water" => ramp::WATER,
        "forest" => ramp::FOREST,
        "field" => ramp::FIELD,
        "soil" => ramp::SOIL,
        "urban" => ramp::URBAN,
        "gold" => ramp::GOLD,
        "signal" => ramp::SIGNAL,
        _ => ramp::BRICK,
    }
}

/// Tint class of every region: map-mode levels (0 to 3) win, then the player's holdings (gold),
/// then plain play.
pub fn region_tints(
    scene: &Scene,
    holdings: &[String],
    heat: &HashMap<String, u8>,
) -> HashMap<String, u8> {
    scene
        .regions
        .iter()
        .map(|r| {
            let class = if let Some(level) = heat.get(&r.code) {
                TINT_HEAT + (*level).min(3)
            } else if holdings.contains(&r.code) {
                TINT_PLAYER
            } else {
                TINT_PLAY
            };
            (r.code.clone(), class)
        })
        .collect()
}
