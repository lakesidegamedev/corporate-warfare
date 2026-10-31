//! Diagnostic for the 1.9 balance-4 pass: the state treasury's cash by year, and where it comes
//! from and goes, by memo, with the weekly budgets at the year end. Averages over seeds.
//!
//! cargo run --release -p cw-session --example treasury_flow -- scenario seeds weeks
use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenario = a.get(1).cloned().unwrap_or_default();
    let seeds: u64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(1);
    let weeks: u32 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(260);
    // year -> memo -> cents summed over seeds; year -> [cash, receipts/wk, procurement budget]
    let mut flows: BTreeMap<u32, BTreeMap<String, i64>> = BTreeMap::new();
    let mut ends: BTreeMap<u32, [i64; 4]> = BTreeMap::new();
    let mut start = 0i64;
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
                scenario_id: scenario.clone(),
                path: None,
                timeline: Some(cw_sim::economy::timeline::TimelineMode::Historical),
            },
        };
        let mut session = cw_session::Session::create(config).unwrap();
        start += session.world.government.book.cash();
        let mut seen = 0u64;
        for w in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let world = &session.world;
            let year = (w - 1) / 52 + 1;
            for p in world.government.book.journal() {
                if p.txn > seen && p.account == cw_core::ledger::Account::Cash {
                    let kind = if p.delta >= 0 { "+" } else { "-" };
                    *flows
                        .entry(year)
                        .or_default()
                        .entry(format!("{kind} {}", p.memo))
                        .or_default() += p.delta;
                }
            }
            if let Some(p) = world.government.book.journal().last() {
                seen = seen.max(p.txn);
            }
            if w % 52 == 0 {
                let e = ends.entry(year).or_default();
                e[0] += world.government.book.cash();
                e[1] += world.government.last_tax_receipts_cents;
                e[2] += world.government.procurement_budget_cents;
                e[3] += world.government.bonds_outstanding_cents;
            }
        }
    }
    let n = seeds as i64;
    println!(
        "{scenario}: treasury opens ${}M (mean over {seeds} seeds)",
        start / n / 100_000_000
    );
    for (year, e) in &ends {
        println!(
            "year {year}: cash ${}M  receipts/wk ${}k  procurement budget/wk ${}k  bonds ${}M",
            e[0] / n / 100_000_000,
            e[1] / n / 100_000,
            e[2] / n / 100_000,
            e[3] / n / 100_000_000
        );
        let mut parts: Vec<_> = flows[year].iter().collect();
        parts.sort_by_key(|(_, v)| -v.abs());
        for (k, v) in parts.iter().take(14) {
            println!("    {k}: ${}k a year", **v / n / 100_000);
        }
    }
}
