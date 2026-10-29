//! A long career: per-phase week cost in windows ending at weeks 50, 150, 260 and 520, then
//! save size and export/import time at the end.
//!
//! cargo run --release -p cw-session --features phase-timing --example career_bench -- [scenario] [--weeks N]
use std::time::Instant;

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
            phases.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
            for (name, nanos) in phases.iter().take(12) {
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
}
