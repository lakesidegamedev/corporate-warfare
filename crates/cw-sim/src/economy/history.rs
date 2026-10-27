//! Long results history for the stats screens: each firm's week closes folded into four-week
//! periods with closing balances. Nothing in the rules reads it.

use crate::state::{HISTORY_PERIODS, PERIOD_WEEKS, PeriodClose, World};

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
}
