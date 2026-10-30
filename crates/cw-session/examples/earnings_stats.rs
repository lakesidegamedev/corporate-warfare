//! Results-day statistics for the earnings season (1.9): over several seeds of a scenario, how
//! big the analyst's misses and the price moves are, and whether the estimates lean one way.
//!
//! cargo run --release -p cw-session --example earnings_stats -- [seeds] [weeks] [scenario-id]
fn pct(sorted: &[i64], p: usize) -> i64 {
    if sorted.is_empty() {
        0
    } else {
        sorted[(sorted.len() - 1) * p / 100]
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(6);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(260);
    let scenario_id = args.next().unwrap_or_default();
    let mut surprise: Vec<i64> = Vec::new();
    let mut moves: Vec<i64> = Vec::new();
    let (mut at_bound, mut guided, mut met) = (0usize, 0usize, 0usize);
    for seed in 1..=seeds {
        let config = cw_session::ScenarioConfig {
            seed,
            scenario: cw_sim::state::ScenarioKind::Guided,
            mode: cw_sim::state::ScenarioMode::Sandbox,
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: cw_session::PriceSetup {
                scenario_id: scenario_id.clone(),
                path: None,
                timeline: None,
            },
        };
        let mut session = cw_session::Session::create(config).unwrap();
        for _ in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let tick = session.world.tick.0;
            for report in session.world.earnings.log.iter().filter(|r| r.week == tick) {
                surprise.push(report.surprise_bps as i64);
                moves.push(report.move_bps as i64);
                if report.surprise_bps.abs() >= 10_000 {
                    at_bound += 1;
                }
                if let Some(ok) = report.guidance_met {
                    guided += 1;
                    met += ok as usize;
                }
            }
        }
    }
    let n = surprise.len().max(1);
    let mean = surprise.iter().sum::<i64>() / n as i64;
    let mut abs_s: Vec<i64> = surprise.iter().map(|v| v.abs()).collect();
    let mut abs_m: Vec<i64> = moves.iter().map(|v| v.abs()).collect();
    let mut signed = surprise.clone();
    abs_s.sort();
    abs_m.sort();
    signed.sort();
    let over10 = abs_m.iter().filter(|m| **m > 1_000).count();
    println!(
        "scenario {} seeds {seeds} weeks {weeks}: {n} results days",
        if scenario_id.is_empty() {
            "default"
        } else {
            &scenario_id
        }
    );
    println!(
        "surprise bps: mean {mean}  median {}  |p50| {}  |p90| {}  at the bound {} ({}%)",
        pct(&signed, 50),
        pct(&abs_s, 50),
        pct(&abs_s, 90),
        at_bound,
        at_bound * 100 / n
    );
    println!(
        "move bps: |p50| {}  |p90| {}  max {}  over 10%: {} ({}%)",
        pct(&abs_m, 50),
        pct(&abs_m, 90),
        abs_m.last().copied().unwrap_or(0),
        over10,
        over10 * 100 / n
    );
    println!("guidance: {guided} ranges, {met} met");
}
