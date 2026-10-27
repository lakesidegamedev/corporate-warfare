//! The strategic reserve: a government stockpile of extraction goods.
//!
//! When the law is on, the government buys a commodity when its price has fallen well under
//! its year average (a bust) and sells it back into the market when the price runs well over
//! the average (a spike). It competes with speculators for the same goods: it buys from plants'
//! free stock at the corridor price, and sells to buyers in the region where the stock sits,
//! just under the going price, through the same exchange everyone else uses. It pays and is paid
//! in real cash on the government's book, so the money always has a counterparty.
//!
//! Prices are read from the saved weekly price history, so a loaded game decides exactly as a
//! continued one does.

use std::collections::BTreeMap;

use cw_core::ledger::{Account, Txn};
use cw_core::money::Cents;
use cw_core::quantity::MilliLots;
use cw_data::Dataset;
use serde::{Deserialize, Serialize};

use crate::state::World;

/// A price this far under its year average is a bust, and this far over a spike.
pub const BUST_PCT: i64 = 85;
pub const SPIKE_PCT: i64 = 120;
/// Weeks of price history needed before the reserve judges a price.
pub const MIN_HISTORY: usize = 26;
/// Most lots of one good the reserve will hold, bought per week, and sold per region per week.
pub const MAX_STOCK_LOTS: i64 = 4_000;
pub const BUY_LOTS_PER_WEEK: i64 = 300;
pub const SELL_LOTS_PER_WEEK: i64 = 150;
/// Share of the government's cash the reserve may spend in a week, as one part in this many.
pub const BUDGET_DIVISOR: i64 = 40;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reserve {
    /// Stock by (product, region) in milli-lots, and its carrying cost in cents.
    pub stock: BTreeMap<(u16, u8), i64>,
    pub cost: BTreeMap<(u16, u8), i64>,
    pub bought_cents: i64,
    pub sold_cents: i64,
    pub bought_milli: i64,
    pub sold_milli: i64,
}

impl Reserve {
    pub fn lots_of(&self, product: u16) -> i64 {
        self.stock
            .iter()
            .filter(|((p, _), _)| *p == product)
            .map(|(_, q)| *q)
            .sum::<i64>()
            / 1_000
    }

    pub fn total_lots(&self) -> i64 {
        self.stock.values().sum::<i64>() / 1_000
    }

    pub fn value_cents(&self) -> i64 {
        self.cost.values().sum()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Buying,
    Selling,
}

/// The goods the reserve deals in: what extraction plants make, minus perishables.
pub fn strategic_goods(ds: &Dataset) -> Vec<u16> {
    let mut found: Vec<u16> = ds
        .recipes
        .iter()
        .filter(|r| r.industry == "extraction")
        .filter_map(|r| ds.product_index.get(&r.output).copied())
        .filter(|p| ds.products[*p as usize].spoilage_bps_weekly == 0)
        .collect();
    found.sort_unstable();
    found.dedup();
    found
}

/// The price now and its year average, when there is enough history.
pub fn band(world: &World, ds: &Dataset, product: u16) -> Option<(i64, i64)> {
    let prices: Vec<i64> = world
        .price_history
        .iter()
        .filter_map(|point| point.goods.get(&product).copied())
        .collect();
    if prices.len() < MIN_HISTORY {
        return None;
    }
    let mean = prices.iter().sum::<i64>() / prices.len() as i64;
    let now = crate::economy::pricing::corridor_price(world, ds, product);
    Some((now, mean.max(1)))
}

pub fn phase(world: &World, ds: &Dataset, product: u16) -> Phase {
    if !world.politics.laws.reserve {
        return Phase::Idle;
    }
    match band(world, ds, product) {
        Some((now, mean)) if now * 100 < mean * BUST_PCT => Phase::Buying,
        Some((now, mean)) if now * 100 > mean * SPIKE_PCT => Phase::Selling,
        _ => Phase::Idle,
    }
}

/// What the reserve offers a region this week: (asking price per lot, milli-lots).
pub fn release_offer(world: &World, ds: &Dataset, product: u16, region: u8) -> Option<(i64, i64)> {
    if phase(world, ds, product) != Phase::Selling {
        return None;
    }
    let held = world
        .politics
        .reserve
        .stock
        .get(&(product, region))
        .copied()
        .unwrap_or(0);
    if held <= 0 {
        return None;
    }
    let (now, _) = band(world, ds, product)?;
    Some((
        (now * 97 / 100).max(1),
        held.min(SELL_LOTS_PER_WEEK * 1_000),
    ))
}

/// Book a sale out of the reserve: the buyer paid `paid` cents for `qty`.
pub fn record_sale(world: &mut World, product: u16, region: u8, qty: MilliLots, paid: i64) {
    let held = world
        .politics
        .reserve
        .stock
        .get(&(product, region))
        .copied()
        .unwrap_or(0);
    if held <= 0 || qty.0 <= 0 {
        return;
    }
    let take = qty.0.min(held);
    let cost = world
        .politics
        .reserve
        .cost
        .get(&(product, region))
        .copied()
        .unwrap_or(0);
    let carrying = (cost as i128 * take as i128 / held as i128) as i64;
    {
        let r = &mut world.politics.reserve;
        let left = held - take;
        if left <= 0 {
            r.stock.remove(&(product, region));
            r.cost.remove(&(product, region));
        } else {
            r.stock.insert((product, region), left);
            r.cost.insert((product, region), cost - carrying);
        }
        r.sold_cents += paid;
        r.sold_milli += take;
    }
    let mut txn = Txn::new()
        .debit(Account::Cash, paid, "reserve sale")
        .credit(Account::Inventory, carrying, "reserve sale");
    if paid > carrying {
        txn = txn.credit(Account::OtherIncome, paid - carrying, "reserve gain");
    } else if carrying > paid {
        txn = txn.debit(Account::CostOfGoods, carrying - paid, "reserve loss");
    }
    let _ = world.government.book.commit(world.tick, txn);
}

/// Weekly purchases in a bust. Sales happen on the exchange (see `release_offer`).
pub fn weekly(world: &mut World, ds: &Dataset) {
    if !world.politics.laws.reserve {
        return;
    }
    let goods: Vec<u16> = strategic_goods(ds)
        .into_iter()
        .filter(|p| phase(world, ds, *p) == Phase::Buying)
        .collect();
    if goods.is_empty() {
        return;
    }
    let mut budget = (world.government.book.cash().max(0) / BUDGET_DIVISOR) / goods.len() as i64;
    for product in goods {
        let held = world.politics.reserve.lots_of(product);
        let mut want = (MAX_STOCK_LOTS - held).clamp(0, BUY_LOTS_PER_WEEK);
        let Some((price, _)) = band(world, ds, product) else {
            continue;
        };
        let price = price.max(1);
        want = want.min(budget / price);
        if want <= 0 {
            continue;
        }
        let handles: Vec<_> = world.firms.iter().map(|(h, _)| h).collect();
        for handle in handles {
            if want <= 0 {
                break;
            }
            let free = crate::economy::pricing::free_stock(world, ds, handle, product);
            if free <= 0 {
                continue;
            }
            let regions: Vec<u8> = world
                .firms
                .get(handle)
                .map(|f| {
                    f.inventory
                        .iter()
                        .filter(|((p, _), lot)| *p == product && lot.qty.0 > 0)
                        .map(|((_, r), _)| *r)
                        .collect()
                })
                .unwrap_or_default();
            let mut free_milli = free * 1_000;
            for region in regions {
                if want <= 0 || free_milli <= 0 {
                    break;
                }
                let have = world
                    .firms
                    .get(handle)
                    .map(|f| f.stock(product, region).0)
                    .unwrap_or(0);
                let qty = MilliLots(have.min(free_milli).min(want * 1_000));
                if qty.0 <= 0 {
                    continue;
                }
                let paid = qty.value_at(Cents(price)).unwrap_or(Cents::ZERO).0;
                if paid <= 0 || paid > world.government.book.cash() {
                    continue;
                }
                let carrying = match world.firms.get_mut(handle) {
                    Some(f) => f.issue_stock(product, region, qty).0,
                    None => continue,
                };
                if let Some(f) = world.firms.get_mut(handle) {
                    let _ = f.book.commit(
                        world.tick,
                        Txn::new()
                            .debit(Account::Cash, paid, "reserve purchase")
                            .credit(Account::Revenue, paid, "reserve purchase")
                            .credit(Account::Inventory, carrying, "cogs")
                            .debit(Account::CostOfGoods, carrying, "cogs"),
                    );
                    f.record_sale(region, product, qty);
                }
                let _ = world.government.book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, paid, "reserve purchase")
                        .debit(Account::Inventory, paid, "reserve purchase"),
                );
                let r = &mut world.politics.reserve;
                *r.stock.entry((product, region)).or_insert(0) += qty.0;
                *r.cost.entry((product, region)).or_insert(0) += paid;
                r.bought_cents += paid;
                r.bought_milli += qty.0;
                budget -= paid;
                want -= qty.lots().max(1);
                free_milli -= qty.0;
            }
        }
    }
}
