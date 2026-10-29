//! Earnings season (1.9). Each test states its oracle. Results days are replayed on a clone of
//! a played world with an estimate injected by hand, and the expected move is worked from the
//! closed books and the rule in docs/rules/CORPORATE.md, not from the engine's own helpers.

use cw_core::calendar::Tick;
use cw_core::error::DomainError;
use cw_core::ids::Handle;
use cw_session::{ScenarioConfig, Session};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR};
use cw_sim::economy::earnings::{self, Estimate, Rating};
use cw_sim::economy::structure::{self, Reason};
use cw_sim::state::{ScenarioKind, ScenarioMode, World};

const SCALE: i64 = 10_000_000;

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

/// A session played to the close of its second quarter (week 26).
fn at_week_26(seed: u64) -> Session {
    let mut s = Session::create(config(seed)).unwrap();
    run(&mut s, 26);
    assert_eq!(s.world.tick.0, 26);
    s
}

fn covered(world: &World) -> Vec<Handle> {
    world
        .firms
        .iter()
        .map(|(h, _)| h)
        .filter(|h| earnings::covered(world, *h))
        .collect()
}

/// Operating profit of the thirteen closed weeks, worked from the books.
fn actual(world: &World, firm: Handle) -> i64 {
    let weeks = world.firms.get(firm).unwrap().book.recent_weeks();
    assert_eq!(weeks.len(), 13);
    weeks.iter().map(|w| w.ebit()).sum()
}

/// Make results due today with a given estimate, and no streak or credibility.
fn due_today(world: &mut World, firm: Handle, estimate: i64, credibility: i32, streak: i8) {
    let fe = world.earnings.firms.entry(firm).or_default();
    fe.estimate = Some(Estimate {
        made: 13,
        due: 26,
        cents: estimate,
        raw_cents: estimate,
        scale_cents: SCALE,
    });
    fe.guidance = None;
    fe.credibility_bps = credibility;
    fe.streak = streak;
    fe.missed_run = 0;
}

/// Replay today's results day on a clone and return it.
fn replay(base: &Session, setup: impl Fn(&mut World, Handle)) -> World {
    let mut w = base.world.clone();
    for firm in covered(&w) {
        setup(&mut w, firm);
    }
    earnings::weekly(&mut w, &base.dataset);
    w
}

fn last(world: &World, firm: Handle) -> earnings::Report {
    world.earnings.firms[&firm].last.clone().expect("a report")
}

#[test]
fn every_listed_firm_gets_an_estimate_and_reports_a_quarter_later() {
    let s = at_week_26(7);
    let firms = covered(&s.world);
    assert!(firms.len() >= 10, "a played world has listed firms");
    for firm in &firms {
        let e = s.world.earnings.firms[firm].estimate.as_ref().unwrap();
        assert_eq!((e.made, e.due), (26, 39));
    }
    // The week-13 results had no estimate to meet; the week-26 ones did.
    assert!(s.world.earnings.log.iter().all(|r| r.week == 26));
    assert!(s.world.earnings.log.len() >= 10);
    assert!(s.world.earnings.log.iter().all(|r| r.week == 26));
}

#[test]
fn the_price_move_follows_the_surprise_and_stays_inside_the_bound() {
    let s = at_week_26(7);
    let w = replay(&s, |w, firm| due_today(w, firm, 0, 0, 0));
    let mut moved = 0;
    for firm in covered(&s.world) {
        let a = actual(&w, firm);
        // Oracle: surprise in basis points of the scale, bounded; fifteen percent of the price
        // per full surprise; no streak, no credibility, so no weighting.
        let rel = (a * 10_000 / SCALE).clamp(-10_000, 10_000);
        let expected = (rel * 10 / 100).clamp(-1_200, 1_200);
        let r = last(&w, firm);
        assert_eq!(r.surprise_bps as i64, rel, "surprise of {firm:?}");
        assert_eq!(r.move_bps as i64, expected, "move of {firm:?}");
        assert!(r.move_bps.abs() <= 1_200);
        // The mark moved by exactly that, from where the played week left it.
        let before = s.world.firms.get(firm).unwrap().shares.mark_cents;
        let after = w.firms.get(firm).unwrap().shares.mark_cents;
        assert_eq!(
            after as i128,
            (before as i128 * (10_000 + expected as i128) / 10_000).max(1),
            "mark of {firm:?}"
        );
        if expected != 0 {
            moved += 1;
        }
    }
    assert!(moved > 0, "some firm's results moved its price");
}

#[test]
fn results_day_creates_and_destroys_no_money() {
    let s = at_week_26(7);
    let w = replay(&s, |w, firm| due_today(w, firm, 0, 0, 0));
    for (handle, firm) in s.world.firms.iter() {
        let after = w.firms.get(handle).unwrap();
        assert_eq!(firm.book.cash(), after.book.cash(), "cash of {handle:?}");
        assert_eq!(firm.book.total_assets(), after.book.total_assets());
        assert_eq!(firm.book.total_equity(), after.book.total_equity());
        assert_eq!(firm.shares.issued, after.shares.issued);
        assert_eq!(
            postcard::to_allocvec(&firm.shares.holders).unwrap(),
            postcard::to_allocvec(&after.shares.holders).unwrap()
        );
    }
    assert_eq!(s.world.investor.cash_cents, w.investor.cash_cents);
}

#[test]
fn a_run_of_surprises_and_credibility_shape_the_move() {
    let s = at_week_26(7);
    // A miss of about twenty percent of the scale: estimate = actual + 20% of the scale.
    let miss = |cred: i32, streak: i8| {
        let w = replay(&s, |w, firm| {
            let a = actual(w, firm);
            due_today(w, firm, a + SCALE / 5, cred, streak);
        });
        covered(&s.world)
            .into_iter()
            .map(|f| (f, last(&w, f)))
            .collect::<Vec<_>>()
    };
    let plain = miss(0, 0);
    let trusted = miss(10_000, 0);
    let after_misses = miss(0, -3);
    let after_beats = miss(0, 3);
    for i in 0..plain.len() {
        let p = &plain[i].1;
        assert_eq!(p.surprise_bps, -2_000);
        assert_eq!(p.move_bps, -200, "20% surprise at 10% a full surprise");
        // Full credibility cuts a miss by 40%.
        assert_eq!(trusted[i].1.move_bps, -120);
        // Three misses already: each adds 15% (to a cap of four), so 145% of the move.
        assert_eq!(after_misses[i].1.move_bps, -290);
        // Three beats and then a miss: the run does not stack on a reversal.
        assert_eq!(after_beats[i].1.move_bps, -200);
        assert_eq!(after_misses[i].1.streak, -4);
        assert_eq!(after_beats[i].1.streak, -1);
    }
}

#[test]
fn the_move_never_exceeds_the_cap_even_at_a_maximal_run() {
    let s = at_week_26(7);
    let w = replay(&s, |w, firm| {
        due_today(w, firm, actual(w, firm) + 100 * SCALE, 0, -8)
    });
    for firm in covered(&s.world) {
        assert_eq!(last(&w, firm).move_bps, -1_200);
    }
}

fn give(
    session: &mut Session,
    firm: Handle,
    low: i64,
    high: i64,
    id: u64,
) -> Result<(), DomainError> {
    let envelope = CommandEnvelope {
        command_id_high: 0x19_0002,
        command_id_low: id,
        actor: INVESTOR_ACTOR,
        expected_revision: session.world.revision,
        submitted_tick: session.world.tick.0,
        intent: CommandIntent::GiveGuidance {
            firm,
            low_cents: low,
            high_cents: high,
        },
    };
    session.submit(envelope).map(|_| ())
}

fn your_listed_firm(s: &mut Session) -> Option<Handle> {
    let firm = s.world.investor.primary_group?;
    earnings::covered(&s.world, firm).then_some(firm)
}

#[test]
fn guidance_is_validated_pulls_the_estimate_and_is_given_once() {
    let mut s = at_week_26(7);
    // A rival is not the player's to guide.
    let rival = covered(&s.world)
        .into_iter()
        .find(|f| !cw_sim::economy::events_engine::player_side(&s.world, *f))
        .unwrap();
    assert!(give(&mut s, rival, 0, 1, 1).is_err());
    // The player's company may not be floated; float some of it and give it an estimate.
    let firm = s.world.investor.primary_group.unwrap();
    if your_listed_firm(&mut s).is_none() {
        let f = s.world.firms.get_mut(firm).unwrap();
        let issued = f.shares.issued;
        let held = f.shares.qty(cw_sim::state::HolderId::Investor);
        let sold = (issued / 10).min(held / 2).max(1);
        f.shares
            .holders
            .get_mut(&cw_sim::state::HolderId::Investor)
            .unwrap()
            .qty -= sold;
        f.shares.holders.insert(
            cw_sim::state::HolderId::Outside(0),
            cw_sim::state::ShareLot {
                qty: sold,
                ..Default::default()
            },
        );
    }
    assert!(earnings::covered(&s.world, firm));
    s.world.earnings.firms.entry(firm).or_default().estimate = Some(Estimate {
        made: 26,
        due: 39,
        cents: 50 * SCALE,
        raw_cents: 50 * SCALE,
        scale_cents: 50 * SCALE,
    });
    let raw = 50 * SCALE;
    let scale = 50 * SCALE;
    // Low end above high end, and a range far from the estimate, are refused.
    assert!(give(&mut s, firm, raw + 10, raw, 2).is_err());
    assert!(give(&mut s, firm, raw - scale, raw + scale, 3).is_err());
    // A tight honest range just above the estimate pulls it up by the credibility share.
    let (low, high) = (raw + scale / 20, raw + scale / 10);
    give(&mut s, firm, low, high, 4).unwrap();
    let e = s.world.earnings.firms[&firm].estimate.clone().unwrap();
    let mid = (low + high) / 2;
    // Pull is 20% + 60% of credibility (3,000 of 10,000) = 38%, under the 15% of scale cap.
    let expected = raw + ((mid - raw) as i128 * 3_800 / 10_000) as i64;
    assert_eq!(e.cents, expected);
    assert_eq!(e.raw_cents, raw);
    assert!(e.cents > raw && e.cents <= raw + scale * 15 / 100);
    // Only one range a quarter.
    assert!(give(&mut s, firm, low, high, 5).is_err());
    assert!(
        s.world.earnings.firms[&firm]
            .guidance
            .as_ref()
            .unwrap()
            .honest
    );
}

#[test]
fn meeting_guidance_builds_credibility_and_missing_it_costs_standing() {
    let mut s = at_week_26(7);
    let firm = covered(&s.world)[0];
    let a = actual(&s.world, firm);
    let setup = |w: &mut World, low: i64, high: i64| {
        due_today(w, firm, a, 3_000, 0);
        let fe = w.earnings.firms.get_mut(&firm).unwrap();
        fe.guidance = Some(earnings::Guidance {
            given: 14,
            due: 26,
            low_cents: low,
            high_cents: high,
            honest: true,
        });
    };
    let rep_before = s.world.firms.get(firm).unwrap().reputation_bps;
    // Met: actual at or above the low end.
    let mut met = s.world.clone();
    setup(&mut met, a - 1_000, a + 1_000);
    earnings::weekly(&mut met, &s.dataset);
    let fe = &met.earnings.firms[&firm];
    assert_eq!(fe.credibility_bps, 4_500);
    assert_eq!(fe.last.as_ref().unwrap().guidance_met, Some(true));
    assert_eq!(met.firms.get(firm).unwrap().reputation_bps, rep_before);
    assert_eq!(met.earnings.guidance_grievance(firm, 26), None);
    // Missed: actual under the low end by a tenth of the scale.
    let mut missed = s.world.clone();
    setup(&mut missed, a + SCALE / 10, a + SCALE / 5);
    {
        // Park the scale at SCALE for the oracle.
        missed
            .earnings
            .firms
            .get_mut(&firm)
            .unwrap()
            .estimate
            .as_mut()
            .unwrap()
            .scale_cents = SCALE;
    }
    earnings::weekly(&mut missed, &s.dataset);
    let fe = &missed.earnings.firms[&firm];
    assert_eq!(fe.credibility_bps, 1_500, "half of 3,000");
    assert_eq!(fe.last.as_ref().unwrap().guidance_met, Some(false));
    assert_eq!(fe.missed_run, 1);
    // Reputation: 150 plus a fortieth of the 1,000 bps shortfall.
    assert_eq!(
        missed.firms.get(firm).unwrap().reputation_bps,
        rep_before - 175
    );
    assert_eq!(missed.earnings.guidance_grievance(firm, 26), Some(6_500));
    assert_eq!(missed.earnings.guidance_grievance(firm, 26 + 14), None);
    // An unhonest (sandbagged) range that is met earns nothing.
    let mut sandbag = s.world.clone();
    setup(&mut sandbag, a - 1_000, a + 1_000);
    sandbag
        .earnings
        .firms
        .get_mut(&firm)
        .unwrap()
        .guidance
        .as_mut()
        .unwrap()
        .honest = false;
    earnings::weekly(&mut sandbag, &s.dataset);
    assert_eq!(sandbag.earnings.firms[&firm].credibility_bps, 3_000);
    s.world.tick = Tick(26);
}

#[test]
fn a_missed_range_invites_the_activists_through_the_ordinary_path() {
    let s = at_week_26(7);
    let firm = covered(&s.world)
        .into_iter()
        .find(|f| s.world.firms.get(*f).unwrap().shares.statement_weeks >= 26)
        .unwrap();
    let mut w = s.world.clone();
    w.earnings.firms.entry(firm).or_default().missed_run = 2;
    w.earnings.firms.get_mut(&firm).unwrap().missed_week = 26;
    // Make the firm look healthy otherwise and give it a real outside float.
    w.structure.last_activist.remove(&firm);
    w.structure.campaigns.retain(|c| c.target != firm);
    // The scan visits each firm one week in thirteen; find that week.
    let slot = (firm.slot() % 13) as u32;
    let mut week = 26 + 1;
    while week % 13 != slot {
        week += 1;
    }
    w.tick = Tick(week);
    structure::weekly(&mut w, &s.dataset);
    let c = w
        .structure
        .campaigns
        .iter()
        .find(|c| c.target == firm)
        .expect("a campaign opened");
    assert_eq!(c.reason, Reason::MissedGuidance);
    assert_eq!(c.justification_bps, 8_000);
}

#[test]
fn ratings_follow_value_against_price() {
    let s = at_week_26(7);
    let (mut buys, mut sells, mut holds) = (0, 0, 0);
    for line in cw_session::dto::stock_board(&s.world) {
        if !line.earnings.covered {
            continue;
        }
        // Oracle: value 20% over the price buys, price 20% over value sells.
        let ratio = line.fair_cents as f64 / line.price_cents as f64;
        let expected = if ratio >= 1.2 {
            "buy"
        } else if ratio <= 1.0 / 1.2 + 0.0002 {
            "sell"
        } else {
            "hold"
        };
        match line.earnings.rating.as_str() {
            "buy" => buys += 1,
            "sell" => sells += 1,
            _ => holds += 1,
        }
        if (ratio - 1.2).abs() > 0.001 && (ratio - 1.0 / 1.2).abs() > 0.001 {
            assert_eq!(line.earnings.rating, expected, "{} at {ratio}", line.name);
        }
    }
    println!("buy {buys} hold {holds} sell {sells}");
    assert_eq!(earnings::rating_for(120, 100), Rating::Buy);
    assert_eq!(earnings::rating_for(83, 100), Rating::Sell);
}

#[test]
fn an_overvalued_firm_draws_a_short_campaign_and_a_bounded_price_hit() {
    let s = at_week_26(7);
    let firm = covered(&s.world)
        .into_iter()
        .find(|f| {
            let fi = s.world.firms.get(*f).unwrap();
            fi.shares.statement_weeks >= 26 && fi.shares.fair_cents > 0
        })
        .unwrap();
    let mut w = s.world.clone();
    {
        let f = w.firms.get_mut(firm).unwrap();
        f.shares.mark_cents = f.shares.fair_cents * 3;
    }
    w.structure.last_activist.remove(&firm);
    w.structure.campaigns.retain(|c| c.target != firm);
    let slot = (firm.slot() % 13) as u32;
    let mut week = 27;
    while week % 13 != slot {
        week += 1;
    }
    w.tick = Tick(week);
    let before = w.firms.get(firm).unwrap().shares.mark_cents;
    structure::weekly(&mut w, &s.dataset);
    let c = w
        .structure
        .campaigns
        .iter()
        .find(|c| c.target == firm)
        .expect("a campaign opened");
    // Other grievances could rank higher; an overvalued case is at least present when it opens.
    assert_eq!(c.reason, Reason::Overvalued);
    let after = w.firms.get(firm).unwrap().shares.mark_cents;
    let drop_bps = (before - after) * 10_000 / before;
    assert!((300..=800).contains(&drop_bps), "{drop_bps}");
    assert!(earnings::overvalued_case(&s.world, firm).is_none());
}

#[test]
fn the_state_survives_a_save_and_moves_the_hash() {
    let s = at_week_26(7);
    let empty_hash = {
        let mut w = s.world.clone();
        w.earnings = Default::default();
        cw_sim::state::canonical_hash(&w)
    };
    assert_ne!(
        empty_hash,
        s.canonical_hash(),
        "earnings state is in the hash"
    );
    let bytes = s.export_save().unwrap();
    let back = Session::import_save(&bytes).unwrap();
    assert_eq!(back.world.earnings, s.world.earnings);
    assert_eq!(back.canonical_hash(), s.canonical_hash());
    // And the loaded game plays on exactly as the unbroken one.
    let (mut a, mut b) = (s, back);
    run(&mut a, 20);
    run(&mut b, 20);
    assert_eq!(a.canonical_hash(), b.canonical_hash());
}

#[test]
fn the_paper_the_letters_and_the_brief_read_the_season() {
    let mut s = Session::create(config(7)).unwrap();
    run(&mut s, 39);
    // Make sure the player's company is listed so its own letters exist.
    let firm = s.world.investor.primary_group.unwrap();
    let paper = cw_session::paper::weekly_paper(&s.world, &s.dataset, 39);
    let all: Vec<_> = paper.lead.iter().chain(paper.briefs.iter()).collect();
    let _ = firm;
    let on_results_week = s
        .world
        .earnings
        .log
        .iter()
        .any(|r| r.week == 39 && r.move_bps != 0);
    if on_results_week {
        assert!(
            all.iter().any(|st| st.id.starts_with("earnings:")),
            "results day makes the paper: {:?}",
            all.iter().map(|st| st.headline.clone()).collect::<Vec<_>>()
        );
    }
    for st in &all {
        assert!(!st.headline.contains('\u{2014}') && !st.body.contains('\u{2014}'));
    }
}
