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
        Some("diagnose") => diagnose(&arguments[1..]),
        Some("report") => report(&arguments[1..]),
        _ => {
            eprintln!(
                "Usage: cw-cli <doctor|run|sweep|replay>\n\
                 run    --weeks N [--seed N] [--guided|--founder] [--five-year|--sandbox]\n\
                 sweep  --seeds N --weeks N [--guided|--founder]\n\
                 replay --file PATH
\
                 diagnose --weeks N [--seed N] [--guided|--founder]"
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
    if let Some(primary) = session.world.investor.primary_group {
        if let Some(firm) = session.world.firm(primary) {
            println!(
                "primary={} status={:?} votes={} investor_qty={} issued={} cash={} fair={}",
                firm.name,
                firm.status,
                firm.shares.vote_bps(cw_sim::state::HolderId::Investor),
                firm.shares.qty(cw_sim::state::HolderId::Investor),
                firm.shares.issued,
                firm.book.cash(),
                firm.shares.fair_cents
            );
        }
    } else {
        println!("primary=none");
    }
    let dead: Vec<String> = session
        .world
        .firms
        .iter()
        .filter(|(_, firm)| matches!(firm.status, cw_sim::state::FirmStatus::Dissolved))
        .map(|(_, firm)| firm.name.clone())
        .collect();
    if !dead.is_empty() {
        println!("dissolved: {}", dead.join(", "));
    }
    let holdings: Vec<String> = session
        .world
        .firms
        .iter()
        .filter(|(_, firm)| firm.name.contains("Holdings"))
        .map(|(_, firm)| format!("{}={:?}", firm.name, firm.status))
        .collect();
    if !holdings.is_empty() {
        println!("holdings: {}", holdings.join(", "));
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

fn diagnose(arguments: &[String]) {
    let weeks: u32 = flag(arguments, "--weeks")
        .and_then(|value| value.parse().ok())
        .unwrap_or(26);
    let seed: u64 = flag(arguments, "--seed")
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    let guided = !arguments.iter().any(|argument| argument == "--founder");
    let mut session = match Session::create(config(seed, guided, false)) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("session failed: {:?}", error);
            std::process::exit(1);
        }
    };
    let focus = flag(arguments, "--focus");
    for _ in 0..weeks {
        session.advance(1);
        if let Some(name) = &focus {
            for (handle, firm) in session.world.firms.iter() {
                if &firm.name != name {
                    continue;
                }
                let last = firm.book.recent_weeks().last().copied().unwrap_or_default();
                let outflow =
                    cw_sim::economy::insolvency::mandatory_weekly_outflow(&session.world, handle);
                let mut stock = String::new();
                for facility_handle in &firm.facilities {
                    if let Some(facility) = session.world.facilities.get(*facility_handle) {
                        let recipe = &session.dataset.recipes[facility.recipe as usize];
                        if let Some(code) = session.dataset.product_code(&recipe.output) {
                            stock.push_str(&format!(
                                "{}={} ",
                                recipe.output,
                                firm.stock(code, firm.region).lots()
                            ));
                        }
                    }
                }
                let elec = session.dataset.product_code("electricity");
                let gw = elec.and_then(|code| {
                    session
                        .world
                        .gateways
                        .iter()
                        .find(|g| g.region == firm.region && g.product == code)
                        .map(|g| (g.capacity_lots, g.buy_price_cents))
                });
                let workers: i64 = firm
                    .facilities
                    .iter()
                    .filter_map(|h| session.world.facilities.get(*h))
                    .map(|f| f.workers.iter().sum::<i64>())
                    .sum();
                println!(
                    "w{} {} nfac={} workers={} wage={:?} cash={} rev={} cogs={} wages={} opex={} int={} tax={} ebit={} outflow={} status={:?} stock={} gw={:?}",
                    session.world.tick.0,
                    firm.name,
                    firm.facilities.len(),
                    workers,
                    firm.policies.wage_offer,
                    firm.book.cash(),
                    last.revenue,
                    last.cost_of_goods,
                    last.wages,
                    last.operating_expense,
                    last.interest_expense,
                    last.tax,
                    last.ebit(),
                    outflow,
                    firm.status,
                    stock,
                    gw
                );
            }
        }
        let world = &session.world;
        let mut rows: Vec<(i64, String, i64, i64, i64, i64, String)> = Vec::new();
        for (handle, firm) in world.firms.iter() {
            let outflow = cw_sim::economy::insolvency::mandatory_weekly_outflow(world, handle);
            let revenue = firm
                .book
                .recent_weeks()
                .last()
                .map(|week| week.revenue)
                .unwrap_or(0);
            let output: i64 = firm
                .facilities
                .iter()
                .filter_map(|facility| world.facilities.get(*facility))
                .map(|facility| facility.last_output.lots())
                .sum();
            let sold: i64 = firm
                .last_week_units_sold
                .values()
                .map(|qty| qty.lots())
                .sum();
            let staff: i32 = firm
                .facilities
                .iter()
                .filter_map(|facility| world.facilities.get(*facility))
                .map(|facility| facility.last_staff_ratio_bps)
                .min()
                .unwrap_or(0);
            let last = firm.book.recent_weeks().last().copied().unwrap_or_default();
            let status = match firm.status {
                cw_sim::state::FirmStatus::Operating => "op".to_owned(),
                cw_sim::state::FirmStatus::PastDue { .. } => "due".to_owned(),
                cw_sim::state::FirmStatus::Restructuring { .. } => "rest".to_owned(),
                cw_sim::state::FirmStatus::Liquidating => "liq".to_owned(),
                cw_sim::state::FirmStatus::Merged { .. } => "merged".to_owned(),
                cw_sim::state::FirmStatus::Dissolved => "dead".to_owned(),
            };
            if firm.book.cash() < outflow * 4 || revenue == 0 {
                rows.push((
                    firm.book.cash(),
                    format!(
                        "{} [rev={} cogs={} wages={} opex={} int={} ebit={}]",
                        firm.name,
                        last.revenue / 100,
                        last.cost_of_goods / 100,
                        last.wages / 100,
                        last.operating_expense / 100,
                        last.interest_expense / 100,
                        last.ebit() / 100
                    ),
                    revenue,
                    output,
                    sold,
                    staff as i64,
                    status,
                ));
            }
        }
        rows.sort_by_key(|row| row.0);
        println!(
            "week {} stressed={} of {} gateway_cash={} contracts={}",
            world.tick.0,
            rows.len(),
            world.firms.len(),
            world.outside.gateway_book.cash(),
            world.contracts.len()
        );
        for row in rows.iter().take(6) {
            let detail = world
                .firms
                .iter()
                .find(|(_, firm)| firm.name == row.1.split(" [").next().unwrap_or(""))
                .map(|(_, firm)| {
                    let mut text = format!(
                        "wage={:?} nfac={} ",
                        firm.policies.wage_offer,
                        firm.facilities.len()
                    );
                    for facility_handle in &firm.facilities {
                        if let Some(facility) = world.facilities.get(*facility_handle) {
                            text.push_str(&format!(
                                "blocks={} firm={} workers={:?} ",
                                facility.blocks, facility.firm.0, facility.workers
                            ));
                            let recipe = &session.dataset.recipes[facility.recipe as usize];
                            if let Some(code) = session.dataset.product_code(&recipe.output) {
                                text.push_str(&format!(
                                    "{}={} ",
                                    recipe.output,
                                    firm.stock(code, firm.region).lots()
                                ));
                            }
                        }
                    }
                    text
                })
                .unwrap_or_default();
            println!(
                "  {:>12} cash={:>12} rev={:>10} out={:>6} sold={:>6} staff={:>5} {} stock:{}",
                row.1, row.0, row.2, row.3, row.4, row.5, row.6, detail
            );
        }
    }
}

fn report(arguments: &[String]) {
    let weeks: u32 = flag(arguments, "--weeks")
        .and_then(|value| value.parse().ok())
        .unwrap_or(52);
    let seed: u64 = flag(arguments, "--seed")
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    let mut session = match Session::create(config(seed, true, false)) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("session failed: {:?}", error);
            std::process::exit(1);
        }
    };
    session.advance(weeks);
    #[derive(Default)]
    struct IndustryTotals {
        firms: i64,
        revenue: i64,
        cogs: i64,
        wages: i64,
        opex: i64,
        interest: i64,
        ebit: i64,
    }
    let mut rows: std::collections::BTreeMap<String, IndustryTotals> =
        std::collections::BTreeMap::new();
    for (_, firm) in session.world.firms.iter() {
        let industry = cw_sim::state::INDUSTRY_NAMES[firm.industry as usize].to_owned();
        let last = firm.book.recent_weeks().last().copied().unwrap_or_default();
        let entry = rows.entry(industry).or_default();
        entry.firms += 1;
        entry.revenue += last.revenue;
        entry.cogs += last.cost_of_goods;
        entry.wages += last.wages;
        entry.opex += last.operating_expense;
        entry.interest += last.interest_expense;
        entry.ebit += last.ebit();
    }
    println!("industry           firms   revenue     cogs    wages     opex    ebit");
    for (industry, totals) in rows {
        println!(
            "{industry:16} {:5} {:>9} {:>8} {:>8} {:>8} {:>8}",
            totals.firms,
            totals.revenue / 100,
            totals.cogs / 100,
            totals.wages / 100,
            (totals.opex + totals.interest) / 100,
            totals.ebit / 100
        );
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
