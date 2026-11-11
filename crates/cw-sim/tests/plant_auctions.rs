//! Plant auctions (2.0): a failed company's plants go to the best bid at the deadline as running
//! plants, or for scrap; the player can bid; an owner can list a plant, see the bids and accept or
//! withdraw. The money a plant fetches is exactly what its buyer pays.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::economy::auctions::{self, LotOutcome, LotRoute};
use cw_sim::economy::{disposal, insolvency, ownership, seed};
use cw_sim::state::{FacilityStatus, FirmStatus, HolderId, ScenarioKind, ScenarioMode, World};

fn fund(world: &mut World, firm: Handle, cents: i64) {
    let _ = world.firms.get_mut(firm).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, cents, "test funding")
            .credit(Account::ShareCapital, cents, "test funding"),
    );
}

fn world() -> (World, Dataset) {
    built(ScenarioKind::Founder)
}

fn built(kind: ScenarioKind) -> (World, Dataset) {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 902, kind, ScenarioMode::Sandbox);
    for _ in 0..8 {
        cw_sim::tick::advance(&mut world, &dataset);
    }
    (world, dataset)
}

fn rivals(world: &World) -> Vec<Handle> {
    world
        .firms
        .iter()
        .filter(|(h, f)| {
            f.operative() && ownership::ultimate_controller(world, *h) != Some(HolderId::Investor)
        })
        .map(|(h, _)| h)
        .collect()
}

/// A rival whose plants draw bids once every other rival has ample cash.
fn failing_rival(world: &mut World, dataset: &Dataset) -> Handle {
    for firm in rivals(world) {
        fund(world, firm, 50_000_000_000);
    }
    let plants: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(_, p)| p.status == FacilityStatus::Operating)
        .map(|(h, _)| h)
        .collect();
    let plant = plants
        .into_iter()
        .find(|plant| !disposal::plant_bids(world, dataset, *plant).is_empty())
        .expect("some plant draws a bid");
    world.facilities.get(plant).unwrap().firm
}

/// Run the auction step at each week up to and including `week`, without the rest of the tick.
fn run_auctions_to(world: &mut World, dataset: &Dataset, week: u32) {
    while world.tick.0 < week {
        world.tick.0 += 1;
        auctions::weekly(world, dataset);
    }
}

#[test]
fn a_failed_company_s_plants_are_auctioned_and_go_running_to_the_best_bid() {
    let (mut world, dataset) = world();
    let weeks = auctions::auction_weeks(&dataset);
    assert_eq!(weeks, 4, "content opens plant auctions for four weeks");
    let firm = failing_rival(&mut world, &dataset);
    let plants = auctions::breakup_plants(&world, &dataset, firm);
    assert!(!plants.is_empty());
    let before: Vec<(Handle, u8, FacilityStatus)> = plants
        .iter()
        .map(|h| {
            let p = world.facilities.get(*h).unwrap();
            (*h, p.blocks, p.status)
        })
        .collect();
    insolvency::liquidate(&mut world, firm, &dataset);
    // The company waits in liquidation with its plants on offer; nothing is sold yet.
    assert_eq!(world.firm(firm).unwrap().status, FirmStatus::Liquidating);
    assert_eq!(world.auctions.lots.len(), plants.len());
    for lot in &world.auctions.lots {
        assert_eq!(lot.route, LotRoute::BreakUp);
        assert_eq!(lot.seller, firm);
        assert_eq!(lot.closes.0, world.tick.0 + weeks);
        assert_eq!(
            lot.reserve_cents,
            lot.book_cents * auctions::SCRAP_PERCENT / 100
        );
        assert_eq!(world.facilities.get(lot.facility).unwrap().firm, firm);
    }
    // The plants stay on the books until they are sold.
    let kept: i64 = plants
        .iter()
        .map(|h| world.facilities.get(*h).unwrap().capex_cents)
        .sum();
    assert_eq!(
        world.firm(firm).unwrap().book.balance(Account::FixedAssets),
        kept
    );
    let cash_before: Vec<(Handle, i64)> = world
        .firms
        .iter()
        .map(|(h, f)| (h, f.book.cash()))
        .collect();
    let closes = world.auctions.lots[0].closes.0;
    run_auctions_to(&mut world, &dataset, closes);
    assert!(world.auctions.lots.is_empty());
    assert_eq!(world.firm(firm).unwrap().status, FirmStatus::Dissolved);
    let closed: Vec<_> = world
        .auctions
        .closed
        .iter()
        .filter(|c| c.lot.seller == firm)
        .collect();
    assert_eq!(closed.len(), plants.len());
    let mut sold = 0;
    for c in closed {
        match c.outcome {
            LotOutcome::Sold {
                buyer,
                price_cents,
                by_player,
            } => {
                sold += 1;
                assert!(!by_player);
                assert!(price_cents >= c.lot.reserve_cents);
                let plant = world.facilities.get(c.lot.facility).unwrap();
                let (_, blocks, status) = before
                    .iter()
                    .find(|(h, _, _)| *h == c.lot.facility)
                    .unwrap();
                assert_eq!(plant.firm, buyer);
                assert_eq!(plant.blocks, *blocks);
                assert_eq!(plant.status, *status);
                assert_eq!(plant.capex_cents, price_cents);
                assert!(
                    world
                        .firm(buyer)
                        .unwrap()
                        .facilities
                        .contains(&c.lot.facility)
                );
                // The buyer paid exactly the price (one plant per buyer in this world, or the
                // sum of its purchases).
                let paid: i64 = world
                    .auctions
                    .closed
                    .iter()
                    .filter_map(|o| match o.outcome {
                        LotOutcome::Sold {
                            buyer: b,
                            price_cents: p,
                            ..
                        } if b == buyer => Some(p),
                        _ => None,
                    })
                    .sum();
                let was = cash_before.iter().find(|(h, _)| *h == buyer).unwrap().1;
                assert_eq!(world.firm(buyer).unwrap().book.cash(), was - paid);
            }
            LotOutcome::Scrapped { .. } => {
                assert!(world.facilities.get(c.lot.facility).is_none());
            }
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert!(sold > 0, "a plant that drew a bid is sold");
    assert_eq!(world.auctions.totals.breakup_lots as usize, plants.len());
    // The dissolved company keeps no plant on its books.
    let book = &world.firm(firm).unwrap().book;
    assert_eq!(book.balance(Account::FixedAssets), 0);
    assert_eq!(book.balance(Account::AccumulatedDepreciation), 0);
}

#[test]
fn the_player_s_bid_wins_when_it_is_best_and_falls_away_when_it_cannot_be_paid() {
    let (mut world, dataset) = built(ScenarioKind::Guided);
    let firm = failing_rival(&mut world, &dataset);
    let player = world.investor.primary_group.expect("player company");
    insolvency::liquidate(&mut world, firm, &dataset);
    let lot = world.auctions.lots[0].clone();
    // A bid through the seller is refused; one beyond the company's cash is refused.
    assert!(auctions::bid(&mut world, firm, lot.id, 1).is_err());
    let cash = world.firm(player).unwrap().book.cash();
    assert!(auctions::bid(&mut world, player, lot.id, cash + 1).is_err());
    let rival_best = disposal::plant_bids(&world, &dataset, lot.facility)
        .first()
        .map(|b| b.price_cents)
        .unwrap_or(0);
    let price = rival_best.max(lot.reserve_cents) + 100_000;
    fund(&mut world, player, price * 2);
    auctions::bid(&mut world, player, lot.id, price).unwrap();
    let player_cash = world.firm(player).unwrap().book.cash();
    run_auctions_to(&mut world, &dataset, lot.closes.0);
    let plant = world
        .facilities
        .get(lot.facility)
        .expect("plant sold, not scrapped");
    assert_eq!(plant.firm, player);
    assert_eq!(world.firm(player).unwrap().book.cash(), player_cash - price);
    assert_eq!(world.auctions.totals.sold_to_player, 1);

    // A second failure: the player's bid is beyond its cash at the deadline, so it falls away.
    let (mut world, dataset) = built(ScenarioKind::Guided);
    let firm = failing_rival(&mut world, &dataset);
    let player = world.investor.primary_group.unwrap();
    insolvency::liquidate(&mut world, firm, &dataset);
    let lot = world.auctions.lots[0].clone();
    fund(&mut world, player, 90_000_000_000);
    auctions::bid(&mut world, player, lot.id, 80_000_000_000).unwrap();
    let cash = world.firm(player).unwrap().book.cash();
    let _ = world.firms.get_mut(player).unwrap().book.commit(
        world.tick,
        Txn::new()
            .credit(Account::Cash, cash - 1, "test drain")
            .debit(Account::OperatingExpense, cash - 1, "test drain"),
    );
    run_auctions_to(&mut world, &dataset, lot.closes.0);
    assert!(
        world
            .facilities
            .get(lot.facility)
            .is_none_or(|p| p.firm != player)
    );
    assert_eq!(world.auctions.totals.sold_to_player, 0);
}

#[test]
fn a_listed_plant_runs_until_its_owner_accepts_a_bid_or_withdraws() {
    let (mut world, dataset) = world();
    let firm = failing_rival(&mut world, &dataset);
    let plants = auctions::breakup_plants(&world, &dataset, firm);
    let plant = *plants
        .iter()
        .find(|p| !disposal::plant_bids(&world, &dataset, **p).is_empty())
        .unwrap();
    let id = auctions::list(&mut world, &dataset, plant).unwrap();
    assert!(
        auctions::list(&mut world, &dataset, plant).is_err(),
        "listed once"
    );
    let closes = world.auctions.lot(id).unwrap().closes.0;
    run_auctions_to(&mut world, &dataset, closes);
    let lot = world
        .auctions
        .lot(id)
        .expect("bids put to the owner")
        .clone();
    assert!(!lot.offered.is_empty());
    assert_eq!(
        world.facilities.get(plant).unwrap().firm,
        firm,
        "still theirs"
    );
    let best = lot.offered[0].clone();
    let seller_cash = world.firm(firm).unwrap().book.cash();
    let buyer_cash = world.firm(best.bidder).unwrap().book.cash();
    auctions::accept(&mut world, id).unwrap();
    assert_eq!(world.facilities.get(plant).unwrap().firm, best.bidder);
    assert_eq!(
        world.firm(firm).unwrap().book.cash(),
        seller_cash + best.price_cents
    );
    assert_eq!(
        world.firm(best.bidder).unwrap().book.cash(),
        buyer_cash - best.price_cents
    );
    assert_eq!(world.auctions.totals.listings_sold, 1);

    // Withdrawn before the deadline: nothing moves. Left unanswered: the offer lapses.
    let other = *plants
        .iter()
        .find(|p| world.facilities.get(**p).is_some_and(|f| f.firm == firm))
        .expect("another plant");
    let id = auctions::list(&mut world, &dataset, other).unwrap();
    auctions::withdraw(&mut world, id).unwrap();
    assert!(world.auctions.lot(id).is_none());
    assert_eq!(world.facilities.get(other).unwrap().firm, firm);
    let id = auctions::list(&mut world, &dataset, other).unwrap();
    let closes = world.auctions.lot(id).unwrap().closes.0;
    run_auctions_to(
        &mut world,
        &dataset,
        closes + auctions::auction_weeks(&dataset),
    );
    assert!(world.auctions.lot(id).is_none());
    assert_eq!(world.facilities.get(other).unwrap().firm, firm);
    assert_eq!(world.auctions.totals.listings_withdrawn, 2);
}

#[test]
fn a_rival_folds_in_a_company_it_holds_at_ninety_percent() {
    use cw_sim::commands::{CommandEnvelope, CommandIntent, apply};
    let (mut world, dataset) = built(ScenarioKind::Guided);
    let rivals = rivals(&world);
    let (buyer, target) = (rivals[0], rivals[1]);
    // The buyer comes to hold 92 percent of the target, bought from its other holders.
    let holders: Vec<(HolderId, i64)> = world
        .firm(target)
        .unwrap()
        .shares
        .holders
        .iter()
        .filter(|(h, lot)| !h.is_treasury() && lot.qty > 0)
        .map(|(h, lot)| (*h, lot.qty))
        .collect();
    let issued: i64 = holders.iter().map(|(_, q)| *q).sum();
    let mut wanted = issued * 92 / 100;
    for (holder, qty) in holders {
        let take = qty.min(wanted);
        if take > 0 {
            world.firms.get_mut(target).unwrap().shares.transfer(
                holder,
                HolderId::Firm(buyer),
                take,
                0,
            );
            wanted -= take;
        }
    }
    fund(&mut world, buyer, 50_000_000_000);
    let plants = world.firm(target).unwrap().facilities.clone();
    let cost = cw_sim::economy::deals::minority_cost(&world, buyer, target, 0);
    assert!(cost > 0, "minorities remain");
    let cash = world.firm(buyer).unwrap().book.cash() + world.firm(target).unwrap().book.cash();
    let envelope = CommandEnvelope {
        command_id_high: 0,
        command_id_low: 1,
        actor: buyer.0,
        expected_revision: world.revision,
        submitted_tick: world.tick.0,
        intent: CommandIntent::FoldInCompany { buyer, target },
    };
    apply(&mut world, &dataset, &envelope).unwrap();
    assert_eq!(
        world.firm(target).unwrap().status,
        FirmStatus::Merged { into: buyer }
    );
    for plant in plants {
        assert_eq!(world.facilities.get(plant).unwrap().firm, buyer);
    }
    assert_eq!(world.firm(buyer).unwrap().book.cash(), cash - cost);
    let record = world.auctions.mergers.last().unwrap();
    assert_eq!(
        (record.buyer, record.target, record.minority_cents),
        (buyer, target, cost)
    );
    assert!(world.auctions.merged_lately(buyer, world.tick));
    // A company the buyer does not hold at 90 percent is refused.
    let other = rivals[2];
    let envelope = CommandEnvelope {
        command_id_low: 2,
        expected_revision: world.revision,
        intent: CommandIntent::FoldInCompany {
            buyer,
            target: other,
        },
        ..envelope
    };
    assert!(apply(&mut world, &dataset, &envelope).is_err());
}
