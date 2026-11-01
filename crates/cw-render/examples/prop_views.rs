//! Render the countryside prop views as PNGs: close views of cities, forests and farm belts,
//! day, winter and night, with props on ("after") or off ("before").
//!
//!     cargo run --release -p cw-render --example prop_views -- <out_dir> <before|after> [view-filter] [season-filter]

use std::path::{Path, PathBuf};

use cw_render::{AssetDirs, Camera, Renderer, SCENARIOS, Scene, SceneOptions, Style};

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out dir");
    let props = args.next().as_deref() != Some("before");
    let only = args.next();
    let only_season = args.next();
    std::fs::create_dir_all(&out).expect("out dir");
    let mut renderer = Renderer::new().expect("renderer");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    let cam = |lon: f32, lat: f32, width_km: f32, tilt_deg: f32, heading_deg: f32| Camera {
        lon,
        lat,
        width_km,
        tilt_deg,
        heading_deg,
        fov_deg: 28.0,
    };
    // (scenario, name, camera)
    let views = [
        (
            "great-lakes-2000",
            "columbus",
            cam(-83.0, 39.96, 25.0, 50.0, 0.0),
        ),
        (
            "great-lakes-2000",
            "chicago-lakeshore",
            cam(-87.7, 41.9, 25.0, 50.0, 0.0),
        ),
        (
            "great-lakes-2000",
            "north-woods",
            cam(-84.2, 46.6, 25.0, 50.0, 0.0),
        ),
        (
            "great-lakes-2000",
            "farm-belt",
            cam(-86.4, 40.4, 22.0, 50.0, 0.0),
        ),
        (
            "great-lakes-2000",
            "mid",
            cam(-83.0, 39.96, 150.0, 50.0, 0.0),
        ),
        (
            "great-lakes-2000",
            "far",
            cam(-83.0, 39.96, 1500.0, 40.0, 0.0),
        ),
        ("texas-1981", "houston", cam(-95.2, 29.65, 25.0, 50.0, 0.0)),
        (
            "ruhr-rhine-1991",
            "duisburg",
            cam(6.76, 51.43, 25.0, 50.0, 0.0),
        ),
        (
            "ruhr-rhine-1991",
            "black-forest",
            cam(8.2, 48.4, 25.0, 50.0, 0.0),
        ),
    ];
    for spec in SCENARIOS.iter() {
        let mut scene = Scene::for_scenario(
            &dirs,
            &spec,
            &SceneOptions {
                player_regions: Vec::new(),
                hub_buildings: true,
            },
        )
        .expect("scene");
        for (season, week, night) in [
            ("day", 28u32, false),
            ("autumn", 42, false),
            ("winter", 3, false),
            ("night", 28, true),
        ] {
            if only_season.as_deref().is_some_and(|s| s != season) {
                continue;
            }
            scene.set_week(week);
            let style = Style {
                pixel_scale: 1,
                night,
                props,
                ..Style::default()
            };
            for (sid, name, c) in &views {
                if *sid != spec.id || only.as_deref().is_some_and(|o| o != *name) {
                    continue;
                }
                let t = std::time::Instant::now();
                let f = renderer.render_to_rgba(&scene, c, &style, 1200, 750);
                let rgb: Vec<u8> = f
                    .rgba
                    .chunks_exact(4)
                    .flat_map(|p| [p[0], p[1], p[2]])
                    .collect();
                image::save_buffer(
                    out.join(format!("{name}-{season}.png")),
                    &rgb,
                    f.width,
                    f.height,
                    image::ExtendedColorType::Rgb8,
                )
                .expect("png");
                println!(
                    "{name}-{season}: {:?}, {} vertices",
                    t.elapsed(),
                    f.stats.vertices
                );
            }
        }
    }
}
