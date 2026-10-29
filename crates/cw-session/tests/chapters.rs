//! Chapters, board mandates and achievements (1.9). Each test states its oracle: a chapter is 52
//! weeks, the offer is three mandates, a pick is validated by the ordinary command path, and each
//! mandate kind resolves by the fact it names, built by hand.

use cw_core::error::DomainError;
use cw_core::ids::Handle;
use cw_session::{ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR};
use cw_sim::economy::chapters::{self, Mandate, MandateKind, Outcome};
use cw_sim::state::{HolderId, RaidRecord, ScenarioKind, ScenarioMode, ShareBook, ShareLot};
use std::collections::BTreeMap;

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

fn choose(
    session: &mut Session,
    actor: u64,
    firm: Handle,
    pick: u8,
    id: u64,
) -> Result<(), DomainError> {
    let envelope = CommandEnvelope {
        command_id_high: 0x19_0001,
        command_id_low: id,
        actor,
        expected_revision: session.world.revision,
        submitted_tick: session.world.tick.0,
        intent: CommandIntent::ChooseMandate { firm, pick },
    };
    session.submit(envelope).map(|_| ())
}

fn primary(session: &Session) -> Handle {
    session.world.investor.primary_group.unwrap()
}

/// Put a mandate in force for the primary company as if it had been picked in week 1, then jump
/// to the last week of the chapter so `weekly` settles it.
fn arm(session: &mut Session, kind: MandateKind) -> Handle {
    let firm = primary(session);
    session.world.chapters.offers.remove(&firm);
    session.world.chapters.active.insert(
        firm,
        Mandate {
            firm,
            chapter: 1,
            set: 1,
            due: 53,
            kind,
            met: false,
            broken: false,
        },
    );
    firm
}

fn settle(session: &mut Session) {
    session.world.tick = cw_core::calendar::Tick(53);
    let dataset = session.dataset.clone();
    chapters::weekly(&mut session.world, &dataset);
}

fn outcome_of_chapter_one(session: &Session) -> Option<Outcome> {
    session
        .world
        .chapters
        .log
        .iter()
        .find(|c| c.number == 1)
        .and_then(|c| c.outcome)
}

#[test]
fn the_board_offers_three_mandates_at_each_chapter_start() {
    let mut session = Session::create(config(7)).unwrap();
    assert!(
        session.world.chapters.is_empty(),
        "nothing before the first week"
    );
    run(&mut session, 1);
    let firm = primary(&session);
    let offer = session
        .world
        .chapters
        .offers
        .get(&firm)
        .expect("the player's board offers");
    assert_eq!(offer.chapter, 1);
    assert_eq!(offer.set, 1);
    assert_eq!(offer.kinds.len(), 3, "three distinct mandates");
    let keys: std::collections::BTreeSet<_> = offer.kinds.iter().map(|k| k.key()).collect();
    assert_eq!(keys.len(), 3);
    let log = &session.world.chapters.log[0];
    assert_eq!((log.number, log.start), (1, 1));
    assert!(log.name.starts_with("Chapter 1: "));
    // Rivals' boards offer too.
    assert!(session.world.chapters.offers.len() > 1);

    // Nothing new until week 53, the first week of chapter 2.
    run(&mut session, 51);
    assert_eq!(session.world.tick.0, 52);
    assert_eq!(session.world.chapters.chapter, 1);
    run(&mut session, 1);
    assert_eq!(session.world.chapters.chapter, 2);
    let offer = session
        .world
        .chapters
        .offers
        .get(&primary(&session))
        .expect("chapter 2 offer");
    assert_eq!((offer.chapter, offer.set), (2, 53));
    assert!(
        session
            .world
            .chapters
            .log
            .iter()
            .any(|c| c.number == 2 && c.name.starts_with("Chapter 2: "))
    );
    // The untaken chapter 1 offer lapsed without penalty (nothing was chosen).
    assert_eq!(outcome_of_chapter_one(&session), Some(Outcome::Lapsed));
}

#[test]
fn a_pick_is_validated_by_the_command_path() {
    let mut session = Session::create(config(8)).unwrap();
    run(&mut session, 1);
    let firm = primary(&session);
    // A pick outside the three is rejected and changes nothing.
    assert!(matches!(
        choose(&mut session, INVESTOR_ACTOR, firm, 3, 1),
        Err(DomainError::Arithmetic { .. })
    ));
    assert!(session.world.chapters.offers.contains_key(&firm));
    // Someone who does not run the company cannot pick for it.
    let rival = session
        .world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| *h != firm && session.world.chapters.offers.contains_key(h))
        .unwrap();
    assert!(matches!(
        choose(&mut session, rival.0, firm, 0, 2),
        Err(DomainError::NoAuthority { .. })
    ));
    // The player cannot pick for a rival's board either.
    assert!(matches!(
        choose(&mut session, INVESTOR_ACTOR, rival, 0, 3),
        Err(DomainError::NoAuthority { .. })
    ));
    // A valid pick is taken once.
    let kind = session.world.chapters.offers[&firm].kinds[1].clone();
    choose(&mut session, INVESTOR_ACTOR, firm, 1, 4).unwrap();
    assert_eq!(session.world.chapters.active[&firm].kind, kind);
    assert!(!session.world.chapters.offers.contains_key(&firm));
    assert_eq!(session.world.chapters.log[0].chosen, Some(1));
    assert!(matches!(
        choose(&mut session, INVESTOR_ACTOR, firm, 0, 5),
        Err(DomainError::WrongPhase { .. })
    ));
}

#[test]
fn an_offer_not_taken_in_four_weeks_is_closed() {
    let mut session = Session::create(config(9)).unwrap();
    run(&mut session, 5);
    let firm = primary(&session);
    // Week 5 is the offer's week 1 plus four: the window is weeks 1 to 4.
    assert!(matches!(
        choose(&mut session, INVESTOR_ACTOR, firm, 0, 1),
        Err(DomainError::WrongPhase { .. })
    ));
}

#[test]
fn market_share_profit_and_debt_resolve_on_their_measured_figure() {
    let mut met_cases = 0;
    for (name, met_kind, miss_kind) in {
        let probe = {
            let mut s = Session::create(config(11)).unwrap();
            run(&mut s, 6);
            s
        };
        let firm = primary(&probe);
        let division = cw_sim::economy::sectors::divisions_of(&probe.world, &probe.dataset, firm)
            .into_iter()
            .next()
            .unwrap();
        let share = cw_sim::economy::sectors::share_bps(
            &probe.world,
            &probe.dataset,
            firm,
            division.region,
            division.industry,
            0,
        );
        vec![
            (
                "share",
                MandateKind::MarketShare {
                    region: division.region,
                    industry: division.industry,
                    target_bps: share.max(1),
                },
                MandateKind::MarketShare {
                    region: division.region,
                    industry: division.industry,
                    target_bps: share + 5_000,
                },
            ),
            (
                "profit",
                MandateKind::ProfitGrowth {
                    baseline_cents: -1_000_000_000,
                    target_cents: -1_000_000_000,
                },
                MandateKind::ProfitGrowth {
                    baseline_cents: 0,
                    target_cents: 1_000_000_000_000,
                },
            ),
            (
                "debt",
                MandateKind::DebtToEquity {
                    target_bps: i64::MAX / 4,
                },
                MandateKind::DebtToEquity { target_bps: -1 },
            ),
        ]
    } {
        for (kind, want) in [(met_kind, Outcome::Met), (miss_kind, Outcome::Missed)] {
            let mut session = Session::create(config(11)).unwrap();
            run(&mut session, 6);
            let firm = arm(&mut session, kind);
            let before_rep = session.world.firms.get(firm).unwrap().reputation_bps;
            settle(&mut session);
            assert_eq!(outcome_of_chapter_one(&session), Some(want), "{name}");
            let after = session.world.firms.get(firm).unwrap().reputation_bps;
            let table = cw_data::chapters::mandate_table();
            let expected = if want == Outcome::Met {
                table.rewards.reputation_success_bps
            } else {
                table.rewards.reputation_failure_bps
            };
            assert_eq!(after - before_rep, expected, "{name} reputation");
            met_cases += (want == Outcome::Met) as u32;
        }
    }
    assert_eq!(met_cases, 3);
}

#[test]
fn a_met_mandate_pays_the_bonus_from_the_company_and_mints_nothing() {
    let mut session = Session::create(config(12)).unwrap();
    run(&mut session, 6);
    let firm = arm(&mut session, MandateKind::WinRaid);
    session.world.raids.push(RaidRecord {
        tick: 3,
        attacker: firm,
        defender: Handle(999_999),
        region: 0,
        industry: 0,
        budget_cents: 0,
        chance_bps: 5_000,
        success: true,
        blocks: 1,
        price_cents: 0,
    });
    let total =
        |s: &Session| s.world.investor.cash_cents + s.world.firms.get(firm).unwrap().book.cash();
    let (investor, company, sum) = (
        session_investor(&session),
        session.world.firms.get(firm).unwrap().book.cash(),
        total(&session),
    );
    settle(&mut session);
    let log = session
        .world
        .chapters
        .log
        .iter()
        .find(|c| c.number == 1)
        .unwrap()
        .clone();
    assert_eq!(log.outcome, Some(Outcome::Met));
    let min = cw_data::chapters::mandate_table().rewards.bonus_min_cents;
    assert!(
        log.bonus_cents >= min.min(company),
        "at least the minimum unless the purse is thinner"
    );
    assert!(
        log.bonus_cents <= company,
        "never more than the company's cash"
    );
    assert_eq!(session_investor(&session), investor + log.bonus_cents);
    assert_eq!(
        total(&session),
        sum,
        "the bonus moves money, it never mints it"
    );
}

fn session_investor(session: &Session) -> i64 {
    session.world.investor.cash_cents
}

#[test]
fn event_mandates_latch_and_cash_cover_breaks() {
    // Raid: lost without a win, met with one.
    let mut session = Session::create(config(13)).unwrap();
    run(&mut session, 6);
    arm(&mut session, MandateKind::WinRaid);
    settle(&mut session);
    assert_eq!(outcome_of_chapter_one(&session), Some(Outcome::Missed));

    // Integrate: a product one of the company's operating plants makes counts, one nothing makes does not.
    let mut session = Session::create(config(13)).unwrap();
    run(&mut session, 6);
    let firm = primary(&session);
    let plant = session.world.firms.get(firm).unwrap().facilities[0];
    let recipe = session.world.facilities.get(plant).unwrap().recipe;
    let made = session.dataset.product_index[&session.dataset.recipes[recipe as usize].output];
    arm(&mut session, MandateKind::Integrate { product: made });
    settle(&mut session);
    assert_eq!(outcome_of_chapter_one(&session), Some(Outcome::Met));

    let mut session = Session::create(config(13)).unwrap();
    run(&mut session, 6);
    arm(&mut session, MandateKind::Integrate { product: u16::MAX });
    settle(&mut session);
    assert_eq!(outcome_of_chapter_one(&session), Some(Outcome::Missed));

    // List a subsidiary: a controlled company that has listed since the start.
    let mut session = Session::create(config(13)).unwrap();
    run(&mut session, 6);
    let firm = arm(&mut session, MandateKind::ListSubsidiary { baseline: 0 });
    let child = session
        .world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| *h != firm)
        .unwrap();
    let mut holders = BTreeMap::new();
    holders.insert(
        HolderId::Firm(firm),
        ShareLot {
            qty: 1_000,
            cost_cents: 1_000,
        },
    );
    let entity = session.world.firms.get_mut(child).unwrap();
    entity.shares = ShareBook::new(holders);
    entity.shares.ipo_done = true;
    settle(&mut session);
    assert_eq!(outcome_of_chapter_one(&session), Some(Outcome::Met));

    // Cash cover: a one week line held, an impossible one broken the first week it is tested.
    for (weeks, want) in [(1u32, Outcome::Met), (100_000, Outcome::Missed)] {
        let mut session = Session::create(config(13)).unwrap();
        run(&mut session, 8);
        let firm = arm(&mut session, MandateKind::CashCover { cover_weeks: weeks });
        run(&mut session, 1);
        let broken = session.world.chapters.active.get(&firm).map(|m| m.broken);
        assert_eq!(broken, Some(weeks == 100_000));
        settle(&mut session);
        assert_eq!(outcome_of_chapter_one(&session), Some(want));
    }
}

#[test]
fn rivals_pick_through_the_same_command_and_deterministically() {
    let mut a = Session::create(config(21)).unwrap();
    let mut b = Session::create(config(21)).unwrap();
    run(&mut a, 3);
    b.advance(3);
    assert_eq!(a.canonical_hash(), b.canonical_hash());
    let primary_firm = primary(&a);
    let mut picked = 0;
    for (firm, mandate) in &a.world.chapters.active {
        assert_ne!(*firm, primary_firm, "the player picks for the player");
        assert!(mandate.set == 1 && mandate.chapter == 1);
        picked += 1;
    }
    assert!(picked > 0, "rival boards took a mandate");
    // The pick is the rule, not luck: the offer it came from was ranked by progress.
    let offers_left = a
        .world
        .chapters
        .offers
        .keys()
        .filter(|f| **f != primary_firm)
        .count();
    assert_eq!(
        offers_left, 0,
        "every rival offer was taken inside the window"
    );
}

#[test]
fn reload_in_a_chapter_keeps_the_hash_and_the_future() {
    let mut original = Session::create(config(31)).unwrap();
    run(&mut original, 10);
    let firm = primary(&original);
    let _ = choose(&mut original, INVESTOR_ACTOR, firm, 0, 1); // the window may have closed
    let bytes = original.export_save().unwrap();
    let mut loaded = Session::import_save(&bytes).unwrap();
    assert_eq!(original.canonical_hash(), loaded.canonical_hash());
    assert_eq!(original.world.chapters, loaded.world.chapters);
    run(&mut original, 60);
    run(&mut loaded, 60);
    assert_eq!(
        original.canonical_hash(),
        loaded.canonical_hash(),
        "across the chapter boundary"
    );
    assert_eq!(
        original.world.chapters.achievements,
        loaded.world.chapters.achievements
    );
}

#[test]
fn a_mid_chapter_mandate_survives_a_reload() {
    let mut original = Session::create(config(32)).unwrap();
    run(&mut original, 2);
    let firm = primary(&original);
    choose(&mut original, INVESTOR_ACTOR, firm, 0, 1).unwrap();
    run(&mut original, 10);
    let loaded = Session::import_save(&original.export_save().unwrap()).unwrap();
    assert_eq!(original.canonical_hash(), loaded.canonical_hash());
    assert_eq!(
        loaded.world.chapters.active[&firm],
        original.world.chapters.active[&firm]
    );
}

#[test]
fn achievements_are_earned_once_with_their_first_date() {
    let mut session = Session::create(config(41)).unwrap();
    run(&mut session, 3);
    let firm = primary(&session);
    assert!(session.world.chapters.achievements.is_empty());
    session.world.raids.push(RaidRecord {
        tick: 3,
        attacker: firm,
        defender: Handle(999_999),
        region: 0,
        industry: 0,
        budget_cents: 0,
        chance_bps: 5_000,
        success: true,
        blocks: 1,
        price_cents: 0,
    });
    run(&mut session, 1);
    let earned: Vec<_> = session
        .world
        .chapters
        .achievements
        .iter()
        .filter(|e| e.id == "first_raid")
        .collect();
    assert_eq!(earned.len(), 1);
    let week = earned[0].tick;
    assert_eq!(week, 4);
    run(&mut session, 5);
    let again: Vec<_> = session
        .world
        .chapters
        .achievements
        .iter()
        .filter(|e| e.id == "first_raid")
        .collect();
    assert_eq!((again.len(), again[0].tick), (1, week));

    // A thirty year career: set the clock, one weekly step earns it, and not before.
    assert!(
        !session
            .world
            .chapters
            .achievements
            .iter()
            .any(|e| e.id == "thirty_years")
    );
    let dataset = session.dataset.clone();
    session.world.tick = cw_core::calendar::Tick(1_559);
    cw_sim::economy::achievements::weekly(&mut session.world, &dataset);
    assert!(
        !session
            .world
            .chapters
            .achievements
            .iter()
            .any(|e| e.id == "thirty_years")
    );
    session.world.tick = cw_core::calendar::Tick(1_560);
    cw_sim::economy::achievements::weekly(&mut session.world, &dataset);
    assert!(
        session
            .world
            .chapters
            .achievements
            .iter()
            .any(|e| e.id == "thirty_years" && e.tick == 1_560)
    );
}

#[test]
fn a_leftover_quarterly_objective_from_an_older_save_is_retired_unpaid() {
    let mut session = Session::create(config(51)).unwrap();
    run(&mut session, 2);
    let firm = primary(&session);
    session.world.objective = Some(cw_sim::economy::objectives::Objective {
        firm,
        set: 1,
        due: 14,
        kind: cw_sim::economy::objectives::ObjectiveKind::NewRegion { baseline: 1 },
    });
    let cash = session.world.investor.cash_cents;
    run(&mut session, 1);
    assert!(session.world.objective.is_none());
    assert_eq!(
        session.world.investor.cash_cents, cash,
        "no bonus is paid for it"
    );
}

#[test]
fn two_missed_mandates_in_a_row_count_for_the_activist_trigger() {
    let mut session = Session::create(config(61)).unwrap();
    run(&mut session, 6);
    for chapter in 1..=2u32 {
        let firm = primary(&session);
        session.world.chapters.active.insert(
            firm,
            Mandate {
                firm,
                chapter,
                set: chapters::chapter_start(chapter),
                due: chapters::chapter_start(chapter + 1),
                kind: MandateKind::WinRaid,
                met: false,
                broken: false,
            },
        );
        session.world.tick = cw_core::calendar::Tick(chapters::chapter_start(chapter + 1));
        let dataset = session.dataset.clone();
        // Make sure the log has the chapter to record on.
        if !session
            .world
            .chapters
            .log
            .iter()
            .any(|c| c.number == chapter)
        {
            session.world.chapters.log.push(chapters::ChapterLog {
                number: chapter,
                name: String::new(),
                start: chapters::chapter_start(chapter),
                firm: Some(firm),
                offered: vec![MandateKind::WinRaid],
                chosen: Some(0),
                outcome: None,
                closed: 0,
                bonus_cents: 0,
                reputation_bps: 0,
            });
        }
        chapters::weekly(&mut session.world, &dataset);
    }
    let results: Vec<_> = session.world.chapters.results().collect();
    assert_eq!(
        &results[results.len() - 2..],
        &[Outcome::Missed, Outcome::Missed]
    );
}
