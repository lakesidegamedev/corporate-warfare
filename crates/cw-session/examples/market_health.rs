//! Market health report: per good made, sold, imported and exported, plus price oscillation.
//!
//! cargo run --release -p cw-session --example market_health -- [scenario-id|all] [seeds] [weeks] [good-to-print]
#[path = "../tests/common/market_health.rs"]
mod market_health;

fn main() {
    let mut args = std::env::args().skip(1);
    let which = args.next().unwrap_or_else(|| "all".to_owned());
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(8);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(104);
    let watch = args.next();
    let scenarios: Vec<&str> = if which == "all" {
        market_health::SCENARIOS.to_vec()
    } else {
        vec![which.as_str()]
    };
    for scenario in scenarios {
        let rows = market_health::run(scenario, seeds, weeks);
        println!("{}", market_health::render(scenario, seeds, weeks, &rows));
        if let Some(good) = &watch {
            for (seed, series) in rows[good].series.iter().enumerate() {
                println!("seed {}: {:?}", seed + 1, series);
            }
        }
    }
}
