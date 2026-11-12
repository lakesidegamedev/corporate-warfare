//! Countryside props: placement is deterministic, stays off water, roads, rivers and buildings,
//! never exceeds its budget, and a wider view only thins the closer one out. No GPU needed.

use std::collections::HashSet;
use std::path::Path;
use std::sync::OnceLock;

use cw_render::{
    AssetDirs, CHUNK_MAX_CITY, CHUNK_MAX_PROPS, CITY_MAX_WIDTH_KM, ChunkKey, LINE_RIVER, PropIndex,
    PropKind, SCENARIOS, ScenarioSpec, Scene, SceneOptions, city_street_at, prop_chunk_mesh,
    prop_chunk_placed, prop_chunks, prop_stride,
};

fn scene() -> &'static Scene {
    static SCENE: OnceLock<Scene> = OnceLock::new();
    SCENE.get_or_init(|| {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let res = repo.join("apple/Resources");
        let baked = repo.join("crates/cw-render/assets");
        let mut s = Scene::for_scenario(
            &AssetDirs {
                resources: &res,
                baked: &baked,
            },
            ScenarioSpec::find("great-lakes-2000").unwrap(),
            &SceneOptions::default(),
        )
        .unwrap();
        s.set_week(28);
        s
    })
}

fn index() -> &'static PropIndex {
    static INDEX: OnceLock<PropIndex> = OnceLock::new();
    INDEX.get_or_init(|| PropIndex::build(scene()))
}

/// Where the sample views look: a city edge, farmland, north woods and a lakeshore.
const SPOTS: [(&str, f32, f32); 4] = [
    ("columbus", -83.0, 39.96),
    ("farms", -86.4, 40.4),
    ("woods", -84.2, 46.6),
    ("chicago", -87.7, 41.9),
];

fn placed_at(lon: f32, lat: f32, width: f32) -> Vec<(ChunkKey, Vec<cw_render::Placed>)> {
    let s = scene();
    let target = s.projection.to_km(lon, lat);
    prop_chunks(target, width)
        .into_iter()
        .map(|k| (k, prop_chunk_placed(s, index(), k)))
        .collect()
}

#[test]
fn placement_is_deterministic() {
    let s = scene();
    for (_, lon, lat) in SPOTS {
        let target = s.projection.to_km(lon, lat);
        for k in prop_chunks(target, 20.0).into_iter().take(6) {
            let a = prop_chunk_mesh(s, index(), k);
            let b = prop_chunk_mesh(s, index(), k);
            assert_eq!(a.vertices.len(), b.vertices.len());
            assert!(
                a.vertices
                    .iter()
                    .zip(&b.vertices)
                    .all(|(p, q)| p.pos == q.pos && p.colour == q.colour),
                "chunk {k:?} differs between builds"
            );
        }
    }
}

#[test]
fn nothing_stands_on_water_roads_rivers_or_buildings() {
    let s = scene();
    let mut checked = 0usize;
    for (name, lon, lat) in SPOTS {
        for (_, props) in placed_at(lon, lat, 25.0) {
            for p in props {
                let (plon, plat) = s.projection.to_lonlat(p.x, p.y);
                let kind = s.mask.at(plon, plat)[2];
                assert!(
                    kind != 1 && kind != 2,
                    "{name}: {:?} on water at {plon},{plat}",
                    p.kind
                );
                for line in &s.lines {
                    let clear = if line.class == LINE_RIVER { 0.12 } else { 0.09 };
                    for w in line.points.windows(2) {
                        let a = s.projection.to_km(w[0].0, w[0].1);
                        let b = s.projection.to_km(w[1].0, w[1].1);
                        let d = seg_dist((p.x, p.y), a, b);
                        assert!(
                            d >= clear,
                            "{name}: {:?} {d:.3} km from a line of class {}",
                            p.kind,
                            line.class
                        );
                    }
                }
                for b in &s.buildings {
                    let Some(m) = s.models.get(&b.model) else {
                        continue;
                    };
                    let (bx, by) = s.projection.to_km(b.lon, b.lat);
                    let half = m.footprint[0].max(m.footprint[1]) * s.voxel_km * b.scale / 2.0;
                    assert!(
                        (p.x - bx).abs() >= half || (p.y - by).abs() >= half,
                        "{name}: {:?} inside a building footprint",
                        p.kind
                    );
                }
                checked += 1;
            }
        }
    }
    assert!(checked > 500, "only {checked} props checked");
}

fn seg_dist(p: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 {
        (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p.0 - (a.0 + t * dx)).hypot(p.1 - (a.1 + t * dy))
}

#[test]
fn counts_stay_inside_the_budget() {
    let t = std::time::Instant::now();
    let mut built = 0;
    for (name, lon, lat) in SPOTS {
        for width in [8.0f32, 25.0, 45.0, 60.0] {
            let chunks = placed_at(lon, lat, width);
            let (mut total, mut city_total) = (0, 0);
            for (k, props) in &chunks {
                // City lots have their own budget; trees, scrub and farms keep the old one.
                let city = props
                    .iter()
                    .filter(|p| p.half != [0.0, 0.0] || p.kind == PropKind::House)
                    .count();
                let rest = props.len() - city;
                assert!(rest <= CHUNK_MAX_PROPS, "{name} {k:?}: {rest}");
                assert!(
                    city <= CHUNK_MAX_CITY,
                    "{name} {k:?}: {city} city buildings"
                );
                total += rest;
                city_total += city;
                built += 1;
            }
            eprintln!(
                "{name} at {width} km: {} chunks, {total} props, {city_total} city buildings",
                chunks.len()
            );
            assert!(total <= 16_000, "{name} at {width} km: {total} props");
            assert!(
                city_total <= 30_000,
                "{name} at {width} km: {city_total} city buildings"
            );
        }
    }
    eprintln!("{built} chunks placed in {:?}", t.elapsed());
}

#[test]
fn only_the_close_band_has_props() {
    let s = scene();
    let target = s.projection.to_km(-83.0, 39.96);
    assert!(prop_stride(60.0).is_some());
    assert!(prop_stride(60.5).is_none());
    assert!(prop_chunks(target, 150.0).is_empty());
    assert!(prop_chunks(target, 1500.0).is_empty());
    assert_eq!(prop_stride(10.0), Some(1));
    assert!(prop_stride(55.0) > prop_stride(25.0));
}

#[test]
fn a_wider_view_is_a_subset_of_the_closer_one() {
    let s = scene();
    // The trees of one 8 km square at stride 2 are also there at stride 1.
    let (lon, lat) = (-84.2, 46.6);
    let c = s.projection.to_km(lon, lat);
    let at = |stride: u8| -> HashSet<(i32, i32)> {
        let mut set = HashSet::new();
        let ck = 4.0 * stride as f32;
        let (ix, iy) = ((c.0 / ck).floor() as i32, (c.1 / ck).floor() as i32);
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let k = ChunkKey {
                ix: ix + dx,
                iy: iy + dy,
                stride,
            };
            for p in prop_chunk_placed(s, index(), k) {
                if p.kind == PropKind::Tree {
                    set.insert(((p.x * 1000.0) as i32, (p.y * 1000.0) as i32));
                }
            }
        }
        set
    };
    let near: HashSet<_> = {
        let mut set = HashSet::new();
        let (ix, iy) = ((c.0 / 4.0).floor() as i32, (c.1 / 4.0).floor() as i32);
        for dy in -2..4 {
            for dx in -2..4 {
                for p in prop_chunk_placed(
                    s,
                    index(),
                    ChunkKey {
                        ix: ix + dx,
                        iy: iy + dy,
                        stride: 1,
                    },
                ) {
                    set.insert(((p.x * 1000.0) as i32, (p.y * 1000.0) as i32));
                }
            }
        }
        set
    };
    let wide = at(2);
    assert!(wide.len() > 50);
    let inside = wide.iter().filter(|p| near.contains(p)).count();
    assert!(
        inside as f32 >= wide.len() as f32 * 0.9,
        "{inside} of {} wide-band trees are also in the close band",
        wide.len()
    );
}

#[test]
fn every_scenario_builds_props_without_panic() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let res = repo.join("apple/Resources");
    let baked = repo.join("crates/cw-render/assets");
    for spec in SCENARIOS.iter().filter(|s| s.id != "great-lakes-2000") {
        let s = Scene::for_scenario(
            &AssetDirs {
                resources: &res,
                baked: &baked,
            },
            spec,
            &SceneOptions::default(),
        )
        .unwrap();
        let index = PropIndex::build(&s);
        let target = s.projection.to_km(spec.centre.0, spec.centre.1);
        for k in prop_chunks(target, 25.0).into_iter().take(4) {
            let _ = prop_chunk_mesh(&s, &index, k);
        }
    }
}

/// Downtown, the inner city and the docks of three cities, close up.
const CITIES: [(&str, f32, f32); 3] = [
    ("cleveland", -81.69, 41.49),
    ("chicago", -87.65, 41.88),
    ("detroit", -83.05, 42.33),
];

#[test]
fn city_buildings_fill_the_blocks_and_never_stand_on_a_street() {
    let s = scene();
    for (name, lon, lat) in CITIES {
        let mut buildings = 0usize;
        let mut houses = 0usize;
        for (_, props) in placed_at(lon, lat, 10.0) {
            for p in props {
                if p.half == [0.0, 0.0] {
                    continue;
                }
                if p.kind == PropKind::House {
                    houses += 1;
                } else {
                    assert_eq!(p.kind, PropKind::Building);
                    buildings += 1;
                }
                // Every point of the footprint (on a 6 m grid) is block ground as the shader
                // draws it, never a street or an avenue.
                let [hw, hd] = p.half;
                let n = 6;
                for a in 0..=n {
                    for b in 0..=n {
                        let x = p.x - hw + 2.0 * hw * a as f32 / n as f32;
                        let y = p.y - hd + 2.0 * hd * b as f32 / n as f32;
                        assert!(
                            !city_street_at(s, x, y),
                            "{name}: a {:?} at ({:.3}, {:.3}) covers a street at ({x:.3}, {y:.3})",
                            p.kind,
                            p.x,
                            p.y
                        );
                    }
                }
            }
        }
        eprintln!("{name} at 10 km: {buildings} city buildings, {houses} houses");
        assert!(buildings > 300, "{name}: only {buildings} city buildings");
    }
}

#[test]
fn city_buildings_thin_out_by_count_and_stop_past_their_band() {
    let s = scene();
    let (_, lon, lat) = CITIES[0];
    let target = s.projection.to_km(lon, lat);
    let count = |width: f32| -> usize {
        prop_chunks(target, width)
            .into_iter()
            .map(|k| {
                prop_chunk_placed(s, index(), k)
                    .into_iter()
                    .filter(|p| p.kind == PropKind::Building)
                    .filter(|p| (p.x - target.0).abs() < 4.0 && (p.y - target.1).abs() < 4.0)
                    .count()
            })
            .sum()
    };
    let (near, mid, far) = (count(10.0), count(20.0), count(CITY_MAX_WIDTH_KM + 2.0));
    eprintln!(
        "buildings within 4 km of downtown: {near} at 10 km, {mid} at 20 km, {far} past the band"
    );
    assert!(near > 100);
    assert!(mid < near / 2, "a wider band keeps fewer: {mid} of {near}");
    assert_eq!(far, 0, "no city buildings past {CITY_MAX_WIDTH_KM} km");
    // The same lot gives the same building at the same size in both bands: positions of the wider
    // band are a subset of the closer one's.
    let at = |width: f32| -> HashSet<(i32, i32, i32)> {
        prop_chunks(target, width)
            .into_iter()
            .flat_map(|k| prop_chunk_placed(s, index(), k))
            .filter(|p| p.kind == PropKind::Building)
            .filter(|p| (p.x - target.0).abs() < 3.0 && (p.y - target.1).abs() < 3.0)
            .map(|p| {
                (
                    (p.x * 1000.0) as i32,
                    (p.y * 1000.0) as i32,
                    (p.half[0] * 1e4) as i32,
                )
            })
            .collect()
    };
    let (close, wide) = (at(10.0), at(20.0));
    assert!(!wide.is_empty());
    assert!(wide.is_subset(&close), "a wider band only drops buildings");
}
