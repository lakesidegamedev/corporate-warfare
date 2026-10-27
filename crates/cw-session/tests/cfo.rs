//! The CFO speaks only when the company is losing money, names the biggest reasons, and each
//! button's commands remove the reason.

use cw_session::cfo::{MAX_ADVICE, advice};
use cw_session::{ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR};
use cw_sim::state::{ScenarioKind, ScenarioMode};

fn session() -> Session {
    Session::create(ScenarioConfig {
        seed: 7,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    })
    .unwrap()
}

fn send(session: &mut Session, low: u64, intent: CommandIntent) {
    let envelope = CommandEnvelope {
        command_id_high: 0xCF0,
        command_id_low: low,
        actor: INVESTOR_ACTOR,
        expected_revision: session.revision(),
        submitted_tick: session.world.tick.0,
        intent,
    };
    session.submit(envelope).unwrap();
}

#[test]
fn the_cfo_is_quiet_while_the_company_makes_money() {
    let mut session = session();
    let firm = session.world.investor.primary_group.unwrap().0;
    session.advance(3);
    assert!(advice(&session.world, &session.dataset, firm).is_empty());
}

#[test]
fn heavy_spending_in_the_red_is_named_and_the_button_stops_it() {
    let mut session = session();
    let firm = session.world.investor.primary_group.unwrap();
    for (low, intent) in [
        CommandIntent::SetResearchBudget {
            firm,
            amount_cents: 6_000_000,
        },
        CommandIntent::SetMarketingBudget {
            firm,
            amount_cents: 3_000_000,
        },
        CommandIntent::SetTrainingBudget {
            firm,
            amount_cents: 2_500_000,
        },
    ]
    .into_iter()
    .enumerate()
    {
        send(&mut session, low as u64, intent);
    }
    session.advance(5);
    session.dismiss_pause();
    let items = advice(&session.world, &session.dataset, firm.0);
    assert!(!items.is_empty() && items.len() <= MAX_ADVICE);
    assert_eq!(items[0].key, "research", "the biggest bill comes first");
    assert!(
        items
            .windows(2)
            .all(|pair| pair[0].weekly_cents >= pair[1].weekly_cents)
    );
    let research = &items[0];
    assert_eq!(research.weekly_cents, 6_000_000);
    for (index, command) in research.commands.iter().enumerate() {
        let intent: CommandIntent = serde_json::from_str(command).expect("a valid command");
        send(&mut session, 100 + index as u64, intent);
    }
    assert_eq!(
        session
            .world
            .firm(firm)
            .unwrap()
            .policies
            .research_budget_cents,
        0
    );
    assert!(
        advice(&session.world, &session.dataset, firm.0)
            .iter()
            .all(|item| item.key != "research")
    );
}
