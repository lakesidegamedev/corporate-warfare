//! Diagnostic for the 1.9 era-macro pass: walks the farm link at chosen weeks. Per farm good,
//! the gateway bid and ask against the reference price (corridor average, local money), the
//! price index and the background wage index, what the corridor made, sold and exported that
//! week, how many plants make it, and the family index the world price path holds.
//!
//! cargo run --release -p cw-session --example farm_trace -- scenario [seed] [timeline]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenario = a.get(1).cloned().unwrap_or_default();
    let seed: u64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(1);
    let timeline = a
        .get(3)
        .and_then(|key| cw_sim::economy::timeline::TimelineMode::from_key(key));
    let config = cw_session::ScenarioConfig {
        seed,
        scenario: cw_sim::state::ScenarioKind::Guided,
        mode: cw_sim::state::ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: cw_session::PriceSetup {
            scenario_id: scenario,
            path: None,
            timeline,
        },
    };
    let mut session = cw_session::Session::create(config).unwrap();
    let goods = ["grain", "food_raw", "fertilizers", "food_retail"];
    let wage0 = session.world.regions[0].background_wage_cents[0];
    for week in 1..=260u32 {
        let r = session.advance(1);
        if r.pause.is_some() {
            for e in session.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            session.dismiss_pause();
        }
        if ![8, 52, 104, 156, 208, 260].contains(&week) {
            continue;
        }
        let world = &session.world;
        let dataset = &session.dataset;
        println!(
            "week {week}: prices {:.1} wages {:.1} inflation {:.2} rate {:.2}",
            world.macro_state.price_index_bps as f64 / 100.0,
            world.regions[0].background_wage_cents[0] as f64 * 100.0 / wage0 as f64,
            world.macro_state.inflation_bps as f64 / 100.0,
            world.macro_state.policy_rate_bps as f64 / 100.0
        );
        let snapshot = session.market_snapshot();
        for id in goods {
            let Some(code) = dataset.product_code(id) else {
                continue;
            };
            let reference = dataset.reference_price(code);
            let (mut bid, mut ask, mut n) = (0i64, 0i64, 0i64);
            for g in world.gateways.iter().filter(|g| g.product == code) {
                bid += g.buy_price_cents * 10_000 / reference.max(1);
                ask += g.sell_price_cents * 10_000 / reference.max(1);
                n += 1;
            }
            let plants = world
                .facilities
                .iter()
                .filter(|(_, p)| {
                    p.status == cw_sim::state::FacilityStatus::Operating
                        && dataset.recipes[p.recipe as usize].output == id
                })
                .count();
            let row = snapshot.goods.iter().find(|g| g.id == id);
            let (made, sold, exported, imported, sold_ask) = row
                .map(|g| {
                    (
                        g.made_lots,
                        g.sold_lots,
                        g.exported_lots,
                        g.imported_lots,
                        g.sold_ask_cents * 10_000 / g.reference_cents.max(1),
                    )
                })
                .unwrap_or_default();
            println!(
                "  {id:12} bid {:>5.1}% ask {:>5.1}% of ref ({n} gateways)  sold ask {:>5.1}%  plants {plants:>4}  made {made:>6} sold {sold:>6} exported {exported:>6} imported {imported:>5}",
                bid as f64 / n.max(1) as f64 / 100.0,
                ask as f64 / n.max(1) as f64 / 100.0,
                sold_ask as f64 / 100.0
            );
        }
        for f in &world.world_prices.families {
            if f.key == "grain" || f.key == "fertilizer" {
                println!("  family {} index {}", f.key, f.index_bps);
            }
        }
    }
}
