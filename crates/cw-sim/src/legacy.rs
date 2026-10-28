//! Frozen save layouts of older releases, and their conversion to the current state.
//!
//! The binary save payload is postcard, which writes each struct as its fields in order with no
//! names or count. A field added to a saved struct therefore changes the layout, and
//! `#[serde(default)]` only helps self-describing formats such as JSON. Every layout change bumps
//! `LAYOUT` and keeps a frozen copy here of each struct that changed, as it was in the older
//! layout. Reading an older save runs inside `read_layout`, which makes the changed structs
//! decode their frozen copy and convert it, filling new fields with neutral defaults.
//!
//! Layout 1: releases up to 1.8 preview 2. Layout 2 added `Facility.cut_bps`,
//! `Policies.production_locked` and `Household.wanted_milli` / `unmet_milli`.

use std::cell::Cell;
use std::collections::BTreeMap;

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::quantity::MilliLots;
use serde::Deserialize;

use crate::state::{Facility, FacilityStatus, Household, Policies, PricePosture};

/// The layout this build writes.
pub const LAYOUT: u32 = 2;
/// The oldest layout this build can read.
pub const MIN_LAYOUT: u32 = 1;

thread_local! {
    static READING: Cell<u32> = const { Cell::new(LAYOUT) };
}

/// Run `read` with the changed structs decoding the given older layout. The current layout is
/// restored afterwards, also on a panic, so no later decode on this thread is affected.
pub fn read_layout<R>(layout: u32, read: impl FnOnce() -> R) -> R {
    struct Restore(u32);
    impl Drop for Restore {
        fn drop(&mut self) {
            READING.with(|cell| cell.set(self.0));
        }
    }
    let _restore = Restore(READING.with(|cell| cell.replace(layout)));
    read()
}

/// The layout being read on this thread.
pub(crate) fn reading() -> u32 {
    READING.with(Cell::get)
}

#[derive(Deserialize)]
pub(crate) struct FacilityV1 {
    firm: Handle,
    region: u8,
    recipe: u16,
    blocks: u8,
    status: FacilityStatus,
    started: Tick,
    completes: Tick,
    escrow_cents: i64,
    equipment_due_cents: i64,
    capex_cents: i64,
    accumulated_depreciation: i64,
    workers: [i64; 3],
    target_bps: i32,
    quality_bps: i32,
    deposit_remaining: MilliLots,
    last_output: MilliLots,
    last_inputs_short: bool,
    last_staff_ratio_bps: i32,
    mothballed_since: Option<Tick>,
    outage: Option<(i32, Tick)>,
    stock_plan: crate::economy::storage::StockPlan,
    ship: crate::economy::logistics::ShipPolicy,
    serves: Vec<u8>,
}

impl From<FacilityV1> for Facility {
    fn from(old: FacilityV1) -> Self {
        Facility {
            firm: old.firm,
            region: old.region,
            recipe: old.recipe,
            blocks: old.blocks,
            status: old.status,
            started: old.started,
            completes: old.completes,
            escrow_cents: old.escrow_cents,
            equipment_due_cents: old.equipment_due_cents,
            capex_cents: old.capex_cents,
            accumulated_depreciation: old.accumulated_depreciation,
            workers: old.workers,
            target_bps: old.target_bps,
            quality_bps: old.quality_bps,
            deposit_remaining: old.deposit_remaining,
            last_output: old.last_output,
            last_inputs_short: old.last_inputs_short,
            last_staff_ratio_bps: old.last_staff_ratio_bps,
            mothballed_since: old.mothballed_since,
            outage: old.outage,
            stock_plan: old.stock_plan,
            ship: old.ship,
            serves: old.serves,
            // The recipe's fixed split, as the plant ran before cuts could shift.
            cut_bps: Vec::new(),
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct PoliciesV1 {
    prices: BTreeMap<u16, i64>,
    wage_offer: [i64; 3],
    production_target_bps: i32,
    marketing_budget_cents: i64,
    research_budget_cents: i64,
    logistics_budget_cents: i64,
    training_budget_cents: i64,
    quality_budgets_cents: BTreeMap<u16, i64>,
    input_cover_weeks: i32,
    manual_prices: std::collections::BTreeSet<u16>,
    price_postures: BTreeMap<u16, PricePosture>,
    manual_production: bool,
}

impl From<PoliciesV1> for Policies {
    fn from(old: PoliciesV1) -> Self {
        Policies {
            prices: old.prices,
            wage_offer: old.wage_offer,
            production_target_bps: old.production_target_bps,
            marketing_budget_cents: old.marketing_budget_cents,
            research_budget_cents: old.research_budget_cents,
            logistics_budget_cents: old.logistics_budget_cents,
            training_budget_cents: old.training_budget_cents,
            quality_budgets_cents: old.quality_budgets_cents,
            input_cover_weeks: old.input_cover_weeks,
            manual_prices: old.manual_prices,
            price_postures: old.price_postures,
            manual_production: old.manual_production,
            // A hand-set rate from before the lock existed leaves services following sales.
            production_locked: false,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct HouseholdV1 {
    savings_cents: i64,
    last_income_cents: i64,
    baseline_income_cents: i64,
    weights: BTreeMap<u16, i64>,
    baseline_lots: BTreeMap<u16, i64>,
    last_sale_price_cents: BTreeMap<u16, i64>,
    spend_cents: BTreeMap<u16, i64>,
}

impl From<HouseholdV1> for Household {
    fn from(old: HouseholdV1) -> Self {
        Household {
            savings_cents: old.savings_cents,
            last_income_cents: old.last_income_cents,
            baseline_income_cents: old.baseline_income_cents,
            weights: old.weights,
            baseline_lots: old.baseline_lots,
            last_sale_price_cents: old.last_sale_price_cents,
            spend_cents: old.spend_cents,
            // No week of unmet demand recorded yet; the first played week fills both.
            wanted_milli: BTreeMap::new(),
            unmet_milli: BTreeMap::new(),
        }
    }
}

/// Serde glue for a struct whose layout changed: the derive is generated as inherent functions
/// (`#[serde(remote = "Self")]`) and the trait impls pick the frozen copy when reading an older
/// layout. Writing always uses the current layout.
macro_rules! versioned {
    ($current:ty, $v1:ty) => {
        impl serde::Serialize for $current {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                <$current>::serialize(self, serializer)
            }
        }
        impl<'de> serde::Deserialize<'de> for $current {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                if reading() < 2 {
                    <$v1 as serde::Deserialize>::deserialize(deserializer).map(Into::into)
                } else {
                    <$current>::deserialize(deserializer)
                }
            }
        }
    };
}

versioned!(Facility, FacilityV1);
versioned!(Policies, PoliciesV1);
versioned!(Household, HouseholdV1);
