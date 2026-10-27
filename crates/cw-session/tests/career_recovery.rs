//! The investor survives losing control and can start again.

use cw_session::{ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent};
use cw_sim::state::{HolderId, ScenarioKind, ScenarioMode};

fn session() -> Session {
    Session::create(ScenarioConfig {
        seed: 601,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::FiveYear,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
    })
    .unwrap()
}

fn submit(
    session: &mut Session,
    intent: CommandIntent,
    nonce: u64,
) -> Result<(), cw_core::error::DomainError> {
    session
        .submit(CommandEnvelope {
            command_id_high: 11,
            command_id_low: nonce,
            actor: 0,
            expected_revision: session.revision(),
            submitted_tick: session.world.tick.0,
            intent,
        })
        .map(|_| ())
}

#[test]
fn losing_control_keeps_personal_cash_and_allows_a_new_firm() {
    let mut session = session();
    let firm = session.world.investor.primary_group.unwrap();
    let personal_cash = session.world.investor.cash_cents;
    assert!(personal_cash > 0);
    // The investor sells the controlling stake to an outside holder through the real market
    // ownership rules. Personal cash and career history are untouched.
    {
        let world = &mut session.world;
        let entity = world.firms.get_mut(firm).unwrap();
        let qty = entity.shares.qty(HolderId::Investor);
        entity
            .shares
            .transfer(HolderId::Investor, HolderId::Outside(3), qty, 0);
        world.investor.primary_group = None;
    }
    assert_eq!(
        session
            .world
            .firm(firm)
            .unwrap()
            .shares
            .qty(HolderId::Investor),
        0
    );
    assert_eq!(
        session.world.investor.cash_cents, personal_cash,
        "personal cash is not firm cash"
    );
    assert_eq!(session.world.investor.career_losses_cents, 0);

    // The investor can found a new company with personal capital.
    let capital = 50_000_000;
    let result = submit(
        &mut session,
        CommandIntent::FoundFirm {
            name: "Second Chance Holdings".to_owned(),
            region: 3,
            capital_cents: capital,
            recipe: 7,
            secondary: None,
        },
        1,
    );
    assert!(result.is_ok(), "new firm: {result:?}");
    let new_firm = session
        .world
        .investor
        .primary_group
        .expect("new primary firm");
    assert_ne!(new_firm, firm);
    assert_eq!(
        session
            .world
            .firm(new_firm)
            .unwrap()
            .shares
            .vote_bps(HolderId::Investor),
        10_000
    );
    assert_eq!(
        session.world.investor.cash_cents,
        personal_cash - capital - 2_500_000
    );
    assert_eq!(session.world.investor.primary_group, Some(new_firm));
}

#[test]
fn destitution_offers_one_management_contract_with_a_cooldown() {
    let mut session = session();
    let firm = session.world.investor.primary_group.unwrap();
    {
        let world = &mut session.world;
        world.investor.cash_cents = 0;
        world.investor.primary_group = None;
        let entity = world.firms.get_mut(firm).unwrap();
        entity.shares.holders.remove(&HolderId::Investor);
        entity.shares.holders.remove(&HolderId::Outside(4));
        entity
            .shares
            .add(HolderId::Outside(1), entity.shares.issued, 0);
        entity.board.ceo = None;
    }
    let before = session.world.investor.cash_cents;
    submit(&mut session, CommandIntent::AcceptManagementContract, 2).expect("contract offered");
    assert_eq!(session.world.investor.cash_cents, before + 10_000_000);
    assert!(
        session.world.investor.assisted_score,
        "assisted score is labelled"
    );
    assert_eq!(session.world.investor.recovery_used, 1);
    assert!(session.world.investor.primary_group.is_some());
    // A second contract inside 260 weeks is refused, even after losing the managed firm.
    session.world.investor.cash_cents = 0;
    session.world.investor.primary_group = None;
    session.world.investor.managed_firms.clear();
    let result = submit(&mut session, CommandIntent::AcceptManagementContract, 3);
    assert!(result.is_err(), "cooldown refuses a repeat");
}
