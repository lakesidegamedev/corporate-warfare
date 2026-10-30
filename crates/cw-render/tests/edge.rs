//! The world edge of the map: beyond the scenario's detailed terrain the far land follows the
//! world's coastline, is calm (no long thin stripes, no streaks) and fades from the terrain
//! without a seam.

use std::path::Path;

use cw_render::{
    AssetDirs, Camera, Renderer, ScenarioSpec, Scene, SceneOptions, Style, apron_land_boxes,
    apron_seam_steps, project,
};

const IDS: [&str; 4] = [
    "great-lakes-2000",
    "texas-1981",
    "silicon-valley-1995",
    "ruhr-rhine-1991",
];

fn scenario(id: &str) -> Scene {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Scene::for_scenario(
        &AssetDirs {
            resources: &repo.join("apple/Resources"),
            baked: &repo.join("crates/cw-render/assets"),
        },
        ScenarioSpec::find(id).unwrap(),
        &SceneOptions::default(),
    )
    .unwrap()
}

/// The world's land picture, 2048 by 1024 over the whole earth, as land (true) or sea.
fn world_land() -> impl Fn(f32, f32) -> bool {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let img = image::open(repo.join("apple/Resources/earth-land.png"))
        .unwrap()
        .to_rgba8();
    move |lon: f32, lat: f32| {
        let x = (((lon + 180.0) / 360.0 * img.width() as f32).floor() as i64)
            .rem_euclid(img.width() as i64) as u32;
        let y = (((90.0 - lat) / 180.0 * img.height() as f32).floor() as i64)
            .clamp(0, img.height() as i64 - 1) as u32;
        img.get_pixel(x, y).0[3] > 127
    }
}

#[test]
fn far_land_stays_inside_the_world_coastline() {
    let land = world_land();
    for id in IDS {
        let scene = scenario(id);
        let hb = scene.heightmap.bounds;
        let boxes = apron_land_boxes(&scene);
        assert!(boxes.len() > 100, "{id}: far land exists");
        let mut checked = 0;
        for b in &boxes {
            let (lon, lat) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
            if lon > hb.west && lon < hb.east && lat > hb.south && lat < hb.north {
                continue;
            }
            // Land within three pixels of the picture's outline counts as on the coast.
            let near_land = (-3..=3)
                .any(|i| (-3..=3).any(|j| land(lon + i as f32 * 0.176, lat + j as f32 * 0.176)));
            checked += 1;
            assert!(near_land, "{id}: far land at {lon:.2},{lat:.2} is open sea");
        }
        assert!(checked > 100, "{id}: only {checked} far land pieces");
    }
}

#[test]
fn the_near_terrain_meets_the_far_land_without_a_wall() {
    for id in IDS {
        assert_eq!(apron_seam_steps(&scenario(id)), 0, "{id}: wall at the seam");
    }
}

/// 0 sea, 1 green land, 2 brown or other land, 3 snow.
fn kind(p: &[u8]) -> u8 {
    let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
    if r > 170 && g > 180 {
        3
    } else if b > g && b > r {
        0
    } else if g > r && g > b {
        1
    } else {
        2
    }
}

/// The longest run of one row where the picture changes kind every one to three pixels,
/// counted in changes, skipping `skip` (the terrain's own box, which has real fine detail).
fn longest_flicker(
    rgba: &[u8],
    w: usize,
    h: usize,
    skip: (usize, usize, usize, usize),
) -> (usize, usize) {
    let (mut best, mut at) = (0, 0);
    for y in 0..h {
        let (mut cur, mut run, mut last) = (0usize, 0usize, 255u8);
        for x in 0..=w {
            let inside = x >= skip.0 && x <= skip.2 && y >= skip.1 && y <= skip.3;
            let k = if x == w || inside {
                254
            } else {
                kind(&rgba[(y * w + x) * 4..])
            };
            if k == last {
                run += 1;
                continue;
            }
            if last < 254 && run <= 3 && k < 254 {
                cur += 1;
                if cur > best {
                    best = cur;
                    at = y;
                }
            } else {
                cur = 0;
            }
            run = 1;
            last = k;
        }
    }
    (best, at)
}

#[test]
fn the_metric_catches_stripes() {
    // Alternating sea and land every two pixels over a whole row is a stripe run.
    let (w, h) = (64, 2);
    let mut rgba = vec![255u8; w * h * 4];
    for x in 0..w {
        let c: [u8; 3] = if (x / 2) % 2 == 0 {
            [30, 70, 115]
        } else {
            [60, 110, 55]
        };
        for y in 0..h {
            rgba[(y * w + x) * 4..(y * w + x) * 4 + 3].copy_from_slice(&c);
        }
    }
    assert!(longest_flicker(&rgba, w, h, (0, 9, 0, 9)).0 > 20);
}

#[test]
fn zoomed_out_renders_have_no_stripes() {
    let (w, h) = (800u32, 500u32);
    for id in IDS {
        // A renderer per scenario: it keys its uploaded scene by the scene's address, which a
        // freed scene's successor can reuse.
        let mut renderer = match Renderer::new() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("skipping: {e}");
                return;
            }
        };
        let scene = scenario(id);
        let spec = ScenarioSpec::find(id).unwrap();
        let hb = scene.heightmap.bounds;
        for width_km in [3500.0, 7000.0, 14000.0] {
            let cam = Camera {
                lon: spec.centre.0,
                lat: spec.centre.1,
                width_km,
                tilt_deg: 0.0,
                heading_deg: 0.0,
                fov_deg: 28.0,
            };
            let f = renderer.render_to_rgba(&scene, &cam, &Style::default(), w, h);
            // The terrain's box on screen, with a margin: the far land is everything else.
            let corners = [
                (hb.west, hb.south),
                (hb.east, hb.south),
                (hb.west, hb.north),
                (hb.east, hb.north),
            ]
            .map(|(lon, lat)| project(&scene, &cam, w as f32, h as f32, lon, lat, 4.0).unwrap());
            let lo =
                |f: fn((f32, f32)) -> f32| corners.iter().copied().map(f).fold(f32::MAX, f32::min);
            let hi =
                |f: fn((f32, f32)) -> f32| corners.iter().copied().map(f).fold(f32::MIN, f32::max);
            let skip = (
                (lo(|p| p.0) - 36.0).max(0.0) as usize,
                (lo(|p| p.1) - 36.0).max(0.0) as usize,
                (hi(|p| p.0) + 36.0).max(0.0) as usize,
                (hi(|p| p.1) + 36.0).max(0.0) as usize,
            );
            let (worst, row) = longest_flicker(&f.rgba, f.width as usize, f.height as usize, skip);
            eprintln!("{id} {width_km} km: longest flicker {worst} at row {row}");
            assert!(
                worst <= 12,
                "{id} at {width_km} km: {worst} changes in a row at row {row} look like stripes"
            );
        }
    }
}
