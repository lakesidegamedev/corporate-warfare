//! Authored operating calibration: products, facility recipes and baseline fixture wages.
//!
//! These are game constants, not measurements. One lot is a standardised game batch; the
//! display metadata must state what a lot means for that product.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductKind {
    Goods,
    LocalService,
    ProjectService,
    TransportService,
    ConsumerGoods,
    Subscription,
    Service,
    LeaseService,
    ContractGoods,
    FinancialContract,
}

impl ProductKind {
    pub const fn code(self) -> u8 {
        match self {
            ProductKind::Goods => 0,
            ProductKind::LocalService => 1,
            ProductKind::ProjectService => 2,
            ProductKind::TransportService => 3,
            ProductKind::ConsumerGoods => 4,
            ProductKind::Subscription => 5,
            ProductKind::Service => 6,
            ProductKind::LeaseService => 7,
            ProductKind::ContractGoods => 8,
            ProductKind::FinancialContract => 9,
        }
    }

    /// Services expire: they cannot be stockpiled into a later week.
    pub const fn is_service(self) -> bool {
        matches!(
            self,
            ProductKind::LocalService
                | ProductKind::ProjectService
                | ProductKind::TransportService
                | ProductKind::Service
                | ProductKind::Subscription
                | ProductKind::LeaseService
        )
    }

    /// Household final demand flows through retail delivery, never a second direct purchase.
    pub const fn is_consumer_goods(self) -> bool {
        matches!(self, ProductKind::ConsumerGoods)
    }

    /// Financial contracts are claims, not physical stock, and never enter goods clearing.
    pub const fn is_financial(self) -> bool {
        matches!(self, ProductKind::FinancialContract)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub reference_price_cents: i64,
    pub kind: ProductKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recipe {
    pub industry: String,
    pub id: String,
    pub output: String,
    pub weekly_output_lots: i64,
    pub weekly_inputs_lots: BTreeMap<String, i64>,
    pub workers_low_mid_high: [i64; 3],
    pub capex_cents: i64,
    pub build_weeks: u32,
    pub life_years: u32,
}

impl Recipe {
    /// Full-capacity weekly payroll at baseline fixture wages.
    pub fn baseline_weekly_payroll(&self, wages: [i64; 3]) -> i64 {
        self.workers_low_mid_high
            .iter()
            .zip(wages.iter())
            .map(|(workers, wage)| workers * wage)
            .sum()
    }

    /// Weekly straight-line depreciation over the declared life.
    pub fn weekly_depreciation(&self) -> i64 {
        let weeks = self.life_years as i64 * 52;
        if weeks <= 0 {
            return 0;
        }
        cw_core::money::round_even_div(self.capex_cents as i128, weeks as i128) as i64
    }

    /// Weekly maintenance at the declared annual basis points.
    pub fn weekly_maintenance(&self, annual_bps: i32) -> i64 {
        cw_core::money::round_even_div(self.capex_cents as i128 * annual_bps as i128, 10_000 * 52)
            as i64
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Balance {
    pub schema: u32,
    #[serde(default)]
    pub classification: String,
    pub baseline_weekly_wage_cents: [i64; 3],
    pub products: Vec<Product>,
    pub recipes: Vec<Recipe>,
    pub financial_businesses: Vec<String>,
    pub maintenance_annual_bps: i32,
    pub fixture_land_rent_cents_weekly: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn farm_worked_example_matches_the_contract() {
        let balance: Balance = serde_json::from_str(crate::BALANCE_JSON).expect("balance parses");
        let farm = balance.recipes.iter().find(|r| r.id == "farm").unwrap();
        let prices: BTreeMap<&str, i64> = balance
            .products
            .iter()
            .map(|p| (p.id.as_str(), p.reference_price_cents))
            .collect();
        let revenue = farm.weekly_output_lots * prices[farm.output.as_str()];
        let inputs: i64 = farm
            .weekly_inputs_lots
            .iter()
            .map(|(id, qty)| qty * prices[id.as_str()])
            .sum();
        let payroll = farm.baseline_weekly_payroll(balance.baseline_weekly_wage_cents);
        // 600 * $100 = $60,000; fuel 20 * $300 = $6,000; power 10 * $1,000 = $10,000.
        assert_eq!(revenue, 6_000_000);
        assert_eq!(inputs, 1_600_000);
        assert_eq!(payroll, 3_480_000);
        let maintenance = farm.weekly_maintenance(balance.maintenance_annual_bps);
        let depreciation = farm.weekly_depreciation();
        // $2,000,000 * 2 percent / 52 weeks = $769.23, carried exactly in cents.
        assert_eq!(maintenance, 76_923);
        // $2,000,000 / (20 * 52) = $1,923.08.
        assert_eq!(depreciation, 192_308);
        let ebitda =
            revenue - inputs - payroll - maintenance - balance.fixture_land_rent_cents_weekly;
        let ebit = ebitda - depreciation;
        assert_eq!(ebitda, 743_077);
        assert_eq!(ebit, 550_769);
    }
}
