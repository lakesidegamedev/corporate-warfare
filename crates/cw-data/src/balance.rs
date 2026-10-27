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

/// How a good is kept. Each class has its own finite capacity in a region: dry warehouses,
/// tank farms for liquids and gas, silos for grain and bulk, cold storage for perishables.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageClass {
    #[default]
    Dry,
    Tank,
    Silo,
    Cold,
}

impl StorageClass {
    pub const ALL: [StorageClass; 4] = [
        StorageClass::Dry,
        StorageClass::Tank,
        StorageClass::Silo,
        StorageClass::Cold,
    ];

    pub const fn code(self) -> u8 {
        match self {
            StorageClass::Dry => 0,
            StorageClass::Tank => 1,
            StorageClass::Silo => 2,
            StorageClass::Cold => 3,
        }
    }

    pub const fn key(self) -> &'static str {
        match self {
            StorageClass::Dry => "dry",
            StorageClass::Tank => "tank",
            StorageClass::Silo => "silo",
            StorageClass::Cold => "cold",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            StorageClass::Dry => "Warehouse",
            StorageClass::Tank => "Tank farm",
            StorageClass::Silo => "Silo",
            StorageClass::Cold => "Cold store",
        }
    }

    /// Weekly fee in cents per lot at the region's outside storage, who takes the overflow of
    /// every firm so goods never vanish for lack of space. Always dearer than owning a store.
    pub const fn outside_fee_cents(self) -> i64 {
        match self {
            StorageClass::Dry => 80,
            StorageClass::Tank => 120,
            StorageClass::Silo => 60,
            StorageClass::Cold => 200,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Product {
    pub id: String,
    pub name: String,
    pub reference_price_cents: i64,
    pub kind: ProductKind,
    /// Share of unsold stock that spoils each week, in basis points. Zero for durable goods.
    #[serde(default)]
    pub spoilage_bps_weekly: u32,
    /// Which kind of store holds the good (balance schema 3). Services are never stored.
    #[serde(default)]
    pub storage_class: StorageClass,
    /// Weekly cost of holding stock beyond a plant's working yard, in basis points of the
    /// reference price per lot: capital, insurance and handling.
    #[serde(default)]
    pub holding_bps_weekly: u32,
}

/// A further product a plant makes alongside its primary output, in a fixed yield: the lots a
/// single block makes each week at full capacity. It scales with the plant's run rate exactly as
/// the primary output does, so a refinery never makes gasoline without its diesel.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ByProduct {
    pub product: String,
    pub weekly_lots: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Recipe {
    pub industry: String,
    pub id: String,
    /// Operating strategy shown to the player, after A House Divided's sector strategies.
    #[serde(default)]
    pub name: String,
    pub output: String,
    pub weekly_output_lots: i64,
    /// Further products made in a fixed yield alongside `output` (balance schema 2).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub by_products: Vec<ByProduct>,
    /// Set on a storage facility: the class of store it adds. Its `weekly_output_lots` is then
    /// the lots of space one block holds (balance schema 3) and it makes no goods.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<StorageClass>,
    pub weekly_inputs_lots: BTreeMap<String, i64>,
    pub workers_low_mid_high: [i64; 3],
    pub capex_cents: i64,
    pub build_weeks: u32,
    pub life_years: u32,
}

impl Recipe {
    /// Lots of space one block of a storage facility holds; zero for a producing plant.
    pub fn storage_lots(&self) -> i64 {
        if self.storage_class.is_some() {
            self.weekly_output_lots
        } else {
            0
        }
    }

    /// Every product the plant makes with its weekly lots per block at full capacity, the
    /// primary output first.
    pub fn outputs(&self) -> impl Iterator<Item = (&str, i64)> {
        std::iter::once((self.output.as_str(), self.weekly_output_lots)).chain(
            self.by_products
                .iter()
                .map(|by| (by.product.as_str(), by.weekly_lots)),
        )
    }

    /// Weekly lots per block of one product at full capacity, zero when the plant does not make it.
    pub fn lots_of(&self, product: &str) -> i64 {
        self.outputs()
            .filter(|(id, _)| *id == product)
            .map(|(_, lots)| lots)
            .sum()
    }

    /// Whether the plant makes the product, as its primary output or a by-product.
    pub fn makes(&self, product: &str) -> bool {
        self.output == product || self.by_products.iter().any(|by| by.product == product)
    }

    /// Weekly value per block at full capacity of all outputs, priced by `price`.
    pub fn output_value(&self, price: impl Fn(&str) -> i64) -> i64 {
        self.outputs().map(|(id, lots)| lots * price(id)).sum()
    }

    /// Extraction from a finite deposit: every extraction strategy except renewable timber.
    pub fn depletes(&self) -> bool {
        self.industry == "extraction" && self.output != "timber"
    }

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

impl Balance {
    /// Oldest balance schema the loader still reads. Schema 1 had no by-products and no spoilage;
    /// both default to none, so a schema 1 file migrates unchanged in meaning. Schema 3 adds storage
    /// classes and holding costs, which default to a dry store with no holding cost.
    pub const MIN_SCHEMA: u32 = 1;
    pub const SCHEMA: u32 = 3;

    /// Bring a parsed file up to the current schema, or say why it cannot be read.
    pub fn migrate(mut self) -> Result<Balance, String> {
        if self.schema < Self::MIN_SCHEMA || self.schema > Self::SCHEMA {
            return Err(format!(
                "balance schema {} is not supported (reads {} to {})",
                self.schema,
                Self::MIN_SCHEMA,
                Self::SCHEMA
            ));
        }
        // Schema 1 to 2 added optional fields only; their defaults are the schema 1 meaning.
        self.schema = Self::SCHEMA;
        Ok(self)
    }
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
        // One plant: 129 * $100 = $12,900; fuel 4 * $300 + power 2 * $1,200 = $3,600;
        // crew 8 * $400 + 2 * $800 + 1 * $1,400 = $6,200 (trimmed in 1.3.1 so the plant clears
        // about a fifth of revenue after depreciation; tools/content/rebalance_margins.py).
        assert_eq!(revenue, 1_290_000);
        assert_eq!(inputs, 360_000);
        assert_eq!(payroll, 620_000);
        let maintenance = farm.weekly_maintenance(balance.maintenance_annual_bps);
        let depreciation = farm.weekly_depreciation();
        // $400,000 * 2 percent / 52 weeks = $153.846, rounded to $153.85.
        assert_eq!(maintenance, 15_385);
        // $400,000 / (20 * 52) = $384.615, rounded to $384.62.
        assert_eq!(depreciation, 38_462);
        // Land is $200 a week per plant.
        assert_eq!(balance.fixture_land_rent_cents_weekly, 20_000);
        let ebitda =
            revenue - inputs - payroll - maintenance - balance.fixture_land_rent_cents_weekly;
        let ebit = ebitda - depreciation;
        assert_eq!(ebitda, 274_615);
        assert_eq!(ebit, 236_153);
        // $2,361.53 of $12,900 is an 18.3 percent margin after depreciation.
        assert_eq!(ebit * 1_000 / revenue, 183);
    }
}
