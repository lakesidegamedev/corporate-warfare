//! Diagnostic for the 1.9 balance pass: every week a firm leaves the operative set (operating or
//! past due), record what it left for, its age, its plants and its sector; and whether it later
//! came back. Counted over all seeds.
//!
//! cargo run --release -p cw-session --example exit_census -- scenario seeds weeks
use std::collections::{BTreeMap, BTreeSet};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenario = a.get(1).cloned().unwrap_or_default();
    let seeds: u64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);
    let weeks: u32 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(260);
    let mut by: BTreeMap<(String, u32), i64> = BTreeMap::new();
    let mut returned = 0i64;
    let mut exits = 0i64;
    let mut final_states: BTreeMap<String, i64> = BTreeMap::new();
    let mut founded_by_year: BTreeMap<u32, i64> = BTreeMap::new();
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
        let mut alive: BTreeSet<u64> = BTreeSet::new();
        let mut seen: BTreeSet<u64> = BTreeSet::new();
        let mut left: BTreeSet<u64> = BTreeSet::new();
        for w in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            let world = &session.world;
            let now: BTreeSet<u64> = world
                .firms
                .iter()
                .filter(|(_, f)| f.operative())
                .map(|(h, _)| h.0)
                .collect();
            for id in &now {
                if left.remove(id) {
                    returned += 1;
                }
                if seen.insert(*id) && w > 1 {
                    *founded_by_year.entry((w - 1) / 52 + 1).or_default() += 1;
                }
            }
            for id in alive.difference(&now) {
                let (_, f) = world.firms.iter().find(|(h, _)| h.0 == *id).unwrap();
                let status = match f.status {
                    cw_sim::state::FirmStatus::Operating => "operating",
                    cw_sim::state::FirmStatus::PastDue { .. } => "pastdue",
                    cw_sim::state::FirmStatus::Restructuring { .. } => "restructuring",
                    cw_sim::state::FirmStatus::Liquidating => "liquidating",
                    cw_sim::state::FirmStatus::Merged { .. } => "merged",
                    cw_sim::state::FirmStatus::Dissolved => "dissolved",
                };
                let age = w.saturating_sub(f.founding_week.0);
                let plants = f
                    .facilities
                    .iter()
                    .filter_map(|h| world.facilities.get(*h))
                    .count();
                let class = if age <= 52 { "young" } else { "established" };
                let key = format!(
                    "{status:13} {class:11} plants={} {}",
                    plants.min(2),
                    cw_sim::state::INDUSTRY_NAMES[f.industry as usize]
                );
                *by.entry((key, (w - 1) / 52 + 1)).or_default() += 1;
                exits += 1;
                left.insert(*id);
            }
            alive = now;
        }
        for (_, f) in session.world.firms.iter() {
            let s = format!("{:?}", f.status);
            let s = s.split([' ', '{', '(']).next().unwrap().to_owned();
            *final_states.entry(s).or_default() += 1;
        }
    }
    println!(
        "exits {exits}, returned later {returned}; final states over all seeds {final_states:?}"
    );
    println!("operative firms first seen after week 1, by year: {founded_by_year:?}");
    let mut totals: BTreeMap<String, [i64; 5]> = BTreeMap::new();
    for ((k, y), n) in &by {
        totals.entry(k.clone()).or_default()[(*y as usize - 1).min(4)] += n;
    }
    let mut rows: Vec<_> = totals.into_iter().collect();
    rows.sort_by_key(|(_, v)| -v.iter().sum::<i64>());
    for (k, v) in rows.iter().take(40) {
        println!("{k:60} {v:?}");
    }
}
