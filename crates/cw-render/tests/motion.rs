//! The map in motion: a pan and a zoom from the region view down to the streets of Cleveland and
//! back out, frame after frame, as the camera moves. No frame may show a near-black patch over the
//! land or water it should draw, and inside the props band no frame may lose its trees and city
//! buildings while a new band is built (the last complete band stands in). Skips cleanly with no
//! GPU adapter.

use std::path::Path;

use cw_render::{
    AssetDirs, Camera, Renderer, ScenarioSpec, Scene, SceneOptions, Style, prop_stride,
};

fn scene() -> Scene {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scene::for_scenario(
        &AssetDirs {
            resources: &repo.join("apple/Resources"),
            baked: &repo.join("crates/cw-render/assets"),
        },
        ScenarioSpec::find("great-lakes-2000").unwrap(),
        &SceneOptions::default(),
    )
    .unwrap();
    s.set_week(28);
    s
}

/// The darkest 8 by 8 block of a frame, as its mean of the brightest channel: a hole or an
/// undrawn edge is the clear colour or black, far below any palette colour of the day map.
fn darkest_block(rgba: &[u8], w: u32, h: u32) -> (f32, u32, u32) {
    let mut worst = (f32::MAX, 0, 0);
    for by in 0..h / 8 {
        for bx in 0..w / 8 {
            let mut sum = 0u32;
            for y in 0..8 {
                for x in 0..8 {
                    let i = (((by * 8 + y) * w + bx * 8 + x) * 4) as usize;
                    sum += rgba[i].max(rgba[i + 1]).max(rgba[i + 2]) as u32;
                }
            }
            let mean = sum as f32 / 64.0;
            if mean < worst.0 {
                worst = (mean, bx * 8, by * 8);
            }
        }
    }
    worst
}

#[test]
fn a_pan_and_a_zoom_never_show_black_or_drop_the_props() {
    let Ok(mut r) = Renderer::new() else {
        eprintln!("skipping: no adapter");
        return;
    };
    let scene = scene();
    let style = Style {
        pixel_scale: 1,
        shadow_size: 1024,
        ..Style::default()
    };
    let (w, h) = (320u32, 200u32);
    // In from 160 km to 6 km over Cleveland while drifting east, a pan along the lake shore, then
    // back out: every props band edge is crossed both ways.
    let mut path: Vec<Camera> = Vec::new();
    let base = Camera {
        lon: -81.9,
        lat: 41.45,
        width_km: 160.0,
        tilt_deg: 45.0,
        heading_deg: 0.0,
        fov_deg: 28.0,
    };
    let mut c = base;
    for _ in 0..34 {
        path.push(c);
        c.width_km *= 0.9;
        c.lon += 0.004 * c.width_km / 10.0;
    }
    for _ in 0..12 {
        path.push(c);
        c.lon += 0.02;
        c.heading_deg += 2.0;
    }
    for _ in 0..34 {
        path.push(c);
        c.width_km /= 0.9;
    }
    let mut darkest = f32::MAX;
    let mut stand_ins = 0;
    let mut last_stride = None;
    for (i, cam) in path.iter().enumerate() {
        let f = r.render_to_rgba(&scene, cam, &style, w, h);
        let (mean, x, y) = darkest_block(&f.rgba, f.width, f.height);
        darkest = darkest.min(mean);
        assert!(
            mean >= 24.0,
            "frame {i} at {:.1} km: a near-black patch at ({x}, {y}), brightest channel mean {mean:.1}",
            cam.width_km
        );
        let s = &f.stats;
        if prop_stride(cam.width_km).is_some() {
            assert!(
                s.prop_chunks > 0,
                "frame {i} at {:.1} km: the props band drew no chunk",
                cam.width_km
            );
            if s.props_waiting > 0 {
                stand_ins += 1;
                assert_ne!(
                    Some(s.prop_stride),
                    prop_stride(cam.width_km),
                    "frame {i}: chunks are waiting, so the last complete band stands in"
                );
            }
        } else {
            assert_eq!(s.prop_chunks, 0, "frame {i}: props past their band");
        }
        if last_stride != Some(s.prop_stride) {
            eprintln!(
                "frame {i}: {:.1} km, band {} ({} chunks, {} waiting)",
                cam.width_km, s.prop_stride, s.prop_chunks, s.props_waiting
            );
            last_stride = Some(s.prop_stride);
        }
    }
    eprintln!(
        "{} frames, darkest block {darkest:.1}, {stand_ins} frames drawn with the last band standing in",
        path.len()
    );
}
