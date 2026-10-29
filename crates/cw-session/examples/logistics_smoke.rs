//! Logistics smoke: fuel and food import shares, plus lane use, freight rates and outside
//! carrier share when the engine reports them.
//!
//! cargo run --release -p cw-session --example logistics_smoke -- [seeds] [weeks]
fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(4);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(104);
    let mut shares: std::collections::BTreeMap<&str, (i64, i64)> = Default::default();
    let goods = ["fuel", "food_raw"];
    // Firms dissolved by the end, by sector, summed across seeds.
    let mut dissolved: std::collections::BTreeMap<&str, i64> = Default::default();
    let mut dissolved_total = 0i64;
    // The two seeded freight carriers: how many seeds each is still operating at the end, and
    // its mean closing cash among the survivors.
    let mut carrier_fate: std::collections::BTreeMap<String, (i64, i64, i64)> = Default::default();
    // Lane use over the last 52 weeks, summed across seeds.
    let mut lots_by_mode = [0i64; 4];
    let mut outside_by_mode = [0i64; 4];
    let (mut own, mut contract, mut cents, mut outside_cents, mut shipments, mut lot_km) =
        (0i64, 0i64, 0i64, 0i64, 0i64, 0i64);
    let (mut cap_sum, mut weeks_seen, mut hot_lanes, mut carriers_seen) = (0i64, 0i64, 0i64, 0i64);
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
                ..Default::default()
            },
        };
        let mut session = cw_session::Session::create(config).unwrap();
        for week in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            // Average the last year of weekly snapshots.
            if week > weeks.saturating_sub(52) {
                let last = &session.world.logistics.last;
                for m in 0..4 {
                    lots_by_mode[m] += last.lots_by_mode[m];
                    outside_by_mode[m] += last.outside_lots_by_mode[m];
                }
                own += last.own_lots;
                contract += last.contract_lots;
                cents += last.cents;
                outside_cents += last.outside_cents;
                shipments += last.shipments;
                lot_km += last.lot_km;
                cap_sum += session
                    .world
                    .logistics
                    .lanes
                    .values()
                    .map(|l| l.cap() as i64)
                    .sum::<i64>();
                hot_lanes += session
                    .world
                    .logistics
                    .stats
                    .values()
                    .filter(|s| s.hot_weeks >= 4)
                    .count() as i64;
                carriers_seen += session.world.logistics.carriers.len() as i64;
                weeks_seen += 1;
                let snap = session.market_snapshot();
                for g in snap.goods.iter().filter(|g| goods.contains(&g.id.as_str())) {
                    let key = goods.iter().find(|k| **k == g.id).unwrap();
                    let row = shares.entry(key).or_default();
                    row.0 += g.imported_lots;
                    row.1 += g.imported_lots + g.made_lots;
                }
            }
        }
        for (_, firm) in session.world.firms.iter() {
            if firm.name == "Crossroads Carriers" || firm.name == "Oaken Freight Lines" {
                let row = carrier_fate.entry(firm.name.clone()).or_default();
                row.0 += 1;
                if firm.operative() {
                    row.1 += 1;
                    row.2 += firm.book.cash();
                }
            }
            if matches!(firm.status, cw_sim::state::FirmStatus::Dissolved)
                && firm.archetype.is_some()
            {
                dissolved_total += 1;
                *dissolved
                    .entry(cw_sim::state::INDUSTRY_NAMES[firm.industry as usize])
                    .or_default() += 1;
            }
        }
    }
    for (good, (imported, supply)) in shares {
        println!(
            "{good} import share over last 52 weeks: {:.1}% ({imported} of {supply} lots)",
            imported as f64 * 100.0 / supply.max(1) as f64
        );
    }
    for (name, (seeds_seen, alive, cash)) in &carrier_fate {
        println!(
            "{name}: operating in {alive} of {seeds_seen} seeds at week {weeks}, mean cash of survivors {} dollars",
            cash / 100 / (*alive).max(1)
        );
    }
    println!(
        "rival firms dissolved by week {weeks} across {seeds} seeds: {dissolved_total} {dissolved:?}"
    );
    let moved: i64 = lots_by_mode.iter().sum();
    let outside: i64 = outside_by_mode.iter().sum();
    println!(
        "lots moved by mode truck/rail/pipeline/sea: {lots_by_mode:?}; outside overflow {outside_by_mode:?}"
    );
    println!(
        "outside carrier share {:.1}% of leg lots; own-group {:.1}%; under contract {:.1}%",
        outside as f64 * 100.0 / moved.max(1) as f64,
        own as f64 * 100.0 / moved.max(1) as f64,
        contract as f64 * 100.0 / moved.max(1) as f64
    );
    println!(
        "freight paid {:.0} dollars a week per seed, outside carriers took {:.1}%, {:.2} cents per lot-km over {} shipments",
        cents as f64 / 100.0 / weeks_seen.max(1) as f64,
        outside_cents as f64 * 100.0 / cents.max(1) as f64,
        cents as f64 / lot_km.max(1) as f64,
        shipments
    );
    println!(
        "average weekly lane capacity {:.0} lots per seed, {:.1} carriers, {:.1} lanes congested four weeks or more",
        cap_sum as f64 / weeks_seen.max(1) as f64,
        carriers_seen as f64 / weeks_seen.max(1) as f64,
        hot_lanes as f64 / weeks_seen.max(1) as f64
    );
}
