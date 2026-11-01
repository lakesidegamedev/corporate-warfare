//! Frame sequences for the moving light of the map: drifting clouds and moving water.
//!
//!     cargo run --release -p cw-render --example night_views -- <out_dir> [clouds|water]
//!
//! `clouds` writes 8 frames of the far and mid views a quarter hour of map time apart, day and
//! night. `water` writes 8 frames of a lake shore and a sea coast at close zoom a few seconds
//! apart. Every frame is a palette colour image, 1200 x 750.

use std::path::{Path, PathBuf};

use cw_render::{AssetDirs, Camera, Renderer, ScenarioSpec, Scene, SceneOptions, Style};

/// One sequence: where, how far, and how long between frames.
struct Job {
    name: &'static str,
    scenario: &'static str,
    at: (f32, f32),
    width_km: f32,
    tilt: f32,
    step_s: f64,
    night: bool,
}

fn save(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    image::save_buffer(path, &rgb, w, h, image::ExtendedColorType::Rgb8).expect("png");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out dir");
    let mode = args.next().unwrap_or_else(|| "clouds".into());
    std::fs::create_dir_all(&out).expect("out dir");
    let mut renderer = Renderer::new().expect("renderer");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    let job = |name, scenario, at, width_km, tilt, step_s, night| Job {
        name,
        scenario,
        at,
        width_km,
        tilt,
        step_s,
        night,
    };
    let gl = "great-lakes-2000";
    let jobs = match mode.as_str() {
        "clouds" => vec![
            job("clouds-far", gl, (-83.0, 40.0), 2250.0, 30.0, 900.0, false),
            job("clouds-mid", gl, (-83.0, 40.0), 1500.0, 40.0, 900.0, false),
            job(
                "clouds-far-night",
                gl,
                (-83.0, 40.0),
                1500.0,
                30.0,
                900.0,
                true,
            ),
        ],
        "water" => vec![
            job("water-lake", gl, (-87.2, 42.6), 12.0, 50.0, 1.5, false),
            job(
                "water-gulf",
                "texas-1981",
                (-94.6, 29.0),
                12.0,
                50.0,
                1.5,
                false,
            ),
        ],
        _ => panic!("mode: clouds or water"),
    };
    for j in &jobs {
        let spec = ScenarioSpec::find(j.scenario).expect("scenario");
        let scene = Scene::for_scenario(&dirs, spec, &SceneOptions::default()).expect("scene");
        let cam = Camera {
            lon: j.at.0,
            lat: j.at.1,
            width_km: j.width_km,
            tilt_deg: j.tilt,
            heading_deg: 0.0,
            fov_deg: 28.0,
        };
        for k in 0..8 {
            let style = Style {
                night: j.night,
                animate: true,
                time_s: 10.0 + k as f64 * j.step_s,
                pixel_scale: 2,
                ..Style::default()
            };
            let f = renderer.render_to_rgba(&scene, &cam, &style, 1200, 750);
            save(
                &out.join(format!("{}-f{k:02}.png", j.name)),
                f.width,
                f.height,
                &f.rgba,
            );
        }
        println!("{} done", j.name);
    }
}
