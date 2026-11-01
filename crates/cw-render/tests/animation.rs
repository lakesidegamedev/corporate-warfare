//! The animated layer: smoke, construction stages, growth and the clock. A frame is a pure
//! function of the scene, the camera and the clock; with animations off the clock never matters.
//! With no graphics adapter at all (CI) the tests skip cleanly.

use cw_render::{
    Activity, Camera, Owner, Palette, Placement, Renderer, Scene, SiteStatus, Style, VoxelBox,
    VoxelModel,
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

/// A small plant with a tall chimney, standing in the tiny scene.
fn scene_with(activity: Activity) -> Scene {
    let mut scene = Scene::tiny();
    let boxes = vec![
        VoxelBox {
            min: [-8.0, -8.0, 0.0],
            max: [8.0, 8.0, 5.0],
            colour: [0x7A, 0x7F, 0x86],
            trim: false,
        },
        VoxelBox {
            min: [3.0, 3.0, 5.0],
            max: [5.0, 5.0, 14.0],
            colour: [0x56, 0x5B, 0x63],
            trim: false,
        },
    ];
    let model = VoxelModel {
        boxes,
        footprint: [16.0, 16.0],
    };
    let scale = Placement::scale_for_km(scene.voxel_km, &model, 1.0);
    scene.models.insert("energy/1".into(), model.clone());
    scene.models.insert("site/1".into(), model);
    scene.set_buildings(vec![Placement {
        model: if activity.status == Some(SiteStatus::Site) {
            "site/1"
        } else {
            "energy/1"
        }
        .into(),
        lon: -83.2,
        lat: 41.0,
        yaw: 0.0,
        scale,
        owner: Owner::Player,
        activity,
    }]);
    scene
}

fn running() -> Activity {
    Activity {
        status: Some(SiteStatus::Running),
        utilisation: 1.0,
        ..Activity::default()
    }
}

fn camera() -> Camera {
    Camera {
        lon: -83.2,
        lat: 41.0,
        width_km: 6.0,
        tilt_deg: 50.0,
        heading_deg: 0.0,
        fov_deg: 28.0,
    }
}

fn style(animate: bool, t: f64) -> Style {
    Style {
        shadow_size: 512,
        animate,
        time_s: t,
        ..Style::default()
    }
}

/// Scenes are identified by address, and these short lived ones can reuse one, so each frame
/// starts from a renderer that has forgotten the last.
fn frame(r: &mut Renderer, scene: &Scene, s: &Style) -> Vec<u8> {
    r.forget_scene();
    r.render_to_rgba(scene, &camera(), s, 160, 100).rgba
}

#[test]
fn still_mode_gives_the_same_frame_at_every_time() {
    let Some(mut r) = renderer() else { return };
    let scene = scene_with(running());
    let a = frame(&mut r, &scene, &style(false, 0.0));
    for t in [1.0, 17.3, 5000.0] {
        assert!(a == frame(&mut r, &scene, &style(false, t)), "time {t}");
    }
}

#[test]
fn one_clock_value_is_one_picture_and_the_wrap_is_invisible() {
    let Some(mut r) = renderer() else { return };
    let scene = scene_with(running());
    let a = frame(&mut r, &scene, &style(true, 7.25));
    assert!(a == frame(&mut r, &scene, &style(true, 7.25)));
    assert!(a == frame(&mut r, &scene, &style(true, 7.25 + 3600.0)));
    assert!(a == frame(&mut r, &scene, &style(true, 7.25 + 36000.0)));
}

#[test]
fn smoke_moves_with_the_clock_and_stays_in_the_palette() {
    let Some(mut r) = renderer() else { return };
    let scene = scene_with(running());
    let palette = Palette::day().colours();
    let a = frame(&mut r, &scene, &style(true, 0.0));
    let b = frame(&mut r, &scene, &style(true, 0.4));
    assert!(a != b, "smoke did not move");
    for f in [&a, &b] {
        for p in f.chunks_exact(4) {
            assert!(palette.contains(&[p[0], p[1], p[2]]), "off palette {p:?}");
        }
    }
    // Animations off is the plain building: neither smoke frame equals it.
    let still = frame(&mut r, &scene, &style(false, 0.0));
    assert!(a != still);
}

#[test]
fn a_mothballed_plant_gives_off_no_smoke() {
    let Some(mut r) = renderer() else { return };
    let scene = scene_with(Activity {
        status: Some(SiteStatus::Mothballed),
        ..Activity::default()
    });
    let a = frame(&mut r, &scene, &style(true, 0.0));
    let b = frame(&mut r, &scene, &style(true, 2.0));
    assert!(a == b, "a mothballed plant changed over time");
}

#[test]
fn utilisation_sets_how_much_smoke() {
    let Some(mut r) = renderer() else { return };
    let busy = scene_with(running());
    let idle = scene_with(Activity {
        utilisation: 0.0,
        ..running()
    });
    assert!(frame(&mut r, &busy, &style(true, 1.0)) != frame(&mut r, &idle, &style(true, 1.0)));
}

#[test]
fn a_site_shows_its_stage_even_with_animations_off() {
    let Some(mut r) = renderer() else { return };
    let site = |progress| {
        scene_with(Activity {
            status: Some(SiteStatus::Site),
            progress,
            ..Activity::default()
        })
    };
    let (a, b, c) = (site(0.1), site(0.5), site(0.9));
    let s = style(false, 0.0);
    let (fa, fb, fc) = (
        frame(&mut r, &a, &s),
        frame(&mut r, &b, &s),
        frame(&mut r, &c, &s),
    );
    assert!(fa != fb, "foundation and frame look the same");
    assert!(fb != fc, "frame and cladding look the same");
}

#[test]
fn the_crane_turns_only_with_animations_on() {
    let Some(mut r) = renderer() else { return };
    let scene = scene_with(Activity {
        status: Some(SiteStatus::Site),
        progress: 0.5,
        ..Activity::default()
    });
    assert!(frame(&mut r, &scene, &style(true, 0.0)) != frame(&mut r, &scene, &style(true, 12.0)));
    assert!(
        frame(&mut r, &scene, &style(false, 0.0)) == frame(&mut r, &scene, &style(false, 12.0))
    );
}

#[test]
fn a_new_building_grows_in_steps_and_ends_as_the_settled_building() {
    let Some(mut r) = renderer() else { return };
    let settled = scene_with(Activity {
        status: Some(SiteStatus::Mothballed),
        ..Activity::default()
    });
    let born = scene_with(Activity {
        status: Some(SiteStatus::Mothballed),
        born: Some(10.0),
        ..Activity::default()
    });
    let end = frame(&mut r, &settled, &style(true, 11.5));
    let early = frame(&mut r, &born, &style(true, 10.1));
    let mid = frame(&mut r, &born, &style(true, 10.5));
    let late = frame(&mut r, &born, &style(true, 11.5));
    assert!(early != mid, "growth is not progressing");
    assert!(mid != late);
    assert!(late == end, "a grown building must equal the settled one");
}
