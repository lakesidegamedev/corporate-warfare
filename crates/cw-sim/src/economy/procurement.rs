//! Funded defense procurement.
//!
//! The government pays a ten percent deposit up front, held as unearned revenue by the supplier.
//! Each week the supplier delivers accepted output and the government pays the balance from its
//! procurement budget; the deposit is earned as it is consumed. A shortfall beyond two weeks is
//! penalised at five percent of undelivered value. Unearned deposit is refunded when the contract
//! completes.

use cw_core::ledger::{Account, Txn};
use cw_core::money::Cents;
use cw_core::quantity::MilliLots;
use cw_data::Dataset;

use crate::state::{ContractStatus, World};

/// Open a funded contract for every operative defense producer that does not already have one.
/// This is how a newly built or acquired defense plant wins government demand; the offer is not
/// reserved for the seeded firms.
pub fn ensure_contracts(world: &mut World, dataset: &Dataset) {
    let Some(product) = dataset.product_code("defense") else {
        return;
    };
    if world.government.procurement_budget_cents <= 0 {
        return;
    }
    let price = dataset.reference_price(product);
    let mut producers: std::collections::BTreeMap<(cw_core::ids::Handle, u8), i64> =
        std::collections::BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        if !firm.operative() {
            continue;
        }
        for facility_handle in &firm.facilities {
            let Some(facility) = world.facilities.get(*facility_handle) else {
                continue;
            };
            let recipe = &dataset.recipes[facility.recipe as usize];
            if recipe.output == "defense" {
                *producers.entry((handle, facility.region)).or_default() +=
                    recipe.weekly_output_lots * facility.blocks as i64;
            }
        }
    }
    for ((supplier, region), capacity) in producers {
        if capacity <= 0 {
            continue;
        }
        let has_active = world.procurements.iter().any(|(_, existing)| {
            existing.supplier == supplier
                && existing.region == region
                && existing.status == ContractStatus::Active
        });
        if has_active {
            continue;
        }
        let quantity = MilliLots::from_lots(capacity);
        let total_value = quantity.value_at(Cents(price)).unwrap_or(Cents::ZERO).0 * 52;
        let deposit = total_value * 10 / 100;
        let treasury = world.government.book.cash();
        if deposit <= 0 || treasury < deposit {
            continue;
        }
        let _ = world.government.book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, deposit, "procurement deposit")
                .debit(Account::PrepaidExpenses, deposit, "procurement deposit"),
        );
        if let Some(firm) = world.firms.get_mut(supplier) {
            let _ = firm.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, deposit, "procurement deposit")
                    .credit(Account::UnearnedRevenue, deposit, "procurement deposit"),
            );
        }
        world.procurements.insert(crate::state::Procurement {
            supplier,
            region,
            quantity_weekly: quantity,
            price_cents: price,
            remaining_weeks: 52,
            next_delivery: cw_core::calendar::Tick(world.tick.0 + 1),
            unearned_cents: deposit,
            shortfall_weeks: 0,
            status: ContractStatus::Active,
        });
    }
}

pub fn settle(world: &mut World, dataset: &Dataset) {
    let handles: Vec<_> = world
        .procurements
        .iter()
        .map(|(handle, _)| handle)
        .collect();
    let Some(product) = dataset.product_code("defense") else {
        return;
    };
    let mut budget = world.government.procurement_budget_cents.max(0);
    for handle in handles {
        let Some(mut procurement) = world.procurements.get(handle).cloned() else {
            continue;
        };
        if procurement.status != ContractStatus::Active
            || procurement.next_delivery.0 > world.tick.0
        {
            continue;
        }
        let available = world
            .firm(procurement.supplier)
            .map(|firm| firm.stock(product, procurement.region))
            .unwrap_or(MilliLots::ZERO);
        let requested = available.min(procurement.quantity_weekly);
        // A funded contract cannot accept more value than the budget plus the deposit already
        // held by the supplier. The treasury must also hold the cash it spends.
        let cash_limit = budget.min(world.government.book.cash().max(0));
        let max_value = cash_limit.saturating_add(procurement.unearned_cents);
        let max_lots = if procurement.price_cents > 0 {
            MilliLots(max_value.saturating_mul(1_000) / procurement.price_cents)
        } else {
            requested
        };
        let delivered = requested.min(max_lots).max_zero();
        let supplier_shortfall =
            procurement.quantity_weekly.0 - available.0.min(procurement.quantity_weekly.0);
        let deferred = procurement.quantity_weekly.0 - delivered.0;
        if deferred > 0 {
            let pending = world
                .government
                .pending_procurement
                .entry(product)
                .or_insert(MilliLots::ZERO);
            pending.0 = pending.0.saturating_add(deferred);
        }
        if delivered.0 > 0 {
            let value = delivered
                .value_at(Cents(procurement.price_cents.max(1)))
                .unwrap_or(Cents::ZERO)
                .0;
            let deposit_used = procurement.unearned_cents.min(value);
            let cash_due = value - deposit_used;
            let carrying = world
                .firms
                .get_mut(procurement.supplier)
                .map(|firm| firm.issue_stock(product, procurement.region, delivered).0)
                .unwrap_or(0);
            if let Some(supplier) = world.firms.get_mut(procurement.supplier) {
                supplier.record_sale(procurement.region, product, delivered);
                let _ = supplier.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, cash_due, "procurement delivery")
                        .debit(Account::UnearnedRevenue, deposit_used, "deposit earned")
                        .credit(Account::Revenue, value, "procurement delivery")
                        .credit(Account::Inventory, carrying, "procurement cogs")
                        .debit(Account::CostOfGoods, carrying, "procurement cogs"),
                );
            }
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, cash_due, "procurement delivery")
                    .credit(Account::PrepaidExpenses, deposit_used, "deposit earned")
                    .debit(Account::OperatingExpense, value, "procurement delivery"),
            );
            budget -= cash_due;
            procurement.unearned_cents -= deposit_used;
        }
        if supplier_shortfall > 0 {
            procurement.shortfall_weeks += 1;
            if procurement.shortfall_weeks > 2 {
                let undelivered = MilliLots(supplier_shortfall);
                let penalty = undelivered
                    .value_at(Cents(procurement.price_cents.max(1)))
                    .unwrap_or(Cents::ZERO)
                    .0
                    * 5
                    / 100;
                let cash = world
                    .firm(procurement.supplier)
                    .map(|firm| firm.book.cash())
                    .unwrap_or(0)
                    .max(0);
                let paid = penalty.min(cash);
                if paid > 0 {
                    let _ = world
                        .firms
                        .get_mut(procurement.supplier)
                        .unwrap()
                        .book
                        .commit(
                            world.tick,
                            Txn::new()
                                .credit(Account::Cash, paid, "procurement penalty")
                                .debit(Account::OperatingExpense, paid, "procurement penalty"),
                        );
                    let _ = world.government.book.commit(
                        world.tick,
                        Txn::new()
                            .debit(Account::Cash, paid, "procurement penalty")
                            .credit(Account::OtherIncome, paid, "procurement penalty"),
                    );
                }
            }
        } else {
            procurement.shortfall_weeks = 0;
        }
        procurement.remaining_weeks = procurement.remaining_weeks.saturating_sub(1);
        procurement.next_delivery = cw_core::calendar::Tick(world.tick.0 + 1);
        if procurement.remaining_weeks == 0 {
            procurement.status = ContractStatus::Completed;
            // Refund any deposit that was never earned.
            let refund = procurement.unearned_cents.max(0);
            if refund > 0 {
                let cash = world
                    .firm(procurement.supplier)
                    .map(|firm| firm.book.cash())
                    .unwrap_or(0)
                    .max(0);
                let paid = refund.min(cash);
                if paid > 0 {
                    let _ = world
                        .firms
                        .get_mut(procurement.supplier)
                        .unwrap()
                        .book
                        .commit(
                            world.tick,
                            Txn::new()
                                .credit(Account::Cash, paid, "deposit refund")
                                .debit(Account::UnearnedRevenue, paid, "deposit refund"),
                        );
                    let _ = world.government.book.commit(
                        world.tick,
                        Txn::new()
                            .debit(Account::Cash, paid, "deposit refund")
                            .credit(Account::PrepaidExpenses, paid, "deposit refund"),
                    );
                }
                procurement.unearned_cents -= paid;
            }
        }
        if let Some(stored) = world.procurements.get_mut(handle) {
            *stored = procurement;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{ScenarioKind, ScenarioMode};

    fn world() -> (World, Dataset) {
        let dataset = Dataset::load().unwrap();
        let world =
            crate::economy::seed::build(&dataset, 7, ScenarioKind::Guided, ScenarioMode::Sandbox);
        (world, dataset)
    }

    #[test]
    fn defense_procurement_is_funded_by_a_ten_percent_deposit() {
        let (world, _) = world();
        let (_, procurement) = world
            .procurements
            .iter()
            .next()
            .expect("a defense contract");
        let quantity = procurement.quantity_weekly;
        let total = quantity.value_at(Cents(procurement.price_cents)).unwrap().0 * 52;
        assert_eq!(procurement.unearned_cents, total * 10 / 100);
        assert_eq!(procurement.remaining_weeks, 52);
        let supplier = world.firm(procurement.supplier).unwrap();
        assert!(supplier.book.is_balanced());
        assert!(world.government.book.is_balanced());
        // The deposits are an asset for the government and a liability for the suppliers.
        let deposits: i64 = world
            .procurements
            .iter()
            .map(|(_, item)| item.unearned_cents)
            .sum();
        assert_eq!(
            world.government.book.balance(Account::PrepaidExpenses),
            deposits
        );
        assert_eq!(
            supplier.book.balance(Account::UnearnedRevenue),
            -procurement.unearned_cents
        );
    }

    #[test]
    fn accepted_delivery_earns_the_deposit_and_pays_the_supplier() {
        let (mut world, dataset) = world();
        let (handle, _) = world.procurements.iter().next().unwrap();
        let supplier = world.procurements.get(handle).unwrap().supplier;
        let initial = world.procurements.get(handle).unwrap().unearned_cents;
        let product = dataset.product_code("defense").unwrap();
        let mut delivered = false;
        for _ in 0..6 {
            crate::tick::advance(&mut world, &dataset);
            let procurement = world.procurements.get(handle).unwrap();
            if procurement.unearned_cents < initial {
                delivered = true;
                break;
            }
        }
        assert!(delivered, "the supplier delivered accepted output");
        let procurement = world.procurements.get(handle).unwrap();
        assert!(procurement.remaining_weeks < 52, "weeks were served");
        assert_eq!(
            procurement.shortfall_weeks, 0,
            "a delivery clears the shortfall"
        );
        assert!(
            world
                .firm(supplier)
                .unwrap()
                .stock(product, procurement.region)
                .0
                >= 0
        );
        assert!(world.firm(supplier).unwrap().book.is_balanced());
        assert!(world.government.book.is_balanced());
    }

    #[test]
    fn a_shortfall_beyond_two_weeks_costs_five_percent() {
        let (mut world, dataset) = world();
        let (handle, _) = world.procurements.iter().next().unwrap();
        let procurement = world.procurements.get(handle).unwrap().clone();
        let supplier = procurement.supplier;
        let product = dataset.product_code("defense").unwrap();
        {
            let firm = world.firms.get_mut(supplier).unwrap();
            firm.inventory.remove(&(product, procurement.region));
        }
        {
            let stored = world.procurements.get_mut(handle).unwrap();
            stored.shortfall_weeks = 2;
            stored.next_delivery = world.tick;
        }
        let before = world.firm(supplier).unwrap().book.cash();
        let treasury = world.government.book.cash();
        let penalty = procurement
            .quantity_weekly
            .value_at(Cents(procurement.price_cents))
            .unwrap()
            .0
            * 5
            / 100;
        settle(&mut world, &dataset);
        let after = world.firm(supplier).unwrap().book.cash();
        assert_eq!(before - after, penalty.min(before));
        assert_eq!(world.government.book.cash(), treasury + penalty.min(before));
        assert!(world.firm(supplier).unwrap().book.is_balanced());
        assert!(world.government.book.is_balanced());
    }

    #[test]
    fn an_unfunded_week_is_deferred_not_penalised() {
        let (mut world, dataset) = world();
        // Produce one week of output so the supplier has stock to defer.
        crate::tick::advance(&mut world, &dataset);
        let (handle, _) = world.procurements.iter().next().unwrap();
        let procurement = world.procurements.get(handle).unwrap().clone();
        let supplier = procurement.supplier;
        let product = dataset.product_code("defense").unwrap();
        let budget = world.government.procurement_budget_cents;
        world.government.procurement_budget_cents = 0;
        {
            let stored = world.procurements.get_mut(handle).unwrap();
            stored.unearned_cents = 0;
            stored.next_delivery = world.tick;
        }
        let stock = world
            .firm(supplier)
            .unwrap()
            .stock(product, procurement.region);
        let cash = world.firm(supplier).unwrap().book.cash();
        settle(&mut world, &dataset);
        let stored = world.procurements.get(handle).unwrap();
        assert_eq!(stored.shortfall_weeks, 0, "the supplier had stock");
        assert_eq!(
            world
                .firm(supplier)
                .unwrap()
                .stock(product, procurement.region),
            stock,
            "nothing is accepted without a funded payment"
        );
        assert_eq!(world.firm(supplier).unwrap().book.cash(), cash);
        assert!(
            world.government.pending_procurement[&product].0 > 0,
            "unmet demand is preserved"
        );
        world.government.procurement_budget_cents = budget;
        assert!(world.firm(supplier).unwrap().book.is_balanced());
        assert!(world.government.book.is_balanced());
    }

    #[test]
    fn completing_a_contract_refunds_the_unearned_deposit() {
        let (mut world, dataset) = world();
        let (handle, _) = world.procurements.iter().next().unwrap();
        let supplier = world.procurements.get(handle).unwrap().supplier;
        {
            let stored = world.procurements.get_mut(handle).unwrap();
            stored.remaining_weeks = 1;
            stored.unearned_cents = 1_000_000;
            stored.next_delivery = world.tick;
        }
        settle(&mut world, &dataset);
        let stored = world.procurements.get(handle).unwrap();
        assert_eq!(stored.status, ContractStatus::Completed);
        assert_eq!(stored.unearned_cents, 0, "unearned deposit is refunded");
        assert!(world.firm(supplier).unwrap().book.is_balanced());
        assert!(world.government.book.is_balanced());
    }

    #[test]
    fn every_operative_defense_producer_can_win_a_contract() {
        let (mut world, dataset) = world();
        let supplier = world
            .procurements
            .iter()
            .next()
            .map(|(_, item)| item.supplier)
            .unwrap();
        // Retire the existing offers; the government reopens demand for any defense producer.
        for (_, item) in world.procurements.iter_mut() {
            item.status = ContractStatus::Completed;
        }
        ensure_contracts(&mut world, &dataset);
        assert!(
            world
                .procurements
                .iter()
                .any(|(_, item)| item.supplier == supplier
                    && item.status == ContractStatus::Active),
            "the producer is offered a replacement contract"
        );
        assert!(world.firm(supplier).unwrap().book.is_balanced());
        assert!(world.government.book.is_balanced());
    }
}
