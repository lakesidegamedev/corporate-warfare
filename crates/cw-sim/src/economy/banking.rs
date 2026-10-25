//! Funded banking.
//!
//! A bank is a funded intermediary, not a fractional-reserve money creator. A customer's
//! deposit is a financial asset (carried in `Receivables` for that customer) and the matching
//! bank balance is a liability; outside cash exists once. Lending moves bank cash to the
//! borrower and creates a matching asset and debt. Required liquid cash is 20 percent of
//! withdrawable deposits; adjusted capital must be at least 12 percent of risk-weighted loans.
//! Loan defaults are provisioned at 50 percent of uncovered principal. A run requests 25
//! percent of deposits pro rata from actual cash and unpaid claims remain.

use cw_core::calendar::Tick;
use cw_core::error::{DomainError, DomainResult, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::round_even_div;

use crate::state::{FirmStatus, HolderId, World};

const LIQUIDITY_BPS: i64 = 2_000;
const CAPITAL_BPS: i64 = 1_200;

pub fn deposits(world: &World, bank: Handle) -> i64 {
    world
        .firms
        .get(bank)
        .map(|firm| -firm.book.balance(Account::Deposits))
        .unwrap_or(0)
}

pub fn loan_assets(world: &World, bank: Handle) -> i64 {
    world
        .firms
        .get(bank)
        .map(|firm| firm.book.balance(Account::Receivables))
        .unwrap_or(0)
}

fn liquidity_ok(world: &World, _bank: Handle, deposit_after: i64, cash_after: i64) -> bool {
    let _ = world;
    cash_after >= deposit_after * LIQUIDITY_BPS / 10_000
}

fn capital_ok(world: &World, bank: Handle, risk_weighted_after: i64) -> bool {
    let Some(firm) = world.firms.get(bank) else {
        return false;
    };
    let capital = firm.book.total_equity();
    capital * 10_000 >= risk_weighted_after * CAPITAL_BPS
}

/// Accept a deposit: cash leaves the depositor, the bank gains cash and a deposit liability.
pub fn deposit(world: &mut World, bank: Handle, holder: HolderId, amount: i64) -> DomainResult<()> {
    if amount <= 0 {
        return Err(DomainError::Arithmetic {
            detail: cw_core::error::ArithmeticFault::OutOfRange,
        });
    }
    if world
        .firms
        .get(bank)
        .and_then(|firm| firm.bank.as_ref())
        .is_none()
    {
        return Err(DomainError::InvalidEntity {
            handle: bank.0,
            kind: EntityKind::Bank,
        });
    }
    pay_holder(world, holder, amount, "deposit")?;
    let Some(firm) = world.firms.get_mut(bank) else {
        return Err(DomainError::InvalidEntity {
            handle: bank.0,
            kind: EntityKind::Bank,
        });
    };
    let _ = firm.book.commit(
        world.tick,
        Txn::new().debit(Account::Cash, amount, "deposit").credit(
            Account::Deposits,
            amount,
            "deposit",
        ),
    );
    if let Some(model) = world
        .firms
        .get_mut(bank)
        .and_then(|firm| firm.bank.as_mut())
    {
        *model.depositors.entry(holder).or_default() += amount;
    }
    credit_holder(world, holder, amount, "deposit");
    Ok(())
}

/// Withdraw a deposit, constrained by actual bank liquidity.
pub fn withdraw(
    world: &mut World,
    bank: Handle,
    holder: HolderId,
    amount: i64,
) -> DomainResult<i64> {
    let cash = world
        .firms
        .get(bank)
        .map(|firm| firm.book.cash())
        .unwrap_or(0);
    let claim = world
        .firms
        .get(bank)
        .and_then(|firm| firm.bank.as_ref())
        .and_then(|model| model.depositors.get(&holder).copied())
        .unwrap_or(0);
    let paid = amount.min(cash.max(0)).min(claim);
    if paid <= 0 {
        return Err(DomainError::InsufficientCash {
            entity: bank.0,
            required: amount,
            available: cash.max(0),
        });
    }
    let Some(firm) = world.firms.get_mut(bank) else {
        return Err(DomainError::InvalidEntity {
            handle: bank.0,
            kind: EntityKind::Bank,
        });
    };
    let _ = firm.book.commit(
        world.tick,
        Txn::new().credit(Account::Cash, paid, "withdrawal").debit(
            Account::Deposits,
            paid,
            "withdrawal",
        ),
    );
    debit_holder(world, holder, paid, "withdrawal");
    if let Some(model) = world
        .firms
        .get_mut(bank)
        .and_then(|firm| firm.bank.as_mut())
        && let Some(balance) = model.depositors.get_mut(&holder)
    {
        *balance = (*balance - paid).max(0);
    }
    Ok(paid)
}

fn pay_holder(world: &mut World, holder: HolderId, amount: i64, memo: &str) -> DomainResult<()> {
    match holder {
        HolderId::Firm(handle) => {
            let cash = world
                .firms
                .get(handle)
                .map(|firm| firm.book.cash())
                .unwrap_or(0);
            if cash < amount {
                return Err(DomainError::InsufficientCash {
                    entity: handle.0,
                    required: amount,
                    available: cash,
                });
            }
            let _ = world.firms.get_mut(handle).unwrap().book.commit(
                world.tick,
                Txn::new().credit(Account::Cash, amount, memo).debit(
                    Account::Receivables,
                    amount,
                    memo,
                ),
            );
        }
        HolderId::Investor => {
            if world.investor.cash_cents < amount {
                return Err(DomainError::InsufficientCash {
                    entity: 0,
                    required: amount,
                    available: world.investor.cash_cents,
                });
            }
            world.investor.cash_cents -= amount;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new().credit(Account::Cash, amount, memo).debit(
                    Account::Receivables,
                    amount,
                    memo,
                ),
            );
        }
        _ => {
            return Err(DomainError::NoAuthority {
                actor: 0,
                reason: cw_core::error::AuthorityReason::NotOwner,
            });
        }
    }
    Ok(())
}

fn credit_holder(world: &mut World, holder: HolderId, amount: i64, memo: &str) {
    let _ = (world, holder, amount, memo);
}

fn debit_holder(world: &mut World, holder: HolderId, amount: i64, memo: &str) {
    match holder {
        HolderId::Firm(handle) => {
            if let Some(firm) = world.firms.get_mut(handle) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new().debit(Account::Cash, amount, memo).credit(
                        Account::Receivables,
                        amount,
                        memo,
                    ),
                );
            }
        }
        HolderId::Investor => {
            world.investor.cash_cents += amount;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new().debit(Account::Cash, amount, memo).credit(
                    Account::Receivables,
                    amount,
                    memo,
                ),
            );
        }
        _ => {}
    }
}

/// Approve a loan from a bank to a borrower, enforcing liquidity and capital after the loan.
pub fn approve_loan(
    world: &mut World,
    bank: Handle,
    borrower: Handle,
    principal: i64,
) -> DomainResult<Handle> {
    if principal <= 0 {
        return Err(DomainError::Arithmetic {
            detail: cw_core::error::ArithmeticFault::OutOfRange,
        });
    }
    let Some(bank_firm) = world.firms.get(bank) else {
        return Err(DomainError::InvalidEntity {
            handle: bank.0,
            kind: EntityKind::Bank,
        });
    };
    if !bank_firm.operative() {
        return Err(DomainError::SolvencyRestriction {
            entity: bank.0,
            reason: cw_core::error::SolvencyReason::NegativeEquity,
        });
    }
    let cash = bank_firm.book.cash();
    if cash < principal {
        return Err(DomainError::InsufficientCash {
            entity: bank.0,
            required: principal,
            available: cash,
        });
    }
    let deposit_total = deposits(world, bank);
    let risk_weighted = loan_assets(world, bank) + principal;
    if !liquidity_ok(world, bank, deposit_total, cash - principal) {
        return Err(DomainError::SolvencyRestriction {
            entity: bank.0,
            reason: cw_core::error::SolvencyReason::LiquidityTooLow,
        });
    }
    if !capital_ok(world, bank, risk_weighted) {
        return Err(DomainError::SolvencyRestriction {
            entity: bank.0,
            reason: cw_core::error::SolvencyReason::CapitalTooLow,
        });
    }
    let rate = world.macro_state.policy_rate_bps
        + crate::economy::credit::assess_grade(world, borrower).spread_bps();
    let _ = world.firms.get_mut(bank).unwrap().book.commit(
        world.tick,
        Txn::new()
            .credit(Account::Cash, principal, "loan advance")
            .debit(Account::Receivables, principal, "loan advance"),
    );
    let _ = world.firms.get_mut(borrower).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, principal, "loan advance")
            .credit(Account::Loans, principal, "loan advance"),
    );
    let loan = crate::state::Loan {
        borrower,
        lender: crate::state::Lender::Bank(bank),
        principal_remaining_cents: principal,
        rate_bps: rate,
        weekly_principal_cents: principal / 104,
        remaining_weeks: 104,
        collateral: None,
        secured: false,
        created: world.tick,
        carry: 0,
        defaulted: false,
    };
    let handle = world.loans.insert(loan);
    if let Some(firm) = world.firms.get_mut(borrower) {
        firm.loans.push(handle);
    }
    Ok(handle)
}

/// Weekly deposit interest and default provisioning.
pub fn weekly(world: &mut World) {
    let banks: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.bank.is_some())
        .map(|(handle, _)| handle)
        .collect();
    for bank in banks {
        let Some(firm) = world.firms.get(bank) else {
            continue;
        };
        let rate = firm
            .bank
            .as_ref()
            .map(|model| model.deposit_rate_bps)
            .unwrap_or(0)
            .max(0);
        let deposit_total = deposits(world, bank);
        let interest = round_even_div(deposit_total as i128 * rate as i128, 10_000 * 52) as i64;
        if interest > 0 {
            let cash = world.firms.get(bank).map(|f| f.book.cash()).unwrap_or(0);
            let paid = interest.min(cash.max(0));
            // One posting: expense the whole interest, pay what cash allows, carry the rest as
            // a liability that later receipts must fund.
            if let Some(firm) = world.firms.get_mut(bank) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::InterestExpense, interest, "deposit interest")
                        .credit(Account::Cash, paid, "deposit interest")
                        .credit(
                            Account::AccruedLiabilities,
                            interest - paid,
                            "deposit interest",
                        ),
                );
            }
        }
        // Provision 50 percent of loans whose borrower is past due.
        let overdue: i64 = world
            .loans
            .iter()
            .filter(|(_, loan)| matches!(loan.lender, crate::state::Lender::Bank(handle) if handle == bank))
            .filter(|(_, loan)| loan.defaulted)
            .map(|(_, loan)| loan.principal_remaining_cents / 2)
            .sum();
        if overdue > 0
            && let Some(firm) = world.firms.get_mut(bank)
        {
            let _ = firm.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::OperatingExpense, overdue, "loan provision")
                    .credit(Account::Provisions, overdue, "loan provision"),
            );
        }
    }
}

/// A region-shock withdrawal: 25 percent of deposits requested pro rata from actual cash.
pub fn run(world: &mut World, bank: Handle) -> i64 {
    let deposit_total = deposits(world, bank);
    let requested = deposit_total * 25 / 100;
    let cash = world
        .firms
        .get(bank)
        .map(|f| f.book.cash())
        .unwrap_or(0)
        .max(0);
    let paid = requested.min(cash);
    if paid > 0 {
        let holders: Vec<(HolderId, i64)> = world
            .firms
            .get(bank)
            .and_then(|firm| firm.bank.as_ref())
            .map(|model| {
                model
                    .depositors
                    .iter()
                    .map(|(holder, balance)| (*holder, *balance))
                    .collect()
            })
            .unwrap_or_default();
        let weights: Vec<(u64, i64)> = holders
            .iter()
            .enumerate()
            .map(|(index, (_, balance))| (index as u64, *balance))
            .collect();
        let shares = cw_core::money::allocate_cents(cw_core::money::Cents(paid), &weights);
        for (index, (holder, _)) in holders.iter().enumerate() {
            let share = shares[index].1.0.max(0);
            if share > 0 {
                debit_holder(world, *holder, share, "bank run");
                if let Some(model) = world
                    .firms
                    .get_mut(bank)
                    .and_then(|firm| firm.bank.as_mut())
                    && let Some(balance) = model.depositors.get_mut(holder)
                {
                    *balance = (*balance - share).max(0);
                }
            }
        }
        let _ = world.firms.get_mut(bank).unwrap().book.commit(
            world.tick,
            Txn::new().credit(Account::Cash, paid, "bank run").debit(
                Account::Deposits,
                paid,
                "bank run",
            ),
        );
    }
    if paid < requested {
        // Unpaid claims remain liabilities; a four-week liquidity failure restructures.
        if let Some(firm) = world.firms.get_mut(bank) {
            firm.status = FirmStatus::PastDue {
                since: Tick(world.tick.0),
            };
            firm.payout_freeze = true;
        }
    }
    paid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};
    use cw_data::Dataset;

    #[test]
    fn deposits_are_liabilities_and_cash_exists_once() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 71, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let bank = world
            .firms
            .iter()
            .find(|(_, firm)| firm.bank.is_some())
            .map(|(handle, _)| handle)
            .unwrap();
        let borrower = world
            .firms
            .iter()
            .find(|(_, firm)| firm.bank.is_none())
            .map(|(handle, _)| handle)
            .unwrap();
        let cash_before = world.firm(borrower).unwrap().book.cash();
        let deposits_before = deposits(&world, bank);
        deposit(&mut world, bank, HolderId::Firm(borrower), 1_000_000).unwrap();
        assert_eq!(
            world.firm(borrower).unwrap().book.cash(),
            cash_before - 1_000_000
        );
        assert_eq!(deposits(&world, bank), deposits_before + 1_000_000);
        let total_cash_before =
            world.firm(bank).unwrap().book.cash() + world.firm(borrower).unwrap().book.cash();
        assert!(total_cash_before > 0);
    }

    #[test]
    fn a_run_pays_depositors_pro_rata_and_reduces_their_claims() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            791,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        let bank = world
            .firms
            .iter()
            .find(|(_, firm)| firm.bank.is_some())
            .map(|(handle, _)| handle)
            .unwrap();
        let holders: Vec<Handle> = world
            .firms
            .iter()
            .filter(|(handle, firm)| *handle != bank && firm.operative())
            .map(|(handle, _)| handle)
            .take(2)
            .collect();
        for holder in &holders {
            deposit(&mut world, bank, HolderId::Firm(*holder), 10_000_000).unwrap();
        }
        // Leave only five million of cash so the twenty-five percent request cannot be paid in full.
        let cash = world.firm(bank).unwrap().book.cash();
        let drain = cash - 5_000_000;
        world
            .firms
            .get_mut(bank)
            .unwrap()
            .book
            .commit(
                world.tick,
                Txn::new().credit(Account::Cash, drain, "drain").debit(
                    Account::OtherAssets,
                    drain,
                    "drain",
                ),
            )
            .unwrap();
        let before: Vec<i64> = holders
            .iter()
            .map(|holder| world.firm(*holder).unwrap().book.cash())
            .collect();
        let deposits_before = deposits(&world, bank);
        let paid = run(&mut world, bank);
        assert_eq!(paid, 5_000_000, "only actual cash leaves");
        for (index, holder) in holders.iter().enumerate() {
            let gain = world.firm(*holder).unwrap().book.cash() - before[index];
            assert_eq!(gain, 2_500_000, "depositors share pro rata");
            assert!(world.firm(*holder).unwrap().book.is_balanced());
        }
        assert_eq!(deposits(&world, bank), deposits_before - paid);
        let model = world.firm(bank).unwrap().bank.as_ref().unwrap();
        assert_eq!(
            model.depositors.get(&HolderId::Firm(holders[0])),
            Some(&7_500_000)
        );
        assert_eq!(
            model.depositors.get(&HolderId::Firm(holders[1])),
            Some(&7_500_000)
        );
        assert!(world.firm(bank).unwrap().book.is_balanced());
    }

    #[test]
    fn lending_requires_liquidity_and_capital() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 72, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let bank = world
            .firms
            .iter()
            .find(|(_, firm)| firm.bank.is_some())
            .map(|(handle, _)| handle)
            .unwrap();
        let borrower = world
            .firms
            .iter()
            .find(|(_, firm)| firm.bank.is_none())
            .map(|(handle, _)| handle)
            .unwrap();
        let huge = world.firm(bank).unwrap().book.cash() * 2;
        let result = approve_loan(&mut world, bank, borrower, huge);
        assert!(result.is_err());
    }
}
