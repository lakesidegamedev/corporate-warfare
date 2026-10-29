//! Headless orchestration: diagnostics, scenario runs, replay and sweeps.
//!
//! This binary is a developer and evidence tool. It never touches a network, a live game
//! database or the host's AHD simulation services.

mod policy;

use std::collections::BTreeMap;
use std::env;
use std::fs;

use cw_data::Dataset;
use cw_session::{Query, ScenarioConfig, Session};
use policy::{PolicyRunner, PolicyStats, ReferencePolicy};

fn main() {
    let arguments: Vec<String> = env::args().skip(1).collect();
    match arguments.first().map(String::as_str) {
        Some("doctor") => doctor(),
        Some("run") => run(&arguments[1..]),
        Some("sweep") => sweep(&arguments[1..]),
        Some("replay") => replay(&arguments[1..]),
        Some("diagnose") => diagnose(&arguments[1..]),
        Some("report") => report(&arguments[1..]),
        Some("bench") => bench(&arguments[1..]),
        _ => {
            eprintln!(
                "Usage: cw-cli <doctor|run|sweep|replay|diagnose|report>\n\
                 run    --weeks N [--seed N] [--scenario guided|founder] [--five-year|--sandbox] \
                 [--policy none|conservative|growth|acquirer|expander] [--verbose]\n\
                 sweep  --seeds N --weeks N [--from N] [--scenario guided|founder] \
                 [--five-year|--sandbox] [--policy none|conservative|growth|acquirer|expander] \
                 [--json PATH] [--verbose]\n\
                 replay --file PATH\n\
                 diagnose --weeks N [--seed N] [--guided|--founder]\n\
                 report --weeks N [--seed N]"
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

pub(crate) fn config(seed: u64, guided: bool, five_year: bool) -> ScenarioConfig {
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
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    }
}

fn flag(arguments: &[String], name: &str) -> Option<String> {
    arguments
        .iter()
        .position(|argument| argument == name)
        .and_then(|index| arguments.get(index + 1))
        .cloned()
}

fn scenario_flags(arguments: &[String]) -> (bool, bool) {
    let guided = match flag(arguments, "--scenario").as_deref() {
        Some("founder") => false,
        Some("guided") => true,
        Some(other) => {
            eprintln!("unknown scenario {other}; use guided or founder");
            std::process::exit(2);
        }
        None => !arguments.iter().any(|argument| argument == "--founder"),
    };
    // The score pause only exists in five-year mode; sandbox continues past week 260.
    let five_year = !arguments.iter().any(|argument| argument == "--sandbox");
    (guided, five_year)
}

/// `--prices off|default|historical|historical_variance|random`. Off is the pre-1.7 behaviour
/// (no world commodity prices); the others use the scenario named by `--scenario`, Great Lakes
/// 2000 unless given. Default takes the scenario's own mode. `--timeline historical|off|shuffled`
/// sets whether the scenario's dated history plays (historical when not given).
fn prices_flag(arguments: &[String]) -> cw_session::PriceSetup {
    let name = flag(arguments, "--prices").unwrap_or_else(|| "default".to_owned());
    let scenario_id =
        flag(arguments, "--scenario-id").unwrap_or_else(|| "great-lakes-2000".to_owned());
    let timeline = flag(arguments, "--timeline").map(|key| {
        cw_sim::economy::timeline::TimelineMode::from_key(&key).unwrap_or_else(|| {
            eprintln!("unknown timeline {key}; use historical, off or shuffled");
            std::process::exit(2);
        })
    });
    match name.as_str() {
        "off" => cw_session::PriceSetup::default(),
        "default" => cw_session::PriceSetup {
            scenario_id,
            path: None,
            timeline,
        },
        other => match cw_data::PricePath::from_key(other) {
            Some(path) => cw_session::PriceSetup {
                scenario_id,
                path: Some(path),
                timeline,
            },
            None => {
                eprintln!(
                    "unknown prices {other}; use off, default, historical, historical_variance or random"
                );
                std::process::exit(2);
            }
        },
    }
}

fn policy_flag(arguments: &[String]) -> ReferencePolicy {
    let name = flag(arguments, "--policy").unwrap_or_else(|| "none".to_owned());
    match ReferencePolicy::parse(&name) {
        Some(policy) => policy,
        None => {
            eprintln!("unknown policy {name}; use none, conservative, growth or acquirer");
            std::process::exit(2);
        }
    }
}

/// Step one week at a time so a reference policy can act on its cadence week. Batching is
/// outcome-neutral (see `batching_does_not_change_the_result`), but policy decisions need the
/// intervening revisions.
fn drive(session: &mut Session, weeks: u32, runner: &mut PolicyRunner) -> (u32, Option<String>) {
    runner.act(session);
    let mut advanced = 0;
    let mut paused = None;
    while advanced < weeks {
        // Answer every waiting mandatory card before advancing, so a backlog cannot stall.
        let waiting: Vec<u64> = session
            .world
            .events
            .pending
            .iter()
            .filter(|event| {
                !event.resolved
                    && event.mandatory
                    && event
                        .subject_firm
                        .map(|firm| {
                            cw_sim::economy::events_engine::player_side(&session.world, firm)
                        })
                        .unwrap_or(true)
            })
            .map(|event| event.id)
            .collect();
        for event in waiting {
            runner.answer_event(session, event);
        }
        let result = session.advance(1);
        advanced += result.weeks_advanced;
        if let Some(reason) = result.pause.clone() {
            // A mandatory card is answered by the policy through the command path; the scored
            // horizon stops the run; advisory pauses are cleared so the policy keeps running.
            if let Some(event) = reason
                .strip_prefix("event:")
                .and_then(|id| id.parse::<u64>().ok())
            {
                if !runner.answer_event(session, event) {
                    paused = Some(reason);
                    break;
                }
                session.dismiss_pause();
            } else if reason == "five_year_score" {
                paused = Some(reason);
                break;
            } else {
                session.dismiss_pause();
            }
        }
        if result.weeks_advanced == 0 {
            break;
        }
        runner.act(session);
    }
    (advanced, paused)
}

fn run(arguments: &[String]) {
    let weeks: u32 = flag(arguments, "--weeks")
        .and_then(|value| value.parse().ok())
        .unwrap_or(52);
    let seed: u64 = flag(arguments, "--seed")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let (guided, five_year) = scenario_flags(arguments);
    let policy = policy_flag(arguments);
    let verbose = arguments.iter().any(|argument| argument == "--verbose");
    let mut scenario = config(seed, guided, five_year);
    scenario.prices = prices_flag(arguments);
    let mut session = match Session::create(scenario) {
        Ok(session) => session,
        Err(error) => {
            eprintln!("session failed: {:?}", error);
            std::process::exit(1);
        }
    };
    let mut runner = PolicyRunner::new(policy, verbose);
    let (_advanced, paused) = drive(&mut session, weeks, &mut runner);
    if let Some(reason) = &paused {
        println!("paused at week {}: {reason}", session.world.tick.0);
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
    let firms = session.query(Query::Firms);
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
                "primary={} status={:?} votes={} investor_qty={} issued={} cash={} fair={} \
                 ni13={} debt={} target_bps={} marketing={}",
                firm.name,
                firm.status,
                firm.shares.vote_bps(cw_sim::state::HolderId::Investor),
                firm.shares.qty(cw_sim::state::HolderId::Investor),
                firm.shares.issued,
                firm.book.cash(),
                firm.shares.fair_cents,
                firm.book
                    .recent_weeks()
                    .iter()
                    .map(|week| week.net_income)
                    .sum::<i64>(),
                firm.book.total_liabilities(),
                firm.policies.production_target_bps,
                firm.policies.marketing_budget_cents
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
    {
        let stats = &session.world.structure.stats;
        println!(
            "structure bonds={} bond_defaults={} jvs={} ipos={} spin_offs={} carve_outs={} activists={} activist_wins={} group_lots={} squeeze_cents={} tax_shift_cents={} repeat_raids={}",
            stats.bonds_issued,
            stats.bond_defaults,
            stats.jvs_formed,
            stats.ipos,
            stats.spin_offs,
            stats.carve_outs,
            stats.activist_events,
            stats.activist_wins,
            stats.group_lots_moved,
            stats.squeeze_cents,
            stats.tax_shift_cents,
            stats.raids_repeat
        );
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
            "score total={} growth={} expansion={} leadership={} resilience={} control={} conduct={}",
            score.total,
            score.return_component,
            score.expansion_component,
            score.leadership_component,
            score.resilience_component,
            score.control_component,
            score.conduct_component
        );
    }
    if let cw_session::Detail::Deals(deals) = session.query(Query::Deals)
        && !deals.is_empty()
    {
        let summary: Vec<String> = deals
            .iter()
            .map(|deal| format!("{}->{}:{}", deal.buyer, deal.target, deal.state))
            .collect();
        println!("deals={}", summary.join(", "));
    }
    if policy != ReferencePolicy::None {
        println!(
            "policy={} attempted={} accepted={} rejected={}",
            policy.key(),
            runner.stats.attempted,
            runner.stats.accepted,
            runner.stats.rejected
        );
        if !runner.stats.rejections.is_empty() {
            println!("policy_rejections={:?}", runner.stats.rejections);
        }
    }
    if let Ok(bytes) = session.export_save() {
        println!("save_bytes={}", bytes.len());
    }
}

/// One seed's row in the machine-readable distribution.
#[derive(Clone, serde::Serialize)]
struct ScoreRow {
    total: i32,
    return_component: i32,
    expansion_component: i32,
    leadership_component: i32,
    resilience_component: i32,
    control_component: i32,
    conduct_component: i32,
    personal_net_worth_cents: i64,
}

/// Conservation checks the sweep performs on every final state. A non-zero count is a defect,
/// not a balance result.
#[derive(Clone, serde::Serialize)]
struct Invariants {
    ledger_balanced: bool,
    negative_cash: u32,
    negative_stock: u32,
}

#[derive(Clone, serde::Serialize)]
struct RunReport {
    seed: u64,
    final_week: u32,
    paused: Option<String>,
    score: Option<ScoreRow>,
    investor_net_worth_cents: i64,
    investor_cash_cents: i64,
    /// Firms in the investor's controlled groups, including subsidiaries of a controlled firm.
    controlled_firms: u64,
    dissolved: u64,
    past_due: u64,
    weeks_with_past_due: u32,
    deals_settled: u64,
    canonical_hash: String,
    policy: PolicyStats,
    invariants: Invariants,
    /// Rival outcomes by archetype: a win is a surviving firm whose book equity grew, a loss
    /// is a dissolved firm or one whose equity fell.
    archetypes: BTreeMap<String, ArchetypeRow>,
}

#[derive(serde::Serialize, Default, Clone)]
struct ArchetypeRow {
    firms: u64,
    wins: u64,
    losses: u64,
    dissolved: u64,
    accepted_commands: u64,
    rejected_commands: u64,
    deals_started: u64,
    deals_settled: u64,
}

fn opening_equity(session: &Session) -> BTreeMap<u64, i64> {
    session
        .world
        .firms
        .iter()
        .map(|(handle, firm)| (handle.0, firm.book.total_equity()))
        .collect()
}

fn archetype_rows(
    session: &Session,
    opening: &BTreeMap<u64, i64>,
) -> BTreeMap<String, ArchetypeRow> {
    let mut rows: BTreeMap<String, ArchetypeRow> = BTreeMap::new();
    for (handle, firm) in session.world.firms.iter() {
        let Some(archetype) = firm.archetype else {
            continue;
        };
        let Some(start) = opening.get(&handle.0) else {
            continue;
        };
        let row = rows.entry(archetype.key().to_owned()).or_default();
        row.firms += 1;
        let dissolved = matches!(firm.status, cw_sim::state::FirmStatus::Dissolved);
        let merged = matches!(firm.status, cw_sim::state::FirmStatus::Merged { .. });
        if dissolved {
            row.dissolved += 1;
            row.losses += 1;
        } else if !merged && firm.book.total_equity() > *start {
            row.wins += 1;
        } else if !merged {
            row.losses += 1;
        }
        for (_, deal) in session.world.deals.iter() {
            if deal.buyer == handle {
                row.deals_started += 1;
                if deal.state == cw_sim::state::DealState::Settled {
                    row.deals_settled += 1;
                }
            }
        }
    }
    for (key, actions) in &session.ai_log.by_archetype {
        if let Some(row) = rows.get_mut(key) {
            row.accepted_commands = actions.accepted;
            row.rejected_commands = actions.rejected;
        }
    }
    rows
}

#[derive(serde::Serialize)]
struct Distribution {
    count: usize,
    min: i64,
    median: i64,
    max: i64,
    mean: i64,
}

#[derive(serde::Serialize)]
struct Outlier {
    seed: u64,
    kind: String,
    metric: Option<String>,
    value: Option<i64>,
    week: u32,
    reason: Option<String>,
}

#[derive(serde::Serialize)]
struct SweepReport {
    schema: &'static str,
    engine_version: &'static str,
    interface_version: u32,
    rules_version: u32,
    dataset_hash: String,
    policy: &'static str,
    scenario: &'static str,
    mode: &'static str,
    weeks: u32,
    seed_from: u64,
    seed_count: u64,
    runs: Vec<RunReport>,
    summary: BTreeMap<String, Distribution>,
    archetypes: BTreeMap<String, ArchetypeRow>,
    outliers: Vec<Outlier>,
    errors: Vec<String>,
}

fn observe(session: &Session, seed: u64, paused: Option<String>) -> RunReport {
    let mut dissolved = 0u64;
    let mut past_due = 0u64;
    if let cw_session::Detail::Firms(rows) = session.query(Query::Firms) {
        dissolved = rows
            .iter()
            .filter(|firm| firm.status == "dissolved")
            .count() as u64;
        past_due = rows.iter().filter(|firm| firm.status == "past_due").count() as u64;
    }
    // "Controlled" means the group, not only the directly held firm: a settled acquisition is
    // controlled through the parent.
    let mut group: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
    for (handle, _) in session.world.firms.iter() {
        if cw_sim::economy::ownership::ultimate_controller(&session.world, handle)
            == Some(cw_sim::state::HolderId::Investor)
        {
            group.insert(handle.0);
        }
    }
    let controlled = group.len() as u64;
    let deals_settled = session
        .world
        .deals
        .iter()
        .filter(|(_, deal)| deal.state == cw_sim::state::DealState::Settled)
        .count() as u64;
    let mut invariants = Invariants {
        ledger_balanced: session.world.investor.book.is_balanced(),
        negative_cash: 0,
        negative_stock: 0,
    };
    if session.world.investor.cash_cents < 0 {
        invariants.negative_cash += 1;
    }
    for (_, firm) in session.world.firms.iter() {
        invariants.ledger_balanced &= firm.book.is_balanced();
        if firm.book.cash() < 0 {
            invariants.negative_cash += 1;
        }
        for (key, lot) in &firm.inventory {
            if lot.qty.0 < 0 {
                invariants.negative_stock += 1;
                eprintln!(
                    "negative stock seed={seed} week={} firm={} product={} region={} qty={}",
                    session.world.tick.0, firm.name, key.0, key.1, lot.qty.0
                );
            }
        }
        for handle in &firm.facilities {
            if let Some(facility) = session.world.facilities.get(*handle)
                && facility.deposit_remaining.0 < 0
            {
                invariants.negative_stock += 1;
                eprintln!(
                    "negative deposit seed={seed} week={} firm={} facility={} recipe={} deposit={}",
                    session.world.tick.0,
                    firm.name,
                    handle.0,
                    facility.recipe,
                    facility.deposit_remaining.0
                );
            }
        }
    }
    let score = session.world.score.as_ref().map(|score| ScoreRow {
        total: score.total,
        return_component: score.return_component,
        expansion_component: score.expansion_component,
        leadership_component: score.leadership_component,
        resilience_component: score.resilience_component,
        control_component: score.control_component,
        conduct_component: score.conduct_component,
        personal_net_worth_cents: score.personal_net_worth_cents,
    });
    RunReport {
        seed,
        final_week: session.world.tick.0,
        paused,
        score,
        investor_net_worth_cents: session.overview().investor_net_worth_cents,
        investor_cash_cents: session.world.investor.cash_cents,
        controlled_firms: controlled,
        dissolved,
        past_due,
        weeks_with_past_due: session.world.weeks_with_past_due,
        deals_settled,
        canonical_hash: session.canonical_hash(),
        policy: PolicyStats::default(),
        invariants,
        archetypes: BTreeMap::new(),
    }
}

fn median(sorted: &[i64]) -> i64 {
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2
    } else {
        sorted[middle]
    }
}

fn distribution(mut values: Vec<i64>) -> Option<Distribution> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let count = values.len();
    let mean = values.iter().sum::<i64>() / count as i64;
    Some(Distribution {
        count,
        min: values[0],
        median: median(&values),
        max: values[count - 1],
        mean,
    })
}

/// Tukey fences (1.5 x IQR) on a metric, returning the offending seeds.
fn tukey_outliers(
    runs: &[RunReport],
    metric: &str,
    value: impl Fn(&RunReport) -> i64,
    outliers: &mut Vec<Outlier>,
) {
    if runs.len() < 4 {
        return;
    }
    let mut values: Vec<i64> = runs.iter().map(&value).collect();
    values.sort_unstable();
    let q1 = values[values.len() / 4];
    let q3 = values[values.len() * 3 / 4];
    let iqr = q3 - q1;
    let low = q1 - iqr * 3 / 2;
    let high = q3 + iqr * 3 / 2;
    for run in runs {
        let observed = value(run);
        if observed < low || observed > high {
            outliers.push(Outlier {
                seed: run.seed,
                kind: "tukey".to_owned(),
                metric: Some(metric.to_owned()),
                value: Some(observed),
                week: run.final_week,
                reason: None,
            });
        }
    }
}

fn sweep(arguments: &[String]) {
    let seeds: u64 = flag(arguments, "--seeds")
        .and_then(|value| value.parse().ok())
        .unwrap_or(10);
    let weeks: u32 = flag(arguments, "--weeks")
        .and_then(|value| value.parse().ok())
        .unwrap_or(260);
    let from: u64 = flag(arguments, "--from")
        .and_then(|value| value.parse().ok())
        .unwrap_or(1);
    let (guided, five_year) = scenario_flags(arguments);
    let policy = policy_flag(arguments);
    let verbose = arguments.iter().any(|argument| argument == "--verbose");
    let (scenario_name, mode_name) = if guided {
        ("guided", if five_year { "five_year" } else { "sandbox" })
    } else {
        ("founder", if five_year { "five_year" } else { "sandbox" })
    };

    let mut runs: Vec<RunReport> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut dataset_hash = String::new();
    for seed in from..from + seeds {
        let mut session = match Session::create(config(seed, guided, five_year)) {
            Ok(session) => session,
            Err(error) => {
                let message = format!("seed {seed} failed: {:?}", error);
                eprintln!("{message}");
                errors.push(message);
                continue;
            }
        };
        dataset_hash = session.dataset_hash().to_owned();
        let opening = opening_equity(&session);
        let mut runner = PolicyRunner::new(policy, verbose);
        let (advanced, paused) = drive(&mut session, weeks, &mut runner);
        if advanced < weeks && paused.is_none() {
            eprintln!("seed {seed} advanced only {advanced} of {weeks} weeks");
        }
        let mut row = observe(&session, seed, paused);
        row.policy = runner.stats.clone();
        row.archetypes = archetype_rows(&session, &opening);
        if !row.invariants.ledger_balanced
            || row.invariants.negative_cash > 0
            || row.invariants.negative_stock > 0
        {
            eprintln!(
                "seed {seed} invariant violation: balanced={} negative_cash={} negative_stock={}",
                row.invariants.ledger_balanced,
                row.invariants.negative_cash,
                row.invariants.negative_stock
            );
        }
        println!(
            "seed={} week={} score={} net_worth={} controlled={} dissolved={} past_due={} past_due_weeks={} deals={} hash={}",
            row.seed,
            row.final_week,
            row.score.as_ref().map(|score| score.total).unwrap_or(-1),
            row.investor_net_worth_cents,
            row.controlled_firms,
            row.dissolved,
            row.past_due,
            row.weeks_with_past_due,
            row.deals_settled,
            row.canonical_hash
        );
        runs.push(row);
    }

    let mut summary: BTreeMap<String, Distribution> = BTreeMap::new();
    let mut metric = |name: &str, values: Vec<i64>| {
        if let Some(dist) = distribution(values) {
            summary.insert(name.to_owned(), dist);
        }
    };
    metric(
        "score_total",
        runs.iter()
            .filter_map(|run| run.score.as_ref().map(|score| score.total as i64))
            .collect(),
    );
    metric(
        "investor_net_worth_cents",
        runs.iter()
            .map(|run| run.investor_net_worth_cents)
            .collect(),
    );
    metric(
        "controlled_firms",
        runs.iter().map(|run| run.controlled_firms as i64).collect(),
    );
    metric(
        "dissolved",
        runs.iter().map(|run| run.dissolved as i64).collect(),
    );
    metric(
        "past_due",
        runs.iter().map(|run| run.past_due as i64).collect(),
    );
    metric(
        "weeks_with_past_due",
        runs.iter()
            .map(|run| run.weeks_with_past_due as i64)
            .collect(),
    );
    metric(
        "deals_settled",
        runs.iter().map(|run| run.deals_settled as i64).collect(),
    );

    let mut archetype_totals: BTreeMap<String, ArchetypeRow> = BTreeMap::new();
    for run in &runs {
        for (key, row) in &run.archetypes {
            let total = archetype_totals.entry(key.clone()).or_default();
            total.firms += row.firms;
            total.wins += row.wins;
            total.losses += row.losses;
            total.dissolved += row.dissolved;
            total.accepted_commands += row.accepted_commands;
            total.rejected_commands += row.rejected_commands;
            total.deals_started += row.deals_started;
            total.deals_settled += row.deals_settled;
        }
    }

    let mut outliers: Vec<Outlier> = runs
        .iter()
        .filter(|run| run.final_week < weeks)
        .map(|run| Outlier {
            seed: run.seed,
            kind: "early_stop".to_owned(),
            metric: None,
            value: None,
            week: run.final_week,
            reason: run.paused.clone(),
        })
        .collect();
    tukey_outliers(
        &runs,
        "score_total",
        |run| {
            run.score
                .as_ref()
                .map(|score| score.total as i64)
                .unwrap_or(-1)
        },
        &mut outliers,
    );
    tukey_outliers(
        &runs,
        "investor_net_worth_cents",
        |run| run.investor_net_worth_cents,
        &mut outliers,
    );
    tukey_outliers(
        &runs,
        "dissolved",
        |run| run.dissolved as i64,
        &mut outliers,
    );

    let report = SweepReport {
        schema: "cw-balance-sweep/1",
        engine_version: cw_core::ENGINE_VERSION,
        interface_version: cw_core::INTERFACE_VERSION,
        rules_version: cw_core::RULES_VERSION,
        dataset_hash,
        policy: policy.key(),
        scenario: scenario_name,
        mode: mode_name,
        weeks,
        seed_from: from,
        seed_count: seeds,
        runs,
        summary,
        archetypes: archetype_totals,
        outliers,
        errors,
    };

    if let Some(path) = flag(arguments, "--json") {
        let text = serde_json::to_string_pretty(&report).expect("sweep report serialises");
        if let Err(error) = fs::write(&path, format!("{text}\n")) {
            eprintln!("cannot write {path}: {error}");
            std::process::exit(1);
        }
        println!("wrote {path}");
    }

    for (name, dist) in &report.summary {
        println!(
            "summary {name} count={} min={} median={} max={} mean={}",
            dist.count, dist.min, dist.median, dist.max, dist.mean
        );
    }
    for (key, row) in &report.archetypes {
        println!(
            "archetype {key} firms={} wins={} losses={} dissolved={} accepted={} rejected={} deals={}/{}",
            row.firms,
            row.wins,
            row.losses,
            row.dissolved,
            row.accepted_commands,
            row.rejected_commands,
            row.deals_settled,
            row.deals_started
        );
    }
    println!(
        "sweep policy={} scenario={} mode={} seeds={}..{} weeks={} outliers={} errors={}",
        report.policy,
        report.scenario,
        report.mode,
        report.seed_from,
        report.seed_from + report.seed_count - 1,
        report.weeks,
        report.outliers.len(),
        report.errors.len()
    );
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
                .last_closed_sales()
                .map(|week| week.values().sum())
                .unwrap_or(0);
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

fn bench(arguments: &[String]) {
    use std::time::Instant;
    let envelope = flag(arguments, "--envelope").unwrap_or_else(|| "representative".to_owned());
    let weeks: u32 = flag(arguments, "--weeks")
        .and_then(|value| value.parse().ok())
        .unwrap_or(3);
    let dataset = cw_data::Dataset::load().expect("dataset");
    let base = cw_sim::economy::seed::build(
        &dataset,
        501,
        cw_sim::state::ScenarioKind::Guided,
        cw_sim::state::ScenarioMode::Sandbox,
    );
    let Some(mut world) = cw_sim::fixtures::envelope(&base, &envelope) else {
        eprintln!("unknown envelope {envelope}");
        std::process::exit(2);
    };
    let (firms, facilities, contracts) = cw_sim::fixtures::counts(&world);
    let mut times = Vec::new();
    for _ in 0..weeks {
        let start = Instant::now();
        cw_sim::tick::advance(&mut world, &dataset);
        times.push(start.elapsed().as_secs_f64() * 1_000.0);
    }
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = times[times.len() / 2];
    let p99 = times[(times.len() as f64 * 0.99).ceil() as usize - 1];
    let total: f64 = times.iter().sum();
    println!(
        "envelope={envelope} firms={firms} facilities={facilities} contracts={contracts} weeks={weeks} \
         p50_ms={p50:.2} p99_ms={p99:.2} max_ms={:.2} total_ms={total:.2}",
        times.last().copied().unwrap_or(0.0)
    );
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
