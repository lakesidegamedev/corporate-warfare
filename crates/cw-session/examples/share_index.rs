//! Share price index path over a headless career, for the earnings season check (1.9): per
//! scenario, over several seeds, the equal-weighted index of every share on the board (chained
//! from weekly returns, 100 at the start) at the end of each year, the worst single week of the
//! index and of any share, and how many share-weeks moved more than ten percent. Reads only the
//! price history, so the same file runs on a build without earnings for the "before" numbers.
//!
//! cargo run --release -p cw-session --example share_index -- [seeds] [weeks] [scenario-id]
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(6);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(260);
    let scenario_id = args.next().unwrap_or_default();
    // year -> index sum over seeds (in basis points of the start), count
    let mut years: BTreeMap<u32, (i128, i64)> = BTreeMap::new();
    let (mut worst_index, mut worst_share, mut big_weeks, mut share_weeks) = (0i64, 0i64, 0i64, 0i64);
    let (mut low, mut high) = (i64::MAX, i64::MIN);
    let (mut losing, mut counted) = (0i64, 0i64);
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
        let mut index: i128 = 10_000;
        let mut previous: BTreeMap<u64, i64> = BTreeMap::new();
        for week in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let world = &session.world;
            let mut now: BTreeMap<u64, i64> = BTreeMap::new();
            for (handle, firm) in world.firms.iter().filter(|(_, f)| f.on_board()) {
                now.insert(handle.0, firm.shares.mark_cents.max(1));
            }
            let (mut sum, mut n) = (0i64, 0i64);
            for (h, price) in &now {
                if let Some(prev) = previous.get(h).filter(|p| **p > 0) {
                    let change = (price - prev) * 10_000 / prev;
                    sum += change;
                    n += 1;
                    share_weeks += 1;
                    if change.abs() > 1_000 {
                        big_weeks += 1;
                    }
                    worst_share = worst_share.max(change.abs());
                }
            }
            if n > 0 {
                let mean = sum / n;
                index = index * (10_000 + mean as i128) / 10_000;
                worst_index = worst_index.max(mean.abs());
            }
            low = low.min(index as i64);
            high = high.max(index as i64);
            previous = now;
            if week % 52 == 0 {
                let e = years.entry(week / 52).or_default();
                e.0 += index;
                e.1 += 1;
            }
            if week == weeks {
                for (_, firm) in world.firms.iter().filter(|(_, f)| f.operative()) {
                    let net: i64 = firm.book.recent_weeks().iter().map(|w| w.net_income).sum();
                    losing += (net < 0) as i64;
                    counted += 1;
                }
            }
        }
    }
    println!(
        "scenario {} seeds {seeds} weeks {weeks}",
        if scenario_id.is_empty() { "default" } else { &scenario_id }
    );
    let path: Vec<String> = years
        .iter()
        .map(|(y, (s, c))| format!("y{y} {}", *s / (*c as i128) / 100))
        .collect();
    println!("index (100 = start): {}", path.join("  "));
    println!(
        "index range {}..{}  worst index week {} bps  worst share week {} bps  share-weeks over 10%: {} of {}",
        low / 100,
        high / 100,
        worst_index,
        worst_share,
        big_weeks,
        share_weeks
    );
    println!("losing share at the end: {} of {} firms", losing, counted);
}
