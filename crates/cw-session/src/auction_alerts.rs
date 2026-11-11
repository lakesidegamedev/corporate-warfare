//! Plant auction alerts (2.0), read from the lots on offer and never stored: a plant the player
//! could bid for, with the engine's opening figures and the deadline; and bids put to the player
//! for a plant they listed, to accept or refuse.

use cw_data::Dataset;
use cw_sim::state::World;

use crate::{Alert, paper};

fn weeks(n: u32) -> String {
    format!("{n} week{}", if n == 1 { "" } else { "s" })
}

pub fn auction_alerts(world: &World, dataset: &Dataset, alerts: &mut Vec<Alert>) {
    for view in crate::plant_lots(world, dataset) {
        let seller = cw_core::ids::Handle(view.seller);
        let money = |cents: i64| paper::money(world, dataset, seller, cents);
        let plant = format!(
            "{} plant in {} ({} block{})",
            view.plant_name,
            view.region_name,
            view.blocks,
            if view.blocks == 1 { "" } else { "s" }
        );
        if view.yours {
            if let Some(best) = view.bids.first() {
                alerts.push(Alert {
                    key: "alert.plant_bids_in".to_owned(),
                    subject: view.seller_name.clone(),
                    severity: 2,
                    detail: format!(
                        "{} bid{} for {}'s {plant}. Best: {} from {} (book value {}). Accept the best with AcceptPlantBid on lot {}, or keep the plant with WithdrawListing, by week {}.",
                        view.bids.len(),
                        if view.bids.len() == 1 { "" } else { "s" },
                        view.seller_name,
                        money(best.price_cents),
                        best.bidder_name,
                        money(view.book_cents),
                        view.lot,
                        view.answer_by_week,
                    ),
                });
            }
            continue;
        }
        if !view.bids.is_empty() {
            // A rival's listing with its bids in: the bidding is over.
            continue;
        }
        let (key, opener) = if view.route == "break_up" {
            (
                "alert.plant_auction",
                format!(
                    "{} is in liquidation and its {plant} is up for auction for its creditors.",
                    view.seller_name
                ),
            )
        } else {
            (
                "alert.plant_listed",
                format!("{} has put its {plant} up for sale.", view.seller_name),
            )
        };
        let reading = if view.blocks_used > 0 {
            format!(
                " Your company reads it as earning {} a week on {} block{} ({} over its payback, {} to build the same today).",
                money(view.weekly_cents),
                view.blocks_used,
                if view.blocks_used == 1 { "" } else { "s" },
                money(view.earnings_cents),
                money(view.replacement_cents),
            )
        } else {
            " Your company sees no room to sell what it makes at today's prices.".to_owned()
        };
        let floor = if view.reserve_cents > 0 {
            format!(
                " A scrap buyer pays {}, so a bid must beat that.",
                money(view.reserve_cents)
            )
        } else {
            String::new()
        };
        let yours = if view.player_bid_cents > 0 {
            format!(" Your sealed bid: {}.", money(view.player_bid_cents))
        } else {
            format!(" Bid a price with BidForPlant on lot {}.", view.lot)
        };
        alerts.push(Alert {
            key: key.to_owned(),
            subject: view.seller_name.clone(),
            severity: 1,
            detail: format!(
                "{opener} Book value {}.{floor}{reading} Bids are sealed; the best at the deadline buys it, running, staff and all. Bidding closes in {}.{yours}",
                money(view.book_cents),
                weeks(view.weeks_left),
            ),
        });
    }
}
