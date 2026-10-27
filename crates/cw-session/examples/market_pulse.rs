//! Diagnostic: is the corridor economy active? Every four weeks of a guided career, per good:
//! production, units sold by firms (all buyers), household spend, imports, exports, firm stock
//! and the average asking price against reference; then the player's company P&L.
//!
//! cargo run --release -p cw-session --example market_pulse -- [seed] [weeks]
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let seed: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(1);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(104);
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
    let goods = [
        "food_raw",
        "fuel",
        "coal",
        "ore",
        "steel",
        "chemicals",
        "plastics",
        "components",
        "goods",
        "vehicles",
        "food_retail",
        "goods_retail",
        "electricity",
        "construction",
    ];
    for week in 1..=weeks {
        let r = session.advance(1);
        if r.pause.is_some() {
            for e in session.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            session.dismiss_pause();
        }
        if week % 4 != 0 && week != 1 {
            continue;
        }
        let world = &session.world;
        let dataset = &session.dataset;
        println!("== week {week}");
        println!(
            "{:14} {:>8} {:>8} {:>10} {:>7} {:>7} {:>8} {:>7} {:>6}",
            "good", "made", "sold", "hh_spend$", "import", "export", "stock", "ask$", "vsRef"
        );
        for id in goods {
            let Some(code) = dataset.product_index.get(id).copied() else {
                continue;
            };
            let made: i64 = world
                .facilities
                .iter()
                .map(|(_, f)| f.last_made(dataset, id).lots())
                .sum();
            let sold: i64 = world
                .firms
                .iter()
                .filter_map(|(_, f)| f.last_closed_sales())
                .flat_map(|w| w.iter())
                .filter(|((_, p), _)| *p == code)
                .map(|(_, q)| *q)
                .sum();
            let spend: i64 = world
                .regions
                .iter()
                .map(|r| r.households.spend_cents.get(&code).copied().unwrap_or(0))
                .sum();
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
            let reference = dataset.reference_price(code).max(1);
            let mut asks: Vec<i64> = world
                .firms
                .iter()
                .filter(|(_, f)| f.operative())
                .filter_map(|(_, f)| f.policies.prices.get(&code).copied())
                .collect();
            asks.sort();
            let ask = asks.get(asks.len() / 2).copied().unwrap_or(reference);
            println!(
                "{:14} {:>8} {:>8} {:>10} {:>7} {:>7} {:>8} {:>7} {:>5}%",
                id,
                made,
                sold,
                spend / 100,
                import,
                export,
                stock,
                ask / 100,
                (ask - reference) * 100 / reference
            );
        }
        if let Some(firm) = world
            .investor
            .primary_group
            .and_then(|h| world.firms.get(h))
        {
            let stock: Vec<String> = firm
                .inventory
                .iter()
                .filter(|(_, lot)| lot.qty.0 > 0)
                .map(|((p, r), lot)| {
                    format!(
                        "{}@{}:{}",
                        dataset.products[*p as usize].id,
                        dataset.regions[*r as usize].code,
                        lot.qty.lots()
                    )
                })
                .collect();
            let prices: Vec<String> = firm
                .policies
                .prices
                .iter()
                .map(|(p, c)| format!("{}={}", dataset.products[*p as usize].id, c / 100))
                .collect();
            let sold: Vec<String> = firm
                .last_closed_sales()
                .map(|w| {
                    w.iter()
                        .map(|((r, p), q)| {
                            format!(
                                "{}@{}:{q}",
                                dataset.products[*p as usize].id, dataset.regions[*r as usize].code
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();
            println!("player stock {stock:?} prices {prices:?} sold {sold:?}");
            if let Some(w) = firm.book.recent_weeks().last() {
                let mut by: BTreeMap<&str, i64> = BTreeMap::new();
                by.insert("revenue", w.revenue);
                by.insert("cogs", w.cost_of_goods);
                by.insert("wages", w.wages);
                by.insert("opex", w.operating_expense);
                by.insert("dep", w.depreciation);
                by.insert("interest", w.interest_expense);
                by.insert("tax", w.tax);
                by.insert("net", w.net_income);
                println!(
                    "player cash ${} | {}",
                    firm.book.cash() / 100,
                    by.iter()
                        .map(|(k, v)| format!("{k} ${}", v / 100))
                        .collect::<Vec<_>>()
                        .join("  ")
                );
            }
        }
    }
}
