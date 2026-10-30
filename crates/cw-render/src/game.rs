//! Turning game facts into scene data: where each firm's plants stand and how the regions are
//! tinted. Plain inputs only, so the renderer stays free of the engine crates; the FFI layer
//! converts the session's plants into these.

use std::collections::HashMap;

use crate::data::{KM_PER_DEG_LAT, Placement, Scene, TINT_HEAT, TINT_PLAY, TINT_PLAYER};

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
const RING_KM: f32 = 6.5;

/// The buildings for `sites`, each beside its region's hub. The same sites always give the same
/// layout, so a rebuild each week moves only what changed.
pub fn layout_plants(scene: &Scene, sites: &[PlantSite]) -> Vec<Placement> {
    let mut by_region: HashMap<&str, Vec<&PlantSite>> = HashMap::new();
    for s in sites {
        by_region.entry(s.region.as_str()).or_default().push(s);
    }
    let mut regions: Vec<&&str> = by_region.keys().collect();
    regions.sort();
    let kx = scene.projection.km_per_deg_lon();
    let mut out = Vec::new();
    for code in regions {
        let Some(info) = scene.regions.iter().find(|r| r.code == **code) else {
            continue;
        };
        let mut list = by_region[*code].clone();
        list.sort_by(|a, b| {
            b.player
                .cmp(&a.player)
                .then(b.plants.cmp(&a.plants))
                .then(a.firm.cmp(&b.firm))
                .then(a.industry.cmp(&b.industry))
        });
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
            let (dx, dy) = if i == 0 {
                (0.0, 0.0)
            } else {
                let a = (i as f32 - 1.0) * std::f32::consts::TAU / 5.0 + 0.4;
                (a.cos() * RING_KM, a.sin() * RING_KM)
            };
            out.push(Placement {
                model,
                lon: info.lon + dx / kx,
                lat: info.lat + dy / KM_PER_DEG_LAT,
                yaw: 0.0,
                scale: 1.0,
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
