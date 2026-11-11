//! The tender card tells the player what tendering costs before it asks. Oracle: the shares and
//! votes are set by hand, so the expected text is plain arithmetic. 1,000,000 votes, the player
//! holds all of them, a bid takes 500,001, so 499,999 are left: 49.9999 percent, shown as 49.99.

use cw_core::ids::Handle;
use cw_session::{ScenarioConfig, Session};
use cw_sim::economy::events_engine;
use cw_sim::state::{
    Deal, DealKind, DealState, HolderId, ScenarioKind, ScenarioMode, ShareBook, ShareLot,
};
use std::collections::BTreeMap;

fn session() -> Session {
    Session::create(ScenarioConfig {
        seed: 41,
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

/// A bid by a rival for `max_shares` of the player's company, which the player holds
/// `player_shares` of out of 1,000,000 (the rest sits with an outside holder).
fn bid(session: &mut Session, player_shares: i64, max_shares: i64) -> (Handle, Handle) {
    let target = session.world.investor.primary_group.unwrap();
    let buyer = session
        .world
        .firms
        .iter()
        .map(|(h, _)| h)
        .find(|h| *h != target && !events_engine::player_side(&session.world, *h))
        .unwrap();
    let mut holders = BTreeMap::new();
    holders.insert(
        HolderId::Investor,
        ShareLot {
            qty: player_shares,
            cost_cents: 0,
        },
    );
    if player_shares < 1_000_000 {
        holders.insert(
            HolderId::Outside(0),
            ShareLot {
                qty: 1_000_000 - player_shares,
                cost_cents: 0,
            },
        );
    }
    let firm = session.world.firms.get_mut(target).unwrap();
    firm.shares = ShareBook::new(holders);
    firm.shares.fair_cents = 300;
    let tick = session.world.tick;
    let deal = session.world.deals.insert(Deal {
        buyer,
        target,
        kind: DealKind::SharePurchase,
        state: DealState::OfferOpen,
        escrow_cents: 0,
        offer_price_cents: 372,
        max_shares,
        committed_shares: 0,
        started: tick,
        diligence_until: tick,
        offer_deadline: cw_core::calendar::Tick(tick.0 + 4),
        review_deadline: None,
        frozen_fair_cents: 300,
        review_required: false,
        remedy_share_bps: 0,
        hostile: false,
        diligence_paid: true,
        investor_tendered: false,
    });
    events_engine::open_tender(&mut session.world, deal);
    (target, buyer)
}

fn tender_line(session: &Session) -> cw_session::dto::EventLine {
    cw_session::dto::events(&session.world, &session.dataset)
        .into_iter()
        .find(|e| e.card == "tender_offer")
        .expect("a tender card")
}

#[test]
fn a_tender_that_costs_control_puts_refusal_first_and_says_what_it_costs() {
    let mut s = session();
    let (target, _) = bid(&mut s, 1_000_000, 500_001);
    let name = s.world.firm(target).unwrap().name.clone();
    let line = tender_line(&s);
    assert_eq!(line.choices[0].id, "refuse", "{:?}", line.choices);
    assert_eq!(line.choices[1].id, "tender");
    let tender = &line.choices[1].label;
    assert!(
        tender.contains(&format!(
            "You would hold 49.99 percent and lose control of {name}"
        )),
        "{tender}"
    );
    // 372 against a fair 300 is 24 percent over.
    assert!(
        tender.contains("24.00 percent above fair value of $3.00"),
        "{tender}"
    );
    assert!(tender.contains("500,001 shares at $3.72"), "{tender}");
    assert!(
        line.choices[0]
            .label
            .contains(&format!("keep control of {name}"))
    );
    for choice in &line.choices {
        assert!(!choice.label.contains('\u{2014}') && !choice.label.contains('\u{2013}'));
    }
    // Both letters and the card list the refusal first.
    let letters = cw_session::letters::letters(&s.world, &s.dataset, 0);
    let letter = letters.iter().find(|l| l.id.starts_with("card:")).unwrap();
    assert_eq!(letter.choices[0].id, "refuse");
}

#[test]
fn a_tender_that_leaves_a_majority_is_unchanged() {
    let mut s = session();
    // 800,000 of 1,000,000 held; the bid takes at most 100,000, leaving 700,000.
    bid(&mut s, 800_000, 100_000);
    let line = tender_line(&s);
    assert_eq!(line.choices[0].id, "tender");
    assert_eq!(
        line.choices[0].label,
        "Tender held shares at the stated terms"
    );
}
