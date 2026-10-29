//! A pause always says why and always has a way out. Every scenario plays 156 weeks through the
//! same calls the apps make: advance, read the pause, answer the card it names or press Continue.
//! The clock never stays stopped without a card the player can answer.

use cw_data::{Difficulty, PricePath};
use cw_session::dto::PausePolicy;
use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent};
use cw_sim::state::{ScenarioKind, ScenarioMode};

fn config(seed: u64, scenario: &str) -> ScenarioConfig {
    ScenarioConfig {
        seed,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: scenario.to_owned(),
            path: Some(PricePath::HistoricalVariance),
            ..Default::default()
        },
    }
}

fn events(session: &Session) -> Vec<cw_session::dto::EventLine> {
    match session.query(cw_session::Query::Events) {
        cw_session::Detail::Events(rows) => rows,
        _ => Vec::new(),
    }
}

/// Answer card `event` the way the inbox does: try each choice until one is accepted.
fn answer(session: &mut Session, event: u64) -> bool {
    let choices: Vec<String> = events(session)
        .into_iter()
        .find(|line| line.id == event)
        .map(|line| line.choices.into_iter().map(|choice| choice.id).collect())
        .unwrap_or_default();
    for choice in choices {
        let accepted = session
            .submit(CommandEnvelope {
                command_id_high: 0xA11C,
                command_id_low: event,
                actor: 0,
                expected_revision: session.revision(),
                submitted_tick: session.world.tick.0,
                intent: CommandIntent::RespondEvent { event, choice },
            })
            .is_ok();
        if accepted {
            return true;
        }
    }
    false
}

fn play(scenario: &str, seed: u64, policy: PausePolicy) -> (u32, u32) {
    let mut session = Session::create(config(seed, scenario)).unwrap();
    session.set_pause_policy(policy);
    let (mut cards, mut continues) = (0, 0);
    let mut guard = 0;
    while session.world.tick.0 < 156 {
        guard += 1;
        assert!(guard < 2_000, "{scenario} {seed}: the clock stopped moving");
        let left = 156 - session.world.tick.0;
        let result = session.advance(left.min(16));
        let Some(reason) = result.pause else {
            assert!(
                session.world.pending_pause.is_none(),
                "{scenario} {seed}: a pause the advance did not report"
            );
            continue;
        };
        let info = session.pause_info().unwrap_or_else(|| {
            panic!(
                "{scenario} {seed} week {}: pause {reason} has no reason text",
                session.world.tick.0
            )
        });
        assert_eq!(info.reason, reason);
        assert!(!info.title.is_empty() && !info.detail.is_empty());
        assert_eq!(session.overview().pause_info.as_ref(), Some(&info));
        if reason == "five_year_score" {
            assert_eq!(info.kind, "milestone");
        }
        match info.action.as_str() {
            "answer" => {
                let id = info.event_id.expect("an answer pause names its card");
                assert!(
                    events(&session)
                        .iter()
                        .any(|line| line.id == id && !line.choices.is_empty()),
                    "{scenario} {seed} week {}: pause {reason} names a card the inbox cannot show",
                    session.world.tick.0
                );
                assert!(
                    answer(&mut session, id),
                    "{scenario} {seed} week {}: card {id} cannot be answered",
                    session.world.tick.0
                );
                // Answering always clears the pause.
                assert_ne!(
                    session.world.pending_pause.as_deref(),
                    Some(reason.as_str()),
                    "{scenario} {seed}: answered card {id} left the clock paused"
                );
                cards += 1;
            }
            "continue" => {
                session.dismiss_pause();
                assert!(
                    session.world.pending_pause.is_none(),
                    "{scenario} {seed}: Continue left the clock paused on {reason}"
                );
                continues += 1;
            }
            other => panic!("unknown pause action {other}"),
        }
    }
    (cards, continues)
}

#[test]
fn no_scenario_ever_stays_paused_without_an_answerable_card() {
    for scenario in [
        "great-lakes-2000",
        "texas-1981",
        "silicon-valley-1995",
        "ruhr-rhine-1991",
    ] {
        play(scenario, 11, PausePolicy::Decisions);
    }
}

/// Every mandatory card in the deck, forced open on the player's company, either retires because
/// it no longer applies or pauses with a card the player can answer, and answering clears it.
#[test]
fn every_mandatory_card_pauses_with_an_answer_and_answering_clears_it() {
    let mut session = Session::create(config(3, "great-lakes-2000")).unwrap();
    let firm = session.world.investor.primary_group.unwrap();
    let mandatory: Vec<String> = session
        .dataset
        .events
        .iter()
        .filter(|card| card.mandatory)
        .map(|card| card.id.clone())
        .collect();
    assert!(mandatory.len() >= 6);
    let mut paused = 0;
    for card in mandatory {
        session.world.events.cooldowns.clear();
        let id =
            cw_sim::economy::events_engine::open(&mut session.world, &card, Some(firm), None, true);
        assert!(id > 0, "{card} did not open");
        let result = session.advance(1);
        let Some(info) = session.pause_info() else {
            // Retired as stale: the card is gone from the inbox too.
            assert!(result.pause.is_none(), "{card}");
            assert!(events(&session).iter().all(|line| line.id != id), "{card}");
            continue;
        };
        paused += 1;
        assert_eq!(info.action, "answer", "{card}");
        let open = info.event_id.unwrap();
        assert!(
            events(&session).iter().any(|line| line.id == open),
            "{card}"
        );
        assert!(answer(&mut session, open), "{card} could not be answered");
        assert!(session.world.pending_pause.is_none(), "{card} left a pause");
        assert!(session.advance(1).weeks_advanced >= 1, "{card}");
        session.dismiss_pause();
    }
    assert!(paused > 0, "no forced card ever paused");
}

#[test]
fn warnings_pause_only_when_asked_and_always_have_a_continue() {
    let mut warnings = 0;
    for scenario in ["great-lakes-2000", "texas-1981"] {
        let (_, continues) = play(scenario, 21, PausePolicy::DecisionsAndWarnings);
        warnings += continues;
        // The default never stops for a warning, only for a card, a deadline or the score.
        let mut session = Session::create(config(21, scenario)).unwrap();
        while session.world.tick.0 < 156 {
            let result = session.advance(8);
            if let Some(reason) = result.pause {
                let info = session.pause_info().unwrap();
                assert_ne!(info.kind, "warning", "{reason} paused under decisions only");
                if info.action == "answer" {
                    assert!(answer(&mut session, info.event_id.unwrap()));
                } else {
                    session.dismiss_pause();
                }
            }
        }
    }
    let _ = warnings;
}

#[test]
fn a_card_pause_whose_card_is_gone_heals_instead_of_sticking() {
    let mut session = Session::create(config(5, "great-lakes-2000")).unwrap();
    session.world.pending_pause = Some("event:999999".to_owned());
    assert!(
        session.pause_info().is_none() || session.pause_info().unwrap().event_id == Some(999_999)
    );
    session.dismiss_pause();
    assert!(session.world.pending_pause.is_none());
    // A resolved card cannot hold the clock either.
    session.advance(40);
    if let Some(event) = session.world.events.pending.first_mut() {
        event.resolved = true;
        session.world.pending_pause = Some(format!("event:{}", event.id));
    }
    assert!(session.advance(1).weeks_advanced == 1);
}
