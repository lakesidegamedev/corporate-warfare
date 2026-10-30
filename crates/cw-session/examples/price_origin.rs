//! Diagnostic for opening share prices (1.9 balance-4): for the firms on the board at the start,
//! the total market value (mark times issued shares), total fair value, total book net tangible
//! assets and total trailing EBITDA at chosen weeks, so the opening valuation can be walked
//! against the path the prices then take.
//!
//! cargo run --release -p cw-session --example price_origin -- [seeds] [weeks] [scenario-id]
fn main() {
    let mut args = std::env::args().skip(1);
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(3);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(104);
    let scenario_id = args.next().unwrap_or_default();
    let marks = [1u32, 2, 4, 8, 13, 26, 39, 52, 78, 104, 156, 208, 260];
    let mut rows: std::collections::BTreeMap<u32, [i128; 7]> = Default::default();
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
                scenario_id: scenario_id.clone(),
                path: None,
                timeline: None,
            },
        };
        let mut session = cw_session::Session::create(config).unwrap();
        let mut seed_set: Vec<u64> = Vec::new();
        for week in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let world = &session.world;
            if week == 1 {
                seed_set = world
                    .firms
                    .iter()
                    .filter(|(_, f)| f.on_board())
                    .map(|(h, _)| h.0)
                    .collect();
            }
            if marks.contains(&week) {
                let row = rows.entry(week).or_default();
                for (h, f) in world.firms.iter() {
                    if !seed_set.contains(&h.0) || !f.on_board() {
                        continue;
                    }
                    let issued = f.shares.eligible_votes().max(1) as i128;
                    row[0] += f.shares.mark_cents as i128 * issued;
                    row[1] += f.shares.fair_cents as i128 * issued;
                    row[2] += f.valuation.net_tangible_cents as i128;
                    row[3] += f.valuation.ebitda_trailing_cents as i128 * 4;
                    row[4] += f.valuation.enterprise_cents as i128;
                    row[5] += f.book.cash() as i128;
                    row[6] += 1;
                }
            }
        }
    }
    println!("scenario {scenario_id} seeds {seeds}; seed-board firms; sums over seeds, $k");
    println!(
        "week   firms   market   fair   book(NTA)  EBITDA*4  EV   cash   mark/fair  mark/book"
    );
    for (w, r) in rows {
        let d = |v: i128| v / 100 / 1000;
        println!(
            "{w:>4} {:>6} {:>8} {:>8} {:>9} {:>9} {:>8} {:>8}  {:>6.2}  {:>6.2}",
            r[6],
            d(r[0]),
            d(r[1]),
            d(r[2]),
            d(r[3]),
            d(r[4]),
            d(r[5]),
            r[0] as f64 / r[1].max(1) as f64,
            r[0] as f64 / r[2].max(1) as f64
        );
    }
}
