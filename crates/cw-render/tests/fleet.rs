//! Vehicles, routes from freight lanes, harbours and the selection outline. No GPU needed except
//! where a frame is drawn.

use std::path::Path;

use cw_render::{
    AssetDirs, Lane, PlantSite, ROAD_BANDS, ScenarioSpec, Scene, SceneOptions, SiteStatus,
    harbour_sites, layout_plants, river_chains, road_chains_for_band, vehicles_at,
};

fn load(id: &str) -> Scene {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Scene::for_scenario(
        &AssetDirs {
            resources: &repo.join("apple/Resources"),
            baked: &repo.join("crates/cw-render/assets"),
        },
        ScenarioSpec::find(id).unwrap(),
        &SceneOptions::default(),
    )
    .unwrap()
}

/// Lanes between every pair of regions within 600 km, in all three modes.
fn lanes_for(scene: &Scene) -> Vec<Lane> {
    let km = scene.projection.km_per_deg_lon();
    let mut out = Vec::new();
    for (i, a) in scene.regions.iter().enumerate() {
        for b in scene.regions.iter().skip(i + 1) {
            let d = (((a.lon - b.lon) * km).powi(2) + ((a.lat - b.lat) * 111.0).powi(2)).sqrt();
            if d > 600.0 {
                continue;
            }
            for mode in ["truck", "rail", "sea"] {
                out.push(Lane {
                    from: a.code.clone(),
                    to: b.code.clone(),
                    mode: mode.into(),
                    volume: 0.6,
                    goods: "grain".into(),
                });
            }
        }
    }
    out
}

fn dist_to_chains(chains: &[Vec<[f32; 3]>], x: f32, y: f32) -> f32 {
    let mut best = f32::MAX;
    for c in chains {
        for w in c.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
            let l2 = (dx * dx + dy * dy).max(1e-9);
            let t = (((x - a[0]) * dx + (y - a[1]) * dy) / l2).clamp(0.0, 1.0);
            let (px, py) = (a[0] + dx * t, a[1] + dy * t);
            best = best.min(((x - px).powi(2) + (y - py).powi(2)).sqrt());
        }
    }
    best
}

#[test]
fn vehicles_are_the_same_for_the_same_clock() {
    let mut scene = load("great-lakes-2000");
    scene.set_lanes(lanes_for(&scene));
    let a = vehicles_at(&scene, 123.4);
    let b = vehicles_at(&scene, 123.4);
    assert!(!a.is_empty(), "lanes give vehicles");
    assert_eq!(a, b);
    assert_ne!(a, vehicles_at(&scene, 124.4), "they move with the clock");
}

#[test]
fn road_and_rail_vehicles_stay_on_the_drawn_lines() {
    let mut scene = load("great-lakes-2000");
    scene.set_lanes(lanes_for(&scene));
    let roads: Vec<Vec<[f32; 3]>> = road_chains_for_band(&scene, &ROAD_BANDS[2])
        .into_iter()
        .map(|(_, pts)| pts.into_iter().map(|p| p.xyz).collect())
        .collect();
    let mut checked = 0;
    for t in [0.0f32, 31.7, 400.0, 2500.0] {
        for (kind, _, _, lon, lat) in vehicles_at(&scene, t) {
            if kind == 2 {
                continue;
            }
            let (x, y) = scene.projection.to_km(lon, lat);
            let d = dist_to_chains(&roads, x, y);
            assert!(d < 1.0, "a kind {kind} vehicle is {d} km off the lines");
            checked += 1;
        }
    }
    assert!(checked > 100, "only {checked} vehicles checked");
}

#[test]
fn river_ships_sail_on_the_rivers() {
    let mut scene = load("ruhr-rhine-1991");
    scene.set_lanes(lanes_for(&scene));
    let rivers: Vec<Vec<[f32; 3]>> = river_chains(&scene, 2)
        .into_iter()
        .map(|c| c.into_iter().map(|p| p.xyz).collect())
        .collect();
    let barges: Vec<_> = vehicles_at(&scene, 50.0)
        .into_iter()
        .filter(|v| v.0 == 2 && v.2 == 3)
        .collect();
    assert!(!barges.is_empty(), "the Rhine carries barges");
    for (_, _, _, lon, lat) in barges {
        let (x, y) = scene.projection.to_km(lon, lat);
        let d = dist_to_chains(&rivers, x, y);
        assert!(d < 1.0, "a barge is {d} km from a river");
    }
}

#[test]
fn harbours_stand_only_at_a_coast_or_a_river() {
    for id in ["great-lakes-2000", "texas-1981", "ruhr-rhine-1991"] {
        let mut scene = load(id);
        scene.set_lanes(lanes_for(&scene));
        let rivers: Vec<Vec<[f32; 3]>> = river_chains(&scene, 2)
            .into_iter()
            .map(|c| c.into_iter().map(|p| p.xyz).collect())
            .collect();
        let sites = harbour_sites(&scene);
        assert!(!sites.is_empty(), "{id}: no harbours");
        for (lon, lat) in sites {
            let (x, y) = scene.projection.to_km(lon, lat);
            let by_river = dist_to_chains(&rivers, x, y) < 0.5;
            // Water within a few kilometres in some direction means a shore.
            let wet = |dx: f32, dy: f32| {
                let (lo, la) = scene.projection.to_lonlat(x + dx, y + dy);
                let w = scene.mask.at(lo, la)[2];
                w == cw_render::WATER_OCEAN || w == cw_render::WATER_LAKE
            };
            let by_coast = (0..16).any(|k| {
                let a = k as f32 * std::f32::consts::TAU / 16.0;
                wet(a.cos() * 3.0, a.sin() * 3.0)
            });
            assert!(
                by_river || by_coast,
                "{id}: a harbour inland at {lon} {lat}"
            );
        }
    }
}

#[test]
fn no_lanes_falls_back_to_plant_routes() {
    let mut scene = load("great-lakes-2000");
    let region = scene.regions[0].code.clone();
    let sites = vec![PlantSite {
        region,
        industry: "manufacturing".into(),
        firm: 1,
        plants: 4,
        player: true,
        status: SiteStatus::Running,
        utilisation: 0.8,
        progress: 0.0,
    }];
    let placed = layout_plants(&scene, &sites);
    scene.set_buildings(placed);
    assert!(scene.lanes.is_empty());
    let [trucks, trains, _] = cw_render::route_counts(&scene);
    assert!(trucks + trains > 0, "plants alone still get routes");
}
