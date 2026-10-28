//! Financial statements and long price series for the deeper financial views.
//!
//! Every figure comes from the engine's history archive. Ratios are computed here in basis
//! points with checked arithmetic, so no client derives a financial identity on its own. A ratio
//! whose denominator is zero or negative is absent rather than a misleading number.

use cw_core::ids::Handle;
use cw_sim::economy::history::{PeriodRecord, years};

use crate::SessionHandle;

/// One division's estimated results over a statement period.
#[derive(uniffi::Record)]
pub struct DivisionPeriodDto {
    pub region_name: String,
    pub industry: String,
    pub revenue_cents: i64,
    pub costs_cents: i64,
    pub profit_cents: i64,
    pub plants: u32,
    pub blocks: u32,
    pub share_bps: i64,
}

/// One period of a firm's statements: income statement, balance sheet at the close, cash flow
/// and ratios. `label` is a game date such as `2001 Q3`, `2001` or `2001 W05`.
#[derive(uniffi::Record)]
pub struct StatementDto {
    pub first_week: u32,
    pub weeks: u32,
    pub label: String,
    pub revenue_cents: i64,
    pub cost_of_goods_cents: i64,
    pub gross_profit_cents: i64,
    pub wages_cents: i64,
    pub operating_expense_cents: i64,
    pub depreciation_cents: i64,
    pub other_cents: i64,
    pub ebit_cents: i64,
    pub interest_expense_cents: i64,
    pub interest_income_cents: i64,
    pub tax_cents: i64,
    pub net_income_cents: i64,
    pub cash_cents: i64,
    pub inventory_cents: i64,
    pub current_assets_cents: i64,
    pub fixed_assets_cents: i64,
    pub assets_cents: i64,
    pub current_liabilities_cents: i64,
    pub debt_cents: i64,
    pub liabilities_cents: i64,
    pub equity_cents: i64,
    pub share_price_cents: i64,
    pub shares_issued: i64,
    pub cash_opening_cents: i64,
    pub cash_operating_cents: i64,
    pub cash_investing_cents: i64,
    pub cash_financing_cents: i64,
    pub cash_tax_cents: i64,
    pub cash_closing_cents: i64,
    pub gross_margin_bps: Option<i64>,
    pub net_margin_bps: Option<i64>,
    /// Return on equity and on assets, annualised from the period.
    pub roe_bps: Option<i64>,
    pub roa_bps: Option<i64>,
    pub debt_to_equity_bps: Option<i64>,
    pub current_ratio_bps: Option<i64>,
    pub interest_cover_bps: Option<i64>,
    /// Revenue growth over the previous period of the same length.
    pub revenue_growth_bps: Option<i64>,
    pub divisions: Vec<DivisionPeriodDto>,
}

/// One weekly close of a price series.
#[derive(uniffi::Record)]
pub struct PricePointDto {
    pub week: u32,
    pub cents: i64,
}

/// `numerator / denominator` in basis points, absent when the denominator is not positive.
pub fn ratio_bps(numerator: i64, denominator: i64) -> Option<i64> {
    if denominator <= 0 {
        return None;
    }
    let value = (numerator as i128).checked_mul(10_000)? / denominator as i128;
    i64::try_from(value).ok()
}

/// A period's ratio annualised to 52 weeks.
fn annualised_bps(numerator: i64, denominator: i64, weeks: u32) -> Option<i64> {
    if weeks == 0 {
        return None;
    }
    let yearly = (numerator as i128).checked_mul(52)? / weeks as i128;
    ratio_bps(i64::try_from(yearly).ok()?, denominator)
}

/// The game date of a period's first week at the given granularity.
pub fn period_label(first_week: u32, start_year: i32, period: &str) -> String {
    let tick = cw_core::calendar::Tick(first_week.max(1));
    let year = tick.year_from(start_year);
    match period {
        "year" => format!("{year}"),
        "quarter" => format!("{year} Q{}", tick.quarter()),
        _ => tick.label_from(start_year),
    }
}

fn statement(
    record: &PeriodRecord,
    previous: Option<&PeriodRecord>,
    label: String,
    names: &dyn Fn(u8) -> String,
) -> StatementDto {
    let ebit = record.ebit();
    let growth = previous
        .filter(|p| p.weeks == record.weeks && p.revenue > 0)
        .and_then(|p| ratio_bps(record.revenue.saturating_sub(p.revenue), p.revenue));
    StatementDto {
        first_week: record.first_week,
        weeks: record.weeks,
        label,
        revenue_cents: record.revenue,
        cost_of_goods_cents: record.cost_of_goods,
        gross_profit_cents: record.gross_profit(),
        wages_cents: record.wages,
        operating_expense_cents: record.operating_expense,
        depreciation_cents: record.depreciation,
        other_cents: record.other,
        ebit_cents: ebit,
        interest_expense_cents: record.interest_expense,
        interest_income_cents: record.interest_income,
        tax_cents: record.tax,
        net_income_cents: record.net_income,
        cash_cents: record.cash,
        inventory_cents: record.inventory,
        current_assets_cents: record.current_assets,
        fixed_assets_cents: record.fixed_assets,
        assets_cents: record.assets,
        current_liabilities_cents: record.current_liabilities,
        debt_cents: record.debt,
        liabilities_cents: record.liabilities,
        equity_cents: record.equity,
        share_price_cents: record.share_price_cents,
        shares_issued: record.shares_issued,
        cash_opening_cents: record.cash_flow.opening,
        cash_operating_cents: record.cash_flow.operating,
        cash_investing_cents: record.cash_flow.investing,
        cash_financing_cents: record.cash_flow.financing,
        cash_tax_cents: record.cash_flow.tax,
        cash_closing_cents: record.cash_flow.closing,
        gross_margin_bps: ratio_bps(record.gross_profit(), record.revenue),
        net_margin_bps: ratio_bps(record.net_income, record.revenue),
        roe_bps: annualised_bps(record.net_income, record.equity, record.weeks),
        roa_bps: annualised_bps(record.net_income, record.assets, record.weeks),
        debt_to_equity_bps: ratio_bps(record.debt, record.equity),
        current_ratio_bps: ratio_bps(record.current_assets, record.current_liabilities),
        interest_cover_bps: ratio_bps(ebit, record.interest_expense),
        revenue_growth_bps: growth,
        divisions: record
            .divisions
            .iter()
            .map(|d| DivisionPeriodDto {
                region_name: names(d.region),
                industry: cw_sim::state::INDUSTRY_NAMES
                    .get(d.industry as usize)
                    .copied()
                    .unwrap_or("other")
                    .to_owned(),
                revenue_cents: d.revenue,
                costs_cents: d.costs,
                profit_cents: d.profit,
                plants: d.plants,
                blocks: d.blocks,
                share_bps: d.share_bps,
            })
            .collect(),
    }
}

/// Build the statements of one series, oldest first, with growth against the period before.
pub fn statements_of(
    records: &[PeriodRecord],
    start_year: i32,
    period: &str,
    names: &dyn Fn(u8) -> String,
) -> Vec<StatementDto> {
    records
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let previous = i.checked_sub(1).and_then(|p| records.get(p));
            statement(
                r,
                previous,
                period_label(r.first_week, start_year, period),
                names,
            )
        })
        .collect()
}

#[uniffi::export]
impl SessionHandle {
    /// A firm's statements, oldest first. `period` is `week` (one year), `quarter` (the whole
    /// career) or `year`. Your companies, inspected companies and companies on the board only; a save
    /// from before the archive has none until weeks are played.
    pub fn statements(&self, firm: u64, period: String) -> Vec<StatementDto> {
        let session = self.lock();
        let world = &session.world;
        let handle = Handle(firm);
        let Some(entity) = world.firms.get(handle) else {
            return Vec::new();
        };
        let listed = entity.on_board();
        if !listed && !session.can_inspect(firm) {
            return Vec::new();
        }
        let names = |region: u8| {
            world
                .regions
                .get(region as usize)
                .map(|r| r.name.clone())
                .unwrap_or_default()
        };
        let archive = &world.archive;
        match period.as_str() {
            "week" => archive
                .weeks
                .get(&handle)
                .map(|w| statements_of(w, world.start_year, "week", &names))
                .unwrap_or_default(),
            "year" => archive
                .quarters
                .get(&handle)
                .map(|q| statements_of(&years(q), world.start_year, "year", &names))
                .unwrap_or_default(),
            _ => archive
                .quarters
                .get(&handle)
                .map(|q| statements_of(q, world.start_year, "quarter", &names))
                .unwrap_or_default(),
        }
    }

    /// Weekly closing share prices of a firm, oldest first, up to ten years.
    pub fn share_price_history(&self, firm: u64) -> Vec<PricePointDto> {
        let session = self.lock();
        session
            .world
            .archive
            .prices
            .iter()
            .filter_map(|p| {
                p.shares.get(&Handle(firm)).map(|cents| PricePointDto {
                    week: p.week,
                    cents: *cents,
                })
            })
            .collect()
    }

    /// Weekly closing corridor prices of a good, oldest first, up to ten years.
    pub fn good_price_history(&self, product: u32) -> Vec<PricePointDto> {
        let session = self.lock();
        let code = product as u16;
        session
            .world
            .archive
            .prices
            .iter()
            .filter_map(|p| {
                p.goods.get(&code).map(|cents| PricePointDto {
                    week: p.week,
                    cents: *cents,
                })
            })
            .collect()
    }

    /// The corridor share index, oldest first, up to ten years: the average weekly change of
    /// every listed share chained from 1,000.00 (as cents, 100,000) at the first close. Shares
    /// that list or leave mid-way only count for the weeks they trade both ends.
    pub fn corridor_index_history(&self) -> Vec<PricePointDto> {
        let session = self.lock();
        chained_index(&session.world.archive.prices)
    }
}

/// Chain an equal-weight share index from 100,000 through each week's average change.
pub fn chained_index(prices: &[cw_sim::state::PricePoint]) -> Vec<PricePointDto> {
    let mut out = Vec::with_capacity(prices.len());
    let mut level: i128 = 100_000;
    for (i, point) in prices.iter().enumerate() {
        if let Some(before) = i.checked_sub(1).and_then(|p| prices.get(p)) {
            let changes: Vec<i128> = point
                .shares
                .iter()
                .filter_map(|(firm, now)| {
                    let was = *before.shares.get(firm)?;
                    (was > 0).then(|| (*now as i128) * 1_000_000 / was as i128)
                })
                .collect();
            if !changes.is_empty() {
                let mean = changes.iter().sum::<i128>() / changes.len() as i128;
                level = level * mean / 1_000_000;
            }
        }
        out.push(PricePointDto {
            week: point.week,
            cents: i64::try_from(level).unwrap_or(i64::MAX),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use cw_sim::economy::history::CashFlow;
    use cw_sim::state::PricePoint;

    #[test]
    fn ratios_match_hand_arithmetic() {
        let record = PeriodRecord {
            first_week: 14,
            weeks: 13,
            revenue: 400_000,
            cost_of_goods: 300_000,
            wages: 40_000,
            operating_expense: 20_000,
            depreciation: 10_000,
            interest_expense: 5_000,
            tax: 6_000,
            net_income: 19_000,
            assets: 2_000_000,
            equity: 1_000_000,
            debt: 500_000,
            current_assets: 300_000,
            current_liabilities: 200_000,
            cash_flow: CashFlow::default(),
            ..PeriodRecord::default()
        };
        let previous = PeriodRecord {
            weeks: 13,
            revenue: 320_000,
            ..PeriodRecord::default()
        };
        let s = statement(&record, Some(&previous), "2000 Q2".into(), &|_| {
            String::new()
        });
        // Gross 100,000 of 400,000 is 25 percent; net 19,000 is 4.75 percent.
        assert_eq!(s.gross_profit_cents, 100_000);
        assert_eq!(s.gross_margin_bps, Some(2_500));
        assert_eq!(s.net_margin_bps, Some(475));
        // EBIT 30,000 covers 5,000 of interest six times.
        assert_eq!(s.ebit_cents, 30_000);
        assert_eq!(s.interest_cover_bps, Some(60_000));
        // 19,000 a quarter is 76,000 a year: 7.6 percent of equity, 3.8 percent of assets.
        assert_eq!(s.roe_bps, Some(760));
        assert_eq!(s.roa_bps, Some(380));
        assert_eq!(s.debt_to_equity_bps, Some(5_000));
        assert_eq!(s.current_ratio_bps, Some(15_000));
        // 400,000 over 320,000 is 25 percent growth.
        assert_eq!(s.revenue_growth_bps, Some(2_500));
        assert_eq!(s.label, "2000 Q2");
        // No equity, no return on it; a period of another length is not compared.
        let broke = PeriodRecord {
            equity: -5,
            weeks: 4,
            ..record
        };
        let s = statement(&broke, Some(&previous), String::new(), &|_| String::new());
        assert_eq!(s.roe_bps, None);
        assert_eq!(s.debt_to_equity_bps, None);
        assert_eq!(s.revenue_growth_bps, None);
    }

    #[test]
    fn a_session_reports_quarters_that_sum_their_weeks() {
        let session = SessionHandle::new(181, true, true).unwrap();
        session.advance(30);
        let me = session.overview().primary_firm.unwrap();
        let weeks = session.statements(me, "week".into());
        let quarters = session.statements(me, "quarter".into());
        let years = session.statements(me, "year".into());
        assert!(weeks.len() >= 14, "{} weeks", weeks.len());
        assert!(quarters.len() >= 2);
        assert_eq!(years.len(), 1);
        for q in &quarters {
            let inside: Vec<&StatementDto> = weeks
                .iter()
                .filter(|w| (w.first_week - 1) / 13 == (q.first_week - 1) / 13)
                .collect();
            let revenue: i64 = inside.iter().map(|w| w.revenue_cents).sum();
            assert_eq!(q.revenue_cents, revenue, "{}", q.label);
            let operating: i64 = inside.iter().map(|w| w.cash_operating_cents).sum();
            assert_eq!(q.cash_operating_cents, operating);
            assert_eq!(
                q.cash_opening_cents
                    + q.cash_operating_cents
                    + q.cash_investing_cents
                    + q.cash_financing_cents
                    + q.cash_tax_cents,
                q.cash_closing_cents
            );
        }
        let total: i64 = quarters.iter().map(|q| q.net_income_cents).sum();
        assert_eq!(years[0].net_income_cents, total);
        assert!(quarters.last().unwrap().label.contains(" Q"));
        assert!(!quarters.last().unwrap().divisions.is_empty());
        // Cash spent between weeks lands in the next week's flows, so the weeks chain.
        for pair in weeks.windows(2) {
            assert_eq!(pair[1].cash_opening_cents, pair[0].cash_closing_cents);
        }
        assert!(!session.corridor_index_history().is_empty());
    }

    /// Every company on the stock board shows its quarters, floated or not, before and after a
    /// save. Statements once required a completed float, which hid half the board.
    #[test]
    fn every_firm_on_the_board_has_statements() {
        let session = SessionHandle::start(crate::NewGameDto {
            seed: 7,
            scenario_id: "great-lakes-2000".to_owned(),
            guided: true,
            five_year: false,
            difficulty: "standard".to_owned(),
            economy: "historic".to_owned(),
            starting_capital_cents: None,
            price_path: "historical".to_owned(),
        })
        .unwrap();
        let mut guard = 0;
        while session.overview().tick < 30 && guard < 40 {
            session.dismiss_pause();
            session.advance(26);
            guard += 1;
        }
        // A saved and reopened career keeps the same history.
        let reopened = crate::import_save(session.export_save().unwrap()).unwrap();
        for session in [&session, &reopened] {
            let board = session.stock_board();
            assert!(board.len() > 10, "{} on the board", board.len());
            let missing: Vec<String> = board
                .iter()
                .filter(|s| session.statements(s.firm, "quarter".into()).len() < 3)
                .map(|s| s.name.clone())
                .collect();
            assert!(missing.is_empty(), "missing quarters: {missing:?}");
        }
    }

    #[test]
    fn labels_are_game_dates() {
        assert_eq!(period_label(1, 1981, "quarter"), "1981 Q1");
        assert_eq!(period_label(27, 1981, "quarter"), "1981 Q3");
        assert_eq!(period_label(53, 1981, "year"), "1982");
        assert_eq!(period_label(57, 1981, "week"), "1982 W05");
    }

    #[test]
    fn the_index_chains_the_average_change() {
        let point = |week: u32, a: i64, b: Option<i64>| {
            let mut p = PricePoint {
                week,
                ..PricePoint::default()
            };
            p.shares.insert(Handle(1), a);
            if let Some(b) = b {
                p.shares.insert(Handle(2), b);
            }
            p
        };
        // Week 2: +10 percent and -10 percent average to flat. Week 3: the second share left,
        // the first rose 20 percent. Week 4: it fell back 50 percent.
        let prices = vec![
            point(1, 100, Some(200)),
            point(2, 110, Some(180)),
            point(3, 132, None),
            point(4, 66, None),
        ];
        let index: Vec<i64> = chained_index(&prices).iter().map(|p| p.cents).collect();
        assert_eq!(index, vec![100_000, 100_000, 120_000, 60_000]);
    }
}
