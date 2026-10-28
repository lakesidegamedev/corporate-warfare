//! Long results history for the stats screens: each firm's week closes folded into four-week
//! periods with closing balances. Nothing in the rules reads it.
//!
//! The archive keeps the deeper presentation history for the financial views: ten years of
//! weekly closing prices, and each firm's statements weekly for two years and quarterly for the
//! whole career. Every record carries an income statement, closing balances, a cash flow
//! statement and, quarterly, each division's estimated results. It lives outside the world's
//! postcard encoding and hash, so it never moves a determinism check.
//!
//! Cash flow derives from the ledger journal: every committed transaction since the last record
//! that moved cash (including restricted cash) is classified by the other accounts it touches.
//! Tax accounts make it tax; borrowing, deposits and equity make it financing; plant,
//! construction, intangibles and holdings make it investing; everything else is operating.
//! Opening cash is closing cash less the classified flows, so the statement always reconciles.

use std::collections::BTreeMap;

use cw_core::calendar::{WEEKS_PER_QUARTER, WEEKS_PER_YEAR};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Ledger};
use cw_data::Dataset;
use serde::{Deserialize, Serialize};

use crate::state::{HISTORY_PERIODS, PERIOD_WEEKS, PeriodClose, PricePoint, World};

/// Weeks of closing prices the archive keeps: ten years.
pub const ARCHIVE_PRICE_WEEKS: usize = 520;
/// Weekly statements the archive keeps per firm: two years.
pub const ARCHIVE_WEEKS: usize = 104;
/// Quarterly statements the archive keeps per firm: forty years, a whole career.
pub const ARCHIVE_QUARTERS: usize = 160;

/// Cash moved in one period, by activity. `opening + operating + investing + financing + tax`
/// is `closing` exactly.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CashFlow {
    pub opening: i64,
    pub operating: i64,
    pub investing: i64,
    pub financing: i64,
    pub tax: i64,
    pub closing: i64,
}

impl CashFlow {
    /// Net change in cash over the period.
    pub fn net(&self) -> i64 {
        self.operating
            .saturating_add(self.investing)
            .saturating_add(self.financing)
            .saturating_add(self.tax)
    }
}

/// One division's estimated results over a period: the firm's plants of one sector type in one
/// region. Plants, blocks and share are as at the period's close.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DivisionPeriod {
    pub region: u8,
    pub industry: u8,
    pub revenue: i64,
    pub costs: i64,
    pub profit: i64,
    pub plants: u32,
    pub blocks: u32,
    pub share_bps: i64,
}

/// One period of a firm's statements: a week, a quarter or a year.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeriodRecord {
    pub first_week: u32,
    pub weeks: u32,
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
    pub cash: i64,
    pub current_assets: i64,
    pub assets: i64,
    pub current_liabilities: i64,
    pub liabilities: i64,
    pub debt: i64,
    pub equity: i64,
    pub inventory: i64,
    pub fixed_assets: i64,
    pub share_price_cents: i64,
    pub shares_issued: i64,
    pub cash_flow: CashFlow,
    pub divisions: Vec<DivisionPeriod>,
}

impl PeriodRecord {
    pub fn gross_profit(&self) -> i64 {
        self.revenue.saturating_sub(self.cost_of_goods)
    }

    /// Earnings before interest and tax.
    pub fn ebit(&self) -> i64 {
        self.revenue
            .saturating_sub(self.cost_of_goods)
            .saturating_sub(self.wages)
            .saturating_sub(self.operating_expense)
            .saturating_sub(self.depreciation)
            .saturating_add(self.other)
    }

    /// Fold a later, adjacent period into this one: flows add, balances and the cash flow's
    /// closing take the later period's close, the opening stays the earlier one's. Divisions
    /// add their results and take the later plant counts and share.
    pub fn absorb(&mut self, later: &PeriodRecord) {
        if self.weeks == 0 {
            self.first_week = later.first_week;
            self.cash_flow.opening = later.cash_flow.opening;
        }
        self.weeks += later.weeks;
        let add = |a: &mut i64, b: i64| *a = a.saturating_add(b);
        add(&mut self.revenue, later.revenue);
        add(&mut self.cost_of_goods, later.cost_of_goods);
        add(&mut self.wages, later.wages);
        add(&mut self.operating_expense, later.operating_expense);
        add(&mut self.depreciation, later.depreciation);
        add(&mut self.interest_expense, later.interest_expense);
        add(&mut self.interest_income, later.interest_income);
        add(&mut self.tax, later.tax);
        add(&mut self.other, later.other);
        add(&mut self.net_income, later.net_income);
        add(&mut self.cash_flow.operating, later.cash_flow.operating);
        add(&mut self.cash_flow.investing, later.cash_flow.investing);
        add(&mut self.cash_flow.financing, later.cash_flow.financing);
        add(&mut self.cash_flow.tax, later.cash_flow.tax);
        self.cash_flow.closing = later.cash_flow.closing;
        self.cash = later.cash;
        self.current_assets = later.current_assets;
        self.assets = later.assets;
        self.current_liabilities = later.current_liabilities;
        self.liabilities = later.liabilities;
        self.debt = later.debt;
        self.equity = later.equity;
        self.inventory = later.inventory;
        self.fixed_assets = later.fixed_assets;
        self.share_price_cents = later.share_price_cents;
        self.shares_issued = later.shares_issued;
        for d in &mut self.divisions {
            d.plants = 0;
            d.blocks = 0;
        }
        for row in &later.divisions {
            match self
                .divisions
                .iter_mut()
                .find(|d| d.region == row.region && d.industry == row.industry)
            {
                Some(d) => {
                    add(&mut d.revenue, row.revenue);
                    add(&mut d.costs, row.costs);
                    add(&mut d.profit, row.profit);
                    d.plants = row.plants;
                    d.blocks = row.blocks;
                    d.share_bps = row.share_bps;
                }
                None => self.divisions.push(row.clone()),
            }
        }
    }
}

/// The archive. Bounded on every axis.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Archive {
    /// Weekly closing prices of every good and share, oldest first.
    pub prices: Vec<PricePoint>,
    /// Each firm's weekly statements, oldest first. Weekly records carry no divisions.
    pub weeks: BTreeMap<Handle, Vec<PeriodRecord>>,
    /// Each firm's statements by calendar quarter, oldest first. The last may be open.
    pub quarters: BTreeMap<Handle, Vec<PeriodRecord>>,
    /// The last journal transaction already classified into cash flow, per firm.
    pub seen_txn: BTreeMap<Handle, u64>,
}

impl Archive {
    pub fn is_empty(&self) -> bool {
        self.prices.is_empty() && self.weeks.is_empty() && self.quarters.is_empty()
    }
}

/// Calendar quarter index of a week: weeks 1 to 13 are quarter 0.
pub fn quarter_index(week: u32) -> u32 {
    week.saturating_sub(1) / WEEKS_PER_QUARTER
}

/// Calendar year index of a week: weeks 1 to 52 are year 0.
pub fn year_index(week: u32) -> u32 {
    week.saturating_sub(1) / WEEKS_PER_YEAR
}

/// Fold quarterly records into calendar years, oldest first. The last year may be partial.
pub fn years(quarters: &[PeriodRecord]) -> Vec<PeriodRecord> {
    let mut out: Vec<PeriodRecord> = Vec::new();
    for q in quarters {
        let year = year_index(q.first_week);
        match out.last_mut() {
            Some(last) if year_index(last.first_week) == year => last.absorb(q),
            _ => {
                let mut fresh = PeriodRecord::default();
                fresh.absorb(q);
                out.push(fresh);
            }
        }
    }
    out
}

/// Which activity a transaction that moved cash belongs to, from the other accounts it touches.
fn activity(accounts: &[Account]) -> usize {
    use Account::*;
    let has = |set: &[Account]| accounts.iter().any(|a| set.contains(a));
    if has(&[TaxesPayable, TaxExpense]) {
        3
    } else if has(&[Loans, Bonds, Deposits, ShareCapital, RetainedEarnings]) {
        2
    } else if has(&[
        FixedAssets,
        AccumulatedDepreciation,
        WorkInProgress,
        Intangibles,
        Investments,
    ]) {
        1
    } else {
        0
    }
}

/// Classify every transaction after `seen` (or, without a marker, this week's) into operating,
/// investing, financing and tax cash flows. Returns the flows and the newest transaction id.
pub fn classify_cash(book: &Ledger, seen: Option<u64>, week: u32) -> ([i64; 4], u64) {
    let mut flows = [0i64; 4];
    let mut newest = seen.unwrap_or(0);
    let take = |txn: u64, posting_week: u32| match seen {
        Some(marker) => txn > marker,
        None => posting_week == week,
    };
    let mut current: Option<u64> = None;
    let mut cash = 0i64;
    let mut others: Vec<Account> = Vec::new();
    let settle = |cash: i64, others: &[Account], flows: &mut [i64; 4]| {
        if cash != 0 {
            let slot = activity(others);
            flows[slot] = flows[slot].saturating_add(cash);
        }
    };
    for posting in book.journal() {
        if !take(posting.txn, posting.week.0) {
            continue;
        }
        if current != Some(posting.txn) {
            settle(cash, &others, &mut flows);
            current = Some(posting.txn);
            cash = 0;
            others.clear();
        }
        newest = newest.max(posting.txn);
        match posting.account {
            Account::Cash | Account::RestrictedCash => cash = cash.saturating_add(posting.delta),
            other => others.push(other),
        }
    }
    settle(cash, &others, &mut flows);
    // Everything committed so far is either classified or, without a marker, older than this
    // week and deliberately left out.
    (flows, newest.max(book.txn_seq()))
}

fn balances(book: &Ledger) -> (i64, i64, i64, i64, i64, i64) {
    let b = |a: Account| book.balance(a);
    let cash = b(Account::Cash).saturating_add(b(Account::RestrictedCash));
    let current_assets = cash
        .saturating_add(b(Account::Receivables))
        .saturating_add(b(Account::PrepaidExpenses))
        .saturating_add(b(Account::Inventory));
    let current_liabilities = -(b(Account::Payables)
        .saturating_add(b(Account::OverduePayables))
        .saturating_add(b(Account::AccruedLiabilities))
        .saturating_add(b(Account::TaxesPayable))
        .saturating_add(b(Account::UnearnedRevenue)));
    let fixed = b(Account::FixedAssets)
        .saturating_add(b(Account::AccumulatedDepreciation))
        .saturating_add(b(Account::WorkInProgress));
    (
        cash,
        current_assets,
        current_liabilities,
        book.debt(),
        b(Account::Inventory),
        fixed,
    )
}

/// Archive this week: copy the closing prices and record every firm's week, folding it into
/// the open calendar quarter with this week's division results. Runs after `record_week`.
pub fn archive_week(world: &mut World, dataset: &Dataset) {
    let week = world.tick.0;
    if world.archive.prices.is_empty() && !world.price_history.is_empty() {
        // A save from before the archive starts it from the year of closes it already has.
        world.archive.prices = world.price_history.clone();
    } else if let Some(point) = world.price_history.last()
        && point.week == week
        && world.archive.prices.last().is_none_or(|p| p.week < week)
    {
        world.archive.prices.push(point.clone());
    }
    let excess = world
        .archive
        .prices
        .len()
        .saturating_sub(ARCHIVE_PRICE_WEEKS);
    world.archive.prices.drain(..excess);

    let mut rows = Vec::new();
    for (handle, firm) in world.firms.iter() {
        let Some(close) = firm.book.recent_weeks().last().copied() else {
            continue;
        };
        if close.week.0 != week {
            continue;
        }
        let book = &firm.book;
        let seen = world.archive.seen_txn.get(&handle).copied();
        let (flows, newest) = classify_cash(book, seen, week);
        let (cash, current_assets, current_liabilities, debt, inventory, fixed) = balances(book);
        let net = flows.iter().fold(0i64, |sum, f| sum.saturating_add(*f));
        let divisions = crate::economy::sectors::divisions_of(world, dataset, handle)
            .into_iter()
            .map(|d| {
                let report = crate::economy::division_report::report(
                    world, dataset, handle, d.region, d.industry,
                );
                let (revenue, profit) = report
                    .as_ref()
                    .map(|r| (r.revenue_cents, r.operating_profit_cents))
                    .unwrap_or((0, 0));
                DivisionPeriod {
                    region: d.region,
                    industry: d.industry,
                    revenue,
                    costs: revenue.saturating_sub(profit),
                    profit,
                    plants: d.plants.len() as u32,
                    blocks: d.blocks,
                    share_bps: crate::economy::sectors::share_bps(
                        world, dataset, handle, d.region, d.industry, 0,
                    ),
                }
            })
            .collect::<Vec<_>>();
        let record = PeriodRecord {
            first_week: week,
            weeks: 1,
            revenue: close.revenue,
            cost_of_goods: close.cost_of_goods,
            wages: close.wages,
            operating_expense: close.operating_expense,
            depreciation: close.depreciation,
            interest_expense: close.interest_expense,
            interest_income: close.interest_income,
            tax: close.tax,
            other: close.other,
            net_income: close.net_income,
            cash,
            current_assets,
            assets: book.total_assets(),
            current_liabilities,
            liabilities: book.total_liabilities(),
            debt,
            equity: book.total_equity(),
            inventory,
            fixed_assets: fixed,
            share_price_cents: firm.shares.mark_cents,
            shares_issued: firm.shares.issued,
            cash_flow: CashFlow {
                opening: cash.saturating_sub(net),
                operating: flows[0],
                investing: flows[1],
                financing: flows[2],
                tax: flows[3],
                closing: cash,
            },
            divisions: Vec::new(),
        };
        rows.push((handle, record, divisions, newest));
    }
    let archive = &mut world.archive;
    for (handle, record, divisions, newest) in rows {
        archive.seen_txn.insert(handle, newest);
        let quarters = archive.quarters.entry(handle).or_default();
        let mut with_divisions = record.clone();
        with_divisions.divisions = divisions;
        match quarters.last_mut() {
            Some(open) if quarter_index(open.first_week) == quarter_index(week) => {
                open.absorb(&with_divisions)
            }
            _ => quarters.push(with_divisions),
        }
        let excess = quarters.len().saturating_sub(ARCHIVE_QUARTERS);
        quarters.drain(..excess);
        let weeks = archive.weeks.entry(handle).or_default();
        weeks.push(record);
        let excess = weeks.len().saturating_sub(ARCHIVE_WEEKS);
        weeks.drain(..excess);
    }
    let firms = &world.firms;
    archive
        .weeks
        .retain(|handle, _| firms.get(*handle).is_some());
    archive
        .quarters
        .retain(|handle, _| firms.get(*handle).is_some());
    archive
        .seen_txn
        .retain(|handle, _| firms.get(*handle).is_some());
}

/// Fold this week's close for every firm into its open period, opening a new one every four
/// weeks, and drop firms that no longer exist.
pub fn record_week(world: &mut World) {
    let week = world.tick.0;
    let mut rows = Vec::new();
    for (handle, firm) in world.firms.iter() {
        let Some(close) = firm.book.recent_weeks().last().copied() else {
            continue;
        };
        if close.week.0 != week {
            continue;
        }
        let book = &firm.book;
        rows.push((
            handle,
            close,
            book.cash() + book.restricted_cash(),
            book.total_assets(),
            book.total_liabilities(),
            book.total_equity(),
            firm.shares.mark_cents,
            firm.shares.issued,
        ));
    }
    for (handle, close, cash, assets, liabilities, equity, price, issued) in rows {
        let periods = world.firm_history.entry(handle).or_default();
        let open = periods
            .last()
            .is_some_and(|p| p.weeks < PERIOD_WEEKS && p.first_week + p.weeks == week);
        if !open {
            periods.push(PeriodClose {
                first_week: week,
                ..PeriodClose::default()
            });
        }
        if let Some(period) = periods.last_mut() {
            period.weeks += 1;
            period.revenue += close.revenue;
            period.cost_of_goods += close.cost_of_goods;
            period.wages += close.wages;
            period.operating_expense += close.operating_expense;
            period.depreciation += close.depreciation;
            period.net_interest += close.interest_income - close.interest_expense;
            period.tax += close.tax;
            period.other += close.other;
            period.net_income += close.net_income;
            period.cash = cash;
            period.assets = assets;
            period.liabilities = liabilities;
            period.equity = equity;
            period.share_price_cents = price;
            period.shares_issued = issued;
        }
        let excess = periods.len().saturating_sub(HISTORY_PERIODS);
        periods.drain(..excess);
    }
    let firms = &world.firms;
    world
        .firm_history
        .retain(|handle, _| firms.get(*handle).is_some());
}

#[cfg(test)]
mod tests {
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn weeks_fold_into_four_week_periods_with_closing_balances() {
        let dataset = cw_data::Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 41, ScenarioKind::Guided, ScenarioMode::Sandbox);
        let start = world.tick.0;
        for _ in 0..30 {
            world.pending_pause = None;
            crate::tick::advance(&mut world, &dataset);
        }
        let weeks = world.tick.0 - start;
        assert!(weeks >= 20, "the world advanced");
        let (handle, periods) = world
            .firm_history
            .iter()
            .max_by_key(|(_, p)| p.iter().map(|x| x.weeks).sum::<u32>())
            .expect("firms have history");
        let recorded: u32 = periods.iter().map(|p| p.weeks).sum();
        assert_eq!(recorded, weeks, "every closed week is in a period");
        let (last, full) = periods.split_last().unwrap();
        assert!(
            full.iter().all(|p| p.weeks == 4),
            "closed periods are four weeks"
        );
        assert!((1..=4).contains(&last.weeks));
        let book = &world.firms.get(*handle).unwrap().book;
        assert_eq!(
            last.cash,
            book.cash() + book.restricted_cash(),
            "closing cash is the book's"
        );
        let open: i64 = book
            .recent_weeks()
            .iter()
            .rev()
            .take(last.weeks as usize)
            .map(|w| w.net_income)
            .sum();
        assert_eq!(last.net_income, open, "the open period sums its weeks");
    }

    #[test]
    fn cash_flow_classifies_each_transaction_by_its_other_accounts() {
        use super::classify_cash;
        use cw_core::calendar::Tick;
        use cw_core::ledger::{Account, Ledger, Txn};
        let mut book = Ledger::default();
        let week = Tick(3);
        let post = |book: &mut Ledger, txn: Txn| book.commit(week, txn).unwrap();
        // Paid in capital 50,000.00, a 20,000.00 loan, 12,000.00 of sales, 3,000.00 of wages,
        // a 9,000.00 plant, 1,500.00 of tax paid, 400.00 of loan interest and 2,000.00 repaid.
        post(
            &mut book,
            Txn::new().debit(Account::Cash, 5_000_000, "").credit(
                Account::ShareCapital,
                5_000_000,
                "",
            ),
        );
        post(
            &mut book,
            Txn::new()
                .debit(Account::Cash, 2_000_000, "")
                .credit(Account::Loans, 2_000_000, ""),
        );
        post(
            &mut book,
            Txn::new()
                .debit(Account::Cash, 1_200_000, "")
                .credit(Account::Revenue, 1_200_000, ""),
        );
        post(
            &mut book,
            Txn::new()
                .debit(Account::Wages, 300_000, "")
                .credit(Account::Cash, 300_000, ""),
        );
        post(
            &mut book,
            Txn::new()
                .debit(Account::FixedAssets, 900_000, "")
                .credit(Account::Cash, 900_000, ""),
        );
        post(
            &mut book,
            Txn::new()
                .debit(Account::TaxExpense, 150_000, "")
                .credit(Account::Cash, 150_000, ""),
        );
        post(
            &mut book,
            Txn::new()
                .debit(Account::InterestExpense, 40_000, "")
                .credit(Account::Cash, 40_000, ""),
        );
        post(
            &mut book,
            Txn::new()
                .debit(Account::Loans, 200_000, "")
                .credit(Account::Cash, 200_000, ""),
        );
        // Moving money into restricted cash is not a cash flow at all.
        post(
            &mut book,
            Txn::new().transfer(Account::Cash, Account::RestrictedCash, 70_000, ""),
        );
        let (flows, newest) = classify_cash(&book, None, 3);
        assert_eq!(
            flows,
            [
                1_200_000 - 300_000 - 40_000,
                -900_000,
                5_000_000 + 2_000_000 - 200_000,
                -150_000
            ]
        );
        assert_eq!(newest, 9);
        let total = book.cash() + book.restricted_cash();
        assert_eq!(
            flows.iter().sum::<i64>(),
            total,
            "flows explain every cent from zero"
        );
        // With a marker, only later transactions count.
        post(
            &mut book,
            Txn::new()
                .debit(Account::Cash, 10_000, "")
                .credit(Account::Revenue, 10_000, ""),
        );
        let (later, newest) = classify_cash(&book, Some(9), 4);
        assert_eq!(later, [10_000, 0, 0, 0]);
        assert_eq!(newest, 10);
        // Without a marker, other weeks are left out.
        assert_eq!(classify_cash(&book, None, 2).0, [0; 4]);
    }

    #[test]
    fn quarters_fold_into_years_keeping_the_first_opening_and_last_close() {
        use super::{CashFlow, DivisionPeriod, PeriodRecord, years};
        let quarter =
            |first: u32, revenue: i64, opening: i64, flow: i64, plants: u32| PeriodRecord {
                first_week: first,
                weeks: 13,
                revenue,
                net_income: revenue / 10,
                cash: opening + flow,
                equity: 1_000 + first as i64,
                cash_flow: CashFlow {
                    opening,
                    operating: flow,
                    closing: opening + flow,
                    ..CashFlow::default()
                },
                divisions: vec![DivisionPeriod {
                    region: 1,
                    industry: 2,
                    revenue,
                    plants,
                    ..DivisionPeriod::default()
                }],
                ..PeriodRecord::default()
            };
        let quarters = vec![
            quarter(1, 100, 500, 10, 1),
            quarter(14, 200, 510, -30, 1),
            quarter(27, 300, 480, 5, 2),
            quarter(40, 400, 485, 15, 2),
            quarter(53, 50, 500, 1, 3),
        ];
        let out = years(&quarters);
        assert_eq!(out.len(), 2);
        let first = &out[0];
        assert_eq!((first.first_week, first.weeks), (1, 52));
        assert_eq!(first.revenue, 1_000);
        assert_eq!(first.net_income, 10 + 20 + 30 + 40);
        assert_eq!(first.cash_flow.opening, 500);
        assert_eq!(first.cash_flow.operating, 10 - 30 + 5 + 15);
        assert_eq!(first.cash_flow.closing, 500);
        assert_eq!(
            first.cash_flow.opening + first.cash_flow.net(),
            first.cash_flow.closing
        );
        assert_eq!(first.equity, 1_040, "balances are the year's close");
        assert_eq!(first.divisions.len(), 1);
        assert_eq!(first.divisions[0].revenue, 1_000);
        assert_eq!(first.divisions[0].plants, 2, "plants are as at the close");
        assert_eq!(
            (out[1].first_week, out[1].weeks, out[1].revenue),
            (53, 13, 50)
        );
    }

    #[test]
    fn the_archive_reconciles_weekly_cash_and_folds_quarters() {
        use super::{ARCHIVE_WEEKS, quarter_index};
        let dataset = cw_data::Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 43, ScenarioKind::Guided, ScenarioMode::Sandbox);
        for _ in 0..40 {
            world.pending_pause = None;
            crate::tick::advance(&mut world, &dataset);
        }
        assert_eq!(
            world.archive.prices.len(),
            world.price_history.len(),
            "the archive keeps every close the year holds"
        );
        assert!(!world.archive.weeks.is_empty());
        for (handle, weeks) in &world.archive.weeks {
            assert!(weeks.len() <= ARCHIVE_WEEKS);
            for w in weeks {
                let cf = w.cash_flow;
                assert_eq!(
                    cf.opening + cf.operating + cf.investing + cf.financing + cf.tax,
                    cf.closing
                );
                assert_eq!(cf.closing, w.cash);
            }
            for pair in weeks.windows(2) {
                if pair[1].first_week == pair[0].first_week + 1 {
                    assert_eq!(
                        pair[1].cash_flow.opening, pair[0].cash_flow.closing,
                        "weeks chain"
                    );
                }
            }
            let book = &world.firms.get(*handle).unwrap().book;
            let last = weeks.last().unwrap();
            assert_eq!(last.cash, book.cash() + book.restricted_cash());
            assert_eq!(last.equity, book.total_equity());
            // Quarters hold exactly the sum of their weeks.
            let quarters = &world.archive.quarters[handle];
            for q in quarters {
                let inside: Vec<_> = weeks
                    .iter()
                    .filter(|w| quarter_index(w.first_week) == quarter_index(q.first_week))
                    .collect();
                assert_eq!(q.weeks as usize, inside.len());
                assert!(q.weeks <= 13);
                assert_eq!(q.revenue, inside.iter().map(|w| w.revenue).sum::<i64>());
                assert_eq!(
                    q.net_income,
                    inside.iter().map(|w| w.net_income).sum::<i64>()
                );
                assert_eq!(
                    q.cash_flow.operating,
                    inside.iter().map(|w| w.cash_flow.operating).sum::<i64>()
                );
                assert_eq!(q.cash_flow.opening, inside[0].cash_flow.opening);
                assert_eq!(
                    q.cash_flow.closing,
                    inside.last().unwrap().cash_flow.closing
                );
            }
            // An independent oracle: the four-week history sums the same weeks.
            let periods = &world.firm_history[handle];
            let from = periods.first().unwrap().first_week;
            let four: i64 = periods.iter().map(|p| p.net_income).sum();
            let archived: i64 = weeks
                .iter()
                .filter(|w| w.first_week >= from)
                .map(|w| w.net_income)
                .sum();
            assert_eq!(four, archived);
        }
        let divisions: usize = world
            .archive
            .quarters
            .values()
            .filter_map(|q| q.last())
            .map(|q| q.divisions.len())
            .sum();
        assert!(divisions > 0, "quarters carry division results");
    }
}
