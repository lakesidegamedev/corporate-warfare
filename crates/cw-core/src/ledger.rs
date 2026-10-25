//! Double-entry ledger with checked arithmetic and a bounded posting journal.
//!
//! Sign convention: debits positive, credits negative. Assets and expenses are debit-positive;
//! liabilities, equity and revenue are credit-negative. Every committed transaction sums to
//! zero across its lines, so every ledger's total balance is always zero and each firm's
//! balance sheet identity holds by construction.
//!
//! Profit-and-loss accounts are closed into retained earnings at the end of every week. The
//! journal keeps thirteen weeks of detailed postings; older postings are folded into a
//! chained hash so compaction cannot silently change the future.

use serde::{Deserialize, Serialize};

use crate::calendar::Tick;
use crate::hash::{CanonicalHasher, DOMAIN_STATE};

/// Number of accounts in the chart.
pub const ACCOUNT_COUNT: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(usize)]
pub enum Account {
    Cash = 0,
    RestrictedCash = 1,
    Receivables = 2,
    PrepaidExpenses = 3,
    Inventory = 4,
    WorkInProgress = 5,
    FixedAssets = 6,
    AccumulatedDepreciation = 7,
    Intangibles = 8,
    Investments = 9,
    OtherAssets = 10,
    Payables = 11,
    OverduePayables = 12,
    AccruedLiabilities = 13,
    TaxesPayable = 14,
    UnearnedRevenue = 15,
    Loans = 16,
    Bonds = 17,
    Deposits = 18,
    ClaimLiabilities = 19,
    Provisions = 20,
    ShareCapital = 21,
    RetainedEarnings = 22,
    Revenue = 23,
    CostOfGoods = 24,
    Wages = 25,
    OperatingExpense = 26,
    DepreciationExpense = 27,
    InterestExpense = 28,
    InterestIncome = 29,
    TaxExpense = 30,
    OtherIncome = 31,
}

impl Account {
    pub const ALL: [Account; ACCOUNT_COUNT] = [
        Account::Cash,
        Account::RestrictedCash,
        Account::Receivables,
        Account::PrepaidExpenses,
        Account::Inventory,
        Account::WorkInProgress,
        Account::FixedAssets,
        Account::AccumulatedDepreciation,
        Account::Intangibles,
        Account::Investments,
        Account::OtherAssets,
        Account::Payables,
        Account::OverduePayables,
        Account::AccruedLiabilities,
        Account::TaxesPayable,
        Account::UnearnedRevenue,
        Account::Loans,
        Account::Bonds,
        Account::Deposits,
        Account::ClaimLiabilities,
        Account::Provisions,
        Account::ShareCapital,
        Account::RetainedEarnings,
        Account::Revenue,
        Account::CostOfGoods,
        Account::Wages,
        Account::OperatingExpense,
        Account::DepreciationExpense,
        Account::InterestExpense,
        Account::InterestIncome,
        Account::TaxExpense,
        Account::OtherIncome,
    ];

    pub const fn index(self) -> usize {
        self as usize
    }

    pub const fn is_profit_and_loss(self) -> bool {
        (self as usize) >= Account::Revenue as usize
    }

    pub const fn is_asset(self) -> bool {
        (self as usize) <= Account::OtherAssets as usize
    }

    pub const fn is_liability(self) -> bool {
        (self as usize) >= Account::Payables as usize
            && (self as usize) <= Account::Provisions as usize
    }

    pub const fn is_equity(self) -> bool {
        (self as usize) == Account::ShareCapital as usize
            || (self as usize) == Account::RetainedEarnings as usize
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Posting {
    pub week: Tick,
    pub txn: u64,
    pub account: Account,
    pub delta: i64,
    pub memo: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxnLine {
    pub account: Account,
    pub delta: i64,
    pub memo: String,
}

/// A staged balanced transaction.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Txn {
    pub lines: Vec<TxnLine>,
    /// Only settlement code converting an overdraft into a past-due liability may set this.
    pub allow_overdraft: bool,
}

impl Txn {
    pub fn new() -> Self {
        Txn::default()
    }

    /// Debit an account (assets and expenses increase).
    pub fn debit(mut self, account: Account, amount: i64, memo: &str) -> Self {
        if amount != 0 {
            self.lines.push(TxnLine {
                account,
                delta: amount,
                memo: memo.to_owned(),
            });
        }
        self
    }

    /// Credit an account (liabilities, equity and revenue increase).
    pub fn credit(mut self, account: Account, amount: i64, memo: &str) -> Self {
        if amount != 0 {
            self.lines.push(TxnLine {
                account,
                delta: -amount,
                memo: memo.to_owned(),
            });
        }
        self
    }

    /// Move value from one account to another within the same ledger.
    pub fn transfer(self, from: Account, to: Account, amount: i64, memo: &str) -> Self {
        self.credit(from, amount, memo).debit(to, amount, memo)
    }

    pub fn signed(mut self, account: Account, delta: i64, memo: &str) -> Self {
        if delta != 0 {
            self.lines.push(TxnLine {
                account,
                delta,
                memo: memo.to_owned(),
            });
        }
        self
    }

    pub fn sum(&self) -> i128 {
        self.lines.iter().map(|line| line.delta as i128).sum()
    }

    pub fn is_balanced(&self) -> bool {
        self.sum() == 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LedgerError {
    Empty,
    Unbalanced,
    Overflow,
    NegativeCash,
    JournalLimit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lifetime {
    pub revenue: i64,
    pub cost_of_goods: i64,
    pub wages: i64,
    pub operating_expense: i64,
    pub depreciation: i64,
    pub interest_expense: i64,
    pub interest_income: i64,
    pub tax: i64,
    pub other: i64,
    pub net_income: i64,
}

/// Result of closing one week of profit-and-loss activity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WeekClose {
    pub week: Tick,
    pub revenue: i64,
    pub cost_of_goods: i64,
    pub wages: i64,
    pub operating_expense: i64,
    pub depreciation: i64,
    pub interest_expense: i64,
    pub interest_income: i64,
    pub tax: i64,
    pub other: i64,
    pub net_income: i64,
}

impl WeekClose {
    pub fn ebitda(&self) -> i64 {
        self.revenue - self.cost_of_goods - self.wages - self.operating_expense + self.other
    }

    pub fn ebit(&self) -> i64 {
        self.ebitda() - self.depreciation
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ledger {
    balances: [i64; ACCOUNT_COUNT],
    journal: Vec<Posting>,
    /// BLAKE3 chain over postings pruned out of the thirteen-week window.
    pruned_hash: [u8; 32],
    txn_seq: u64,
    lifetime: Lifetime,
    recent_weeks: Vec<WeekClose>,
    overdrawn: bool,
}

impl Default for Ledger {
    fn default() -> Self {
        Ledger {
            balances: [0; ACCOUNT_COUNT],
            journal: Vec::new(),
            pruned_hash: [0; 32],
            txn_seq: 0,
            lifetime: Lifetime::default(),
            recent_weeks: Vec::new(),
            overdrawn: false,
        }
    }
}

impl Ledger {
    pub const JOURNAL_WEEKS: u32 = 13;
    pub const RECENT_WEEKS: usize = 13;

    pub fn balance(&self, account: Account) -> i64 {
        self.balances[account.index()]
    }

    pub fn cash(&self) -> i64 {
        self.balances[Account::Cash.index()]
    }

    pub fn restricted_cash(&self) -> i64 {
        self.balances[Account::RestrictedCash.index()]
    }

    pub fn lifetime(&self) -> Lifetime {
        self.lifetime
    }

    pub fn recent_weeks(&self) -> &[WeekClose] {
        &self.recent_weeks
    }

    pub fn pruned_hash(&self) -> [u8; 32] {
        self.pruned_hash
    }

    pub fn txn_seq(&self) -> u64 {
        self.txn_seq
    }

    pub fn journal_len(&self) -> usize {
        self.journal.len()
    }

    pub fn journal(&self) -> &[Posting] {
        &self.journal
    }

    pub fn is_overdrawn(&self) -> bool {
        self.overdrawn
    }

    /// Sum of asset accounts (contra-asset depreciation is stored negative).
    pub fn total_assets(&self) -> i64 {
        Account::ALL
            .iter()
            .filter(|a| a.is_asset())
            .map(|a| self.balance(*a))
            .sum()
    }

    pub fn total_liabilities(&self) -> i64 {
        -Account::ALL
            .iter()
            .filter(|a| a.is_liability())
            .map(|a| self.balance(*a))
            .sum::<i64>()
    }

    pub fn total_equity(&self) -> i64 {
        -Account::ALL
            .iter()
            .filter(|a| a.is_equity())
            .map(|a| self.balance(*a))
            .sum::<i64>()
    }

    /// Net tangible operating asset book value used by valuation and borrowing rules.
    pub fn net_tangible_assets(&self) -> i64 {
        self.balance(Account::FixedAssets)
            + self.balance(Account::AccumulatedDepreciation)
            + self.balance(Account::Inventory)
            + self.balance(Account::WorkInProgress)
            + self.balance(Account::Receivables)
            + self.balance(Account::PrepaidExpenses)
    }

    /// Cash plus restricted cash plus money owed to the entity.
    pub fn liquid_funds(&self) -> i64 {
        self.cash() + self.restricted_cash() + self.balance(Account::Receivables)
    }

    pub fn debt(&self) -> i64 {
        -(self.balance(Account::Loans)
            + self.balance(Account::Bonds)
            + self.balance(Account::Deposits)
            + self.balance(Account::OverduePayables))
    }

    /// True only when every posting balances and the chart totals zero.
    pub fn is_balanced(&self) -> bool {
        self.balances.iter().map(|b| *b as i128).sum::<i128>() == 0
    }

    pub fn commit(&mut self, week: Tick, txn: Txn) -> Result<u64, LedgerError> {
        if txn.lines.is_empty() {
            return Err(LedgerError::Empty);
        }
        if !txn.is_balanced() {
            return Err(LedgerError::Unbalanced);
        }
        // Validate every resulting balance for overflow before mutating anything.
        let mut staged = self.balances;
        for line in &txn.lines {
            let index = line.account.index();
            staged[index] = staged[index]
                .checked_add(line.delta)
                .ok_or(LedgerError::Overflow)?;
        }
        if staged[Account::Cash.index()] < 0 && !txn.allow_overdraft {
            return Err(LedgerError::NegativeCash);
        }
        self.balances = staged;
        if self.balances[Account::Cash.index()] < 0 {
            self.overdrawn = true;
        }
        self.txn_seq += 1;
        let txn_id = self.txn_seq;
        for line in txn.lines {
            self.journal.push(Posting {
                week,
                txn: txn_id,
                account: line.account,
                delta: line.delta,
                memo: line.memo,
            });
        }
        Ok(txn_id)
    }

    /// Close profit-and-loss accounts into retained earnings and return the week summary.
    ///
    /// This is also the journal compaction point: postings older than thirteen weeks are
    /// folded into the chained hash. Compaction never touches a balance.
    pub fn close_week(&mut self, week: Tick) -> WeekClose {
        let revenue = -self.balance(Account::Revenue);
        let cost_of_goods = self.balance(Account::CostOfGoods);
        let wages = self.balance(Account::Wages);
        let operating_expense = self.balance(Account::OperatingExpense);
        let depreciation = self.balance(Account::DepreciationExpense);
        let interest_expense = self.balance(Account::InterestExpense);
        let interest_income = -self.balance(Account::InterestIncome);
        let tax = self.balance(Account::TaxExpense);
        let other = -self.balance(Account::OtherIncome);
        let net_income =
            revenue - cost_of_goods - wages - operating_expense - depreciation - interest_expense
                + interest_income
                - tax
                + other;

        let mut txn = Txn::new();
        for account in Account::ALL.iter().filter(|a| a.is_profit_and_loss()) {
            let balance = self.balance(*account);
            if balance != 0 {
                txn = txn.signed(*account, -balance, "week close");
            }
        }
        txn = txn.signed(Account::RetainedEarnings, -net_income, "week close");
        if !txn.lines.is_empty() {
            // Closing lines always balance because the P&L accounts sum to net income.
            let _ = self.commit(week, txn);
        }

        let close = WeekClose {
            week,
            revenue,
            cost_of_goods,
            wages,
            operating_expense,
            depreciation,
            interest_expense,
            interest_income,
            tax,
            other,
            net_income,
        };
        self.lifetime.revenue += revenue;
        self.lifetime.cost_of_goods += cost_of_goods;
        self.lifetime.wages += wages;
        self.lifetime.operating_expense += operating_expense;
        self.lifetime.depreciation += depreciation;
        self.lifetime.interest_expense += interest_expense;
        self.lifetime.interest_income += interest_income;
        self.lifetime.tax += tax;
        self.lifetime.other += other;
        self.lifetime.net_income += net_income;
        self.recent_weeks.push(close);
        if self.recent_weeks.len() > Self::RECENT_WEEKS {
            self.recent_weeks.remove(0);
        }
        self.prune(week);
        self.overdrawn = self.balances[Account::Cash.index()] < 0;
        close
    }

    fn prune(&mut self, week: Tick) {
        let cutoff = week.0.saturating_sub(Self::JOURNAL_WEEKS - 1);
        if cutoff <= 1 {
            return;
        }
        let mut keep = Vec::with_capacity(self.journal.len());
        let mut hasher = CanonicalHasher::new(DOMAIN_STATE);
        let mut pruned = 0u64;
        for posting in self.journal.drain(..) {
            if posting.week.0 < cutoff {
                hasher
                    .u32(posting.week.0)
                    .u64(posting.txn)
                    .u32(posting.account.index() as u32)
                    .i64(posting.delta)
                    .string(&posting.memo);
                pruned += 1;
            } else {
                keep.push(posting);
            }
        }
        if pruned > 0 {
            let mut chained = CanonicalHasher::new(DOMAIN_STATE);
            chained.bytes(&self.pruned_hash).u64(pruned);
            let new_hash = hasher.finish();
            chained.bytes(&new_hash);
            self.pruned_hash = chained.finish();
        }
        self.journal = keep;
    }

    pub fn canonical_hash(&self, hasher: &mut CanonicalHasher) {
        for balance in self.balances {
            hasher.i64(balance);
        }
        hasher.u64(self.txn_seq).bytes(&self.pruned_hash);
        for close in &self.recent_weeks {
            hasher
                .u32(close.week.0)
                .i64(close.revenue)
                .i64(close.cost_of_goods)
                .i64(close.wages)
                .i64(close.operating_expense)
                .i64(close.depreciation)
                .i64(close.interest_expense)
                .i64(close.interest_income)
                .i64(close.tax)
                .i64(close.other)
                .i64(close.net_income);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn week(n: u32) -> Tick {
        Tick(n)
    }

    #[test]
    fn borrowing_and_repayment_match_the_contract() {
        let mut ledger = Ledger::default();
        ledger
            .commit(
                week(1),
                Txn::new().debit(Account::Cash, 10_000, "loan").credit(
                    Account::Loans,
                    10_000,
                    "loan",
                ),
            )
            .unwrap();
        assert_eq!(ledger.cash(), 10_000);
        assert_eq!(ledger.debt(), 10_000);
        ledger
            .commit(
                week(2),
                Txn::new()
                    .credit(Account::Cash, 2_200, "repay")
                    .debit(Account::Loans, 2_000, "principal")
                    .debit(Account::InterestExpense, 200, "interest"),
            )
            .unwrap();
        assert_eq!(ledger.cash(), 7_800);
        assert_eq!(ledger.debt(), 8_000);
        let close = ledger.close_week(week(2));
        assert_eq!(close.net_income, -200);
        assert!(ledger.is_balanced());
    }

    #[test]
    fn capex_moves_to_cip_then_fixed_assets() {
        let mut ledger = Ledger::default();
        ledger
            .commit(
                week(1),
                Txn::new().debit(Account::Cash, 100_000, "capital").credit(
                    Account::ShareCapital,
                    100_000,
                    "capital",
                ),
            )
            .unwrap();
        ledger
            .commit(
                week(2),
                Txn::new().transfer(Account::Cash, Account::WorkInProgress, 10_000, "build"),
            )
            .unwrap();
        assert_eq!(ledger.balance(Account::WorkInProgress), 10_000);
        ledger
            .commit(
                week(3),
                Txn::new().transfer(
                    Account::WorkInProgress,
                    Account::FixedAssets,
                    10_000,
                    "complete",
                ),
            )
            .unwrap();
        assert_eq!(ledger.balance(Account::WorkInProgress), 0);
        assert_eq!(ledger.balance(Account::FixedAssets), 10_000);
        // Capitalising an asset is not an expense.
        assert_eq!(ledger.close_week(week(3)).net_income, 0);
    }

    #[test]
    fn inventory_purchase_and_sale_gross_profit() {
        let mut ledger = Ledger::default();
        ledger
            .commit(
                week(1),
                Txn::new()
                    .debit(Account::Cash, 1_000_000, "capital")
                    .credit(Account::ShareCapital, 1_000_000, "capital"),
            )
            .unwrap();
        // Buy ten units for $20 total.
        ledger
            .commit(
                week(1),
                Txn::new().transfer(Account::Cash, Account::Inventory, 2_000, "buy"),
            )
            .unwrap();
        // Sell four for $12, carrying cost $8.
        ledger
            .commit(
                week(1),
                Txn::new()
                    .debit(Account::Cash, 1_200, "sale")
                    .credit(Account::Revenue, 1_200, "sale")
                    .credit(Account::Inventory, 800, "cogs")
                    .debit(Account::CostOfGoods, 800, "cogs"),
            )
            .unwrap();
        let close = ledger.close_week(week(1));
        assert_eq!(close.revenue, 1_200);
        assert_eq!(close.cost_of_goods, 800);
        assert_eq!(close.net_income, 400);
        assert_eq!(ledger.balance(Account::Inventory), 1_200);
        assert!(ledger.is_balanced());
    }

    #[test]
    fn negative_cash_is_rejected_unless_settlement_allows_it() {
        let mut ledger = Ledger::default();
        let rejected = ledger.commit(
            week(1),
            Txn::new().credit(Account::Cash, 500, "spend").debit(
                Account::OperatingExpense,
                500,
                "spend",
            ),
        );
        assert_eq!(rejected, Err(LedgerError::NegativeCash));
        assert_eq!(ledger.cash(), 0);
        assert_eq!(ledger.txn_seq(), 0);
    }

    #[test]
    fn unbalanced_transaction_leaves_no_trace() {
        let mut ledger = Ledger::default();
        let result = ledger.commit(week(1), Txn::new().debit(Account::Cash, 100, "typo"));
        assert_eq!(result, Err(LedgerError::Unbalanced));
        assert_eq!(ledger.cash(), 0);
        assert_eq!(ledger.journal_len(), 0);
    }

    #[test]
    fn journal_is_bounded_and_pruning_chain_advances() {
        let mut ledger = Ledger::default();
        ledger
            .commit(
                week(1),
                Txn::new()
                    .debit(Account::Cash, 1_000_000, "capital")
                    .credit(Account::ShareCapital, 1_000_000, "capital"),
            )
            .unwrap();
        for tick in 1..=40u32 {
            ledger
                .commit(
                    week(tick),
                    Txn::new().credit(Account::Cash, 1, "fee").debit(
                        Account::OperatingExpense,
                        1,
                        "fee",
                    ),
                )
                .unwrap();
            ledger.close_week(week(tick));
        }
        let oldest_allowed = 40 - (Ledger::JOURNAL_WEEKS - 1);
        assert!(ledger.journal().iter().all(|p| p.week.0 >= oldest_allowed));
        assert_ne!(ledger.pruned_hash(), [0; 32]);
        // Balances are unaffected by pruning.
        assert_eq!(ledger.cash(), 1_000_000 - 40);
    }

    #[test]
    fn every_committed_transaction_keeps_the_chart_balanced() {
        let mut ledger = Ledger::default();
        let mut week_no = 0;
        for i in 0..50 {
            week_no += 1;
            ledger
                .commit(
                    week(week_no),
                    Txn::new()
                        .debit(Account::Cash, 10_000 + i, "capital")
                        .credit(Account::ShareCapital, 10_000 + i, "capital"),
                )
                .unwrap();
            ledger
                .commit(
                    week(week_no),
                    Txn::new().credit(Account::Cash, 100 + i, "buy").debit(
                        Account::Inventory,
                        100 + i,
                        "buy",
                    ),
                )
                .unwrap();
            ledger.close_week(week(week_no));
            assert!(ledger.is_balanced());
            assert_eq!(
                ledger.total_assets() - ledger.total_liabilities() - ledger.total_equity(),
                0
            );
        }
    }
}
