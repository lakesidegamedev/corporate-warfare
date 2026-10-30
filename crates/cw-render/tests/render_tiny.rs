//! Renders a tiny synthetic scene on whatever adapter exists. With no adapter at all (CI) the
//! tests skip cleanly instead of failing.

use cw_render::{Camera, CloudLevel, Palette, Renderer, Scene, Style};

fn renderer() -> Option<Renderer> {
    match Renderer::new() {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("skipping: {e}");
            None
        }
    }
}

fn camera() -> Camera {
    Camera {
        lon: -83.0,
        lat: 41.0,
        width_km: 220.0,
        tilt_deg: 35.0,
        heading_deg: 20.0,
        fov_deg: 28.0,
    }
}

fn small() -> Style {
    Style {
        shadow_size: 512,
        ..Style::default()
    }
}

#[test]
fn every_pixel_is_a_palette_colour() {
    let Some(mut r) = renderer() else { return };
    let scene = Scene::tiny();
    for night in [false, true] {
        let style = Style { night, ..small() };
        let f = r.render_to_rgba(&scene, &camera(), &style, 128, 80);
        assert_eq!((f.width, f.height), (128, 80));
        let palette = if night {
            Palette::night()
        } else {
            Palette::day()
        }
        .colours();
        let mut seen = std::collections::HashSet::new();
        for p in f.rgba.chunks_exact(4) {
            assert_eq!(p[3], 255);
            let c = [p[0], p[1], p[2]];
            assert!(
                palette.contains(&c),
                "pixel {c:?} is not a palette colour (night {night})"
            );
            seen.insert(c);
        }
        assert!(
            seen.len() >= 3,
            "frame is nearly blank: {} colours",
            seen.len()
        );
    }
}

#[test]
fn rendering_is_deterministic_and_counts_draws() {
    let Some(mut r) = renderer() else { return };
    let scene = Scene::tiny();
    let a = r.render_to_rgba(&scene, &camera(), &small(), 96, 64);
    let b = r.render_to_rgba(&scene, &camera(), &small(), 96, 64);
    assert_eq!(a.rgba, b.rgba);
    assert!(a.stats.vertices > 0 && a.stats.triangles > 0);
    // Shadow pass: terrain and buildings. Main pass: terrain, the horizon apron, buildings, roads
    // and clouds.
    assert_eq!(a.stats.draw_calls, 7);
}

#[test]
fn pixel_scale_repeats_pixels() {
    let Some(mut r) = renderer() else { return };
    let scene = Scene::tiny();
    let style = Style {
        pixel_scale: 2,
        ..small()
    };
    let f = r.render_to_rgba(&scene, &camera(), &style, 96, 64);
    for y in (0..64usize).step_by(2) {
        for x in (0..96usize).step_by(2) {
            let at = |xx: usize, yy: usize| &f.rgba[(yy * 96 + xx) * 4..(yy * 96 + xx) * 4 + 3];
            assert_eq!(at(x, y), at(x + 1, y));
            assert_eq!(at(x, y), at(x, y + 1));
        }
    }
}

#[test]
fn roads_are_drawn_and_clouds_levels_work() {
    let Some(mut r) = renderer() else { return };
    let scene = Scene::tiny();
    let on = r.render_to_rgba(&scene, &camera(), &small(), 160, 100);
    let off = r.render_to_rgba(
        &scene,
        &camera(),
        &Style {
            roads: false,
            ..small()
        },
        160,
        100,
    );
    assert_ne!(on.rgba, off.rgba, "roads left no pixels");
    assert_eq!(off.stats.draw_calls, 6);
    let none = r.render_to_rgba(
        &scene,
        &camera(),
        &Style {
            clouds: CloudLevel::Off,
            ..small()
        },
        96,
        64,
    );
    assert_eq!(none.stats.draw_calls, 6);
}
