//! Credit evidence (R18): the final principal cent, payments to the named lender, and missed
//! principal carried as arrears without being counted twice. docs/rules/FINANCE-LABOUR-AI.md.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::economy::{credit, insolvency, seed};
use cw_sim::state::{FirmStatus, Lender, Loan, ScenarioKind, ScenarioMode, World};

fn setup() -> (World, Dataset, Handle, Handle) {
    let dataset = Dataset::load().unwrap();
    let world = seed::build(&dataset, 701, ScenarioKind::Founder, ScenarioMode::Sandbox);
    let operative: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.operative() && firm.loans.is_empty())
        .map(|(handle, _)| handle)
        .collect();
    let (borrower, lender) = (operative[0], operative[1]);
    (world, dataset, borrower, lender)
}

fn lend(world: &mut World, borrower: Handle, lender: Handle, principal: i64, weeks: u32) -> Handle {
    let tick = world.tick;
    world
        .firms
        .get_mut(lender)
        .unwrap()
        .book
        .commit(
            tick,
            Txn::new()
                .debit(Account::Receivables, principal, "fixture loan")
                .credit(Account::Cash, principal, "fixture loan"),
        )
        .unwrap();
    world
        .firms
        .get_mut(borrower)
        .unwrap()
        .book
        .commit(
            tick,
            Txn::new()
                .debit(Account::Cash, principal, "fixture loan")
                .credit(Account::Loans, principal, "fixture loan"),
        )
        .unwrap();
    let handle = world.loans.insert(Loan {
        borrower,
        lender: Lender::Firm(lender),
        principal_remaining_cents: principal,
        rate_bps: 0,
        weekly_principal_cents: principal / weeks as i64,
        remaining_weeks: weeks,
        collateral: None,
        secured: false,
        created: tick,
        carry: 0,
        defaulted: false,
        arrears_cents: 0,
    });
    world.firms.get_mut(borrower).unwrap().loans.push(handle);
    handle
}

fn cash(world: &World, firm: Handle) -> i64 {
    world.firm(firm).unwrap().book.cash()
}

#[test]
fn the_final_instalment_clears_the_last_cent_to_the_named_lender() {
    let (mut world, dataset, borrower, lender) = setup();
    // 1,000,001 cents over three weeks: 333,333 + 333,333 + 333,335. Zero rate isolates principal.
    let loan = lend(&mut world, borrower, lender, 1_000_001, 3);
    let lender_cash = cash(&world, lender);
    for _ in 0..3 {
        world.tick = world.tick.next();
        credit::service(&mut world, &dataset);
    }
    assert_eq!(world.loans.get(loan).unwrap().principal_remaining_cents, 0);
    assert_eq!(
        cash(&world, lender),
        lender_cash + 1_000_001,
        "every cent reaches the lender"
    );
    assert!(world.firm(borrower).unwrap().book.is_balanced());
    assert!(world.firm(lender).unwrap().book.is_balanced());
}

#[test]
fn missed_principal_becomes_arrears_not_a_second_liability() {
    let (mut world, dataset, borrower, lender) = setup();
    let loan = lend(&mut world, borrower, lender, 520_000, 52);
    // Empty the borrower's cash so the first instalment is missed.
    let all = cash(&world, borrower);
    let tick = world.tick;
    world
        .firms
        .get_mut(borrower)
        .unwrap()
        .book
        .commit(
            tick,
            Txn::new()
                .credit(Account::Cash, all, "fixture spend")
                .debit(Account::OperatingExpense, all, "fixture spend"),
        )
        .unwrap();
    let liabilities = world.firm(borrower).unwrap().book.total_liabilities();
    world.tick = world.tick.next();
    credit::service(&mut world, &dataset);
    let stored = world.loans.get(loan).unwrap();
    assert_eq!(stored.arrears_cents, 10_000);
    assert_eq!(
        stored.principal_remaining_cents, 520_000,
        "still owed, once"
    );
    assert_eq!(
        world.firm(borrower).unwrap().book.total_liabilities(),
        liabilities,
        "no phantom expense or duplicate payable at zero interest"
    );
    assert!(matches!(
        world.firm(borrower).unwrap().status,
        FirmStatus::PastDue { .. }
    ));
    // Cash returns: next service collects the arrears and this week's instalment.
    let tick = world.tick;
    world
        .firms
        .get_mut(borrower)
        .unwrap()
        .book
        .commit(
            tick,
            Txn::new()
                .debit(Account::Cash, 100_000, "fixture income")
                .credit(Account::ShareCapital, 100_000, "fixture income"),
        )
        .unwrap();
    let lender_cash = cash(&world, lender);
    world.tick = world.tick.next();
    credit::service(&mut world, &dataset);
    assert_eq!(cash(&world, lender), lender_cash + 20_000);
    assert_eq!(world.loans.get(loan).unwrap().arrears_cents, 0);
    insolvency::settle(&mut world, &dataset);
    assert_eq!(
        world.firm(borrower).unwrap().status,
        FirmStatus::Operating,
        "cured"
    );
}
