//! Plant sales (2.0): a plant leaves its owner only when someone pays for it.
//!
//! Selling a plant offers it to every rival outside the seller's group that could run it at a
//! profit and can pay. A bidder values the plant on the same room-to-build signal the sector board
//! shows (`capacity::opening_with` from the bidder's head office), and bids the least of three
//! sums: what the blocks it could keep busy earn over the payback horizon it judges a new plant
//! on, what building the same blocks would cost it today (scaled by the share of the plant's
//! cost not yet written off), and its cash above an eight-week reserve. The highest bid buys and
//! the plant changes hands as a running plant, staff and all. When nobody bids, the plant is
//! written off with no cash: the engine has no outside buyer for scrap.
//!
//! The player's companies never bid automatically: buying a plant is a choice the player makes.

use cw_core::error::{DomainError, EntityKind};
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;

use crate::economy::{capacity, insolvency, ownership, sectors};
use crate::state::{FacilityStatus, HolderId, World};

/// Weeks of a bidder's mandatory outflow it keeps back before bidding.
pub const BID_RESERVE_WEEKS: i64 = 8;

/// One firm's offer for a plant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlantBid {
    pub bidder: Handle,
    pub price_cents: i64,
    /// The bidder's weekly contribution for the blocks it could keep busy, at public prices.
    pub weekly_cents: i64,
    /// Blocks of the plant the bidder's market reading says it could keep busy.
    pub blocks_used: i64,
}

/// What became of a plant put up for sale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlantSale {
    Sold { buyer: Handle, price_cents: i64 },
    WrittenOff { book_cents: i64 },
}

/// Every bid for `facility`, best first (highest price, then lowest handle). Empty when the plant
/// is not running or mothballed, or when no firm could use it and pay.
pub fn plant_bids(world: &World, dataset: &Dataset, facility: Handle) -> Vec<PlantBid> {
    let Some(plant) = world.facilities.get(facility) else {
        return Vec::new();
    };
    if !matches!(
        plant.status,
        FacilityStatus::Operating | FacilityStatus::Mothballed
    ) {
        return Vec::new();
    }
    let seller = plant.firm;
    let blocks = plant.blocks as i64;
    let capex = plant.capex_cents.max(0);
    let book = (plant.capex_cents - plant.accumulated_depreciation).max(0);
    if blocks <= 0 {
        return Vec::new();
    }
    let template = &dataset.recipes[plant.recipe as usize];
    let new_blocks = sectors::build_price(world, dataset, template.capex_cents) * blocks;
    let replacement = if capex > 0 {
        (new_blocks as i128 * book as i128 / capex as i128) as i64
    } else {
        0
    };
    let rooms = capacity::input_rooms(world, dataset);
    let selling_lots = plant.last_output.lots().max(0);
    let mut bids: Vec<PlantBid> = world
        .firms
        .iter()
        .filter(|(handle, firm)| {
            *handle != seller
                && firm.operative()
                && !ownership::same_group(world, *handle, seller)
                && ownership::ultimate_controller(world, *handle) != Some(HolderId::Investor)
        })
        .filter_map(|(handle, firm)| {
            let opening = capacity::opening_with(
                world,
                dataset,
                &rooms,
                firm.region,
                plant.recipe,
                plant.region,
            )?;
            if opening.weekly_contribution_cents <= 0 {
                return None;
            }
            // What the market would take from this plant: the room a new plant would fill plus
            // what the plant makes today, in whole blocks.
            let used =
                ((opening.room_lots.max(0) + selling_lots) / opening.block_lots.max(1)).min(blocks);
            if used <= 0 {
                return None;
            }
            let weekly = opening.weekly_contribution_cents * used;
            let horizon = capacity::horizon_weeks(dataset, firm.facilities.len() as i64 + 1);
            let earnings = weekly.saturating_mul(horizon);
            let spare = firm.book.cash()
                - insolvency::mandatory_weekly_outflow(world, handle)
                    .saturating_mul(BID_RESERVE_WEEKS);
            let price = earnings.min(replacement).min(spare);
            (price > 0).then_some(PlantBid {
                bidder: handle,
                price_cents: price,
                weekly_cents: weekly,
                blocks_used: used,
            })
        })
        .collect();
    bids.sort_by(|a, b| {
        b.price_cents
            .cmp(&a.price_cents)
            .then(a.bidder.cmp(&b.bidder))
    });
    bids
}

/// Sell `facility` to its best bidder, or write it off when there is none. The seller gives up the
/// plant's cost and depreciation and books the difference between the price and the book value;
/// the buyer pays cash and carries the plant at the price it paid.
pub fn sell_plant(
    world: &mut World,
    dataset: &Dataset,
    facility: Handle,
) -> Result<PlantSale, DomainError> {
    let (seller, capex, depreciation) = world
        .facilities
        .get(facility)
        .map(|plant| {
            (
                plant.firm,
                plant.capex_cents,
                plant.accumulated_depreciation,
            )
        })
        .ok_or(DomainError::InvalidEntity {
            handle: facility.0,
            kind: EntityKind::Facility,
        })?;
    let book = (capex - depreciation).max(0);
    let best = plant_bids(world, dataset, facility).into_iter().next();
    let Some(bid) = best else {
        world.facilities.remove(facility);
        if let Some(entity) = world.firms.get_mut(seller) {
            entity.facilities.retain(|handle| *handle != facility);
            let _ = entity.book.commit(
                world.tick,
                Txn::new()
                    .debit(
                        Account::AccumulatedDepreciation,
                        depreciation,
                        "plant written off",
                    )
                    .debit(Account::OperatingExpense, book, "plant written off")
                    .credit(Account::FixedAssets, capex, "plant written off"),
            );
        }
        return Ok(PlantSale::WrittenOff { book_cents: book });
    };
    let price = bid.price_cents;
    if let Some(entity) = world.firms.get_mut(seller) {
        entity.facilities.retain(|handle| *handle != facility);
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Cash, price, "plant sale")
                .debit(Account::AccumulatedDepreciation, depreciation, "plant sale")
                .credit(Account::FixedAssets, capex, "plant sale")
                .signed(Account::OtherIncome, book - price, "plant sale"),
        );
    }
    if let Some(entity) = world.firms.get_mut(bid.bidder) {
        if !entity.facilities.contains(&facility) {
            entity.facilities.push(facility);
        }
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, price, "plant purchase")
                .debit(Account::FixedAssets, price, "plant purchase"),
        );
    }
    if let Some(plant) = world.facilities.get_mut(facility) {
        plant.firm = bid.bidder;
        plant.capex_cents = price;
        plant.accumulated_depreciation = 0;
    }
    Ok(PlantSale::Sold {
        buyer: bid.bidder,
        price_cents: price,
    })
}
