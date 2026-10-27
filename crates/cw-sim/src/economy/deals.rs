//! Mergers, hostile takeovers and competition review.
//!
//! DRAFT -> FUNDED -> DILIGENCE -> OFFER_OPEN -> REVIEW -> SETTLED, with WITHDRAWN, EXPIRED and
//! REJECTED as terminal alternatives. No path skips escrow or review, and review happens before
//! any share or asset moves, so a rejected deal leaves the target exactly as it was. The buyer
//! escrows the full maximum cash price plus the two percent transaction fee; diligence costs half
//! a percent of bid value and lasts two weeks; the offer stays open four weeks. Tendered shares
//! above the bid cap are accepted pro rata. Control needs more than half the votes after the
//! tender; squeeze-out needs ninety percent and pays every remaining minority holder in cash at
//! max(offer, fair value).
//!
//! Antitrust: HHI = sum((group regional product share * 100)^2), with groups taken at the
//! ultimate controller so a shell subsidiary cannot hide its parent's sales. Review triggers
//! when the combined share is at least 35 percent, or post-HHI exceeds 2500 with a delta above
//! 200. Review lasts four weeks. A feasible divestiture remedy clears it; an infeasible remedy
//! rejects the deal rather than inventing regulator discretion.

use std::collections::{BTreeMap, BTreeSet};

use cw_core::calendar::Tick;
use cw_core::error::{ArithmeticFault, DomainError, DomainResult, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::round_even_div;

use crate::state::{Deal, DealKind, DealState, HolderId, World};

const DILIGENCE_WEEKS: u32 = 2;
const OFFER_WEEKS: u32 = 4;
const REVIEW_WEEKS: u32 = 4;
const TRANSACTION_FEE_BPS: i128 = 200;
const SQUEEZE_OUT_BPS: i128 = 9_000;
pub const SHARE_REVIEW_BPS: i64 = 3_500;
pub const HHI_REVIEW: i64 = 2_500;
pub const HHI_DELTA: i64 = 200;

pub fn escrow_required(max_price_cents: i64, shares: i64) -> i64 {
    let gross = max_price_cents.saturating_mul(shares);
    gross + gross * 2 / 100
}

fn transaction_fee(consideration: i64) -> i64 {
    round_even_div(consideration as i128 * TRANSACTION_FEE_BPS, 10_000) as i64
}

fn diligence_fee(price_per_share: i64, max_shares: i64) -> i64 {
    round_even_div(price_per_share as i128 * max_shares as i128 * 50, 10_000) as i64
}

/// Start a deal: funds the escrow and pays the diligence cost.
pub fn start(
    world: &mut World,
    buyer: Handle,
    target: Handle,
    kind: DealKind,
    price_per_share: i64,
    max_shares: i64,
) -> DomainResult<Handle> {
    if price_per_share <= 0 || max_shares <= 0 {
        return Err(DomainError::Arithmetic {
            detail: ArithmeticFault::NegativeQuantity,
        });
    }
    if !world.firms.contains(buyer) {
        return Err(DomainError::InvalidEntity {
            handle: buyer.0,
            kind: EntityKind::Firm,
        });
    }
    let (target_fair, target_mark, target_tradable) = match world.firms.get(target) {
        Some(firm) => (
            firm.shares.fair_cents,
            firm.shares.mark_cents,
            firm.status.can_trade(),
        ),
        None => {
            return Err(DomainError::InvalidEntity {
                handle: target.0,
                kind: EntityKind::Firm,
            });
        }
    };
    if !target_tradable {
        return Err(DomainError::SolvencyRestriction {
            entity: target.0,
            reason: cw_core::error::SolvencyReason::PastDue,
        });
    }
    if buyer == target || crate::economy::ownership::would_create_cycle(world, buyer, target) {
        return Err(DomainError::OwnershipCycle {
            buyer: buyer.0,
            target: target.0,
        });
    }
    let escrow = escrow_required(price_per_share, max_shares);
    let fee = diligence_fee(price_per_share, max_shares);
    let required = escrow.saturating_add(fee);
    let cash = world.firms.get(buyer).map(|f| f.book.cash()).unwrap_or(0);
    if cash < required {
        return Err(DomainError::InsufficientCash {
            entity: buyer.0,
            required,
            available: cash,
        });
    }
    if let Some(bidder) = world.firms.get_mut(buyer) {
        let _ = bidder.book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, escrow, "deal escrow")
                .debit(Account::RestrictedCash, escrow, "deal escrow"),
        );
        if fee > 0 {
            let _ = bidder.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, fee, "diligence fee")
                    .debit(Account::OperatingExpense, fee, "diligence fee"),
            );
        }
    }
    if fee > 0 {
        let _ = world.outside.legal_services_book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, fee, "diligence fee")
                .credit(Account::Revenue, fee, "diligence fee"),
        );
    }
    let hostile = !crate::economy::ownership::same_group(world, buyer, target)
        && price_per_share <= target_mark.max(target_fair);
    let mut deal = Deal {
        buyer,
        target,
        kind,
        state: DealState::Diligence,
        escrow_cents: escrow,
        offer_price_cents: price_per_share,
        max_shares,
        committed_shares: 0,
        started: world.tick,
        diligence_until: Tick(world.tick.0 + DILIGENCE_WEEKS),
        offer_deadline: Tick(world.tick.0 + DILIGENCE_WEEKS + OFFER_WEEKS),
        review_deadline: None,
        frozen_fair_cents: target_fair,
        review_required: false,
        remedy_share_bps: 0,
        hostile,
        diligence_paid: fee > 0,
        investor_tendered: false,
    };
    deal.review_required = review_triggered(world, buyer, target);
    Ok(world.deals.insert(deal))
}

fn triggers(review_bps: i64, share: i64, hhi: i64, delta: i64) -> bool {
    share >= review_bps || (hhi > HHI_REVIEW && delta > HHI_DELTA)
}

fn review_triggered(world: &World, buyer: Handle, target: Handle) -> bool {
    let (share, hhi, delta) = trigger_metrics(world, buyer, target);
    triggers(world.politics.laws.merger_review_bps, share, hhi, delta)
        || vertical_share_bps(world, buyer, target) >= vertical_threshold(world)
}

fn vertical_threshold(world: &World) -> i64 {
    match world.politics.laws.vertical_review_bps {
        0 => i64::MAX,
        bps => bps,
    }
}

/// Vertical integration: the largest share of a regional market held by the target in a good
/// the buyer's plants use as an input, or by the buyer in a good the target's plants use. Buying
/// your own supplier or customer is reviewed when that share is high enough to foreclose rivals.
/// Zero when the two are not linked, or when the law does not review vertical deals.
pub fn vertical_share_bps(world: &World, buyer: Handle, target: Handle) -> i64 {
    if world.politics.laws.vertical_review_bps == 0 {
        return 0;
    }
    // The largest share the maker holds in a regional market for a good the user's plants use.
    let supplier = |maker: Handle, user: Handle| -> i64 {
        let (Some(maker_firm), Some(user_firm)) = (world.firm(maker), world.firm(user)) else {
            return 0;
        };
        let regions: BTreeSet<(u8, u16)> = maker_firm
            .sales_history
            .iter()
            .flat_map(|week| week.keys().copied())
            .filter(|(_, product)| user_firm.inputs_used.contains(product))
            .collect();
        let key = group_key(world, maker);
        let mut worst = 0i64;
        for (region, product) in regions {
            let volumes = group_volumes(world, region, product);
            let total: i64 = volumes.values().sum();
            if total > 0 {
                let own = volumes.get(&key).copied().unwrap_or(0);
                worst = worst.max((own as i128 * 10_000 / total as i128) as i64);
            }
        }
        worst
    };
    supplier(target, buyer).max(supplier(buyer, target))
}

/// Advance every deal at its declared boundary.
pub fn weekly(world: &mut World) {
    let handles: Vec<Handle> = world.deals.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let Some(deal) = world.deals.get(handle).cloned() else {
            continue;
        };
        if deal.state.terminal() {
            continue;
        }
        match deal.state {
            DealState::Funded | DealState::Draft => {
                if let Some(stored) = world.deals.get_mut(handle) {
                    stored.state = DealState::Diligence;
                    stored.diligence_until = Tick(world.tick.0 + DILIGENCE_WEEKS);
                }
            }
            DealState::Diligence => {
                if world.tick.0 >= deal.diligence_until.0 {
                    let review = review_triggered(world, deal.buyer, deal.target);
                    if let Some(stored) = world.deals.get_mut(handle) {
                        stored.state = DealState::OfferOpen;
                        stored.offer_deadline = Tick(world.tick.0 + OFFER_WEEKS);
                        stored.review_required = review;
                    }
                }
            }
            DealState::OfferOpen => {
                collect_tenders(world, handle, &deal);
                if world.tick.0 >= deal.offer_deadline.0 {
                    let updated = world.deals.get(handle).cloned().unwrap_or(deal);
                    close_offer(world, handle, &updated);
                }
            }
            DealState::Review => {
                let deadline = deal.review_deadline.map(|tick| tick.0).unwrap_or(u32::MAX);
                if world.tick.0 >= deadline {
                    finish_review(world, handle, &deal);
                }
            }
            _ => {}
        }
    }
}

/// The offer window has closed: expire, queue review, or settle.
fn close_offer(world: &mut World, handle: Handle, deal: &Deal) {
    if deal.kind != DealKind::AssetPurchase && !reaches_control(world, deal) {
        expire(world, handle, deal);
        return;
    }
    let review = deal.review_required || review_triggered(world, deal.buyer, deal.target);
    if review {
        if let Some(stored) = world.deals.get_mut(handle) {
            stored.state = DealState::Review;
            stored.review_deadline = Some(Tick(world.tick.0 + REVIEW_WEEKS));
            stored.review_required = true;
        }
        return;
    }
    settle(world, handle, deal, false);
}

/// Review has run its four weeks. Nothing has moved yet, so rejection is a clean refund.
fn finish_review(world: &mut World, handle: Handle, deal: &Deal) {
    if !review_triggered(world, deal.buyer, deal.target) {
        settle(world, handle, deal, false);
    } else if deal.remedy_share_bps > 0
        && deal.kind != DealKind::AssetPurchase
        && remedy_feasible(world, deal)
    {
        settle(world, handle, deal, true);
    } else {
        reject(world, handle, deal);
    }
}

fn settle(world: &mut World, handle: Handle, deal: &Deal, remedy: bool) {
    if deal.kind == DealKind::AssetPurchase {
        settle_asset_purchase(world, handle, deal);
    } else {
        settle_tender(world, handle, deal, remedy);
    }
}

fn reaches_control(world: &World, deal: &Deal) -> bool {
    let Some(target) = world.firm(deal.target) else {
        return false;
    };
    let eligible = target.shares.eligible_votes();
    let bought: i64 = tender_plan(world, deal).iter().map(|(_, qty)| *qty).sum();
    let held = target.shares.qty(HolderId::Firm(deal.buyer));
    eligible > 0 && (held + bought) * 2 > eligible
}

fn expire(world: &mut World, handle: Handle, deal: &Deal) {
    refund_escrow(world, handle, deal, "deal expired");
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.state = DealState::Expired;
    }
}

/// A remedy is a real divestiture: the buyer sells enough target shares to an outside holder to
/// fall below control, so the target's sales stop counting inside the buyer's group.
fn divest_control(world: &mut World, deal: &Deal) -> bool {
    let Some(target) = world.firm(deal.target) else {
        return false;
    };
    let eligible = target.shares.eligible_votes();
    let held = target.shares.qty(HolderId::Firm(deal.buyer));
    let keep = eligible * 49 / 100;
    let sell = held.saturating_sub(keep);
    if sell <= 0 {
        return false;
    }
    let price = target.shares.fair_cents.max(1);
    let proceeds = sell.saturating_mul(price);
    let basis = target
        .shares
        .holders
        .get(&HolderId::Firm(deal.buyer))
        .map(|lot| {
            if lot.qty > 0 {
                lot.cost_cents * sell / lot.qty
            } else {
                0
            }
        })
        .unwrap_or(0);
    let Some(index) = world
        .outside
        .investors
        .iter()
        .position(|holder| holder.capital_cents >= proceeds)
    else {
        return false;
    };
    world.outside.investors[index].capital_cents -= proceeds;
    if let Some(bidder) = world.firms.get_mut(deal.buyer) {
        let _ = bidder.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, proceeds, "divestiture")
                .credit(Account::Investments, basis, "divestiture")
                .credit(Account::OtherIncome, proceeds - basis, "divestiture"),
        );
    }
    if let Some(target) = world.firms.get_mut(deal.target) {
        target.shares.transfer(
            HolderId::Firm(deal.buyer),
            HolderId::Outside(index as u32),
            sell,
            proceeds,
        );
    }
    true
}

/// The highest firm in a chain of firm controllers.
fn top_firm(world: &World, firm: Handle) -> Handle {
    let mut current = firm;
    let mut seen = BTreeSet::new();
    while seen.insert(current) {
        match world.firm(current).and_then(|f| f.shares.controller()) {
            Some(HolderId::Firm(parent)) if world.firms.contains(parent) => current = parent,
            _ => break,
        }
    }
    current
}

/// The competition group key: the ultimate non-firm controller when there is one, otherwise the
/// top firm of the chain. Every firm the investor or one outside holder controls is one group.
fn group_key(world: &World, firm: Handle) -> HolderId {
    let top = top_firm(world, firm);
    match world.firm(top).and_then(|f| f.shares.controller()) {
        Some(holder @ (HolderId::Investor | HolderId::Outside(_))) => holder,
        _ => HolderId::Firm(top),
    }
}

/// Every firm in the same competition group as `firm`.
pub fn competition_group(world: &World, firm: Handle) -> BTreeSet<Handle> {
    let key = group_key(world, firm);
    world
        .firms
        .iter()
        .map(|(handle, _)| handle)
        .filter(|handle| group_key(world, *handle) == key)
        .collect()
}

/// Every (region, product) market the target has sold into over the trailing window.
fn target_markets(world: &World, target: Handle) -> BTreeSet<(u8, u16)> {
    world
        .firms
        .get(target)
        .map(|firm| {
            firm.sales_history
                .iter()
                .flat_map(|week| week.keys().copied())
                .collect()
        })
        .unwrap_or_default()
}

fn trigger_metrics(world: &World, buyer: Handle, target: Handle) -> (i64, i64, i64) {
    let mut worst = (0i64, 0i64, 0i64);
    for (region, product) in target_markets(world, target) {
        let (share, hhi, delta) = concentration(world, region, product, buyer, target);
        if share > worst.0 {
            worst.0 = share;
        }
        if hhi > worst.1 {
            worst.1 = hhi;
            worst.2 = delta;
        }
    }
    worst
}

/// Trailing thirteen-week unit volume per competition group in one regional product market.
fn group_volumes(world: &World, region: u8, product: u16) -> BTreeMap<HolderId, i64> {
    let mut volumes: BTreeMap<HolderId, i64> = BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        let volume = firm.trailing_units(region, product);
        if volume > 0 {
            *volumes.entry(group_key(world, handle)).or_insert(0) += volume;
        }
    }
    volumes
}

fn hhi_term(volume: i64, total: i64) -> i64 {
    let share = volume as i128 * 10_000 / total as i128;
    (share * share / 10_000) as i64
}

/// Regional product concentration if the buyer's and target's groups combine, using trailing
/// thirteen-week unit sales and ultimate-controller group aggregation. Returns the combined
/// share in basis points, the post-deal HHI and the HHI delta.
pub fn concentration(
    world: &World,
    region: u8,
    product: u16,
    buyer: Handle,
    target: Handle,
) -> (i64, i64, i64) {
    let volumes = group_volumes(world, region, product);
    let total: i64 = volumes.values().sum();
    if total <= 0 {
        return (0, 0, 0);
    }
    let buyer_key = group_key(world, buyer);
    let target_key = group_key(world, target);
    let before: i64 = volumes.values().map(|v| hhi_term(*v, total)).sum();
    let combined = volumes.get(&buyer_key).copied().unwrap_or(0)
        + if target_key == buyer_key {
            0
        } else {
            volumes.get(&target_key).copied().unwrap_or(0)
        };
    let after: i64 = volumes
        .iter()
        .filter(|(key, _)| **key != buyer_key && **key != target_key)
        .map(|(_, v)| hhi_term(*v, total))
        .sum::<i64>()
        + hhi_term(combined, total);
    let combined_share = (combined as i128 * 10_000 / total as i128) as i64;
    (combined_share, after, after - before)
}

/// The remedy divests control of the target, so it clears review only if the buyer's group on
/// its own stays under both triggers in every market the target serves, and a finite outside
/// holder can actually fund the divested block.
fn remedy_feasible(world: &World, deal: &Deal) -> bool {
    let buyer_key = group_key(world, deal.buyer);
    for (region, product) in target_markets(world, deal.target) {
        let volumes = group_volumes(world, region, product);
        let total: i64 = volumes.values().sum();
        if total <= 0 {
            continue;
        }
        let own = volumes.get(&buyer_key).copied().unwrap_or(0);
        if (own as i128 * 10_000 / total as i128) as i64 >= world.politics.laws.merger_review_bps {
            return false;
        }
    }
    let Some(target) = world.firm(deal.target) else {
        return false;
    };
    let eligible = target.shares.eligible_votes();
    let bought: i64 = tender_plan(world, deal).iter().map(|(_, qty)| *qty).sum();
    let after = target.shares.qty(HolderId::Firm(deal.buyer)) + bought;
    let sell = after.saturating_sub(eligible * 49 / 100);
    let proceeds = sell.saturating_mul(target.shares.fair_cents.max(1));
    world
        .outside
        .investors
        .iter()
        .any(|holder| holder.capital_cents >= proceeds)
}

/// Whether a holder tenders at the offer price against the frozen fair value.
fn accepts(world: &World, deal: &Deal, holder: HolderId, buyer_group: &BTreeSet<Handle>) -> bool {
    let fair = deal.frozen_fair_cents.max(1) as i128;
    let offer = deal.offer_price_cents as i128;
    match holder {
        HolderId::Outside(index) => {
            let factor = world
                .outside
                .investors
                .get(index as usize)
                .map(|investor| investor.reservation_factor_bps as i128)
                .unwrap_or(10_000);
            offer * 10_000 >= fair * factor
        }
        HolderId::Investor => deal.investor_tendered,
        HolderId::Firm(owner) => {
            if owner == deal.buyer {
                return false;
            }
            if buyer_group.contains(&owner) {
                return true;
            }
            // A company in the player's group never sells without a player decision.
            group_key(world, owner) != HolderId::Investor && offer >= fair
        }
        HolderId::Treasury | HolderId::State => false,
    }
}

/// Shares each holder sells into the tender, capped at the bid maximum. Oversubscription is
/// allocated pro rata by largest remainder, ties in holder order, so the total never exceeds
/// the escrowed maximum.
fn tender_plan(world: &World, deal: &Deal) -> Vec<(HolderId, i64)> {
    let Some(target) = world.firms.get(deal.target) else {
        return Vec::new();
    };
    let buyer_group = competition_group(world, deal.buyer);
    let offers: Vec<(HolderId, i64)> = target
        .shares
        .holders
        .iter()
        .filter(|(holder, lot)| {
            lot.qty > 0
                && !holder.is_treasury()
                && **holder != HolderId::Firm(deal.buyer)
                && accepts(world, deal, **holder, &buyer_group)
        })
        .map(|(holder, lot)| (*holder, lot.qty))
        .collect();
    let total: i64 = offers.iter().map(|(_, qty)| *qty).sum();
    let cap = deal.max_shares.max(0);
    if total <= cap {
        return offers;
    }
    let mut allocated: Vec<(HolderId, i64, i128)> = offers
        .iter()
        .map(|(holder, qty)| {
            let scaled = *qty as i128 * cap as i128;
            (
                *holder,
                (scaled / total as i128) as i64,
                scaled % total as i128,
            )
        })
        .collect();
    let mut left = cap - allocated.iter().map(|(_, qty, _)| *qty).sum::<i64>();
    let mut order: Vec<usize> = (0..allocated.len()).collect();
    order.sort_by(|a, b| allocated[*b].2.cmp(&allocated[*a].2).then(a.cmp(b)));
    for index in order {
        if left <= 0 {
            break;
        }
        allocated[index].1 += 1;
        left -= 1;
    }
    allocated
        .into_iter()
        .filter(|(_, qty, _)| *qty > 0)
        .map(|(holder, qty, _)| (holder, qty))
        .collect()
}

fn collect_tenders(world: &mut World, handle: Handle, deal: &Deal) {
    let committed: i64 = tender_plan(world, deal).iter().map(|(_, qty)| *qty).sum();
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.committed_shares = committed;
    }
}

fn lot_basis(world: &World, target: Handle, holder: HolderId, qty: i64) -> i64 {
    world
        .firm(target)
        .and_then(|firm| firm.shares.holders.get(&holder).copied())
        .map(|lot| {
            if lot.qty > 0 {
                (lot.cost_cents as i128 * qty as i128 / lot.qty as i128) as i64
            } else {
                0
            }
        })
        .unwrap_or(0)
}

/// Atomic share settlement. Every leg is priced and funded before anything posts: the tender
/// legs and fee come from escrow, and a squeeze-out tops up from the buyer's free cash. If the
/// squeeze-out cannot be funded the buyer takes control only; if the tender itself no longer
/// reaches control (holders sold elsewhere) the deal expires with a refund.
fn settle_tender(world: &mut World, handle: Handle, deal: &Deal, remedy: bool) {
    let plan = tender_plan(world, deal);
    let Some(target) = world.firm(deal.target) else {
        return;
    };
    let eligible = target.shares.eligible_votes();
    let held = target.shares.qty(HolderId::Firm(deal.buyer));
    let bought: i64 = plan.iter().map(|(_, qty)| *qty).sum();
    if eligible <= 0 || (held + bought) * 2 <= eligible {
        expire(world, handle, deal);
        return;
    }
    let fair_now = target.shares.fair_cents;
    let offer = deal.offer_price_cents;
    let mut legs: Vec<(HolderId, i64, i64)> = plan
        .iter()
        .map(|(holder, qty)| (*holder, *qty, qty.saturating_mul(offer)))
        .collect();
    let tender_cost: i64 = legs.iter().map(|(_, _, cash)| *cash).sum();
    let fee = transaction_fee(tender_cost);
    let escrow = world
        .deals
        .get(handle)
        .map(|stored| stored.escrow_cents)
        .unwrap_or(0);
    if tender_cost + fee > escrow {
        refund_escrow(world, handle, deal, "escrow shortfall");
        if let Some(stored) = world.deals.get_mut(handle) {
            stored.state = DealState::Rejected;
        }
        return;
    }
    // Squeeze-out: at ninety percent every remaining holder is paid max(offer, fair value).
    let squeeze_price = offer.max(fair_now);
    let tendered: BTreeMap<HolderId, i64> = plan.iter().copied().collect();
    let mut squeeze: Vec<(HolderId, i64, i64)> = Vec::new();
    if !remedy && (held + bought) as i128 * 10_000 >= eligible as i128 * SQUEEZE_OUT_BPS {
        for (holder, lot) in &target.shares.holders {
            if holder.is_treasury() || *holder == HolderId::Firm(deal.buyer) {
                continue;
            }
            let rest = lot.qty - tendered.get(holder).copied().unwrap_or(0);
            if rest > 0 {
                squeeze.push((*holder, rest, rest.saturating_mul(squeeze_price)));
            }
        }
    }
    let squeeze_cost: i64 = squeeze.iter().map(|(_, _, cash)| *cash).sum();
    let free_cash = world.firm(deal.buyer).map(|f| f.book.cash()).unwrap_or(0);
    if tender_cost + fee + squeeze_cost > escrow + free_cash {
        squeeze.clear();
    }
    legs.extend(squeeze);
    let consideration: i64 = legs.iter().map(|(_, _, cash)| *cash).sum();
    let needed = consideration + fee;

    for (holder, qty, cash) in &legs {
        let basis = lot_basis(world, deal.target, *holder, *qty);
        pay_seller(world, *holder, *cash, basis, "tender");
        if let Some(target) = world.firms.get_mut(deal.target) {
            target
                .shares
                .transfer(*holder, HolderId::Firm(deal.buyer), *qty, *cash);
        }
    }
    if let Some(bidder) = world.firms.get_mut(deal.buyer) {
        let _ = bidder.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Investments, consideration, "tender")
                .debit(Account::OperatingExpense, fee, "transaction fee")
                .credit(Account::RestrictedCash, escrow, "escrow release")
                .signed(Account::Cash, escrow - needed, "escrow refund"),
        );
    }
    if fee > 0 {
        let _ = world.outside.legal_services_book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, fee, "transaction fee")
                .credit(Account::Revenue, fee, "transaction fee"),
        );
    }
    if world.investor.primary_group == Some(deal.target) && world.investor_holds(deal.target) == 0 {
        world.investor.primary_group = None;
    }
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.escrow_cents = 0;
        stored.committed_shares = bought;
        stored.state = DealState::Settled;
    }
    if remedy {
        let _ = divest_control(world, deal);
    }
}

/// Asset purchase: the buyer acquires the target's operating assets (facilities, inventory,
/// fixed assets and work in progress) at the agreed price. Shares, cash, debt and contracts stay
/// with the target, which is the distinction from a share purchase or merger. Assumed liabilities
/// are none in this release; that limitation is visible in the deal preview. A company the
/// player controls cannot be stripped of its assets by an outside group, because the player has
/// no consent step for an asset sale.
fn settle_asset_purchase(world: &mut World, handle: Handle, deal: &Deal) {
    let cost = deal
        .offer_price_cents
        .saturating_mul(deal.max_shares.max(1));
    let fee = transaction_fee(cost);
    let player_target = group_key(world, deal.target) == HolderId::Investor
        && group_key(world, deal.buyer) != HolderId::Investor;
    if cost <= 0 || cost + fee > deal.escrow_cents || player_target {
        refund_escrow(world, handle, deal, "asset purchase refused");
        if let Some(stored) = world.deals.get_mut(handle) {
            stored.state = DealState::Rejected;
        }
        return;
    }
    let target_book = match world.firm(deal.target) {
        Some(firm) => firm.book.clone(),
        None => return,
    };
    let accounts = [
        Account::FixedAssets,
        Account::AccumulatedDepreciation,
        Account::Inventory,
        Account::WorkInProgress,
    ];
    let net_book: i64 = accounts
        .iter()
        .map(|account| target_book.balance(*account))
        .sum();
    // Buyer: releases escrow, receives the assets at book value and books the difference.
    let mut buyer_txn = Txn::new();
    for account in accounts {
        let balance = target_book.balance(account);
        if balance != 0 {
            buyer_txn = buyer_txn.signed(account, balance, "asset purchase");
        }
    }
    let refund = deal.escrow_cents - cost - fee;
    buyer_txn = buyer_txn
        .credit(Account::RestrictedCash, deal.escrow_cents, "asset purchase")
        .debit(Account::OperatingExpense, fee, "transaction fee")
        .signed(Account::Cash, refund, "asset escrow refund")
        .signed(Account::OtherIncome, cost - net_book, "asset purchase");
    let _ = world
        .firms
        .get_mut(deal.buyer)
        .unwrap()
        .book
        .commit(world.tick, buyer_txn);
    // Target: gives up the assets, receives the cash and recognises the gain or loss.
    let mut target_txn = Txn::new();
    for account in accounts {
        let balance = target_book.balance(account);
        if balance != 0 {
            target_txn = target_txn.signed(account, -balance, "asset sale");
        }
    }
    target_txn = target_txn.debit(Account::Cash, cost, "asset sale").signed(
        Account::OtherIncome,
        net_book - cost,
        "asset sale",
    );
    let _ = world
        .firms
        .get_mut(deal.target)
        .unwrap()
        .book
        .commit(world.tick, target_txn);
    if fee > 0 {
        let _ = world.outside.legal_services_book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, fee, "transaction fee")
                .credit(Account::Revenue, fee, "transaction fee"),
        );
    }

    let facilities: Vec<Handle> = world
        .firm(deal.target)
        .map(|firm| firm.facilities.clone())
        .unwrap_or_default();
    for facility in &facilities {
        if let Some(stored) = world.facilities.get_mut(*facility) {
            stored.firm = deal.buyer;
        }
        let bidder = world.firms.get_mut(deal.buyer).unwrap();
        if !bidder.facilities.contains(facility) {
            bidder.facilities.push(*facility);
        }
    }
    let inventory = world
        .firm(deal.target)
        .map(|firm| firm.inventory.clone())
        .unwrap_or_default();
    for (key, lot) in inventory {
        world
            .firms
            .get_mut(deal.buyer)
            .unwrap()
            .inventory
            .insert(key, lot);
    }
    if let Some(target) = world.firms.get_mut(deal.target) {
        target.facilities.clear();
        target.inventory.clear();
    }
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.state = DealState::Settled;
        stored.escrow_cents = 0;
    }
}

/// Pay a selling holder. A sale below cost is a realised loss, not a company expense.
fn pay_seller(world: &mut World, holder: HolderId, amount: i64, basis: i64, memo: &str) {
    let gain = amount - basis;
    match holder {
        HolderId::Investor => {
            world.investor.cash_cents += amount;
            world.investor.realised_gains_cents += gain;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, amount, memo)
                    .credit(Account::Investments, basis, memo)
                    .credit(Account::OtherIncome, gain, memo),
            );
        }
        HolderId::Outside(index) => {
            if let Some(investor) = world.outside.investors.get_mut(index as usize) {
                investor.capital_cents += amount;
            }
        }
        HolderId::Firm(handle) => {
            if let Some(firm) = world.firms.get_mut(handle) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, amount, memo)
                        .credit(Account::Investments, basis, memo)
                        .credit(Account::OtherIncome, gain, memo),
                );
            }
        }
        HolderId::Treasury | HolderId::State => {}
    }
}

/// Cash the merger must pay remaining minority holders: max(offer, fair value) per share.
pub fn minority_buyout_cost(world: &World, deal: &Deal) -> i64 {
    let Some(target) = world.firm(deal.target) else {
        return 0;
    };
    let price = deal.offer_price_cents.max(target.shares.fair_cents);
    target
        .shares
        .holders
        .iter()
        .filter(|(holder, _)| !holder.is_treasury() && **holder != HolderId::Firm(deal.buyer))
        .map(|(_, lot)| lot.qty.saturating_mul(price))
        .sum()
}

/// Before a legal merger retires the target's shares, every remaining minority holder is paid in
/// cash from the buyer at max(offer, fair value). Fails without posting if the buyer cannot pay.
pub fn buy_out_minorities(world: &mut World, deal: &Deal) -> DomainResult<()> {
    let cost = minority_buyout_cost(world, deal);
    let cash = world.firm(deal.buyer).map(|f| f.book.cash()).unwrap_or(0);
    if cash < cost {
        return Err(DomainError::InsufficientCash {
            entity: deal.buyer.0,
            required: cost,
            available: cash,
        });
    }
    let Some(target) = world.firm(deal.target) else {
        return Ok(());
    };
    let price = deal.offer_price_cents.max(target.shares.fair_cents);
    let legs: Vec<(HolderId, i64)> = target
        .shares
        .holders
        .iter()
        .filter(|(holder, lot)| {
            lot.qty > 0 && !holder.is_treasury() && **holder != HolderId::Firm(deal.buyer)
        })
        .map(|(holder, lot)| (*holder, lot.qty))
        .collect();
    for (holder, qty) in legs {
        let cash = qty.saturating_mul(price);
        let basis = lot_basis(world, deal.target, holder, qty);
        pay_seller(world, holder, cash, basis, "merger minority payout");
        if let Some(target) = world.firms.get_mut(deal.target) {
            target
                .shares
                .transfer(holder, HolderId::Firm(deal.buyer), qty, cash);
        }
        if let Some(bidder) = world.firms.get_mut(deal.buyer) {
            let _ = bidder.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, cash, "merger minority payout")
                    .debit(Account::Investments, cash, "merger minority payout"),
            );
        }
    }
    if world.investor.primary_group == Some(deal.target) {
        world.investor.primary_group = None;
    }
    Ok(())
}

fn refund_escrow(world: &mut World, handle: Handle, deal: &Deal, memo: &str) {
    let escrow = world
        .deals
        .get(handle)
        .map(|stored| stored.escrow_cents)
        .unwrap_or(0);
    if escrow > 0 {
        if let Some(bidder) = world.firms.get_mut(deal.buyer) {
            let _ = bidder.book.commit(
                world.tick,
                Txn::new().debit(Account::Cash, escrow, memo).credit(
                    Account::RestrictedCash,
                    escrow,
                    memo,
                ),
            );
        }
        if let Some(stored) = world.deals.get_mut(handle) {
            stored.escrow_cents = 0;
        }
    }
}

fn reject(world: &mut World, handle: Handle, deal: &Deal) {
    refund_escrow(world, handle, deal, "review rejected");
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.state = DealState::Rejected;
    }
    // The score's conduct component is the player's record; a rival's rejected bid is not it.
    if crate::economy::events_engine::player_side(world, deal.buyer) {
        world.regulatory_breaches += 1;
    }
}

/// Offer the divestiture remedy during review. The remedy sells the buyer down below control,
/// so it is accepted only when the buyer's group alone clears both triggers and an outside
/// holder can fund the divested block; otherwise the request is refused now rather than at the
/// review deadline.
pub fn submit_remedy(world: &mut World, handle: Handle, share_bps: i32) -> DomainResult<()> {
    let Some(deal) = world.deals.get(handle).cloned() else {
        return Err(DomainError::InvalidEntity {
            handle: handle.0,
            kind: EntityKind::Deal,
        });
    };
    if deal.state != DealState::Review {
        return Err(DomainError::WrongPhase {
            expected: "review",
            actual: deal.state.key(),
        });
    }
    if share_bps <= 0 || deal.kind == DealKind::AssetPurchase || !remedy_feasible(world, &deal) {
        return Err(DomainError::SolvencyRestriction {
            entity: handle.0,
            reason: cw_core::error::SolvencyReason::CoverageTooLow,
        });
    }
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.remedy_share_bps = share_bps;
    }
    Ok(())
}

/// Record the player's tender choice. Shares move only at settlement, from escrow.
pub fn respond_tender(world: &mut World, handle: Handle, accept: bool) -> DomainResult<()> {
    let Some(deal) = world.deals.get(handle) else {
        return Err(DomainError::InvalidEntity {
            handle: handle.0,
            kind: EntityKind::Deal,
        });
    };
    if !matches!(deal.state, DealState::OfferOpen | DealState::Diligence) {
        return Err(DomainError::WrongPhase {
            expected: "offer_open",
            actual: deal.state.key(),
        });
    }
    if accept && world.investor_holds(deal.target) <= 0 {
        return Err(DomainError::InsufficientStock {
            product: 0,
            required: 1,
            available: 0,
        });
    }
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.investor_tendered = accept;
    }
    Ok(())
}

/// Withdraw a deal and return unused escrow minus incurred costs.
pub fn withdraw(world: &mut World, handle: Handle) -> DomainResult<()> {
    let Some(deal) = world.deals.get(handle).cloned() else {
        return Err(DomainError::InvalidEntity {
            handle: handle.0,
            kind: EntityKind::Deal,
        });
    };
    if deal.state.terminal() {
        return Err(DomainError::WrongPhase {
            expected: "open",
            actual: deal.state.key(),
        });
    }
    refund_escrow(world, handle, &deal, "deal withdrawn");
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.state = DealState::Withdrawn;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};
    use cw_data::Dataset;

    #[test]
    fn escrow_includes_the_two_percent_fee() {
        assert_eq!(escrow_required(1_000, 1_000), 1_020_000);
    }

    #[test]
    fn concentration_is_group_aware() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 81, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
        let buyer = handles[0];
        let target = handles[1];
        // Concentration aggregates the buyer and its controlled target only.
        {
            let issued = world.firm(target).unwrap().shares.issued;
            world
                .firms
                .get_mut(target)
                .unwrap()
                .shares
                .add(HolderId::Firm(buyer), issued + 1, 0);
            world.firms.get_mut(target).unwrap().shares.issued += issued + 1;
        }
        let region = world.firm(target).unwrap().region;
        let mut week = BTreeMap::new();
        week.insert((region, 7u16), 600i64);
        if let Some(firm) = world.firms.get_mut(buyer) {
            firm.region = region;
            firm.sales_history = vec![week.clone(); 13];
        }
        if let Some(firm) = world.firms.get_mut(target) {
            firm.region = region;
            firm.sales_history = vec![week.clone(); 13];
        }
        if let Some(firm) = world.firms.get_mut(handles[2]) {
            firm.region = region;
            let mut other = BTreeMap::new();
            other.insert((region, 7u16), 400i64);
            firm.sales_history = vec![other; 13];
        }
        let (share, _hhi, _delta) = concentration(&world, region, 7, buyer, target);
        assert_eq!(share, 7_500);
    }

    #[test]
    fn a_remedy_divests_enough_shares_to_remove_control() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            771,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        let handles: Vec<Handle> = world
            .firms
            .iter()
            .map(|(handle, _)| handle)
            .take(2)
            .collect();
        let (buyer, target) = (handles[0], handles[1]);
        // Buyer holds eighty percent; a remedy must take it below half.
        {
            let issued = world.firm(target).unwrap().shares.issued;
            // More than the whole existing issue, so the buyer holds strictly over half.
            let acquired = issued + 1;
            world
                .firms
                .get_mut(target)
                .unwrap()
                .shares
                .add(HolderId::Firm(buyer), acquired, 0);
            world.firms.get_mut(target).unwrap().shares.issued += acquired;
        }
        assert_eq!(
            world.firm(target).unwrap().shares.controller(),
            Some(HolderId::Firm(buyer))
        );
        let cash_before = world.firm(buyer).unwrap().book.cash();
        let issued_total = world.firm(target).unwrap().shares.issued;
        let deal = Deal {
            buyer,
            target,
            kind: DealKind::SharePurchase,
            state: DealState::Review,
            escrow_cents: 0,
            offer_price_cents: 1,
            max_shares: 0,
            committed_shares: 0,
            started: world.tick,
            diligence_until: world.tick,
            offer_deadline: world.tick,
            review_deadline: Some(world.tick),
            frozen_fair_cents: 1,
            review_required: true,
            remedy_share_bps: 2_000,
            hostile: false,
            diligence_paid: true,
            investor_tendered: false,
        };
        assert!(divest_control(&mut world, &deal), "shares were divested");
        assert!(
            world.firm(target).unwrap().shares.controller() != Some(HolderId::Firm(buyer)),
            "the buyer no longer controls the target"
        );
        assert!(
            world.firm(buyer).unwrap().book.cash() > cash_before,
            "the buyer is paid"
        );
        let total: i64 = world
            .firm(target)
            .unwrap()
            .shares
            .holders
            .values()
            .map(|lot| lot.qty)
            .sum();
        assert_eq!(total, issued_total, "shares are conserved");
        assert!(world.firm(buyer).unwrap().book.is_balanced());
    }

    #[test]
    fn an_asset_purchase_moves_assets_but_keeps_shares_and_debt() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            761,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        let handles: Vec<Handle> = world
            .firms
            .iter()
            .filter(|(_, firm)| firm.operative() && !firm.facilities.is_empty())
            .map(|(handle, _)| handle)
            .take(2)
            .collect();
        let (buyer, target) = (handles[0], handles[1]);
        let cost = 50_000_000;
        let escrow = escrow_required(cost, 1);
        // Fund the buyer's escrow directly; the command path does the same through StartDiligence.
        world
            .firms
            .get_mut(buyer)
            .unwrap()
            .book
            .commit(
                world.tick,
                cw_core::ledger::Txn::new()
                    .debit(cw_core::ledger::Account::RestrictedCash, escrow, "escrow")
                    .credit(cw_core::ledger::Account::Cash, escrow, "escrow"),
            )
            .unwrap();
        let target_facilities = world.firm(target).unwrap().facilities.len();
        let buyer_facilities = world.firm(buyer).unwrap().facilities.len();
        let target_shares = world.firm(target).unwrap().shares.issued;
        let target_cash = world.firm(target).unwrap().book.cash();
        let deal = Deal {
            buyer,
            target,
            kind: DealKind::AssetPurchase,
            state: DealState::OfferOpen,
            escrow_cents: escrow,
            offer_price_cents: cost,
            max_shares: 1,
            committed_shares: 0,
            started: world.tick,
            diligence_until: world.tick,
            offer_deadline: world.tick,
            review_deadline: None,
            frozen_fair_cents: 0,
            review_required: false,
            remedy_share_bps: 0,
            hostile: false,
            diligence_paid: true,
            investor_tendered: false,
        };
        let handle = world.deals.insert(deal.clone());
        settle_asset_purchase(&mut world, handle, &deal);
        assert_eq!(
            world.firm(target).unwrap().facilities.len(),
            0,
            "assets left the target"
        );
        assert_eq!(
            world.firm(buyer).unwrap().facilities.len(),
            buyer_facilities + target_facilities,
            "assets arrived at the buyer"
        );
        assert_eq!(
            world.firm(target).unwrap().shares.issued,
            target_shares,
            "shares stay"
        );
        assert_eq!(
            world.firm(target).unwrap().book.cash(),
            target_cash + cost,
            "the target is paid"
        );
        assert_eq!(world.deals.get(handle).unwrap().state, DealState::Settled);
        assert!(world.firm(buyer).unwrap().book.is_balanced());
        assert!(world.firm(target).unwrap().book.is_balanced());
    }

    #[test]
    fn a_deal_cannot_start_without_funded_escrow() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 82, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
        let result = start(
            &mut world,
            handles[0],
            handles[1],
            DealKind::SharePurchase,
            1_000_000_000,
            1_000_000,
        );
        assert!(result.is_err());
    }
}
