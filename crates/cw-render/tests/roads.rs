//! Roads follow the ground without a GPU: sample the road chains against the terrain heights.

use cw_render::{LINE_INTERSTATE, Scene, ground_z, road_chains};

/// Lift the roads sit above the terrain, and the slack allowed between a straight piece of
/// road and the ground under it, in kilometres.
const LIFT: f32 = 0.03;
const SLACK: f32 = 0.06;

#[test]
fn roads_stay_on_the_ground_across_terrace_walls() {
    let mut scene = Scene::tiny();
    // Terraces by column: a high plateau, a step down, and a low plain with a short step.
    let w = scene.heightmap.w;
    for (i, l) in scene.levels.iter_mut().enumerate() {
        *l = match i % w {
            0..=4 => 6,
            5..=8 => 3,
            9..=11 => 2,
            _ => 1,
        };
    }
    let chains = road_chains(&scene);
    assert_eq!(chains.len(), 3);
    let step = scene.terrace_m * scene.exaggeration / 1000.0;
    let mut worst = 0.0f32;
    let mut crossed_wall = false;
    for (class, pts) in &chains {
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for w in pts.windows(2) {
            for k in 0..=16 {
                let t = k as f32 / 16.0;
                let p: Vec<f32> = (0..3)
                    .map(|i| w[0].xyz[i] + (w[1].xyz[i] - w[0].xyz[i]) * t)
                    .collect();
                let err = (p[2] - LIFT - ground_z(&scene, p[0], p[1])).abs();
                worst = worst.max(err);
                assert!(
                    err <= SLACK,
                    "class {class} is {err:.3} km off the ground at ({:.1}, {:.1})",
                    p[0],
                    p[1]
                );
                lo = lo.min(p[2]);
                hi = hi.max(p[2]);
            }
        }
        if *class == LINE_INTERSTATE {
            // The interstate climbs the ridge and drops off its far side.
            crossed_wall = hi - lo >= 2.0 * step;
        }
    }
    assert!(
        crossed_wall,
        "the test interstate never meets a terrace wall"
    );
    eprintln!("worst gap between road and ground: {worst:.4} km");
}

#[test]
fn road_geometry_is_deterministic() {
    let a = road_chains(&Scene::tiny());
    let b = road_chains(&Scene::tiny());
    assert_eq!(a.len(), b.len());
    for ((ca, pa), (cb, pb)) in a.iter().zip(&b) {
        assert_eq!(ca, cb);
        assert_eq!(pa.len(), pb.len());
        for (x, y) in pa.iter().zip(pb) {
            assert_eq!(x.xyz, y.xyz);
            assert_eq!(x.along, y.along);
        }
    }
}

#[test]
fn zoom_bands_keep_the_right_classes_and_stay_on_the_ground() {
    use cw_render::{LINE_INTERSTATE, ROAD_BANDS, road_chains_for_band};
    let scene = Scene::tiny();
    let full: usize = road_chains(&scene).iter().map(|c| c.1.len()).sum();
    for (i, band) in ROAD_BANDS.iter().enumerate() {
        let chains = road_chains_for_band(&scene, band);
        assert!(!chains.is_empty(), "band {i} drew nothing");
        let n: usize = chains.iter().map(|c| c.1.len()).sum();
        assert!(n <= full, "band {i} has more points than the full set");
        for (class, pts) in &chains {
            assert!(*class <= band.max_class);
            if i == 0 {
                assert_eq!(*class, LINE_INTERSTATE);
            }
            for p in pts {
                let err = (p.xyz[2] - LIFT - ground_z(&scene, p.xyz[0], p.xyz[1])).abs();
                assert!(err <= SLACK, "band {i} road {err:.3} km off the ground");
            }
        }
    }
}
