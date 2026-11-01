//! Zooming the map: the ground texture steps and the far apron must not pop, and the texture
//! must not repeat visibly. Skips cleanly with no GPU adapter.

use std::collections::HashSet;
use std::path::Path;

use cw_render::{AssetDirs, Camera, Renderer, ScenarioSpec, Scene, SceneOptions, Style};

fn scene() -> Scene {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let res = repo.join("apple/Resources");
    let baked = repo.join("crates/cw-render/assets");
    Scene::for_scenario(
        &AssetDirs {
            resources: &res,
            baked: &baked,
        },
        ScenarioSpec::find("great-lakes-2000").unwrap(),
        &SceneOptions::default(),
    )
    .unwrap()
}

fn cam(lon: f32, lat: f32, width_km: f32) -> Camera {
    Camera {
        lon,
        lat,
        width_km,
        tilt_deg: 40.0,
        heading_deg: 0.0,
        fov_deg: 28.0,
    }
}

fn mean(rgba: &[u8]) -> [f32; 3] {
    let mut s = [0f64; 3];
    for p in rgba.chunks_exact(4) {
        for c in 0..3 {
            s[c] += p[c] as f64;
        }
    }
    let n = (rgba.len() / 4) as f64;
    [(s[0] / n) as f32, (s[1] / n) as f32, (s[2] / n) as f32]
}

#[test]
fn slow_zoom_has_no_jump_in_mean_colour() {
    let Ok(mut r) = Renderer::new() else {
        eprintln!("skipping: no adapter");
        return;
    };
    let scene = scene();
    let style = Style {
        pixel_scale: 2,
        shadow_size: 1024,
        ..Style::default()
    };
    let (mut prev, mut worst, mut at) = (None::<[f32; 3]>, 0f32, 0f32);
    let mut w = 20.0f32;
    let mut frames = 0;
    // Zoom out from 20 km to 1500 km in 5 percent steps, then straight back in: the step the
    // texture is on must not flip on the way (hysteresis) and no frame may jump.
    let mut widths = Vec::new();
    while w < 1500.0 {
        widths.push(w);
        w *= 1.05;
    }
    let mut back = widths.clone();
    back.reverse();
    widths.extend(back);
    for w in widths {
        let f = r.render_to_rgba(&scene, &cam(-82.0, 41.0, w), &style, 320, 200);
        let m = mean(&f.rgba);
        if let Some(p) = prev {
            let d = (0..3).map(|c| (m[c] - p[c]).abs()).sum::<f32>() / 3.0;
            if d > 1.5 {
                eprintln!("  jump {d:.2} to {w:.1} km (frame {frames})");
            }
            if d > worst {
                worst = d;
                at = w;
            }
        }
        if let Ok(dir) = std::env::var("CW_SAVE_FRAMES") {
            let rgb: Vec<u8> = f
                .rgba
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            image::save_buffer(
                format!("{dir}/f{w:07.1}.png"),
                &rgb,
                f.width,
                f.height,
                image::ExtendedColorType::Rgb8,
            )
            .unwrap();
        }
        prev = Some(m);
        frames += 1;
    }
    eprintln!("{frames} frames, worst mean colour jump {worst:.2} at {at:.0} km");
    assert!(worst < 4.0, "mean colour jumped {worst:.2} at {at:.0} km");
}

#[test]
fn ground_texture_does_not_visibly_repeat() {
    let Ok(mut r) = Renderer::new() else {
        eprintln!("skipping: no adapter");
        return;
    };
    let mut scene = scene();
    scene.set_week(28);
    let style = Style {
        pixel_scale: 1,
        shadow_size: 1024,
        clouds: cw_render::CloudLevel::Off,
        roads: false,
        buildings: false,
        ..Style::default()
    };
    // Close views over forest and farmland; count distinct 24 px blocks.
    for (lon, lat, name) in [(-78.0, 41.6, "forest"), (-86.5, 40.2, "fields")] {
        let f = r.render_to_rgba(&scene, &cam(lon, lat, 14.0), &style, 480, 300);
        let mut seen = HashSet::new();
        let mut total = 0;
        for by in 0..(300 / 24) {
            for bx in 0..(480 / 24) {
                let mut block = Vec::new();
                for y in 0..24 {
                    for x in 0..24 {
                        let i = ((by * 24 + y) * 480 + bx * 24 + x) * 4;
                        block.extend_from_slice(&f.rgba[i..i + 3]);
                    }
                }
                seen.insert(block);
                total += 1;
            }
        }
        eprintln!("{name}: {} distinct of {total} blocks", seen.len());
        assert!(seen.len() * 10 >= total * 4, "{name}: texture repeats");
    }
}

#[test]
fn texel_step_holds_near_a_boundary_and_follows_a_real_zoom() {
    use cw_render::texel_step;
    let (tmin, tmax, min_px) = (0.125, 4.0, 2.0);
    // Stateless: smallest power of two that is at least min_px wide.
    assert_eq!(texel_step(tmin, tmax, min_px, 0.05, 0.0), 0.125);
    assert_eq!(texel_step(tmin, tmax, min_px, 0.07, 0.0), 0.25);
    // A camera hovering at the boundary never flips the step once it has one.
    let mut t = texel_step(tmin, tmax, min_px, 0.0624, 0.0);
    let first = t;
    for i in 0..200 {
        let px = 0.0624 * (1.0 + 0.1 * ((i as f32) * 0.9).sin());
        t = texel_step(tmin, tmax, min_px, px, t);
        assert_eq!(t, first, "flipped at frame {i}");
    }
    // A clear zoom out moves it up, then back in moves it down, one power of two at a time.
    let mut steps = Vec::new();
    let mut px = 0.04f32;
    while px < 3.0 {
        t = texel_step(tmin, tmax, min_px, px, t);
        if steps.last() != Some(&t) {
            steps.push(t);
        }
        px *= 1.03;
    }
    assert!(steps.windows(2).all(|w| w[1] == w[0] * 2.0), "{steps:?}");
    while px > 0.04 {
        t = texel_step(tmin, tmax, min_px, px, t);
        if steps.last() != Some(&t) {
            steps.push(t);
        }
        px /= 1.03;
    }
    assert_eq!(t, 0.125);
}
