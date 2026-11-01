//! Light after dark and the slow motion of the sky and the sea: lit city pixels, drifting clouds
//! and moving water. A frame is a pure function of the scene, the camera and the clock; with
//! animations off the clock never matters. With no graphics adapter at all (CI) the render tests
//! skip cleanly.

use std::path::Path;

use cw_render::{
    AssetDirs, Camera, Palette, Renderer, ScenarioSpec, Scene, SceneOptions, Style, WIND,
    cloud_drift_uv,
};

fn renderer() -> Option<Renderer> {
    match Renderer::new() {
        Ok(r) => Some(r),
        Err(e) => {
            eprintln!("skipping: {e}");
            None
        }
    }
}

fn load() -> Scene {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Scene::for_scenario(
        &AssetDirs {
            resources: &repo.join("apple/Resources"),
            baked: &repo.join("crates/cw-render/assets"),
        },
        ScenarioSpec::find("great-lakes-2000").unwrap(),
        &SceneOptions::default(),
    )
    .unwrap()
}

fn camera(lon: f32, lat: f32, width_km: f32) -> Camera {
    Camera {
        lon,
        lat,
        width_km,
        tilt_deg: 50.0,
        heading_deg: 0.0,
        fov_deg: 28.0,
    }
}

fn chicago() -> Camera {
    camera(-87.6, 41.85, 25.0)
}

fn lake() -> Camera {
    camera(-87.2, 42.6, 12.0)
}

fn far() -> Camera {
    camera(-83.0, 40.0, 1500.0)
}

fn style(night: bool, animate: bool, t: f64) -> Style {
    Style {
        night,
        animate,
        time_s: t,
        shadow_size: 512,
        ..Style::default()
    }
}

fn frame(r: &mut Renderer, scene: &Scene, cam: &Camera, s: &Style) -> Vec<u8> {
    r.forget_scene();
    r.render_to_rgba(scene, cam, s, 320, 200).rgba
}

fn count_of(rgba: &[u8], colour: [u8; 3]) -> usize {
    rgba.chunks_exact(4)
        .filter(|p| [p[0], p[1], p[2]] == colour)
        .count()
}

#[test]
fn still_mode_is_the_same_picture_at_every_time_by_night_and_with_clouds() {
    let Some(mut r) = renderer() else { return };
    let scene = load();
    for (cam, night) in [(chicago(), true), (far(), false), (far(), true)] {
        let a = frame(&mut r, &scene, &cam, &style(night, false, 0.0));
        for t in [1.0, 17.3, 5000.0, 123456.0] {
            assert!(
                a == frame(&mut r, &scene, &cam, &style(night, false, t)),
                "time {t}"
            );
        }
    }
}

#[test]
fn one_clock_value_is_one_picture_at_night() {
    let Some(mut r) = renderer() else { return };
    let scene = load();
    let a = frame(&mut r, &scene, &chicago(), &style(true, true, 7.25));
    assert!(a == frame(&mut r, &scene, &chicago(), &style(true, true, 7.25)));
    // No clouds this close, so the clock wrap is invisible too.
    assert!(
        a == frame(
            &mut r,
            &scene,
            &chicago(),
            &style(true, true, 7.25 + 3600.0)
        )
    );
}

#[test]
fn night_views_have_lit_pixels_and_only_palette_colours() {
    let Some(mut r) = renderer() else { return };
    let scene = load();
    let night = Palette::night();
    let palette = night.colours();
    let warm = night.ramps[cw_render::ramp_by_name("signal")][1];
    for (cam, min_lit) in [(chicago(), 40), (far(), 3)] {
        let f = frame(&mut r, &scene, &cam, &style(true, true, 3.0));
        for p in f.chunks_exact(4) {
            assert!(palette.contains(&[p[0], p[1], p[2]]), "off palette {p:?}");
        }
        let lit = count_of(&f, warm);
        assert!(lit >= min_lit, "only {lit} lit pixels");
        let day = frame(&mut r, &scene, &cam, &style(false, true, 3.0));
        assert!(count_of(&day, warm) < lit, "day is not darker than night");
    }
}

#[test]
fn clouds_drift_and_the_picture_stays_in_the_palette() {
    let Some(mut r) = renderer() else { return };
    let scene = load();
    let palette = Palette::day().colours();
    let a = frame(&mut r, &scene, &far(), &style(false, true, 0.0));
    let b = frame(&mut r, &scene, &far(), &style(false, true, 900.0));
    let c = frame(&mut r, &scene, &far(), &style(false, true, 900.0));
    assert!(a != b, "the clouds did not move");
    assert!(b == c);
    for f in [&a, &b] {
        for p in f.chunks_exact(4) {
            assert!(palette.contains(&[p[0], p[1], p[2]]), "off palette {p:?}");
        }
    }
}

#[test]
fn a_lake_moves_in_steps_and_the_wrap_is_invisible() {
    let Some(mut r) = renderer() else { return };
    let scene = load();
    let a = frame(&mut r, &scene, &lake(), &style(false, true, 10.0));
    // Within one step of the swell nothing moves; a step later the waves have.
    let same = frame(&mut r, &scene, &lake(), &style(false, true, 10.0 + 0.4));
    let later = frame(&mut r, &scene, &lake(), &style(false, true, 10.0 + 5.0));
    assert!(a == same, "a lake moved inside one step");
    assert!(a != later, "the lake never moved");
    // The clock wrap is invisible.
    assert!(a == frame(&mut r, &scene, &lake(), &style(false, true, 10.0 + 3600.0)));
}

#[test]
fn the_cloud_drift_follows_the_wind_and_never_jumps() {
    let img = [80.0f32, 48.0];
    let km = [85.0f32, 111.2];
    assert_eq!(cloud_drift_uv(5000.0, false, img, km), [0.0, 0.0]);
    let d = |t: f64| cloud_drift_uv(t, true, img, km);
    assert_eq!(d(0.0), [0.0, 0.0]);
    // East and north: the image is read further west and south of the point.
    let a = d(1000.0);
    assert!(a[0] > 0.0 && a[1] > 1.0, "{a:?}");
    assert!(a[0] / (2.0 - a[1]) > 0.0);
    // The ratio of the two offsets is the wind's.
    let want = (WIND[0] / km[0] / img[0]) / (WIND[1] / km[1] / img[1]);
    assert!((a[0] / (2.0 - a[1]) - want).abs() / want < 1e-3);
    // The offset is folded into one mirror period (2), so it is smooth modulo 2 for ever.
    for t in [0.0f64, 3.0e5, 7.7e6, 9.0e9] {
        let (p, q) = (d(t), d(t + 1.0));
        for k in 0..2 {
            let step = (q[k] - p[k]).rem_euclid(2.0);
            let step = step.min(2.0 - step);
            assert!(step < 1e-3, "jump at {t}: {p:?} {q:?}");
        }
    }
}
