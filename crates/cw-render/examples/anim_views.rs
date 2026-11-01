//! Render the animated layer (smoke, construction, beacons, vehicles, growth) at the review views
//! and as short frame sequences, and time animations on against off.
//!
//!     cargo run --release -p cw-render --example anim_views -- <out_dir> [still|frames|timing]
//!
//! `still` writes the day and night review views at one clock value, `frames` writes 8 frames of
//! the close views a second apart, `timing` prints frame times with animations on and off.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cw_render::{
    AssetDirs, Camera, PlantSite, Renderer, ScenarioSpec, Scene, SceneOptions, SiteStatus, Style,
    layout_plants,
};

struct View {
    name: &'static str,
    scenario: &'static str,
    lon: f32,
    lat: f32,
    width_km: f32,
    tilt: f32,
}

const VIEWS: [View; 7] = [
    View {
        name: "columbus-close",
        scenario: "great-lakes-2000",
        lon: -83.0,
        lat: 40.0,
        width_km: 25.0,
        tilt: 50.0,
    },
    View {
        name: "chicago-lakeshore-close",
        scenario: "great-lakes-2000",
        lon: -87.6,
        lat: 41.85,
        width_km: 25.0,
        tilt: 50.0,
    },
    View {
        name: "houston-coast-close",
        scenario: "texas-1981",
        lon: -95.2,
        lat: 29.6,
        width_km: 25.0,
        tilt: 50.0,
    },
    View {
        name: "duisburg-close",
        scenario: "ruhr-rhine-1991",
        lon: 6.76,
        lat: 51.43,
        width_km: 25.0,
        tilt: 50.0,
    },
    View {
        name: "mid-150km",
        scenario: "great-lakes-2000",
        lon: -83.0,
        lat: 40.0,
        width_km: 150.0,
        tilt: 45.0,
    },
    View {
        name: "far-1500km",
        scenario: "great-lakes-2000",
        lon: -83.0,
        lat: 40.0,
        width_km: 1500.0,
        tilt: 30.0,
    },
    View {
        name: "columbus-wide-60km",
        scenario: "great-lakes-2000",
        lon: -83.0,
        lat: 40.0,
        width_km: 60.0,
        tilt: 50.0,
    },
];

fn save(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    image::save_buffer(path, &rgb, w, h, image::ExtendedColorType::Rgb8).expect("png");
}

/// A game-like set of buildings in the region nearest `(lon, lat)`.
fn populate(scene: &mut Scene, lon: f32, lat: f32) {
    let region = scene
        .regions
        .iter()
        .min_by(|a, b| {
            let d = |r: &cw_render::RegionInfo| (r.lon - lon).powi(2) + (r.lat - lat).powi(2);
            d(a).total_cmp(&d(b))
        })
        .map(|r| r.code.clone())
        .expect("a region");
    let mix = [
        ("energy", 9, true, SiteStatus::Running, 0.95, 0.0),
        (
            "chemical_industries",
            5,
            false,
            SiteStatus::Running,
            0.6,
            0.0,
        ),
        ("logistics", 6, true, SiteStatus::Running, 0.7, 0.0),
        ("manufacturing", 2, true, SiteStatus::Site, 0.0, 0.55),
        ("extraction", 4, false, SiteStatus::Running, 0.3, 0.0),
        ("financial", 3, false, SiteStatus::Running, 0.5, 0.0),
    ];
    let sites: Vec<PlantSite> = mix
        .iter()
        .enumerate()
        .map(|(i, (ind, n, player, status, util, progress))| PlantSite {
            region: region.clone(),
            industry: (*ind).into(),
            firm: i as u64 + 1,
            plants: *n,
            player: *player,
            status: *status,
            utilisation: *util,
            progress: *progress,
        })
        .collect();
    let placed = layout_plants(scene, &sites);
    scene.set_buildings(placed);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out dir");
    let mode = args.next().unwrap_or_else(|| "still".into());
    std::fs::create_dir_all(&out).expect("out dir");
    let mut renderer = Renderer::new().expect("renderer");
    println!("adapter: {}", renderer.adapter().name);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    let width = 1200u32;
    let height = 750u32;
    let mut scenes: Vec<(&str, Scene)> = Vec::new();
    for v in &VIEWS {
        if scenes.iter().any(|(id, _)| *id == v.scenario) {
            continue;
        }
        let spec = ScenarioSpec::find(v.scenario).expect("scenario");
        let scene = Scene::for_scenario(&dirs, spec, &SceneOptions::default()).expect("scene");
        scenes.push((v.scenario, scene));
    }
    for v in &VIEWS {
        let scene = &mut scenes
            .iter_mut()
            .find(|(id, _)| *id == v.scenario)
            .unwrap()
            .1;
        populate(scene, v.lon, v.lat);
        let cam = Camera {
            lon: v.lon,
            lat: v.lat,
            width_km: v.width_km,
            tilt_deg: v.tilt,
            heading_deg: 0.0,
            fov_deg: 28.0,
        };
        let style = |night: bool, animate: bool, t: f64| Style {
            night,
            animate,
            time_s: t,
            pixel_scale: 2,
            ..Style::default()
        };
        match mode.as_str() {
            "still" => {
                for (tag, night, animate) in [
                    ("day", false, true),
                    ("night", true, true),
                    ("day-off", false, false),
                ] {
                    let f = renderer.render_to_rgba(
                        scene,
                        &cam,
                        &style(night, animate, 7.3),
                        width,
                        height,
                    );
                    save(
                        &out.join(format!("{}-{tag}.png", v.name)),
                        f.width,
                        f.height,
                        &f.rgba,
                    );
                }
            }
            "frames" => {
                if !v.name.ends_with("close") && v.name != "mid-150km" {
                    continue;
                }
                for night in [false, true] {
                    for k in 0..8 {
                        let f = renderer.render_to_rgba(
                            scene,
                            &cam,
                            &style(night, true, 3.0 + k as f64 * 0.75),
                            width,
                            height,
                        );
                        let tag = if night { "night" } else { "day" };
                        save(
                            &out.join(format!("{}-{tag}-f{k:02}.png", v.name)),
                            f.width,
                            f.height,
                            &f.rgba,
                        );
                    }
                }
            }
            "timing" => {
                if v.name != "columbus-close" && v.name != "mid-150km" {
                    continue;
                }
                for animate in [false, true] {
                    let mut ms = Vec::new();
                    for k in 0..6 {
                        let t0 = Instant::now();
                        renderer.render_to_rgba(
                            scene,
                            &cam,
                            &style(false, animate, k as f64 * 0.5),
                            width,
                            height,
                        );
                        ms.push(t0.elapsed().as_secs_f64() * 1000.0);
                    }
                    ms.remove(0);
                    println!(
                        "{} animations {}: {:.0} ms per frame (mean of 5, {width}x{height}, software adapter)",
                        v.name,
                        if animate { "on" } else { "off" },
                        ms.iter().sum::<f64>() / ms.len() as f64
                    );
                }
            }
            _ => panic!("mode: still, frames or timing"),
        }
    }
}
