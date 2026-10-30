//! Print every plant's footprint and height in km and in screen pixels for a view, and render it.
//!
//!     cargo run --release -p cw-render --example measure_plants -- <out.png> [region] [width_km] [tilt]

use std::path::{Path, PathBuf};

use cw_render::{
    AssetDirs, Camera, PlantSite, Renderer, SCENARIOS, Scene, SceneOptions, SiteStatus, Style,
    building_boxes, layout_plants, project,
};

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out png");
    let region = args.next().unwrap_or_else(|| "OH-CEN".into());
    let width_km: f32 = args.next().map_or(130.0, |s| s.parse().unwrap());
    let tilt: f32 = args.next().map_or(45.0, |s| s.parse().unwrap());
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    let spec = SCENARIOS
        .iter()
        .find(|s| s.id == "great-lakes-2000")
        .unwrap();
    let mut scene = Scene::for_scenario(
        &dirs,
        spec,
        &SceneOptions {
            player_regions: vec![region.clone()],
            hub_buildings: false,
        },
    )
    .expect("scene");
    let mix = [
        ("energy", 6, true),
        ("manufacturing", 12, false),
        ("extraction", 3, false),
        ("retail", 1, false),
        ("financial", 9, false),
        ("technology", 5, false),
    ];
    let mut sites: Vec<PlantSite> = mix
        .iter()
        .enumerate()
        .map(|(i, (ind, n, player))| PlantSite {
            region: region.clone(),
            industry: (*ind).into(),
            firm: i as u64,
            plants: *n,
            player: *player,
            status: SiteStatus::Running,
        })
        .collect();
    sites.push(PlantSite {
        region: region.clone(),
        industry: "construction".into(),
        firm: 9,
        plants: 1,
        player: false,
        status: SiteStatus::Site,
    });
    let placed = layout_plants(&scene, &sites);
    let info = scene
        .regions
        .iter()
        .find(|r| r.code == region)
        .unwrap()
        .clone();
    scene.set_buildings(placed);
    let cam = Camera {
        lon: info.lon,
        lat: info.lat,
        width_km,
        tilt_deg: tilt,
        heading_deg: 0.0,
        fov_deg: 28.0,
    };
    let (w, h) = (1127u32, 843u32);
    let kx = scene.projection.km_per_deg_lon();
    for b in building_boxes(&scene) {
        let (dlon, dlat) = (b.half_km / kx, b.half_km / cw_render::KM_PER_DEG_LAT);
        let mut r = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        let mut foot = [f32::MAX, f32::MIN];
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            for lift in [0.0, b.height_km] {
                if let Some((x, y)) = project(
                    &scene,
                    &cam,
                    w as f32,
                    h as f32,
                    b.lon + sx * dlon,
                    b.lat + sy * dlat,
                    lift,
                ) {
                    r = [r[0].min(x), r[1].min(y), r[2].max(x), r[3].max(y)];
                    if lift == 0.0 {
                        foot = [foot[0].min(x), foot[1].max(x)];
                    }
                }
            }
        }
        println!(
            "{:28} foot {:5.2} km  height {:5.2} km  screen {:4.0} x {:4.0} px (footprint width {:4.0})",
            b.model,
            b.half_km * 2.0,
            b.height_km,
            r[2] - r[0],
            r[3] - r[1],
            foot[1] - foot[0]
        );
    }
    let mut renderer = Renderer::new().expect("renderer");
    let f = renderer.render_to_rgba(
        &scene,
        &cam,
        &Style {
            pixel_scale: 1,
            ..Style::default()
        },
        w,
        h,
    );
    let rgb: Vec<u8> = f
        .rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    image::save_buffer(
        &out,
        &rgb,
        f.width,
        f.height,
        image::ExtendedColorType::Rgb8,
    )
    .unwrap();
}
