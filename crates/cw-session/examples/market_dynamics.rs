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
//! it runs a plant in).
//!
//! cargo run --release -p cw-session --example market_dynamics -- [seeds] [weeks]
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
    const GROUPS: [&str; 6] = [
        "pig_iron",
        "freight",
        "components",
        "plastics",
        "timber",
        "fuel",
    ];
    let mut groups: BTreeMap<(usize, u32), [i64; 4]> = BTreeMap::new();
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
        let mut alive: Vec<u64> = Vec::new();
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
}
