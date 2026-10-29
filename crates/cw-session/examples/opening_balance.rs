//! Diagnostic for the 1.9 balance pass: at the end of week N (default 8), per good, what the
//! corridor made, sold, imported and exported that week, in lots, and the export share of what
//! was made. Totals are valued at reference price in the numeraire.
//!
//! cargo run --release -p cw-session --example opening_balance -- scenario [week] [seed]
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenario = a.get(1).cloned().unwrap_or_default();
    let week: u32 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(8);
    let seed: u64 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(1);
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
            timeline: Some(cw_sim::economy::timeline::TimelineMode::Historical),
        },
    };
    let mut session = cw_session::Session::create(config).unwrap();
    {
        let w = &session.world;
        let cap: i64 = w.gateways.iter().map(|g| cw_sim::economy::fx::region_to_numeraire(w, &session.dataset, g.region, g.capacity_lots * g.buy_price_cents)).sum();
        println!("gateway export room at the bid, numeraire cents: {} ($k a week {})", cap, cap / 100_000);
    }
    for _ in 1..=week {
        let r = session.advance(1);
        if r.pause.is_some() {
            for e in session.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            session.dismiss_pause();
        }
    }
    let snap = session.market_snapshot();
    println!("{:22} {:>7} {:>7} {:>7} {:>7} {:>9} {:>8}", "good", "made", "sold", "import", "export", "made$k", "export%");
    let (mut made_v, mut exp_v, mut imp_v, mut sold_v) = (0i64, 0i64, 0i64, 0i64);
    for g in &snap.goods {
        let r = g.reference_cents;
        made_v += g.made_lots * r;
        exp_v += g.exported_lots * r;
        imp_v += g.imported_lots * r;
        sold_v += g.sold_lots * r;
        if g.made_lots + g.imported_lots + g.sold_lots > 0 {
            println!(
                "{:22} {:>7} {:>7} {:>7} {:>7} {:>9} {:>7}%",
                g.id,
                g.made_lots,
                g.sold_lots,
                g.imported_lots,
                g.exported_lots,
                g.made_lots * r / 100_000,
                g.exported_lots * 100 / g.made_lots.max(1)
            );
        }
    }
    println!(
        "TOTAL at reference, $k a week: made {} sold {} imported {} exported {} (export share of made {}%)",
        made_v / 100_000,
        sold_v / 100_000,
        imp_v / 100_000,
        exp_v / 100_000,
        exp_v * 100 / made_v.max(1)
    );
    {
        let l = &session.world.logistics;
        let mut by: std::collections::BTreeMap<u8, (i64, i64, i64)> = Default::default();
        for c in &l.carriers {
            let e = by.entry(c.mode).or_default();
            e.0 += 1;
            e.1 += c.cap as i64;
            e.2 += c.used_lot_km;
        }
        for (m, (n, cap, used)) in by {
            println!(
                "carriers mode {m}: {n} carriers, capacity {cap} lots over the reference haul, fill {}% by lot-km",
                used * 100 / (cap * cw_sim::economy::logistics::REF_KM).max(1)
            );
        }
        println!(
            "lots by mode {:?} outside {:?}",
            l.totals.lots_by_mode, l.totals.outside_lots_by_mode
        );
    }
    println!("outside gateway cash ${}k", session.world.outside.gateway_book.cash() / 100_000);
}
