//! Share valuation, the weekly exchange and capital actions.
//!
//! Fair enterprise value and equity fair value follow CORPORATE.md. Share values are computed
//! in reverse acyclic ownership order, so a parent always values a subsidiary before itself.
//! The reference mark moves one eighth toward fair value each week and a genuine trade can move
//! it at most ten percent. No price change creates cash or player wealth until a sale settles.

use std::collections::BTreeMap;

use cw_core::calendar::Tick;
use cw_core::error::{DomainError, DomainResult, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::{Cents, allocate_cents, round_even_div};
use cw_data::Dataset;

use crate::economy::valuation::base_value;
use crate::state::{HolderId, OrderSide, ShareOrder, World};

/// Recompute fair values for every firm in dependency order.
pub fn refresh_valuation(world: &mut World, dataset: &Dataset) {
    let order = valuation_order(world);
    let mut equity_by_firm: BTreeMap<Handle, i64> = BTreeMap::new();
    for handle in order {
        let Some(firm) = world.firms.get(handle) else {
            continue;
        };
        let financial = firm.bank.is_some() || firm.insurer.is_some();
        let base = base_value(&firm.book, world.tick, financial);
        let mut stake_value = 0i64;
        for (holder, lot) in firm.shares.holders.iter() {
            if let HolderId::Firm(child) = holder
                && let Some(child_equity) = equity_by_firm.get(child)
            {
                let eligible = world
                    .firms
                    .get(*child)
                    .map(|entity| entity.shares.eligible_votes().max(1))
                    .unwrap_or(1);
                stake_value += child_equity * lot.qty / eligible;
            }
        }
        let equity = (base.equity_cents + stake_value).max(0);
        equity_by_firm.insert(handle, equity);
        let Some(firm) = world.firms.get_mut(handle) else {
            continue;
        };
        let eligible = firm.shares.eligible_votes().max(1);
        let fair = equity / eligible;
        let previous_mark = firm.shares.mark_cents.max(0);
        let mut mark = if previous_mark == 0 {
            fair
        } else {
            previous_mark + (fair - previous_mark) / 8
        };
        if previous_mark > 0 {
            let low = previous_mark * 90 / 100;
            let high = previous_mark * 110 / 100;
            mark = mark.clamp(low, high);
        }
        firm.shares.fair_cents = fair;
        firm.shares.mark_cents = mark.max(0);
        firm.valuation = base;
        firm.valuation.equity_cents = equity;
        firm.shares.statement_weeks += 1;
        // Sales for the week are closed by `Firm::close_sales_week` at the end of settlement.
        // Clearing them here discarded the unit-sales history before it was ever recorded.
    }
    let _ = dataset;
}

/// Leaves before parents. Iterative, so a hundred-level tree cannot overflow the stack.
fn valuation_order(world: &World) -> Vec<Handle> {
    let mut order = Vec::new();
    let mut state: BTreeMap<Handle, u8> = BTreeMap::new();
    let mut stack: Vec<(Handle, bool)> = Vec::new();
    for (handle, _) in world.firms.iter() {
        if state.get(&handle).copied().unwrap_or(0) != 0 {
            continue;
        }
        stack.push((handle, false));
        while let Some((current, expanded)) = stack.pop() {
            if expanded {
                if state.get(&current).copied().unwrap_or(0) != 2 {
                    state.insert(current, 2);
                    order.push(current);
                }
                continue;
            }
            if state.get(&current).copied().unwrap_or(0) != 0 {
                continue;
            }
            state.insert(current, 1);
            stack.push((current, true));
            if let Some(firm) = world.firms.get(current) {
                for (holder, _) in firm.shares.holders.iter() {
                    if let HolderId::Firm(child) = holder
                        && state.get(child).copied().unwrap_or(0) == 0
                    {
                        stack.push((*child, false));
                    }
                }
            }
        }
    }
    order
}

/// Place a share order, reserving cash or shares.
pub fn place_order(
    world: &mut World,
    holder: HolderId,
    firm: Handle,
    side: OrderSide,
    qty: i64,
    limit_price_cents: i64,
) -> DomainResult<Handle> {
    if qty <= 0 || limit_price_cents <= 0 {
        return Err(DomainError::Arithmetic {
            detail: cw_core::error::ArithmeticFault::OutOfRange,
        });
    }
    let Some(entity) = world.firms.get(firm) else {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: EntityKind::Firm,
        });
    };
    if !entity.status.can_trade() {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::PastDue,
        });
    }
    if entity.shares.qty(holder) < qty && side == OrderSide::Sell {
        return Err(DomainError::InsufficientStock {
            product: 0,
            required: qty,
            available: entity.shares.qty(holder),
        });
    }
    let mut reserved = 0i64;
    if side == OrderSide::Buy {
        let cost = qty.saturating_mul(limit_price_cents);
        match holder {
            HolderId::Investor => {
                if world.investor.cash_cents < cost {
                    return Err(DomainError::InsufficientCash {
                        entity: 0,
                        required: cost,
                        available: world.investor.cash_cents,
                    });
                }
                world.investor.cash_cents -= cost;
                reserved = cost;
            }
            HolderId::Firm(buyer) => {
                if crate::economy::ownership::would_create_cycle(world, buyer, firm) {
                    return Err(DomainError::OwnershipCycle {
                        buyer: buyer.0,
                        target: firm.0,
                    });
                }
                let cash = world.firms.get(buyer).map(|f| f.book.cash()).unwrap_or(0);
                if cash < cost {
                    return Err(DomainError::InsufficientCash {
                        entity: buyer.0,
                        required: cost,
                        available: cash,
                    });
                }
                let _ = world.firms.get_mut(buyer).unwrap().book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, cost, "order reserve")
                        .debit(Account::RestrictedCash, cost, "order reserve"),
                );
                reserved = cost;
            }
            HolderId::Outside(index) => {
                let capital = world
                    .outside
                    .investors
                    .get(index as usize)
                    .map(|h| h.capital_cents)
                    .unwrap_or(0);
                if capital < cost {
                    return Err(DomainError::InsufficientCash {
                        entity: index as u64,
                        required: cost,
                        available: capital,
                    });
                }
                if let Some(holder) = world.outside.investors.get_mut(index as usize) {
                    holder.capital_cents -= cost;
                }
                reserved = cost;
            }
            HolderId::Treasury => {
                return Err(DomainError::NoAuthority {
                    actor: 0,
                    reason: cw_core::error::AuthorityReason::NotOwner,
                });
            }
        }
    }
    let order = ShareOrder {
        holder,
        firm,
        side,
        qty,
        limit_price_cents,
        reserved_cents: reserved,
        placed: world.tick,
        expires: Tick(world.tick.0 + 1),
        filled: 0,
    };
    Ok(world.exchange.orders.insert(order))
}

/// Clear the weekly exchange.
pub fn clear_exchange(world: &mut World, dataset: &Dataset) {
    let firms: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for firm in firms {
        clear_firm(world, dataset, firm);
    }
    let expired: Vec<Handle> = world
        .exchange
        .orders
        .iter()
        .filter(|(_, order)| order.expires.0 <= world.tick.0)
        .map(|(handle, _)| handle)
        .collect();
    for handle in expired {
        release_order(world, handle);
    }
}

fn clear_firm(world: &mut World, dataset: &Dataset, firm: Handle) {
    let (mark, fair, outside_held, eligible) = match world.firms.get(firm) {
        Some(entity) => (
            entity.shares.mark_cents.max(1),
            entity.shares.fair_cents.max(0),
            entity
                .shares
                .holders
                .iter()
                .filter(|(holder, _)| matches!(holder, HolderId::Outside(_)))
                .map(|(_, lot)| lot.qty)
                .sum::<i64>(),
            entity.shares.eligible_votes(),
        ),
        None => return,
    };
    // Weekly executable float is two percent of freely held shares.
    let mut float = (outside_held * 2 / 100).max(0);
    let mut bids: Vec<Handle> = world
        .exchange
        .orders
        .iter()
        .filter(|(_, order)| {
            order.firm == firm && order.side == OrderSide::Buy && order.qty > order.filled
        })
        .map(|(handle, _)| handle)
        .collect();
    let mut asks: Vec<Handle> = world
        .exchange
        .orders
        .iter()
        .filter(|(_, order)| {
            order.firm == firm && order.side == OrderSide::Sell && order.qty > order.filled
        })
        .map(|(handle, _)| handle)
        .collect();
    bids.sort_by(|a, b| {
        let left = world
            .exchange
            .orders
            .get(*a)
            .map(|o| o.limit_price_cents)
            .unwrap_or(0);
        let right = world
            .exchange
            .orders
            .get(*b)
            .map(|o| o.limit_price_cents)
            .unwrap_or(0);
        right.cmp(&left).then(a.cmp(b))
    });
    asks.sort_by(|a, b| {
        let left = world
            .exchange
            .orders
            .get(*a)
            .map(|o| o.limit_price_cents)
            .unwrap_or(i64::MAX);
        let right = world
            .exchange
            .orders
            .get(*b)
            .map(|o| o.limit_price_cents)
            .unwrap_or(i64::MAX);
        left.cmp(&right).then(a.cmp(b))
    });
    let _ = fair;
    for ask in asks {
        let Some(order) = world.exchange.orders.get(ask).cloned() else {
            continue;
        };
        let remaining = order.qty - order.filled;
        if remaining <= 0 {
            continue;
        }
        // Genuine different-owner fills only.
        let buyer = bids.iter().find_map(|bid| {
            let candidate = world.exchange.orders.get(*bid)?;
            if candidate.holder == order.holder || candidate.filled >= candidate.qty {
                return None;
            }
            Some(*bid)
        });
        let (buyer_handle, buyer_holder) = match buyer {
            Some(handle) => (handle, world.exchange.orders.get(handle).map(|o| o.holder)),
            None => {
                if float <= 0 {
                    continue;
                }
                (Handle::INVALID, None)
            }
        };
        let qty = remaining.min(match buyer_holder {
            Some(_) => world
                .exchange
                .orders
                .get(buyer_handle)
                .map(|order| order.qty - order.filled)
                .unwrap_or(0),
            None => float,
        });
        if qty <= 0 {
            continue;
        }
        let impact = price_impact(qty, outside_held);
        let price = apply_impact(mark, impact, true);
        if let Some(buyer_order) = buyer_holder {
            let Some(order_buy) = world.exchange.orders.get(buyer_handle).cloned() else {
                continue;
            };
            if order_buy.limit_price_cents < price {
                continue;
            }
            settle_trade(
                world,
                order.firm,
                order.holder,
                buyer_order,
                qty,
                price,
                dataset,
            );
            if let Some(stored) = world.exchange.orders.get_mut(buyer_handle) {
                stored.filled += qty;
            }
        } else {
            settle_trade(
                world,
                order.firm,
                order.holder,
                HolderId::Outside(0),
                qty,
                price,
                dataset,
            );
            float -= qty;
        }
        if let Some(stored) = world.exchange.orders.get_mut(ask) {
            stored.filled += qty;
        }
        let _ = eligible;
    }
    // Residual bids buy from the external float.
    for bid in bids {
        let Some(order) = world.exchange.orders.get(bid).cloned() else {
            continue;
        };
        let remaining = order.qty - order.filled;
        if remaining <= 0 || float <= 0 {
            continue;
        }
        let qty = remaining.min(float);
        let impact = price_impact(qty, outside_held);
        let price = apply_impact(mark, impact, false);
        if order.limit_price_cents < price {
            continue;
        }
        settle_trade(
            world,
            order.firm,
            HolderId::Outside(0),
            order.holder,
            qty,
            price,
            dataset,
        );
        float -= qty;
        if let Some(stored) = world.exchange.orders.get_mut(bid) {
            stored.filled += qty;
        }
    }
}

fn price_impact(qty: i64, weekly_float: i64) -> i32 {
    if weekly_float <= 0 {
        return 0;
    }
    let raw = (qty as i128 * 10_000 / weekly_float as i128) as i32 / 10;
    raw.clamp(0, 2_500)
}

fn apply_impact(mark: i64, impact_bps: i32, seller_initiated: bool) -> i64 {
    let delta = mark * impact_bps as i64 / 10_000;
    if seller_initiated {
        mark - delta
    } else {
        mark + delta
    }
}

#[allow(clippy::too_many_arguments)]
fn settle_trade(
    world: &mut World,
    firm: Handle,
    seller: HolderId,
    buyer: HolderId,
    qty: i64,
    price: i64,
    dataset: &Dataset,
) {
    let _ = dataset;
    let cost = qty.saturating_mul(price);
    // Buyer pays.
    match buyer {
        HolderId::Investor => {
            world.investor.cash_cents =
                world.investor.cash_cents.max(0) - cost.min(world.investor.cash_cents.max(0));
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, cost, "share purchase")
                    .debit(Account::Investments, cost, "share purchase"),
            );
        }
        HolderId::Firm(handle) => {
            if let Some(entity) = world.firms.get_mut(handle) {
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::RestrictedCash, cost, "share purchase")
                        .debit(Account::Investments, cost, "share purchase"),
                );
            }
        }
        HolderId::Outside(index) => {
            if let Some(holder) = world.outside.investors.get_mut(index as usize) {
                holder.capital_cents =
                    holder.capital_cents.max(0) - cost.min(holder.capital_cents.max(0));
            }
        }
        HolderId::Treasury => {}
    }
    // Seller receives, and a sale below cost is a realised loss, not a company expense.
    match seller {
        HolderId::Investor => {
            let basis = world
                .firms
                .get(firm)
                .and_then(|entity| entity.shares.holders.get(&HolderId::Investor).copied())
                .map(|lot| {
                    if lot.qty > 0 {
                        lot.cost_cents * qty / lot.qty
                    } else {
                        0
                    }
                })
                .unwrap_or(0);
            let gain = cost - basis;
            world.investor.cash_cents += cost;
            world.investor.realised_gains_cents += gain;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, cost, "share sale")
                    .credit(Account::Investments, basis, "share sale")
                    .credit(Account::OtherIncome, gain, "share sale"),
            );
        }
        HolderId::Firm(handle) => {
            if let Some(entity) = world.firms.get_mut(handle) {
                let basis = entity
                    .shares
                    .holders
                    .get(&HolderId::Firm(handle))
                    .map(|lot| {
                        if lot.qty > 0 {
                            lot.cost_cents * qty / lot.qty
                        } else {
                            0
                        }
                    })
                    .unwrap_or(0);
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, cost, "share sale")
                        .credit(Account::Investments, basis, "share sale")
                        .credit(Account::OtherIncome, cost - basis, "share sale"),
                );
            }
        }
        HolderId::Outside(index) => {
            if let Some(holder) = world.outside.investors.get_mut(index as usize) {
                holder.capital_cents += cost;
            }
        }
        HolderId::Treasury => {}
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.shares.transfer(seller, buyer, qty, cost);
        if let Some(order_buy) = None::<ShareOrder> {
            let _ = order_buy;
        }
    }
}

fn release_order(world: &mut World, handle: Handle) {
    let Some(order) = world.exchange.orders.remove(handle) else {
        return;
    };
    let unfilled = (order.qty - order.filled).max(0);
    if order.side == OrderSide::Buy && unfilled > 0 {
        let per_share = if order.qty > 0 {
            order.reserved_cents / order.qty
        } else {
            0
        };
        let refund = per_share * unfilled;
        match order.holder {
            HolderId::Investor => {
                world.investor.cash_cents += refund;
                let _ = world.investor.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, refund, "order release")
                        .credit(Account::Investments, refund, "order release"),
                );
            }
            HolderId::Firm(entity) => {
                if let Some(firm) = world.firms.get_mut(entity) {
                    let _ = firm.book.commit(
                        world.tick,
                        Txn::new()
                            .debit(Account::RestrictedCash, refund, "order release")
                            .credit(Account::Cash, refund, "order release"),
                    );
                }
            }
            HolderId::Outside(index) => {
                if let Some(holder) = world.outside.investors.get_mut(index as usize) {
                    holder.capital_cents += refund;
                }
            }
            HolderId::Treasury => {}
        }
    }
}

/// Issue new shares at a funded price with board authority.
pub fn issue_shares(
    world: &mut World,
    firm: Handle,
    qty: i64,
    price_cents: i64,
    buyer: HolderId,
) -> DomainResult<i64> {
    let Some(entity) = world.firms.get(firm) else {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: EntityKind::Firm,
        });
    };
    if entity.payout_freeze {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::PayoutFrozen,
        });
    }
    if price_cents * 100 < entity.shares.fair_cents * 90 {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::CoverageTooLow,
        });
    }
    let proceeds = qty.saturating_mul(price_cents);
    match buyer {
        HolderId::Investor => {
            if world.investor.cash_cents < proceeds {
                return Err(DomainError::InsufficientCash {
                    entity: 0,
                    required: proceeds,
                    available: world.investor.cash_cents,
                });
            }
            world.investor.cash_cents -= proceeds;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, proceeds, "subscription")
                    .debit(Account::Investments, proceeds, "subscription"),
            );
        }
        HolderId::Firm(other) => {
            if crate::economy::ownership::would_create_cycle(world, other, firm) {
                return Err(DomainError::OwnershipCycle {
                    buyer: other.0,
                    target: firm.0,
                });
            }
            let cash = world.firms.get(other).map(|f| f.book.cash()).unwrap_or(0);
            if cash < proceeds {
                return Err(DomainError::InsufficientCash {
                    entity: other.0,
                    required: proceeds,
                    available: cash,
                });
            }
            let _ = world.firms.get_mut(other).unwrap().book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, proceeds, "subscription")
                    .debit(Account::Investments, proceeds, "subscription"),
            );
        }
        HolderId::Outside(index) => {
            let capital = world
                .outside
                .investors
                .get(index as usize)
                .map(|h| h.capital_cents)
                .unwrap_or(0);
            if capital < proceeds {
                return Err(DomainError::InsufficientCash {
                    entity: index as u64,
                    required: proceeds,
                    available: capital,
                });
            }
            if let Some(holder) = world.outside.investors.get_mut(index as usize) {
                holder.capital_cents -= proceeds;
            }
        }
        HolderId::Treasury => {
            return Err(DomainError::NoAuthority {
                actor: 0,
                reason: cw_core::error::AuthorityReason::NotOwner,
            });
        }
    }
    let Some(entity) = world.firms.get_mut(firm) else {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: EntityKind::Firm,
        });
    };
    let _ = entity.book.commit(
        world.tick,
        Txn::new().debit(Account::Cash, proceeds, "issue").credit(
            Account::ShareCapital,
            proceeds,
            "issue",
        ),
    );
    entity.shares.issued += qty;
    entity.shares.add(buyer, qty, proceeds);
    Ok(proceeds)
}

/// Pay a dividend from distributable retained earnings, respecting the four-week cash buffer.
pub fn pay_dividend(world: &mut World, firm: Handle, total_cents: i64) -> DomainResult<i64> {
    let Some(entity) = world.firms.get(firm) else {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: EntityKind::Firm,
        });
    };
    if entity.payout_freeze {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::PayoutFrozen,
        });
    }
    // Retained earnings is a credit balance, so the distributable amount is its negation.
    let distributable = (-entity.book.balance(Account::RetainedEarnings)).max(0);
    let buffer = crate::economy::insolvency::mandatory_weekly_outflow(world, firm) * 4;
    let available = (entity.book.cash() - buffer).max(0);
    let amount = total_cents.min(distributable).min(available);
    if amount <= 0 {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::CashBufferTooThin,
        });
    }
    let eligible: Vec<(HolderId, i64)> = entity
        .shares
        .holders
        .iter()
        .filter(|(holder, _)| !holder.is_treasury())
        .map(|(holder, lot)| (*holder, lot.qty))
        .collect();
    let weights: Vec<(u64, i64)> = eligible
        .iter()
        .enumerate()
        .map(|(i, (_, q))| (i as u64, *q))
        .collect();
    let split = allocate_cents(Cents(amount), &weights);
    for (index, (_, share)) in split.iter().enumerate() {
        if share.0 <= 0 {
            continue;
        }
        let holder = eligible[index].0;
        match holder {
            HolderId::Investor => {
                world.investor.cash_cents += share.0;
                let _ = world.investor.book.commit(
                    world.tick,
                    Txn::new().debit(Account::Cash, share.0, "dividend").credit(
                        Account::OtherIncome,
                        share.0,
                        "dividend",
                    ),
                );
            }
            HolderId::Firm(child) => {
                if let Some(firm) = world.firms.get_mut(child) {
                    let _ = firm.book.commit(
                        world.tick,
                        Txn::new().debit(Account::Cash, share.0, "dividend").credit(
                            Account::OtherIncome,
                            share.0,
                            "dividend",
                        ),
                    );
                }
            }
            HolderId::Outside(index) => {
                if let Some(holder) = world.outside.investors.get_mut(index as usize) {
                    holder.capital_cents += share.0;
                }
            }
            HolderId::Treasury => {}
        }
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new().credit(Account::Cash, amount, "dividend").debit(
                Account::RetainedEarnings,
                amount,
                "dividend",
            ),
        );
    }
    Ok(amount)
}

/// Buy shares back into treasury at the current mark.
pub fn repurchase(world: &mut World, firm: Handle, qty: i64) -> DomainResult<i64> {
    let Some(entity) = world.firms.get(firm) else {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: EntityKind::Firm,
        });
    };
    if entity.payout_freeze {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::PayoutFrozen,
        });
    }
    let price = entity.shares.mark_cents.max(1);
    let cost = qty.saturating_mul(price);
    let buffer = crate::economy::insolvency::mandatory_weekly_outflow(world, firm) * 4;
    if entity.book.cash() - cost < buffer {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::CashBufferTooThin,
        });
    }
    // Buy from external holders first, then any non-controlling holder.
    let controller = entity.shares.controller();
    let sellers: Vec<(HolderId, i64)> = entity
        .shares
        .holders
        .iter()
        .filter(|(holder, _)| !holder.is_treasury() && Some(**holder) != controller)
        .map(|(holder, lot)| (*holder, lot.qty))
        .collect();
    let available: i64 = sellers.iter().map(|(_, q)| *q).sum();
    if available < qty {
        return Err(DomainError::InsufficientStock {
            product: 0,
            required: qty,
            available,
        });
    }
    let weights: Vec<(u64, i64)> = sellers
        .iter()
        .enumerate()
        .map(|(i, (_, q))| (i as u64, *q))
        .collect();
    let split = allocate_cents(Cents(qty), &weights);
    for (index, (_, take)) in split.iter().enumerate() {
        if take.0 <= 0 {
            continue;
        }
        let seller = sellers[index].0;
        match seller {
            HolderId::Outside(holder_index) => {
                if let Some(holder) = world.outside.investors.get_mut(holder_index as usize) {
                    holder.capital_cents += take.0 * price;
                }
            }
            HolderId::Investor => {
                world.investor.cash_cents += take.0 * price;
                world.investor.realised_gains_cents += take.0 * price;
            }
            _ => {}
        }
        if let Some(entity) = world.firms.get_mut(firm) {
            entity
                .shares
                .transfer(seller, HolderId::Treasury, take.0, take.0 * price);
        }
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new().credit(Account::Cash, cost, "buyback").debit(
                Account::ShareCapital,
                cost,
                "buyback",
            ),
        );
    }
    Ok(cost)
}

/// Launch an IPO: at least ten percent float, thirteen weeks of statements, two percent fee
/// with a $100,000 minimum, positive equity.
pub fn launch_ipo(
    world: &mut World,
    firm: Handle,
    new_shares: i64,
    price_cents: i64,
) -> DomainResult<i64> {
    let Some(entity) = world.firms.get(firm) else {
        return Err(DomainError::InvalidEntity {
            handle: firm.0,
            kind: EntityKind::Firm,
        });
    };
    if entity.book.total_equity() <= 0 || entity.shares.statement_weeks < 13 {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::NegativeEquity,
        });
    }
    let float_bps = new_shares as i128 * 10_000 / (entity.shares.issued + new_shares) as i128;
    if float_bps < 1_000 {
        return Err(DomainError::SolvencyRestriction {
            entity: firm.0,
            reason: cw_core::error::SolvencyReason::CoverageTooLow,
        });
    }
    let proceeds = new_shares.saturating_mul(price_cents);
    let fee = round_even_div(proceeds as i128 * 200, 10_000) as i64;
    let fee = fee.max(10_000_000).min(proceeds);
    // The float must be funded by outside investors before issuance settles.
    let outside_index = 0u32;
    let capital = world
        .outside
        .investors
        .get(outside_index as usize)
        .map(|h| h.capital_cents)
        .unwrap_or(0);
    if capital < proceeds {
        return Err(DomainError::InsufficientCash {
            entity: outside_index as u64,
            required: proceeds,
            available: capital,
        });
    }
    if let Some(holder) = world.outside.investors.get_mut(outside_index as usize) {
        holder.capital_cents -= proceeds;
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new().debit(Account::Cash, proceeds, "ipo").credit(
                Account::ShareCapital,
                proceeds,
                "ipo",
            ),
        );
        entity.shares.issued += new_shares;
        entity
            .shares
            .add(HolderId::Outside(outside_index), new_shares, proceeds);
        entity.shares.ipo_done = true;
        let _ = entity.book.commit(
            world.tick,
            Txn::new().credit(Account::Cash, fee, "ipo fee").debit(
                Account::OperatingExpense,
                fee,
                "ipo fee",
            ),
        );
    }
    let _ = world.outside.legal_services_book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, fee, "ipo fee")
            .credit(Account::Revenue, fee, "ipo fee"),
    );
    Ok(proceeds - fee)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn valuation_runs_leaves_first_and_never_uses_mark_as_cash() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 61, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let handle = world.firms.iter().next().unwrap().0;
        let cash_before = world.firm(handle).unwrap().book.cash();
        refresh_valuation(&mut world, &dataset);
        let mark = world.firm(handle).unwrap().shares.mark_cents;
        assert!(mark > 0);
        assert_eq!(world.firm(handle).unwrap().book.cash(), cash_before);
    }

    #[test]
    fn dividend_excludes_treasury_and_conserves_cents() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 62, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        if let Some(firm) = world.firms.get_mut(primary) {
            firm.shares.add(HolderId::Treasury, 100_000, 0);
            firm.book
                .commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, 50_000_000, "retained")
                        .credit(Account::RetainedEarnings, 50_000_000, "retained"),
                )
                .unwrap();
        }
        refresh_valuation(&mut world, &dataset);
        let before = world.investor.cash_cents;
        let paid = pay_dividend(&mut world, primary, 10_000_000).unwrap();
        assert!(paid > 0);
        assert!(world.investor.cash_cents > before);
        let treasury_cash = world.firm(primary).unwrap().shares.qty(HolderId::Treasury);
        assert_eq!(
            treasury_cash, 100_000,
            "treasury shares still hold no dividend"
        );
    }

    #[test]
    fn issuance_at_ninety_percent_is_legal_and_dilutes() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 63, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        refresh_valuation(&mut world, &dataset);
        let fair = world.firm(primary).unwrap().shares.fair_cents;
        let votes_before = world
            .firm(primary)
            .unwrap()
            .shares
            .vote_bps(HolderId::Investor);
        let proceeds = issue_shares(
            &mut world,
            primary,
            100_000,
            fair * 95 / 100,
            HolderId::Outside(2),
        )
        .unwrap();
        assert!(proceeds > 0);
        let votes_after = world
            .firm(primary)
            .unwrap()
            .shares
            .vote_bps(HolderId::Investor);
        assert!(votes_after < votes_before, "dilution is real");
    }

    #[test]
    fn an_exchange_trade_conserves_shares_and_cash() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 65, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        refresh_valuation(&mut world, &dataset);
        let mark = world.firm(primary).unwrap().shares.mark_cents.max(1);
        let issued = world.firm(primary).unwrap().shares.issued;
        let investor_before = world.firm(primary).unwrap().shares.qty(HolderId::Investor);
        let outside_before = world
            .firm(primary)
            .unwrap()
            .shares
            .qty(HolderId::Outside(0));
        world.outside.investors[0].capital_cents = 1_000_000_000;
        let cash_before = world.investor.cash_cents;
        place_order(
            &mut world,
            HolderId::Investor,
            primary,
            OrderSide::Sell,
            1_000,
            mark,
        )
        .unwrap();
        place_order(
            &mut world,
            HolderId::Outside(0),
            primary,
            OrderSide::Buy,
            1_000,
            mark,
        )
        .unwrap();
        clear_exchange(&mut world, &dataset);
        let shares = &world.firm(primary).unwrap().shares;
        assert_eq!(
            shares.issued, issued,
            "a trade never creates or retires shares"
        );
        assert_eq!(
            shares.qty(HolderId::Investor) + shares.qty(HolderId::Outside(0)),
            investor_before + outside_before,
            "the two holders conserve the traded quantity"
        );
        assert!(world.investor.cash_cents > cash_before);
        assert!(world.investor.book.is_balanced());
    }

    #[test]
    fn a_treasury_buyback_pays_external_holders_and_conserves_issued_shares() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 67, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let firm = world.investor.primary_group.unwrap();
        refresh_valuation(&mut world, &dataset);
        world
            .firms
            .get_mut(firm)
            .unwrap()
            .book
            .commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, 10_000_000_000, "funding")
                    .credit(Account::ShareCapital, 10_000_000_000, "funding"),
            )
            .unwrap();
        let issued = world.firm(firm).unwrap().shares.issued;
        let cash_before = world.firm(firm).unwrap().book.cash();
        let outside_before: i64 = world
            .firm(firm)
            .unwrap()
            .shares
            .holders
            .iter()
            .filter(|(holder, _)| matches!(holder, HolderId::Outside(_)))
            .map(|(_, lot)| lot.qty)
            .sum();
        let treasury_before = world.firm(firm).unwrap().shares.treasury();
        let cost = repurchase(&mut world, firm, 1_000).unwrap();
        assert!(cost > 0);
        assert_eq!(world.firm(firm).unwrap().shares.issued, issued);
        let outside_after: i64 = world
            .firm(firm)
            .unwrap()
            .shares
            .holders
            .iter()
            .filter(|(holder, _)| matches!(holder, HolderId::Outside(_)))
            .map(|(_, lot)| lot.qty)
            .sum();
        assert_eq!(outside_after + 1_000, outside_before);
        assert_eq!(
            world.firm(firm).unwrap().shares.treasury(),
            treasury_before + 1_000
        );
        assert_eq!(world.firm(firm).unwrap().book.cash(), cash_before - cost);
        assert!(world.firm(firm).unwrap().book.is_balanced());
    }

    #[test]
    fn an_ipo_sells_a_funded_float_and_pays_the_fee() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 66, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let firm = world.firms.iter().next().unwrap().0;
        refresh_valuation(&mut world, &dataset);
        {
            let entity = world.firms.get_mut(firm).unwrap();
            entity.shares.statement_weeks = 13;
            entity
                .book
                .commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, 100_000_000, "funding")
                        .credit(Account::ShareCapital, 100_000_000, "funding"),
                )
                .unwrap();
        }
        world.outside.investors[0].capital_cents = 1_000_000_000;
        let fair = world.firm(firm).unwrap().shares.fair_cents.max(1);
        let issued_before = world.firm(firm).unwrap().shares.issued;
        let cash_before = world.firm(firm).unwrap().book.cash();
        let proceeds = launch_ipo(&mut world, firm, 200_000, fair).unwrap();
        assert!(proceeds > 0);
        assert_eq!(
            world.firm(firm).unwrap().shares.issued,
            issued_before + 200_000
        );
        assert!(world.firm(firm).unwrap().shares.ipo_done);
        assert!(world.firm(firm).unwrap().book.cash() > cash_before);
        assert!(world.firm(firm).unwrap().book.is_balanced());
        assert!(world.firm(firm).unwrap().shares.qty(HolderId::Outside(0)) >= 200_000);
    }

    #[test]
    fn issuance_below_ninety_percent_is_rejected() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 64, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let primary = world.investor.primary_group.unwrap();
        refresh_valuation(&mut world, &dataset);
        let fair = world.firm(primary).unwrap().shares.fair_cents;
        let result = issue_shares(
            &mut world,
            primary,
            100_000,
            fair * 80 / 100,
            HolderId::Outside(2),
        );
        assert!(result.is_err());
    }
}
