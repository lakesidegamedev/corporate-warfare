//! Diagnostic for market dynamics (1.8 preview 3): runs a guided career headless for several
//! seeds and reports, per good, the averages over the last year of what the corridor made and
//! sold a week, the stock it held split into sellers' unsold stock and buyers' input cover,
//! weeks of cover (unsold stock over weekly sales), the median asking price and the
//! sales-weighted asking price (`sold_ask_cents`) against reference,
//! imports against domestic output, and the share of operating firms that lost money over their
//! last thirteen weeks.
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
            if week % 13 == 0 {
                // Board-wide price level and losing share by year, to show drift or collapse.
                let year = (week - 1) / 52 + 1;
                let entry = by_year.entry(year).or_default();
                for (handle, firm) in world.firms.iter().filter(|(_, f)| f.operative()) {
                    let net: i64 = firm.book.recent_weeks().iter().map(|w| w.net_income).sum();
                    entry.0 += (net < 0) as i64;
                    entry.1 += 1;
                    let _ = handle;
                }
                let snapshot = session.market_snapshot();
                for good in snapshot.goods.iter().filter(|g| g.sold_lots > 0) {
                    entry.2 += good.median_ask_cents * 10_000 / good.reference_cents.max(1);
                    entry.3 += 1;
                }
            }
            if week + 52 <= weeks {
                continue;
            }
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
    println!("by year: losing share, mean median ask over reference across traded goods");
    for (year, (lose, firms, ask, goods)) in &by_year {
        println!(
            "  year {year}: losing {}%  ask {:+}%",
            lose * 100 / firms.max(&1),
            (ask / goods.max(&1) - 10_000) / 100
        );
    }
}
