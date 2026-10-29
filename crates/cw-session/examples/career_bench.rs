//! A long career: per-phase week cost in windows ending at weeks 50, 150, 260 and 520, an
//! interleaved A/B of the weekly record reads (one-at-a-time against batched) at each of those
//! weeks, then save size, export/import time and the autosave stall at the end.
//!
//! cargo run --release -p cw-session --features phase-timing --example career_bench -- [scenario] [--weeks N]
use std::time::Instant;

use cw_sim::economy::{division_report, events_engine, sectors, systems};

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}

/// The archive and systems record reads as they were: a good at a time, the full division
/// report and a cell scan per division.
fn reads_before(world: &cw_sim::state::World, dataset: &cw_data::Dataset) -> i64 {
    let mut sum = 0i64;
    for g in systems::tracked_goods(dataset).map(|c| systems::good_week(world, dataset, c)) {
        if g.made | g.sold | g.imported | g.exported | g.stock != 0 {
            sum = sum.wrapping_add(g.ask_cents);
        }
    }
    for (h, _) in world.firms.iter() {
        let player = events_engine::player_side(world, h);
        for d in sectors::divisions_of(world, dataset, h) {
            for _ in 0..1 + player as usize {
                let r = division_report::report(world, dataset, h, d.region, d.industry);
                sum = sum.wrapping_add(r.map(|r| r.operating_profit_cents).unwrap_or(0));
                sum = sum.wrapping_add(sectors::share_bps(
                    world, dataset, h, d.region, d.industry, 0,
                ));
            }
        }
    }
    sum
}

/// The same reads batched: one goods pass, lean figures, shared cell and corridor reads.
fn reads_after(world: &cw_sim::state::World, dataset: &cw_data::Dataset) -> i64 {
    let mut sum = 0i64;
    for g in systems::goods_week(world, dataset) {
        sum = sum.wrapping_add(g.ask_cents);
    }
    let mut shares = sectors::ShareCache::default();
    let mut corridors = division_report::Corridors::default();
    let (mut p_shares, mut p_corridors) = (
        sectors::ShareCache::default(),
        division_report::Corridors::default(),
    );
    for (h, _) in world.firms.iter() {
        let player = events_engine::player_side(world, h);
        for d in sectors::divisions_of(world, dataset, h) {
            for pass in 0..1 + player as usize {
                let (s, c) = if pass == 0 {
                    (&mut shares, &mut corridors)
                } else {
                    (&mut p_shares, &mut p_corridors)
                };
                let r = division_report::figures(world, dataset, h, d.region, d.industry, c);
                sum = sum.wrapping_add(r.map(|r| r.operating_profit_cents).unwrap_or(0));
                sum = sum.wrapping_add(s.share_bps(world, dataset, h, d.region, d.industry));
            }
        }
    }
    sum
}

fn main() {
    let mut weeks: u32 = 520;
    let mut scenario = "great-lakes-2000".to_string();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--weeks" {
            weeks = args.next().and_then(|v| v.parse().ok()).unwrap_or(520);
        } else {
            scenario = arg;
        }
    }
    let config = cw_session::ScenarioConfig {
        seed: 7,
        scenario: cw_sim::state::ScenarioKind::Guided,
        mode: cw_sim::state::ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: cw_session::PriceSetup {
            scenario_id: scenario.clone(),
            path: None,
            ..Default::default()
        },
    };
    let mut session = cw_session::Session::create(config).unwrap();
    let windows = [50u32, 150, 260, 520];
    let width = 10u32;
    let mut times: Vec<f64> = Vec::new();
    for week in 1..=weeks {
        let in_window = windows.iter().any(|w| week > w - width && week <= *w);
        if week > 1 && windows.iter().any(|w| week == w - width + 1) {
            let _ = cw_sim::prof::take();
            times.clear();
        }
        let t = Instant::now();
        let r = session.advance(1);
        if in_window {
            times.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        if r.pause.is_some() {
            for event in session.world.events.pending.iter_mut() {
                event.resolved = true;
            }
            session.world.pending_pause = None;
            session.dismiss_pause();
        }
        if windows.contains(&week) {
            let mut phases = cw_sim::prof::take();
            let mut sorted = times.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
            println!(
                "week {week}: median {:.2} ms/week (window of {})",
                sorted[sorted.len() / 2],
                sorted.len()
            );
            let (mut a, mut b) = (Vec::new(), Vec::new());
            for _ in 0..31 {
                let t = Instant::now();
                let x = reads_before(&session.world, &session.dataset);
                a.push(t.elapsed().as_secs_f64() * 1000.0);
                let t = Instant::now();
                let y = reads_after(&session.world, &session.dataset);
                b.push(t.elapsed().as_secs_f64() * 1000.0);
                assert_eq!(x, y, "batched reads differ");
            }
            println!(
                "    A/B record reads: before {:.3} ms, after {:.3} ms (medians of 31, interleaved)",
                median(a),
                median(b)
            );
            phases.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            for (name, nanos) in phases.iter().take(26) {
                println!(
                    "    {name:<34} {:>8.3} ms/week",
                    *nanos as f64 / 1e6 / width as f64
                );
            }
        }
    }
    println!("hash {}", session.canonical_hash());
    let mut export = Vec::new();
    let mut import = Vec::new();
    let mut size = 0;
    for _ in 0..7 {
        let t = Instant::now();
        let bytes = session.export_save().unwrap();
        export.push(t.elapsed().as_secs_f64() * 1000.0);
        size = bytes.len();
        let t = Instant::now();
        let back = cw_session::Session::import_save(&bytes).unwrap();
        import.push(t.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(back.canonical_hash(), session.canonical_hash());
    }
    export.sort_by(|a, b| a.partial_cmp(b).unwrap());
    import.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "save {} KB, export median {:.1} ms, import median {:.1} ms",
        size / 1024,
        export[3],
        import[3]
    );
    // The autosave stall on the engine thread: export and write the file, as the desktop did,
    // against export alone with the write handed to another thread.
    let dir = std::env::temp_dir().join(format!("cw-career-bench-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (mut a, mut b) = (Vec::new(), Vec::new());
    for _ in 0..15 {
        let t = Instant::now();
        let bytes = session.export_save().unwrap();
        let temp = dir.join("autosave.tmp");
        std::fs::write(&temp, &bytes).unwrap();
        std::fs::rename(&temp, dir.join("autosave.cwsave")).unwrap();
        a.push(t.elapsed().as_secs_f64() * 1000.0);
        let t = Instant::now();
        let bytes = session.export_save().unwrap();
        b.push(t.elapsed().as_secs_f64() * 1000.0);
        let dir = dir.clone();
        std::thread::spawn(move || std::fs::write(dir.join("autosave2.tmp"), &bytes))
            .join()
            .unwrap()
            .unwrap();
    }
    std::fs::remove_dir_all(&dir).ok();
    println!(
        "autosave stall: export+write {:.1} ms, export only {:.1} ms (medians of 15, interleaved)",
        median(a),
        median(b)
    );
}
