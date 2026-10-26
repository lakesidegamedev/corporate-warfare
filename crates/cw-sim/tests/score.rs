//! Five-year score evidence (R34).
//!
//! docs/rules/CONTENT-PACING.md: return = clamp(50 + 25*(end/start - 1)), resilience = share of
//! weeks without past-due liabilities, control = controlled productive-group value / portfolio,
//! conduct = 100 - 10*breaches - 5*failed settlements; total 0.6/0.2/0.1/0.1. Net worth is
//! personal cash plus direct holdings at fair value, never internal revenue or a parent and its
//! subsidiary counted twice. Week 260 pauses for the score; the sandbox continues after it.

use std::collections::BTreeMap;

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_data::Dataset;
use cw_sim::economy::{ownership, score, seed};
use cw_sim::state::{HolderId, ScenarioKind, ScenarioMode, ShareBook, ShareLot, World};

fn world(mode: ScenarioMode) -> World {
    let dataset = Dataset::load().unwrap();
    seed::build(&dataset, 401, ScenarioKind::Guided, mode)
}

fn register(world: &mut World, firm: Handle, holders: &[(HolderId, i64)], fair: i64) {
    let lots: BTreeMap<HolderId, ShareLot> = holders
        .iter()
        .map(|(holder, qty)| {
            (
                *holder,
                ShareLot {
                    qty: *qty,
                    cost_cents: 0,
                },
            )
        })
        .collect();
    let book = &mut world.firms.get_mut(firm).unwrap().shares;
    *book = ShareBook {
        fair_cents: fair,
        mark_cents: fair,
        ..ShareBook::new(lots)
    };
}

#[test]
fn control_counts_a_controlled_parent_once_and_ignores_passive_stakes() {
    let mut world = world(ScenarioMode::FiveYear);
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in &handles {
        register(&mut world, *handle, &[(HolderId::Outside(0), 1_000)], 100);
    }
    let (parent, child, passive) = (handles[0], handles[1], handles[2]);
    // The investor owns the parent outright; the parent controls the child with 60 percent; the
    // investor also holds a passive tenth of a third company.
    register(&mut world, parent, &[(HolderId::Investor, 1_000)], 5_000);
    register(
        &mut world,
        child,
        &[(HolderId::Firm(parent), 600), (HolderId::Outside(0), 400)],
        2_000,
    );
    register(
        &mut world,
        passive,
        &[(HolderId::Investor, 100), (HolderId::Outside(0), 900)],
        3_000,
    );
    world.investor.cash_cents = 0;
    world.investor.personal_loans_cents = 0;
    // Only the parent stake is controlled value: 1,000 * $50.00. The child is inside the
    // parent's fair value already; the passive stake is not control.
    assert_eq!(ownership::group_value(&world), 5_000_000);
    // Portfolio: 5,000,000 + 100 * 3,000 = 5,300,000. Control = 5.0m / 5.3m = 94 percent.
    assert_eq!(score::personal_net_worth(&world), 5_300_000);
    assert_eq!(
        score::compute(&world, &Dataset::load().unwrap()).control_component,
        94
    );
}

#[test]
fn growth_resilience_and_conduct_follow_the_published_formulas() {
    let mut world = world(ScenarioMode::FiveYear);
    let start = score::personal_net_worth(&world);
    world.investor.starting_net_worth_cents = start;
    // Half as much again: 20 + 80 * 0.5 = 60.
    world.investor.cash_cents += start / 2;
    world.weeks_elapsed = 260;
    world.weeks_with_past_due = 26;
    world.regulatory_breaches = 2;
    world.failed_worker_settlements = 1;
    let dataset = Dataset::load().unwrap();
    let result = score::compute(&world, &dataset);
    assert_eq!(result.return_component, 60);
    assert_eq!(result.resilience_component, 90);
    assert_eq!(result.conduct_component, 75);
    // The guided start is one division in one state: no expansion yet.
    assert_eq!(result.expansion_component, 0);
    let expected = (result.return_component * 45
        + result.expansion_component * 25
        + result.leadership_component * 10
        + result.resilience_component * 10
        + result.conduct_component * 10)
        / 100;
    // The total is computed on unrounded components, so it may sit one point above the
    // rounded-component recombination but never below it.
    assert!((expected..=expected + 1).contains(&result.total));
}

#[test]
fn expansion_counts_divisions_and_states_beyond_the_first() {
    let dataset = Dataset::load().unwrap();
    let mut world = world(ScenarioMode::FiveYear);
    let (firm, plant) = world
        .firms
        .iter()
        .find_map(|(handle, firm)| {
            cw_sim::economy::events_engine::player_side(&world, handle)
                .then(|| firm.facilities.first().copied())
                .flatten()
                .map(|plant| (handle, plant))
        })
        .expect("the player's farm");
    let home = world.facilities.get(plant).unwrap().region;
    // Two more divisions: another sector in the home region, and the farm strategy in a
    // region of another state.
    let other_state = (0..dataset.regions.len() as u8)
        .find(|r| dataset.regions[*r as usize].state() != dataset.regions[home as usize].state())
        .unwrap();
    let retail = dataset.recipe_index["grocer"];
    for (region, recipe) in [
        (home, retail),
        (other_state, world.facilities.get(plant).unwrap().recipe),
    ] {
        let mut copy = world.facilities.get(plant).unwrap().clone();
        copy.region = region;
        copy.recipe = recipe;
        let handle = world.facilities.insert(copy);
        world.firms.get_mut(firm).unwrap().facilities.push(handle);
    }
    let (divisions, states, _) = score::footprint(&world, &dataset);
    assert_eq!((divisions, states), (3, 2));
    // 20 * 2 more divisions + 10 * 1 more state = 50.
    assert_eq!(score::compute(&world, &dataset).expansion_component, 50);
}

#[test]
fn the_score_pauses_at_week_260_and_the_sandbox_continues_with_it_frozen() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 402, ScenarioKind::Guided, ScenarioMode::FiveYear);
    let mut paused_at = None;
    for _ in 0..300 {
        let outcome = cw_sim::tick::advance(&mut world, &dataset);
        if outcome.pause.as_deref() == Some("five_year_score") && paused_at.is_none() {
            paused_at = Some(world.tick);
        }
        for event in world.events.pending.iter_mut() {
            event.resolved = true;
        }
        world.pending_pause = None;
    }
    assert_eq!(paused_at, Some(Tick(260)));
    let frozen = world.score.clone().expect("the five-year score is kept");
    assert_eq!(frozen.week, Tick(260));
    assert_eq!(world.tick, Tick(300), "the career continues past the score");
}

#[test]
fn career_losses_and_assisted_recovery_survive_a_reload() {
    let mut world = world(ScenarioMode::FiveYear);
    world.investor.career_losses_cents = 123_456;
    world.investor.assisted_score = true;
    world.investor.recovery_used = 1;
    let bytes = postcard::to_allocvec(&world).unwrap();
    let reloaded: World = postcard::from_bytes(&bytes).unwrap();
    assert_eq!(reloaded.investor.career_losses_cents, 123_456);
    assert_eq!(reloaded.investor.recovery_used, 1);
    assert!(
        score::compute(&reloaded, &Dataset::load().unwrap()).assisted,
        "assisted scores are flagged, not erased"
    );
}
