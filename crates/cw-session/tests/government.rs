//! Government laws, the reserve and systemic incidents at the session layer: alerts only when
//! something touches the investor's companies, telemetry fields, and a save that round-trips
//! the new state.

use cw_session::{ScenarioConfig, Session};
use cw_sim::economy::systemic::{self, Incident};
use cw_sim::state::{Rescue, ScenarioKind, ScenarioMode};

fn config(seed: u64) -> ScenarioConfig {
    ScenarioConfig {
        seed,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: Default::default(),
    }
}

fn run(session: &mut Session, weeks: u32) {
    for _ in 0..weeks {
        let r = session.advance(1);
        if r.pause.is_some() {
            for e in session.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            session.dismiss_pause();
        }
    }
}

fn texts(session: &Session) -> Vec<String> {
    session
        .overview()
        .alerts
        .into_iter()
        .map(|a| format!("{}|{}", a.key, a.subject))
        .collect()
}

#[test]
fn an_incident_alerts_the_investor_only_when_it_touches_their_company() {
    let mut session = Session::create(config(71)).unwrap();
    run(&mut session, 3);
    let mine = session.world.investor.primary_group.unwrap();
    let theirs = session
        .world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| {
            !cw_sim::economy::events_engine::player_side(&session.world, *h)
                && session.world.firms.get(*h).unwrap().operative()
        })
        .unwrap();
    let week = session.world.tick.0;
    session.world.systemic.incidents.push(Incident {
        week,
        kind: systemic::REFINERY_FIRE,
        a: 0,
        b: 0,
        mode: 0,
        weeks: 8,
        firms: vec![theirs],
        text: "Fire at a rival's plant.".to_owned(),
    });
    assert!(
        !texts(&session)
            .iter()
            .any(|t| t.starts_with("alert.incident")),
        "a rival's fire is not the investor's business"
    );
    session.world.systemic.incidents.push(Incident {
        week,
        kind: systemic::PORT_STRIKE,
        a: 0,
        b: 0,
        mode: 3,
        weeks: 3,
        firms: vec![mine],
        text: "Port strike at Toledo: ships stop for 3 weeks.".to_owned(),
    });
    assert!(
        texts(&session)
            .iter()
            .any(|t| t == "alert.incident|Port strike at Toledo: ships stop for 3 weeks.")
    );
    // Four weeks on, the alert is gone.
    run(&mut session, 5);
    assert!(
        !texts(&session)
            .iter()
            .any(|t| t.contains("Port strike at Toledo"))
    );
}

#[test]
fn a_rescue_law_alerts_only_a_strategic_investor_and_a_rescue_alerts_the_rescued() {
    let mut session = Session::create(config(72)).unwrap();
    run(&mut session, 2);
    let mine = session.world.investor.primary_group.unwrap();
    // Announce a nationalisation law: the investor's company is not strategic, so silence.
    let announced = session.world.tick.0;
    let mut change = session.world.politics.laws;
    change.rescue = Rescue::Nationalise;
    session.world.politics.pending = Some(cw_sim::state::LawChange {
        announced,
        effective: announced + 13,
        laws: change,
    });
    session.world.politics.laws.rescue = Rescue::Bailout;
    let strategic = cw_sim::economy::rescue::is_strategic(&session.world, &session.dataset, mine);
    let alerted = texts(&session)
        .iter()
        .any(|t| t.starts_with("alert.law_rescue"));
    assert_eq!(alerted, strategic);
    // A firm that was just rescued says so.
    session.world.firms.get_mut(mine).unwrap().rescued_week = session.world.tick.0;
    assert!(
        texts(&session)
            .iter()
            .any(|t| t.starts_with("alert.state_rescue"))
    );
}

#[test]
fn telemetry_carries_laws_reserve_and_incidents_without_names_and_the_state_saves() {
    let mut session = Session::create(config(73)).unwrap();
    run(&mut session, 30);
    session.world.systemic.counts[systemic::SPILL as usize] = 2;
    session.world.politics.reserve.bought_cents = 123;
    let snapshot = session.market_snapshot();
    let g = &snapshot.government;
    assert_eq!(snapshot.schema, 4);
    assert_eq!(g.party, "business");
    assert_eq!(g.rescue, "bailout");
    assert!(!g.common_carrier && g.quota_states > 0);
    assert_eq!(g.incident_kinds.len(), systemic::KINDS);
    assert_eq!(g.incidents[systemic::SPILL as usize], 2);
    assert_eq!(g.reserve_bought_cents, 123);
    let json = serde_json::to_string(&snapshot).unwrap();
    for (_, firm) in session.world.firms.iter() {
        assert!(!json.contains(&firm.name), "{} leaked", firm.name);
    }
    assert!(json.len() < 16_000, "{} bytes", json.len());
    // The new state saves and a loaded game continues exactly as the original.
    let bytes = session.export_save().unwrap();
    let mut restored = Session::import_save(&bytes).unwrap();
    assert_eq!(session.canonical_hash(), restored.canonical_hash());
    run(&mut session, 20);
    run(&mut restored, 20);
    assert_eq!(session.canonical_hash(), restored.canonical_hash());
}
