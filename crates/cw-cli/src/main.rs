//! Headless orchestration: diagnostics, scenario runs, replay and sweeps.
//!
//! This binary is a developer and evidence tool. It never touches a network, a live game
//! database or the host's AHD simulation services.

use std::env;

use cw_data::Dataset;
use cw_session::{ScenarioConfig, Session};

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("doctor") => doctor(),
        Some("run") => run(&arguments[1..]),
        Some("sweep") => sweep(&arguments[1..]),
        Some("replay") => replay(&arguments[1..]),
        _ => {
            eprintln!(
                "Usage: cw-cli <doctor|run|sweep|replay>\n\
                 run    --weeks N [--seed N] [--guided|--founder] [--five-year|--sandbox]\n\
                 sweep  --seeds N --weeks N [--guided|--founder]\n\
                 replay --file PATH"
            );
            std::process::exit(2);
        }
    }
}

fn doctor() {
    let info = cw_core::ENGINE_VERSION;
    let interface = cw_core::INTERFACE_VERSION;
    let rules = cw_core::RULES_VERSION;
    match Dataset::load() {
        Ok(dataset) => {
            println!("engine={info} interface={interface} rules={rules}");
            println!("dataset_hash={}", dataset.hash);
            println!(
                "products={} recipes={} regions={} events={} firms={}",
                dataset.products.len(),
                dataset.recipes.len(),
                dataset.regions.len(),
                dataset.events.len(),
                dataset.seed_firms.len()
            );
            println!("note={}", dataset.geometry_note);
            println!("economy=implemented; release evidence still required");
        }
        Err(errors) => {
            eprintln!("dataset invalid: {}", errors.join("; "));
            std::process::exit(1);
        }
    }
}

fn config(seed: u64, guided: bool, five_year: bool) -> ScenarioConfig {
    ScenarioConfig {
        seed,
        scenario: if guided {
            cw_sim::state::ScenarioKind::Guided
        } else {
            cw_sim::state::ScenarioKind::Founder
        },
        mode: if five_year {
            cw_sim::state::ScenarioMode::FiveYear
        } else {
            cw_sim::state::ScenarioMode::Sandbox
        },
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
    }
}

fn flag(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .iter()
        .position(|argument| argument == name)
        .and_then(|index| arguments.get(index + 1))
        .cloned()
}

fn run(arguments: &[String]) {
    let weeks: u32 = flag(arguments, "--weeks")
        .and_then(|value| value.parse().ok())
        .unwrap_or(52);
    let seed: u64 = flag(arguments, "--seed")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let guided = !arguments.iter().any(|argument| argument == "--founder");
    let five_year = !arguments.iter().any(|argument| argument == "--sandbox");
    let mut session = match Session::create(config(seed, guided, five_year)) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("session failed: {:?}", error);
            std::process::exit(1);
        }
    };
    let mut advanced = 0;
    while advanced < weeks {
        let result = session.advance((weeks - advanced).min(16));
        advanced += result.weeks_advanced;
        if result.weeks_advanced == 0 {
            println!(
                "paused at week {}: {:?}",
                session.world.tick.0, result.pause
            );
            break;
        }
        if result.pause.is_some() {
            println!(
                "paused at week {}: {:?}",
                session.world.tick.0, result.pause
            );
            break;
        }
    }
    let overview = session.overview();
    println!(
        "seed={seed} week={} net_worth={} investor_cash={} firm_cash={} hash={}",
        overview.tick,
        overview.investor_net_worth_cents,
        overview.investor_cash_cents,
        overview.firm_cash_cents,
        session.canonical_hash()
    );
    let firms = session.query(cw_session::Query::Firms);
    if let cw_session::Detail::Firms(rows) = firms {
        let controlled = rows.iter().filter(|firm| firm.controlled).count();
        let past_due = rows.iter().filter(|firm| firm.status == "past_due").count();
        let dissolved = rows
            .iter()
            .filter(|firm| firm.status == "dissolved")
            .count();
        println!(
            "firms={} controlled={controlled} past_due={past_due} dissolved={dissolved}",
            rows.len()
        );
    }
    if let Some(score) = &session.world.score {
        println!(
            "score total={} return={} resilience={} control={} conduct={}",
            score.total,
            score.return_component,
            score.resilience_component,
            score.control_component,
            score.conduct_component
        );
    }
    if let Ok(bytes) = session.export_save() {
        println!("save_bytes={}", bytes.len());
    }
}

fn sweep(arguments: &[String]) {
    let seeds: u64 = flag(arguments, "--seeds")
        .and_then(|value| value.parse().ok())
        .unwrap_or(10);
    let weeks: u32 = flag(arguments, "--weeks")
        .and_then(|value| value.parse().ok())
        .unwrap_or(260);
    let guided = !arguments.iter().any(|argument| argument == "--founder");
    let mut totals: Vec<i64> = Vec::new();
    for seed in 1..=seeds {
        let mut session = match Session::create(config(seed, guided, true)) {
            Ok(session) => session,
            Err(error) => {
                eprintln!("seed {seed} failed: {:?}", error);
                continue;
            }
        };
        let mut advanced = 0;
        while advanced < weeks {
            let result = session.advance((weeks - advanced).min(16));
            advanced += result.weeks_advanced;
            if result.pause.is_some() || result.weeks_advanced == 0 {
                break;
            }
        }
        let score = session
            .world
            .score
            .as_ref()
            .map(|score| score.total as i64)
            .unwrap_or(-1);
        totals.push(score);
        println!(
            "seed={seed} week={} score={score} net_worth={}",
            session.world.tick.0,
            session.overview().investor_net_worth_cents
        );
    }
    let scored: Vec<i64> = totals.iter().copied().filter(|value| *value >= 0).collect();
    if !scored.is_empty() {
        let mean = scored.iter().sum::<i64>() / scored.len() as i64;
        let min = scored.iter().min().copied().unwrap_or(0);
        let max = scored.iter().max().copied().unwrap_or(0);
        println!("scored={} mean={mean} min={min} max={max}", scored.len());
    }
}

fn replay(arguments: &[String]) {
    let Some(path) = flag(arguments, "--file") else {
        eprintln!("replay needs --file PATH");
        std::process::exit(2);
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("cannot read {path}: {error}");
            std::process::exit(1);
        }
    };
    let log: cw_session::ReplayLog = match serde_json::from_str(&text) {
        Ok(log) => log,
        Err(error) => {
            eprintln!("invalid replay log: {error}");
            std::process::exit(1);
        }
    };
    match Session::replay(&log) {
        Ok(hash) => println!("replay_hash={hash}"),
        Err(error) => {
            eprintln!("replay failed: {:?}", error);
            std::process::exit(1);
        }
    }
}
