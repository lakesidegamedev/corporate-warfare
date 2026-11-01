//! Render the vehicle, harbour and outline work: freight lanes between the scenario's regions,
//! trucks, trains and ships on them, harbours with cranes, and the hover and selection outline.
//!
//!     cargo run --release -p cw-render --example fleet_views -- <out_dir> [still|frames|ports]
//!
//! `still` writes the review views by day and night, `frames` writes 8 frames of a truck close
//! view, `ports` writes the harbour close views (Houston port, Duisburg Rhine barges, Chicago
//! rail yards) by day and night, and the hover and selected outline picture.

use std::path::{Path, PathBuf};

use cw_render::{
    AssetDirs, Camera, Lane, PlantSite, Renderer, ScenarioSpec, Scene, SceneOptions, SiteStatus,
    Style, layout_plants,
};

struct View {
    name: &'static str,
    scenario: &'static str,
    lon: f32,
    lat: f32,
    width_km: f32,
    tilt: f32,
}

const fn view(
    name: &'static str,
    scenario: &'static str,
    lon: f32,
    lat: f32,
    width_km: f32,
    tilt: f32,
) -> View {
    View {
        name,
        scenario,
        lon,
        lat,
        width_km,
        tilt,
    }
}

const REVIEW: [View; 11] = [
    view(
        "columbus-close",
        "great-lakes-2000",
        -83.0,
        40.0,
        25.0,
        50.0,
    ),
    view(
        "chicago-lakeshore-close",
        "great-lakes-2000",
        -87.6,
        41.85,
        25.0,
        50.0,
    ),
    view("houston-coast-close", "texas-1981", -95.2, 29.6, 25.0, 50.0),
    view("duisburg-close", "ruhr-rhine-1991", 6.76, 51.43, 25.0, 50.0),
    view("mid-150km", "great-lakes-2000", -83.0, 40.0, 150.0, 45.0),
    view("far-1500km", "great-lakes-2000", -83.0, 40.0, 1500.0, 30.0),
    view(
        "chicago-rail-yards",
        "great-lakes-2000",
        -87.75,
        41.8,
        12.0,
        45.0,
    ),
    view(
        "columbus-trucks-4km",
        "great-lakes-2000",
        -83.0,
        40.0,
        4.0,
        50.0,
    ),
    view(
        "houston-port-close",
        "texas-1981",
        -95.008,
        29.551,
        3.0,
        45.0,
    ),
    view(
        "duisburg-rhine-barges",
        "ruhr-rhine-1991",
        6.97,
        50.94,
        5.0,
        45.0,
    ),
    view(
        "chicago-port-close",
        "great-lakes-2000",
        -87.625,
        41.883,
        4.0,
        45.0,
    ),
];

fn save(path: &Path, w: u32, h: u32, rgba: &[u8]) {
    let rgb: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| [p[0], p[1], p[2]])
        .collect();
    image::save_buffer(path, &rgb, w, h, image::ExtendedColorType::Rgb8).expect("png");
}

/// Plants in the region nearest `(lon, lat)` and freight lanes between nearby regions.
pub fn populate(scene: &mut Scene, lon: f32, lat: f32) {
    let regions: Vec<(String, f32, f32)> = scene
        .regions
        .iter()
        .map(|r| (r.code.clone(), r.lon, r.lat))
        .collect();
    let nearest = regions
        .iter()
        .min_by(|a, b| {
            let d = |r: &(String, f32, f32)| (r.1 - lon).powi(2) + (r.2 - lat).powi(2);
            d(a).total_cmp(&d(b))
        })
        .map(|r| r.0.clone())
        .expect("a region");
    let mix = [
        ("energy", 9, true, SiteStatus::Running, 0.95, 0.0),
        (
            "chemical_industries",
            5,
            false,
            SiteStatus::Running,
            0.6,
            0.0,
        ),
        ("logistics", 6, true, SiteStatus::Running, 0.7, 0.0),
        ("manufacturing", 2, true, SiteStatus::Site, 0.0, 0.55),
        ("extraction", 4, false, SiteStatus::Running, 0.3, 0.0),
        ("financial", 3, false, SiteStatus::Running, 0.5, 0.0),
    ];
    let sites: Vec<PlantSite> = mix
        .iter()
        .enumerate()
        .map(|(i, (ind, n, player, status, util, progress))| PlantSite {
            region: nearest.clone(),
            industry: (*ind).into(),
            firm: i as u64 + 1,
            plants: *n,
            player: *player,
            status: *status,
            utilisation: *util,
            progress: *progress,
        })
        .collect();
    let placed = layout_plants(scene, &sites);
    scene.set_buildings(placed);
    let goods = [
        "fuel",
        "steel",
        "goods",
        "grain",
        "ore",
        "chemicals",
        "timber",
        "machinery",
    ];
    let km = scene.projection.km_per_deg_lon();
    let mut lanes = Vec::new();
    for (i, a) in regions.iter().enumerate() {
        for (j, b) in regions.iter().enumerate() {
            if j <= i {
                continue;
            }
            let d = (((a.1 - b.1) * km).powi(2) + ((a.2 - b.2) * 111.0).powi(2)).sqrt();
            if d > 700.0 {
                continue;
            }
            for (m, mode) in ["truck", "rail", "sea"].iter().enumerate() {
                lanes.push(Lane {
                    from: a.0.clone(),
                    to: b.0.clone(),
                    mode: (*mode).into(),
                    volume: 0.25 + 0.75 * (((i * 7 + j * 3 + m) % 5) as f32 / 4.0),
                    goods: goods[(i + j * 2 + m) % goods.len()].into(),
                });
            }
        }
    }
    println!("{} lanes", lanes.len());
    scene.set_lanes(lanes);
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out: PathBuf = args.next().map(PathBuf::from).expect("out dir");
    let mode = args.next().unwrap_or_else(|| "still".into());
    let only = args.next();
    std::fs::create_dir_all(&out).expect("out dir");
    let mut renderer = Renderer::new().expect("renderer");
    println!("adapter: {}", renderer.adapter().name);
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dirs = AssetDirs {
        resources: &repo.join("apple/Resources"),
        baked: &repo.join("crates/cw-render/assets"),
    };
    let (width, height) = (1200u32, 750u32);
    let mut scenes: Vec<(&str, Scene)> = Vec::new();
    for v in &REVIEW {
        if scenes.iter().any(|(id, _)| *id == v.scenario) {
            continue;
        }
        let spec = ScenarioSpec::find(v.scenario).expect("scenario");
        let scene = Scene::for_scenario(&dirs, spec, &SceneOptions::default()).expect("scene");
        scenes.push((v.scenario, scene));
    }
    for v in &REVIEW {
        if only.as_deref().is_some_and(|o| !v.name.contains(o)) {
            continue;
        }
        let scene = &mut scenes
            .iter_mut()
            .find(|(id, _)| *id == v.scenario)
            .unwrap()
            .1;
        if v.name.contains("barges") {
            populate(scene, 6.76, 51.43);
        } else {
            populate(scene, v.lon, v.lat);
        }
        let t0 = std::time::Instant::now();
        let (routes, harbours) = cw_render::fleet_summary(scene);
        println!(
            "{}: routes (truck, train, ship) {routes:?}, harbours {harbours} ({:.1}s)",
            v.name,
            t0.elapsed().as_secs_f32()
        );
        // Port views sit on the harbour nearest the nominal spot.
        let (lon, lat) = if v.name.contains("port") || v.name.contains("barges") {
            let km = scene.projection.km_per_deg_lon();
            cw_render::harbour_sites(scene)
                .into_iter()
                .min_by(|a, b| {
                    let d = |h: &(f32, f32)| {
                        ((h.0 - v.lon) * km).powi(2) + ((h.1 - v.lat) * 111.0).powi(2)
                    };
                    d(a).total_cmp(&d(b))
                })
                .unwrap_or((v.lon, v.lat))
        } else {
            (v.lon, v.lat)
        };
        let cam = Camera {
            lon,
            lat,
            width_km: v.width_km,
            tilt_deg: v.tilt,
            heading_deg: 0.0,
            fov_deg: 28.0,
        };
        // The clock value with the most ships lying within a few hundred metres of a harbour
        // near the view, so the port pictures show ships at the quays.
        let berth_t = {
            let km = scene.projection.km_per_deg_lon();
            let near = |lon: f32, lat: f32| {
                (((lon - cam.lon) * km).powi(2) + ((lat - cam.lat) * 111.0).powi(2)).sqrt() < 2.0
            };
            let harbours: Vec<(f32, f32)> = cw_render::harbour_sites(scene)
                .into_iter()
                .filter(|h| near(h.0, h.1))
                .collect();
            let mut best = (0usize, 7.3f32);
            if !harbours.is_empty() {
                for k in 0..120 {
                    let t = k as f32 * 5.0;
                    let n = cw_render::vehicles_at(scene, t)
                        .iter()
                        .filter(|a| {
                            a.0 == 2
                                && harbours.iter().any(|h| {
                                    (((a.3 - h.0) * km).powi(2) + ((a.4 - h.1) * 111.0).powi(2))
                                        .sqrt()
                                        < 0.35
                                })
                        })
                        .count();
                    if n > best.0 {
                        best = (n, t);
                    }
                }
            }
            best.1 as f64
        };
        let style = |night: bool, t: f64| Style {
            night,
            animate: true,
            time_s: t,
            pixel_scale: 2,
            ..Style::default()
        };
        if mode == "sites" {
            for (lon, lat) in cw_render::harbour_sites(scene) {
                println!("  harbour {lon:.3} {lat:.3}");
            }
            continue;
        }
        match mode.as_str() {
            "still" => {
                for (tag, night) in [("day", false), ("night", true)] {
                    let t = if v.width_km < 8.0 { berth_t } else { 7.3 };
                    let f = renderer.render_to_rgba(scene, &cam, &style(night, t), width, height);
                    save(
                        &out.join(format!("{}-{tag}.png", v.name)),
                        f.width,
                        f.height,
                        &f.rgba,
                    );
                }
            }
            "models" => {
                // One close view per vehicle type: the first vehicle of each, followed.
                if v.name != "columbus-close" {
                    continue;
                }
                let all = cw_render::vehicles_at(scene, 7.3);
                let wants: [(&str, u8, u8, u8, f32); 12] = [
                    ("truck-tanker", 0, 0, 0, 0.5),
                    ("truck-flatbed", 0, 1, 0, 0.5),
                    ("truck-box", 0, 2, 0, 0.5),
                    ("truck-hopper", 0, 3, 0, 0.5),
                    ("train-tank", 1, 0, 0, 1.2),
                    ("train-flat", 1, 1, 0, 1.2),
                    ("train-box", 1, 2, 0, 1.2),
                    ("train-hopper", 1, 3, 0, 1.2),
                    ("ship-bulk", 2, 9, 0, 1.2),
                    ("ship-tanker", 2, 9, 1, 1.2),
                    ("ship-container", 2, 9, 2, 1.2),
                    ("ship-barge", 2, 9, 3, 1.2),
                ];
                for (tag, kind, cargo, ship, w) in wants {
                    let Some(hit) = all.iter().find(|a| {
                        a.0 == kind && (cargo == 9 || a.1 == cargo) && (kind != 2 || a.2 == ship)
                    }) else {
                        println!("no {tag}");
                        continue;
                    };
                    let cam = Camera {
                        lon: hit.3,
                        lat: hit.4,
                        width_km: w,
                        tilt_deg: 40.0,
                        heading_deg: 20.0,
                        fov_deg: 28.0,
                    };
                    let f = renderer.render_to_rgba(scene, &cam, &style(false, 7.3), 900, 560);
                    save(
                        &out.join(format!("model-{tag}.png")),
                        f.width,
                        f.height,
                        &f.rgba,
                    );
                }
            }
            "frames" => {
                if v.name != "columbus-close" {
                    continue;
                }
                for k in 0..8 {
                    let f = renderer.render_to_rgba(
                        scene,
                        &cam,
                        &style(false, 3.0 + k as f64 * 0.75),
                        width,
                        height,
                    );
                    save(
                        &out.join(format!("{}-f{k:02}.png", v.name)),
                        f.width,
                        f.height,
                        &f.rgba,
                    );
                }
            }
            "outline" => {
                if v.name != "columbus-close" {
                    continue;
                }
                let first = scene
                    .buildings
                    .iter()
                    .position(|b| b.owner == cw_render::Owner::Player)
                    .expect("a player building");
                let rival = scene
                    .buildings
                    .iter()
                    .position(|b| b.owner == cw_render::Owner::Rival)
                    .expect("a rival building");
                let cam = Camera {
                    lon: scene.buildings[first].lon,
                    lat: scene.buildings[first].lat,
                    width_km: 3.0,
                    ..cam
                };
                for (tag, hover, selected, t) in [
                    ("hover-player", Some(first), None, 7.3),
                    ("hover-rival", Some(rival), None, 7.3),
                    ("selected-a", None, Some(first), 0.0),
                    ("selected-b", None, Some(first), 0.4),
                ] {
                    let st = Style {
                        hover,
                        selected,
                        ..style(false, t)
                    };
                    let f = renderer.render_to_rgba(scene, &cam, &st, width, height);
                    save(&out.join(format!("{tag}.png")), f.width, f.height, &f.rgba);
                }
            }
            _ => panic!("mode: still, frames or outline"),
        }
    }
}
