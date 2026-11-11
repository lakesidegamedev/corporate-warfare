//! Plant sales (2.0): a plant leaves its owner only when another company pays for it. The cash
//! the seller receives is exactly what the buyer pays, the plant keeps running under its new
//! owner, the player's companies never bid on their own, and a plant nobody wants is written off
//! with no cash.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::economy::disposal::{self, PlantSale};
use cw_sim::economy::{ownership, seed};
use cw_sim::state::{FacilityStatus, HolderId, ScenarioKind, ScenarioMode, World};

fn fund(world: &mut World, firm: Handle, cents: i64) {
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, cents, "test funding")
            .credit(Account::ShareCapital, cents, "test funding"),
    );
}

fn cash(world: &World) -> i64 {
    world.firms.iter().map(|(_, f)| f.book.cash()).sum()
}

fn world() -> (World, Dataset) {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 902, ScenarioKind::Founder, ScenarioMode::Sandbox);
    // A few weeks of trading, so plants have output and prices have formed.
    for _ in 0..8 {
        cw_sim::tick::advance(&mut world, &dataset);
    }
    (world, dataset)
}

/// The first running plant of a rival that draws a bid once every rival has ample cash.
fn plant_with_bids(world: &mut World, dataset: &Dataset) -> Handle {
    let rivals: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(h, _)| ownership::ultimate_controller(world, *h) != Some(HolderId::Investor))
        .map(|(h, _)| h)
        .collect();
    for firm in rivals {
        fund(world, firm, 50_000_000_000);
    }
    let plants: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(_, p)| p.status == FacilityStatus::Operating)
        .map(|(h, _)| h)
        .collect();
    plants
        .into_iter()
        .find(|plant| !disposal::plant_bids(world, dataset, *plant).is_empty())
        .expect("some plant draws a bid")
}

#[test]
fn a_sold_plant_changes_hands_running_and_the_money_adds_up() {
    let (mut world, dataset) = world();
    let plant = plant_with_bids(&mut world, &dataset);
    let seller = world.facilities.get(plant).unwrap().firm;
    let bids = disposal::plant_bids(&world, &dataset, plant);
    // Best first, and nobody in the seller's group or the player's bids.
    for pair in bids.windows(2) {
        assert!(pair[0].price_cents >= pair[1].price_cents);
    }
    for bid in &bids {
        assert!(bid.price_cents > 0 && bid.blocks_used > 0);
        assert!(!ownership::same_group(&world, bid.bidder, seller));
        assert_ne!(
            ownership::ultimate_controller(&world, bid.bidder),
            Some(HolderId::Investor)
        );
    }
    let best = bids[0].clone();
    let (capex, depreciation, blocks) = {
        let p = world.facilities.get(plant).unwrap();
        (p.capex_cents, p.accumulated_depreciation, p.blocks)
    };
    // The bid never exceeds what building the blocks would cost today, scaled to the share of the
    // plant's cost not yet written off.
    let template = &dataset.recipes[world.facilities.get(plant).unwrap().recipe as usize];
    let new_cost = cw_sim::economy::sectors::build_price(&world, &dataset, template.capex_cents)
        * blocks as i64;
    assert!(best.price_cents <= new_cost * (capex - depreciation) / capex + 1);

    let seller_cash = world.firm(seller).unwrap().book.cash();
    let buyer_cash = world.firm(best.bidder).unwrap().book.cash();
    let seller_fixed = world
        .firm(seller)
        .unwrap()
        .book
        .balance(Account::FixedAssets);
    let total = cash(&world);

    let sale = disposal::sell_plant(&mut world, &dataset, plant).unwrap();
    assert_eq!(
        sale,
        PlantSale::Sold {
            buyer: best.bidder,
            price_cents: best.price_cents
        }
    );
    // Cash moved from buyer to seller and nowhere else.
    assert_eq!(cash(&world), total);
    assert_eq!(
        world.firm(seller).unwrap().book.cash(),
        seller_cash + best.price_cents
    );
    assert_eq!(
        world.firm(best.bidder).unwrap().book.cash(),
        buyer_cash - best.price_cents
    );
    // The seller gives up the plant's whole cost; the buyer carries it at the price paid.
    assert_eq!(
        world
            .firm(seller)
            .unwrap()
            .book
            .balance(Account::FixedAssets),
        seller_fixed - capex
    );
    let moved = world.facilities.get(plant).unwrap();
    assert_eq!(moved.firm, best.bidder);
    assert_eq!(moved.status, FacilityStatus::Operating);
    assert_eq!(moved.blocks, blocks);
    assert_eq!(moved.capex_cents, best.price_cents);
    assert_eq!(moved.accumulated_depreciation, 0);
    assert!(world.firm(best.bidder).unwrap().facilities.contains(&plant));
    assert!(!world.firm(seller).unwrap().facilities.contains(&plant));
}

#[test]
fn a_plant_nobody_bids_for_is_written_off_for_nothing() {
    let (mut world, dataset) = world();
    // Every other company is left with no cash, so nobody can bid.
    let plant = world
        .facilities
        .iter()
        .find(|(_, p)| p.status == FacilityStatus::Operating)
        .map(|(h, _)| h)
        .unwrap();
    let seller = world.facilities.get(plant).unwrap().firm;
    let others: Vec<(Handle, i64)> = world
        .firms
        .iter()
        .filter(|(h, _)| *h != seller)
        .map(|(h, f)| (h, f.book.cash()))
        .collect();
    for (firm, held) in others {
        if held > 0 {
            let _ = world.firms.get_mut(firm).unwrap().book.commit(
                world.tick,
                Txn::new().credit(Account::Cash, held, "test drain").debit(
                    Account::ShareCapital,
                    held,
                    "test drain",
                ),
            );
        }
    }
    assert!(disposal::plant_bids(&world, &dataset, plant).is_empty());
    let (capex, depreciation) = {
        let p = world.facilities.get(plant).unwrap();
        (p.capex_cents, p.accumulated_depreciation)
    };
    let total = cash(&world);
    let seller_cash = world.firm(seller).unwrap().book.cash();
    let sale = disposal::sell_plant(&mut world, &dataset, plant).unwrap();
    assert_eq!(
        sale,
        PlantSale::WrittenOff {
            book_cents: capex - depreciation
        }
    );
    assert_eq!(cash(&world), total);
    assert_eq!(world.firm(seller).unwrap().book.cash(), seller_cash);
    assert!(world.facilities.get(plant).is_none());
    assert!(!world.firm(seller).unwrap().facilities.contains(&plant));
}

#[test]
fn a_plant_being_built_draws_no_bids() {
    let (mut world, dataset) = world();
    let plant = plant_with_bids(&mut world, &dataset);
    world.facilities.get_mut(plant).unwrap().status = FacilityStatus::Building;
    assert!(disposal::plant_bids(&world, &dataset, plant).is_empty());
}
