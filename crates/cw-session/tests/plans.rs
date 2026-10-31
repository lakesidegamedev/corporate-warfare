//! Rival long plans and workforce migration (1.9) through the session: the save keeps them and
//! the hash, a disclosed stake in your company reaches the paper, your letters and your brief,
//! and a long play with rivals planning stays deterministic and keeps every share accounted for.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_session::{ScenarioConfig, Session};
use cw_sim::economy::plans::{self, Plan, Stage};
use cw_sim::state::{HolderId, ScenarioKind, ScenarioMode};

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

fn firm(session: &Session, name: &str) -> Handle {
    session
        .world
        .firms
        .iter()
        .find(|(_, f)| f.name == name)
        .map(|(h, _)| h)
        .unwrap()
}

fn give(session: &mut Session, raider: Handle, target: Handle, bps: i64) {
    let f = session.world.firms.get_mut(target).unwrap();
    let want = f.shares.eligible_votes() * bps / 10_000;
    let have = f.shares.qty(HolderId::Firm(raider));
    let from = f
        .shares
        .holders
        .iter()
        .find(|(h, l)| matches!(h, HolderId::Outside(_)) && l.qty > want)
        .map(|(h, _)| *h)
        .unwrap();
    f.shares.transfer(
        from,
        HolderId::Firm(raider),
        want - have,
        (want - have) * 100,
    );
}

fn plan(session: &mut Session, raider: Handle, target: Handle) {
    use cw_core::ledger::{Account, Txn};
    let tick = session.world.tick;
    session
        .world
        .firms
        .get_mut(raider)
        .unwrap()
        .book
        .commit(
            tick,
            Txn::new()
                .debit(Account::Cash, 100_000_000_000, "fixture")
                .credit(Account::ShareCapital, 100_000_000_000, "fixture"),
        )
        .unwrap();
    let f = session.world.firms.get(target).unwrap();
    session.world.plans.plans.push(Plan {
        raider,
        target,
        temperament: 2,
        stage: Stage::Building,
        started: session.world.tick.0,
        stage_since: session.world.tick.0,
        entry_mark_cents: f.shares.mark_cents,
        entry_fair_cents: f.shares.fair_cents,
        base_held: 0,
        disclosed: None,
        deal: None,
        stake_bps: 0,
        reason: None,
        history: Vec::new(),
    });
}

#[test]
fn the_save_keeps_plans_and_migration_and_the_hash() {
    let mut session = Session::create(config(42)).unwrap();
    run(&mut session, 3);
    let raider = firm(&session, "Great Lakes Fiber");
    let target = firm(&session, "Cedar Bend Software");
    plan(&mut session, raider, target);
    give(&mut session, raider, target, 700);
    run(&mut session, 5);
    assert!(
        session
            .world
            .plans
            .plan_of(raider)
            .is_some_and(|p| p.disclosed.is_some())
    );
    let before = session.canonical_hash();
    let bytes = session.export_save().unwrap();
    assert!(
        bytes.windows(4).any(|w| w == b"PLAN"),
        "a saved trailer carries them"
    );
    let mut loaded = Session::import_save(&bytes).unwrap();
    assert_eq!(loaded.world.plans, session.world.plans);
    assert_eq!(loaded.canonical_hash(), before);
    run(&mut session, 10);
    run(&mut loaded, 10);
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());
    assert_eq!(loaded.world.plans, session.world.plans);
}

#[test]
fn a_save_without_the_trailer_loads_and_takes_the_migration_references_next_week() {
    let mut session = Session::create(config(7)).unwrap();
    assert!(session.world.plans.is_empty(), "nothing before play");
    let bytes = session.export_save().unwrap();
    assert!(!bytes.windows(4).any(|w| w == b"PLAN"));
    let mut loaded = Session::import_save(&bytes).unwrap();
    run(&mut loaded, 1);
    run(&mut session, 1);
    assert!(!loaded.world.plans.migration.reference_wage.is_empty());
    assert_eq!(loaded.canonical_hash(), session.canonical_hash());
}

#[test]
fn a_stake_in_your_company_reaches_the_paper_the_letters_and_the_brief() {
    let mut session = Session::create(config(42)).unwrap();
    run(&mut session, 30);
    let me = session.world.investor.primary_group.unwrap();
    let raider = firm(&session, "Great Lakes Fiber");
    plan(&mut session, raider, me);
    give(&mut session, raider, me, 520);
    let week = session.world.tick.0;
    plans::weekly(&mut session.world, &session.dataset);
    let paper = cw_session::paper::weekly_paper(&session.world, &session.dataset, week);
    let stories: Vec<_> = paper.lead.iter().chain(paper.briefs.iter()).collect();
    assert!(
        stories
            .iter()
            .any(|s| s.headline.contains("discloses a 5 percent stake")),
        "{:?}",
        stories.iter().map(|s| &s.headline).collect::<Vec<_>>()
    );
    let letters = cw_session::letters::letters(&session.world, &session.dataset, 1);
    let letter = letters
        .iter()
        .find(|l| l.id.starts_with("stake:"))
        .expect("a letter to you");
    assert!(letter.paragraphs.join(" ").contains("5 percent"));
    assert!(!letter.paragraphs.join(" ").contains('\u{2014}'));
    // Four weeks on and more bought: the brief warns.
    session.world.tick = Tick(week + 4);
    give(&mut session, raider, me, 640);
    plans::weekly(&mut session.world, &session.dataset);
    let brief = cw_session::brief::weekly_brief(&session.world, &session.dataset, me.0);
    let line = brief
        .changes
        .iter()
        .find(|c| c.kind == "stake")
        .expect("the brief warns when a disclosed stake grows");
    assert!(
        line.text
            .contains("6.4 percent of your company, up from 5.2 percent"),
        "{}",
        line.text
    );
    // Another rival's disclosed stake shows in its dossier line only from what is filed.
    let next = plans::likely_next_move(&session.world, raider).unwrap();
    assert!(next.contains("6 percent"), "{next}");
}

#[test]
fn a_long_play_with_rivals_planning_is_deterministic_and_loses_no_shares() {
    let mut a = Session::create(config(5)).unwrap();
    let mut b = Session::create(config(5)).unwrap();
    run(&mut a, 156);
    run(&mut b, 156);
    assert_eq!(a.canonical_hash(), b.canonical_hash());
    for (_, f) in a.world.firms.iter() {
        let held: i64 = f.shares.holders.values().map(|l| l.qty).sum();
        assert!(
            held <= f.shares.issued,
            "{} holds more than was issued",
            f.name
        );
    }
    eprintln!(
        "plans {:?} ended {}",
        a.world.plans.stats,
        a.world.plans.ended.len()
    );
}
