//! The five review views for the plant and city models, with a grid of plants placed round each
//! view's centre: Columbus, Chicago lakeshore, Houston coast, Duisburg, plus a mid and a far
//! zoom. Writes a desktop PNG and a phone JPEG (under 400 KB) per view.
//!
//!     cargo run --release -p cw-render --example model_views -- <out_dir> <tag> [night] [view...]

use std::path::{Path, PathBuf};

use cw_render::{
    AssetDirs, Camera, Owner, Placement, Renderer, SCENARIOS, Scene, SceneOptions, Style,
    plant_size_km,
};

/// Sector and size of the nine plants of a grid, in rows from the south.
const SETS: [[(&str, usize); 9]; 3] = [
    [
        ("energy", 2),
        ("manufacturing", 1),
        ("logistics", 1),
        ("technology", 1),
        ("healthcare", 1),
        ("retail", 1),
        ("chemical_industries", 1),
        ("media", 1),
        ("financial", 1),
    ],
    [
        ("extraction", 1),
        ("agriculture", 1),
        ("defense", 1),
        ("construction", 1),
        ("real_estate", 1),
        ("telecommunications", 1),
        ("energy", 0),
        ("manufacturing", 2),
        ("logistics", 2),
    ],
    [
        ("chemical_industries", 2),
        ("energy", 1),
        ("manufacturing", 0),
        ("media", 2),
        ("financial", 2),
        ("healthcare", 2),
        ("retail", 2),
        ("technology", 2),
        ("site", 1),
    ],
];

struct View {
    name: &'static str,
    scenario: &'static str,
    lon: f32,
    lat: f32,
    width_km: f32,
    tilt: f32,
    heading: f32,
    set: usize,
    grid: bool,
}

const VIEWS: [View; 8] = [
    View {
        name: "columbus",
        scenario: "great-lakes-2000",
        lon: -83.0,
        lat: 40.0,
        width_km: 25.0,
        tilt: 50.0,
        heading: 0.0,
        set: 0,
        grid: true,
    },
    View {
        name: "chicago",
        scenario: "great-lakes-2000",
        lon: -87.68,
        lat: 41.85,
        width_km: 25.0,
        tilt: 50.0,
        heading: 0.0,
        set: 1,
        grid: true,
    },
    View {
        name: "houston",
        scenario: "texas-1981",
        lon: -95.1,
        lat: 29.62,
        width_km: 25.0,
        tilt: 50.0,
        heading: 0.0,
        set: 2,
        grid: true,
    },
    View {
        name: "duisburg",
        scenario: "ruhr-rhine-1991",
        lon: 6.75,
        lat: 51.42,
        width_km: 25.0,
        tilt: 50.0,
        heading: 0.0,
        set: 0,
        grid: true,
    },
    View {
        name: "skyline",
        scenario: "great-lakes-2000",
        lon: -87.63,
        lat: 41.86,
        width_km: 9.0,
        tilt: 40.0,
        heading: 20.0,
        set: 0,
        grid: false,
    },
    View {
        name: "skyline2",
        scenario: "great-lakes-2000",
        lon: -83.05,
        lat: 42.33,
        width_km: 9.0,
        tilt: 40.0,
        heading: 200.0,
        set: 0,
        grid: false,
    },
    View {
        name: "mid",
        scenario: "great-lakes-2000",
        lon: -83.0,
        lat: 40.0,
        width_km: 150.0,
        tilt: 42.0,
        heading: 0.0,
        set: 0,
        grid: true,
    },
    View {
        name: "far",
        scenario: "great-lakes-2000",
        lon: -84.0,
        lat: 41.5,
        width_km: 1500.0,
        tilt: 30.0,
        heading: 0.0,
        set: 0,
        grid: false,
    },
];

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out dir");
    let tag = args.next().expect("tag");
    let rest: Vec<String> = args.collect();
    let night = rest.iter().any(|a| a == "night");
    let only: Vec<&String> = rest.iter().filter(|a| *a != "night").collect();
    std::fs::create_dir_all(&out).expect("out dir");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    let mut renderer = Renderer::new().expect("renderer");
    let mut scenes: Vec<(&str, Scene)> = Vec::new();
    for v in VIEWS
        .iter()
        .filter(|v| only.is_empty() || only.iter().any(|o| *o == v.name))
    {
        if !scenes.iter().any(|(id, _)| *id == v.scenario) {
            let spec = SCENARIOS.iter().find(|s| s.id == v.scenario).unwrap();
            let scene = Scene::for_scenario(&dirs, spec, &SceneOptions::default()).expect("scene");
            scenes.push((v.scenario, scene));
        }
        let scene = &mut scenes
            .iter_mut()
            .find(|(id, _)| *id == v.scenario)
            .unwrap()
            .1;
        scene.set_week(28);
        let base: Vec<Placement> = scene
            .buildings
            .iter()
            .filter(|b| b.owner == Owner::Scenery)
            .cloned()
            .collect();
        let mut list = base;
        if v.grid {
            let kx = scene.projection.km_per_deg_lon();
            let mut i = 0;
            for row in 0..3 {
                for col in 0..3 {
                    let (kind, size) = SETS[v.set][i];
                    let (dx, dy) = ((col as f32 - 1.0) * 4.6, (row as f32 - 1.0) * 4.2);
                    let key = if kind == "site" {
                        format!("site/{size}")
                    } else {
                        format!("{kind}/{size}")
                    };
                    let km = plant_size_km(kind, 1 + size as u32 * 2, kind == "site").max(0.9);
                    if let Some(m) = scene.models.get(&key) {
                        let owner = if i == 4 { Owner::Player } else { Owner::Rival };
                        list.push(placement(
                            key,
                            v.lon + dx / kx,
                            v.lat + dy / 111.2,
                            Placement::scale_for_km(scene.voxel_km, m, km),
                            owner,
                            i,
                        ));
                    }
                    i += 1;
                }
            }
        }
        scene.set_buildings(list);
        let cam = Camera {
            lon: v.lon,
            lat: v.lat,
            width_km: v.width_km,
            tilt_deg: v.tilt,
            heading_deg: v.heading,
            fov_deg: 28.0,
        };
        let style = Style {
            pixel_scale: 2,
            ..if night {
                Style::night()
            } else {
                Style::default()
            }
        };
        let f = renderer.render_to_rgba(scene, &cam, &style, 1200, 750);
        let rgb: Vec<u8> = f
            .rgba
            .chunks_exact(4)
            .flat_map(|p| [p[0], p[1], p[2]])
            .collect();
        let stem = format!("{}-{}-{}", v.name, if night { "night" } else { "day" }, tag);
        image::save_buffer(
            out.join(format!("{stem}.png")),
            &rgb,
            f.width,
            f.height,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
        let img = image::RgbImage::from_raw(f.width, f.height, rgb).unwrap();
        let phone = image::imageops::resize(&img, 900, 562, image::imageops::FilterType::Nearest);
        for q in [85u8, 70, 55, 40] {
            let mut buf = Vec::new();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, q)
                .encode_image(&phone)
                .unwrap();
            if buf.len() < 380_000 || q == 40 {
                std::fs::write(out.join(format!("{stem}-phone.jpg")), buf).unwrap();
                break;
            }
        }
        println!("{stem}");
    }
}

fn placement(model: String, lon: f32, lat: f32, scale: f32, owner: Owner, i: usize) -> Placement {
    Placement {
        model,
        lon,
        lat,
        yaw: 0.0,
        scale,
        owner,
        look: cw_render::Look {
            firm: (owner == Owner::Rival).then(|| cw_render::firm_colour(i as u64 + 2)),
            idle: i == 6,
        },
        activity: Default::default(),
    }
}
