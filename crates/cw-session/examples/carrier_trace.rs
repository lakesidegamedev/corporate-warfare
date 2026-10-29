//! Diagnostic for the 1.9 balance pass: carrier overbuild. At each year end, per freight mode:
//! carrier plants (opened at the start / built later), capacity in lot-km against lot-km moved
//! (fill), leg lots that overflowed to outside carriers, and the net margin of firms whose plants
//! are all carriers. Counted over all seeds.
//!
//! cargo run --release -p cw-session --example carrier_trace -- scenario seeds weeks
use std::collections::BTreeMap;

#[derive(Default)]
struct Cell {
    opening: i64,
    later: i64,
    cap_km: i64,
    used_km: i64,
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let scenario = a.get(1).cloned().unwrap_or_default();
    let seeds: u64 = a.get(2).and_then(|v| v.parse().ok()).unwrap_or(3);
    let weeks: u32 = a.get(3).and_then(|v| v.parse().ok()).unwrap_or(260);
    // (year, mode) -> cell; year -> [outside lots, all lots]; year -> [carrier firm net, revenue, n, losing]
    let mut cells: BTreeMap<(u32, u8), Cell> = BTreeMap::new();
    let mut lots: BTreeMap<u32, [i64; 2]> = BTreeMap::new();
    let mut firms: BTreeMap<u32, [i64; 4]> = BTreeMap::new();
    let mut modes: Vec<String> = Vec::new();
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
        modes = session
            .dataset
            .logistics
            .modes
            .iter()
            .map(|m| m.id.clone())
            .collect();
        for w in 1..=weeks {
            let r = session.advance(1);
            if r.pause.is_some() {
                for e in session.world.events.pending.iter_mut() {
                    e.resolved = true;
                }
                session.dismiss_pause();
            }
            if w % 52 != 0 {
                continue;
            }
            let year = w / 52;
            let world = &session.world;
            for c in &world.logistics.carriers {
                let cell = cells.entry((year, c.mode)).or_default();
                let plant = world.facilities.get(c.facility);
                if plant.is_some_and(|p| p.started.0 <= 1) {
                    cell.opening += 1;
                } else {
                    cell.later += 1;
                }
                cell.cap_km += c.cap as i64 * cw_sim::economy::logistics::REF_KM;
                cell.used_km += c.used_lot_km;
            }
            let l = lots.entry(year).or_default();
            l[0] += world
                .logistics
                .totals
                .outside_lots_by_mode
                .iter()
                .sum::<i64>();
            l[1] += world.logistics.totals.lots_by_mode.iter().sum::<i64>();
            let carrier_firms: std::collections::BTreeSet<u64> =
                world.logistics.carriers.iter().map(|c| c.firm.0).collect();
            for (h, f) in world.firms.iter().filter(|(_, f)| f.operative()) {
                if !carrier_firms.contains(&h.0) {
                    continue;
                }
                let weeks13 = f.book.recent_weeks();
                let net: i64 = weeks13.iter().map(|x| x.net_income).sum();
                let rev: i64 = weeks13.iter().map(|x| x.revenue).sum();
                let row = firms.entry(year).or_default();
                row[0] += net;
                row[1] += rev;
                row[2] += 1;
                row[3] += (net < 0) as i64;
            }
        }
    }
    println!(
        "{scenario}: {seeds} seeds, carrier plants per seed (opening+later), fill %, then outside share and carrier-firm margin"
    );
    for year in 1..=weeks / 52 {
        let mut parts = Vec::new();
        for (m, id) in modes.iter().enumerate() {
            if let Some(c) = cells.get(&(year, m as u8)) {
                parts.push(format!(
                    "{id} {}+{} fill {}%",
                    c.opening / seeds as i64,
                    c.later / seeds as i64,
                    c.used_km * 100 / c.cap_km.max(1)
                ));
            }
        }
        let l = lots.get(&year).copied().unwrap_or_default();
        let f = firms.get(&year).copied().unwrap_or_default();
        println!(
            "  y{year}: {} | outside {}% | carrier firms {} per seed, losing {}%, margin {:+.1}%",
            parts.join(", "),
            l[0] * 100 / l[1].max(1),
            f[2] / seeds as i64,
            f[3] * 100 / f[2].max(1),
            f[0] as f64 * 100.0 / f[1].max(1) as f64
        );
    }
}
