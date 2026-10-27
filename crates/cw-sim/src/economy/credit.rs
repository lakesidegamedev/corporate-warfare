//! Credit grades, loans, bonds and debt service.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::round_even_div;
use cw_data::Dataset;

use crate::state::{Bond, Grade, HolderId, Lender, Loan, World};

/// Assess the credit grade from trailing interest coverage and leverage.
pub fn assess_grade(world: &World, firm: Handle) -> Grade {
    let Some(entity) = world.firms.get(firm) else {
        return Grade::D;
    };
    let ledger = &entity.book;
    if ledger.total_equity() < 0 {
        return Grade::D;
    }
    let debt = ledger.debt();
    let tangible = ledger.net_tangible_assets();
    // Cross-multiplied integer comparisons avoid floating point thresholds.
    let leverage_bps = if debt == 0 {
        0
    } else if tangible <= 0 {
        i32::MAX
    } else {
        (debt as i128 * 10_000 / tangible as i128).min(i32::MAX as i128) as i32
    };
    let ebitda: i64 = ledger.recent_weeks().iter().map(|week| week.ebitda()).sum();
    let interest_due: i64 = entity
        .loans
        .iter()
        .filter_map(|handle| world.loans.get(*handle))
        .map(weekly_interest)
        .sum();
    let coverage_unlimited = interest_due == 0 && ebitda > 0;
    let coverage_zero = ebitda <= 0;
    let coverage_quarters = if interest_due == 0 {
        i64::MAX
    } else {
        ebitda * 10_000 / interest_due
    };
    if coverage_zero {
        if ledger.cash() >= 13 * crate::economy::insolvency::mandatory_weekly_outflow(world, firm) {
            return Grade::B;
        }
        return Grade::C;
    }
    if (coverage_unlimited || coverage_quarters >= 40_000) && leverage_bps <= 3_000 {
        Grade::A
    } else if coverage_quarters >= 20_000 && leverage_bps <= 5_000 {
        Grade::B
    } else if coverage_quarters >= 10_000 && leverage_bps <= 7_000 {
        Grade::C
    } else {
        Grade::D
    }
}

pub fn weekly_interest(loan: &Loan) -> i64 {
    round_even_div(
        loan.principal_remaining_cents as i128 * loan.rate_bps as i128,
        10_000 * 52,
    ) as i64
}

/// Interest for one week with a persisted sub-cent carry, so repeated rounding cannot create or
/// destroy value across the term.
pub fn accrued_interest(loan: &Loan) -> (i64, i64) {
    let mut carry = cw_core::money::SubCentCarry::new(10_000 * 52);
    carry.numerator = loan.carry;
    let interest = carry.accrue(
        loan.principal_remaining_cents as i128 * loan.rate_bps as i128,
        10_000 * 52,
    );
    (interest.0, carry.numerator)
}

/// Weekly grade refresh, then scheduled principal and interest on every loan.
pub fn service(world: &mut World, dataset: &Dataset) {
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let grade = assess_grade(world, handle);
        if let Some(firm) = world.firms.get_mut(handle) {
            firm.grade = grade;
        }
    }

    let loan_handles: Vec<Handle> = world.loans.iter().map(|(handle, _)| handle).collect();
    for handle in loan_handles {
        let Some(loan) = world.loans.get(handle).cloned() else {
            continue;
        };
        if loan.principal_remaining_cents <= 0 {
            continue;
        }
        let borrower = loan.borrower;
        let (interest, carry_next) = accrued_interest(&loan);
        // Arrears first, then this week's instalment; the final week clears everything.
        let principal = if loan.remaining_weeks <= 1 {
            loan.principal_remaining_cents
        } else {
            (loan.arrears_cents + loan.weekly_principal_cents).min(loan.principal_remaining_cents)
        };
        let due = interest + principal;
        let cash = world
            .firms
            .get(borrower)
            .map(|firm| firm.book.cash())
            .unwrap_or(0)
            .max(0);
        let paid = due.min(cash);
        let interest_paid = paid.min(interest);
        let principal_paid = paid - interest_paid;
        if paid > 0 {
            let _ = world.firms.get_mut(borrower).unwrap().book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, paid, "debt service")
                    .debit(Account::InterestExpense, interest_paid, "interest")
                    .debit(Account::Loans, principal_paid, "principal"),
            );
            credit_lender(world, &loan, paid, interest_paid);
        }
        if paid < due {
            let missed_interest = interest - interest_paid;
            let missed_principal = principal - principal_paid;
            crate::economy::insolvency::defer_expense(
                world,
                borrower,
                missed_interest,
                Account::InterestExpense,
                "unpaid interest",
            );
            // Unpaid principal is not an expense and is already in the loan balance: it
            // becomes arrears owed to the same lender, and the borrower is past due.
            if missed_principal > 0 {
                crate::economy::insolvency::mark_firm_past_due(world, borrower);
            }
            if let Some(stored) = world.loans.get_mut(handle) {
                stored.defaulted = true;
            }
        }
        if let Some(stored) = world.loans.get_mut(handle) {
            stored.arrears_cents = (principal - principal_paid).max(0);
            stored.carry = carry_next;
            stored.principal_remaining_cents =
                (stored.principal_remaining_cents - principal_paid).max(0);
            stored.remaining_weeks = stored.remaining_weeks.saturating_sub(1);
            if stored.remaining_weeks == 0 || stored.principal_remaining_cents == 0 {
                stored.remaining_weeks = 0;
            }
        }
    }
    service_bonds(world, dataset);
}

fn credit_lender(world: &mut World, loan: &Loan, paid: i64, interest_paid: i64) {
    match loan.lender {
        Lender::Outside(crate::state::STATE_LENDER) => {
            // The state's books are in the numeraire.
            let paid_n = crate::economy::fx::firm_to_numeraire_world(world, loan.borrower, paid);
            let interest_n =
                crate::economy::fx::firm_to_numeraire_world(world, loan.borrower, interest_paid);
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, paid_n, "state loan receipt")
                    .credit(Account::InterestIncome, interest_n, "interest income")
                    .credit(
                        Account::Receivables,
                        paid_n - interest_n,
                        "principal received",
                    ),
            );
        }
        Lender::Outside(index) => {
            let paid_n = crate::economy::fx::firm_to_numeraire_world(world, loan.borrower, paid);
            if let Some(lender) = world.outside.lenders.get_mut(index as usize) {
                lender.capital_cents += paid_n;
            }
        }
        Lender::Bank(handle) | Lender::Firm(handle) => {
            if let Some(firm) = world.firms.get_mut(handle) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, paid, "loan receipt")
                        .credit(Account::InterestIncome, interest_paid, "interest income")
                        .credit(
                            Account::Receivables,
                            paid - interest_paid,
                            "principal received",
                        ),
                );
            }
        }
    }
}

/// Quarterly coupon with a persisted sub-cent carry.
fn coupon_due(bond: &Bond) -> (i64, i64) {
    let mut carry = cw_core::money::SubCentCarry::new(10_000 * 4);
    carry.numerator = bond.carry;
    let coupon = carry.accrue(
        bond.principal_cents as i128 * bond.coupon_bps as i128,
        10_000 * 4,
    );
    (coupon.0, carry.numerator)
}

fn service_bonds(world: &mut World, dataset: &Dataset) {
    let _ = dataset;
    let handles: Vec<Handle> = world.bonds.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let Some(bond) = world.bonds.get(handle).cloned() else {
            continue;
        };
        if world.tick.0 >= bond.maturity.0 {
            // A coupon that falls on the maturity date is still owed before principal.
            let mut cash = world
                .firms
                .get(bond.issuer)
                .map(|f| f.book.cash())
                .unwrap_or(0)
                .max(0);
            if world.tick.0 >= bond.next_coupon.0 {
                let (coupon, _) = coupon_due(&bond);
                let paid = coupon.min(cash);
                if paid > 0 {
                    let _ = world.firms.get_mut(bond.issuer).unwrap().book.commit(
                        world.tick,
                        Txn::new()
                            .credit(Account::Cash, paid, "final coupon")
                            .debit(Account::InterestExpense, paid, "final coupon"),
                    );
                    distribute_to_holders(world, &bond, paid);
                    cash -= paid;
                }
                if paid < coupon {
                    crate::economy::insolvency::defer_expense(
                        world,
                        bond.issuer,
                        coupon - paid,
                        Account::InterestExpense,
                        "unpaid final coupon",
                    );
                }
            }
            let paid = bond.principal_cents.min(cash);
            if paid > 0 {
                let _ = world.firms.get_mut(bond.issuer).unwrap().book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, paid, "bond maturity")
                        .debit(Account::Bonds, paid, "bond maturity"),
                );
                distribute_principal(world, handle, &bond, paid);
            }
            if paid < bond.principal_cents {
                crate::economy::structure::note_bond_default(world);
                crate::economy::insolvency::defer_expense(
                    world,
                    bond.issuer,
                    bond.principal_cents - paid,
                    Account::OperatingExpense,
                    "unpaid bond principal",
                );
            }
            world.bonds.remove(handle);
            continue;
        }
        if world.tick.0 >= bond.next_coupon.0 {
            let (coupon, carry_next) = coupon_due(&bond);
            let cash = world
                .firms
                .get(bond.issuer)
                .map(|f| f.book.cash())
                .unwrap_or(0)
                .max(0);
            let paid = coupon.min(cash);
            if paid > 0 {
                let _ = world.firms.get_mut(bond.issuer).unwrap().book.commit(
                    world.tick,
                    Txn::new().credit(Account::Cash, paid, "coupon").debit(
                        Account::InterestExpense,
                        paid,
                        "coupon",
                    ),
                );
                distribute_to_holders(world, &bond, paid);
            }
            if paid < coupon {
                crate::economy::insolvency::defer_expense(
                    world,
                    bond.issuer,
                    coupon - paid,
                    Account::InterestExpense,
                    "unpaid coupon",
                );
            }
            if let Some(stored) = world.bonds.get_mut(handle) {
                stored.carry = carry_next;
                stored.next_coupon = Tick(stored.next_coupon.0 + 13);
            }
        }
    }
}

/// Principal returned at maturity. A holder's cost comes off its investment and the difference
/// from what it paid is its gain or loss, so the principal never shows up as interest.
fn distribute_principal(world: &mut World, handle: Handle, bond: &Bond, amount: i64) {
    let total: i64 = bond.holders.values().sum();
    if total <= 0 {
        return;
    }
    for (holder, qty) in &bond.holders {
        let share = (amount as i128 * *qty as i128 / total as i128) as i64;
        if share <= 0 {
            continue;
        }
        let released =
            crate::economy::structure::release_basis(world, handle, *holder, share, *qty);
        let released = if matches!(holder, HolderId::Investor) {
            released
        } else {
            share
        };
        let gain = share - released;
        let lines = |memo: &str| {
            Txn::new()
                .debit(Account::Cash, share, memo)
                .credit(Account::Investments, released, memo)
                .signed(Account::OtherIncome, -gain, memo)
        };
        match holder {
            HolderId::Investor => {
                world.investor.cash_cents += share;
                let _ = world
                    .investor
                    .book
                    .commit(world.tick, lines("bond principal"));
            }
            HolderId::Firm(h) => {
                if let Some(firm) = world.firms.get_mut(*h) {
                    let _ = firm.book.commit(world.tick, lines("bond principal"));
                }
            }
            HolderId::Outside(index) => {
                if let Some(holder) = world.outside.investors.get_mut(*index as usize) {
                    holder.capital_cents += share;
                }
            }
            HolderId::Treasury | HolderId::State => {}
        }
    }
}

fn distribute_to_holders(world: &mut World, bond: &Bond, amount: i64) {
    let total: i64 = bond.holders.values().sum();
    if total <= 0 {
        return;
    }
    for (holder, qty) in &bond.holders {
        let share = (amount as i128 * *qty as i128 / total as i128) as i64;
        match holder {
            HolderId::Investor => {
                world.investor.cash_cents += share;
                let _ = world.investor.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, share, "bond coupon")
                        .credit(Account::InterestIncome, share, "bond coupon"),
                );
            }
            HolderId::Firm(handle) => {
                if let Some(firm) = world.firms.get_mut(*handle) {
                    let _ = firm.book.commit(
                        world.tick,
                        Txn::new()
                            .debit(Account::Cash, share, "bond coupon")
                            .credit(Account::InterestIncome, share, "bond coupon"),
                    );
                }
            }
            HolderId::Outside(index) => {
                if let Some(holder) = world.outside.investors.get_mut(*index as usize) {
                    holder.capital_cents += share;
                }
            }
            HolderId::Treasury | HolderId::State => {}
        }
    }
}

/// Maximum new secured loan: 50 percent of unpledged tangible assets.
pub fn secured_capacity(world: &World, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let pledged: i64 = entity
        .loans
        .iter()
        .filter_map(|handle| world.loans.get(*handle))
        .filter(|loan| loan.secured)
        .map(|loan| loan.principal_remaining_cents)
        .sum();
    (entity.book.net_tangible_assets() * 50 / 100 - pledged).max(0)
}

/// Unsecured capacity: thirteen weeks of positive operating cash flow, A/B grade only.
pub fn unsecured_capacity(world: &World, firm: Handle) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    if !matches!(entity.grade, Grade::A | Grade::B) {
        return 0;
    }
    let ebitda: i64 = entity
        .book
        .recent_weeks()
        .iter()
        .map(|week| week.ebitda())
        .sum();
    (ebitda.max(0) / entity.book.recent_weeks().len().max(1) as i64 * 13).max(0)
}

/// Repay loan principal from the borrower's cash to the actual lender. The caller has checked
/// authority and that the borrower holds the cash.
pub fn repay(world: &mut World, loan: Handle, amount: i64) {
    let Some(entity) = world.loans.get(loan) else {
        return;
    };
    let payment = amount.min(entity.principal_remaining_cents).max(0);
    if payment == 0 {
        return;
    }
    let borrower = entity.borrower;
    let lender = entity.lender;
    if let Some(firm) = world.firms.get_mut(borrower) {
        let _ = firm.book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, payment, "repayment")
                .debit(Account::Loans, payment, "repayment"),
        );
    }
    match lender {
        Lender::Outside(crate::state::STATE_LENDER) => {
            let payment_n = crate::economy::fx::firm_to_numeraire_world(world, borrower, payment);
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, payment_n, "state loan repaid")
                    .credit(Account::Receivables, payment_n, "state loan repaid"),
            );
        }
        Lender::Outside(index) => {
            let payment_n = crate::economy::fx::firm_to_numeraire_world(world, borrower, payment);
            if let Some(holder) = world.outside.lenders.get_mut(index as usize) {
                holder.capital_cents += payment_n;
            }
        }
        Lender::Firm(handle) | Lender::Bank(handle) => {
            if let Some(firm) = world.firms.get_mut(handle) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, payment, "repayment received")
                        .credit(Account::Receivables, payment, "repayment received"),
                );
            }
        }
    }
    if let Some(stored) = world.loans.get_mut(loan) {
        stored.principal_remaining_cents -= payment;
        stored.arrears_cents = (stored.arrears_cents - payment).max(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn negative_equity_is_grade_d() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 41, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let handle = world.firms.iter().next().unwrap().0;
        crate::economy::insolvency::defer_expense(
            &mut world,
            handle,
            500_000_000,
            Account::OperatingExpense,
            "loss",
        );
        world
            .firms
            .get_mut(handle)
            .unwrap()
            .book
            .close_week(world.tick);
        assert_eq!(assess_grade(&world, handle), Grade::D);
    }

    #[test]
    fn a_bond_pays_its_final_coupon_before_principal() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            781,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        let issuer = world.firms.iter().next().unwrap().0;
        world
            .firms
            .get_mut(issuer)
            .unwrap()
            .book
            .commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, 200_000_000, "funding")
                    .credit(Account::ShareCapital, 200_000_000, "funding"),
            )
            .unwrap();
        let mut holders = std::collections::BTreeMap::new();
        holders.insert(crate::state::HolderId::Outside(0), 100_000_000i64);
        let bond = world.bonds.insert(Bond {
            issuer,
            principal_cents: 100_000_000,
            coupon_bps: 600,
            maturity: Tick(52),
            issued: Tick(0),
            holders,
            carry: 0,
            next_coupon: Tick(52),
        });
        world.tick = Tick(52);
        let outside_before = world.outside.investors[0].capital_cents;
        let issuer_cash = world.firm(issuer).unwrap().book.cash();
        service(&mut world, &dataset);
        assert!(world.bonds.get(bond).is_none(), "the bond is retired");
        let received = world.outside.investors[0].capital_cents - outside_before;
        // Principal 100,000,000 plus one 6 percent annual coupon quarter (1,500,000).
        assert_eq!(received, 101_500_000);
        // Other debt service may also run in the same weekly pass, so the issuer pays at least
        // the bond amount while the holder receives exactly it.
        assert!(world.firm(issuer).unwrap().book.cash() <= issuer_cash - 101_500_000);
        assert!(world.firm(issuer).unwrap().book.is_balanced());
    }

    #[test]
    fn secured_capacity_cannot_reuse_a_pledge() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 44, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let firm = world.firms.iter().next().unwrap().0;
        let before = secured_capacity(&world, firm);
        assert!(before > 0);
        let loan = world.loans.insert(Loan {
            borrower: firm,
            lender: Lender::Outside(0),
            principal_remaining_cents: 1_000_000,
            rate_bps: 600,
            weekly_principal_cents: 0,
            remaining_weeks: 52,
            collateral: None,
            secured: true,
            created: world.tick,
            carry: 0,
            defaulted: false,
            arrears_cents: 0,
        });
        world.firms.get_mut(firm).unwrap().loans.push(loan);
        assert_eq!(secured_capacity(&world, firm), before - 1_000_000);
    }

    #[test]
    fn a_missed_payment_marks_the_loan_defaulted() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 45, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let firm = world
            .firms
            .iter()
            .find(|(_, firm)| firm.operative() && firm.loans.is_empty() && firm.book.cash() > 0)
            .map(|(handle, _)| handle)
            .expect("an unborrowed employer");
        let cash = world.firm(firm).unwrap().book.cash();
        world
            .firms
            .get_mut(firm)
            .unwrap()
            .book
            .commit(
                world.tick,
                cw_core::ledger::Txn::new()
                    .debit(cw_core::ledger::Account::RetainedEarnings, cash, "drain")
                    .credit(cw_core::ledger::Account::Cash, cash, "drain"),
            )
            .unwrap();
        let loan = world.loans.insert(Loan {
            borrower: firm,
            lender: Lender::Outside(0),
            principal_remaining_cents: 10_000_000,
            rate_bps: 800,
            weekly_principal_cents: 1_000_000,
            remaining_weeks: 10,
            collateral: None,
            secured: false,
            created: world.tick,
            carry: 0,
            defaulted: false,
            arrears_cents: 0,
        });
        world.firms.get_mut(firm).unwrap().loans.push(loan);
        service(&mut world, &dataset);
        assert!(world.loans.get(loan).unwrap().defaulted);
        assert!(world.firm(firm).unwrap().book.is_balanced());
    }

    #[test]
    fn weekly_interest_carry_posts_the_exact_term_amount() {
        let mut loan = Loan {
            borrower: Handle::INVALID,
            lender: Lender::Outside(0),
            principal_remaining_cents: 100_000_000,
            rate_bps: 800,
            weekly_principal_cents: 0,
            remaining_weeks: 104,
            collateral: None,
            secured: false,
            created: Tick::OPENING,
            carry: 0,
            defaulted: false,
            arrears_cents: 0,
        };
        let mut posted = 0i64;
        for _ in 0..13 {
            let (interest, carry) = accrued_interest(&loan);
            posted += interest;
            loan.carry = carry;
        }
        // 100,000,000 at 8 percent for one quarter is exactly 2,000,000 cents.
        assert_eq!(posted, 2_000_000);
        assert_eq!(loan.carry, 0);
    }

    #[test]
    fn interest_is_a_52_week_rule() {
        let loan = Loan {
            borrower: Handle::INVALID,
            lender: Lender::Outside(0),
            principal_remaining_cents: 100_000_000,
            rate_bps: 800,
            weekly_principal_cents: 0,
            remaining_weeks: 104,
            collateral: None,
            secured: false,
            created: Tick::OPENING,
            carry: 0,
            defaulted: false,
            arrears_cents: 0,
        };
        // 100,000,000 * 8% / 52 = 153,846.15 -> 153,846 cents.
        assert_eq!(weekly_interest(&loan), 153_846);
    }
}
