//! Plant auctions (2.0): a plant is offered for a set number of weeks and the best bid at the
//! deadline buys it as a running plant, staff and all.
//!
//! Two routes lead here. A failed company's plants go to a break-up auction run for its creditors
//! (`insolvency::liquidate`): the estate keeps them, idle, until the deadline; each goes to the
//! best bid that beats what a scrap buyer pays (40 percent of book, the old forced-sale rate), or
//! is scrapped for that when none does, and the proceeds reach the creditors in their usual
//! order. An owner can also list a plant (`ListPlant`): it keeps running while bids come in, the
//! bids are put to the owner at the deadline, and the owner accepts the best or withdraws.
//!
//! Rivals bid by `disposal::plant_bids`, the rule a plant sale has always used, read at the
//! deadline. The player bids a price through one of their companies (`BidForPlant`); a bid is
//! sealed until the deadline and stands only if the company can still pay then. The player's bid
//! wins a tie, since it was made first. The rival bids are worked out only at the deadline, so
//! the player sees the engine's opening figures for the plant, never the rivals' bids, until then.
//!
//! The same state keeps the merger log (`ProposeMerger`), which a company's merger cooldown reads.
//! The state is a self-contained list outside the postcard payload, in its own save trailer, so
//! no save schema moves (docs/rules/INTERFACE.md).

use cw_core::calendar::Tick;
use cw_core::error::{DomainError, EntityKind};
use cw_core::ids::Handle;
use cw_data::Dataset;
use serde::{Deserialize, Serialize};

use crate::economy::disposal::{self, PlantBid};
use crate::economy::{insolvency, ownership};
use crate::state::{FacilityStatus, FirmStatus, HolderId, World};

/// Share of book value a scrap buyer pays for a plant nobody bids on, in percent: the same rate a
/// liquidation's forced sale of other assets fetches.
pub const SCRAP_PERCENT: i64 = 40;
/// Closed lots kept in the log, newest last.
pub const LOG_LEN: usize = 256;

/// Why a plant is on offer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LotRoute {
    /// A failed company's plant, sold for its creditors.
    BreakUp,
    /// A plant its owner listed for sale.
    Listing,
}

/// The player's sealed bid on a lot, through one of their companies.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedBid {
    pub firm: Handle,
    pub price_cents: i64,
    pub placed: Tick,
}

/// A plant on offer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlantLot {
    pub id: u32,
    pub facility: Handle,
    pub seller: Handle,
    pub route: LotRoute,
    pub opened: Tick,
    /// The deadline: bids are read at the start of this week's settlement.
    pub closes: Tick,
    /// Book value when offered.
    pub book_cents: i64,
    /// What a scrap buyer pays: the least a break-up bid must reach. Zero for a listing.
    pub reserve_cents: i64,
    /// The plant's blocks, recipe and region when offered, so the record stays readable after
    /// the plant changes hands.
    pub blocks: u8,
    pub recipe: u16,
    pub region: u8,
    pub player_bid: Option<SealedBid>,
    /// A listing past its deadline: the bids put to the owner, best first, and the week the offer
    /// lapses. Empty while bidding is open.
    pub offered: Vec<PlantBid>,
    pub answer_by: Tick,
}

/// How a lot ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LotOutcome {
    Sold {
        buyer: Handle,
        price_cents: i64,
        by_player: bool,
    },
    Scrapped {
        proceeds_cents: i64,
    },
    /// The owner withdrew the listing, or let the offer lapse.
    Withdrawn,
    /// The plant left the seller or stopped existing before the deadline.
    Void,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosedLot {
    pub lot: PlantLot,
    pub closed: Tick,
    pub outcome: LotOutcome,
    /// Bids read at the deadline, the player's included.
    pub bids: u32,
}

/// Counts since the career began, for the career statistics.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuctionTotals {
    pub breakup_lots: u32,
    pub breakup_sold: u32,
    pub breakup_scrapped: u32,
    pub listings: u32,
    pub listings_sold: u32,
    pub listings_withdrawn: u32,
    pub sold_to_player: u32,
    pub breakup_proceeds_cents: i64,
    /// What the break-up lots would have fetched at the scrap rate (the rule before 2.0).
    pub breakup_scrap_cents: i64,
    pub listing_proceeds_cents: i64,
    pub mergers: u32,
}

/// A company folded into the one holding 90 percent of it (`ProposeMerger`), with the cash its
/// last minority holders were paid and the shares the player held then.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MergerRecord {
    pub tick: Tick,
    pub buyer: Handle,
    pub target: Handle,
    pub minority_cents: i64,
    pub investor_shares: i64,
}

/// Weeks a company waits after one merger before it proposes another.
pub const MERGER_COOLDOWN_WEEKS: u32 = 52;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuctionState {
    pub next_id: u32,
    pub lots: Vec<PlantLot>,
    pub closed: Vec<ClosedLot>,
    pub totals: AuctionTotals,
    /// Mergers, newest last (the last `LOG_LEN`); `totals.mergers` counts them all.
    pub mergers: Vec<MergerRecord>,
}

impl AuctionState {
    pub fn is_empty(&self) -> bool {
        self.lots.is_empty()
            && self.closed.is_empty()
            && self.next_id == 0
            && self.mergers.is_empty()
    }

    /// Whether `buyer` merged a company in within the cooldown.
    pub fn merged_lately(&self, buyer: Handle, now: Tick) -> bool {
        self.mergers
            .iter()
            .any(|m| m.buyer == buyer && now.0.saturating_sub(m.tick.0) < MERGER_COOLDOWN_WEEKS)
    }

    pub fn record_merger(&mut self, record: MergerRecord) {
        self.totals.mergers += 1;
        self.mergers.push(record);
        let excess = self.mergers.len().saturating_sub(LOG_LEN);
        if excess > 0 {
            self.mergers.drain(..excess);
        }
    }

    pub fn lot(&self, id: u32) -> Option<&PlantLot> {
        self.lots.iter().find(|lot| lot.id == id)
    }

    /// The open lot for a plant, if any.
    pub fn for_facility(&self, facility: Handle) -> Option<&PlantLot> {
        self.lots.iter().find(|lot| lot.facility == facility)
    }
}

/// Weeks a lot stays open; zero turns auctions off.
pub fn auction_weeks(dataset: &Dataset) -> u32 {
    dataset.capacity.plant_auction_weeks
}

fn open_lot(world: &mut World, dataset: &Dataset, facility: Handle, route: LotRoute) -> u32 {
    let plant = world.facilities.get(facility).expect("offerable plant");
    let book = (plant.capex_cents - plant.accumulated_depreciation).max(0);
    let id = world.auctions.next_id;
    world.auctions.next_id += 1;
    let lot = PlantLot {
        id,
        facility,
        seller: plant.firm,
        route,
        opened: world.tick,
        closes: Tick(world.tick.0 + auction_weeks(dataset)),
        book_cents: book,
        reserve_cents: match route {
            LotRoute::BreakUp => book * SCRAP_PERCENT / 100,
            LotRoute::Listing => 0,
        },
        blocks: plant.blocks,
        recipe: plant.recipe,
        region: plant.region,
        player_bid: None,
        offered: Vec::new(),
        answer_by: Tick(0),
    };
    match route {
        LotRoute::BreakUp => world.auctions.totals.breakup_lots += 1,
        LotRoute::Listing => world.auctions.totals.listings += 1,
    }
    world.auctions.lots.push(lot);
    id
}

/// The plants of a failing company that go to its break-up auction: every one that could run.
/// Empty when auctions are off.
pub fn breakup_plants(world: &World, dataset: &Dataset, firm: Handle) -> Vec<Handle> {
    if auction_weeks(dataset) == 0 {
        return Vec::new();
    }
    let mut plants: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(handle, plant)| plant.firm == firm && disposal::offerable(world, *handle))
        .map(|(handle, _)| handle)
        .collect();
    plants.sort();
    plants
}

/// Put a failed company's plants up for auction. A listing already open for one of them becomes
/// part of the break-up: the owner can no longer withdraw it.
pub fn open_breakup(world: &mut World, dataset: &Dataset, plants: &[Handle]) {
    for plant in plants {
        world.auctions.lots.retain(|lot| lot.facility != *plant);
        open_lot(world, dataset, *plant, LotRoute::BreakUp);
    }
}

/// Whether `firm` still has plants in a break-up auction.
pub fn in_breakup(world: &World, firm: Handle) -> bool {
    world
        .auctions
        .lots
        .iter()
        .any(|lot| lot.seller == firm && lot.route == LotRoute::BreakUp)
}

/// `ListPlant`: the owner offers a running or mothballed plant for `plant_auction_weeks`.
pub fn list(world: &mut World, dataset: &Dataset, facility: Handle) -> Result<u32, DomainError> {
    if auction_weeks(dataset) == 0 {
        return Err(DomainError::WrongPhase {
            expected: "plant_auctions_on",
            actual: "off",
        });
    }
    if !disposal::offerable(world, facility) {
        return Err(DomainError::WrongPhase {
            expected: "running_or_mothballed_plant",
            actual: "not_offerable",
        });
    }
    if world.auctions.for_facility(facility).is_some() {
        return Err(DomainError::WrongPhase {
            expected: "not_on_offer",
            actual: "on_offer",
        });
    }
    let seller = world.facilities.get(facility).map(|p| p.firm);
    if !seller.is_some_and(|f| world.firm(f).is_some_and(|e| e.operative())) {
        return Err(DomainError::WrongPhase {
            expected: "trading_owner",
            actual: "owner_not_trading",
        });
    }
    Ok(open_lot(world, dataset, facility, LotRoute::Listing))
}

fn lot_index(world: &World, id: u32) -> Result<usize, DomainError> {
    world
        .auctions
        .lots
        .iter()
        .position(|lot| lot.id == id)
        .ok_or(DomainError::InvalidEntity {
            handle: id as u64,
            kind: EntityKind::Facility,
        })
}

/// `BidForPlant`: the player's sealed bid through `firm`, replacing any earlier one. A price of
/// zero withdraws the bid. The company must be on the player's side, trading, outside the
/// seller's group and able to pay today; it must still be able to pay at the deadline.
pub fn bid(world: &mut World, firm: Handle, id: u32, price_cents: i64) -> Result<(), DomainError> {
    let index = lot_index(world, id)?;
    let lot = &world.auctions.lots[index];
    if !lot.offered.is_empty() || world.tick.0 >= lot.closes.0 {
        return Err(DomainError::WrongPhase {
            expected: "bidding_open",
            actual: "closed",
        });
    }
    if price_cents < 0 {
        return Err(DomainError::WrongPhase {
            expected: "price_not_negative",
            actual: "negative",
        });
    }
    if price_cents == 0 {
        world.auctions.lots[index].player_bid = None;
        return Ok(());
    }
    let entity = world.firm(firm).ok_or(DomainError::InvalidEntity {
        handle: firm.0,
        kind: EntityKind::Firm,
    })?;
    if !entity.operative() {
        return Err(DomainError::WrongPhase {
            expected: "trading_bidder",
            actual: "bidder_not_trading",
        });
    }
    if firm == lot.seller || ownership::same_group(world, firm, lot.seller) {
        return Err(DomainError::WrongPhase {
            expected: "outside_seller_group",
            actual: "seller_group",
        });
    }
    if entity.book.cash() < price_cents {
        return Err(DomainError::WrongPhase {
            expected: "cash_covers_bid",
            actual: "short_of_cash",
        });
    }
    let placed = world.tick;
    world.auctions.lots[index].player_bid = Some(SealedBid {
        firm,
        price_cents,
        placed,
    });
    Ok(())
}

/// `AcceptPlantBid`: the owner sells a listed plant to the best bid put to it that can still pay.
pub fn accept(world: &mut World, id: u32) -> Result<(), DomainError> {
    let index = lot_index(world, id)?;
    let lot = world.auctions.lots[index].clone();
    if lot.offered.is_empty() {
        return Err(DomainError::WrongPhase {
            expected: "bids_put_to_owner",
            actual: "no_offer",
        });
    }
    let buyer = lot.offered.iter().find(|bid| can_pay(world, bid)).cloned();
    let Some(buyer) = buyer else {
        return Err(DomainError::WrongPhase {
            expected: "bidder_can_pay",
            actual: "no_bidder_can_pay",
        });
    };
    world.auctions.lots.remove(index);
    let by_player = is_player_bid(world, &lot, buyer.bidder);
    disposal::transfer_plant(world, lot.facility, buyer.bidder, buyer.price_cents)?;
    let totals = &mut world.auctions.totals;
    totals.listings_sold += 1;
    totals.listing_proceeds_cents += buyer.price_cents;
    if by_player {
        totals.sold_to_player += 1;
    }
    let bids = lot.offered.len() as u32;
    record(
        world,
        lot,
        LotOutcome::Sold {
            buyer: buyer.bidder,
            price_cents: buyer.price_cents,
            by_player,
        },
        bids,
    );
    Ok(())
}

/// `WithdrawListing`: the owner takes a listed plant off the market, before or after the bids.
pub fn withdraw(world: &mut World, id: u32) -> Result<(), DomainError> {
    let index = lot_index(world, id)?;
    if world.auctions.lots[index].route != LotRoute::Listing {
        return Err(DomainError::WrongPhase {
            expected: "listing",
            actual: "break_up",
        });
    }
    let lot = world.auctions.lots.remove(index);
    world.auctions.totals.listings_withdrawn += 1;
    let bids = lot.offered.len() as u32;
    record(world, lot, LotOutcome::Withdrawn, bids);
    Ok(())
}

fn is_player_bid(world: &World, lot: &PlantLot, buyer: Handle) -> bool {
    lot.player_bid.as_ref().is_some_and(|b| b.firm == buyer)
        || ownership::ultimate_controller(world, buyer) == Some(HolderId::Investor)
}

fn can_pay(world: &World, bid: &PlantBid) -> bool {
    world
        .firm(bid.bidder)
        .is_some_and(|f| f.operative() && f.book.cash() >= bid.price_cents)
}

fn record(world: &mut World, lot: PlantLot, outcome: LotOutcome, bids: u32) {
    world.auctions.closed.push(ClosedLot {
        lot,
        closed: world.tick,
        outcome,
        bids,
    });
    let excess = world.auctions.closed.len().saturating_sub(LOG_LEN);
    if excess > 0 {
        world.auctions.closed.drain(..excess);
    }
}

/// Every bid at the deadline, best first: the rivals' by the plant sale rule and the player's
/// sealed bid if its company can still pay. The player's bid ranks ahead of an equal rival bid.
pub fn bids_at_deadline(world: &World, dataset: &Dataset, lot: &PlantLot) -> Vec<PlantBid> {
    let mut bids = disposal::plant_bids(world, dataset, lot.facility);
    if let Some(sealed) = &lot.player_bid {
        let bid = PlantBid {
            bidder: sealed.firm,
            price_cents: sealed.price_cents,
            weekly_cents: 0,
            blocks_used: 0,
        };
        if can_pay(world, &bid) && !ownership::same_group(world, sealed.firm, lot.seller) {
            let at = bids
                .iter()
                .position(|b| b.price_cents <= bid.price_cents)
                .unwrap_or(bids.len());
            bids.insert(at, bid);
        }
    }
    bids
}

/// What a scrap buyer can pay for a break-up lot today: the reserve, as far as outside capital
/// goes.
fn scrap_available(world: &World, reserve: i64) -> i64 {
    let capital: i64 = world
        .outside
        .investors
        .iter()
        .map(|buyer| buyer.capital_cents.max(0))
        .fold(0i64, i64::saturating_add);
    reserve.min(capital).max(0)
}

fn take_scrap_cash(world: &mut World, amount: i64) -> i64 {
    let mut taken = 0i64;
    for buyer in world.outside.investors.iter_mut() {
        if taken >= amount {
            break;
        }
        let take = (amount - taken).min(buyer.capital_cents.max(0));
        buyer.capital_cents -= take;
        taken += take;
    }
    taken
}

/// Weekly: settle lots at their deadline. Break-up lots sell to the best bid above scrap value or
/// are scrapped, and a failed company whose last lot closed is wound up; listings put their bids
/// to the owner, and an offer left unanswered lapses. Returns the lots closed this week.
pub fn weekly(world: &mut World, dataset: &Dataset) -> Vec<u32> {
    let now = world.tick;
    let mut closed = Vec::new();
    let due: Vec<u32> = world
        .auctions
        .lots
        .iter()
        .filter(|lot| {
            (lot.offered.is_empty() && now.0 >= lot.closes.0)
                || (!lot.offered.is_empty() && now.0 >= lot.answer_by.0)
        })
        .map(|lot| lot.id)
        .collect();
    let mut wound_up: Vec<Handle> = Vec::new();
    for id in due {
        let Ok(index) = lot_index(world, id) else {
            continue;
        };
        let lot = world.auctions.lots[index].clone();
        let still_theirs = world
            .facilities
            .get(lot.facility)
            .is_some_and(|plant| plant.firm == lot.seller);
        if !still_theirs {
            world.auctions.lots.remove(index);
            record(world, lot, LotOutcome::Void, 0);
            closed.push(id);
            continue;
        }
        match lot.route {
            LotRoute::BreakUp => {
                let bids = bids_at_deadline(world, dataset, &lot);
                let scrap = scrap_available(world, lot.reserve_cents);
                world.auctions.lots.remove(index);
                let count = bids.len() as u32;
                let winner = bids
                    .into_iter()
                    .find(|bid| bid.price_cents >= scrap && bid.price_cents > 0);
                world.auctions.totals.breakup_scrap_cents += lot.reserve_cents;
                let outcome = match winner {
                    Some(bid) => {
                        let by_player = is_player_bid(world, &lot, bid.bidder);
                        let _ = disposal::transfer_plant(
                            world,
                            lot.facility,
                            bid.bidder,
                            bid.price_cents,
                        );
                        let totals = &mut world.auctions.totals;
                        totals.breakup_sold += 1;
                        totals.breakup_proceeds_cents += bid.price_cents;
                        if by_player {
                            totals.sold_to_player += 1;
                        }
                        LotOutcome::Sold {
                            buyer: bid.bidder,
                            price_cents: bid.price_cents,
                            by_player,
                        }
                    }
                    None => {
                        let proceeds = take_scrap_cash(world, scrap);
                        let _ = disposal::scrap_plant(world, lot.facility, proceeds);
                        let totals = &mut world.auctions.totals;
                        totals.breakup_scrapped += 1;
                        totals.breakup_proceeds_cents += proceeds;
                        LotOutcome::Scrapped {
                            proceeds_cents: proceeds,
                        }
                    }
                };
                if !wound_up.contains(&lot.seller) {
                    wound_up.push(lot.seller);
                }
                record(world, lot, outcome, count);
            }
            LotRoute::Listing if lot.offered.is_empty() => {
                let bids = bids_at_deadline(world, dataset, &lot);
                if bids.is_empty() {
                    world.auctions.lots.remove(index);
                    world.auctions.totals.listings_withdrawn += 1;
                    record(world, lot, LotOutcome::Withdrawn, 0);
                } else {
                    let stored = &mut world.auctions.lots[index];
                    stored.offered = bids;
                    stored.answer_by = Tick(now.0 + auction_weeks(dataset).max(1));
                }
            }
            LotRoute::Listing => {
                // The owner let the offer lapse.
                world.auctions.lots.remove(index);
                world.auctions.totals.listings_withdrawn += 1;
                let count = lot.offered.len() as u32;
                record(world, lot, LotOutcome::Withdrawn, count);
            }
        }
        closed.push(id);
    }
    for firm in wound_up {
        if !in_breakup(world, firm)
            && world
                .firm(firm)
                .is_some_and(|f| f.status == FirmStatus::Liquidating)
        {
            insolvency::finish_liquidation(world, firm, dataset);
        }
    }
    // A plant that stopped running while listed (building over it, a retool) leaves the market.
    let stale: Vec<u32> = world
        .auctions
        .lots
        .iter()
        .filter(|lot| {
            lot.route == LotRoute::Listing
                && !world.facilities.get(lot.facility).is_some_and(|p| {
                    p.firm == lot.seller
                        && matches!(
                            p.status,
                            FacilityStatus::Operating | FacilityStatus::Mothballed
                        )
                })
        })
        .map(|lot| lot.id)
        .collect();
    for id in stale {
        if let Ok(index) = lot_index(world, id) {
            let lot = world.auctions.lots.remove(index);
            record(world, lot, LotOutcome::Void, 0);
        }
    }
    closed
}
