//! Fiscal receipts and the trailing weekly budget.
//!
//! Taxable labour income pays twenty percent to the government. Corporate tax is accrued
//! weekly and paid quarterly. The three spending budgets are eighty percent of the trailing
//! thirteen-week mean of actual receipts, split fifty/forty/ten between procurement,
//! healthcare and training. Unspent budget is not carried forward.

use crate::state::World;

pub const LABOUR_TAX_BPS: i64 = 2_000;

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
    let weeks = world.government.tax_receipts_history.len() as i64;
    let mean = if weeks > 0 {
        world.government.tax_receipts_history.iter().sum::<i64>() / weeks
    } else {
        0
    };
    let spendable = mean * 80 / 100;
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
        // A single high receipt week shifts the mean; the window is bounded.
        for _ in 0..14 {
            super::record(&mut world, 130_000);
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
