//! Weekly tick timing per scenario.
//!
//! cargo run --release -p cw-session --example week_bench -- [scenario-id ...] [--weeks N]
use std::time::Instant;

fn main() {
    let mut weeks: u32 = 104;
    let mut scenarios: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--weeks" {
            weeks = args.next().and_then(|v| v.parse().ok()).unwrap_or(104);
        } else {
            scenarios.push(arg);
        }
    }
    if scenarios.is_empty() {
        scenarios = [
            "great-lakes-2000",
            "texas-1981",
            "silicon-valley-1995",
            "ruhr-rhine-1991",
        ]
        .map(String::from)
        .to_vec();
    }
    for scenario in scenarios {
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
        let t = Instant::now();
        let mut session = cw_session::Session::create(config).unwrap();
        let create_ms = t.elapsed().as_secs_f64() * 1000.0;
        let mut times: Vec<f64> = Vec::new();
        for _ in 0..weeks {
            let t = Instant::now();
            let r = session.advance(1);
            times.push(t.elapsed().as_secs_f64() * 1000.0);
            if r.pause.is_some() {
                for event in session.world.events.pending.iter_mut() {
                    event.resolved = true;
                }
                session.world.pending_pause = None;
                session.dismiss_pause();
            }
        }
        let phases = cw_sim::prof::take();
        let total: f64 = times.iter().sum();
        let mut sorted = times.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        println!(
            "{scenario}: create {create_ms:.0} ms, {weeks} weeks total {total:.0} ms, mean {:.2} ms, p50 {:.2}, p95 {:.2}, max {:.2}; hash {}",
            total / weeks as f64,
            sorted[sorted.len() / 2],
            sorted[sorted.len() * 95 / 100],
            sorted[sorted.len() - 1],
            session.canonical_hash()
        );
        let mut phases = phases;
        phases.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
        for (name, nanos) in phases.iter().take(16) {
            println!(
                "    {name:<34} {:>8.3} ms/week",
                *nanos as f64 / 1e6 / weeks as f64
            );
        }
    }
}
