//! Scenario smoke: runs a scenario for several seeds and reports the investor's net worth,
//! dissolutions, crude and fuel flows, refinery utilisation and freight by mode.
//!
//! cargo run --release -p cw-session --example scenario_smoke -- [scenario-id] [seeds] [weeks]
use std::collections::BTreeMap;

fn main() {
    let mut args = std::env::args().skip(1);
    let scenario = args.next().unwrap_or_else(|| "texas-1981".to_owned());
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(8);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(104);
    let goods = [
        "crude_oil",
        "fuel",
        "gasoline",
        "natural_gas",
        "food_raw",
        "livestock",
    ];
    let mut flows: BTreeMap<&str, (i64, i64, i64)> = BTreeMap::new();
    let mut dissolved: BTreeMap<String, i64> = BTreeMap::new();
    let (mut start_total, mut end_total) = (0i64, 0i64);
    let (mut util_made, mut util_cap) = (0i64, 0i64);
    let mut lots_by_mode = [0i64; 4];
    let mut outside_by_mode = [0i64; 4];
    let mut pipelines = 0i64;
    let (mut firms_end, mut firms_start) = (0i64, 0i64);
    let mut not_operating: BTreeMap<String, i64> = BTreeMap::new();
    let (mut investor_cash_start, mut investor_cash_end) = (0i64, 0i64);
    let (mut group_cash_end, mut group_alive) = (0i64, 0i64);
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
            },
        };
        let mut session = cw_session::Session::create(config).unwrap();
        let start = cw_sim::economy::seed::investor_net_worth(&session.world, &session.dataset);
        start_total += start;
        investor_cash_start += session.world.investor.cash_cents;
        firms_start += session
            .world
            .firms
            .iter()
            .filter(|(_, f)| f.archetype.is_some())
            .count() as i64;
        let refinery = session.dataset.recipe_index["refinery"];
        for week in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            if week > weeks.saturating_sub(52) {
                let last = &session.world.logistics.last;
                for m in 0..4 {
                    lots_by_mode[m] += last.lots_by_mode[m];
                    outside_by_mode[m] += last.outside_lots_by_mode[m];
                }
                let snap = session.market_snapshot();
                for g in snap.goods.iter().filter(|g| goods.contains(&g.id.as_str())) {
                    let key = goods.iter().find(|k| **k == g.id).unwrap();
                    let row = flows.entry(key).or_default();
                    row.0 += g.made_lots;
                    row.1 += g.imported_lots;
                    row.2 += g.exported_lots;
                }
                for (_, f) in session.world.facilities.iter() {
                    if f.recipe == refinery && f.status == cw_sim::state::FacilityStatus::Operating
                    {
                        util_made += f.last_output.0 / 1000;
                        util_cap += session.dataset.recipes[refinery as usize].weekly_output_lots
                            * f.blocks as i64;
                    }
                }
            }
        }
        end_total += cw_sim::economy::seed::investor_net_worth(&session.world, &session.dataset);
        investor_cash_end += session.world.investor.cash_cents;
        if let Some(group) = session
            .world
            .investor
            .primary_group
            .and_then(|h| session.world.firms.get(h))
            .filter(|group| group.operative())
        {
            group_alive += 1;
            group_cash_end += group.book.cash();
        }
        pipelines += session
            .world
            .logistics
            .carriers
            .iter()
            .filter(|c| Some(c.mode) == session.dataset.logistics.mode_index("pipeline"))
            .count() as i64;
        for (_, firm) in session.world.firms.iter() {
            if firm.archetype.is_some() {
                firms_end += firm.operative() as i64;
                if matches!(firm.status, cw_sim::state::FirmStatus::Dissolved) {
                    *dissolved.entry(firm.name.clone()).or_default() += 1;
                }
                if !firm.operative() {
                    let kind = format!("{:?}", firm.status);
                    let kind = kind.split([' ', '{', '(']).next().unwrap_or("").to_owned();
                    *not_operating
                        .entry(format!("{} ({kind})", firm.name))
                        .or_default() += 1;
                }
            }
        }
    }
    println!("scenario {scenario}, {seeds} seeds x {weeks} weeks");
    println!(
        "investor net worth: start {} dollars mean, end {} dollars mean",
        start_total / 100 / seeds as i64,
        end_total / 100 / seeds as i64
    );
    println!(
        "player cash: investor {} dollars at start, {} at the end (mean); company alive in {group_alive} of {seeds} seeds, mean cash of survivors {} dollars",
        investor_cash_start / 100 / seeds as i64,
        investor_cash_end / 100 / seeds as i64,
        group_cash_end / 100 / group_alive.max(1)
    );
    println!(
        "named firms operating at the end: {firms_end} of {firms_start}; dissolved: {:?}",
        dissolved
    );
    println!("named firms not operating, with seeds: {not_operating:?}");
    for (good, (made, imported, exported)) in &flows {
        println!(
            "{good}: last 52 weeks made {made}, imported {imported} ({:.1}% of supply), exported {exported}",
            *imported as f64 * 100.0 / (*made + *imported).max(1) as f64
        );
    }
    println!(
        "refinery utilisation (last 52 weeks): {:.1}% of nameplate",
        util_made as f64 * 100.0 / util_cap.max(1) as f64
    );
    println!(
        "lots moved truck/rail/pipeline/sea: {lots_by_mode:?}; outside overflow {outside_by_mode:?}"
    );
    println!("pipeline carriers at the end, all seeds: {pipelines}");
}
