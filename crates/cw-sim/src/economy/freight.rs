//! Supply contracts and inter-region delivery.
//!
//! Distance comes from validated lane lengths. `freight_cost` is the reference truck rate (three
//! cents per lot per kilometre with a one dollar minimum per inter-region shipment) used to
//! rank sellers and size plans; the price actually charged comes from `economy::logistics`,
//! which routes each shipment over lanes with finite capacity. Lead time over a single truck leg
//! is `max(1, ceil(km / 500))` weeks. Contracts are honoured before spot capacity, delivery
//! settles once, and stock never teleports between regions.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::{Cents, round_even_div};
use cw_core::quantity::MilliLots;
use cw_data::Dataset;

use crate::state::{ContractStatus, Shipment, World};

pub const FREIGHT_CENTS_PER_LOT_KM: i64 = 3;
pub const FREIGHT_MINIMUM_CENTS: i64 = 100;

pub fn lead_time_weeks(km: u32) -> u32 {
    ((km as u64).div_ceil(500)).max(1) as u32
}

pub fn freight_cost(dataset: &Dataset, from: u8, to: u8, qty: MilliLots) -> i64 {
    if from == to {
        return 0;
    }
    let km = dataset.km(from, to);
    let lots = qty.lots().max(1);
    let cost = lots
        * km as i64
        * FREIGHT_CENTS_PER_LOT_KM
        * crate::economy::operations::road_bps(dataset, from, to)
        / 10_000;
    cost.max(FREIGHT_MINIMUM_CENTS)
}

/// Deliver shipments that have arrived, then run contracts due this week.
pub fn settle(world: &mut World, dataset: &Dataset) {
    let arrivals: Vec<Shipment> = world
        .shipments
        .iter()
        .filter(|shipment| shipment.arrive.0 <= world.tick.0)
        .cloned()
        .collect();
    world
        .shipments
        .retain(|shipment| shipment.arrive.0 > world.tick.0);
    for shipment in arrivals {
        receive_shipment(world, &shipment, dataset);
    }
    settle_contracts(world, dataset);
}

fn receive_shipment(world: &mut World, shipment: &Shipment, dataset: &Dataset) {
    let _ = dataset;
    let same_group = crate::economy::ownership::same_group(world, shipment.seller, shipment.buyer);
    if let Some(buyer) = world.firms.get_mut(shipment.buyer) {
        buyer
            .stock_lot_mut(shipment.product, shipment.to_region)
            // Carry what the buyer actually paid and posted to inventory at dispatch. A buyer
            // short of cash pays part and the rest is deferred as an expense, so the list
            // price would carry cost the ledger never recorded.
            .receive(shipment.quantity, Cents(shipment.goods_value_cents));
        if same_group {
            buyer.record_intercompany_receipt(
                shipment.seller,
                shipment.product,
                shipment.to_region,
                shipment.quantity,
                shipment.margin_cents,
            );
        }
    }
}

fn settle_contracts(world: &mut World, dataset: &Dataset) {
    let due: Vec<Handle> = world
        .contracts
        .iter()
        .filter(|(_, contract)| {
            contract.status == ContractStatus::Active && contract.next_delivery.0 <= world.tick.0
        })
        .map(|(handle, _)| handle)
        .collect();
    for handle in due {
        resource_contract(world, dataset, handle);
        let Some(contract) = world.contracts.get(handle).cloned() else {
            continue;
        };
        let available = world
            .firms
            .get(contract.seller)
            .map(|firm| firm.stock(contract.product, contract.from_region))
            .unwrap_or(MilliLots::ZERO);
        let deliverable = available.min(contract.quantity_weekly);
        if deliverable.0 <= 0 {
            // Documented shortfall, then a contractual penalty. No off-map replacement.
            let shortfall = contract.quantity_weekly;
            let penalty = round_even_div(
                contract.price_cents as i128 * shortfall.0 as i128 * contract.penalty_bps as i128,
                1_000 * 10_000,
            ) as i64;
            if let Some(entity) = world.firms.get_mut(contract.seller) {
                let cash = entity.book.cash().max(0);
                let paid = penalty.min(cash);
                if paid > 0 {
                    let _ = entity.book.commit(
                        world.tick,
                        Txn::new()
                            .credit(Account::Cash, paid, "contract penalty")
                            .debit(Account::OperatingExpense, paid, "contract penalty"),
                    );
                    if let Some(buyer) = world.firms.get_mut(contract.buyer) {
                        let _ = buyer.book.commit(
                            world.tick,
                            Txn::new()
                                .debit(Account::Cash, paid, "penalty received")
                                .credit(Account::OtherIncome, paid, "penalty received"),
                        );
                    }
                }
            }
            if let Some(stored) = world.contracts.get_mut(handle) {
                stored.shortfalls = MilliLots(stored.shortfalls.0 + shortfall.0);
                stored.next_delivery = Tick(world.tick.0 + 1);
                stored.remaining_weeks = stored.remaining_weeks.saturating_sub(1);
                if stored.remaining_weeks == 0 {
                    if stored.auto_renew {
                        stored.remaining_weeks = 52;
                    } else {
                        stored.status = ContractStatus::Defaulted;
                    }
                }
            }
            continue;
        }

        // The buyer takes only what it can pay for in full, goods and freight together; a
        // seller never hands over goods for part of their price. What the buyer cannot afford
        // stays with the seller for the exchange, and the week counts as delivered.
        let price = contract.price_cents;
        let product = contract.product;
        let buyer_cash = world
            .firms
            .get(contract.buyer)
            .map(|f| f.book.cash())
            .unwrap_or(0)
            .max(0);
        let full_cost = deliverable.value_at(Cents(price)).unwrap_or(Cents::ZERO).0
            + quoted(
                world,
                dataset,
                contract.buyer,
                product,
                contract.from_region,
                contract.to_region,
                deliverable,
            );
        let wanted = deliverable;
        let mut deliverable = deliverable;
        let bill = |world: &mut World, lots: MilliLots| {
            lots.value_at(Cents(price)).unwrap_or(Cents::ZERO).0
                + quoted(
                    world,
                    dataset,
                    contract.buyer,
                    product,
                    contract.from_region,
                    contract.to_region,
                    lots,
                )
        };
        if full_cost > buyer_cash {
            // Whole lots only. Start from the proportional share, trim until the goods and the
            // freight for that many lots fit, then take any further lots that still fit: freight
            // per lot falls with the size of a haul, so the first guess can land short.
            let lots = (deliverable.0 as i128 * buyer_cash as i128
                / full_cost.max(1) as i128
                / 1_000) as i64;
            deliverable = MilliLots::from_lots(lots);
            while deliverable.0 > 0 && bill(world, deliverable) > buyer_cash {
                deliverable = MilliLots(deliverable.0 - 1_000);
            }
            for _ in 0..24 {
                let next = MilliLots(deliverable.0 + 1_000);
                if next.0 > wanted.0 || bill(world, next) > buyer_cash {
                    break;
                }
                deliverable = next;
            }
        }
        if deliverable.0 <= 0 {
            if let Some(stored) = world.contracts.get_mut(handle) {
                stored.next_delivery = Tick(world.tick.0 + 1);
                stored.remaining_weeks = stored.remaining_weeks.saturating_sub(1);
                if stored.remaining_weeks == 0 {
                    if stored.auto_renew {
                        stored.remaining_weeks = 52;
                    } else {
                        stored.status = ContractStatus::Completed;
                    }
                }
            }
            continue;
        }
        let carrying = if let Some(seller) = world.firms.get_mut(contract.seller) {
            seller
                .issue_stock(contract.product, contract.from_region, deliverable)
                .0
        } else {
            continue;
        };
        let goods_value = deliverable.value_at(Cents(price)).unwrap_or(Cents::ZERO).0;
        let margin = goods_value.saturating_sub(carrying).max(0);
        crate::economy::structure::note_delivery(world, handle, deliverable, price);
        let freight = quoted(
            world,
            dataset,
            contract.buyer,
            product,
            contract.from_region,
            contract.to_region,
            deliverable,
        );
        let paid = (goods_value + freight).min(buyer_cash);
        let paid_goods = goods_value.min(paid);
        if paid_goods > 0 {
            let _ = world.firms.get_mut(contract.buyer).unwrap().book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, paid_goods, "contract purchase")
                    .debit(Account::Inventory, paid_goods, "contract purchase"),
            );
        }
        if let Some(seller) = world.firms.get_mut(contract.seller) {
            let _ = seller.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, paid_goods, "contract sale")
                    .credit(Account::Revenue, paid_goods, "contract sale")
                    .credit(Account::Inventory, carrying, "contract cogs")
                    .debit(Account::CostOfGoods, carrying, "contract cogs"),
            );
            // A contract delivery is a sale like any other: it belongs in the unit-sales record
            // that pricing, the market board and the AI read.
            seller.record_sale(contract.to_region, contract.product, deliverable);
        }
        // The route is booked after the goods change hands. A buyer that could only pay part of
        // the bill pays part of the freight, as before.
        let mut transit_weeks = 1;
        let mut paid_freight = 0;
        if contract.from_region != contract.to_region {
            let booked = crate::economy::logistics::book(
                world,
                dataset,
                contract.buyer,
                product,
                contract.from_region,
                contract.to_region,
                deliverable,
                "contract freight",
            );
            transit_weeks = booked.weeks;
            paid_freight = booked.cents;
        }

        if contract.from_region == contract.to_region {
            let same_group =
                crate::economy::ownership::same_group(world, contract.seller, contract.buyer);
            if let Some(buyer) = world.firms.get_mut(contract.buyer) {
                buyer
                    .stock_lot_mut(contract.product, contract.to_region)
                    .receive(deliverable, Cents(paid_goods));
                if same_group {
                    buyer.record_intercompany_receipt(
                        contract.seller,
                        contract.product,
                        contract.to_region,
                        deliverable,
                        margin,
                    );
                }
            }
        } else {
            world.shipments.push(Shipment {
                seller: contract.seller,
                buyer: contract.buyer,
                product: contract.product,
                quantity: deliverable,
                from_region: contract.from_region,
                to_region: contract.to_region,
                arrive: Tick(world.tick.0 + transit_weeks),
                unit_price_cents: price,
                goods_value_cents: paid_goods,
                freight_cents: paid_freight,
                margin_cents: margin,
            });
        }

        if let Some(stored) = world.contracts.get_mut(handle) {
            stored.next_delivery = Tick(world.tick.0 + 1);
            stored.remaining_weeks = stored.remaining_weeks.saturating_sub(1);
            if stored.remaining_weeks == 0 {
                if stored.auto_renew {
                    // Baseline trade relationships renew for another year instead of leaving
                    // every producer without a buyer after week 52.
                    stored.remaining_weeks = 52;
                } else {
                    stored.status = ContractStatus::Completed;
                }
            }
        }
    }
}

/// A seller ships a contract from where it makes the good. When the seller has none of the good
/// in the contract's origin state and no operating plant there that makes it (the plant was sold,
/// closed or carved out), the origin moves to the state of its largest operating plant for the
/// good, or failing that the state where it holds the most beyond its own plants' input cover.
/// A firm whose mill is in one state then does not default on a contract written from another
/// and pay the penalty while its output leaves at the export bid. The buyer pays freight from the
/// new origin. Nothing moves while the origin holds stock or makes the good.
pub fn resource_contract(world: &mut World, dataset: &Dataset, handle: Handle) {
    let Some(contract) = world.contracts.get(handle) else {
        return;
    };
    let (seller, product, origin) = (contract.seller, contract.product, contract.from_region);
    let Some(firm) = world.firms.get(seller) else {
        return;
    };
    if firm.stock(product, origin).0 > 0 {
        return;
    }
    let id = &dataset.products[product as usize].id;
    // (lots a week at full run, region) of each operating plant making the good.
    let plants: Vec<(i64, u8)> = firm
        .facilities
        .iter()
        .filter_map(|plant| world.facilities.get(*plant))
        .filter(|plant| plant.status == crate::state::FacilityStatus::Operating)
        .map(|plant| {
            let lots = dataset.recipes[plant.recipe as usize]
                .lots_of(id)
                .saturating_mul(plant.blocks as i64);
            (lots, plant.region)
        })
        .filter(|(lots, _)| *lots > 0)
        .collect();
    if plants.iter().any(|(_, region)| *region == origin) {
        return;
    }
    // Largest first; ties go to the lower region index.
    let largest = |list: &[(i64, u8)]| {
        list.iter()
            .max_by_key(|(lots, region)| (*lots, std::cmp::Reverse(*region)))
            .map(|(_, region)| *region)
    };
    let stocked: Vec<(i64, u8)> = firm
        .inventory
        .iter()
        .filter(|((code, region), _)| *code == product && *region != origin)
        .map(|((_, region), lot)| {
            let cover =
                crate::economy::storage::reserved_milli(world, dataset, firm, *region, product);
            (lot.qty.0 - cover, *region)
        })
        .filter(|(spare, _)| *spare > 0)
        .collect();
    if let Some(region) = largest(&plants).or_else(|| largest(&stocked))
        && let Some(stored) = world.contracts.get_mut(handle)
    {
        stored.from_region = region;
    }
}

/// The freight bill for a shipment as the router would book it now, in cents. Within a region
/// there is none.
pub fn quoted(
    world: &mut World,
    dataset: &Dataset,
    buyer: Handle,
    product: u16,
    from: u8,
    to: u8,
    qty: MilliLots,
) -> i64 {
    if from == to {
        return 0;
    }
    crate::economy::logistics::quote(world, dataset, buyer, product, from, to, qty)
        .map(|plan| plan.cents)
        .unwrap_or_else(|| freight_cost(dataset, from, to, qty))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lead_time_and_freight_follow_the_rule() {
        assert_eq!(lead_time_weeks(0), 1);
        assert_eq!(lead_time_weeks(400), 1);
        assert_eq!(lead_time_weeks(500), 1);
        assert_eq!(lead_time_weeks(501), 2);
        assert_eq!(lead_time_weeks(1000), 2);
    }

    #[test]
    fn freight_cost_is_per_whole_lot_and_has_a_minimum() {
        let dataset = Dataset::load().unwrap();
        let km = dataset.lane_km("OH-CEN", "MI-DET").unwrap();
        assert_eq!(
            dataset.km(
                dataset.region_index["OH-CEN"],
                dataset.region_index["MI-DET"]
            ),
            km
        );
        let (oh, mi) = (
            dataset.region_index["OH-CEN"],
            dataset.region_index["MI-DET"],
        );
        // Ohio roads 6400 and Michigan 5200 average 5800: freight costs 1 percent more.
        let road = crate::economy::operations::road_bps(&dataset, oh, mi);
        assert_eq!(road, 10_100);
        let cost = freight_cost(&dataset, oh, mi, MilliLots::from_lots(10));
        assert_eq!(
            cost,
            10 * km as i64 * FREIGHT_CENTS_PER_LOT_KM * road / 10_000
        );
        let tiny = freight_cost(&dataset, oh, mi, MilliLots::from_lots(1));
        assert_eq!(tiny, km as i64 * FREIGHT_CENTS_PER_LOT_KM * road / 10_000);
        // The minimum only binds on a lane shorter than twenty kilometres.
        assert!(freight_cost(&dataset, oh, mi, MilliLots::from_lots(1)) >= FREIGHT_MINIMUM_CENTS);
        assert_eq!(freight_cost(&dataset, 3, 3, MilliLots::from_lots(10)), 0);
    }
}
