//! Freight, supply contracts and inter-region delivery.
//!
//! Distance comes from validated lane lengths. Baseline road rate is five cents per lot per
//! kilometre with a one dollar minimum per inter-region shipment; lead time is
//! `max(1, ceil(km / 500))` weeks. Contracts are honoured before spot capacity, delivery
//! settles once, and stock never teleports between regions.

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::{Cents, round_even_div};
use cw_core::quantity::MilliLots;
use cw_data::Dataset;

use crate::state::{ContractStatus, Shipment, World};

pub const FREIGHT_CENTS_PER_LOT_KM: i64 = 10;
pub const FREIGHT_MINIMUM_CENTS: i64 = 100;

pub fn lead_time_weeks(km: u32) -> u32 {
    ((km as u64).div_ceil(500)).max(1) as u32
}

pub fn freight_cost(dataset: &Dataset, from: u8, to: u8, qty: MilliLots) -> i64 {
    if from == to {
        return 0;
    }
    let km = dataset
        .lane_km(
            &dataset.regions[from as usize].code,
            &dataset.regions[to as usize].code,
        )
        .unwrap_or(500);
    let lots = qty.lots().max(1);
    let cost = lots * km as i64 * FREIGHT_CENTS_PER_LOT_KM;
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
            .receive(
                shipment.quantity,
                Cents(shipment.unit_price_cents * shipment.quantity.lots()),
            );
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

        // Remove seller stock and charge the buyer.
        let carrying = if let Some(seller) = world.firms.get_mut(contract.seller) {
            seller
                .issue_stock(contract.product, contract.from_region, deliverable)
                .0
        } else {
            continue;
        };
        let price = contract.price_cents;
        let goods_value = deliverable.value_at(Cents(price)).unwrap_or(Cents::ZERO).0;
        let margin = goods_value.saturating_sub(carrying).max(0);
        let freight = freight_cost(
            dataset,
            contract.from_region,
            contract.to_region,
            deliverable,
        );
        let total = goods_value + freight;
        let buyer_cash = world
            .firms
            .get(contract.buyer)
            .map(|f| f.book.cash())
            .unwrap_or(0);
        let paid = total.min(buyer_cash.max(0));
        if paid < total {
            crate::economy::insolvency::defer_expense(
                world,
                contract.buyer,
                total - paid,
                Account::CostOfGoods,
                "contract payment",
            );
        }
        let paid_goods = if total > 0 {
            goods_value * paid / total
        } else {
            0
        };
        let paid_freight = paid - paid_goods;
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
        }
        if paid_freight > 0 {
            pay_freight(world, contract.buyer, paid_freight, "contract freight");
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
            let km = dataset
                .lane_km(
                    &dataset.regions[contract.from_region as usize].code,
                    &dataset.regions[contract.to_region as usize].code,
                )
                .unwrap_or(500);
            world.shipments.push(Shipment {
                seller: contract.seller,
                buyer: contract.buyer,
                product: contract.product,
                quantity: deliverable,
                from_region: contract.from_region,
                to_region: contract.to_region,
                arrive: Tick(world.tick.0 + lead_time_weeks(km)),
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

/// Charge one freight bill to the payer and pay it to a named carrier when one exists,
/// otherwise to the finite outside carrier sector. The payer is debited exactly once so the
/// transport service has a real counterparty instead of minting carrier revenue.
pub fn pay_freight(world: &mut World, payer: Handle, amount: i64, memo: &str) {
    if amount <= 0 {
        return;
    }
    let cash = world
        .firms
        .get(payer)
        .map(|firm| firm.book.cash())
        .unwrap_or(0)
        .max(0);
    let paid = amount.min(cash);
    if paid <= 0 {
        return;
    }
    let _ = world.firms.get_mut(payer).unwrap().book.commit(
        world.tick,
        Txn::new()
            .credit(Account::Cash, paid, memo)
            .debit(Account::CostOfGoods, paid, memo),
    );
    // Industry index 6 is logistics in INDUSTRY_NAMES.
    let carriers: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.industry == 6 && firm.operative())
        .map(|(handle, _)| handle)
        .collect();
    let share = if carriers.is_empty() {
        0
    } else {
        paid / carriers.len() as i64
    };
    let mut remaining = paid;
    for carrier in carriers {
        let portion = share.min(remaining);
        if portion > 0 {
            if let Some(firm) = world.firms.get_mut(carrier) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new().debit(Account::Cash, portion, memo).credit(
                        Account::Revenue,
                        portion,
                        memo,
                    ),
                );
            }
            remaining -= portion;
        }
    }
    if remaining > 0 {
        let _ = world.outside.gateway_book.commit(
            world.tick,
            Txn::new().debit(Account::Cash, remaining, memo).credit(
                Account::Revenue,
                remaining,
                memo,
            ),
        );
    }
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
        let km = dataset.lane_km("OH", "MI").unwrap();
        let cost = freight_cost(&dataset, 3, 6, MilliLots::from_lots(10));
        assert_eq!(cost, 10 * km as i64 * FREIGHT_CENTS_PER_LOT_KM);
        let tiny = freight_cost(&dataset, 3, 6, MilliLots::from_lots(1));
        assert_eq!(tiny, km as i64 * FREIGHT_CENTS_PER_LOT_KM);
        // The minimum only binds on a lane shorter than twenty kilometres.
        assert!(freight_cost(&dataset, 3, 6, MilliLots::from_lots(1)) >= FREIGHT_MINIMUM_CENTS);
        assert_eq!(freight_cost(&dataset, 3, 3, MilliLots::from_lots(10)), 0);
    }
}
