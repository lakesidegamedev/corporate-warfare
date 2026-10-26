//! Board objectives: a quarterly goal for the player's primary company, paid as a management
//! bonus moved from the company to the investor when met. Rules in
//! crates/cw-sim/src/economy/objectives.rs.

use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR, apply};
use cw_sim::economy::objectives::{self, ObjectiveKind};
use cw_sim::economy::seed;
use cw_sim::state::{ScenarioKind, ScenarioMode};

#[test]
fn a_met_objective_pays_the_bonus_once_and_mints_nothing() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 77, ScenarioKind::Guided, ScenarioMode::Sandbox);
    cw_sim::tick::advance(&mut world, &dataset);
    let objective = world
        .objective
        .clone()
        .expect("the board sets a goal in week one");
    let firm = world.investor.primary_group.unwrap();
    assert_eq!(objective.firm, firm);
    assert_eq!(objective.due, objective.set + objectives::QUARTER_WEEKS);
    let ObjectiveKind::NewRegion { baseline } = objective.kind else {
        panic!("the first quarter asks for a new region");
    };
    assert_eq!(baseline, 1);
    assert_eq!(objectives::progress_bps(&world, &dataset, &objective), 0);

    // Open a division in another region.
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, 100_000_000, "test funding")
            .credit(Account::ShareCapital, 100_000_000, "test funding"),
    );
    let home = world.firms.get(firm).unwrap().region;
    let away = (0..dataset.regions.len() as u8)
        .find(|r| *r != home)
        .unwrap();
    let recipe = dataset.recipe_index["farm"];
    let (revision, tick) = (world.revision, world.tick.0);
    apply(
        &mut world,
        &dataset,
        &CommandEnvelope {
            command_id_high: 3,
            command_id_low: 1,
            actor: INVESTOR_ACTOR,
            expected_revision: revision,
            submitted_tick: tick,
            intent: CommandIntent::BuildFacility {
                firm,
                region: away,
                recipe,
                blocks: 1,
            },
        },
    )
    .expect("the build is accepted");
    assert_eq!(
        objectives::progress_bps(&world, &dataset, &objective),
        10_000
    );

    let investor_before = world.investor.cash_cents;
    cw_sim::tick::advance(&mut world, &dataset);
    let result = world
        .objective_history
        .last()
        .expect("the objective closed")
        .clone();
    assert!(result.met);
    assert!(result.bonus_cents >= objectives::MIN_BONUS_CENTS);
    assert!(
        world.objective.is_none(),
        "no new goal until the next quarter"
    );
    // The bonus is the only change to personal cash this week.
    assert_eq!(
        world.investor.cash_cents - investor_before,
        result.bonus_cents
    );
    assert!(world.investor.book.is_balanced());
    assert_eq!(world.objective_history.len(), 1, "paid exactly once");
    cw_sim::tick::advance(&mut world, &dataset);
    assert_eq!(world.objective_history.len(), 1);
}
