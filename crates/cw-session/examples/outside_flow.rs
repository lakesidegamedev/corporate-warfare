//! Diagnostic for the 1.9 balance pass: where the outside gateway account's cash comes from and
//! goes, by memo, per year, plus its balance at each year end and the lots exported.
//!
//! cargo run --release -p cw-session --example outside_flow -- scenario seeds weeks
use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenario = a.get(1).cloned().unwrap_or_default();
    let seeds: u64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(1);
    let weeks: u32 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(260);
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
        let mut seen = 0u64;
        let mut flows: BTreeMap<String, i64> = BTreeMap::new();
        let mut exports = 0i64;
        let mut last_cash = session.world.outside.gateway_book.cash();
        for w in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let world = &session.world;
            exports += world
                .gateways
                .iter()
                .map(|g| g.last_export_lots)
                .sum::<i64>();
            for p in world.outside.gateway_book.journal() {
                if p.txn > seen && p.account == cw_core::ledger::Account::Cash {
                    *flows.entry(p.memo.to_string()).or_default() += p.delta;
                }
            }
            if let Some(p) = world.outside.gateway_book.journal().last() {
                seen = seen.max(p.txn);
            }
            if w % 52 == 0 {
                let cash = world.outside.gateway_book.cash();
                let mut parts: Vec<_> = flows.iter().collect();
                parts.sort_by_key(|(_, v)| -v.abs());
                let text: Vec<String> = parts
                    .iter()
                    .take(8)
                    .map(|(k, v)| format!("{k} {}", **v / 100_000))
                    .collect();
                println!(
                    "{scenario} seed {seed} year {} cash ${}M (change {}M) exports {} lots/wk; flows $k: {}",
                    w / 52,
                    cash / 100_000_000,
                    (cash - last_cash) / 100_000_000,
                    exports / 52,
                    text.join(", ")
                );
                last_cash = cash;
                flows.clear();
                exports = 0;
            }
        }
    }
}
