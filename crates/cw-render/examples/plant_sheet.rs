//! A contact sheet of every plant model: one row per model, four headings at two tilts, then the
//! same building as the player's. Phone sized JPEG strips are written beside the PNG.
//!
//!     cargo run --release -p cw-render --example plant_sheet -- <out_dir> [size 0|1|2]

use std::path::{Path, PathBuf};

use cw_render::{
    AssetDirs, Camera, Owner, Placement, Renderer, SCENARIOS, Scene, SceneOptions, Style,
    plant_size_km,
};

const KEYS: [&str; 17] = [
    "energy",
    "chemical_industries",
    "manufacturing",
    "extraction",
    "agriculture",
    "logistics",
    "construction",
    "defense",
    "media",
    "healthcare",
    "technology",
    "financial",
    "retail",
    "real_estate",
    "telecommunications",
    "city",
    "site",
];

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out dir");
    let size: u32 = args.next().map_or(1, |s| s.parse().unwrap());
    std::fs::create_dir_all(&out).expect("out dir");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    let spec = SCENARIOS
        .iter()
        .find(|s| s.id == "great-lakes-2000")
        .unwrap();
    let mut scene = Scene::for_scenario(&dirs, spec, &SceneOptions::default()).expect("scene");
    scene.buildings.clear();
    let mut renderer = Renderer::new().expect("renderer");
    // Flat farmland in central Ohio, away from any city.
    let (lon, lat) = (-83.4, 40.3);
    let (tw, th) = (240u32, 180u32);
    let views: Vec<(f32, f32, Owner)> = [25.0f32, 55.0]
        .iter()
        .flat_map(|t| {
            [0.0f32, 90.0, 180.0, 270.0]
                .iter()
                .map(move |h| (*t, *h, Owner::Rival))
        })
        .chain([(40.0, 30.0, Owner::Player)])
        .collect();
    let cols = views.len() as u32;
    let mut sheet = vec![0u8; (tw * cols * th * KEYS.len() as u32 * 3) as usize];
    for (row, key) in KEYS.iter().enumerate() {
        let model = format!("{key}/{size}");
        let m = &scene.models[&model];
        let km = plant_size_km(
            if *key == "city" { "retail" } else { key },
            5,
            *key == "site",
        )
        .max(0.9);
        let scale = Placement::scale_for_km(scene.voxel_km, m, km);
        for (col, (tilt, heading, owner)) in views.iter().enumerate() {
            scene.set_buildings(vec![Placement {
                model: model.clone(),
                lon,
                lat,
                yaw: 0.0,
                scale,
                owner: *owner,
            }]);
            let cam = Camera {
                lon,
                lat,
                width_km: km * 1.9,
                tilt_deg: *tilt,
                heading_deg: *heading,
                fov_deg: 28.0,
            };
            let f = renderer.render_to_rgba(
                &scene,
                &cam,
                &Style {
                    pixel_scale: 1,
                    ..Style::default()
                },
                tw,
                th,
            );
            for y in 0..th {
                for x in 0..tw {
                    let s = ((y * tw + x) * 4) as usize;
                    let d =
                        (((row as u32 * th + y) * tw * cols + col as u32 * tw + x) * 3) as usize;
                    sheet[d..d + 3].copy_from_slice(&f.rgba[s..s + 3]);
                }
            }
        }
    }
    let (w, h) = (tw * cols, th * KEYS.len() as u32);
    image::save_buffer(
        out.join(format!("plants-size{size}.png")),
        &sheet,
        w,
        h,
        image::ExtendedColorType::Rgb8,
    )
    .unwrap();
    // Phone strips: six models a strip, shrunk to 1080 wide.
    let img = image::RgbImage::from_raw(w, h, sheet).unwrap();
    for (n, chunk) in (0..KEYS.len() as u32)
        .collect::<Vec<_>>()
        .chunks(6)
        .enumerate()
    {
        let (y0, rows) = (chunk[0] * th, chunk.len() as u32 * th);
        let crop = image::imageops::crop_imm(&img, 0, y0, w, rows).to_image();
        let nw = 1080;
        let nh = rows * nw / w;
        let small = image::imageops::resize(&crop, nw, nh, image::imageops::FilterType::Triangle);
        small
            .save_with_format(
                out.join(format!("plants-size{size}-phone{}.jpg", n + 1)),
                image::ImageFormat::Jpeg,
            )
            .unwrap();
    }
}
