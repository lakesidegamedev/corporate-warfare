//! Takeover and competition-review evidence (R21, R22).
//!
//! Each fixture sets an explicit share register, explicit trailing sales and explicit reservation
//! factors, then checks the outcome against arithmetic worked out by hand from the rules in
//! docs/rules/CORPORATE.md, not against values read back from the engine.

use std::collections::BTreeMap;

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::economy::{deals, seed};
use cw_sim::state::{
    DealKind, DealState, HolderId, OutsideHolder, ScenarioKind, ScenarioMode, ShareBook, ShareLot,
    World,
};

const FAIR: i64 = 1_000;
const REGION: u8 = 0;
const PRODUCT: u16 = 7;

struct Fixture {
    world: World,
    buyer: Handle,
    target: Handle,
    others: Vec<Handle>,
}

/// A seeded world with every firm's market history cleared, three outside holders with known
/// capital and a 1,000-share target register held by those outside holders.
fn fixture(seed_value: u64, register: &[(HolderId, i64)]) -> Fixture {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(
        &dataset,
        seed_value,
        ScenarioKind::Founder,
        ScenarioMode::Sandbox,
    );
    let handles: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.operative())
        .map(|(handle, _)| handle)
        .collect();
    for (_, firm) in world.firms.iter_mut() {
        firm.sales_history.clear();
    }
    world.outside.investors = (0..4)
        .map(|index| OutsideHolder {
            name: format!("Fixture holder {index}"),
            capital_cents: 10_000_000_000,
            reservation_factor_bps: 10_000,
        })
        .collect();
    // Every firm gets its own dispersed outside controller, so no two fixture firms share a
    // competition group unless a test says so.
    let all: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for (index, handle) in all.iter().enumerate() {
        set_register(
            &mut world,
            *handle,
            &[(HolderId::Outside(1_000 + index as u32), 1_000)],
        );
    }
    let buyer = handles[0];
    let target = handles[1];
    set_register(&mut world, target, register);
    fund(&mut world, buyer, 5_000_000_000);
    Fixture {
        world,
        buyer,
        target,
        others: handles[2..].to_vec(),
    }
}

fn set_register(world: &mut World, firm: Handle, register: &[(HolderId, i64)]) {
    let holders: BTreeMap<HolderId, ShareLot> = register
        .iter()
        .map(|(holder, qty)| {
            (
                *holder,
                ShareLot {
                    qty: *qty,
                    cost_cents: qty * 500,
                },
            )
        })
        .collect();
    let shares = &mut world.firms.get_mut(firm).unwrap().shares;
    let (mark, fair) = (FAIR, FAIR);
    *shares = ShareBook {
        mark_cents: mark,
        fair_cents: fair,
        ..ShareBook::new(holders)
    };
}

fn fund(world: &mut World, firm: Handle, cents: i64) {
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .book
        .commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, cents, "fixture funding")
                .credit(Account::ShareCapital, cents, "fixture funding"),
        )
        .unwrap();
}

fn sell(world: &mut World, firm: Handle, units: i64) {
    let mut week = BTreeMap::new();
    week.insert((REGION, PRODUCT), units);
    world.firms.get_mut(firm).unwrap().sales_history = vec![week; 13];
}

fn weeks(world: &mut World, count: u32) {
    for _ in 0..count {
        world.tick = world.tick.next();
        deals::weekly(world);
    }
}

fn cash(world: &World, firm: Handle) -> i64 {
    world.firm(firm).unwrap().book.cash()
}

fn restricted(world: &World, firm: Handle) -> i64 {
    world
        .firm(firm)
        .unwrap()
        .book
        .balance(Account::RestrictedCash)
}

fn capital(world: &World, index: usize) -> i64 {
    world.outside.investors[index].capital_cents
}

fn qty(world: &World, firm: Handle, holder: HolderId) -> i64 {
    world.firm(firm).unwrap().shares.qty(holder)
}

#[test]
fn an_oversubscribed_tender_is_prorated_and_paid_exactly_from_escrow() {
    let mut f = fixture(
        901,
        &[
            (HolderId::Outside(0), 500),
            (HolderId::Outside(1), 300),
            (HolderId::Outside(2), 200),
        ],
    );
    let before = cash(&f.world, f.buyer);
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        1_200,
        600,
    )
    .unwrap();
    // Escrow 1,200 * 600 * 1.02 = 734,400; diligence 0.5 percent of 720,000 = 3,600.
    assert_eq!(f.world.deals.get(deal).unwrap().escrow_cents, 734_400);
    assert_eq!(cash(&f.world, f.buyer), before - 734_400 - 3_600);
    weeks(&mut f.world, 6);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Settled);
    // All 1,000 shares tender for 600 places: 500/300/200 scale to 300/180/120 exactly.
    assert_eq!(qty(&f.world, f.target, HolderId::Firm(f.buyer)), 600);
    assert_eq!(qty(&f.world, f.target, HolderId::Outside(0)), 200);
    assert_eq!(qty(&f.world, f.target, HolderId::Outside(1)), 120);
    assert_eq!(qty(&f.world, f.target, HolderId::Outside(2)), 80);
    assert_eq!(capital(&f.world, 0), 10_000_000_000 + 300 * 1_200);
    assert_eq!(capital(&f.world, 1), 10_000_000_000 + 180 * 1_200);
    assert_eq!(capital(&f.world, 2), 10_000_000_000 + 120 * 1_200);
    // Consideration 720,000 plus the 14,400 fee uses the whole escrow; nothing is refunded.
    assert_eq!(cash(&f.world, f.buyer), before - 734_400 - 3_600);
    assert_eq!(restricted(&f.world, f.buyer), 0);
    assert_eq!(
        f.world.firm(f.target).unwrap().shares.controller(),
        Some(HolderId::Firm(f.buyer))
    );
    assert!(f.world.firm(f.buyer).unwrap().book.is_balanced());
}

#[test]
fn a_ninety_percent_tender_squeezes_out_minorities_at_current_fair_value() {
    let mut f = fixture(
        902,
        &[
            (HolderId::Outside(0), 500),
            (HolderId::Outside(1), 420),
            (HolderId::Outside(2), 80),
        ],
    );
    // Holder 2 is loyal beyond the offer: 1,200 < 1,000 * 1.3.
    f.world.outside.investors[2].reservation_factor_bps = 13_000;
    let before = cash(&f.world, f.buyer);
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        1_200,
        1_000,
    )
    .unwrap();
    // Fair value rises after the bid; the frozen bid reference does not move, but the minority
    // is paid max(offer, current fair) = 1,500.
    f.world.firms.get_mut(f.target).unwrap().shares.fair_cents = 1_500;
    weeks(&mut f.world, 6);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Settled);
    assert_eq!(qty(&f.world, f.target, HolderId::Firm(f.buyer)), 1_000);
    assert_eq!(capital(&f.world, 2), 10_000_000_000 + 80 * 1_500);
    // Escrow 1,224,000 and diligence 6,000 out; tender 1,104,000, fee 22,080 and squeeze-out
    // 120,000 are the whole cost, so the buyer ends 1,252,080 lower.
    assert_eq!(
        cash(&f.world, f.buyer),
        before - 6_000 - 1_104_000 - 22_080 - 120_000
    );
    assert_eq!(restricted(&f.world, f.buyer), 0);
    assert!(f.world.firm(f.buyer).unwrap().book.is_balanced());
}

#[test]
fn an_offer_below_every_reservation_expires_with_escrow_back_less_diligence() {
    let mut f = fixture(
        903,
        &[(HolderId::Outside(0), 600), (HolderId::Outside(1), 400)],
    );
    let before = cash(&f.world, f.buyer);
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        900,
        1_000,
    )
    .unwrap();
    weeks(&mut f.world, 6);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Expired);
    assert_eq!(cash(&f.world, f.buyer), before - 4_500);
    assert_eq!(restricted(&f.world, f.buyer), 0);
    assert_eq!(qty(&f.world, f.target, HolderId::Outside(0)), 600);
}

#[test]
fn a_withdrawn_offer_returns_escrow_minus_incurred_diligence() {
    let mut f = fixture(904, &[(HolderId::Outside(0), 1_000)]);
    let before = cash(&f.world, f.buyer);
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        1_100,
        1_000,
    )
    .unwrap();
    weeks(&mut f.world, 3);
    deals::withdraw(&mut f.world, deal).unwrap();
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Withdrawn);
    assert_eq!(cash(&f.world, f.buyer), before - 5_500);
    assert_eq!(restricted(&f.world, f.buyer), 0);
    assert!(
        deals::withdraw(&mut f.world, deal).is_err(),
        "terminal deals stay closed"
    );
}

#[test]
fn a_deal_cannot_fund_diligence_on_top_of_an_exact_escrow() {
    let mut f = fixture(905, &[(HolderId::Outside(0), 1_000)]);
    let available = cash(&f.world, f.buyer);
    // Escrow alone would fit; escrow plus diligence does not.
    let shares = 1_000;
    let price = available * 100 / 102 / shares;
    assert!(deals::escrow_required(price, shares) <= available);
    let result = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        price,
        shares,
    );
    assert!(result.is_err());
    assert_eq!(cash(&f.world, f.buyer), available, "nothing posted");
}

#[test]
fn the_player_tender_moves_only_at_settlement() {
    let mut f = fixture(
        906,
        &[(HolderId::Investor, 300), (HolderId::Outside(0), 700)],
    );
    let investor_before = f.world.investor.cash_cents;
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        1_100,
        1_000,
    )
    .unwrap();
    deals::respond_tender(&mut f.world, deal, true).unwrap();
    assert_eq!(
        f.world.investor.cash_cents, investor_before,
        "no cash before settlement"
    );
    assert_eq!(qty(&f.world, f.target, HolderId::Investor), 300);
    weeks(&mut f.world, 6);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Settled);
    assert_eq!(f.world.investor.cash_cents, investor_before + 300 * 1_100);
    assert_eq!(qty(&f.world, f.target, HolderId::Investor), 0);
    // Cost basis 500/share: the realised gain is 600/share.
    assert!(f.world.investor.book.is_balanced());
    assert_eq!(restricted(&f.world, f.buyer), 0);
}

#[test]
fn a_player_who_declines_keeps_shares_and_blocks_control() {
    let mut f = fixture(
        907,
        &[(HolderId::Investor, 600), (HolderId::Outside(0), 400)],
    );
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        1_100,
        1_000,
    )
    .unwrap();
    weeks(&mut f.world, 6);
    // Only 400 of 1,000 shares tender, short of control.
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Expired);
    assert_eq!(qty(&f.world, f.target, HolderId::Investor), 600);
    assert_eq!(qty(&f.world, f.target, HolderId::Outside(0)), 400);
}

#[test]
fn concurrent_bids_conserve_shares_and_escrow() {
    let mut f = fixture(
        908,
        &[(HolderId::Outside(0), 550), (HolderId::Outside(1), 450)],
    );
    let rival = f.others[0];
    fund(&mut f.world, rival, 5_000_000_000);
    let first = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        1_200,
        1_000,
    )
    .unwrap();
    let second = deals::start(
        &mut f.world,
        rival,
        f.target,
        DealKind::SharePurchase,
        1_300,
        1_000,
    )
    .unwrap();
    weeks(&mut f.world, 6);
    let total: i64 = f
        .world
        .firm(f.target)
        .unwrap()
        .shares
        .holders
        .values()
        .map(|lot| lot.qty)
        .sum();
    assert_eq!(total, 1_000, "no share is sold twice");
    for deal in [first, second] {
        let stored = f.world.deals.get(deal).unwrap();
        assert!(
            stored.state.terminal(),
            "both bids resolve, {:?}",
            stored.state
        );
        assert_eq!(stored.escrow_cents, 0);
    }
    for firm in [f.buyer, rival] {
        assert_eq!(
            restricted(&f.world, firm),
            0,
            "no escrow is left or overdrawn"
        );
        assert!(f.world.firm(firm).unwrap().book.is_balanced());
    }
    let controller = f.world.firm(f.target).unwrap().shares.controller();
    assert!(
        controller == Some(HolderId::Firm(f.buyer)) || controller == Some(HolderId::Firm(rival)),
        "exactly one bidder controls"
    );
}

#[test]
fn review_happens_before_settlement_and_rejection_moves_nothing() {
    let mut f = fixture(909, &[(HolderId::Outside(0), 1_000)]);
    let neutral = f.others[0];
    sell(&mut f.world, f.buyer, 300);
    sell(&mut f.world, f.target, 300);
    sell(&mut f.world, neutral, 400);
    // Combined share 60 percent >= 35: review.
    let (share, hhi, delta) = deals::concentration(&f.world, REGION, PRODUCT, f.buyer, f.target);
    assert_eq!(share, 6_000);
    // Before: 900 + 900 + 1,600 = 3,400. After: 3,600 + 1,600 = 5,200.
    assert_eq!((hhi, delta), (5_200, 1_800));
    let before = cash(&f.world, f.buyer);
    let breaches = f.world.regulatory_breaches;
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        1_100,
        1_000,
    )
    .unwrap();
    assert!(f.world.deals.get(deal).unwrap().review_required);
    weeks(&mut f.world, 6);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Review);
    assert_eq!(
        qty(&f.world, f.target, HolderId::Outside(0)),
        1_000,
        "nothing moves during review"
    );
    // The buyer alone holds 30 percent, so a stake divestiture would clear; without one the
    // review rejects.
    weeks(&mut f.world, 4);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Rejected);
    assert_eq!(qty(&f.world, f.target, HolderId::Outside(0)), 1_000);
    assert_eq!(cash(&f.world, f.buyer), before - 5_500);
    assert_eq!(restricted(&f.world, f.buyer), 0);
    assert_eq!(f.world.regulatory_breaches, breaches + 1);
}

#[test]
fn a_feasible_remedy_settles_as_a_minority_stake() {
    let mut f = fixture(910, &[(HolderId::Outside(0), 1_000)]);
    let neutral = f.others[0];
    sell(&mut f.world, f.buyer, 200);
    sell(&mut f.world, f.target, 200);
    sell(&mut f.world, neutral, 600);
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        1_100,
        1_000,
    )
    .unwrap();
    weeks(&mut f.world, 6);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Review);
    deals::submit_remedy(&mut f.world, deal, 2_000).unwrap();
    weeks(&mut f.world, 4);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Settled);
    // Bought 1,000, divested down to 49 percent of 1,000.
    assert_eq!(qty(&f.world, f.target, HolderId::Firm(f.buyer)), 490);
    assert_ne!(
        f.world.firm(f.target).unwrap().shares.controller(),
        Some(HolderId::Firm(f.buyer))
    );
    assert!(
        !deals::competition_group(&f.world, f.buyer).contains(&f.target),
        "the target is outside the buyer group again"
    );
    assert!(f.world.firm(f.buyer).unwrap().book.is_balanced());
}

#[test]
fn an_infeasible_remedy_is_refused_and_the_deal_rejected() {
    let mut f = fixture(911, &[(HolderId::Outside(0), 1_000)]);
    let neutral = f.others[0];
    // The buyer alone already holds 40 percent: no divestiture of the target can clear it.
    sell(&mut f.world, f.buyer, 400);
    sell(&mut f.world, f.target, 100);
    sell(&mut f.world, neutral, 500);
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::SharePurchase,
        1_100,
        1_000,
    )
    .unwrap();
    weeks(&mut f.world, 6);
    assert!(deals::submit_remedy(&mut f.world, deal, 5_000).is_err());
    weeks(&mut f.world, 4);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Rejected);
    assert_eq!(qty(&f.world, f.target, HolderId::Outside(0)), 1_000);
}

#[test]
fn a_shell_subsidiary_cannot_bypass_review() {
    let mut f = fixture(912, &[(HolderId::Outside(0), 1_000)]);
    let parent = f.buyer;
    let shell = f.others[0];
    let neutral = f.others[1];
    // The shell has no sales of its own; its parent controls it and holds 30 percent.
    set_register(&mut f.world, shell, &[(HolderId::Firm(parent), 1_000)]);
    fund(&mut f.world, shell, 5_000_000_000);
    sell(&mut f.world, parent, 300);
    sell(&mut f.world, f.target, 100);
    sell(&mut f.world, neutral, 600);
    let (share, _, _) = deals::concentration(&f.world, REGION, PRODUCT, shell, f.target);
    assert_eq!(share, 4_000, "the parent's sales count for its shell");
    let deal = deals::start(
        &mut f.world,
        shell,
        f.target,
        DealKind::SharePurchase,
        1_100,
        1_000,
    )
    .unwrap();
    assert!(f.world.deals.get(deal).unwrap().review_required);
}

#[test]
fn neutral_concentration_and_hhi_only_triggers() {
    let mut f = fixture(913, &[(HolderId::Outside(0), 1_000)]);
    let rest = f.others.clone();
    // Ten equal sellers: combining two gives 20 percent, HHI 1,000 -> 1,200. No review.
    sell(&mut f.world, f.buyer, 100);
    sell(&mut f.world, f.target, 100);
    for firm in rest.iter().take(8) {
        sell(&mut f.world, *firm, 100);
    }
    assert_eq!(
        deals::concentration(&f.world, REGION, PRODUCT, f.buyer, f.target),
        (2_000, 1_200, 200)
    );
    // Shares 30/15/55: combined 45 >= 35 triggers on share alone.
    let mut g = fixture(914, &[(HolderId::Outside(0), 1_000)]);
    let neutral = g.others[0];
    sell(&mut g.world, g.buyer, 300);
    sell(&mut g.world, g.target, 150);
    sell(&mut g.world, neutral, 550);
    let (share, _, _) = deals::concentration(&g.world, REGION, PRODUCT, g.buyer, g.target);
    assert_eq!(share, 4_500);
    // Shares 30/4/66: combined 34 percent stays under the share trigger, but HHI goes
    // 900 + 16 + 4,356 = 5,272 -> 1,156 + 4,356 = 5,512, delta 240 > 200 with post > 2,500.
    let mut h = fixture(915, &[(HolderId::Outside(0), 1_000)]);
    let neutral = h.others[0];
    sell(&mut h.world, h.buyer, 300);
    sell(&mut h.world, h.target, 40);
    sell(&mut h.world, neutral, 660);
    assert_eq!(
        deals::concentration(&h.world, REGION, PRODUCT, h.buyer, h.target),
        (3_400, 5_512, 240)
    );
    let deal = deals::start(
        &mut h.world,
        h.buyer,
        h.target,
        DealKind::SharePurchase,
        1_100,
        1_000,
    )
    .unwrap();
    assert!(h.world.deals.get(deal).unwrap().review_required);
}

#[test]
fn a_legal_merger_pays_remaining_minorities_before_retiring_shares() {
    let mut f = fixture(
        916,
        &[(HolderId::Outside(0), 950), (HolderId::Outside(1), 50)],
    );
    f.world.outside.investors[1].reservation_factor_bps = 20_000;
    let deal = deals::start(
        &mut f.world,
        f.buyer,
        f.target,
        DealKind::Merger,
        1_100,
        950,
    )
    .unwrap();
    // 95 percent tenders, so settlement squeezes the 50-share minority out at 1,100.
    weeks(&mut f.world, 6);
    assert_eq!(f.world.deals.get(deal).unwrap().state, DealState::Settled);
    assert_eq!(qty(&f.world, f.target, HolderId::Firm(f.buyer)), 1_000);
    assert_eq!(capital(&f.world, 1), 10_000_000_000 + 50 * 1_100);
    assert_eq!(
        deals::minority_buyout_cost(&f.world, f.world.deals.get(deal).unwrap()),
        0
    );
}

#[test]
fn a_merger_after_open_market_buying_pays_the_minority_in_cash() {
    let mut f = fixture(
        917,
        &[(HolderId::Outside(0), 920), (HolderId::Outside(1), 80)],
    );
    // The buyer reached 92 percent outside a tender, so the merger itself must pay holder 1.
    set_register(
        &mut f.world,
        f.target,
        &[(HolderId::Firm(f.buyer), 920), (HolderId::Outside(1), 80)],
    );
    let deal = f.world.deals.insert(cw_sim::state::Deal {
        buyer: f.buyer,
        target: f.target,
        kind: DealKind::Merger,
        state: DealState::Settled,
        escrow_cents: 0,
        offer_price_cents: 900,
        max_shares: 0,
        committed_shares: 0,
        started: f.world.tick,
        diligence_until: f.world.tick,
        offer_deadline: f.world.tick,
        review_deadline: None,
        frozen_fair_cents: FAIR,
        review_required: false,
        remedy_share_bps: 0,
        hostile: false,
        diligence_paid: true,
        investor_tendered: false,
    });
    let before = cash(&f.world, f.buyer);
    let stored = f.world.deals.get(deal).unwrap().clone();
    // max(offer 900, fair 1,000) = 1,000 per share.
    assert_eq!(deals::minority_buyout_cost(&f.world, &stored), 80_000);
    deals::buy_out_minorities(&mut f.world, &stored).unwrap();
    assert_eq!(capital(&f.world, 1), 10_000_000_000 + 80_000);
    assert_eq!(cash(&f.world, f.buyer), before - 80_000);
    assert_eq!(qty(&f.world, f.target, HolderId::Firm(f.buyer)), 1_000);
    assert!(f.world.firm(f.buyer).unwrap().book.is_balanced());
}
