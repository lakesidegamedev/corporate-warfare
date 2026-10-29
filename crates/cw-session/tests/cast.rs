//! The character cast at the session layer: posts follow the chosen scenario, turnover and the
//! telemetry counter survive a save, and snapshots carry counts but no names.

use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::state::{PeriodClose, ScenarioKind, ScenarioMode};

fn config(seed: u64, scenario_id: &str) -> ScenarioConfig {
    ScenarioConfig {
        seed,
        scenario: ScenarioKind::Guided,
        mode: ScenarioMode::Sandbox,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: scenario_id.to_owned(),
            path: None,
            ..Default::default()
        },
    }
}

#[test]
fn the_scenarios_people_take_the_posts_and_the_same_seed_gives_the_same_cast() {
    let a = Session::create(config(31, "great-lakes-2000")).unwrap();
    let b = Session::create(config(31, "great-lakes-2000")).unwrap();
    assert_eq!(a.world.cast, b.world.cast);
    let chair = a.world.macro_state.cycle.chair.person.unwrap();
    assert_eq!(
        a.dataset.cast[chair as usize].scenario, "great-lakes-2000",
        "the chair is one of the scenario's own"
    );
    let named = a
        .world
        .firms
        .iter()
        .filter(|(_, f)| f.ceo.is_some())
        .count();
    assert!(named >= 3, "rivals have named chief executives: {named}");
}

#[test]
fn a_replaced_chief_executive_is_counted_and_survives_a_save() {
    let mut session = Session::create(config(32, "great-lakes-2000")).unwrap();
    let firm = session
        .world
        .firms
        .handles_sorted()
        .into_iter()
        .find(|h| session.world.firms.get(*h).is_some_and(|f| f.ceo.is_some()))
        .unwrap();
    let old = session.world.firms.get(firm).unwrap().ceo;
    // A long, losing record under a chief executive who has served over a year.
    session.world.cast.appointed.insert(firm, 0);
    let history: Vec<PeriodClose> = (0..20u32)
        .map(|i| PeriodClose {
            first_week: 100 + i * 4,
            weeks: 4,
            net_income: -5_000,
            ..Default::default()
        })
        .collect();
    session.world.firm_history.insert(firm, history);
    session.world.tick = cw_core::calendar::Tick(176);
    for _ in 0..80 {
        if session.world.cast.turnovers > 0 {
            break;
        }
        cw_sim::economy::cast::weekly(&mut session.world, &session.dataset);
        session.world.tick = cw_core::calendar::Tick(session.world.tick.0 + 1);
    }
    assert_eq!(session.world.cast.turnovers, 1);
    assert_ne!(session.world.firms.get(firm).unwrap().ceo, old);
    let snapshot = session.market_snapshot();
    assert_eq!(snapshot.cast.ceo_turnovers, 1);
    let json = serde_json::to_string(&snapshot).unwrap();
    for person in &session.dataset.cast {
        assert!(!json.contains(&person.name), "{} leaked", person.name);
    }
    let bytes = session.export_save().unwrap();
    let restored = Session::import_save(&bytes).unwrap();
    assert_eq!(session.canonical_hash(), restored.canonical_hash());
    assert_eq!(restored.world.cast.turnovers, 1);
    assert_eq!(
        restored.world.firms.get(firm).unwrap().ceo,
        session.world.firms.get(firm).unwrap().ceo
    );
}
