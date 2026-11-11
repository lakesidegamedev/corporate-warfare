//! Careers (2.0): the helm start, the ambition, the ending and the saved state. Oracles: money
//! conservation of the helm purchase (the sellers' capital rises by exactly what the player paid),
//! the control rule (more than half of the eligible votes), a save and load that plays on
//! identically to an unbroken game, and a replay log that rebuilds the same world.

use cw_session::{PriceSetup, ScenarioConfig, Session};
use cw_sim::economy::career::{Ambition, CareerSetup, EndKind, Start};
use cw_sim::state::{HolderId, ScenarioKind, ScenarioMode};

fn config(seed: u64, mode: ScenarioMode) -> ScenarioConfig {
    ScenarioConfig {
        seed,
        scenario: ScenarioKind::Founder,
        mode,
        dataset_hash: String::new(),
        difficulty: cw_data::Difficulty::Standard,
        economy_start: Default::default(),
        starting_capital_cents: None,
        prices: PriceSetup {
            scenario_id: "great-lakes-2000".to_owned(),
            path: None,
            timeline: None,
        },
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

fn helm_setup(firm: u64, ambition: Ambition, years: u32) -> CareerSetup {
    CareerSetup {
        start: Start::Helm,
        helm_firm: Some(firm),
        ambition: Some(ambition),
        years,
    }
}

#[test]
fn a_game_without_a_career_is_unchanged() {
    let plain = Session::create(config(31, ScenarioMode::Sandbox)).unwrap();
    assert!(plain.world.career.is_none());
    assert!(plain.career_summary().is_none());
    let again = Session::create(config(31, ScenarioMode::Sandbox)).unwrap();
    assert_eq!(plain.canonical_hash(), again.canonical_hash());
}

#[test]
fn the_helm_offers_three_mid_table_companies_with_their_price() {
    let offers = Session::helm_offers(config(32, ScenarioMode::Sandbox)).unwrap();
    assert_eq!(offers.len(), 3);
    let industries: std::collections::BTreeSet<&str> =
        offers.iter().map(|o| o.industry.as_str()).collect();
    assert_eq!(industries.len(), 3, "different sectors where possible");
    for o in &offers {
        assert!((8..=15).contains(&o.rank));
        assert_eq!(o.price_cents, o.shares_needed * o.mark_cents);
        assert!(o.fortune_cents >= o.price_cents);
    }
}

#[test]
fn taking_the_helm_buys_control_from_outside_holders() {
    let cfg = config(33, ScenarioMode::Sandbox);
    let offer = Session::helm_offers(cfg.clone()).unwrap().remove(0);
    let before = Session::create(cfg.clone()).unwrap();
    let outside_before: i64 = before
        .world
        .outside
        .investors
        .iter()
        .map(|h| h.capital_cents)
        .sum();
    let session =
        Session::create_career(cfg, &helm_setup(offer.firm, Ambition::Biggest, 10)).unwrap();
    let world = &session.world;
    let outside_after: i64 = world
        .outside
        .investors
        .iter()
        .map(|h| h.capital_cents)
        .sum();
    assert_eq!(outside_after - outside_before, offer.price_cents);
    assert_eq!(
        world.investor.cash_cents,
        offer.fortune_cents - offer.price_cents
    );
    let firm = world.firm(cw_core::ids::Handle(offer.firm)).unwrap();
    let held = firm.shares.qty(HolderId::Investor);
    assert!(held * 2 > firm.shares.eligible_votes());
    assert!(firm.ceo.is_none(), "the sitting chief executive leaves");
    assert_eq!(world.investor.primary_group.map(|h| h.0), Some(offer.firm));
    let summary = session.career_summary().unwrap();
    assert_eq!(summary.start, "helm");
    assert_eq!(summary.ambition.as_deref(), Some("biggest"));
    assert!(!summary.ended);
}

#[test]
fn the_rival_ai_stops_playing_the_helm_company() {
    let cfg = config(34, ScenarioMode::Sandbox);
    let offer = Session::helm_offers(cfg.clone()).unwrap().remove(0);
    let session =
        Session::create_career(cfg, &helm_setup(offer.firm, Ambition::Survivor, 10)).unwrap();
    assert!(cw_ai::player_controlled(
        &session.world,
        cw_core::ids::Handle(offer.firm)
    ));
}

#[test]
fn a_helm_career_saves_loads_and_plays_on_identically() {
    let cfg = config(35, ScenarioMode::Sandbox);
    let offer = Session::helm_offers(cfg.clone()).unwrap().remove(1);
    let mut session =
        Session::create_career(cfg, &helm_setup(offer.firm, Ambition::Dominant, 5)).unwrap();
    run(&mut session, 60);
    let summary = session.career_summary().unwrap();
    assert_eq!(summary.years_log.len(), 1, "one year line after a year");
    let bytes = session.export_save().unwrap();
    let mut loaded = Session::import_save(&bytes).unwrap();
    assert_eq!(loaded.world.career, session.world.career);
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());
    run(&mut session, 20);
    run(&mut loaded, 20);
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());
}

#[test]
fn a_career_replays_from_its_log() {
    let cfg = config(36, ScenarioMode::Sandbox);
    let offer = Session::helm_offers(cfg.clone()).unwrap().remove(0);
    let mut session =
        Session::create_career(cfg, &helm_setup(offer.firm, Ambition::Biggest, 10)).unwrap();
    session.advance(8);
    let log: cw_session::ReplayLog = serde_json::from_str(&session.export_replay()).unwrap();
    assert!(log.career.is_some());
    assert_eq!(Session::replay(&log).unwrap(), session.canonical_hash());
}

#[test]
fn a_five_year_game_with_a_career_plays_five_years() {
    let mut session = Session::create_career(
        config(37, ScenarioMode::FiveYear),
        &CareerSetup {
            ambition: Some(Ambition::Survivor),
            ..CareerSetup::default()
        },
    )
    .unwrap();
    let state = session.world.career.clone().unwrap();
    assert_eq!(state.years, 5);
    assert_eq!(state.end_week, 260);
    // A founder who never founds and has spent their fortune loses after the grace.
    session.world.investor.cash_cents = 0;
    run(&mut session, 30);
    let summary = session.career_summary().unwrap();
    assert!(summary.ended && summary.lost);
    assert_eq!(summary.ending, EndKind::NoCompany.key());
    assert!(!summary.sentence.is_empty());
}

#[test]
fn a_pick_off_the_list_is_refused_in_plain_words() {
    let refused = Session::create_career(
        config(38, ScenarioMode::Sandbox),
        &helm_setup(u64::MAX, Ambition::Biggest, 10),
    );
    assert!(refused.is_err());
    let refused = Session::create_career(
        config(38, ScenarioMode::Sandbox),
        &CareerSetup {
            years: 7,
            ..CareerSetup::default()
        },
    );
    assert!(refused.is_err(), "lengths are 5, 10 or 20 years");
}

#[test]
fn the_board_writes_each_quarter_with_progress() {
    let cfg = config(39, ScenarioMode::Sandbox);
    let offer = Session::helm_offers(cfg.clone()).unwrap().remove(0);
    let mut session =
        Session::create_career(cfg, &helm_setup(offer.firm, Ambition::Biggest, 10)).unwrap();
    run(&mut session, 27);
    let letters = cw_session::letters::letters(&session.world, &session.dataset, 0);
    let progress: Vec<_> = letters
        .iter()
        .filter(|l| l.id.starts_with("career:"))
        .collect();
    assert_eq!(progress.len(), 2, "two quarters, two progress letters");
    assert!(
        progress
            .iter()
            .all(|l| l.paragraphs.iter().any(|f| f.contains("ranks")))
    );
}
