//! Rivers, the bridges where roads cross them, and the zones of city ground. No GPU needed.

use std::path::Path;

use cw_render::{
    AssetDirs, LINE_RIVER, ROAD_BANDS, ScenarioSpec, Scene, SceneOptions, bridge_sites,
    river_chains, road_chains_for_band,
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

#[test]
fn rivers_load_and_roads_bridge_them() {
    for (id, near) in [
        ("great-lakes-2000", (-84.52f32, 39.10f32)),
        ("ruhr-rhine-1991", (6.74, 51.42)),
    ] {
        let scene = load(id);
        assert!(
            scene.lines.iter().any(|l| l.class == LINE_RIVER),
            "{id}: no rivers"
        );
        assert!(
            river_chains(&scene, 0).is_empty(),
            "far band draws no rivers"
        );
        let rivers = river_chains(&scene, 2);
        let roads = road_chains_for_band(&scene, &ROAD_BANDS[2]);
        let sites = bridge_sites(&roads, &rivers);
        assert!(!sites.is_empty(), "{id}: no bridges");
        let kx = 111.32 * near.1.to_radians().cos();
        let close = sites.iter().any(|(_, at, _, stretch)| {
            let (lon, lat) = scene.projection.to_lonlat(at[0], at[1]);
            *stretch >= 1.0 && ((lon - near.0) * kx).hypot((lat - near.1) * 111.2) < 25.0
        });
        assert!(close, "{id}: no bridge near {near:?}");
    }
}

#[test]
fn big_cities_have_every_zone() {
    let scene = load("great-lakes-2000");
    let g = &scene.ground;
    let mut zones = [0usize; 3];
    let (mut core, mut urban) = (0usize, 0usize);
    for i in 0..g.w * g.h {
        match g.classes[i] {
            cw_render::G_CORE => core += 1,
            cw_render::G_URBAN => {
                urban += 1;
                zones[(g.variant[i] & 3).min(2) as usize] += 1;
            }
            _ => {}
        }
    }
    assert!(core > 20, "downtown cores: {core}");
    assert!(urban > core, "city ground: {urban}");
    for (z, n) in zones.iter().enumerate() {
        assert!(
            *n * 100 > urban,
            "zone {z} has only {n} of {urban} city texels"
        );
    }
}
