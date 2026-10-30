//! The map's look rules without a GPU: where snow falls, how big a building stands, and that the
//! horizon reaches past the terrain.

use std::path::Path;

use cw_render::{
    AssetDirs, PlantSite, ScenarioSpec, Scene, SceneOptions, SiteStatus, apron_extent_km,
    apron_seam_steps, building_boxes, layout_plants, plant_size_km, snow_cover,
};

const WINTER: u32 = 3;
const SUMMER: u32 = 28;

fn scenario(id: &str) -> Scene {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let res = repo.join("apple/Resources");
    let baked = repo.join("crates/cw-render/assets");
    Scene::for_scenario(
        &AssetDirs {
            resources: &res,
            baked: &baked,
        },
        ScenarioSpec::find(id).unwrap(),
        &SceneOptions::default(),
    )
    .unwrap()
}

#[test]
fn snow_falls_in_northern_winters_and_on_high_mountains_only() {
    // Hot lowlands and plateaus never see it, even in midwinter.
    assert!(snow_cover(WINTER, 30.5, -101.0, 700.0) < 0.2, "west Texas");
    assert!(
        snow_cover(WINTER, 27.0, -104.0, 1400.0) < 0.2,
        "northern Mexico"
    );
    assert!(
        snow_cover(WINTER, 36.8, -120.5, 60.0) < 0.2,
        "the Central Valley"
    );
    // Northern lowlands do.
    assert!(snow_cover(WINTER, 51.4, 7.0, 100.0) > 0.2, "the Ruhr");
    assert!(snow_cover(WINTER, 43.0, -85.0, 250.0) > 0.2, "Michigan");
    // High mountains hold it where the valley below does not.
    assert!(snow_cover(WINTER, 37.7, -119.3, 3200.0) > 0.2, "the Sierra");
    // Midsummer: nothing in any of those places.
    for (lat, lon, m) in [
        (51.4, 7.0, 100.0),
        (43.0, -85.0, 250.0),
        (37.7, -119.3, 3200.0),
    ] {
        assert!(snow_cover(SUMMER, lat, lon, m) < 0.2, "summer at {lat}");
    }
}

#[test]
fn hot_deserts_are_scrub_not_rock_or_snow() {
    let mut scene = scenario("texas-1981");
    scene.set_week(WINTER);
    let g = &scene.ground;
    let (mut land, mut rock, mut snow) = (0usize, 0usize, 0usize);
    for y in 0..g.h {
        let lat = g.bounds.north - (y as f32 + 0.5) / g.ppd;
        for x in 0..g.w {
            let lon = g.bounds.west + (x as f32 + 0.5) / g.ppd;
            // West Texas and northern Mexico.
            if !(-106.0..-100.0).contains(&lon) || !(26.0..32.0).contains(&lat) {
                continue;
            }
            land += 1;
            match g.classes[y * g.w + x] {
                3 => rock += 1,
                4 => snow += 1,
                _ => {}
            }
        }
    }
    assert!(land > 1000, "the box covers the heightmap");
    assert!(
        (rock as f32 / land as f32) < 0.05,
        "rock covers {rock} of {land} desert texels"
    );
    assert!(
        (snow as f32 / land as f32) < 0.02,
        "snow covers {snow} of {land} desert texels in winter"
    );
}

#[test]
fn the_ruhr_whitens_in_winter_and_the_sierra_holds_its_snow_high() {
    let mut ruhr = scenario("ruhr-rhine-1991");
    let summer = ruhr.ground.classes.iter().filter(|c| **c == 4).count();
    ruhr.set_week(WINTER);
    let winter = ruhr.ground.classes.iter().filter(|c| **c == 4).count();
    assert!(
        winter > summer + ruhr.ground.classes.len() / 10,
        "{summer} then {winter}"
    );
    ruhr.set_week(SUMMER);
    assert_eq!(
        ruhr.ground.classes.iter().filter(|c| **c == 4).count(),
        summer,
        "summer returns exactly"
    );

    let mut sv = scenario("silicon-valley-1995");
    sv.set_week(WINTER);
    let (hm, g) = (&sv.heightmap, &sv.ground);
    let (mut high, mut high_snow, mut valley, mut valley_snow) = (0, 0, 0, 0);
    for y in (0..g.h).step_by(2) {
        let lat = g.bounds.north - (y as f32 + 0.5) / g.ppd;
        for x in (0..g.w).step_by(2) {
            let lon = g.bounds.west + (x as f32 + 0.5) / g.ppd;
            let (c, r) = hm.cell_of(lon, lat);
            let m = hm.at(c, r);
            let class = g.classes[y * g.w + x];
            if m > 3000 {
                high += 1;
                high_snow += (class == 4) as usize;
            }
            if m < 150 && (36.0..37.5).contains(&lat) && (-121.5..-120.0).contains(&lon) {
                valley += 1;
                valley_snow += (class == 4) as usize;
            }
        }
    }
    assert!(
        high > 50 && valley > 50,
        "{high} high texels, {valley} valley texels"
    );
    assert!(
        high_snow * 10 > high * 8,
        "the high Sierra is snowy: {high_snow} of {high}"
    );
    assert_eq!(valley_snow, 0, "the Central Valley stays bare");
}

const SECTORS: [&str; 15] = [
    "energy",
    "chemical_industries",
    "manufacturing",
    "extraction",
    "agriculture",
    "logistics",
    "construction",
    "defense",
    "media",
    "healthcare",
    "technology",
    "financial",
    "retail",
    "real_estate",
    "telecommunications",
];

#[test]
fn plants_stand_a_few_hundred_metres_to_two_kilometres_and_never_tower() {
    for industry in SECTORS {
        for plants in [1, 2, 5, 12, 40] {
            let km = plant_size_km(industry, plants, false);
            assert!((0.3..=2.0).contains(&km), "{industry} x{plants}: {km} km");
        }
    }
    assert!(plant_size_km("energy", 6, false) > plant_size_km("energy", 1, false));
    assert!(
        plant_size_km("energy", 1, true) < 0.5,
        "a site is a small yard"
    );

    // Every sector at every plant count: the footprint and the height, measured from the
    // placed buildings, stay in range. Heights are at most one footprint width (a lone mast
    // may reach a quarter more), so nothing dwarfs the counties round it.
    let mut scene = scenario("great-lakes-2000");
    let region = scene.regions[0].code.clone();
    let mut worst = (0.0f32, 0.0f32);
    for industry in SECTORS.iter().chain(&["site"]) {
        for plants in [1u32, 3, 4, 8, 9, 40] {
            let sites = [PlantSite {
                region: region.clone(),
                industry: (*industry).into(),
                firm: 1,
                plants,
                player: true,
                status: if *industry == "site" {
                    SiteStatus::Site
                } else {
                    SiteStatus::Running
                },
            }];
            scene.set_buildings(layout_plants(&scene, &sites));
            let boxes = building_boxes(&scene);
            assert_eq!(boxes.len(), 1);
            let b = &boxes[0];
            let across = b.half_km * 2.0;
            assert!(
                (0.3..=2.0).contains(&across),
                "{industry} x{plants}: {across} km across"
            );
            let limit = if *industry == "telecommunications" {
                1.3
            } else {
                1.0
            };
            assert!(
                b.height_km <= across * limit + 1e-3,
                "{industry} x{plants}: {} km tall on {across} km",
                b.height_km
            );
            worst = (worst.0.max(across), worst.1.max(b.height_km));
        }
    }
    assert!(worst.0 <= 2.0 && worst.1 <= 2.6, "worst {worst:?}");

    let region = scene.regions[0].code.clone();
    let sites: Vec<PlantSite> = (0..4)
        .map(|i| PlantSite {
            region: region.clone(),
            industry: ["energy", "retail", "extraction", "financial"][i].into(),
            firm: i as u64,
            plants: 1 + 3 * i as u32,
            player: i == 0,
            status: SiteStatus::Running,
        })
        .collect();
    scene.set_buildings(layout_plants(&scene, &sites));
    let boxes = building_boxes(&scene);
    assert_eq!(boxes.len(), 4);
    // The size is a property of the building, not of any camera: the footprint is the same
    // however often it is asked for.
    let again = building_boxes(&scene);
    for (a, b) in boxes.iter().zip(&again) {
        assert_eq!((a.half_km, a.height_km), (b.half_km, b.height_km));
    }
    // Downtown blocks are small too: about a block cluster, not a district.
    let scenery: Vec<_> = building_boxes(&scenario("great-lakes-2000"))
        .into_iter()
        .filter(|b| b.owner == cw_render::Owner::Scenery)
        .collect();
    assert!(!scenery.is_empty());
    assert!(
        scenery
            .iter()
            .all(|b| b.half_km * 2.0 < 0.8 && b.height_km < 0.8),
        "a downtown block is half a kilometre"
    );
}

#[test]
fn no_building_footprint_touches_water_in_any_scenario() {
    let mut checked = 0;
    for id in [
        "great-lakes-2000",
        "texas-1981",
        "ruhr-rhine-1991",
        "silicon-valley-1995",
    ] {
        let mut scene = scenario(id);
        let kx = scene.projection.km_per_deg_lon();
        let sites: Vec<PlantSite> = scene
            .regions
            .iter()
            .flat_map(|r| {
                (0..6).map(move |i| PlantSite {
                    region: r.code.clone(),
                    industry: SECTORS[(i * 2) % SECTORS.len()].into(),
                    firm: i as u64,
                    plants: [1, 5, 12, 3, 9, 2][i],
                    player: i == 0,
                    status: SiteStatus::Running,
                })
            })
            .collect();
        let mut all = layout_plants(&scene, &sites);
        // The downtown blocks the scene starts with stand on land too.
        all.extend(scene.buildings.iter().cloned());
        scene.set_buildings(all);
        let boxes = building_boxes(&scene);
        for b in &boxes {
            checked += 1;
            assert!(
                scene.mask.footprint_on_land(b.lon, b.lat, b.half_km, kx),
                "{id}: a {} footprint at {:.3}, {:.3} touches water",
                b.model,
                b.lon,
                b.lat
            );
        }
    }
    assert!(checked > 400, "{checked} buildings checked");
}

#[test]
fn every_plant_model_is_clean_solid_geometry() {
    let json: serde_json::Value = serde_json::from_str(cw_render::PLANT_MODELS).unwrap();
    let models = json["models"].as_object().unwrap();
    assert!(models.len() >= 50, "{} models", models.len());
    for (key, m) in models {
        let boxes: Vec<Vec<f64>> = m["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|b| (0..6).map(|i| b[i].as_f64().unwrap()).collect())
            .collect();
        let overlap = |a: &[f64], b: &[f64], i: usize| a[i + 3].min(b[i + 3]) - a[i].max(b[i]);
        for (n, a) in boxes.iter().enumerate() {
            assert!(
                a[0] >= -12.01 && a[3] <= 12.01 && a[1] >= -12.01 && a[4] <= 12.01,
                "{key}: a box leaves the footprint"
            );
            for b in &boxes[n + 1..] {
                assert!(
                    !(overlap(a, b, 0) > 1e-6
                        && overlap(a, b, 1) > 1e-6
                        && overlap(a, b, 2) > 1e-6),
                    "{key}: two boxes poke through each other"
                );
            }
            if a[2] > 1e-6 {
                assert!(
                    boxes.iter().any(|b| {
                        (b[5] - a[2]).abs() < 1e-6
                            && overlap(a, b, 0) > 1e-6
                            && overlap(a, b, 1) > 1e-6
                    }),
                    "{key}: a box floats"
                );
            }
        }
    }
}

#[test]
fn the_horizon_reaches_far_past_the_terrain_on_every_side() {
    for id in [
        "great-lakes-2000",
        "texas-1981",
        "silicon-valley-1995",
        "ruhr-rhine-1991",
    ] {
        let scene = scenario(id);
        let (w, s) = scene
            .projection
            .to_km(scene.heightmap.bounds.west, scene.heightmap.bounds.south);
        let (e, n) = scene
            .projection
            .to_km(scene.heightmap.bounds.east, scene.heightmap.bounds.north);
        let ext = apron_extent_km(&scene);
        let span = (e - w).max(n - s);
        assert!(
            ext[0] < w - span && ext[2] > e + span,
            "{id}: east and west reach"
        );
        assert!(
            ext[1] < s - span && ext[3] > n + span,
            "{id}: north and south reach"
        );
    }
}

/// Steps (changes of terrace level) met walking along a line of latitude.
fn steps_along(scene: &Scene, lat: f32, lon0: f32, lon1: f32) -> usize {
    let hm = &scene.heightmap;
    let (dx, _) = hm.cell_degrees();
    let n = ((lon1 - lon0) / dx) as usize;
    let mut last = None;
    let mut steps = 0;
    for k in 0..n {
        let (c, r) = hm.cell_of(lon0 + k as f32 * dx, lat);
        let l = scene.level_at(c, r);
        if last.is_some_and(|p| p != l) {
            steps += 1;
        }
        last = Some(l);
    }
    steps
}

/// The biggest difference in terrace level between two neighbouring cells (the Sierra front was 13 before the cap).
fn steepest_step(scene: &Scene, west: f32, south: f32, east: f32, north: f32) -> i32 {
    let hm = &scene.heightmap;
    let (dx, dy) = hm.cell_degrees();
    let (c0, r1) = hm.cell_of(west, south);
    let (c1, r0) = hm.cell_of(east, north);
    let mut worst = 0;
    for r in r0..r1 {
        for c in c0..c1 {
            let l = scene.level_at(c, r);
            worst = worst
                .max((l - scene.level_at(c + 1, r)).abs())
                .max((l - scene.level_at(c, r + 1)).abs());
        }
    }
    let _ = (dx, dy);
    worst
}

#[test]
fn flat_deserts_read_flat_and_escarpments_stay_clean() {
    let scene = scenario("texas-1981");
    // West Texas, east of the Pecos to the Caprock: a few wide steps, not a dense run of stripes.
    let steps = steps_along(&scene, 31.5, -104.0, -98.0);
    println!("west Texas steps along 31.5N: {steps}");
    assert!(steps <= 6, "west Texas is striped: {steps} steps");
    // The Caprock is still a step you can see.
    let rise = steepest_step(&scene, -103.0, 32.5, -100.5, 34.5);
    println!("Caprock steepest step: {rise}");
    assert!(rise >= 1, "the Caprock escarpment vanished");
}

#[test]
fn steep_mountain_fronts_are_capped() {
    let scene = scenario("silicon-valley-1995");
    let worst = steepest_step(&scene, -122.5, 35.0, -117.0, 40.0);
    println!("Sierra and coast ranges steepest step: {worst}");
    assert!(
        worst <= 6,
        "a mountain front is a wall: {worst} levels in one cell"
    );
}

#[test]
fn the_horizon_meets_the_terrain_without_a_wall() {
    for id in [
        "great-lakes-2000",
        "texas-1981",
        "silicon-valley-1995",
        "ruhr-rhine-1991",
    ] {
        assert_eq!(apron_seam_steps(&scenario(id)), 0, "{id}: wall at the seam");
    }
}
