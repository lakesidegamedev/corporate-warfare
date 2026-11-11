//! A new seller sells what the sector board promised it.
//!
//! In each scenario the world runs to week 36 with no player. The test then founds a new company
//! straight into each of the board's top three openings, in a fresh copy of the same world, and
//! follows its plant for two quarters after it opens. Over the second quarter (the first is the
//! ramp up) the plant must sell, on average, between half and six fifths of the lots a week the
//! opening said it could sell: its block's output at the share its inputs and its room allow.
//! The oracle is the company's own booked sales of the good in every market, not the division
//! report the board's figures come from.

use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent};
use cw_sim::state::{FacilityStatus, ScenarioKind, ScenarioMode};

const SCENARIOS: [&str; 4] = [
    "great-lakes-2000",
    "ruhr-rhine-1991",
    "silicon-valley-1995",
    "texas-1981",
];
const AT: u32 = 36;
const QUARTER: usize = 13;
const LOW_PCT: i64 = 50;
const HIGH_PCT: i64 = 120;

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

/// Lots of `product` the firm booked as sold last week, in every market.
fn sold(session: &Session, firm: cw_core::ids::Handle, product: u16) -> i64 {
    session
        .world
        .firm(firm)
        .and_then(|f| f.last_closed_sales())
        .map(|week| {
            week.iter()
                .filter(|((_, p), _)| *p == product)
                .map(|(_, lots)| *lots)
                .sum()
        })
        .unwrap_or(0)
}

#[test]
fn a_new_seller_sells_what_its_opening_promised() {
    let mut failures = Vec::new();
    for scenario in SCENARIOS {
        let board = {
            let session = world(scenario);
            cw_sim::economy::capacity::openings(&session.world, &session.dataset, 0)
        };
        assert!(board.len() >= 3, "{scenario}: fewer than three openings");
        for open in board.iter().take(3) {
            let mut session = world(scenario);
            let expected = open.block_lots * open.input_fill_bps / 10_000;
            let receipt = session.submit(CommandEnvelope {
                command_id_high: 21,
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
            let mut weeks: Vec<i64> = Vec::new();
            let limit = AT + 80;
            while weeks.len() < 2 * QUARTER && session.world.tick.0 < limit {
                week(&mut session);
                let open_now = session.world.firm(firm).is_some_and(|f| {
                    f.facilities.iter().any(|h| {
                        session
                            .world
                            .facilities
                            .get(*h)
                            .is_some_and(|p| p.status == FacilityStatus::Operating)
                    })
                });
                if open_now {
                    weeks.push(sold(&session, firm, open.product));
                }
            }
            let recipe = &session.dataset.recipes[open.recipe as usize].id;
            let region = &session.dataset.regions[open.region as usize].code;
            if weeks.len() < 2 * QUARTER {
                failures.push(format!(
                    "{scenario} {recipe} {region}: never ran two quarters"
                ));
                continue;
            }
            let second: i64 = weeks[QUARTER..].iter().sum::<i64>() / QUARTER as i64;
            let pct = second * 100 / expected.max(1);
            println!(
                "{scenario} {recipe} {region}: expected {expected} a week, sold {second} ({pct}%)"
            );
            if !(LOW_PCT..=HIGH_PCT).contains(&pct) {
                failures.push(format!(
                    "{scenario} {recipe} {region}: expected {expected}, sold {second} ({pct}%)"
                ));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
