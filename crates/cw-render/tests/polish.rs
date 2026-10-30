//! The map's look rules without a GPU: where snow falls, how big a building stands, and that the
//! horizon reaches past the terrain.

use std::path::Path;

use cw_render::{
    AssetDirs, PlantSite, ScenarioSpec, Scene, SceneOptions, SiteStatus, apron_extent_km,
    building_boxes, layout_plants, plant_size_km, snow_cover,
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

#[test]
fn plants_stand_one_to_three_kilometres_across_and_keep_that_size() {
    for industry in [
        "energy",
        "financial",
        "manufacturing",
        "retail",
        "extraction",
    ] {
        for plants in [1, 2, 5, 12, 40] {
            let km = plant_size_km(industry, plants, false);
            assert!((1.0..=3.0).contains(&km), "{industry} x{plants}: {km} km");
        }
    }
    assert!(plant_size_km("energy", 6, false) > plant_size_km("energy", 1, false));

    let scene = scenario("texas-1981");
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
    let mut scene = scene;
    scene.set_buildings(layout_plants(&scene, &sites));
    let boxes = building_boxes(&scene);
    assert_eq!(boxes.len(), 4);
    for b in &boxes {
        let across = b.half_km * 2.0;
        assert!(
            (0.95..=3.05).contains(&across),
            "{}: {across} km across",
            b.model
        );
    }
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
        scenery.iter().all(|b| b.half_km * 2.0 < 1.6),
        "a downtown block is about a kilometre"
    );
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
