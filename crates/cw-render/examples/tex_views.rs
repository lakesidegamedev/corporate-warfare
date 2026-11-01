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
        ("great-lakes-2000", "chicago", cam(-87.6, 41.88, 25.0, 50.0)),
        ("texas-1981", "houston", cam(-95.37, 29.76, 25.0, 50.0)),
        ("ruhr-rhine-1991", "duisburg", cam(6.76, 51.43, 25.0, 50.0)),
        ("great-lakes-2000", "bridge", cam(-84.52, 39.10, 12.0, 50.0)),
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
        check_view(scene, name, c);
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

/// Every close view must show land: its centre is land, and the 25 km ground under it is
/// partly land (a lakeshore view half water, the rest mostly land). Rivers views must hold river.
fn check_view(scene: &Scene, name: &str, c: &Camera) {
    if c.width_km > 100.0 {
        return;
    }
    let kx = cw_render::KM_PER_DEG_LAT * c.lat.to_radians().cos();
    let kind = |lon: f32, lat: f32| scene.mask.at(lon, lat)[2];
    let is_land = |k: u8| k == 0 || k == 3;
    assert!(is_land(kind(c.lon, c.lat)), "{name}: view centre is not land");
    let (mut land, mut n) = (0, 0);
    for i in -10..=10 {
        for j in -10..=10 {
            let lon = c.lon + i as f32 * c.width_km * 0.05 / kx;
            let lat = c.lat + j as f32 * c.width_km * 0.05 / cw_render::KM_PER_DEG_LAT;
            n += 1;
            if is_land(kind(lon, lat)) {
                land += 1;
            }
        }
    }
    let frac = land as f32 / n as f32;
    let (lo, hi) = if name == "chicago" { (0.25, 0.85) } else { (0.6, 1.01) };
    assert!(frac >= lo && frac <= hi, "{name}: land share {frac:.2} outside {lo}..{hi}");
    if name == "duisburg" || name == "bridge" {
        let near = cw_render::river_chains(scene, 2).iter().any(|ch| {
            ch.iter().any(|p| {
                let (lon, lat) = scene.projection.to_lonlat(p.xyz[0], p.xyz[1]);
                ((lon - c.lon) * kx).hypot((lat - c.lat) * cw_render::KM_PER_DEG_LAT) < c.width_km * 0.5
            })
        });
        assert!(near, "{name}: no river in view");
    }
    println!("{name}: land share {frac:.2}");
}
