//! Render every scenario from the review views (scenario, close, turned and the horizon edges),
//! day and winter, as PNGs.
//!
//!     cargo run --release -p cw-render --example review_views -- <out_dir> [scenario-id]

use std::path::{Path, PathBuf};

use cw_render::{AssetDirs, Camera, Renderer, SCENARIOS, Scene, SceneOptions, Style};

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out dir");
    let only = args.next();
    std::fs::create_dir_all(&out).expect("out dir");
    let mut renderer = Renderer::new().expect("renderer");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    for spec in SCENARIOS
        .iter()
        .filter(|s| only.as_deref().is_none_or(|o| o == s.id))
    {
        let mut scene = Scene::for_scenario(
            &dirs,
            spec,
            &SceneOptions {
                player_regions: Vec::new(),
                hub_buildings: true,
            },
        )
        .expect("scene");
        let hb = scene.heightmap.bounds;
        let cam = |lon: f32, lat: f32, width_km: f32, tilt_deg: f32, heading_deg: f32| Camera {
            lon,
            lat,
            width_km,
            tilt_deg,
            heading_deg,
            fov_deg: 28.0,
        };
        let (cx, cy) = spec.centre;
        let mut views = vec![
            ("scenario", cam(cx, cy, spec.start_width_km, 24.0, 0.0)),
            ("close", cam(cx, cy, 780.0, 40.0, 0.0)),
            ("turned", cam(cx, cy, 900.0, 52.0, 38.0)),
            ("edge-east", cam(hb.east - 1.5, cy, 900.0, 58.0, 90.0)),
            ("edge-west", cam(hb.west + 1.5, cy, 900.0, 58.0, 270.0)),
            ("edge-north", cam(cx, hb.north - 1.0, 900.0, 58.0, 0.0)),
            ("edge-south", cam(cx, hb.south + 1.0, 900.0, 58.0, 180.0)),
        ];
        if spec.id == "texas-1981" {
            views.push(("tilted", cam(-101.5, 31.0, 1800.0, 50.0, 0.0)));
            views.push(("desert", cam(-103.0, 31.0, 500.0, 45.0, 0.0)));
        }
        if spec.id == "silicon-valley-1995" {
            views.push(("sierra", cam(-120.6, 37.5, 500.0, 55.0, 90.0)));
        }
        for (season, week) in [("day", 28u32), ("winter", 3u32)] {
            scene.set_week(week);
            let style = Style {
                pixel_scale: 2,
                ..Style::default()
            };
            for (name, c) in &views {
                let f = renderer.render_to_rgba(&scene, c, &style, 1600, 1000);
                let rgb: Vec<u8> = f
                    .rgba
                    .chunks_exact(4)
                    .flat_map(|p| [p[0], p[1], p[2]])
                    .collect();
                image::save_buffer(
                    out.join(format!("{}-{name}-{season}.png", spec.id)),
                    &rgb,
                    f.width,
                    f.height,
                    image::ExtendedColorType::Rgb8,
                )
                .expect("png");
            }
        }
    }
}
