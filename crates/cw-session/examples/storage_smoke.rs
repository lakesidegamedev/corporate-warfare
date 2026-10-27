//! Storage and farm-season smoke: per quarter, stock of the seasonal goods, farm capacity,
//! run rate and sales, and the corridor's storage fees, holding costs, spoilage and held goods.
//!
//! cargo run --release -p cw-session --example storage_smoke -- [seeds] [weeks]
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(4);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(104);
    // quarter index -> (stock food_raw, stock grain, farm capacity, target sum, sold, fees, holding, spoiled lots, held)
    let mut rows: BTreeMap<u32, [i64; 9]> = BTreeMap::new();
    let mut orders_total = 0i64;
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
                scenario_id: "great-lakes-2000".to_owned(),
                path: Some(cw_data::PricePath::HistoricalVariance),
            },
        };
        let mut session = cw_session::Session::create(config).unwrap();
        let food = session.dataset.product_index["food_raw"];
        let grain = session.dataset.product_index["grain"];
        for week in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            if week % 13 != 0 {
                continue;
            }
            let world = &session.world;
            let dataset = &session.dataset;
            let stock = |p: u16| -> i64 {
                world
                    .firms
                    .iter()
                    .flat_map(|(_, f)| f.inventory.iter())
                    .filter(|((q, _), _)| *q == p)
                    .map(|((_, _), l)| l.qty.lots())
                    .sum()
            };
            let mut cap = 0i64;
            let mut tgt = 0i64;
            for (_, f) in world.facilities.iter() {
                let r = &dataset.recipes[f.recipe as usize];
                if r.industry == "agriculture"
                    && f.status == cw_sim::state::FacilityStatus::Operating
                {
                    let c = r.weekly_output_lots * f.blocks as i64;
                    cap += c;
                    tgt += c * f.target_bps as i64 / 10_000;
                }
            }
            let sold: i64 = world
                .firms
                .iter()
                .filter_map(|(_, f)| f.last_closed_sales())
                .flat_map(|w| w.iter())
                .filter(|((_, p), _)| *p == food || *p == grain)
                .map(|(_, q)| *q)
                .sum();
            let (mut fee, mut hold, mut spoil, mut held) = (0, 0, 0, 0);
            for (_, f) in world.firms.iter() {
                let q = f.storage.recent(13);
                fee += q.fee_cents;
                hold += q.holding_cents;
                spoil += q.spoiled_milli / 1000;
                held += f.storage.held.values().map(|m| m.lots()).sum::<i64>();
                orders_total += f.storage.orders.len() as i64;
            }
            if std::env::var("STORAGE_DEBUG").is_ok()
                && week
                    == std::env::var("STORAGE_DEBUG")
                        .unwrap()
                        .parse::<u32>()
                        .unwrap()
                && seed == 1
            {
                let mut payers: Vec<(i64, String)> = Vec::new();
                for (_, f) in world.firms.iter() {
                    let q = f.storage.recent(13);
                    if q.fee_cents + q.holding_cents > 0 {
                        let cells: Vec<String> = cw_sim::economy::storage::uses(world, dataset, f)
                            .iter()
                            .filter(|u| u.outside_milli() > 0 || u.used_milli > u.yard_milli)
                            .map(|u| {
                                format!(
                                    "r{} {} used {} yard {} owned {}",
                                    u.region,
                                    u.class.key(),
                                    u.used_milli / 1000,
                                    u.yard_milli / 1000,
                                    u.owned_milli / 1000
                                )
                            })
                            .collect();
                        payers.push((
                            q.fee_cents + q.holding_cents,
                            format!(
                                "{} fee {} hold {} {:?}",
                                f.name,
                                q.fee_cents / 100,
                                q.holding_cents / 100,
                                cells
                            ),
                        ));
                    }
                }
                payers.sort_by_key(|p| std::cmp::Reverse(p.0));
                for (_, line) in payers.iter().take(6) {
                    println!("  {line}");
                }
            }
            let row = rows.entry(week / 13).or_default();
            let add = [
                stock(food),
                stock(grain),
                cap,
                tgt,
                sold,
                fee / 100,
                hold / 100,
                spoil,
                held,
            ];
            for (a, b) in row.iter_mut().zip(add) {
                *a += b;
            }
        }
    }
    println!("per seed mean at each quarter end ({seeds} seeds)");
    println!(
        "qtr food_stock grain_stock farm_cap farm_run sold_food+grain fee$ hold$ spoiled_lots held"
    );
    for (q, r) in rows {
        let n = seeds as i64;
        println!(
            "{q:3} {:10} {:11} {:8} {:8} {:15} {:5} {:5} {:12} {:4}",
            r[0] / n,
            r[1] / n,
            r[2] / n,
            r[3] / n,
            r[4] / n,
            r[5] / n,
            r[6] / n,
            r[7] / n,
            r[8] / n
        );
    }
    println!("open buy orders seen at quarter ends: {orders_total}");
}
