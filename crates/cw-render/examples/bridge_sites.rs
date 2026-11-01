//! Lists the road and rail crossings of rivers in a scenario: `bridge_sites <scenario> [band]`.
use std::path::Path;

use cw_render::{
    AssetDirs, ROAD_BANDS, ScenarioSpec, Scene, SceneOptions, bridge_sites, river_chains,
    road_chains_for_band,
};

fn main() {
    let mut a = std::env::args().skip(1);
    let sid = a.next().expect("scenario");
    let band: usize = a.next().and_then(|v| v.parse().ok()).unwrap_or(2);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    let scene = Scene::for_scenario(
        &dirs,
        ScenarioSpec::find(&sid).unwrap(),
        &SceneOptions::default(),
    )
    .expect("scene");
    let roads = road_chains_for_band(&scene, &ROAD_BANDS[band]);
    let rivers = river_chains(&scene, band);
    let sites = bridge_sites(&roads, &rivers);
    println!("{} river chains, {} sites", rivers.len(), sites.len());
    for (class, at, _, st) in sites {
        let (lon, lat) = scene.projection.to_lonlat(at[0], at[1]);
        println!("class {class} lon {lon:.3} lat {lat:.3} stretch {st:.2}");
    }
}
