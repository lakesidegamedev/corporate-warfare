//! Entrants against the board over two years.
//!
//! In each scenario the world runs to week 36 with no player. For each sector class (primary,
//! manufacturing, consumer service, business service, retail) the test founds a new company
//! straight into each of the board's top five openings of that class, in a fresh copy of the
//! same world, and follows its plant for two years after it opens. It reports what the plant sold
//! and what the company earned (revenue less inputs, wages and running costs) over the quarter
//! ending 26, 52 and 104 weeks after opening, against what the opening promised: its block's
//! output at the share its inputs and room allow, and its weekly contribution. The gate: at 52
//! weeks at least four of every five entrants of a class sell 60 to 140 percent of the promise.
//!
//! Heavy (up to a hundred two-year worlds), so it is ignored in the normal run:
//! `cargo test -p cw-session --release --test entry_two_years -- --ignored --nocapture`.
//! `ENTRY_CLASS=<class>` runs one class and `ENTRY_N=<n>` takes the top n openings of it.

use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent};
use cw_sim::state::{FacilityStatus, ScenarioKind, ScenarioMode};

const AT: u32 = 36;
const QUARTER: usize = 13;
const MARKS: [usize; 3] = [26, 52, 104];
const LOW_PCT: i64 = 60;
const HIGH_PCT: i64 = 140;
const PER_CLASS: usize = 5;
const CLASSES: [&str; 5] = [
    "primary",
    "manufacturing",
    "consumer service",
    "business service",
    "retail",
];

fn world(scenario: &str) -> Session {
    let mut session = Session::create(ScenarioConfig {
        seed: 1,
        scenario: ScenarioKind::Founder,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: scenario.to_owned(),
            path: None,
            timeline: None,
        },
    })
    .unwrap();
    while session.world.tick.0 < AT {
        week(&mut session);
    }
    session
}

fn week(session: &mut Session) {
    session.advance(1);
    for e in session.world.events.pending.iter_mut() {
        e.resolved = true;
    }
    if session.world.pending_pause.is_some() {
        session.dismiss_pause();
    }
}

/// The sector class of a strategy, from what it makes and what it buys.
fn class(dataset: &cw_data::Dataset, recipe: u16) -> &'static str {
    let template = &dataset.recipes[recipe as usize];
    let Some(product) = dataset.product_index.get(&template.output).copied() else {
        return "other";
    };
    let kind = dataset.products[product as usize].kind;
    if kind.is_consumer_goods() {
        return "retail";
    }
    if kind.is_service() {
        return if cw_sim::economy::capacity::household_good(dataset, product) {
            "consumer service"
        } else {
            "business service"
        };
    }
    let buys_goods = template.weekly_inputs_lots.keys().any(|id| {
        dataset
            .product_index
            .get(id)
            .is_some_and(|p| dataset.products[*p as usize].kind == cw_data::ProductKind::Goods)
    });
    if buys_goods {
        "manufacturing"
    } else {
        "primary"
    }
}

/// Lots of `product` the firm booked as sold last week, and what it earned before depreciation,
/// interest and tax.
fn last_week(session: &Session, firm: cw_core::ids::Handle, product: u16) -> (i64, i64) {
    let Some(entity) = session.world.firm(firm) else {
        return (0, 0);
    };
    let sold = entity
        .last_closed_sales()
        .map(|week| {
            week.iter()
                .filter(|((_, p), _)| *p == product)
                .map(|(_, lots)| *lots)
                .sum()
        })
        .unwrap_or(0);
    let earned = entity
        .book
        .recent_weeks()
        .last()
        .map(|w| w.revenue - w.cost_of_goods - w.wages - w.operating_expense)
        .unwrap_or(0);
    (sold, earned)
}

fn measure(scenario: &str) {
    let board = {
        let session = world(scenario);
        let board = cw_sim::economy::capacity::openings(&session.world, &session.dataset, 0);
        board
            .into_iter()
            .map(|open| (class(&session.dataset, open.recipe), open))
            .collect::<Vec<_>>()
    };
    let mut failures = Vec::new();
    for class_name in CLASSES {
        if std::env::var("ENTRY_CLASS").is_ok_and(|c| c != class_name) {
            continue;
        }
        let picks: Vec<_> = board
            .iter()
            .filter(|(c, _)| *c == class_name)
            .map(|(_, open)| open.clone())
            .take(
                std::env::var("ENTRY_N")
                    .ok()
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(PER_CLASS),
            )
            .collect();
        if picks.is_empty() {
            println!("{scenario} {class_name}: no opening on the board");
            continue;
        }
        let mut in_band = 0;
        for open in &picks {
            let mut session = world(scenario);
            let promised = (open.block_lots * open.input_fill_bps / 10_000).max(1);
            let receipt = session.submit(CommandEnvelope {
                command_id_high: 22,
                command_id_low: 1,
                actor: 0,
                expected_revision: session.revision(),
                submitted_tick: session.world.tick.0,
                intent: CommandIntent::FoundFirm {
                    name: "Entrant".to_owned(),
                    region: open.region,
                    capital_cents: open.build_cost_cents * 3 + 50_000_000,
                    recipe: open.recipe,
                    secondary: None,
                },
            });
            assert!(receipt.is_ok(), "{scenario}: found {receipt:?}");
            let firm = session.world.investor.primary_group.unwrap();
            let mut weeks: Vec<(i64, i64)> = Vec::new();
            let limit = AT + 30 + 104;
            while weeks.len() < 104 && session.world.tick.0 < limit {
                week(&mut session);
                let open_now = session.world.firm(firm).is_some_and(|f| {
                    f.facilities.iter().any(|h| {
                        session.world.facilities.get(*h).is_some_and(|p| {
                            matches!(
                                p.status,
                                FacilityStatus::Operating | FacilityStatus::Mothballed
                            )
                        })
                    })
                });
                if open_now || !weeks.is_empty() {
                    weeks.push(last_week(&session, firm, open.product));
                }
            }
            let recipe = &session.dataset.recipes[open.recipe as usize].id;
            let region = &session.dataset.regions[open.region as usize].code;
            let mut cells = Vec::new();
            let mut sold_52 = None;
            for mark in MARKS {
                if weeks.len() < mark {
                    cells.push(format!("w{mark} -"));
                    continue;
                }
                let window = &weeks[mark - QUARTER..mark];
                let sold = window.iter().map(|w| w.0).sum::<i64>() / QUARTER as i64;
                let earned = window.iter().map(|w| w.1).sum::<i64>() / QUARTER as i64;
                let sold_pct = sold * 100 / promised;
                let earned_pct = if open.weekly_contribution_cents > 0 {
                    earned * 100 / open.weekly_contribution_cents
                } else {
                    0
                };
                if mark == 52 {
                    sold_52 = Some(sold_pct);
                }
                cells.push(format!(
                    "w{mark} sold {sold}/{promised} ({sold_pct}%) earned {} vs {} ({earned_pct}%)",
                    earned / 100,
                    open.weekly_contribution_cents / 100
                ));
            }
            if sold_52.is_some_and(|pct| (LOW_PCT..=HIGH_PCT).contains(&pct)) {
                in_band += 1;
            }
            println!(
                "{scenario} | {class_name} | {recipe} {region} | {}",
                cells.join(" | ")
            );
        }
        let need = picks.len().min(PER_CLASS).saturating_sub(1).max(1);
        println!(
            "{scenario} {class_name}: {in_band} of {} in 60 to 140 percent at 52 weeks",
            picks.len()
        );
        if in_band < need {
            failures.push(format!(
                "{scenario} {class_name}: {in_band} of {} in band",
                picks.len()
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
#[ignore]
fn great_lakes_entrants_over_two_years() {
    measure("great-lakes-2000");
}

#[test]
#[ignore]
fn texas_entrants_over_two_years() {
    measure("texas-1981");
}

#[test]
#[ignore]
fn silicon_valley_entrants_over_two_years() {
    measure("silicon-valley-1995");
}

#[test]
#[ignore]
fn ruhr_entrants_over_two_years() {
    measure("ruhr-rhine-1991");
}
