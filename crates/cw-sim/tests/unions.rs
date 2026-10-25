//! Worker organisation evidence (R23): the 95 percent settlement bound, strike staffing and a
//! ballot outcome that survives reload. docs/rules/FINANCE-LABOUR-AI.md.

use cw_core::calendar::Tick;
use cw_data::Dataset;
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR, apply};
use cw_sim::economy::{seed, unions};
use cw_sim::state::{BargainState, ScenarioKind, ScenarioMode, World};

fn settle(world: &World, low: u64, firm: cw_core::ids::Handle, amount: i64) -> CommandEnvelope {
    CommandEnvelope {
        command_id_high: 0x23,
        command_id_low: low,
        actor: INVESTOR_ACTOR,
        expected_revision: world.revision,
        submitted_tick: world.tick.0,
        intent: CommandIntent::ProposeSettlement {
            firm,
            amount_cents: amount,
        },
    }
}

#[test]
fn a_settlement_below_ninety_five_percent_of_the_demand_is_refused() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 801, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    {
        let entity = world.firms.get_mut(firm).unwrap();
        entity.union.state = BargainState::Demanding {
            deadline: Tick(world.tick.0 + 2),
        };
        entity.union.demand_cents = 100_000;
    }
    let low = settle(&world, 1, firm, 94_999);
    assert!(apply(&mut world, &dataset, &low).is_err());
    assert!(matches!(
        world.firm(firm).unwrap().union.state,
        BargainState::Demanding { .. }
    ));
    let fair = settle(&world, 2, firm, 95_000);
    apply(&mut world, &dataset, &fair).expect("95 percent is accepted");
    assert_eq!(world.firm(firm).unwrap().union.state, BargainState::Idle);
}

#[test]
fn a_strike_leaves_three_tenths_of_the_staff_available() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 802, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    assert_eq!(unions::staffing_multiplier_bps(&world, firm), 10_000);
    world.firms.get_mut(firm).unwrap().union.state = BargainState::Striking {
        until: Tick(world.tick.0 + 4),
        started: world.tick,
    };
    assert_eq!(unions::staffing_multiplier_bps(&world, firm), 3_000);
}

#[test]
fn a_ballot_outcome_survives_save_and_reload() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 803, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    let region = world.firm(firm).unwrap().region;
    let prevailing = world.regions[region as usize].labour[0].prevailing_wage_cents;
    {
        let entity = world.firms.get_mut(firm).unwrap();
        entity.union.organisation_bps = 5_500;
        entity.union.ballot_support_bps = 5_500;
        entity.organisation.insert(region, 5_500);
        entity.union.state = BargainState::Ballot;
        entity.union.last_demand_week = 1;
        entity.policies.wage_offer[0] = prevailing * 90 / 100;
    }
    world.tick = Tick(60);
    unions::weekly(&mut world, &dataset);
    let bytes = postcard::to_allocvec(&world).unwrap();
    let reloaded: World = postcard::from_bytes(&bytes).unwrap();
    let (a, b) = (
        world.firm(firm).unwrap().union.clone(),
        reloaded.firm(firm).unwrap().union.clone(),
    );
    assert!(matches!(b.state, BargainState::Demanding { .. }));
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
}
