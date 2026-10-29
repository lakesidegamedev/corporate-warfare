//! Diagnostic for the 1.9 balance pass: fix the established firms alive at week 104 (year 2)
//! and read the same firms again at weeks 156, 208 and 260. Classes: carriers (a plant whose
//! output is freight at week 104), raiders (the firm attacked a rival in any raid after week
//! 104) and the rest. Per class and snapshot: firms still operative, losing share, and the
//! mean weekly P&L per firm over the last thirteen weeks (numeraire cents / 100).
//!
//! cargo run --release -p cw-session --example balance_cohort -- scenario seeds
use std::collections::BTreeMap;

#[derive(Default, Clone, Copy)]
struct Acc {
    n: i64,
    alive: i64,
    losing: i64,
    rev: i64,
    cogs: i64,
    wages: i64,
    opex: i64,
    other: i64,
    net: i64,
    cash: i64,
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenario = a.get(1).cloned().unwrap_or_default();
    let seeds: u64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);
    let snaps = [104u32, 156, 208, 260];
    // (class, snapshot index) -> Acc
    let mut acc: BTreeMap<(&str, usize), Acc> = BTreeMap::new();
    // class -> (raids, budget spent) after week 104, per year index
    let mut raid_spend: BTreeMap<(&str, usize), (i64, i64, i64)> = BTreeMap::new();
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
        let mut cohort: BTreeMap<u64, bool> = BTreeMap::new(); // id -> is carrier
        // per snapshot index: id -> row
        let mut rows: Vec<BTreeMap<u64, [i64; 8]>> = vec![BTreeMap::new(); 4];
        for w in 1..=260u32 {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let Some(si) = snaps.iter().position(|s| *s == w) else {
                continue;
            };
            let world = &session.world;
            let dataset = &session.dataset;
            if si == 0 {
                for (h, firm) in world.firms.iter().filter(|(_, f)| f.operative()) {
                    if cw_session::telemetry::firm_kind(world, firm) == 0 {
                        let carrier = firm.facilities.iter().any(|p| {
                            world.facilities.get(*p).is_some_and(|p| {
                                p.status == cw_sim::state::FacilityStatus::Operating
                                    && dataset.recipes[p.recipe as usize].output == "freight"
                            })
                        });
                        cohort.insert(h.0, carrier);
                    }
                }
            }
            for (h, firm) in world.firms.iter() {
                if !cohort.contains_key(&h.0) {
                    continue;
                }
                let weeks = firm.book.recent_weeks();
                let sum = |f: fn(&cw_core::ledger::WeekClose) -> i64| {
                    weeks.iter().map(f).sum::<i64>() / 13
                };
                rows[si].insert(
                    h.0,
                    [
                        firm.operative() as i64,
                        sum(|x| x.revenue),
                        sum(|x| x.cost_of_goods),
                        sum(|x| x.wages),
                        sum(|x| x.operating_expense),
                        sum(|x| {
                            x.other + x.tax - x.depreciation - x.interest_expense
                                + x.interest_income
                        }),
                        sum(|x| x.net_income),
                        firm.book.cash(),
                    ],
                );
            }
        }
        let raiders: BTreeMap<u64, ()> = session
            .world
            .raids
            .iter()
            .filter(|r| r.tick > 104)
            .map(|r| (r.attacker.0, ()))
            .collect();
        for r in session.world.raids.iter().filter(|r| r.tick > 104) {
            if let Some(carrier) = cohort.get(&r.attacker.0) {
                let class = if *carrier { "carrier" } else { "raider" };
                let y = ((r.tick - 105) / 52).min(3) as usize;
                let e = raid_spend.entry((class, y)).or_default();
                e.0 += 1;
                e.1 += r.success as i64;
                e.2 += r.budget_cents;
            }
        }
        for (id, carrier) in &cohort {
            let class = if *carrier {
                "carrier"
            } else if raiders.contains_key(id) {
                "raider"
            } else {
                "other"
            };
            for si in 0..4 {
                let e = acc.entry((class, si)).or_default();
                e.n += 1;
                if let Some(r) = rows[si].get(id) {
                    if r[0] == 1 {
                        e.alive += 1;
                        e.losing += (r[6] < 0) as i64;
                        e.rev += r[1];
                        e.cogs += r[2];
                        e.wages += r[3];
                        e.opex += r[4];
                        e.other += r[5];
                        e.net += r[6];
                        e.cash += r[7];
                    }
                }
            }
        }
    }
    println!(
        "fixed cohort of established firms at week 104, {seeds} seeds; per firm per week, cents/100"
    );
    for class in ["carrier", "raider", "other"] {
        for (si, w) in snaps.iter().enumerate() {
            let e = acc[&(class, si)];
            let n = e.alive.max(1);
            println!(
                "{class:8} w{w:3} cohort {:3} alive {:3} losing {:3} ({:2}%)  rev {:7} cogs {:7} wages {:7} opex {:7} other {:7} net {:7} cash {:9}",
                e.n,
                e.alive,
                e.losing,
                e.losing * 100 / n,
                e.rev / n / 100,
                e.cogs / n / 100,
                e.wages / n / 100,
                e.opex / n / 100,
                e.other / n / 100,
                e.net / n / 100,
                e.cash / n / 100
            );
        }
    }
    println!("raids after week 104 by cohort class and year after: raids, successful, budget $k");
    for ((c, y), (n, s, b)) in &raid_spend {
        println!(
            "  {c:8} year {} raids {n} successful {s} budget ${}k",
            y + 3,
            b / 100_000
        );
    }
}
