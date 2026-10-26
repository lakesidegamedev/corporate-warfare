//! Liquidation evidence (R25).
//!
//! docs/rules/CORPORATE.md: liquidate assets at 40 percent of carrying value through finite
//! outside buyer funds; pay secured claims, then administrative/payroll, then unsecured
//! (deposits, bonds, loans) proportionately, then equity. Insufficient buyers delay sale, never
//! mint cash. The oracle here is conservation: the world's total money is the same before and
//! after, and each claimant class is paid in order.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::economy::{insolvency, seed};
use cw_sim::state::{
    ContractStatus, FirmStatus, HolderId, Lender, Loan, ScenarioKind, ScenarioMode, World,
};

/// Every cent in the world: firm cash and escrow, the investor, outside holders, lenders,
/// outside sector books and the government.
fn money(world: &World) -> i64 {
    let firms: i64 = world
        .firms
        .iter()
        .map(|(_, firm)| {
            firm.book.balance(Account::Cash) + firm.book.balance(Account::RestrictedCash)
        })
        .sum();
    firms
        + world.investor.cash_cents
        + world
            .outside
            .investors
            .iter()
            .map(|h| h.capital_cents)
            .sum::<i64>()
        + world
            .outside
            .lenders
            .iter()
            .map(|h| h.capital_cents)
            .sum::<i64>()
        + world.outside.advocacy_book.balance(Account::Cash)
        + world.outside.legal_services_book.balance(Account::Cash)
        + world.outside.gateway_book.balance(Account::Cash)
        + world.government.book.balance(Account::Cash)
}

fn loan(world: &mut World, firm: Handle, lender: u32, principal: i64, secured: bool) -> Handle {
    world.outside.lenders[lender as usize].capital_cents -= principal;
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .book
        .commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, principal, "fixture loan")
                .credit(Account::Loans, principal, "fixture loan"),
        )
        .unwrap();
    let handle = world.loans.insert(Loan {
        borrower: firm,
        lender: Lender::Outside(lender),
        principal_remaining_cents: principal,
        rate_bps: 800,
        weekly_principal_cents: 0,
        remaining_weeks: 52,
        collateral: None,
        secured,
        created: world.tick,
        carry: 0,
        defaulted: false,
        arrears_cents: 0,
    });
    world.firms.get_mut(firm).unwrap().loans.push(handle);
    handle
}

fn setup() -> (World, Dataset, Handle) {
    let dataset = Dataset::load().unwrap();
    let world = seed::build(&dataset, 501, ScenarioKind::Founder, ScenarioMode::Sandbox);
    let firm = world
        .firms
        .iter()
        .find(|(_, firm)| firm.operative() && firm.bank.is_none() && firm.loans.is_empty())
        .map(|(handle, _)| handle)
        .unwrap();
    (world, dataset, firm)
}

#[test]
fn liquidation_conserves_money_and_pays_secured_before_unsecured() {
    let (mut world, dataset, firm) = setup();
    for lender in world.outside.lenders.iter_mut() {
        lender.capital_cents = lender.capital_cents.max(10_000_000_000);
    }
    let cash = world.firm(firm).unwrap().book.cash();
    // Claims far above anything the estate can realise.
    let secured = loan(&mut world, firm, 0, cash, true);
    let unsecured = loan(&mut world, firm, 1, cash * 10, false);
    // Spend the borrowed cash so only the original cash plus asset proceeds remain.
    let borrowed = cash * 11;
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .book
        .commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, borrowed, "fixture spend")
                .debit(Account::OperatingExpense, borrowed, "fixture spend"),
        )
        .unwrap();
    world
        .outside
        .gateway_book
        .commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, borrowed, "fixture spend")
                .credit(Account::Revenue, borrowed, "fixture spend"),
        )
        .unwrap();
    let carrying: i64 = [
        Account::FixedAssets,
        Account::AccumulatedDepreciation,
        Account::Inventory,
        Account::Receivables,
        Account::OtherAssets,
        Account::WorkInProgress,
    ]
    .iter()
    .map(|account| world.firm(firm).unwrap().book.balance(*account))
    .sum();
    let estate = world.firm(firm).unwrap().book.cash()
        + world
            .firm(firm)
            .unwrap()
            .book
            .balance(Account::RestrictedCash)
        + carrying * 40 / 100;
    let before = money(&world);
    let secured_lender = world.outside.lenders[0].capital_cents;
    let unsecured_lender = world.outside.lenders[1].capital_cents;

    insolvency::liquidate(&mut world, firm, &dataset);

    assert_eq!(money(&world), before, "no cent minted or destroyed");
    assert_eq!(world.firm(firm).unwrap().status, FirmStatus::Dissolved);
    assert_eq!(world.firm(firm).unwrap().book.cash(), 0, "no zombie cash");
    // The secured lender is paid in full first; the unsecured lender gets the rest.
    assert_eq!(
        world.loans.get(secured).unwrap().principal_remaining_cents,
        0
    );
    assert_eq!(
        world.outside.lenders[0].capital_cents,
        secured_lender + cash
    );
    let recovered = world.outside.lenders[1].capital_cents - unsecured_lender;
    assert_eq!(
        recovered,
        estate - cash,
        "the unsecured lender receives the remainder"
    );
    assert_eq!(
        world
            .loans
            .get(unsecured)
            .unwrap()
            .principal_remaining_cents,
        cash * 10 - recovered
    );
    assert!(world.firm(firm).unwrap().book.is_balanced());
}

#[test]
fn a_solvent_liquidation_pays_equity_the_residual_and_ends_contracts() {
    let (mut world, dataset, firm) = setup();
    {
        let lots = &mut world.firms.get_mut(firm).unwrap().shares;
        let issued = lots.issued;
        lots.holders.clear();
        lots.holders.insert(
            HolderId::Investor,
            cw_sim::state::ShareLot {
                qty: issued,
                cost_cents: 1_000,
            },
        );
    }
    let investor_before = world.investor.cash_cents;
    let before = money(&world);
    insolvency::liquidate(&mut world, firm, &dataset);
    assert_eq!(money(&world), before);
    let residual = world.investor.cash_cents - investor_before;
    assert!(
        residual > 0,
        "with no creditors the owner receives the estate"
    );
    assert!(
        world
            .contracts
            .iter()
            .filter(|(_, c)| c.buyer == firm || c.seller == firm)
            .all(|(_, c)| c.status != ContractStatus::Active),
        "a dissolved firm keeps no live contracts"
    );
    assert!(world.investor.book.is_balanced());
    // The estate sold the stock with the inventory account; no carrying cost lingers in lots
    // the ledger has already written off.
    let entity = world.firms.get(firm).unwrap();
    assert_eq!(entity.book.balance(Account::Inventory), 0);
    assert!(
        entity.inventory.values().all(|lot| lot.qty.0 == 0 && lot.cost_cents == 0),
        "liquidated stock leaves with the sale"
    );
}

#[test]
fn a_thin_buyer_market_sells_less_and_never_mints() {
    let (mut world, dataset, firm) = setup();
    for buyer in world.outside.investors.iter_mut() {
        buyer.capital_cents = 1_000;
    }
    let before = money(&world);
    let buyers: i64 = world
        .outside
        .investors
        .iter()
        .map(|h| h.capital_cents)
        .sum();
    let cash = world.firm(firm).unwrap().book.cash();
    let investor = world.investor.cash_cents;
    let owners_before: i64 = world
        .outside
        .investors
        .iter()
        .map(|h| h.capital_cents)
        .sum::<i64>()
        + investor;
    insolvency::liquidate(&mut world, firm, &dataset);
    assert_eq!(money(&world), before);
    // The buyers could pay at most what they had.
    let paid_by_buyers = buyers;
    let owners_after: i64 = world
        .outside
        .investors
        .iter()
        .map(|h| h.capital_cents)
        .sum::<i64>()
        + world.investor.cash_cents;
    assert!(owners_after - owners_before <= cash + paid_by_buyers);
}
