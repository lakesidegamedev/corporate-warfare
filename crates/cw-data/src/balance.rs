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
    /// Set on a carrier: the freight mode it adds lane capacity in (balance schema 3). Its
    /// `weekly_output_lots` is then the lots a block can move each week.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_mode: Option<String>,
    /// A rail terminal or a port: it opens its region to the mode for every firm and adds a
    /// little capacity of its own.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub terminal: bool,
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
    /// How gateways and sellers respond to trade volume (2.0). Absent in older content files,
    /// which then get the defaults.
    #[serde(default)]
    pub markets: MarketRules,
    /// How building costs follow prices, when rivals build and close, how idle cash is paid out
    /// and how raids are announced (2.0). Absent in older content files, which then get the
    /// defaults (the rules before 2.0).
    #[serde(default)]
    pub capacity: CapacityRules,
}

/// Capacity as the contest: what a new plant costs today, when a company adds or closes one, and
/// what it does with cash it has no use for. Shared by every scenario and by the player and the
/// rivals alike; a scenario supplies prices and plants, never its own rules.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CapacityRules {
    /// Weight of the general price index in a plant's build price, in basis points. The weights
    /// of the index and of `build_inputs` add to 10,000 when build prices follow prices; all
    /// zero means a plant costs its catalogue price for ever (the rule before 2.0).
    pub build_price_level_bps: i64,
    /// Goods and services a plant is built from, each with its weight in the build price, in
    /// basis points. Each counts at last week's closing corridor price against its catalogue
    /// price, so a steel boom makes new mills dearer for everyone who orders one.
    pub build_inputs: Vec<(String, i64)>,
    /// The least and most the build price may be, in basis points of the catalogue price.
    pub build_price_min_bps: i64,
    pub build_price_max_bps: i64,
    /// Whether households going without a service or retail good count as room to build, and
    /// such products are valued at last week's closing price rather than the catalogue's.
    pub local_room: bool,
    /// Weeks of a block's output the opening must absorb, in tenths of a block, before a firm
    /// that waits for proof (a cautious temperament) builds. A keen builder needs less.
    pub opening_patient_tenths: i64,
    pub opening_keen_tenths: i64,
    /// Room, in tenths of a block, an opening outside a firm's own sectors must hold before a
    /// builder with idle cash enters it as a new business. Zero: firms keep to their sectors.
    pub opening_entry_tenths: i64,
    /// Plants a company may run before each further one must pay back faster: the payback a
    /// build must beat shortens by this many weeks for every plant already run. A smooth cost
    /// of size in place of a hard cap on plants.
    pub span_plants: i64,
    pub span_weeks_per_plant: i64,
    /// Weeks a plant must have run before a rival losing money over the quarter mothballs it for
    /// losing money at its own figures. Zero: rivals never close a plant for losses.
    pub close_after_loss_weeks: u32,
    /// Weeks of mandatory outflow a firm keeps as a prudence buffer on top of its plans before
    /// it counts cash as idle. Zero turns payouts off.
    pub payout_buffer_weeks: i64,
    /// Share of idle cash paid out each quarter, in basis points.
    pub payout_share_bps: i64,
    /// Weeks between a raid being announced and its resolution. Zero resolves a raid at once.
    pub raid_warning_weeks: u32,
    /// How households' opening demand for what they buy is sized to the playable market, in basis
    /// points. Household income is the whole scaled regional economy, but only the named firms
    /// sell to households. Where named sellers stand, households ask for what those sellers make
    /// for them over `household_cover_bps`; elsewhere they ask for an even slice of their budget,
    /// sized so the whole corridor opens with `household_unmet_bps` of what households ask for
    /// finding no seller. What households no longer ask for stays in savings. Zero cover:
    /// households ask for their whole category budget (the rule before 2.0).
    pub household_cover_bps: i64,
    pub household_unmet_bps: i64,
    /// Weeks a plant on offer stays open for bids: a failed company's plants in its break-up
    /// auction, and a plant its owner lists for sale. Zero: a failed company's plants are sold
    /// off at once for scrap and no plant can be listed (the rule before 2.0).
    pub plant_auction_weeks: u32,
}

impl Default for CapacityRules {
    fn default() -> Self {
        CapacityRules {
            build_price_level_bps: 0,
            build_inputs: Vec::new(),
            build_price_min_bps: 10_000,
            build_price_max_bps: 10_000,
            local_room: false,
            opening_patient_tenths: 0,
            opening_keen_tenths: 0,
            opening_entry_tenths: 0,
            span_plants: 0,
            span_weeks_per_plant: 0,
            close_after_loss_weeks: 0,
            payout_buffer_weeks: 0,
            payout_share_bps: 0,
            raid_warning_weeks: 0,
            household_cover_bps: 0,
            household_unmet_bps: 0,
            plant_auction_weeks: 0,
        }
    }
}

/// How the outside world's gateways and the corridor's sellers respond to trade volume. Shared by
/// every scenario: a scenario supplies its own gateways, prices and plants, never its own rules.
///
/// A gateway's weekly room is split into steps. The first step trades at the gateway's quote;
/// each further step costs more to import and pays less to export, so a large shortage or glut
/// moves the price paid at the margin instead of hitting a hard pin. Sellers read last week's
/// marginal step: imports in the dear steps mean the corridor is short and home prices rise,
/// exports in the cheap steps mean it is glutted and home prices fall.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MarketRules {
    /// The first import step, traded at the quote, as a share of a gateway's weekly room, in
    /// basis points.
    pub import_first_step_bps: i64,
    /// The first export step, paid at the quote, as a share of a gateway's weekly room. A
    /// gateway's room grows to what the corridor exports, so this is normally all of it.
    pub export_first_step_bps: i64,
    /// Every further step's size as a share of a gateway's weekly room, in basis points.
    pub gateway_step_bps: i64,
    /// Steps a gateway trades in a week, each way. Volume beyond the last step is not traded.
    pub gateway_steps: i64,
    /// How much dearer each further import step is than the quote, in basis points of it.
    pub import_step_premium_bps: i64,
    /// How much less each further export step pays than the quote, in basis points of it.
    pub export_step_discount_bps: i64,
    /// The least an export step pays, in basis points of the quote.
    pub export_floor_bps: i64,
    /// Share of last week's marginal import premium or export discount a home seller passes into
    /// the price it wants, in basis points.
    pub trade_pressure_pass_bps: i64,
    /// Whether a good is priced against the outside world's middle quote (halfway between the
    /// gateway's import and export prices) rather than its fixed opening catalogue price, so home
    /// prices follow world prices and inflation the way the gateways do.
    pub anchor_to_outside: bool,
    /// Share of the outside middle quote's move away from the catalogue price that the anchor
    /// follows, in basis points (10,000 follows it fully). Plant building costs do not follow
    /// world prices, so a full pass-through turned a world boom into a windfall for whoever
    /// built new plants in it, and a bust into a loss.
    pub anchor_pass_bps: i64,
    /// Room a gateway adds in a week its trade ran past the first step (exports filled it, or
    /// imports reached the dear steps), in basis points of its room. Ports and rail are extended
    /// slowly, so a glut or shortage lasts months, not weeks.
    pub room_growth_bps: i64,
    /// Room a gateway gives up in a week its busier direction used less than
    /// `room_shrink_below_bps` of it, in basis points of its room. A gateway that moved nothing
    /// keeps its room: there is no use to read.
    pub room_shrink_bps: i64,
    /// Use below this share of a gateway's room, in basis points, lets the room shrink.
    pub room_shrink_below_bps: i64,
    /// Points of capacity, in basis points, a plant's run rate moves in a week toward what its
    /// good sells. A slow ramp keeps a new plant's surplus, or a closed plant's gap, in the
    /// market for months.
    pub plan_step_bps: i32,
    /// The share of its full capacity a service seller sells at the reference price, in basis
    /// points. Selling more asks more, selling less asks less.
    pub service_balance_bps: i64,
    /// Basis points of price a service seller adds for every basis point of capacity sold over
    /// the balance point (and takes off under it), in hundredths: 60 is 0.6.
    pub service_slope_pct: i64,
    /// The least and most a service seller asks, in basis points of reference.
    pub service_min_bps: i64,
    pub service_max_bps: i64,
    /// Whether households that found no service count as demand on top of what sold, so a
    /// region short of capacity reads as more than full.
    pub service_counts_unmet: bool,
    /// The highest price a good with no import, or a service, may ask, in basis points of its
    /// reference. A good with an import is held under the import instead.
    pub local_ceiling_bps: i64,
}

impl Default for MarketRules {
    fn default() -> Self {
        MarketRules {
            import_first_step_bps: 10_000,
            export_first_step_bps: 10_000,
            gateway_step_bps: 10_000,
            gateway_steps: 1,
            import_step_premium_bps: 0,
            export_step_discount_bps: 0,
            export_floor_bps: 10_000,
            trade_pressure_pass_bps: 0,
            anchor_to_outside: false,
            anchor_pass_bps: 10_000,
            room_growth_bps: 100,
            room_shrink_bps: 0,
            room_shrink_below_bps: 0,
            plan_step_bps: 1_500,
            service_balance_bps: 8_500,
            service_slope_pct: 60,
            service_min_bps: 8_000,
            service_max_bps: 11_500,
            service_counts_unmet: false,
            local_ceiling_bps: 11_500,
        }
    }
}

/// A scenario's starting balance on top of the shared products and strategies: the wage level
/// and the opening reference price of any product whose price differs from the base file.
/// Strategies, inputs and outputs are universal; only money levels are the scenario's.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BalanceOverlay {
    #[serde(default)]
    pub classification: String,
    #[serde(default)]
    pub baseline_weekly_wage_cents: Option<[i64; 3]>,
    #[serde(default)]
    pub fixture_land_rent_cents_weekly: Option<i64>,
    #[serde(default)]
    pub maintenance_annual_bps: Option<i32>,
    /// Product id to opening reference price in cents. Unknown ids are an error.
    #[serde(default)]
    pub reference_price_cents: BTreeMap<String, i64>,
}

impl BalanceOverlay {
    /// Apply to a parsed balance, or name the product the overlay got wrong.
    pub fn apply(&self, balance: &mut Balance) -> Result<(), String> {
        if let Some(wages) = self.baseline_weekly_wage_cents {
            balance.baseline_weekly_wage_cents = wages;
        }
        if let Some(rent) = self.fixture_land_rent_cents_weekly {
            balance.fixture_land_rent_cents_weekly = rent;
        }
        if let Some(bps) = self.maintenance_annual_bps {
            balance.maintenance_annual_bps = bps;
        }
        for (id, price) in &self.reference_price_cents {
            let product = balance
                .products
                .iter_mut()
                .find(|product| &product.id == id)
                .ok_or_else(|| format!("scenario balance prices unknown product {id}"))?;
            product.reference_price_cents = *price;
        }
        Ok(())
    }
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
