//! Diagnostic for the 1.9 balance-4 pass: public tenders over a headless career with nobody
//! playing. For every tender that was awarded, the bids it drew, the winning price against the
//! delivery cost the winner books (margin), whether the investor holds shares in the winner, and
//! the totals paid, so the money flow from the treasury to the firms can be read.
//!
//! cargo run --release -p cw-session --example tender_census -- scenario seeds weeks
use cw_sim::economy::civics::TenderStatus;
use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenario = a.get(1).cloned().unwrap_or_default();
    let seeds: u64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);
    let weeks: u32 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(260);
    let (mut opened, mut awarded, mut lapsed, mut done) = (0u32, 0u32, 0u32, 0u32);
    let (mut bids, mut price, mut cost, mut held) = (0i64, 0i64, 0i64, 0u32);
    let (mut budget_sum, mut late) = (0i64, 0i64);
    let mut by_year: BTreeMap<u32, [i64; 3]> = BTreeMap::new();
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
                timeline: Some(cw_sim::economy::timeline::TimelineMode::Historical),
            },
        };
        let mut session = cw_session::Session::create(config).unwrap();
        let mut counted: std::collections::BTreeSet<u32> = Default::default();
        let mut seen_open: std::collections::BTreeSet<u32> = Default::default();
        for w in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let world = &session.world;
            for t in &world.civics.tenders {
                if seen_open.insert(t.id) {
                    opened += 1;
                }
                if matches!(t.status, TenderStatus::Building | TenderStatus::Done)
                    && counted.insert(t.id)
                {
                    awarded += 1;
                    bids += t.bids.len() as i64;
                    price += t.price_cents;
                    cost += t.cost_cents;
                    budget_sum += t.budget_cents;
                    let year = (w - 1) / 52 + 1;
                    let e = by_year.entry(year).or_default();
                    e[0] += 1;
                    e[1] += t.price_cents - t.cost_cents;
                    e[2] += t.price_cents;
                    if let Some(f) = t.winner.and_then(|h| world.firms.get(h))
                        && f.shares.qty(cw_sim::state::HolderId::Investor) > 0
                    {
                        held += 1;
                    }
                }
            }
            if w == weeks {
                for t in &world.civics.tenders {
                    match t.status {
                        TenderStatus::Done => {
                            done += 1;
                            late += t.finished.saturating_sub(t.started + t.promised_weeks) as i64;
                        }
                        TenderStatus::Lapsed => {
                            lapsed += 1;
                            if std::env::var("LAPSES").is_ok() {
                                println!(
                                    "  lapsed: seed {seed} tender {} {} state {} region {} opened wk {} bids {} budget ${}k",
                                    t.id,
                                    t.kind.key(),
                                    t.state,
                                    t.region,
                                    t.opened,
                                    t.bids.len(),
                                    t.budget_cents / 100_000
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    let n = awarded.max(1) as i64;
    println!(
        "{scenario}: {seeds} seeds x {weeks} wks: tenders seen {opened}, awarded {awarded}, done {done}, lapsed {lapsed}; bids per award {:.1}",
        bids as f64 / n as f64
    );
    println!(
        "  price/budget {:.0}%  price/cost {:.0}% (margin {:.1}%)  investor holds winner in {held} of {awarded}; mean weeks late {:.1}",
        price as f64 * 100.0 / budget_sum.max(1) as f64,
        price as f64 * 100.0 / cost.max(1) as f64,
        (price - cost) as f64 * 100.0 / cost.max(1) as f64,
        late as f64 / done.max(1) as f64
    );
    for (y, e) in by_year {
        println!(
            "  year {y}: awards {} margin pool ${}k price ${}k per seed",
            e[0],
            e[1] / 100_000 / seeds as i64,
            e[2] / 100_000 / seeds as i64
        );
    }
}
