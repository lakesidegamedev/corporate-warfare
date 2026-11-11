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

use crate::economy::capacity::InputRooms;
use crate::economy::{capacity, insolvency, ownership, sectors};
use crate::state::{FacilityStatus, HolderId, World};

/// Weeks of a bidder's mandatory outflow it keeps back before bidding.
pub const BID_RESERVE_WEEKS: i64 = 8;

/// One firm's offer for a plant.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

/// How one firm reads a plant on offer: what the blocks it could keep busy earn, what building
/// them would cost it today, and the cash it could spare. `price_cents` is the least of the three
/// (`spare` may make it zero or less, which is no bid).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Appraisal {
    pub weekly_cents: i64,
    pub blocks_used: i64,
    pub earnings_cents: i64,
    pub replacement_cents: i64,
    pub spare_cents: i64,
    pub price_cents: i64,
}

/// Whether a plant can be offered: running or mothballed, with blocks.
pub fn offerable(world: &World, facility: Handle) -> bool {
    world.facilities.get(facility).is_some_and(|plant| {
        plant.blocks > 0
            && matches!(
                plant.status,
                FacilityStatus::Operating | FacilityStatus::Mothballed
            )
    })
}

/// How `bidder` reads `facility`, from the bidder's head office, whoever owns either. `None` when
/// the plant cannot be offered or the bidder could keep none of it busy at a profit.
pub fn appraise(
    world: &World,
    dataset: &Dataset,
    rooms: &InputRooms,
    bidder: Handle,
    facility: Handle,
) -> Option<Appraisal> {
    if !offerable(world, facility) {
        return None;
    }
    let plant = world.facilities.get(facility)?;
    let firm = world.firms.get(bidder)?;
    let blocks = plant.blocks as i64;
    let capex = plant.capex_cents.max(0);
    let book = (plant.capex_cents - plant.accumulated_depreciation).max(0);
    let template = dataset.recipes.get(plant.recipe as usize)?;
    let new_blocks = sectors::build_price(world, dataset, template.capex_cents) * blocks;
    let replacement = if capex > 0 {
        (new_blocks as i128 * book as i128 / capex as i128) as i64
    } else {
        0
    };
    let opening = capacity::opening_with(
        world,
        dataset,
        rooms,
        firm.region,
        plant.recipe,
        plant.region,
    )?;
    if opening.weekly_contribution_cents <= 0 {
        return None;
    }
    // What the market would take from this plant: the room a new plant would fill plus what the
    // plant makes today, in whole blocks.
    let selling_lots = plant.last_output.lots().max(0);
    let used = ((opening.room_lots.max(0) + selling_lots) / opening.block_lots.max(1)).min(blocks);
    if used <= 0 {
        return None;
    }
    let weekly = opening.weekly_contribution_cents * used;
    let horizon = capacity::horizon_weeks(dataset, firm.facilities.len() as i64 + 1);
    let earnings = weekly.saturating_mul(horizon);
    let spare = firm.book.cash()
        - insolvency::mandatory_weekly_outflow(world, bidder).saturating_mul(BID_RESERVE_WEEKS);
    Some(Appraisal {
        weekly_cents: weekly,
        blocks_used: used,
        earnings_cents: earnings,
        replacement_cents: replacement,
        spare_cents: spare,
        price_cents: earnings.min(replacement).min(spare),
    })
}

/// Every bid for `facility`, best first (highest price, then lowest handle). Empty when the plant
/// is not running or mothballed, or when no firm could use it and pay.
pub fn plant_bids(world: &World, dataset: &Dataset, facility: Handle) -> Vec<PlantBid> {
    let Some(plant) = world.facilities.get(facility) else {
        return Vec::new();
    };
    if !offerable(world, facility) {
        return Vec::new();
    }
    let seller = plant.firm;
    let rooms = capacity::input_rooms(world, dataset);
    let mut bids: Vec<PlantBid> = world
        .firms
        .iter()
        .filter(|(handle, firm)| {
            *handle != seller
                && firm.operative()
                && !ownership::same_group(world, *handle, seller)
                && ownership::ultimate_controller(world, *handle) != Some(HolderId::Investor)
        })
        .filter_map(|(handle, _)| {
            let view = appraise(world, dataset, &rooms, handle, facility)?;
            (view.price_cents > 0).then_some(PlantBid {
                bidder: handle,
                price_cents: view.price_cents,
                weekly_cents: view.weekly_cents,
                blocks_used: view.blocks_used,
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

fn plant_books(world: &World, facility: Handle) -> Result<(Handle, i64, i64), DomainError> {
    world
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
        })
}

/// Sell `facility` to its best bidder, or write it off when there is none. The seller gives up the
/// plant's cost and depreciation and books the difference between the price and the book value;
/// the buyer pays cash and carries the plant at the price it paid.
pub fn sell_plant(
    world: &mut World,
    dataset: &Dataset,
    facility: Handle,
) -> Result<PlantSale, DomainError> {
    let (_, capex, depreciation) = plant_books(world, facility)?;
    let book = (capex - depreciation).max(0);
    let best = plant_bids(world, dataset, facility).into_iter().next();
    let Some(bid) = best else {
        scrap_plant(world, facility, 0)?;
        return Ok(PlantSale::WrittenOff { book_cents: book });
    };
    transfer_plant(world, facility, bid.bidder, bid.price_cents)?;
    Ok(PlantSale::Sold {
        buyer: bid.bidder,
        price_cents: bid.price_cents,
    })
}

/// The plant changes hands as a running plant, staff and all, for `price` paid in cash by `buyer`.
/// The seller gives up the plant's cost and depreciation and books the difference between the
/// price and the book value; the buyer carries the plant at the price with depreciation reset.
pub fn transfer_plant(
    world: &mut World,
    facility: Handle,
    buyer: Handle,
    price: i64,
) -> Result<(), DomainError> {
    let (seller, capex, depreciation) = plant_books(world, facility)?;
    let book = (capex - depreciation).max(0);
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
    if let Some(entity) = world.firms.get_mut(buyer) {
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
        plant.firm = buyer;
        plant.capex_cents = price;
        plant.accumulated_depreciation = 0;
    }
    Ok(())
}

/// The plant is broken up: `proceeds` (cash an outside scrap buyer already paid out of its own
/// capital, or zero) reach the owner, the cost and depreciation leave its books with the loss
/// against book, and the plant is gone with its staff.
pub fn scrap_plant(world: &mut World, facility: Handle, proceeds: i64) -> Result<(), DomainError> {
    let (seller, capex, depreciation) = plant_books(world, facility)?;
    let book = (capex - depreciation).max(0);
    world.facilities.remove(facility);
    if let Some(entity) = world.firms.get_mut(seller) {
        entity.facilities.retain(|handle| *handle != facility);
        let memo = if proceeds > 0 {
            "plant scrapped"
        } else {
            "plant written off"
        };
        let mut txn = Txn::new()
            .debit(Account::AccumulatedDepreciation, depreciation, memo)
            .credit(Account::FixedAssets, capex, memo);
        if proceeds > 0 {
            txn = txn.debit(Account::Cash, proceeds, memo).signed(
                Account::OtherIncome,
                book - proceeds,
                memo,
            );
        } else {
            txn = txn.debit(Account::OperatingExpense, book, memo);
        }
        let _ = entity.book.commit(world.tick, txn);
    }
    Ok(())
}
