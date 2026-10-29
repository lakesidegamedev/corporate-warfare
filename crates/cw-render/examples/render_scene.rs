//! Render the Great Lakes 2000 scenario offscreen and time it.
//!
//!     cargo run --release -p cw-render --example render_scene -- <out_dir> [--no-timing]
//!
//! Writes scenario, close and turned views (day) plus a night close view as PNGs, then prints
//! frame times at 1600x1000 and 2560x1440 with vertex and draw-call counts. The same command
//! runs on a real GPU: the adapter name, backend and driver are printed first.

use std::path::{Path, PathBuf};
use std::time::Instant;

use cw_render::{Camera, Renderer, Scene, Style};

fn save_png(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    image::save_buffer(path, &rgb, w, h, image::ExtendedColorType::Rgb8).expect("write png");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("render-out"));
    let timing = !args.any(|a| a == "--no-timing");
    std::fs::create_dir_all(&out).expect("out dir");

    let mut renderer = match Renderer::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let a = renderer.adapter().clone();
    println!(
        "adapter: {} | backend {} | type {} | driver {}",
        a.name, a.backend, a.device_type, a.driver
    );

    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let t = Instant::now();
    let scene = Scene::great_lakes_2000(&repo).expect("scene");
    println!("scene built in {:.1}s", t.elapsed().as_secs_f32());

    let views = [
        ("scenario", Camera::scenario()),
        ("close", Camera::close()),
        ("turned", Camera::turned()),
    ];
    let day = Style {
        pixel_scale: 2,
        ..Style::default()
    };
    let night = Style {
        pixel_scale: 2,
        ..Style::night()
    };
    for (name, cam) in &views {
        let f = renderer.render_to_rgba(&scene, cam, &day, 1600, 1000);
        save_png(
            &out.join(format!("{name}-day.png")),
            f.width,
            f.height,
            &f.rgba,
        );
    }
    // Close-ups of the new road, rail, city and ground texture work.
    let spot = |lon: f32, lat: f32, width_km: f32, tilt_deg: f32, heading_deg: f32| Camera {
        lon,
        lat,
        width_km,
        tilt_deg,
        heading_deg,
        fov_deg: 28.0,
    };
    let closeups = [
        ("chicago-junction", spot(-87.75, 41.7, 150.0, 38.0, 0.0)),
        ("pittsburgh-rails", spot(-80.05, 40.45, 170.0, 42.0, 0.0)),
        ("ohio-farmland", spot(-83.6, 40.6, 150.0, 38.0, 0.0)),
        ("michigan-forest", spot(-84.9, 45.0, 170.0, 38.0, 0.0)),
        ("detroit-skyline", spot(-83.05, 42.3, 70.0, 48.0, 20.0)),
    ];
    for (name, cam) in &closeups {
        let f = renderer.render_to_rgba(&scene, cam, &day, 1600, 1000);
        save_png(&out.join(format!("{name}.png")), f.width, f.height, &f.rgba);
    }
    for (name, cam) in &views {
        let f = renderer.render_to_rgba(&scene, cam, &night, 1600, 1000);
        save_png(
            &out.join(format!("{name}-night.png")),
            f.width,
            f.height,
            &f.rgba,
        );
    }
    // Cloud check: the heaviest setting on the scenario view shows the outline, underside band
    // and ground shadow; the default (light) is in scenario-day.
    let heavy = Style {
        pixel_scale: 2,
        clouds: cw_render::CloudLevel::Heavy,
        ..Style::default()
    };
    let f = renderer.render_to_rgba(&scene, &Camera::scenario(), &heavy, 1600, 1000);
    save_png(
        &out.join("clouds-heavy-scenario.png"),
        f.width,
        f.height,
        &f.rgba,
    );
    if !timing {
        return;
    }
    // Timing renders at full resolution (pixel scale 1), which is the worst case.
    let day = Style::default();
    println!("\nview      size       frames  mean ms   min ms   vertices  triangles  draw calls");
    for (w, h) in [(1600u32, 1000u32), (2560, 1440)] {
        for (name, cam) in &views {
            renderer.render_to_rgba(&scene, cam, &day, w, h); // warm up
            let mut times = Vec::new();
            let mut last = None;
            for _ in 0..5 {
                let f = renderer.render_to_rgba(&scene, cam, &day, w, h);
                times.push((
                    f.stats.gpu_wait.as_secs_f64() * 1000.0,
                    f.stats.readback.as_secs_f64() * 1000.0,
                ));
                last = Some(f.stats);
            }
            let s = last.unwrap();
            let mean = times.iter().map(|t| t.0).sum::<f64>() / times.len() as f64;
            let min = times.iter().map(|t| t.0).fold(f64::MAX, f64::min);
            println!(
                "{name:<9} {w}x{h}  {:>5}  {mean:>8.1} {min:>8.1}  {:>9}  {:>9}  {:>5}",
                times.len(),
                s.vertices,
                s.triangles,
                s.draw_calls
            );
        }
    }
}
