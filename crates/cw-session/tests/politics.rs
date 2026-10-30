//! Politics (1.9) through the session with rivals playing: they give and bid through the same
//! commands, tenders finish and pay the contractor from the treasury without running it dry, the
//! save keeps the politics state and the hash, and the paper, letters and brief report it.

use cw_session::{ScenarioConfig, Session};
use cw_sim::economy::civics::TenderStatus;
use cw_sim::state::{ScenarioKind, ScenarioMode};

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

fn run(session: &mut Session, weeks: u32) -> i64 {
    let mut low = i64::MAX;
    for _ in 0..weeks {
        let r = session.advance(1);
        if r.pause.is_some() {
            for e in session.world.events.pending.iter_mut() {
                e.resolved = true;
            }
            session.dismiss_pause();
        }
        low = low.min(session.world.government.book.cash());
    }
    low
}

#[test]
fn rivals_play_politics_and_the_treasury_stays_solvent() {
    let mut session = Session::create(config(31)).unwrap();
    let start = session.world.government.book.cash();
    let low = run(&mut session, 156);
    let c = &session.world.civics;
    let gifts = c.gifts.len();
    let awarded = c.tenders.iter().filter(|t| t.winner.is_some()).count();
    eprintln!(
        "gifts {gifts}, tenders opened {}, awarded {awarded}, done {}, paid {}, treasury {start} -> {} (low {low})",
        c.next_tender,
        c.tenders_done,
        c.tender_paid_cents,
        session.world.government.book.cash()
    );
    assert!(gifts > 0, "rivals give in the campaign windows");
    assert!(awarded > 0, "rivals bid and the best bid wins");
    assert!(low >= 0, "the treasury never goes negative");
    for t in &c.tenders {
        assert!(t.paid_cents <= t.price_cents, "never paid beyond the price");
        if t.status == TenderStatus::Done {
            assert!(t.paid_cents > 0);
        }
    }
    // What the treasury paid is a small share of what it collected.
    assert!(c.tender_paid_cents < session.world.government.book.cash().max(1));
}

#[test]
fn the_save_keeps_the_politics_state_and_the_hash() {
    let mut session = Session::create(config(32)).unwrap();
    run(&mut session, 60);
    assert!(!session.world.civics.is_empty());
    let hash = session.canonical_hash();
    let bytes = session.export_save().unwrap();
    assert!(bytes.windows(4).any(|w| w == b"POLI"));
    let restored = Session::import_save(&bytes).unwrap();
    assert_eq!(restored.canonical_hash(), hash);
    assert_eq!(restored.world.civics, session.world.civics);
}

#[test]
fn the_paper_and_brief_report_the_campaign() {
    let mut session = Session::create(config(33)).unwrap();
    run(&mut session, 40);
    let mut headlines: Vec<String> = Vec::new();
    for week in 2..=40 {
        let paper = cw_session::paper::weekly_paper(&session.world, &session.dataset, week);
        headlines.extend(paper.lead.iter().map(|s| s.headline.clone()));
        headlines.extend(paper.briefs.iter().map(|s| s.headline.clone()));
    }
    let joined = headlines.join("\n");
    assert!(
        joined.contains("leads the race")
            || joined.contains("debate")
            || joined.contains("tenders"),
        "politics reaches the paper: {joined}"
    );
}
