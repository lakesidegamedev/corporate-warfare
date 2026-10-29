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

/// Put a firm into the payment-grace state for an unpaid amount already on its books.
pub fn mark_firm_past_due(world: &mut World, firm: Handle) {
    let now = world.tick;
    if let Some(entity) = world.firms.get_mut(firm) {
        mark_past_due(entity, now);
    }
}

/// Loan principal that fell due and is still unpaid.
fn loan_arrears(world: &World, firm: Handle) -> i64 {
    world
        .firm(firm)
        .map(|entity| {
            entity
                .loans
                .iter()
                .filter_map(|handle| world.loans.get(*handle))
                .map(|loan| loan.arrears_cents)
                .sum()
        })
        .unwrap_or(0)
}

fn mark_past_due(firm: &mut crate::state::Firm, now: Tick) {
    // A dissolved, liquidating or merged firm cannot re-enter the payment-grace state. Late
    // interest or contract postings used to resurrect it, which hid a liquidation from reports.
    // A firm in restructuring stays in it for its full year: a bill it defers meanwhile joins its
    // overdue payables, and the year's end decides, rather than the next late posting turning
    // the plan back into a four-week grace and a second haircut a month after the first.
    if matches!(
        firm.status,
        FirmStatus::Dissolved
            | FirmStatus::Liquidating
            | FirmStatus::Merged { .. }
            | FirmStatus::Restructuring { .. }
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
                if overdue <= 0 && loan_arrears(world, handle) == 0 {
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
                    // A failing strategic firm may be rescued by the state before the court.
                    if crate::economy::rescue::try_rescue(world, dataset, handle) {
                        events.push((handle, "rescued"));
                        continue;
                    }
                    let liquid = crate::economy::valuation::liquidation_value(
                        &world.firms.get(handle).unwrap().book,
                    );
                    let debt = world.firms.get(handle).unwrap().book.debt();
                    if liquid * 100 >= debt * 50
                        && restructure(world, dataset, handle, debt) != Plan::NoneViable
                    {
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

/// Public entry for an approved restructuring command. A firm with no plant worth keeping has no
/// plan to put to its creditors, so the request is refused; the same test sends an automatic
/// restructuring to liquidation.
pub fn restructure_for_owner(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    debt: i64,
) -> bool {
    restructure(world, dataset, firm, debt) != Plan::NoneViable
}

/// What a restructuring plan found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Plan {
    /// Every plant that earns its keep stays; the ones that do not were mothballed.
    Done,
    /// Nothing the firm runs covers its own running costs: no plan, nothing changed.
    NoneViable,
}

/// What a plant adds each week at reference prices and the firm's own wages: output sold, less
/// inputs, payroll, upkeep and land. A carrier earns from the freight it gets, so it is judged on how much of
/// its capacity it fills. None for a terminal, which serves the carriers and is not judged alone.
fn plant_pays(world: &World, dataset: &Dataset, facility: Handle) -> Option<bool> {
    let plant = world.facilities.get(facility)?;
    let firm = world.firms.get(plant.firm)?;
    let recipe = &dataset.recipes[plant.recipe as usize];
    if recipe.transport_mode.is_some() {
        if recipe.terminal {
            return None;
        }
        return Some(
            crate::economy::logistics::carrier_fill_bps(world, facility)
                .is_some_and(|fill| fill >= crate::economy::logistics::CARRIER_MIN_FILL_BPS),
        );
    }
    let blocks = plant.blocks as i64;
    let revenue: i64 = recipe
        .outputs()
        .filter_map(|(id, lots)| {
            let product = dataset.product_index.get(id).copied()?;
            Some(lots * blocks * dataset.reference_price(product))
        })
        .sum();
    let inputs: i64 = recipe
        .weekly_inputs_lots
        .iter()
        .filter_map(|(id, qty)| {
            let product = dataset.product_index.get(id).copied()?;
            Some(qty * blocks * dataset.reference_price(product))
        })
        .sum();
    let wages: i64 = (0..3)
        .map(|tier| recipe.workers_low_mid_high[tier] * blocks * firm.policies.wage_offer[tier])
        .sum();
    let upkeep = recipe.weekly_maintenance(dataset.maintenance_annual_bps) * blocks;
    let land = dataset.fixture_land_rent_cents_weekly
        * world.regions[plant.region as usize].land_index_bps as i64
        / 10_000;
    Some(revenue - inputs - wages - upkeep - land >= 0)
}

/// Restructure a firm's debt and its plant list together. Creditors take the haircut only
/// against a plan: every plant that does not pay its way is mothballed, so the firm comes out
/// with a smaller cost base, not the same one. A firm whose plants all lose money has no plan.
fn restructure(world: &mut World, dataset: &Dataset, firm: Handle, debt: i64) -> Plan {
    let plants: Vec<(Handle, Option<bool>)> = world
        .firm(firm)
        .map(|entity| {
            entity
                .facilities
                .iter()
                .copied()
                .filter(|h| {
                    world
                        .facilities
                        .get(*h)
                        .is_some_and(|p| p.status == crate::state::FacilityStatus::Operating)
                })
                .map(|h| (h, plant_pays(world, dataset, h)))
                .collect()
        })
        .unwrap_or_default();
    let judged = plants.iter().filter(|(_, pays)| pays.is_some()).count();
    let keepers = plants
        .iter()
        .filter(|(_, pays)| *pays == Some(true))
        .count();
    if judged > 0 && keepers == 0 {
        return Plan::NoneViable;
    }
    let now = world.tick;
    for (handle, pays) in &plants {
        if *pays == Some(false)
            && let Some(plant) = world.facilities.get_mut(*handle)
        {
            crate::economy::operations::mothball(plant, now);
        }
    }
    cut_debt(world, firm, debt);
    Plan::Done
}

fn cut_debt(world: &mut World, firm: Handle, debt: i64) {
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

/// Liquidate: realise non-cash assets at 40 percent of carrying value through finite outside
/// buyers, pool them with the firm's own cash, and pay claims strictly by seniority: secured
/// loans, then administrative and payroll arrears, then unsecured loans, bonds and deposits pro
/// rata, then equity. Every payment reaches a real claimant; nothing is minted or left behind
/// in a dissolved shell. Open contracts and deals end with the firm.
pub fn liquidate(world: &mut World, firm: Handle, dataset: &Dataset) {
    let book = match world.firms.get(firm) {
        Some(entity) => entity.book.clone(),
        None => return,
    };
    // Open bids return their escrow first; it is the firm's own money.
    let open_deals: Vec<Handle> = world
        .deals
        .iter()
        .filter(|(_, deal)| deal.buyer == firm && !deal.state.terminal())
        .map(|(handle, _)| handle)
        .collect();
    for deal in open_deals {
        let _ = crate::economy::deals::withdraw(world, deal);
    }
    let book_after_refunds = world.firm(firm).map(|f| f.book.clone()).unwrap_or(book);
    let book = book_after_refunds;

    // 1. Asset sale to finite outside buyers. A sale without enough buyer funds sells what
    //    it can; it never mints cash.
    let asset_accounts = [
        Account::FixedAssets,
        Account::AccumulatedDepreciation,
        Account::Inventory,
        Account::Receivables,
        Account::OtherAssets,
        Account::WorkInProgress,
    ];
    let carrying: i64 = asset_accounts
        .iter()
        .map(|account| book.balance(*account))
        .sum();
    let wanted = carrying.max(0) * 40 / 100;
    let mut proceeds = 0i64;
    for buyer in world.outside.investors.iter_mut() {
        if proceeds >= wanted {
            break;
        }
        let take = (wanted - proceeds).min(buyer.capital_cents.max(0));
        buyer.capital_cents -= take;
        proceeds += take;
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        let mut sale = Txn::new().debit(Account::Cash, proceeds, "liquidation proceeds");
        for account in asset_accounts {
            let balance = book.balance(account);
            if balance != 0 {
                sale = sale.signed(account, -balance, "liquidation sale");
            }
        }
        sale = sale.signed(
            Account::OtherIncome,
            carrying - proceeds,
            "liquidation loss",
        );
        let _ = entity.book.commit(world.tick, sale);
        // The sale wrote off the whole inventory account, so the physical stock leaves with it.
        entity.inventory.clear();
        let restricted = entity.book.balance(Account::RestrictedCash);
        if restricted > 0 {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, restricted, "escrow released to estate")
                    .credit(
                        Account::RestrictedCash,
                        restricted,
                        "escrow released to estate",
                    ),
            );
        }
        entity.status = FirmStatus::Liquidating;
    }
    pay_creditors(world, firm);

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
                            .debit(Account::Cash, paid, "guarantee received")
                            .credit(Account::OtherIncome, paid, "guarantee received"),
                    );
                }
                remaining -= paid;
            }
        }
    }

    // Guarantee cash reaches the creditors through the same waterfall.
    pay_creditors(world, firm);

    // 5. Equity takes any residual, pro rata by eligible shares. The investor's lost holding
    //    is recorded as a career loss at its cost basis less any residual received.
    let residual = world.firm(firm).map(|f| f.book.cash().max(0)).unwrap_or(0);
    let holders: Vec<(HolderId, i64, i64)> = world
        .firm(firm)
        .map(|entity| {
            entity
                .shares
                .holders
                .iter()
                .filter(|(holder, lot)| !holder.is_treasury() && lot.qty > 0)
                .map(|(holder, lot)| (*holder, lot.qty, lot.cost_cents))
                .collect()
        })
        .unwrap_or_default();
    let eligible: i64 = holders.iter().map(|(_, qty, _)| *qty).sum();
    let mut paid_out = 0i64;
    for (holder, qty, basis) in &holders {
        let part = if eligible > 0 {
            (residual as i128 * *qty as i128 / eligible as i128) as i64
        } else {
            0
        };
        // The residual is in the failed firm's money; the investor, outside holders and the
        // state are paid in the numeraire.
        let part_paid = crate::economy::fx::firm_to_numeraire_world(world, firm, part);
        pay_equity(world, *holder, part_paid, *basis);
        paid_out += part;
        if *holder == HolderId::Investor {
            world.investor.career_losses_cents += (*basis - part).max(0);
        }
    }
    if paid_out > 0
        && let Some(entity) = world.firms.get_mut(firm)
    {
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, paid_out, "liquidation distribution")
                .debit(
                    Account::RetainedEarnings,
                    paid_out,
                    "liquidation distribution",
                ),
        );
    }
    if world.investor.primary_group == Some(firm) {
        world.investor.primary_group = None;
    }
    // Contracts end with the firm: no dead company keeps buying or selling. A surviving
    // buyer's escrow for the dead seller comes back; the dead buyer's escrow was pooled above.
    let ended: Vec<Handle> = world
        .contracts
        .iter()
        .filter(|(_, contract)| {
            (contract.buyer == firm || contract.seller == firm)
                && contract.status == crate::state::ContractStatus::Active
        })
        .map(|(handle, _)| handle)
        .collect();
    for handle in ended {
        let Some(contract) = world.contracts.get_mut(handle) else {
            continue;
        };
        contract.status = crate::state::ContractStatus::Cancelled;
        let escrow = std::mem::take(&mut contract.escrow_cents);
        let buyer = contract.buyer;
        if buyer != firm
            && escrow > 0
            && let Some(entity) = world.firms.get_mut(buyer)
        {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, escrow, "contract escrow refund")
                    .credit(Account::RestrictedCash, escrow, "contract escrow refund"),
            );
        }
    }
    if let Some(entity) = world.firms.get_mut(firm) {
        entity.status = FirmStatus::Dissolved;
        entity.facilities.clear();
        entity
            .shares
            .holders
            .retain(|holder, _| holder.is_treasury());
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

/// Why a company may not wind itself down, or `None` when it may. A voluntary wind-down is for a
/// solvent company with nothing left to run: no plant operating, building or retooling, no
/// stake in another company, no bills it cannot hand to a named creditor (trade payables,
/// accruals, prepayments, insurance claims, provisions) and an estate, its cash plus the forced
/// sale value of everything else (40 percent of carrying value, as in liquidation), that pays
/// every loan, bond, deposit, arrear and tax in full. The career's primary company cannot wind
/// itself down. The player and rival companies are held to the same test.
pub fn wind_down_blocker(world: &World, firm: Handle) -> Option<&'static str> {
    let entity = world.firm(firm)?;
    if !matches!(entity.status, FirmStatus::Operating) {
        return Some("not trading normally");
    }
    if world.investor.primary_group == Some(firm) {
        return Some("primary company");
    }
    let running = entity.facilities.iter().any(|handle| {
        world
            .facilities
            .get(*handle)
            .is_some_and(|plant| plant.status != crate::state::FacilityStatus::Mothballed)
    });
    if running {
        return Some("has a working plant");
    }
    let holds_stake = world.firms.iter().any(|(other, company)| {
        other != firm
            && company.operative()
            && company
                .shares
                .holders
                .get(&HolderId::Firm(firm))
                .is_some_and(|lot| lot.qty > 0)
    });
    if holds_stake || entity.book.balance(Account::Investments) != 0 {
        return Some("holds stakes in other companies");
    }
    let book = &entity.book;
    let unnamed = [
        Account::Payables,
        Account::AccruedLiabilities,
        Account::UnearnedRevenue,
        Account::ClaimLiabilities,
        Account::Provisions,
    ];
    if unnamed.iter().any(|account| book.balance(*account) != 0) {
        return Some("has unsettled bills");
    }
    if wind_down_estate(book) < wind_down_claims(world, firm) {
        return Some("cannot pay its creditors in full");
    }
    None
}

/// Cash plus the forced-sale value of every other asset liquidation sells.
fn wind_down_estate(book: &Ledger) -> i64 {
    let carrying: i64 = [
        Account::FixedAssets,
        Account::AccumulatedDepreciation,
        Account::Inventory,
        Account::Receivables,
        Account::OtherAssets,
        Account::WorkInProgress,
    ]
    .iter()
    .map(|account| book.balance(*account))
    .sum();
    book.cash()
        .saturating_add(book.balance(Account::RestrictedCash).max(0))
        .saturating_add(carrying.max(0) * 40 / 100)
}

/// Everything the wind-down must pay before shareholders: loans outstanding, the firm's bonds,
/// deposits, past-due bills and tax accrued.
fn wind_down_claims(world: &World, firm: Handle) -> i64 {
    let Some(entity) = world.firm(firm) else {
        return 0;
    };
    let loans: i64 = entity
        .loans
        .iter()
        .filter_map(|handle| world.loans.get(*handle))
        .map(|loan| loan.principal_remaining_cents.max(0))
        .sum();
    let bonds: i64 = world
        .bonds
        .iter()
        .filter(|(_, bond)| bond.issuer == firm)
        .map(|(_, bond)| bond.principal_cents.max(0))
        .sum();
    let book = &entity.book;
    loans
        .saturating_add(bonds)
        .saturating_add((-book.balance(Account::Deposits)).max(0))
        .saturating_add((-book.balance(Account::OverduePayables)).max(0))
        .saturating_add((-book.balance(Account::TaxesPayable)).max(0))
}

/// Voluntary wind-down of a company `wind_down_blocker` clears: the tax it has accrued is paid
/// to the state now, then the ordinary liquidation sells the remaining assets to outside buyers
/// at 40 percent of carrying value, pays creditors by seniority and returns the rest to the
/// shareholders pro rata, and the company is dissolved. Nothing is minted: every cent leaves
/// through a named counterparty.
pub fn wind_down(world: &mut World, firm: Handle, dataset: &Dataset) {
    let due = world
        .firm(firm)
        .map(|entity| (-entity.book.balance(Account::TaxesPayable)).max(0))
        .unwrap_or(0);
    let cash = world.firm(firm).map(|f| f.book.cash().max(0)).unwrap_or(0);
    let paid = due.min(cash);
    if paid > 0 {
        if let Some(entity) = world.firms.get_mut(firm) {
            let _ = entity.book.commit(
                world.tick,
                Txn::new().credit(Account::Cash, paid, "tax payment").debit(
                    Account::TaxesPayable,
                    paid,
                    "tax payment",
                ),
            );
        }
        let receipt = crate::economy::fx::firm_to_numeraire(world, dataset, firm, paid);
        let _ = world.government.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, receipt, "tax receipt")
                .credit(Account::Revenue, receipt, "tax receipt"),
        );
        crate::economy::fiscal::record(world, receipt);
    }
    if paid < due {
        // Unreachable when the blocker cleared, but an unpaid remainder must never vanish.
        record_missed_tax(world, firm, due - paid);
    }
    liquidate(world, firm, dataset);
}

/// Pay the estate's cash to its creditors by seniority: secured loans, administrative arrears,
/// then unsecured loans, bonds and deposits pro rata. Safe to run again after guarantee cash.
fn pay_creditors(world: &mut World, firm: Handle) {
    let mut pool = world.firm(firm).map(|f| f.book.cash().max(0)).unwrap_or(0);
    let loans: Vec<(Handle, bool, i64)> = world
        .firm(firm)
        .map(|entity| {
            entity
                .loans
                .iter()
                .filter_map(|handle| {
                    world
                        .loans
                        .get(*handle)
                        .map(|loan| (*handle, loan.secured, loan.principal_remaining_cents))
                })
                .filter(|(_, _, principal)| *principal > 0)
                .collect()
        })
        .unwrap_or_default();

    // 2. Secured loans, each to its own lender.
    for (loan, secured, principal) in &loans {
        if *secured && pool > 0 {
            let paid = (*principal).min(pool);
            crate::economy::credit::repay(world, *loan, paid);
            pool -= paid;
        }
    }

    // 3. Administrative and payroll arrears, paid to the outside parties that are owed.
    let arrears = (-world
        .firm(firm)
        .map(|f| f.book.balance(Account::OverduePayables))
        .unwrap_or(0))
    .max(0);
    let admin = arrears.min(pool);
    if admin > 0 {
        if let Some(entity) = world.firms.get_mut(firm) {
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, admin, "administrative claims")
                    .debit(Account::OverduePayables, admin, "administrative claims"),
            );
        }
        let _ = world.outside.gateway_book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, admin, "estate arrears received")
                .credit(Account::Revenue, admin, "estate arrears received"),
        );
        pool -= admin;
    }

    // 4. Unsecured loans, bonds and deposits share what is left pro rata.
    let unsecured_loans: Vec<(Handle, i64)> = loans
        .iter()
        .filter(|(loan, secured, _)| {
            !secured
                && world
                    .loans
                    .get(*loan)
                    .map(|l| l.principal_remaining_cents > 0)
                    .unwrap_or(false)
        })
        .map(|(loan, _, _)| {
            (
                *loan,
                world.loans.get(*loan).unwrap().principal_remaining_cents,
            )
        })
        .collect();
    let bonds: Vec<(Handle, i64)> = world
        .bonds
        .iter()
        .filter(|(_, bond)| bond.issuer == firm && bond.principal_cents > 0)
        .map(|(handle, bond)| (handle, bond.principal_cents))
        .collect();
    let depositors: Vec<(HolderId, i64)> = world
        .firm(firm)
        .and_then(|entity| entity.bank.as_ref())
        .map(|model| {
            model
                .depositors
                .iter()
                .filter(|(_, balance)| **balance > 0)
                .map(|(holder, balance)| (*holder, *balance))
                .collect()
        })
        .unwrap_or_default();
    let unsecured: i64 = unsecured_loans.iter().map(|(_, a)| *a).sum::<i64>()
        + bonds.iter().map(|(_, a)| *a).sum::<i64>()
        + depositors.iter().map(|(_, a)| *a).sum::<i64>();
    if unsecured > 0 && pool > 0 {
        let available = pool.min(unsecured);
        let share = |claim: i64| (claim as i128 * available as i128 / unsecured as i128) as i64;
        for (loan, claim) in &unsecured_loans {
            let paid = share(*claim);
            crate::economy::credit::repay(world, *loan, paid);
            pool -= paid;
        }
        for (bond, claim) in &bonds {
            let paid = share(*claim);
            let holders: Vec<(HolderId, i64)> = world
                .bonds
                .get(*bond)
                .map(|b| b.holders.iter().map(|(h, q)| (*h, *q)).collect())
                .unwrap_or_default();
            let total: i64 = holders.iter().map(|(_, q)| *q).sum();
            let mut distributed = 0i64;
            for (holder, qty) in &holders {
                let part = if total > 0 {
                    (paid as i128 * *qty as i128 / total as i128) as i64
                } else {
                    0
                };
                let part_paid = crate::economy::fx::firm_to_numeraire_world(world, firm, part);
                pay_claimant(
                    world,
                    *holder,
                    part_paid,
                    Account::Investments,
                    "bond recovery",
                );
                distributed += part;
            }
            if let Some(entity) = world.firms.get_mut(firm) {
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, distributed, "bond recovery")
                        .debit(Account::Bonds, distributed, "bond recovery"),
                );
            }
            if let Some(stored) = world.bonds.get_mut(*bond) {
                stored.principal_cents -= distributed;
            }
            pool -= distributed;
        }
        for (holder, claim) in &depositors {
            let paid = share(*claim);
            let paid_numeraire = crate::economy::fx::firm_to_numeraire_world(world, firm, paid);
            pay_claimant(
                world,
                *holder,
                paid_numeraire,
                Account::Receivables,
                "deposit recovery",
            );
            if let Some(entity) = world.firms.get_mut(firm) {
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, paid, "deposit recovery")
                        .debit(Account::Deposits, paid, "deposit recovery"),
                );
                if let Some(model) = entity.bank.as_mut()
                    && let Some(balance) = model.depositors.get_mut(holder)
                {
                    *balance -= paid;
                }
            }
            pool -= paid;
        }
    }
    let _ = pool;
}

/// Pay a creditor of a failed firm: the holder's claim asset is realised for cash.
fn pay_claimant(world: &mut World, holder: HolderId, amount: i64, claim: Account, memo: &str) {
    if amount <= 0 {
        return;
    }
    match holder {
        HolderId::Investor => {
            world.investor.cash_cents += amount;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, amount, memo)
                    .credit(claim, amount, memo),
            );
        }
        HolderId::Firm(handle) => {
            if let Some(entity) = world.firms.get_mut(handle) {
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, amount, memo)
                        .credit(claim, amount, memo),
                );
            }
        }
        HolderId::Outside(index) => {
            if let Some(entity) = world.outside.investors.get_mut(index as usize) {
                entity.capital_cents += amount;
            }
        }
        HolderId::State => {
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, amount, memo)
                    .credit(claim, amount, memo),
            );
        }
        HolderId::Treasury => {}
    }
}

/// Pay a shareholder its liquidation residual; the holding is written off at cost basis.
pub(crate) fn pay_equity(world: &mut World, holder: HolderId, amount: i64, basis: i64) {
    match holder {
        HolderId::Investor => {
            world.investor.cash_cents += amount;
            world.investor.realised_gains_cents += amount - basis;
            let _ = world.investor.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, amount, "liquidation distribution")
                    .credit(Account::Investments, basis, "holding written off")
                    .credit(
                        Account::OtherIncome,
                        amount - basis,
                        "liquidation gain or loss",
                    ),
            );
        }
        HolderId::Firm(handle) => {
            if let Some(entity) = world.firms.get_mut(handle) {
                let _ = entity.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, amount, "liquidation distribution")
                        .credit(Account::Investments, basis, "holding written off")
                        .credit(
                            Account::OtherIncome,
                            amount - basis,
                            "liquidation gain or loss",
                        ),
                );
            }
        }
        HolderId::Outside(index) => {
            if let Some(entity) = world.outside.investors.get_mut(index as usize) {
                entity.capital_cents += amount;
            }
        }
        HolderId::State => {
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, amount, "liquidation distribution")
                    .credit(Account::Investments, basis, "holding written off")
                    .credit(
                        Account::OtherIncome,
                        amount - basis,
                        "liquidation gain or loss",
                    ),
            );
        }
        HolderId::Treasury => {}
    }
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

    /// A Ruhr firm with two pipelines, and the fake week of freight each moved. Fill is lot-km
    /// moved over lot-km of capacity, so 100 lots of capacity over the 400 km reference haul is
    /// 40,000 lot-km, and 20,000 moved is exactly half.
    fn pipeline_firm() -> (World, Dataset, Handle, [Handle; 2]) {
        let dataset = Dataset::load_scenario("ruhr-rhine-1991").unwrap();
        let mut world = seed::build(&dataset, 5, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let (firm, plants) = world
            .firms
            .iter()
            .find_map(|(h, f)| {
                let pipes: Vec<Handle> = f
                    .facilities
                    .iter()
                    .copied()
                    .filter(|p| {
                        let r = &dataset.recipes[world.facilities.get(*p).unwrap().recipe as usize];
                        r.transport_mode.as_deref() == Some("pipeline") && !r.terminal
                    })
                    .collect();
                (pipes.len() == 2).then(|| (h, [pipes[0], pipes[1]]))
            })
            .expect("a Ruhr firm with two pipelines");
        // Only the two pipelines are judged: whatever else the firm runs is put away.
        let others: Vec<Handle> = world
            .firm(firm)
            .unwrap()
            .facilities
            .iter()
            .copied()
            .filter(|p| !plants.contains(p))
            .collect();
        for other in others {
            world.facilities.get_mut(other).unwrap().status =
                crate::state::FacilityStatus::Mothballed;
        }
        (world, dataset, firm, plants)
    }

    fn move_freight(world: &mut World, plants: [Handle; 2], used: [i64; 2]) {
        world.logistics.carriers.clear();
        for (plant, used_lot_km) in plants.iter().zip(used) {
            let facility = world.facilities.get(*plant).unwrap();
            world
                .logistics
                .carriers
                .push(crate::economy::logistics::Carrier {
                    firm: facility.firm,
                    facility: *plant,
                    region: facility.region,
                    mode: 0,
                    cap: 100,
                    used_lot_km,
                });
        }
    }

    #[test]
    fn a_restructuring_mothballs_the_carrier_that_sits_idle_and_keeps_the_one_that_fills() {
        let (mut world, dataset, firm, plants) = pipeline_firm();
        // Half full keeps its place; one lot in ten thousand does not (the line is 25 percent).
        move_freight(&mut world, plants, [20_000, 4]);
        assert_eq!(
            restructure(&mut world, &dataset, firm, 1_000_000),
            Plan::Done
        );
        let status = |w: &World, i: usize| w.facilities.get(plants[i]).unwrap().status;
        assert_eq!(status(&world, 0), crate::state::FacilityStatus::Operating);
        assert_eq!(status(&world, 1), crate::state::FacilityStatus::Mothballed);
        assert_eq!(world.facilities.get(plants[1]).unwrap().workers, [0; 3]);
        // The creditors' half of the plan still ran: a 50 percent haircut on the debt.
        assert!(matches!(
            world.firm(firm).unwrap().status,
            FirmStatus::Restructuring { .. }
        ));
        assert!(world.firm(firm).unwrap().book.is_balanced());
    }

    #[test]
    fn a_firm_whose_every_carrier_sits_idle_has_no_plan_and_nothing_changes() {
        let (mut world, dataset, firm, plants) = pipeline_firm();
        move_freight(&mut world, plants, [4, 4]);
        let before = world.firm(firm).unwrap().status;
        assert_eq!(
            restructure(&mut world, &dataset, firm, 1_000_000),
            Plan::NoneViable
        );
        assert!(!restructure_for_owner(
            &mut world, &dataset, firm, 1_000_000
        ));
        assert_eq!(world.firm(firm).unwrap().status, before);
        for plant in plants {
            assert_eq!(
                world.facilities.get(plant).unwrap().status,
                crate::state::FacilityStatus::Operating
            );
        }
    }

    #[test]
    fn a_late_bill_does_not_end_a_restructuring_early() {
        let (mut world, _dataset, firm, _) = pipeline_firm();
        world.firms.get_mut(firm).unwrap().status = FirmStatus::Restructuring { until: Tick(60) };
        defer_expense(&mut world, firm, 5_000, Account::Wages, "late");
        assert_eq!(overdue(&world.firm(firm).unwrap().book), 5_000);
        assert!(matches!(
            world.firm(firm).unwrap().status,
            FirmStatus::Restructuring { .. }
        ));
    }
}
