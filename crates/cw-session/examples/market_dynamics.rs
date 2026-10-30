//! Diagnostic for market dynamics (1.8 preview 3): runs a guided career headless for several
//! seeds and reports, per good, the averages over the last year of what the corridor made and
//! sold a week, the stock it held split into sellers' unsold stock and buyers' input cover,
//! weeks of cover (unsold stock over weekly sales), the median asking price and the
//! sales-weighted asking price (`sold_ask_cents`) against reference,
//! imports against domestic output, and the share of operating firms that lost money over their
//! last thirteen weeks, split into established firms (an operating plant, founded over a year
//! ago), entrants (an operating plant, younger) and shells (no operating plant). Freight is also
//! read by lot-km: carrier fill is lot-km moved over lot-km of capacity, capacity weighted, and
//! the outside share is the leg lots that overflowed to outside carriers. Established firms that
//! run a plant making pig iron, freight, components, plastics, timber or fuel are also reported by
//! year: losing share and net margin over their last thirteen weeks (a firm counts in each group
//! it runs a plant in). The last line is the investor's net worth over a career with no moves of
//! the player's own, start and end, as a whole-economy health check.
//!
//! cargo run --release -p cw-session --example market_dynamics -- [seeds] [weeks] [scenario-id] [timeline]
//!
//! With a scenario id the career gets that scenario's world prices and, by `timeline` (historical,
//! off or shuffled; historical when not given), its dated history.
use std::collections::BTreeMap;

#[derive(Default)]
struct Row {
    made: i64,
    sold: i64,
    imported: i64,
    exported: i64,
    unsold: i64,
    cover: i64,
    ask_bps: i64,
    sold_ask_bps: i64,
    samples: i64,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(4);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(260);
    let scenario_id = args.next().unwrap_or_default();
    let timeline = args
        .next()
        .and_then(|key| cw_sim::economy::timeline::TimelineMode::from_key(&key));
    let mut rows: BTreeMap<String, Row> = BTreeMap::new();
    let (mut losing, mut reporting) = (0i64, 0i64);
    let mut by_year: BTreeMap<u32, (i64, i64, i64, i64)> = BTreeMap::new();
    // year -> [losing, counted] for established, entrants and shells.
    let mut split: BTreeMap<u32, [(i64, i64); 3]> = BTreeMap::new();
    // final year: lot-km used, lot-km capacity, outside leg lots, all leg lots.
    let mut freight = [0i64; 4];
    // year -> export value (numeraire cents), export lots, raids, successful raids, firm exits.
    let mut trade: BTreeMap<u32, [i64; 5]> = BTreeMap::new();
    // (good, year) -> established firms counted, losing, revenue and net income over 13 weeks.
    const GROUPS: [&str; 8] = [
        "pig_iron",
        "freight",
        "components",
        "plastics",
        "timber",
        "fuel",
        "grain",
        "food_raw",
    ];
    let mut groups: BTreeMap<(usize, u32), [i64; 4]> = BTreeMap::new();
    // Player-less career: the investor's net worth at the end and at the start, summed over seeds.
    let mut worth = [0i64; 2];
    // year -> policy rate, inflation, ten-year yield, price index, background wage index (tier 0,
    // first region), summed over seeds at the last week of the year.
    let mut macros: BTreeMap<u32, [i64; 5]> = BTreeMap::new();
    let mut wage0 = 0i64;
    // year -> restructurings started, of them repeats (the firm restructured before); and the
    // outside gateway account at the year end, summed over seeds.
    let mut restructurings: BTreeMap<u32, [i64; 2]> = BTreeMap::new();
    let mut outside: BTreeMap<u32, i64> = BTreeMap::new();
    // Bids for control over all seeds: started, and of those settled (the bidder took control).
    let (mut hostile_started, mut hostile_settled) = (0i64, 0i64);
    // Long plans over all seeds: opened, disclosed, tenders launched, won, abandoned, and of the
    // ended plans those another holder (a friendly firm) took out of the raider's hands.
    let mut plan_totals = [0i64; 6];
    for seed in 1..=seeds {
        let mut hostile_seen: std::collections::BTreeSet<u64> = Default::default();
        let mut hostile_done: std::collections::BTreeSet<u64> = Default::default();
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
                timeline,
            },
        };
        let mut session = cw_session::Session::create(config).unwrap();
        let mut alive: Vec<u64> = Vec::new();
        let mut in_plan: std::collections::BTreeSet<u64> = Default::default();
        let mut had_plan: std::collections::BTreeSet<u64> = Default::default();
        for week in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let world = &session.world;
            let dataset = &session.dataset;
            if week == weeks {
                let st = &world.plans.stats;
                let controlled = world
                    .plans
                    .ended
                    .iter()
                    .filter(|e| e.outcome == cw_sim::economy::plans::Outcome::Controlled)
                    .count();
                for (slot, v) in [st.opened, st.disclosed, st.tenders, st.won, st.abandoned]
                    .into_iter()
                    .enumerate()
                {
                    plan_totals[slot] += v as i64;
                }
                plan_totals[5] += controlled as i64;
            }
            // A bid is a share purchase of a company outside the bidder's own group, seen first.
            for (h, deal) in world.deals.iter().filter(|(_, d)| {
                d.kind == cw_sim::state::DealKind::SharePurchase
                    && (d.hostile
                        || !cw_sim::economy::ownership::same_group(world, d.buyer, d.target))
            }) {
                if hostile_seen.insert(h.0) {
                    hostile_started += 1;
                }
                if deal.state == cw_sim::state::DealState::Settled && hostile_done.insert(h.0) {
                    hostile_settled += 1;
                }
            }
            {
                let year = (week - 1) / 52 + 1;
                for (h, f) in world.firms.iter() {
                    let now = matches!(f.status, cw_sim::state::FirmStatus::Restructuring { .. });
                    if now && in_plan.insert(h.0) {
                        let row = restructurings.entry(year).or_default();
                        row[0] += 1;
                        row[1] += !had_plan.insert(h.0) as i64;
                    } else if !now {
                        in_plan.remove(&h.0);
                    }
                }
                if week % 52 == 0 {
                    *outside.entry(week / 52).or_default() += world.outside.gateway_book.cash();
                }
            }
            if week == 1 {
                wage0 = world.regions[0].background_wage_cents[0];
            }
            if week % 52 == 0 {
                let row = macros.entry(week / 52).or_default();
                row[0] += world.macro_state.policy_rate_bps as i64;
                row[1] += world.macro_state.inflation_bps as i64;
                row[2] += cw_sim::economy::macro_cycle::ten_year_yield_bps(world) as i64;
                row[3] += world.macro_state.price_index_bps;
                row[4] += world.regions[0].background_wage_cents[0] * 10_000 / wage0.max(1);
            }
            {
                let year = (week - 1) / 52 + 1;
                let row = trade.entry(year).or_default();
                for gateway in world.gateways.iter().filter(|g| g.last_export_lots > 0) {
                    row[0] += cw_sim::economy::fx::region_to_numeraire(
                        world,
                        dataset,
                        gateway.region,
                        gateway.last_export_lots * gateway.buy_price_cents,
                    );
                    row[1] += gateway.last_export_lots;
                }
                let now: Vec<u64> = world
                    .firms
                    .iter()
                    .filter(|(_, f)| f.operative())
                    .map(|(h, _)| h.0)
                    .collect();
                row[4] += alive
                    .iter()
                    .filter(|h| now.binary_search(h).is_err())
                    .count() as i64;
                alive = now;
            }
            if week % 13 == 0 {
                // Board-wide price level and losing share by year, to show drift or collapse.
                let year = (week - 1) / 52 + 1;
                let entry = by_year.entry(year).or_default();
                let kinds = split.entry(year).or_default();
                for (_, firm) in world.firms.iter().filter(|(_, f)| f.operative()) {
                    let net: i64 = firm.book.recent_weeks().iter().map(|w| w.net_income).sum();
                    entry.0 += (net < 0) as i64;
                    entry.1 += 1;
                    let kind = cw_session::telemetry::firm_kind(world, firm);
                    kinds[kind].0 += (net < 0) as i64;
                    kinds[kind].1 += 1;
                    if kind != 0 {
                        continue;
                    }
                    let revenue: i64 = firm.book.recent_weeks().iter().map(|w| w.revenue).sum();
                    for (index, good) in GROUPS.iter().enumerate() {
                        let runs = firm.facilities.iter().any(|handle| {
                            world.facilities.get(*handle).is_some_and(|plant| {
                                plant.status == cw_sim::state::FacilityStatus::Operating
                                    && dataset.recipes[plant.recipe as usize].output == *good
                            })
                        });
                        if runs {
                            let row = groups.entry((index, year)).or_default();
                            row[0] += 1;
                            row[1] += (net < 0) as i64;
                            row[2] += revenue;
                            row[3] += net;
                        }
                    }
                }
                let snapshot = session.market_snapshot();
                for good in snapshot.goods.iter().filter(|g| g.sold_lots > 0) {
                    entry.2 += good.median_ask_cents * 10_000 / good.reference_cents.max(1);
                    entry.3 += 1;
                }
            }
            if week == weeks {
                worth[0] += cw_sim::economy::seed::investor_net_worth(world, dataset);
                worth[1] += world.investor.starting_net_worth_cents;
                for raid in &world.raids {
                    let year = (raid.tick.max(1) - 1) / 52 + 1;
                    let row = trade.entry(year).or_default();
                    row[2] += 1;
                    row[3] += raid.success as i64;
                }
            }
            if week + 52 <= weeks {
                continue;
            }
            let logistics = &world.logistics;
            for carrier in &logistics.carriers {
                freight[0] += carrier.used_lot_km;
                freight[1] += carrier.cap as i64 * cw_sim::economy::logistics::REF_KM;
            }
            freight[2] += logistics.totals.outside_lots_by_mode.iter().sum::<i64>();
            freight[3] += logistics.totals.lots_by_mode.iter().sum::<i64>();
            for (_, firm) in world.firms.iter().filter(|(_, f)| f.operative()) {
                if week % 13 == 0 {
                    let net: i64 = firm.book.recent_weeks().iter().map(|w| w.net_income).sum();
                    losing += (net < 0) as i64;
                    reporting += 1;
                }
            }
            let snapshot = session.market_snapshot();
            for good in &snapshot.goods {
                let Some(code) = dataset.product_code(&good.id) else {
                    continue;
                };
                let unsold: i64 = world
                    .firms
                    .iter()
                    .filter(|(_, f)| f.operative())
                    .map(|(h, _)| cw_sim::economy::pricing::free_stock(world, dataset, h, code))
                    .sum();
                let row = rows.entry(good.id.clone()).or_default();
                row.made += good.made_lots;
                row.sold += good.sold_lots;
                row.imported += good.imported_lots;
                row.exported += good.exported_lots;
                row.sold_ask_bps += good.sold_ask_cents * 10_000 / good.reference_cents.max(1);
                row.unsold += unsold;
                row.cover += good.stock_lots - unsold;
                row.ask_bps += good.median_ask_cents * 10_000 / good.reference_cents.max(1);
                row.samples += 1;
            }
        }
    }
    println!("{seeds} seeds x {weeks} weeks, guided; averages over the final 52 weeks");
    println!(
        "{:22} {:>7} {:>7} {:>7} {:>7} {:>8} {:>8} {:>7} {:>7} {:>7}",
        "good", "made", "sold", "import", "export", "unsold", "inputs", "cover", "ask%", "sold%"
    );
    for (id, r) in &rows {
        let n = r.samples.max(1);
        let sold = r.sold / n;
        let cover = if sold > 0 {
            format!(
                "{}.{:02}",
                r.unsold / n / sold,
                r.unsold / n * 100 / sold % 100
            )
        } else {
            "-".to_owned()
        };
        println!(
            "{:22} {:>7} {:>7} {:>7} {:>7} {:>8} {:>8} {:>7} {:>+7} {:>+7}",
            id,
            r.made / n,
            sold,
            r.imported / n,
            r.exported / n,
            r.unsold / n,
            r.cover / n,
            cover,
            (r.ask_bps / n - 10_000) / 100,
            (r.sold_ask_bps / n - 10_000) / 100
        );
    }
    println!(
        "takeover bids over {seeds} runs of {weeks} weeks: {hostile_started} started, \
         {hostile_settled} settled; per run per 5 years: {:.2} started, {:.2} settled",
        hostile_started as f64 * 260.0 / (seeds as f64 * weeks.max(1) as f64),
        hostile_settled as f64 * 260.0 / (seeds as f64 * weeks.max(1) as f64),
    );
    println!(
        "long plans over {seeds} runs: opened {}, disclosed {}, tenders {}, won {}, abandoned {}, \
         ended under a friendly holder's control {}",
        plan_totals[0],
        plan_totals[1],
        plan_totals[2],
        plan_totals[3],
        plan_totals[4],
        plan_totals[5]
    );
    println!(
        "firms losing money over 13 weeks (final year): {}/{} = {}%",
        losing,
        reporting,
        losing * 100 / reporting.max(1)
    );
    println!(
        "carrier fill by lot-km (final year): {}.{}%  outside share of leg lots: {}.{}%",
        freight[0] * 100 / freight[1].max(1),
        freight[0] * 1000 / freight[1].max(1) % 10,
        freight[2] * 100 / freight[3].max(1),
        freight[2] * 1000 / freight[3].max(1) % 10
    );
    println!(
        "by year: losing share (all; established / entrants / shells as losing of counted), \
         mean median ask over reference across traded goods"
    );
    for (year, (lose, firms, ask, goods)) in &by_year {
        let kinds = split.get(year).copied().unwrap_or_default();
        let cell = |(l, n): (i64, i64)| format!("{}% of {}", l * 100 / n.max(1), n);
        println!(
            "  year {year}: losing {}%  [{} / {} / {}]  ask {:+}%",
            lose * 100 / firms.max(&1),
            cell(kinds[0]),
            cell(kinds[1]),
            cell(kinds[2]),
            (ask / goods.max(&1) - 10_000) / 100
        );
    }
    println!(
        "by year: established firms running a plant of each good, losing of counted (quarterly \
         snapshots) and net margin over their last 13 weeks"
    );
    for (index, good) in GROUPS.iter().enumerate() {
        let cells: Vec<String> = (1..=weeks.div_ceil(52))
            .filter_map(|year| groups.get(&(index, year)))
            .map(|r| {
                format!(
                    "{}/{} {:+.1}%",
                    r[1],
                    r[0],
                    r[3] as f64 * 100.0 / r[2].max(1) as f64
                )
            })
            .collect();
        println!("  {good:<12} {}", cells.join("  "));
    }
    println!(
        "by year: exports a week (value at the gateway bid in numeraire, lots), raids (successful), \
         firms that stopped operating; totals over all seeds"
    );
    for (year, row) in &trade {
        let n = (seeds as i64 * 52).max(1);
        println!(
            "  year {year}: exports ${}k / {} lots a week per seed  raids {} ({})  exits {}",
            row[0] / n / 100_000,
            row[1] / n,
            row[2],
            row[3],
            row[4]
        );
    }
    println!(
        "by year: restructurings started (repeats of a firm that restructured before), outside account at year end, mean over seeds"
    );
    for (year, row) in &restructurings {
        println!(
            "  year {year}: restructurings {} ({} repeats)  outside account ${}M",
            row[0],
            row[1],
            outside.get(year).copied().unwrap_or(0) / seeds as i64 / 100_000_000
        );
    }
    println!(
        "by year end: policy rate, inflation, ten-year yield (percent), price index and background wage index (start 100), mean over seeds"
    );
    for (year, row) in &macros {
        let n = seeds as i64;
        println!(
            "  year {year}: rate {:.2}  inflation {:.2}  ten-year {:.2}  prices {:.1}  wages {:.1}",
            row[0] as f64 / n as f64 / 100.0,
            row[1] as f64 / n as f64 / 100.0,
            row[2] as f64 / n as f64 / 100.0,
            row[3] as f64 / n as f64 / 100.0,
            row[4] as f64 / n as f64 / 100.0
        );
    }
    println!(
        "player-less career net worth, mean over seeds: start ${}k, end ${}k ({:+}%)",
        worth[1] / seeds as i64 / 100_000,
        worth[0] / seeds as i64 / 100_000,
        (worth[0] - worth[1]) * 100 / worth[1].max(1)
    );
}
