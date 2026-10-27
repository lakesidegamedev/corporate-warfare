//! Storage: finite regional capacity by class, outside storage, holding costs, spoilage,
//! standing stock targets and speculative buy-and-store.
//!
//! Stock is still kept per firm, product and region. This module adds the space it needs:
//!
//! * Every operating plant brings a working yard, [`YARD_WEEKS`] of its weekly inputs and
//!   outputs per storage class. Storage facilities (logistics recipes with a storage class) add
//!   owned capacity of one class in one region.
//! * Stock beyond yard plus owned capacity sits in the region's outside storage and pays a weekly
//!   fee per lot. Goods never vanish for lack of space. The fee goes to other firms with spare
//!   owned capacity there, pro rata, and the rest to the outside sector.
//! * Stock beyond the yard also pays a holding cost, a weekly share of its reference value.
//! * Perishable spoilage (see `production::spoil_perishables`) is cut by 80 percent for the part of a
//!   class's stock that sits in dedicated owned capacity.
//! * Each plant may carry a stock target, weeks of inputs to keep, and a seasonal build that
//!   buys grain ahead of winter in the harvest quarter.
//! * A firm may hold goods back from sale (`held`) and queue buy orders on the corridor
//!   exchange that add to it: speculation. Held goods are reserved like input cover until
//!   released.

use std::collections::BTreeMap;

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::{Cents, allocate_cents, round_even_div};
use cw_core::quantity::MilliLots;
use cw_data::{Dataset, StorageClass};
use serde::{Deserialize, Serialize};

use crate::state::{Facility, FacilityStatus, Firm, World};

/// Weeks of a plant's weekly inputs and outputs its own yard holds, per class.
pub const YARD_WEEKS: i64 = 6;
/// Share by which dedicated owned storage cuts spoilage, in basis points. A cold store is the
/// answer to a perishable glut, so it removes most of the loss, not half of it.
pub const PROTECTED_SPOILAGE_CUT_BPS: i64 = 8_000;
/// Extra weeks of seasonal input a plant builds in the harvest quarter.
pub const SEASON_EXTRA_WEEKS: i64 = 8;
/// The harvest quarter of the farm season table.
pub const HARVEST_QUARTER: u32 = 3;
/// Longest stock target a plant may set.
pub const MAX_STOCK_WEEKS: i32 = 26;
/// Most whole lots one buy order may ask for, and how many orders a firm may keep.
pub const MAX_ORDER_LOTS: i64 = 5_000;
pub const MAX_ORDERS: usize = 6;
/// Lots one order may take in a single week, so a speculator cannot clear a market in one go.
pub const ORDER_LOTS_PER_WEEK: i64 = 400;
/// Weeks an order stays open.
pub const ORDER_WEEKS: u32 = 13;

/// A plant's standing stock policy. Zero weeks means the firm-wide default input cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StockPlan {
    pub weeks: i32,
    /// Buy extra seasonal inputs (grain) in the harvest quarter. On unless turned off.
    pub seasonal: bool,
}

impl Default for StockPlan {
    fn default() -> Self {
        StockPlan {
            weeks: 0,
            seasonal: true,
        }
    }
}

/// A standing order to buy a good on the exchange into held stock.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecOrder {
    pub product: u16,
    pub region: u8,
    pub lots_left: i64,
    /// Most the buyer will pay per lot, in basis points of the reference price.
    pub limit_bps: i32,
    pub expires: u32,
}

/// One week's storage results for a firm.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageWeek {
    pub fee_cents: i64,
    pub holding_cents: i64,
    pub spoiled_cents: i64,
    pub spoiled_milli: i64,
    /// Lots stored, lots of owned and yard capacity, and lots in outside storage at the close.
    pub used_lots: i64,
    pub capacity_lots: i64,
    pub outside_lots: i64,
    /// Revenue from other firms' overflow fees, for storage owners.
    pub rent_cents: i64,
}

/// A firm's storage book.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageBook {
    /// Stock held back from sale and export, by (product, region).
    pub held: BTreeMap<(u16, u8), MilliLots>,
    pub orders: Vec<SpecOrder>,
    pub last: StorageWeek,
    /// The last thirteen weeks, oldest first.
    pub history: Vec<StorageWeek>,
    /// Every milli-lot lost to spoilage since the firm began: the recorded loss for conservation.
    pub spoiled_total_milli: i64,
}

impl StorageBook {
    pub fn held_of(&self, product: u16, region: u8) -> MilliLots {
        self.held
            .get(&(product, region))
            .copied()
            .unwrap_or(MilliLots::ZERO)
    }

    /// Sum of the last `weeks` closed weeks.
    pub fn recent(&self, weeks: usize) -> StorageWeek {
        let mut sum = StorageWeek::default();
        for week in self.history.iter().rev().take(weeks) {
            sum.fee_cents += week.fee_cents;
            sum.holding_cents += week.holding_cents;
            sum.spoiled_cents += week.spoiled_cents;
            sum.spoiled_milli += week.spoiled_milli;
            sum.rent_cents += week.rent_cents;
        }
        sum
    }
}

/// Space a firm has in one region for one class, in milli-lots.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Space {
    pub yard: i64,
    pub owned: i64,
}

impl Space {
    pub fn total(&self) -> i64 {
        self.yard + self.owned
    }
}

/// A firm's use of one class of storage in one region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Use {
    pub region: u8,
    pub class: StorageClass,
    pub used_milli: i64,
    pub yard_milli: i64,
    pub owned_milli: i64,
}

impl Use {
    pub fn capacity_milli(&self) -> i64 {
        self.yard_milli + self.owned_milli
    }

    pub fn outside_milli(&self) -> i64 {
        (self.used_milli - self.capacity_milli()).max(0)
    }
}

fn operating(facility: &Facility) -> bool {
    facility.status == FacilityStatus::Operating
}

/// Yard and owned space a firm has, by region and class, in milli-lots.
pub fn space(world: &World, dataset: &Dataset, firm: &Firm) -> BTreeMap<(u8, StorageClass), Space> {
    let mut out: BTreeMap<(u8, StorageClass), Space> = BTreeMap::new();
    for handle in &firm.facilities {
        let Some(facility) = world.facilities.get(*handle) else {
            continue;
        };
        if !operating(facility) {
            continue;
        }
        let recipe = &dataset.recipes[facility.recipe as usize];
        let blocks = facility.blocks as i64;
        if let Some(class) = recipe.storage_class {
            out.entry((facility.region, class)).or_default().owned +=
                recipe.storage_lots() * blocks * 1_000;
            continue;
        }
        let mut add = |id: &str, lots: i64| {
            let Some(code) = dataset.product_index.get(id) else {
                return;
            };
            if let Some(class) = dataset.storage_class_of(*code) {
                out.entry((facility.region, class)).or_default().yard +=
                    lots * blocks * YARD_WEEKS * 1_000;
            }
        };
        for (id, lots) in recipe.outputs() {
            add(id, lots);
        }
        for (id, lots) in &recipe.weekly_inputs_lots {
            add(id, *lots);
        }
    }
    out
}

/// Every (region, class) cell where a firm holds stock or has space, with use and capacity.
pub fn uses(world: &World, dataset: &Dataset, firm: &Firm) -> Vec<Use> {
    let space = space(world, dataset, firm);
    let mut used: BTreeMap<(u8, StorageClass), i64> = BTreeMap::new();
    for ((product, region), lot) in &firm.inventory {
        if lot.qty.0 <= 0 {
            continue;
        }
        if let Some(class) = dataset.storage_class_of(*product) {
            *used.entry((*region, class)).or_default() += lot.qty.0;
        }
    }
    let mut keys: Vec<(u8, StorageClass)> = space.keys().chain(used.keys()).copied().collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .map(|(region, class)| {
            let s = space.get(&(region, class)).copied().unwrap_or_default();
            Use {
                region,
                class,
                used_milli: used.get(&(region, class)).copied().unwrap_or(0),
                yard_milli: s.yard,
                owned_milli: s.owned,
            }
        })
        .collect()
}

/// Weeks of a plant's input the firm keeps, after the stock target, perishable limit and the
/// seasonal build.
pub fn plant_cover_weeks(
    world: &World,
    dataset: &Dataset,
    firm: &Firm,
    facility: &Facility,
    product: u16,
) -> i64 {
    let spoils = dataset.products[product as usize].spoilage_bps_weekly >= 200;
    let mut weeks = if facility.stock_plan.weeks > 0 {
        facility.stock_plan.weeks as i64
    } else {
        let default = firm.policies.input_cover_weeks.max(1) as i64;
        if spoils { default.min(2) } else { default }
    };
    if facility.stock_plan.seasonal
        && !spoils
        && world.tick.quarter() == HARVEST_QUARTER
        && is_seasonal_good(dataset, product)
    {
        weeks += SEASON_EXTRA_WEEKS;
    }
    weeks
}

/// A good a farm grows, whose supply swings with the season.
pub fn is_seasonal_good(dataset: &Dataset, product: u16) -> bool {
    let id = &dataset.products[product as usize].id;
    dataset
        .recipes
        .iter()
        .any(|recipe| recipe.industry == "agriculture" && recipe.makes(id))
}

/// Milli-lots of a good a firm reserves in a region: its plants' input cover plus held goods.
/// Reserved stock is never offered for sale or export.
pub fn reserved_milli(
    world: &World,
    dataset: &Dataset,
    firm: &Firm,
    region: u8,
    product: u16,
) -> i64 {
    let mut need = 0i64;
    for facility_handle in &firm.facilities {
        let Some(facility) = world.facilities.get(*facility_handle) else {
            continue;
        };
        if facility.region != region {
            continue;
        }
        let recipe = &dataset.recipes[facility.recipe as usize];
        if let Some(weekly) = recipe
            .weekly_inputs_lots
            .iter()
            .find(|(id, _)| dataset.product_index[*id] == product)
            .map(|(_, qty)| *qty)
        {
            need += weekly
                * facility.blocks as i64
                * plant_cover_weeks(world, dataset, firm, facility, product)
                * 1_000;
        }
    }
    need + firm.storage.held_of(product, region).0
}

fn fee_per_lot(class: StorageClass) -> i64 {
    class.outside_fee_cents()
}

/// Weekly storage charges, run before the week's spoilage. Resets the week's record.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    let handles: Vec<Handle> = world.firms.iter().map(|(h, _)| h).collect();
    // Free owned space of every firm, by (region, class), for the rent of other firms' overflow.
    let mut free: BTreeMap<(u8, StorageClass), Vec<(Handle, i64)>> = BTreeMap::new();
    let mut per_firm: BTreeMap<Handle, Vec<Use>> = BTreeMap::new();
    for handle in &handles {
        let Some(firm) = world.firms.get(*handle) else {
            continue;
        };
        let all = uses(world, dataset, firm);
        if firm.operative() {
            for u in &all {
                if u.owned_milli > 0 {
                    let spare = (u.owned_milli - (u.used_milli - u.yard_milli).max(0)).max(0);
                    if spare > 0 {
                        free.entry((u.region, u.class))
                            .or_default()
                            .push((*handle, spare));
                    }
                }
            }
        }
        per_firm.insert(*handle, all);
    }
    // First pass: every firm's charges and who is owed rent.
    let mut owed: BTreeMap<(u8, StorageClass), Vec<(Handle, i64)>> = BTreeMap::new();
    let mut rent_in: BTreeMap<Handle, i64> = BTreeMap::new();
    let mut to_outside = 0i64;
    for handle in &handles {
        let all = per_firm.remove(handle).unwrap_or_default();
        let Some(firm) = world.firms.get_mut(*handle) else {
            continue;
        };
        clamp_held(firm);
        let mut week = StorageWeek::default();
        let mut fee_total = 0i64;
        let mut holding_total = 0i64;
        let mut fee_by_cell: Vec<((u8, StorageClass), i64)> = Vec::new();
        for u in &all {
            week.used_lots += u.used_milli / 1_000;
            week.capacity_lots += u.capacity_milli() / 1_000;
            week.outside_lots += u.outside_milli() / 1_000;
            if u.used_milli <= 0 {
                continue;
            }
            // Outside storage fee on the overflow.
            let overflow = u.outside_milli();
            let fee = round_even_div(overflow as i128 * fee_per_lot(u.class) as i128, 1_000) as i64;
            if fee > 0 {
                fee_total += fee;
                fee_by_cell.push(((u.region, u.class), fee));
            }
            // Holding cost on stock beyond the yard, by value.
            let excess = (u.used_milli - u.yard_milli).max(0);
            if excess > 0 {
                for ((product, region), lot) in &firm.inventory {
                    if *region != u.region || lot.qty.0 <= 0 {
                        continue;
                    }
                    if dataset.storage_class_of(*product) != Some(u.class) {
                        continue;
                    }
                    let product_data = &dataset.products[*product as usize];
                    let share_milli =
                        round_even_div(lot.qty.0 as i128 * excess as i128, u.used_milli as i128)
                            as i128;
                    let value = share_milli * product_data.reference_price_cents as i128;
                    holding_total += round_even_div(
                        value * product_data.holding_bps_weekly as i128,
                        10_000 * 1_000,
                    ) as i64;
                }
            }
        }
        let cash = firm.book.cash().max(0);
        let total = fee_total + holding_total;
        let paid = total.min(cash);
        if paid > 0 {
            // Fees first; holding cost is the remainder.
            let fee_paid = fee_total.min(paid);
            let holding_paid = paid - fee_paid;
            let _ = firm.book.commit(
                world.tick,
                Txn::new().credit(Account::Cash, paid, "storage").debit(
                    Account::OperatingExpense,
                    paid,
                    "storage",
                ),
            );
            week.fee_cents = fee_paid;
            week.holding_cents = holding_paid;
            to_outside += holding_paid;
            // The fee is owed to storage owners with spare space in the same cells.
            let mut left = fee_paid;
            for (cell, fee) in &fee_by_cell {
                let part = (*fee).min(left);
                if part <= 0 {
                    continue;
                }
                left -= part;
                owed.entry(*cell).or_default().push((*handle, part));
            }
        }
        firm.storage.last = week;
    }
    // Pay rents: each cell's fees go to other firms with spare owned space, pro rata to the
    // space; the remainder goes to the outside sector.
    for (cell, payers) in owed {
        let landlords = free.get(&cell).cloned().unwrap_or_default();
        for (payer, mut fee) in payers {
            let others: Vec<(Handle, i64)> = landlords
                .iter()
                .filter(|(h, _)| *h != payer)
                .copied()
                .collect();
            let capacity: i64 = others.iter().map(|(_, s)| *s).sum();
            if capacity > 0 && fee > 0 {
                // Landlords can earn on at most the overflow their space covers, priced at the
                // fee, so a vast empty store does not collect fees for space nobody used.
                let weights: Vec<(u64, i64)> = others.iter().map(|(h, s)| (h.0, *s)).collect();
                let split = allocate_cents(Cents(fee), &weights);
                for ((handle, _), (_, share)) in others.iter().zip(split) {
                    if share.0 <= 0 {
                        continue;
                    }
                    if let Some(firm) = world.firms.get_mut(*handle) {
                        let _ = firm.book.commit(
                            world.tick,
                            Txn::new()
                                .debit(Account::Cash, share.0, "storage rent")
                                .credit(Account::Revenue, share.0, "storage rent"),
                        );
                        *rent_in.entry(*handle).or_default() += share.0;
                    }
                }
                fee = 0;
            }
            to_outside += fee;
        }
    }
    if to_outside > 0 {
        let _ = world.outside.gateway_book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, to_outside, "storage")
                .credit(Account::Revenue, to_outside, "storage"),
        );
    }
    for (handle, rent) in rent_in {
        if let Some(firm) = world.firms.get_mut(handle) {
            firm.storage.last.rent_cents += rent;
        }
    }
}

/// Held goods cannot exceed the stock actually on hand (production and contracts may use it).
fn clamp_held(firm: &mut Firm) {
    let keys: Vec<(u16, u8)> = firm.storage.held.keys().copied().collect();
    for key in keys {
        let on_hand = firm.stock(key.0, key.1);
        let held = firm.storage.held_of(key.0, key.1);
        if held.0 > on_hand.0 {
            if on_hand.0 <= 0 {
                firm.storage.held.remove(&key);
            } else {
                firm.storage.held.insert(key, on_hand);
            }
        }
    }
}

/// Close the week: push the record into the history and drop expired or finished orders.
pub fn close_week(world: &mut World) {
    let tick = world.tick.0;
    for (_, firm) in world.firms.iter_mut() {
        clamp_held(firm);
        let week = firm.storage.last;
        firm.storage.history.push(week);
        if firm.storage.history.len() > 13 {
            firm.storage.history.remove(0);
        }
        firm.storage
            .orders
            .retain(|order| order.lots_left > 0 && order.expires > tick);
    }
}

/// The spoilage rate for a good in basis points a week after dedicated owned storage: the part
/// of a class's stock that sits in owned capacity spoils at a fifth of the rate.
pub fn spoilage_bps(base_bps: u32, owned_milli: i64, used_milli: i64) -> i64 {
    if base_bps == 0 {
        return 0;
    }
    let protected = if used_milli <= 0 {
        0
    } else {
        (owned_milli.min(used_milli) * 10_000 / used_milli).clamp(0, 10_000)
    };
    let cut = PROTECTED_SPOILAGE_CUT_BPS * protected / 10_000;
    base_bps as i64 * (10_000 - cut) / 10_000
}

/// Place a buy-to-store order. Returns false when the book is full or the order is invalid.
pub fn place_order(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    product: u16,
    region: u8,
    lots: i64,
    limit_bps: i32,
) -> bool {
    let Some(data) = dataset.products.get(product as usize) else {
        return false;
    };
    if data.kind != cw_data::ProductKind::Goods || lots <= 0 || lots > MAX_ORDER_LOTS {
        return false;
    }
    let expires = world.tick.0 + ORDER_WEEKS;
    let Some(entity) = world.firms.get_mut(firm) else {
        return false;
    };
    // A second order for the same good and region tops up the first.
    if let Some(existing) = entity
        .storage
        .orders
        .iter_mut()
        .find(|o| o.product == product && o.region == region)
    {
        existing.lots_left = (existing.lots_left + lots).min(MAX_ORDER_LOTS);
        existing.limit_bps = limit_bps;
        existing.expires = expires;
        return true;
    }
    if entity.storage.orders.len() >= MAX_ORDERS {
        return false;
    }
    entity.storage.orders.push(SpecOrder {
        product,
        region,
        lots_left: lots,
        limit_bps,
        expires,
    });
    true
}

/// Release held goods for sale. `lots` of zero releases everything held in the cell and cancels
/// its open buy order. Returns the milli-lots released.
pub fn release(world: &mut World, firm: Handle, product: u16, region: u8, lots: i64) -> MilliLots {
    let Some(entity) = world.firms.get_mut(firm) else {
        return MilliLots::ZERO;
    };
    let held = entity.storage.held_of(product, region);
    let take = if lots <= 0 {
        entity
            .storage
            .orders
            .retain(|o| !(o.product == product && o.region == region));
        held
    } else {
        MilliLots::from_lots(lots).min(held)
    };
    let left = MilliLots(held.0 - take.0);
    if left.0 <= 0 {
        entity.storage.held.remove(&(product, region));
    } else {
        entity.storage.held.insert((product, region), left);
    }
    take
}

/// Buyers for the speculative pass of the exchange, (firm, region, milli-lots wanted, budget),
/// with the highest delivered price per lot each will pay.
pub type OrderBuyers = (Vec<(Handle, u8, i64, i64)>, BTreeMap<(Handle, u8), i64>);

/// The speculative buy orders open this week, by good.
pub fn order_buyers(world: &World, dataset: &Dataset) -> BTreeMap<u16, OrderBuyers> {
    let mut out: BTreeMap<u16, OrderBuyers> = BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        if !firm.operative() || firm.storage.orders.is_empty() {
            continue;
        }
        for order in &firm.storage.orders {
            if order.lots_left <= 0 {
                continue;
            }
            let lots = order.lots_left.min(ORDER_LOTS_PER_WEEK);
            let reference = dataset.reference_price(order.product);
            let limit = reference * order.limit_bps as i64 / 10_000;
            let cash = (firm.book.cash().max(0) - firm.reserved_cash_cents).max(0);
            let budget = (lots * limit).min(cash);
            if budget <= 0 {
                continue;
            }
            let entry = out.entry(order.product).or_default();
            entry.0.push((handle, order.region, lots * 1_000, budget));
            entry.1.insert((handle, order.region), limit);
        }
    }
    out
}

/// Record goods a buy order received: they join the held stock and reduce the order.
pub fn record_received(world: &mut World, firm: Handle, product: u16, region: u8, got: MilliLots) {
    if got.0 <= 0 {
        return;
    }
    let Some(entity) = world.firms.get_mut(firm) else {
        return;
    };
    let held = entity.storage.held_of(product, region);
    entity
        .storage
        .held
        .insert((product, region), MilliLots(held.0 + got.0));
    let lots = got.whole_lots_floor().max(1);
    if let Some(order) = entity
        .storage
        .orders
        .iter_mut()
        .find(|o| o.product == product && o.region == region)
    {
        order.lots_left = (order.lots_left - lots).max(0);
    }
}
