//! Share price against fair value for every seed firm at opening, week 5 and week 52.
//!
//! cargo run --release -p cw-session --example price_census -- [seeds]
use cw_data::{Difficulty, PricePath};
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::state::{ScenarioKind, ScenarioMode};

fn ratios(session: &Session) -> Vec<(f64, String, i64, i64)> {
    session
        .world
        .firms
        .iter()
        .filter(|(_, f)| f.shares.fair_cents > 0 || f.shares.mark_cents > 0)
        .map(|(_, f)| {
            let fair = f.shares.fair_cents.max(1);
            (
                f.shares.mark_cents as f64 / fair as f64,
                f.name.clone(),
                f.shares.mark_cents,
                f.shares.fair_cents,
            )
        })
        .collect()
}

fn report(label: &str, session: &Session, verbose: bool) {
    let mut r = ratios(session);
    r.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    let n = r.len();
    let q = |p: usize| r[(n - 1) * p / 100].0;
    let over2 = r.iter().filter(|x| x.0 > 2.0).count();
    let under = r.iter().filter(|x| x.0 < 0.5).count();
    println!(
        "{label}: n={n} min={:.2} p10={:.2} med={:.2} p90={:.2} max={:.2} >2x={over2} <0.5x={under}",
        r[0].0,
        q(10),
        q(50),
        q(90),
        r[n - 1].0
    );
    if verbose {
        for x in r.iter().rev().take(5) {
            println!("    hi {:>6.2} {} price={} fair={}", x.0, x.1, x.2, x.3);
        }
        for x in r.iter().take(3) {
            println!("    lo {:>6.2} {} price={} fair={}", x.0, x.1, x.2, x.3);
        }
    }
}

fn main() {
    let seeds: u64 = std::env::args()
        .nth(1)
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    let verbose = std::env::var("V").is_ok();
    for scenario in [
        "great-lakes-2000",
        "texas-1981",
        "silicon-valley-1995",
        "ruhr-rhine-1991",
    ] {
        for seed in 1..=seeds {
            let mut s = Session::create(ScenarioConfig {
                seed,
                scenario: ScenarioKind::Guided,
                mode: ScenarioMode::Sandbox,
                dataset_hash: String::new(),
                difficulty: Difficulty::Standard,
                economy_start: Default::default(),
                starting_capital_cents: None,
                prices: PriceSetup {
                    scenario_id: scenario.to_owned(),
                    path: Some(PricePath::HistoricalVariance),
                    ..Default::default()
                },
            })
            .unwrap();
            println!("== {scenario} seed {seed}");
            report("week 0 ", &s, verbose);
            for w in 1..=52u32 {
                let r = s.advance(1);
                if r.pause.is_some() {
                    for e in s.world.events.pending.iter_mut() {
                        e.resolved = true;
                    }
                    s.world.pending_pause = None;
                }
                if w == 5 {
                    report("week 5 ", &s, verbose);
                }
                if w == 52 {
                    report("week 52", &s, verbose);
                }
            }
        }
    }
}
