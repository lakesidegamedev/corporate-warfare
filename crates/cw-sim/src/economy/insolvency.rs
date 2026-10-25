//! Solvency, restructuring and liquidation.
//!
//! A missed payment becomes a past-due liability; cash never silently becomes an unbounded
//! negative balance. Payouts freeze for a four-week grace. After grace the firm either
//! restructures at 50 percent principal over four quarters when that beats estimated
//! liquidation recovery, or is liquidated at 40 percent carrying value through finite outside
//! buyer funds. Creditors are paid by seniority. Equity is paid last and usually nothing.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Ledger, Txn};
use cw_core::money::round_even_div;
use cw_data::Dataset;

use crate::state::{FirmStatus, HolderId, PAST_DUE_GRACE_WEEKS, World};

/// Recognise an expense that cannot be paid, and carry it as past due.
pub fn defer_expense(world: &mut World, firm: Handle, amount: i64, expense: Account, memo: &str) {
    if amount <= 0 {
        return;
    }
    let Some(entity) = world.firms.get_mut(firm) else {
        return;
    };
    let _ = entity.book.commit(
        world.tick,
        Txn::new()
            .debit(expense, amount, memo)
            .credit(Account::OverduePayables, amount, memo),
    );
    mark_past_due(entity, world.tick);
}

fn mark_past_due(firm: &mut crate::state::Firm, now: Tick) {
    // A dissolved, liquidating or merged firm cannot re-enter the payment-grace state. Late
    // interest or contract postings used to resurrect it, which hid a liquidation from reports.
    if matches!(
        firm.status,
        FirmStatus::Dissolved | FirmStatus::Liquidating | FirmStatus::Merged { .. }
    ) {
        return;
    }
    firm.payout_freeze = true;
    if !matches!(firm.status, FirmStatus::PastDue { .. }) {
        firm.status = FirmStatus::PastDue { since: now };
        firm.past_due_weeks = 0;
    }
}

/// Pay a due amount from free cash, deferring whatever cannot be paid.
pub fn pay_or_defer(world: &mut World, firm: Handle, amount: i64, expense: Account, memo: &str) {
    if amount <= 0 {
        return;
    }
    let cash = world
        .firms
        .get(firm)
        .map(|entity| entity.book.cash())
        .unwrap_or(0)
        .max(0);
    let paid = amount.min(cash);
    if paid > 0
        && let Some(entity) = world.firms.get_mut(firm)
    {
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, paid, memo)
                .debit(expense, paid, memo),
        );
    }
    if paid < amount {
        defer_expense(world, firm, amount - paid, expense, memo);
    }
}

/// Weekly solvency step. Returns firms that entered restructuring or liquidation this week.
pub fn settle(world: &mut World, dataset: &Dataset) -> Vec<(Handle, &'static str)> {
    let mut events = Vec::new();
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let status = match world.firms.get(handle) {
            Some(firm) => firm.status,
            None => continue,
        };
        match status {
            FirmStatus::PastDue { .. } => {
                let overdue = -world
                    .firms
                    .get(handle)
                    .unwrap()
                    .book
                    .balance(Account::OverduePayables);
                if overdue <= 0 {
                    if let Some(firm) = world.firms.get_mut(handle) {
                        firm.status = FirmStatus::Operating;
                        firm.payout_freeze = false;
                        firm.past_due_weeks = 0;
                    }
                    continue;
                }
                let weeks = {
                    let firm = world.firms.get_mut(handle).unwrap();
                    firm.past_due_weeks += 1;
                    firm.past_due_weeks
                };
                if weeks > PAST_DUE_GRACE_WEEKS {
                    let liquid = crate::economy::valuation::liquidation_value(
                        &world.firms.get(handle).unwrap().book,
                    );
                    let debt = world.firms.get(handle).unwrap().book.debt();
                    if liquid * 100 >= debt * 50 {
                        restructure(world, handle, debt);
                        events.push((handle, "restructuring"));
                    } else {
                        liquidate(world, handle, dataset);
                        events.push((handle, "liquidation"));
                    }
                }
            }
            FirmStatus::Restructuring { until } if world.tick.0 >= until.0 => {
                if let Some(firm) = world.firms.get_mut(handle) {
                    firm.status = FirmStatus::Operating;
                    firm.payout_freeze = false;
                    firm.past_due_weeks = 0;
                }
                events.push((handle, "restructured"));
            }
            _ => {}
        }
    }
    events
}

/// Public entry for an approved restructuring command.
pub fn restructure_for_owner(world: &mut World, firm: Handle, debt: i64) {
    restructure(world, firm, debt);
}

fn restructure(world: &mut World, firm: Handle, debt: i64) {
    if debt <= 0 {
        if let Some(entity) = world.firms.get_mut(firm) {
            entity.status = FirmStatus::Operating;
            entity.payout_freeze = false;
        }
        return;
    }
    // Creditors accept 50 percent principal over four quarters; the haircut is a real loss.
    let haircut = debt * 50 / 100;
    if let Some(entity) = world.firms.get_mut(firm) {
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Loans, debt, "restructure")
                .credit(Account::OtherIncome, haircut, "debt forgiveness")
                .credit(
                    Account::OverduePayables,
                    debt - haircut,
                    "restructured principal",
                ),
        );
        entity.status = FirmStatus::Restructuring {
            until: Tick(world.tick.0 + 52),
        };
        entity.past_due_weeks = 0;
    }
}

/// Liquidate assets at 40 percent carrying value and distribute by seniority.
fn liquidate(world: &mut World, firm: Handle, dataset: &Dataset) {
    let book = match world.firms.get(firm) {
        Some(entity) => entity.book.clone(),
        None => return,
    };
    let assets = book.total_assets().max(0) * 40 / 100;
    // Finite outside buyer funds: a sale without a buyer delays, never mints cash.
    let buyer_cash: i64 = world
        .outside
        .lenders
        .iter()
        .map(|lender| lender.capital_cents)
        .sum();
    let proceeds = assets.min(buyer_cash);
    let mut remaining = proceeds;

    // Priority: secured claims (loans with collateral) then payroll/administrative then
    // unsecured (deposits, bonds, ordinary loans) then equity.
    let secured: i64 = world
        .firms
        .get(firm)
        .map(|entity| {
            entity
                .loans
                .iter()
                .filter_map(|handle| world.loans.get(*handle))
                .filter(|loan| loan.secured)
                .map(|loan| loan.principal_remaining_cents)
                .sum()
        })
        .unwrap_or(0);
    let secured_paid = remaining.min(secured);
    remaining -= secured_paid;

    let overdue = -book.balance(Account::OverduePayables);
    let administrative = overdue.max(0);
    let admin_paid = remaining.min(administrative);
    remaining -= admin_paid;

    let unsecured = -book.balance(Account::Loans)
        - book.balance(Account::Bonds)
        - book.balance(Account::Deposits);
    let unsecured = unsecured.max(0) - secured;
    let unsecured_paid = remaining.min(unsecured);
    let _ = unsecured_paid;
    let _ = remaining;

    // A failed firm's unrelated cash is never taken: only its own assets are realised.
    if let Some(entity) = world.firms.get_mut(firm) {
        let carrying = book.balance(Account::FixedAssets)
            + book.balance(Account::Inventory)
            + book.balance(Account::Receivables)
            + book.balance(Account::OtherAssets)
            + book.balance(Account::WorkInProgress);
        let loss = (carrying - proceeds).max(0);
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, proceeds, "liquidation proceeds")
                .debit(Account::OtherIncome, loss, "liquidation loss")
                .credit(
                    Account::FixedAssets,
                    book.balance(Account::FixedAssets),
                    "asset sale",
                )
                .credit(
                    Account::Inventory,
                    book.balance(Account::Inventory),
                    "stock sale",
                )
                .credit(
                    Account::Receivables,
                    book.balance(Account::Receivables),
                    "receivable sale",
                )
                .credit(
                    Account::OtherAssets,
                    book.balance(Account::OtherAssets),
                    "other asset sale",
                )
                .credit(
                    Account::WorkInProgress,
                    book.balance(Account::WorkInProgress),
                    "cip sale",
                ),
        );
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .credit(
                    Account::Cash,
                    secured_paid + admin_paid + unsecured_paid,
                    "creditor payment",
                )
                .debit(Account::Loans, secured_paid, "secured")
                .debit(Account::OverduePayables, admin_paid, "administrative")
                .debit(Account::Loans, unsecured_paid, "unsecured"),
        );
        entity.status = FirmStatus::Liquidating;
    }
    // Explicit guarantees: a guarantor covers the failed firm's unpaid claims up to its
    // contracted cap, paid from its own cash or carried as its own past-due liability. There is
    // no implied guarantee for siblings.
    let unpaid = (-world
        .firms
        .get(firm)
        .map(|entity| {
            entity.book.balance(Account::Loans)
                + entity.book.balance(Account::Bonds)
                + entity.book.balance(Account::Deposits)
                + entity.book.balance(Account::OverduePayables)
        })
        .unwrap_or(0))
    .max(0);
    if unpaid > 0 {
        let mut guarantees: Vec<(Handle, i64, i64)> = Vec::new();
        for (handle, candidate) in world.firms.iter() {
            if handle == firm || !candidate.operative() {
                continue;
            }
            for guarantee in &candidate.guarantees {
                if guarantee.beneficiary == firm {
                    guarantees.push((handle, guarantee.claim_cents, guarantee.max_cents));
                }
            }
        }
        let mut remaining = unpaid;
        for (guarantor, claim, cap) in guarantees {
            if remaining <= 0 {
                break;
            }
            let cover = remaining.min(claim.max(0)).min(cap.max(0));
            let cash = world
                .firm(guarantor)
                .map(|entity| entity.book.cash())
                .unwrap_or(0)
                .max(0);
            let paid = cover.min(cash);
            if paid > 0 {
                if let Some(entity) = world.firms.get_mut(guarantor) {
                    let _ = entity.book.commit(
                        world.tick,
                        Txn::new().credit(Account::Cash, paid, "guarantee").debit(
                            Account::OperatingExpense,
                            paid,
                            "guarantee",
                        ),
                    );
                }
                if let Some(entity) = world.firms.get_mut(firm) {
                    let _ = entity.book.commit(
                        world.tick,
                        Txn::new()
                            .debit(Account::OverduePayables, paid, "guarantee received")
                            .credit(Account::OtherIncome, paid, "guarantee received"),
                    );
                }
                remaining -= paid;
            }
        }
    }

    // Investor loses the holding; career losses are recorded, other assets are untouched.
    let held = world
        .firms
        .get(firm)
        .map(|f| f.shares.qty(HolderId::Investor))
        .unwrap_or(0);
    if held > 0 {
        let lost = held
            * world
                .firms
                .get(firm)
                .map(|f| f.shares.mark_cents)
                .unwrap_or(0);
        world.investor.career_losses_cents += lost;
        if let Some(entity) = world.firms.get_mut(firm) {
            entity.shares.holders.remove(&HolderId::Investor);
        }
        if world.investor.primary_group == Some(firm) {
            world.investor.primary_group = None;
        }
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.status = FirmStatus::Dissolved;
        entity.facilities.clear();
    }
    let facilities: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(_, facility)| facility.firm == firm)
        .map(|(handle, _)| handle)
        .collect();
    for handle in facilities {
        world.facilities.remove(handle);
    }
    let _ = dataset;
}

/// Move an unpaid tax balance into past-due liabilities.
pub fn record_missed_tax(world: &mut World, firm: Handle, amount: i64) {
    if amount <= 0 {
        return;
    }
    let Some(entity) = world.firms.get_mut(firm) else {
        return;
    };
    let _ = entity.book.commit(
        world.tick,
        Txn::new()
            .debit(Account::TaxesPayable, amount, "unpaid tax")
            .credit(Account::OverduePayables, amount, "unpaid tax"),
    );
    mark_past_due(entity, world.tick);
}

/// Total past-due liabilities of one firm.
pub fn overdue(ledger: &Ledger) -> i64 {
    -ledger.balance(Account::OverduePayables)
}

/// Amount a firm must reserve for statutory and debt obligations next week.
pub fn mandatory_weekly_outflow(world: &World, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let payroll: i64 = entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .map(|facility| {
            facility
                .workers
                .iter()
                .enumerate()
                .map(|(tier, workers)| workers * entity.policies.wage_offer[tier])
                .sum::<i64>()
        })
        .sum();
    let debt: i64 = entity
        .loans
        .iter()
        .filter_map(|handle| world.loans.get(*handle))
        .map(|loan| {
            loan.weekly_principal_cents
                + round_even_div(
                    loan.principal_remaining_cents as i128 * loan.rate_bps as i128,
                    10_000 * 52,
                ) as i64
        })
        .sum();
    payroll + debt
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn missed_payment_becomes_past_due_not_negative_cash() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 31, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let handle = world.firms.iter().next().unwrap().0;
        defer_expense(&mut world, handle, 1_000_000, Account::Wages, "unpaid");
        let firm = world.firm(handle).unwrap();
        assert!(firm.book.cash() >= 0);
        assert_eq!(overdue(&firm.book), 1_000_000);
        assert!(matches!(firm.status, FirmStatus::PastDue { .. }));
    }

    #[test]
    fn grace_period_runs_four_weeks_before_resolution() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 32, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let handle = world.firms.iter().next().unwrap().0;
        defer_expense(&mut world, handle, 1_000_000, Account::Wages, "unpaid");
        for week in 1..=4 {
            world.tick = Tick(week);
            settle(&mut world, &dataset);
            assert!(matches!(
                world.firm(handle).unwrap().status,
                FirmStatus::PastDue { .. }
            ));
        }
        world.tick = Tick(5);
        settle(&mut world, &dataset);
        let status = world.firm(handle).unwrap().status;
        assert!(
            matches!(
                status,
                FirmStatus::Restructuring { .. } | FirmStatus::Dissolved
            ),
            "grace expired with {status:?}"
        );
    }
}
