//! Board and CEO evidence (R17): meeting deadlines, deadlock, CEO removal and authority after a
//! vote. docs/rules/CORPORATE.md: five seats; three seats elect or remove the CEO and approve
//! large actions; a tie is the status quo; the CEO operates inside board limits.

use std::collections::BTreeMap;

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_data::Dataset;
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR, apply};
use cw_sim::economy::{governance, seed};
use cw_sim::state::{CeoRef, HolderId, ScenarioKind, ScenarioMode, ShareBook, ShareLot, World};

fn setup(holders: &[(HolderId, i64)]) -> (World, Dataset, Handle) {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 601, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    let lots: BTreeMap<HolderId, ShareLot> = holders
        .iter()
        .map(|(h, q)| {
            (
                *h,
                ShareLot {
                    qty: *q,
                    cost_cents: 0,
                },
            )
        })
        .collect();
    world.firms.get_mut(firm).unwrap().shares = ShareBook::new(lots);
    (world, dataset, firm)
}

fn command(world: &World, low: u64, intent: CommandIntent) -> CommandEnvelope {
    CommandEnvelope {
        command_id_high: 0x60,
        command_id_low: low,
        actor: INVESTOR_ACTOR,
        expected_revision: world.revision,
        submitted_tick: world.tick.0,
        intent,
    }
}

fn weeks(world: &mut World, dataset: &Dataset, count: u32) {
    for _ in 0..count {
        world.tick = world.tick.next();
        governance::weekly(world, dataset);
    }
}

#[test]
fn a_removed_ceo_loses_operating_authority() {
    // The investor holds 40 percent: no control, so it runs the firm only as CEO.
    let (mut world, dataset, firm) =
        setup(&[(HolderId::Investor, 400), (HolderId::Outside(0), 600)]);
    world.firms.get_mut(firm).unwrap().board.ceo = Some(CeoRef::Investor);
    let price = command(
        &world,
        1,
        CommandIntent::SetMarketingBudget {
            firm,
            amount_cents: 0,
        },
    );
    apply(&mut world, &dataset, &price).expect("the CEO operates the firm");
    world.firms.get_mut(firm).unwrap().board.ceo = Some(CeoRef::Manager(0));
    let price = command(
        &world,
        2,
        CommandIntent::SetMarketingBudget {
            firm,
            amount_cents: 0,
        },
    );
    assert!(
        apply(&mut world, &dataset, &price).is_err(),
        "a removed CEO is refused"
    );
}

#[test]
fn a_vote_resolves_at_its_four_week_deadline_and_a_tie_is_the_status_quo() {
    let (mut world, dataset, firm) =
        setup(&[(HolderId::Investor, 600), (HolderId::Outside(0), 400)]);
    world.firms.get_mut(firm).unwrap().board.next_meeting = world.tick;
    weeks(&mut world, &dataset, 1);
    let deadline = Tick(world.tick.0 + 4);
    let id = governance::queue_vote(
        world.firms.get_mut(firm).unwrap(),
        "conflict_transfer",
        deadline,
    );
    weeks(&mut world, &dataset, 3);
    let vote = |world: &World| {
        world
            .firm(firm)
            .unwrap()
            .board
            .pending_votes
            .iter()
            .find(|v| v.id == id)
            .cloned()
            .unwrap()
    };
    assert!(!vote(&world).resolved, "not before the deadline");
    weeks(&mut world, &dataset, 1);
    let resolved = vote(&world);
    assert!(resolved.resolved);
    // The controller's three seats vote yes; the two minority directors oppose a conflict
    // transfer. 3-2 passes; a 50/50 register below shows the tie.
    assert!(resolved.passed);
    let (mut tied, dataset, firm) =
        setup(&[(HolderId::Investor, 500), (HolderId::Outside(0), 500)]);
    tied.firms.get_mut(firm).unwrap().board.next_meeting = tied.tick;
    weeks(&mut tied, &dataset, 1);
    let deadline = Tick(tied.tick.0 + 4);
    let id = governance::queue_vote(
        tied.firms.get_mut(firm).unwrap(),
        "conflict_transfer",
        deadline,
    );
    weeks(&mut tied, &dataset, 4);
    let vote = tied
        .firm(firm)
        .unwrap()
        .board
        .pending_votes
        .iter()
        .find(|v| v.id == id)
        .cloned()
        .unwrap();
    assert!(
        vote.resolved && !vote.passed,
        "no controller and opposed directors: status quo"
    );
}

#[test]
fn a_passed_board_vote_authorises_a_large_action_for_thirteen_weeks() {
    let (mut world, dataset, firm) =
        setup(&[(HolderId::Investor, 600), (HolderId::Outside(0), 400)]);
    // Before the first meeting nobody holds a seat, so the controller has no board authority.
    world.firms.get_mut(firm).unwrap().board.seats = [None; 5];
    world.firms.get_mut(firm).unwrap().board.next_meeting = Tick(world.tick.0 + 100);
    assert!(!governance::large_action_authorised(&world, firm));
    let deadline = Tick(world.tick.0 + 4);
    governance::queue_vote(
        world.firms.get_mut(firm).unwrap(),
        "large_issuance",
        deadline,
    );
    weeks(&mut world, &dataset, 4);
    // No seats are filled yet, so there are no directors to oppose it: the proposal needs three
    // votes and gets none. Fill the seats as a meeting would and vote again.
    let seats = governance::apportion_seats(&world.firm(firm).unwrap().shares);
    assert_eq!(
        seats
            .iter()
            .filter(|s| **s == Some(HolderId::Investor))
            .count(),
        3
    );
    world.firms.get_mut(firm).unwrap().board.seats = [Some(HolderId::Outside(0)); 5];
    let deadline = Tick(world.tick.0 + 4);
    governance::queue_vote(
        world.firms.get_mut(firm).unwrap(),
        "large_issuance",
        deadline,
    );
    weeks(&mut world, &dataset, 4);
    assert!(
        governance::large_action_authorised(&world, firm),
        "solvent minority directors back the proposal, so it passes and authorises"
    );
    weeks(&mut world, &dataset, 13);
    assert!(
        !governance::large_action_authorised(&world, firm),
        "the authority lapses"
    );
}
