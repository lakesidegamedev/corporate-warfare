//! Fiscal receipts and the trailing weekly budget.
//!
//! Taxable labour income pays twenty percent to the government. Corporate tax is accrued
//! weekly and paid quarterly. The three spending budgets are eighty percent of the trailing
//! thirteen-week mean of actual receipts, split fifty/forty/ten between procurement,
//! healthcare and training. Unspent budget is not carried forward.
//!
//! The treasury answers to its own cash. It aims to hold a reserve of thirteen weeks of receipts
//! (what it opens with). Above the reserve it spends more, half of the excess in proportion up to
//! double, and opens public works faster and pays the jobless more; below it the budgets are cut
//! in proportion, down to half, and no new works open. So it runs surpluses and deficits with the
//! cycle and settles near where it spends what it takes. The labour tax follows too: a treasury
//! far over its reserve hands some of the surplus back by taxing wages less (down to eight
//! percent of wages back to eight), and one under it taxes a little more (up to twenty-two).

use crate::state::World;

pub const LABOUR_TAX_BPS: i64 = 2_000;
/// Within a tenth of the reserve either way nothing moves: the plain rate and plain budgets.
const DEAD_BAND_LOW: i64 = 9_000;
const DEAD_BAND_HIGH: i64 = 11_000;
const LABOUR_TAX_MIN_BPS: i64 = 800;
const LABOUR_TAX_MAX_BPS: i64 = 2_200;
/// Weeks of receipts the treasury aims to keep in hand.
pub const RESERVE_WEEKS: i64 = 13;
/// Spending at the reserve is the plain eighty percent; this is the least and most it moves to.
const FACTOR_MIN_BPS: i64 = 5_000;
const FACTOR_MAX_BPS: i64 = 13_000;

/// Trailing mean weekly receipts.
pub fn mean_receipts(world: &World) -> i64 {
    let history = &world.government.tax_receipts_history;
    if history.is_empty() {
        0
    } else {
        history.iter().sum::<i64>() / history.len() as i64
    }
}

/// The treasury's cash against its reserve, in basis points (10,000 is exactly the reserve).
pub fn stance_bps(world: &World) -> i64 {
    let reserve = mean_receipts(world).saturating_mul(RESERVE_WEEKS);
    if reserve <= 0 {
        return 10_000;
    }
    (world.government.book.cash().max(0) as i128 * 10_000 / reserve as i128).min(1_000_000) as i64
}

/// The labour tax rate in basis points for a treasury at `stance_bps` of its reserve: twenty
/// percent at the reserve, falling a point for every eighth of a reserve of surplus over it down
/// to eight, and rising a point for every quarter short of it up to twenty-two.
pub fn labour_tax_bps(stance_bps: i64) -> i64 {
    if (DEAD_BAND_LOW..=DEAD_BAND_HIGH).contains(&stance_bps) {
        return LABOUR_TAX_BPS;
    }
    if stance_bps >= 10_000 {
        (LABOUR_TAX_BPS - (stance_bps - 10_000) / 12).max(LABOUR_TAX_MIN_BPS)
    } else {
        (LABOUR_TAX_BPS + (10_000 - stance_bps) / 25).min(LABOUR_TAX_MAX_BPS)
    }
}

/// How hard the budgets run, in basis points of the plain eighty percent: cut in proportion when
/// the treasury is under its reserve, and raised by half the excess when it is over.
pub fn spending_factor_bps(stance_bps: i64) -> i64 {
    if (DEAD_BAND_LOW..=DEAD_BAND_HIGH).contains(&stance_bps) {
        return 10_000;
    }
    if stance_bps < 10_000 {
        stance_bps.max(FACTOR_MIN_BPS)
    } else {
        (10_000 + (stance_bps - 10_000) / 2).min(FACTOR_MAX_BPS)
    }
}

/// Add one actual tax receipt to the current week.
pub fn record(world: &mut World, amount_cents: i64) {
    if amount_cents <= 0 {
        return;
    }
    world.government.week_tax_receipts_cents = world
        .government
        .week_tax_receipts_cents
        .saturating_add(amount_cents);
}

/// Close the fiscal week, update the trailing window and reset the three budgets.
pub fn close_week(world: &mut World) {
    let receipts = world.government.week_tax_receipts_cents.max(0);
    world.government.last_tax_receipts_cents = receipts;
    world.government.tax_receipts_history.push(receipts);
    if world.government.tax_receipts_history.len() > 13 {
        world.government.tax_receipts_history.remove(0);
    }
    // Budgets follow what the plain twenty percent rate would take, so handing surplus back in
    // lower taxes does not also shrink the state's spending.
    let stance = stance_bps(world);
    let mean = mean_receipts(world) * LABOUR_TAX_BPS / labour_tax_bps(stance);
    let factor = spending_factor_bps(stance);
    let spendable = mean * 80 / 100 * factor / 10_000;
    world.government.procurement_budget_cents = spendable * 50 / 100;
    world.government.healthcare_budget_cents = spendable * 40 / 100;
    world.government.training_budget_cents = spendable * 10 / 100;
    world.government.week_tax_receipts_cents = 0;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ScenarioKind, ScenarioMode};

    fn world() -> World {
        let dataset = cw_data::Dataset::load().unwrap();
        crate::economy::seed::build(&dataset, 91, ScenarioKind::Guided, ScenarioMode::Sandbox)
    }

    #[test]
    fn budgets_are_eighty_percent_of_the_trailing_thirteen_week_mean() {
        let mut world = world();
        // Receipts of a thirteenth of the treasury keep it at its reserve, so the plain eighty
        // percent applies. The window is bounded.
        let receipt = world.government.book.cash() / 13;
        for _ in 0..14 {
            super::record(&mut world, receipt);
            close_week(&mut world);
        }
        assert_eq!(world.government.tax_receipts_history.len(), 13);
        let mean: i64 = world.government.tax_receipts_history.iter().sum::<i64>() / 13;
        assert_eq!(
            world.government.procurement_budget_cents,
            mean * 80 / 100 / 2
        );
        assert_eq!(
            world.government.healthcare_budget_cents,
            mean * 80 / 100 * 40 / 100
        );
        assert_eq!(
            world.government.training_budget_cents,
            mean * 80 / 100 * 10 / 100
        );
        assert_eq!(world.government.week_tax_receipts_cents, 0);
    }

    #[test]
    fn receipts_outside_the_window_are_forgotten() {
        let mut world = world();
        let baseline = world.government.last_tax_receipts_cents;
        for _ in 0..14 {
            close_week(&mut world);
        }
        assert_eq!(world.government.tax_receipts_history.len(), 13);
        assert!(
            world
                .government
                .tax_receipts_history
                .iter()
                .all(|v| *v == 0)
        );
        assert_eq!(world.government.procurement_budget_cents, 0);
        assert!(baseline > 0);
    }
}
