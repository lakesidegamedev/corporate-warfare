//! Texture review views: the five close views plus a mid and a far zoom, day and winter.
//!
//!     cargo run --release -p cw-render --example tex_views -- <out_dir> <tag> [width_px] [view-name]
//!
//! Writes `<out_dir>/<name>-<season>-<tag>.png`.

use std::path::{Path, PathBuf};

use cw_render::{AssetDirs, Camera, Renderer, ScenarioSpec, Scene, SceneOptions, Style};

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out dir");
    let tag = args.next().unwrap_or_else(|| "after".into());
    let wpx: u32 = args.next().and_then(|a| a.parse().ok()).unwrap_or(1200);
    let only = args.next();
    std::fs::create_dir_all(&out).expect("out dir");
    let mut renderer = Renderer::new().expect("renderer");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    let cam = |lon: f32, lat: f32, width_km: f32, tilt_deg: f32| Camera {
        lon,
        lat,
        width_km,
        tilt_deg,
        heading_deg: 0.0,
        fov_deg: 28.0,
    };
    // (scenario, view name, camera)
    let views = [
        ("great-lakes-2000", "columbus", cam(-83.0, 39.96, 25.0, 50.0)),
        ("great-lakes-2000", "chicago", cam(-87.6, 41.85, 25.0, 50.0)),
        ("texas-1981", "houston", cam(-94.95, 29.6, 25.0, 50.0)),
        ("ruhr-rhine-1991", "duisburg", cam(6.78, 51.43, 25.0, 50.0)),
        ("great-lakes-2000", "bridge", cam(-76.38, 39.0, 40.0, 50.0)),
        ("great-lakes-2000", "mid", cam(-82.0, 41.5, 150.0, 40.0)),
        ("great-lakes-2000", "far", cam(-82.0, 41.5, 1500.0, 24.0)),
    ];
    let mut last = "";
    let mut scene: Option<Scene> = None;
    let rh = wpx * 5 / 8;
    for (sid, name, c) in &views {
        if only.as_deref().is_some_and(|o| o != *name) {
            continue;
        }
        if last != *sid {
            scene = Some(
                Scene::for_scenario(
                    &dirs,
                    ScenarioSpec::find(sid).unwrap(),
                    &SceneOptions::default(),
                )
                .expect("scene"),
            );
            last = sid;
        }
        let scene = scene.as_mut().unwrap();
        for (season, week) in [("day", 28u32), ("winter", 3u32)] {
            scene.set_week(week);
            let style = Style {
                pixel_scale: 2,
                ..Style::default()
            };
            let f = renderer.render_to_rgba(scene, c, &style, wpx, rh);
            let rgb: Vec<u8> = f
                .rgba
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            image::save_buffer(
                out.join(format!("{name}-{season}-{tag}.png")),
                &rgb,
                f.width,
                f.height,
                image::ExtendedColorType::Rgb8,
            )
            .expect("png");
        }
    }
}
