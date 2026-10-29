//! Diagnostic for the 1.9 balance pass: take the established firms that lost money over their
//! last thirteen weeks at a chosen week, and show where the P&L breaks. Firms are grouped by the
//! output of their biggest operating plant (or "none"), with revenue, cost of goods, wages,
//! operating expense, depreciation, interest and net income per firm per week, in numeraire cents.
//!
//! cargo run --release -p cw-session --example balance_trace -- scenario seeds week [follow-week]
//!
//! With a follow week, the cohort is also read again then: still operating, still losing.
use std::collections::BTreeMap;

#[derive(Default, Clone)]
struct Acc {
    n: i64,
    still: i64,
    still_losing: i64,
    rev: i64,
    cogs: i64,
    wages: i64,
    opex: i64,
    dep: i64,
    int: i64,
    other: i64,
    net: i64,
    cash: i64,
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenario = a.get(1).cloned().unwrap_or_default();
    let seeds: u64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);
    let week: u32 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(104);
    let follow: u32 = a.get(4).and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut groups: BTreeMap<String, Acc> = BTreeMap::new();
    let mut winners: BTreeMap<String, Acc> = BTreeMap::new();
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
        let mut cohort: Vec<(u64, String)> = Vec::new();
        for w in 1..=week.max(follow) {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let world = &session.world;
            let dataset = &session.dataset;
            if w % 52 == 0 || w == 1 {
                let v: Vec<i64> = world.fx.currencies.iter().map(|c| c.value_ppm).collect();
                let wage: Vec<i64> = (0..dataset.regions.len())
                    .map(|r| world.regions[r].background_wage_cents[0])
                    .collect();
                println!("seed {seed} w{w} fx value_ppm {v:?} outside gateway cash ${}k tier0 wage by region {wage:?}", world.outside.gateway_book.cash() / 100_000);
            }
            if w == week {
                for (h, firm) in world.firms.iter().filter(|(_, f)| f.operative()) {
                    if cw_session::telemetry::firm_kind(world, firm) != 0 {
                        continue;
                    }
                    let weeks = firm.book.recent_weeks();
                    let sum = |f: fn(&cw_core::ledger::WeekClose) -> i64| {
                        weeks.iter().map(f).sum::<i64>() / 13
                    };
                    let net = sum(|x| x.net_income);
                    let best = firm
                        .facilities
                        .iter()
                        .filter_map(|f| world.facilities.get(*f))
                        .filter(|p| p.status == cw_sim::state::FacilityStatus::Operating)
                        .max_by_key(|p| p.blocks);
                    let key = match best {
                        Some(p) => dataset.recipes[p.recipe as usize].output.to_string(),
                        None => "none".to_owned(),
                    };
                    let key = format!(
                        "{key}@{}",
                        dataset.regions[firm.region as usize].country
                    );
                    let e = if net < 0 {
                        cohort.push((h.0, key.clone()));
                        groups.entry(key).or_default()
                    } else {
                        winners.entry(key).or_default()
                    };
                    e.n += 1;
                    e.rev += sum(|x| x.revenue);
                    e.cogs += sum(|x| x.cost_of_goods);
                    e.wages += sum(|x| x.wages);
                    e.opex += sum(|x| x.operating_expense);
                    e.dep += sum(|x| x.depreciation);
                    e.int += sum(|x| x.interest_expense - x.interest_income);
                    e.other += sum(|x| x.other + x.tax);
                    e.net += net;
                    e.cash += firm.book.cash() / 100;
                }
            }
            if follow > 0 && w == follow {
                for (id, key) in &cohort {
                    let e = groups.entry(key.clone()).or_default();
                    if let Some((_, f)) = world.firms.iter().find(|(h, _)| h.0 == *id) {
                        if f.operative() {
                            e.still += 1;
                            let net: i64 = f.book.recent_weeks().iter().map(|x| x.net_income).sum();
                            e.still_losing += (net < 0) as i64;
                        }
                    }
                }
            }
        }
    }
    let print = |title: &str, m: &BTreeMap<String, Acc>| {
        println!("{title}: per firm per week, cents/100 (rev cogs wages opex dep int other+tax net), cash/100");
        let mut rows: Vec<_> = m.iter().collect();
        rows.sort_by_key(|(_, e)| -e.n);
        for (k, e) in rows {
            let n = e.n.max(1);
            println!(
                "  {k:30} n={:4} rev {:7} cogs {:7} wages {:7} opex {:7} dep {:6} int {:6} oth {:6} net {:7} cash {:9}  follow still {} losing {}",
                e.n,
                e.rev / n / 100,
                e.cogs / n / 100,
                e.wages / n / 100,
                e.opex / n / 100,
                e.dep / n / 100,
                e.int / n / 100,
                e.other / n / 100,
                e.net / n / 100,
                e.cash / n,
                e.still,
                e.still_losing
            );
        }
    };
    print("LOSING established firms", &groups);
    print("PROFITABLE established firms", &winners);
}
