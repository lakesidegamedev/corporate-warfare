//! Deterministic multi-seed smoke: player cash and rival dissolutions at the horizon, plus a
//! per-good table of imports, exports, stock and asking price against reference.
//!
//! cargo run --release -p cw-session --example chain_smoke -- [seeds] [weeks]
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(8);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(104);
    let mut cash_total = 0i64;
    let mut dissolved_total = 0u32;
    // product id -> (import, export, stock, ask over ref in percent), summed over seeds.
    let mut goods: BTreeMap<String, (i64, i64, i64, i64, i64)> = BTreeMap::new();
    for seed in 1..=seeds {
        let config = cw_session::ScenarioConfig {
            seed,
            scenario: cw_sim::state::ScenarioKind::Guided,
            mode: cw_sim::state::ScenarioMode::Sandbox,
            dataset_hash: String::new(),
            difficulty: cw_data::Difficulty::Standard,
            economy_start: Default::default(),
            starting_capital_cents: None,
            prices: Default::default(),
        };
        let mut session = cw_session::Session::create(config).unwrap();
        for _ in 0..weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
        }
        let world = &session.world;
        let dataset = &session.dataset;
        let cash = world
            .investor
            .primary_group
            .and_then(|h| world.firms.get(h))
            .map(|f| f.book.cash())
            .unwrap_or(0);
        let dissolved = world
            .firms
            .iter()
            .filter(|(_, f)| f.archetype.is_some())
            .filter(|(_, f)| matches!(f.status, cw_sim::state::FirmStatus::Dissolved))
            .count() as u32;
        let names: Vec<&str> = world
            .firms
            .iter()
            .filter(|(_, f)| f.archetype.is_some())
            .filter(|(_, f)| matches!(f.status, cw_sim::state::FirmStatus::Dissolved))
            .map(|(_, f)| f.name.as_str())
            .collect();
        if !names.is_empty() {
            println!("  dissolved: {names:?}");
        }
        println!(
            "seed {seed}: player cash ${} rival dissolutions {dissolved}",
            cash / 100
        );
        cash_total += cash;
        dissolved_total += dissolved;
        for (code, product) in dataset.products.iter().enumerate() {
            let code = code as u16;
            if product.reference_price_cents <= 0 {
                continue;
            }
            let (import, export) = world
                .gateways
                .iter()
                .filter(|g| g.product == code)
                .fold((0, 0), |(i, e), g| {
                    (i + g.last_import_lots, e + g.last_export_lots)
                });
            let stock: i64 = world
                .firms
                .iter()
                .flat_map(|(_, f)| f.inventory.iter())
                .filter(|((p, _), _)| *p == code)
                .map(|(_, lot)| lot.qty.lots())
                .sum();
            let mut asks: Vec<i64> = world
                .firms
                .iter()
                .filter(|(_, f)| f.operative())
                .filter_map(|(_, f)| f.policies.prices.get(&code).copied())
                .collect();
            asks.sort();
            let reference = product.reference_price_cents.max(1);
            let over = asks
                .get(asks.len() / 2)
                .map(|ask| (ask - reference) * 100 / reference);
            let entry = goods.entry(product.id.clone()).or_default();
            entry.0 += import;
            entry.1 += export;
            entry.2 += stock;
            if let Some(over) = over {
                entry.3 += over;
                entry.4 += 1;
            }
        }
    }
    println!(
        "TOTAL over {seeds} seeds x {weeks} weeks: mean player cash ${} total rival dissolutions {dissolved_total}",
        cash_total / seeds as i64 / 100
    );
    println!(
        "{:20} {:>9} {:>9} {:>9} {:>8}",
        "good", "import", "export", "stock", "ask%ref"
    );
    for (id, (import, export, stock, over, n)) in goods {
        let mean_over = if n > 0 { over / n } else { 0 };
        println!(
            "{:20} {:>9} {:>9} {:>9} {:>7}%",
            id,
            import / seeds as i64,
            export / seeds as i64,
            stock / seeds as i64,
            mean_over
        );
    }
}
