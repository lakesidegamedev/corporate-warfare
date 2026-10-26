//! Firm valuation.
//!
//! Weekly fair enterprise value = max(40 percent of net tangible operating asset book value,
//! 6 * max(0, trailing 13-week EBITDA * 4)). Banks and insurers use adjusted tangible equity
//! instead of an EBITDA multiple. Equity fair value subtracts loans, bonds, deposits, claim
//! liabilities and overdue payables, and adds unrestricted cash and look-through stake values.
//! Parent book investment is never added alongside the stake's fair value.

use serde::{Deserialize, Serialize};

use cw_core::calendar::Tick;
use cw_core::ledger::{Account, Ledger};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct FirmValue {
    pub enterprise_cents: i64,
    pub equity_cents: i64,
    pub liquidation_cents: i64,
    pub net_tangible_cents: i64,
    pub ebitda_trailing_cents: i64,
    pub computed_week: Tick,
}

/// Trailing thirteen-week EBITDA from closed weekly reports.
pub fn trailing_ebitda(ledger: &Ledger) -> i64 {
    ledger.recent_weeks().iter().map(|week| week.ebitda()).sum()
}

/// Annual EBITDA from the closed weeks: the trailing thirteen weeks times four, or the
/// available weeks scaled to a year before thirteen exist. (Before 1.4.0 a full quarter was used
/// unscaled, so every valuation fell to about a quarter of the rule at week thirteen.)
pub fn annualised_ebitda(ledger: &Ledger) -> i64 {
    let weeks = ledger.recent_weeks().len() as i64;
    if weeks == 0 {
        return 0;
    }
    trailing_ebitda(ledger) * 52 / weeks.min(13)
}

pub fn ordinary_enterprise_value(ledger: &Ledger) -> i64 {
    let net_tangible = ledger.net_tangible_assets();
    let book_floor = net_tangible * 40 / 100;
    let ebitda = annualised_ebitda(ledger).max(0);
    let multiple = ebitda * 6;
    book_floor.max(multiple)
}

/// Financial businesses (banks and insurers) are valued on adjusted tangible equity.
///
/// A bank's loans outstanding to customers are carried in `Receivables`, because the chart
/// keeps `Loans` for the firm's own borrowing. This is stated in the banking rules and is
/// why the loan asset is not double counted with the firm's funding.
pub fn financial_enterprise_value(ledger: &Ledger) -> i64 {
    let assets = ledger.balance(Account::Cash)
        + ledger.balance(Account::RestrictedCash)
        + ledger.balance(Account::Receivables)
        + ledger.balance(Account::Investments)
        + ledger.balance(Account::FixedAssets)
        + ledger.balance(Account::AccumulatedDepreciation)
        + ledger.balance(Account::OtherAssets);
    let liabilities = ledger.balance(Account::Loans)
        + ledger.balance(Account::Bonds)
        + ledger.balance(Account::Deposits)
        + ledger.balance(Account::ClaimLiabilities)
        + ledger.balance(Account::Payables)
        + ledger.balance(Account::OverduePayables)
        + ledger.balance(Account::AccruedLiabilities);
    (assets + liabilities).max(0)
}

pub fn liquidation_value(ledger: &Ledger) -> i64 {
    // Assets realise at 40 percent of carrying value; liabilities are paid in full.
    let assets = ledger.total_assets() * 40 / 100;
    (assets - ledger.total_liabilities()).max(0)
}

/// Compute the value record for one firm, before any look-through stakes are added.
pub fn base_value(ledger: &Ledger, week: Tick, financial: bool) -> FirmValue {
    let enterprise = if financial {
        financial_enterprise_value(ledger)
    } else {
        ordinary_enterprise_value(ledger)
    };
    let unrestricted_cash = ledger.cash() + ledger.restricted_cash();
    let debts = ledger.balance(Account::Loans)
        + ledger.balance(Account::Bonds)
        + ledger.balance(Account::Deposits)
        + ledger.balance(Account::ClaimLiabilities)
        + ledger.balance(Account::Payables)
        + ledger.balance(Account::OverduePayables)
        + ledger.balance(Account::AccruedLiabilities);
    let equity = (enterprise + unrestricted_cash + debts).max(0);
    FirmValue {
        enterprise_cents: enterprise,
        equity_cents: equity,
        liquidation_cents: liquidation_value(ledger),
        net_tangible_cents: ledger.net_tangible_assets(),
        ebitda_trailing_cents: trailing_ebitda(ledger),
        computed_week: week,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cw_core::ledger::Txn;

    #[test]
    fn enterprise_uses_the_book_value_floor() {
        let mut ledger = Ledger::default();
        ledger
            .commit(
                Tick(1),
                Txn::new()
                    .debit(Account::Cash, 1_000_000, "capital")
                    .credit(Account::ShareCapital, 1_000_000, "capital"),
            )
            .unwrap();
        ledger
            .commit(
                Tick(2),
                Txn::new().transfer(Account::Cash, Account::FixedAssets, 500_000, "capex"),
            )
            .unwrap();
        let value = base_value(&ledger, Tick(2), false);
        // 40 percent of 500,000 book = 200,000; no EBITDA history yet.
        assert_eq!(value.enterprise_cents, 200_000);
        // Equity = enterprise + unrestricted cash (500,000).
        assert_eq!(value.equity_cents, 700_000);
    }

    #[test]
    fn ebitda_is_annualised_before_and_after_a_full_quarter() {
        // $1,000 of revenue a week and nothing else: $52,000 a year whatever the history length.
        let mut ledger = Ledger::default();
        for week in 1..=20u32 {
            ledger
                .commit(
                    Tick(week),
                    Txn::new().debit(Account::Cash, 100_000, "sale").credit(
                        Account::Revenue,
                        100_000,
                        "sale",
                    ),
                )
                .unwrap();
            ledger.close_week(Tick(week));
            assert_eq!(
                annualised_ebitda(&ledger),
                5_200_000,
                "week {week}: a year of weekly EBITDA"
            );
        }
    }
}
