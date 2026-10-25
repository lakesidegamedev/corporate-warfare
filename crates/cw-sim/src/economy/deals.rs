//! Mergers, hostile takeovers and competition review.
//!
//! DRAFT -> FUNDED -> DILIGENCE -> OFFER_OPEN -> REVIEW -> SETTLED, with WITHDRAWN, EXPIRED and
//! REJECTED as terminal alternatives. No path skips escrow or review. The buyer escrows the
//! full maximum cash price plus two percent; diligence costs half a percent of bid value and
//! lasts two weeks; the offer stays open four weeks. Control needs more than half the votes;
//! squeeze-out needs ninety percent plus review and full minority cash at
//! max(offer, fair value).
//!
//! Antitrust: HHI = sum((group regional product share * 100)^2). Review triggers when the
//! combined share is at least 35 percent, or post-HHI exceeds 2500 with a delta above 200.
//! Review lasts four weeks. A feasible divestiture can clear it; an infeasible remedy rejects
//! the deal rather than inventing regulator discretion.

use std::collections::{BTreeMap, BTreeSet};

use cw_core::calendar::Tick;
use cw_core::error::{DomainError, DomainResult, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::round_even_div;

use crate::state::{Deal, DealKind, DealState, HolderId, World};

const DILIGENCE_WEEKS: u32 = 2;
const OFFER_WEEKS: u32 = 4;
const REVIEW_WEEKS: u32 = 4;
pub const SHARE_REVIEW_BPS: i64 = 3_500;
pub const HHI_REVIEW: i64 = 2_500;
pub const HHI_DELTA: i64 = 200;

pub fn escrow_required(max_price_cents: i64, shares: i64) -> i64 {
    let gross = max_price_cents.saturating_mul(shares);
    gross + gross * 2 / 100
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
    if crate::economy::ownership::would_create_cycle(world, buyer, target) {
        return Err(DomainError::OwnershipCycle {
            buyer: buyer.0,
            target: target.0,
        });
    }
    let escrow = escrow_required(price_per_share, max_shares);
    let cash = world.firms.get(buyer).map(|f| f.book.cash()).unwrap_or(0);
    if cash < escrow {
        return Err(DomainError::InsufficientCash {
            entity: buyer.0,
            required: escrow,
            available: cash,
        });
    }
    let fee = round_even_div(price_per_share as i128 * max_shares as i128 * 50, 10_000) as i64;
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
    let frozen_fair = target_fair;
    let hostile = price_per_share <= target_mark;
    let deal = Deal {
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
        frozen_fair_cents: frozen_fair,
        review_required: false,
        remedy_share_bps: 0,
        hostile,
        diligence_paid: fee > 0,
    };
    Ok(world.deals.insert(deal))
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
                    let (share, hhi, delta) = trigger_metrics(world, deal.buyer, deal.target);
                    let review =
                        share >= SHARE_REVIEW_BPS || (hhi > HHI_REVIEW && delta > HHI_DELTA);
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
                    settle_offer(world, handle, &updated);
                }
            }
            DealState::Review => {
                let deadline = deal.review_deadline.map(|tick| tick.0).unwrap_or(u32::MAX);
                if world.tick.0 >= deadline {
                    let (share, hhi, delta) = trigger_metrics(world, deal.buyer, deal.target);
                    let remedy_ok = deal.remedy_share_bps > 0
                        && share - deal.remedy_share_bps as i64 <= SHARE_REVIEW_BPS
                        && (delta - deal.remedy_share_bps as i64) <= HHI_DELTA;
                    if remedy_ok
                        || (share < SHARE_REVIEW_BPS && !(hhi > HHI_REVIEW && delta > HHI_DELTA))
                    {
                        if remedy_ok {
                            let _ = divest_control(world, &deal);
                        }
                        if let Some(stored) = world.deals.get_mut(handle) {
                            stored.state = DealState::Settled;
                        }
                        world.regulatory_breaches += if remedy_ok { 0 } else { 1 };
                    } else {
                        reject(world, handle, &deal);
                    }
                }
            }
            _ => {}
        }
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
    let outside = world
        .outside
        .investors
        .iter_mut()
        .find(|holder| holder.capital_cents >= proceeds);
    let Some(outside) = outside else {
        return false;
    };
    outside.capital_cents -= proceeds;
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
            HolderId::Outside(0),
            sell,
            proceeds,
        );
    }
    true
}

fn trigger_metrics(world: &World, buyer: Handle, target: Handle) -> (i64, i64, i64) {
    let region = world.firms.get(target).map(|firm| firm.region).unwrap_or(0);
    let products: Vec<u16> = world
        .firms
        .get(target)
        .map(|firm| {
            firm.sales_history
                .iter()
                .flat_map(|week| week.keys().map(|(_, p)| *p))
                .collect()
        })
        .unwrap_or_default();
    let mut worst_share = 0i64;
    let mut worst_hhi = 0i64;
    let mut worst_delta = 0i64;
    for product in products {
        let (share, hhi, delta) = concentration(world, region, product, buyer, target);
        if share > worst_share {
            worst_share = share;
        }
        if hhi > worst_hhi {
            worst_hhi = hhi;
            worst_delta = delta;
        }
    }
    (worst_share, worst_hhi, worst_delta)
}

/// Regional product concentration after the deal, using trailing thirteen-week unit sales and
/// controlled-group aggregation.
pub fn concentration(
    world: &World,
    region: u8,
    product: u16,
    buyer: Handle,
    target: Handle,
) -> (i64, i64, i64) {
    let buyer_group = crate::economy::ownership::controlled_group(world, buyer);
    // Once a remedy has removed control, the target's sales no longer belong to the buyer group.
    let target_group = if world
        .firm(target)
        .map(|firm| firm.shares.controller() == Some(HolderId::Firm(buyer)))
        .unwrap_or(false)
    {
        crate::economy::ownership::controlled_group(world, target)
    } else {
        Vec::new()
    };
    let mut volumes: BTreeMap<Handle, i64> = BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        let volume = firm.trailing_units(region, product);
        if volume > 0 {
            volumes.insert(handle, volume);
        }
    }
    let total: i64 = volumes.values().sum();
    if total <= 0 {
        return (0, 0, 0);
    }
    // Union, not concatenation: a controlled target is already inside the buyer group.
    let mut group: BTreeSet<Handle> = BTreeSet::new();
    group.extend(buyer_group.iter().copied());
    group.extend(target_group.iter().copied());
    let combined: i64 = group
        .iter()
        .map(|handle| volumes.get(handle).copied().unwrap_or(0))
        .sum();
    let mut before_hhi = 0i64;
    let mut after_hhi = 0i64;
    for (handle, volume) in &volumes {
        let share = volume * 10_000 / total;
        before_hhi += (share as i128 * share as i128 / 10_000) as i64;
        if group.contains(handle) {
            continue;
        }
        after_hhi += (share as i128 * share as i128 / 10_000) as i64;
    }
    let combined_share = combined * 10_000 / total;
    after_hhi += (combined_share as i128 * combined_share as i128 / 10_000) as i64;
    (combined_share, after_hhi, after_hhi - before_hhi)
}

fn collect_tenders(world: &mut World, handle: Handle, deal: &Deal) {
    let Some(target) = world.firms.get(deal.target) else {
        return;
    };
    let fair = deal.frozen_fair_cents.max(1);
    let holders: Vec<(HolderId, i64)> = target
        .shares
        .holders
        .iter()
        .filter(|(holder, _)| !holder.is_treasury() && **holder != HolderId::Firm(deal.buyer))
        .map(|(holder, lot)| (*holder, lot.qty))
        .collect();
    let mut committed = 0i64;
    for (holder, qty) in holders {
        let accept = match holder {
            HolderId::Outside(index) => {
                let factor = world
                    .outside
                    .investors
                    .get(index as usize)
                    .map(|investor| investor.reservation_factor_bps as i64)
                    .unwrap_or(10_000);
                deal.offer_price_cents as i128 * 10_000 >= fair as i128 * factor as i128
            }
            HolderId::Investor => false,
            HolderId::Firm(owner) => {
                crate::economy::ownership::controlled_group(world, deal.buyer).contains(&owner)
                    || deal.offer_price_cents >= fair
            }
            HolderId::Treasury => false,
        };
        if accept {
            committed += qty;
        }
    }
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.committed_shares = committed.min(stored.max_shares);
    }
}

fn settle_offer(world: &mut World, handle: Handle, deal: &Deal) {
    if deal.kind == DealKind::AssetPurchase {
        settle_asset_purchase(world, handle, deal);
        return;
    }
    let eligible = world
        .firms
        .get(deal.target)
        .map(|firm| firm.shares.eligible_votes())
        .unwrap_or(0);
    if eligible == 0 || deal.committed_shares * 2 <= eligible {
        refund_escrow(world, handle, deal, "deal expired");
        if let Some(stored) = world.deals.get_mut(handle) {
            stored.state = DealState::Expired;
        }
        return;
    }
    // Pay committed holders from escrow and transfer shares.
    let cost = deal.committed_shares.saturating_mul(deal.offer_price_cents);
    if cost > deal.escrow_cents {
        refund_escrow(world, handle, deal, "escrow shortfall");
        if let Some(stored) = world.deals.get_mut(handle) {
            stored.state = DealState::Rejected;
        }
        return;
    }
    let holders: Vec<(HolderId, i64)> = world
        .firms
        .get(deal.target)
        .map(|firm| {
            firm.shares
                .holders
                .iter()
                .map(|(h, l)| (*h, l.qty))
                .collect()
        })
        .unwrap_or_default();
    let fair = world
        .firms
        .get(deal.target)
        .map(|f| f.shares.fair_cents)
        .unwrap_or(0);
    let squeeze = deal.committed_shares * 10_000 >= eligible * 9_000;
    for (holder, qty) in holders {
        if holder.is_treasury() || holder == HolderId::Firm(deal.buyer) {
            continue;
        }
        let accepted = match holder {
            HolderId::Outside(index) => {
                let factor = world
                    .outside
                    .investors
                    .get(index as usize)
                    .map(|investor| investor.reservation_factor_bps as i64)
                    .unwrap_or(10_000);
                deal.offer_price_cents as i128 * 10_000 >= fair as i128 * factor as i128
            }
            HolderId::Investor => false,
            HolderId::Firm(_) => deal.offer_price_cents >= fair,
            HolderId::Treasury => false,
        };
        if !accepted && !squeeze {
            continue;
        }
        let take = if accepted {
            qty.min(deal.committed_shares)
        } else {
            qty
        };
        let price = if squeeze {
            deal.offer_price_cents.max(fair)
        } else {
            deal.offer_price_cents
        };
        let cash = take.saturating_mul(price);
        pay_seller(world, holder, cash, "tender");
        if let Some(target) = world.firms.get_mut(deal.target) {
            target
                .shares
                .transfer(holder, HolderId::Firm(deal.buyer), take, cash);
        }
        if let Some(bidder) = world.firms.get_mut(deal.buyer) {
            let _ = bidder.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::RestrictedCash, cash, "tender")
                    .debit(Account::Investments, cash, "tender"),
            );
        }
    }
    // Refund whatever escrow was not spent.
    let remaining = world
        .deals
        .get(handle)
        .map(|stored| stored.escrow_cents)
        .unwrap_or(0)
        .saturating_sub(cost);
    if remaining > 0
        && let Some(bidder) = world.firms.get_mut(deal.buyer)
    {
        let _ = bidder.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, remaining, "escrow refund")
                .credit(Account::RestrictedCash, remaining, "escrow refund"),
        );
    }
    let review = world
        .deals
        .get(handle)
        .map(|stored| stored.review_required)
        .unwrap_or(false)
        || trigger_metrics(world, deal.buyer, deal.target).0 >= SHARE_REVIEW_BPS;
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.escrow_cents = 0;
        if review {
            stored.state = DealState::Review;
            stored.review_deadline = Some(Tick(world.tick.0 + REVIEW_WEEKS));
        } else {
            stored.state = DealState::Settled;
        }
    }
}

/// Asset purchase: the buyer acquires the target's operating assets (facilities, inventory,
/// fixed assets and work in progress) at the agreed price. Shares, cash, debt and contracts stay
/// with the target, which is the distinction from a share purchase or merger. Assumed liabilities
/// are none in v0.1; that limitation is visible in the deal preview.
fn settle_asset_purchase(world: &mut World, handle: Handle, deal: &Deal) {
    let cost = deal
        .offer_price_cents
        .saturating_mul(deal.max_shares.max(1));
    if cost <= 0 || cost > deal.escrow_cents {
        refund_escrow(world, handle, deal, "asset escrow shortfall");
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
        cw_core::ledger::Account::FixedAssets,
        cw_core::ledger::Account::AccumulatedDepreciation,
        cw_core::ledger::Account::Inventory,
        cw_core::ledger::Account::WorkInProgress,
    ];
    let net_book: i64 = accounts
        .iter()
        .map(|account| target_book.balance(*account))
        .sum();
    // Buyer: releases escrow, receives the assets at book value and books the difference.
    let mut buyer_txn = cw_core::ledger::Txn::new();
    for account in accounts {
        let balance = target_book.balance(account);
        if balance != 0 {
            buyer_txn = buyer_txn.signed(account, balance, "asset purchase");
        }
    }
    buyer_txn = buyer_txn
        .credit(
            cw_core::ledger::Account::RestrictedCash,
            cost,
            "asset purchase",
        )
        .signed(
            cw_core::ledger::Account::OtherIncome,
            cost - net_book,
            "asset purchase",
        );
    let _ = world
        .firms
        .get_mut(deal.buyer)
        .unwrap()
        .book
        .commit(world.tick, buyer_txn);
    // Target: gives up the assets, receives the cash and recognises the gain or loss.
    let mut target_txn = cw_core::ledger::Txn::new();
    for account in accounts {
        let balance = target_book.balance(account);
        if balance != 0 {
            target_txn = target_txn.signed(account, -balance, "asset sale");
        }
    }
    target_txn = target_txn
        .debit(cw_core::ledger::Account::Cash, cost, "asset sale")
        .signed(
            cw_core::ledger::Account::OtherIncome,
            net_book - cost,
            "asset sale",
        );
    let _ = world
        .firms
        .get_mut(deal.target)
        .unwrap()
        .book
        .commit(world.tick, target_txn);

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
    let refund = deal.escrow_cents.saturating_sub(cost);
    if refund > 0 {
        let _ = world.firms.get_mut(deal.buyer).unwrap().book.commit(
            world.tick,
            cw_core::ledger::Txn::new()
                .debit(
                    cw_core::ledger::Account::Cash,
                    refund,
                    "asset escrow refund",
                )
                .credit(
                    cw_core::ledger::Account::RestrictedCash,
                    refund,
                    "asset escrow refund",
                ),
        );
    }
    if let Some(stored) = world.deals.get_mut(handle) {
        stored.state = DealState::Settled;
        stored.escrow_cents = 0;
    }
}

fn pay_seller(world: &mut World, holder: HolderId, amount: i64, memo: &str) {
    match holder {
        HolderId::Investor => {
            world.investor.cash_cents += amount;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new().debit(Account::Cash, amount, memo).credit(
                    Account::Investments,
                    amount,
                    memo,
                ),
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
                    Txn::new().debit(Account::Cash, amount, memo).credit(
                        Account::Investments,
                        amount,
                        memo,
                    ),
                );
            }
        }
        HolderId::Treasury => {}
    }
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
    world.regulatory_breaches += 1;
}

/// Submit a divestiture remedy: the stated share must clear both thresholds.
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
    let (share, _, delta) = trigger_metrics(world, deal.buyer, deal.target);
    if share - share_bps as i64 > SHARE_REVIEW_BPS || delta - share_bps as i64 > HHI_DELTA {
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
        // Fund the buyer's escrow directly; the command path does the same through StartDiligence.
        world
            .firms
            .get_mut(buyer)
            .unwrap()
            .book
            .commit(
                world.tick,
                cw_core::ledger::Txn::new()
                    .debit(cw_core::ledger::Account::RestrictedCash, cost, "escrow")
                    .credit(cw_core::ledger::Account::Cash, cost, "escrow"),
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
            escrow_cents: cost,
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
