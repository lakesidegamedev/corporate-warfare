//! Zoom-out sequences of every scenario (three zoom levels, tilted and flat) as desktop PNGs and
//! phone JPEGs, to see the world edge of the map.
//!
//!     cargo run --release -p cw-render --example edge_views -- <out_dir> <tag> [scenario-id]

use std::path::{Path, PathBuf};

use cw_render::{AssetDirs, Camera, Renderer, SCENARIOS, Scene, SceneOptions, Style};

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out dir");
    let tag = args.next().expect("tag");
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
        let scene = Scene::for_scenario(
            &dirs,
            spec,
            &SceneOptions {
                player_regions: Vec::new(),
                hub_buildings: true,
            },
        )
        .expect("scene");
        let (cx, cy) = spec.centre;
        for (zoom, width_km) in [("z1", 3500.0), ("z2", 7000.0), ("z3", 14000.0)] {
            for (look, tilt) in [("tilted", 45.0), ("flat", 0.0)] {
                let cam = Camera {
                    lon: cx,
                    lat: cy,
                    width_km,
                    tilt_deg: tilt,
                    heading_deg: 0.0,
                    fov_deg: 28.0,
                };
                for (device, w, h, ps) in
                    [("desktop", 1600u32, 1000u32, 2u32), ("phone", 780, 1560, 2)]
                {
                    let style = Style {
                        pixel_scale: ps,
                        ..Style::default()
                    };
                    let f = renderer.render_to_rgba(&scene, &cam, &style, w, h);
                    let rgb: Vec<u8> = f
                        .rgba
                        .chunks_exact(4)
                        .flat_map(|p| [p[0], p[1], p[2]])
                        .collect();
                    let ext = if device == "phone" { "jpg" } else { "png" };
                    image::save_buffer(
                        out.join(format!("{}-{zoom}-{look}-{device}-{tag}.{ext}", spec.id)),
                        &rgb,
                        f.width,
                        f.height,
                        image::ExtendedColorType::Rgb8,
                    )
                    .expect("image");
                }
            }
        }
    }
}
