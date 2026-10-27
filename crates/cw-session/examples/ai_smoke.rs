//! Rival AI smoke: per scenario and seed, the investor's net worth, rival dissolutions and, when
//! the engine records them, rival moves per temperament and grudges formed. Event cards are
//! answered by clearing them, so the run measures the economy, not the player's attention.
//! Seeds run on parallel threads.
//!
//! cargo run --release -p cw-session --example ai_smoke -- [scenario-id] [seeds] [weeks] [difficulty]
use std::collections::BTreeMap;

struct Row {
    net_worth: i64,
    dissolved: i64,
    not_operating: i64,
    rivals: i64,
    extra: BTreeMap<String, i64>,
}

fn run(scenario: &str, seed: u64, weeks: u32, difficulty: cw_data::Difficulty) -> Row {
    let config = cw_session::ScenarioConfig {
        seed,
        scenario: cw_sim::state::ScenarioKind::Guided,
        mode: cw_sim::state::ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: cw_session::PriceSetup {
            scenario_id: scenario.to_owned(),
            path: None,
        },
    };
    let mut session = cw_session::Session::create(config).unwrap();
    for _ in 1..=weeks {
        let r = session.advance(1);
        if r.pause.is_some() {
            for e in session.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            session.dismiss_pause();
        }
    }
    let world = &session.world;
    let mut row = Row {
        net_worth: cw_sim::economy::seed::investor_net_worth(world, &session.dataset),
        dissolved: 0,
        not_operating: 0,
        rivals: 0,
        extra: BTreeMap::new(),
    };
    for (_, firm) in world.firms.iter() {
        if firm.archetype.is_some() {
            row.rivals += 1;
            row.dissolved += matches!(firm.status, cw_sim::state::FirmStatus::Dissolved) as i64;
            row.not_operating += !firm.operative() as i64;
        }
    }
    let stats = &world.rivalry.stats;
    for (k, v) in &stats.moves_by_temperament {
        row.extra.insert(format!("moves[{k}]"), *v as i64);
    }
    for (h, f) in world.firms.iter() {
        if f.archetype.is_some() {
            *row.extra
                .entry(format!(
                    "rivals[{}]",
                    cw_ai::style(world, &session.dataset, h).key()
                ))
                .or_default() += 1;
        }
    }
    for (k, v) in &stats.moves_by_pair {
        row.extra.insert(format!("pair[{k}]"), *v as i64);
    }
    for (k, v) in &stats.moves_by_class {
        row.extra.insert(format!("class[{k}]"), *v as i64);
    }
    row.extra
        .insert("grudges_formed".into(), stats.grudges_formed as i64);
    row.extra
        .insert("alliances_formed".into(), stats.alliances_formed as i64);
    row.extra
        .insert("grudge_moves".into(), stats.grudge_moves as i64);
    row.extra
        .insert("ceo_turnovers".into(), world.cast.turnovers as i64);
    for (_, firm) in world.firms.iter() {
        if firm.archetype.is_none() {
            continue;
        }
        row.extra.insert(
            "rival_outside_lots_end".into(),
            row.extra
                .get("rival_outside_lots_end")
                .copied()
                .unwrap_or(0)
                + firm.storage.last.outside_lots,
        );
        row.extra.insert(
            "rival_fee_13w_cents".into(),
            row.extra.get("rival_fee_13w_cents").copied().unwrap_or(0)
                + firm.storage.recent(13).fee_cents,
        );
        row.extra.insert(
            "rival_spoiled_13w_cents".into(),
            row.extra
                .get("rival_spoiled_13w_cents")
                .copied()
                .unwrap_or(0)
                + firm.storage.recent(13).spoiled_cents,
        );
        let stores = firm
            .facilities
            .iter()
            .filter_map(|h| world.facilities.get(*h))
            .filter(|p| {
                session.dataset.recipes[p.recipe as usize]
                    .storage_class
                    .is_some()
            })
            .count();
        row.extra.insert(
            "rival_storage_plants".into(),
            row.extra.get("rival_storage_plants").copied().unwrap_or(0) + stores as i64,
        );
    }
    for (_, a) in session.ai_log.by_archetype.iter() {
        for (k, v) in &a.rejected_by_reason {
            if k.starts_with("build_facility") {
                *row.extra.entry(format!("rejected[{k}]")).or_default() += *v as i64;
            }
        }
    }
    row
}

fn main() {
    let mut args = std::env::args().skip(1);
    let scenario = args.next().unwrap_or_else(|| "great-lakes-2000".to_owned());
    let seeds: u64 = args.next().and_then(|v| v.parse().ok()).unwrap_or(8);
    let weeks: u32 = args.next().and_then(|v| v.parse().ok()).unwrap_or(156);
    let difficulty = match args.next().as_deref() {
        Some("easy") => cw_data::Difficulty::Easy,
        Some("hard") => cw_data::Difficulty::Hard,
        _ => cw_data::Difficulty::Standard,
    };
    let mut rows: Vec<(u64, Row)> = Vec::new();
    let all: Vec<u64> = (1..=seeds).collect();
    for chunk in all.chunks(10) {
        let handles: Vec<_> = chunk
            .iter()
            .map(|seed| {
                let scenario = scenario.clone();
                let seed = *seed;
                std::thread::Builder::new()
                    .stack_size(64 << 20)
                    .spawn(move || (seed, run(&scenario, seed, weeks, difficulty)))
                    .unwrap()
            })
            .collect();
        for h in handles {
            rows.push(h.join().unwrap());
        }
    }
    let mut worth: Vec<i64> = rows.iter().map(|(_, r)| r.net_worth / 100).collect();
    worth.sort_unstable();
    let n = worth.len();
    let median = if n % 2 == 1 {
        worth[n / 2]
    } else {
        (worth[n / 2 - 1] + worth[n / 2]) / 2
    };
    let mean = worth.iter().sum::<i64>() / n.max(1) as i64;
    let dissolved: i64 = rows.iter().map(|(_, r)| r.dissolved).sum();
    let not_op: i64 = rows.iter().map(|(_, r)| r.not_operating).sum();
    let rivals: i64 = rows.iter().map(|(_, r)| r.rivals).sum();
    println!("{scenario}: {seeds} seeds x {weeks} weeks");
    println!(
        "player net worth dollars: median {median}, mean {mean}, min {}, max {}",
        worth[0],
        worth[n - 1]
    );
    println!(
        "rival dissolutions {dissolved} of {rivals} rivals ({:.1} per seed); not operating {not_op}",
        dissolved as f64 / n as f64
    );
    let mut extra: BTreeMap<String, i64> = BTreeMap::new();
    for (_, r) in &rows {
        for (k, v) in &r.extra {
            *extra.entry(k.clone()).or_default() += v;
        }
    }
    for (k, v) in extra {
        println!("{k}: {v}");
    }
}
