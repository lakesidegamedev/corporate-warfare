//! Production, utilities and inventory.
//!
//! Order is fixed: utility generators dispatch from opening fuel stock, current-week
//! electricity is allocated to all industrial consumers at once from reserved budgets,
//! unserved consumers lose planned output before other inputs are consumed, and then every
//! facility produces from opening usable stock. Goods bought this week are never usable this
//! week because production runs before market clearing.

use std::collections::BTreeMap;

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::{Cents, allocate_cents, round_even_div};
use cw_core::quantity::MilliLots;
use cw_data::Dataset;

use crate::state::{FacilityStatus, World};

const SEASONAL_YIELD_BPS: [i32; 4] = [8_000, 11_000, 13_000, 8_000];

/// Produce the whole week: utilities first, then goods and services.
pub fn produce(world: &mut World, dataset: &Dataset) {
    let power = dispatch_electricity(world, dataset);
    produce_goods(world, dataset, &power);
}

/// Electricity produced this week and the share each facility receives, in basis points.
fn dispatch_electricity(world: &mut World, dataset: &Dataset) -> BTreeMap<Handle, i32> {
    let electricity = dataset.product_index["electricity"];
    let power_recipe: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(_, facility)| {
            facility.status == FacilityStatus::Operating
                && dataset.recipes[facility.recipe as usize].output == "electricity"
        })
        .map(|(handle, _)| handle)
        .collect();

    let price = dataset.reference_price(electricity);
    // Generators produce from opening fuel and staff, without needing their own output.
    let mut supply_by_region: BTreeMap<u8, Vec<(Handle, MilliLots)>> = BTreeMap::new();
    for handle in power_recipe {
        let produced = run_facility(world, dataset, handle, 10_000, &BTreeMap::new(), true);
        if produced.is_positive() {
            let facility = world.facilities.get(handle).expect("facility");
            supply_by_region
                .entry(facility.region)
                .or_default()
                .push((handle, produced));
        }
    }

    // Gateway imports augment local supply, finite per week.
    let mut gateway_supply: BTreeMap<u8, i64> = BTreeMap::new();
    for gateway in &world.gateways {
        if gateway.product == electricity {
            *gateway_supply.entry(gateway.region).or_default() += gateway.capacity_lots * 1_000;
        }
    }

    let mut result = BTreeMap::new();
    let regions = world.regions.len();
    for region_index in 0..regions {
        let mut demand_entries: Vec<(Handle, Handle, i64, i64)> = Vec::new();
        for (handle, facility) in world.facilities.iter() {
            if facility.region as usize != region_index {
                continue;
            }
            if facility.status != FacilityStatus::Operating {
                continue;
            }
            let recipe = &dataset.recipes[facility.recipe as usize];
            if recipe.output == "electricity" {
                continue;
            }
            let need = recipe
                .weekly_inputs_lots
                .get("electricity")
                .copied()
                .unwrap_or(0);
            if need == 0 {
                continue;
            }
            let firm = match world.firms.get(handle) {
                Some(firm) if firm.operative() => firm,
                _ => continue,
            };
            let planned = need * facility.blocks as i64 * facility.target_bps as i64 / 10_000;
            if planned <= 0 {
                continue;
            }
            // A buyer can only reserve what it actually holds in free cash.
            let affordable = if price > 0 {
                firm.book.cash().max(0) / price
            } else {
                planned
            };
            let requested = planned.min(affordable * 1_000).max(0);
            if requested > 0 {
                demand_entries.push((handle, facility.firm, requested, price));
            }
        }
        demand_entries.sort_by_key(|(handle, _, _, _)| *handle);
        let total_local_supply: i64 = supply_by_region
            .get(&(region_index as u8))
            .map(|entries| entries.iter().map(|(_, qty)| qty.0).sum())
            .unwrap_or(0)
            + gateway_supply
                .get(&(region_index as u8))
                .copied()
                .unwrap_or(0);
        let total_demand: i64 = demand_entries.iter().map(|(_, _, qty, _)| *qty).sum();
        if total_demand == 0 {
            continue;
        }
        let supplied = total_local_supply.min(total_demand);
        if supplied <= 0 {
            for (handle, _, _, _) in &demand_entries {
                result.insert(*handle, 0);
            }
            continue;
        }
        let weights: Vec<(u64, i64)> = demand_entries
            .iter()
            .map(|(handle, _, qty, _)| (handle.0, *qty))
            .collect();
        let allocation = allocate_cents(Cents(supplied), &weights);
        // Local generators are paid in proportion to their share of local supply.
        let local_supply = supply_by_region
            .get(&(region_index as u8))
            .cloned()
            .unwrap_or_default();
        let mut local_remaining: i64 = local_supply.iter().map(|(_, qty)| qty.0).sum();
        let mut paid_local = 0i64;
        for (index, (handle, _, requested, unit_price)) in demand_entries.iter().enumerate() {
            let granted = allocation[index].1.0.min(*requested).max(0);
            let ratio = if *requested > 0 {
                (granted * 10_000 / *requested) as i32
            } else {
                0
            };
            result.insert(*handle, ratio);
            if granted == 0 {
                continue;
            }
            let cost = round_even_div(*unit_price as i128 * granted as i128, 1_000) as i64;
            let buyer_firm = match world.facilities.get(*handle) {
                Some(facility) => facility.firm,
                None => continue,
            };
            let cash = world
                .firms
                .get(buyer_firm)
                .map(|f| f.book.cash())
                .unwrap_or(0);
            let paid = cost.min(cash.max(0));
            if paid > 0 {
                let _ = world.firms.get_mut(buyer_firm).unwrap().book.commit(
                    world.tick,
                    Txn::new().credit(Account::Cash, paid, "electricity").debit(
                        Account::CostOfGoods,
                        paid,
                        "electricity",
                    ),
                );
            }
            let mut remaining_payment = paid;
            for (seller, qty) in &local_supply {
                if remaining_payment <= 0 || local_remaining <= 0 {
                    break;
                }
                let share = round_even_div(
                    remaining_payment as i128 * qty.0.min(local_remaining) as i128,
                    local_remaining as i128,
                ) as i64;
                let share = share.min(remaining_payment);
                if share > 0 {
                    if let Some(firm) = world.firms.get_mut(*seller) {
                        let _ = firm.book.commit(
                            world.tick,
                            Txn::new().debit(Account::Cash, share, "power sale").credit(
                                Account::Revenue,
                                share,
                                "power sale",
                            ),
                        );
                    }
                    paid_local += share;
                    remaining_payment -= share;
                }
                local_remaining -= qty.0.min(local_remaining);
            }
        }
        let _ = paid_local;
    }
    result
}

fn produce_goods(world: &mut World, dataset: &Dataset, power: &BTreeMap<Handle, i32>) {
    let handles: Vec<Handle> = world.facilities.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        let facility = match world.facilities.get(handle) {
            Some(facility) => facility.clone(),
            None => continue,
        };
        if facility.status != FacilityStatus::Operating {
            continue;
        }
        let recipe = &dataset.recipes[facility.recipe as usize];
        if recipe.output == "electricity" {
            continue;
        }
        let needs_power = recipe.weekly_inputs_lots.contains_key("electricity");
        let power_ratio = if needs_power {
            power.get(&handle).copied().unwrap_or(0)
        } else {
            10_000
        };
        let produced = run_facility(world, dataset, handle, power_ratio, power, false);
        let _ = produced;
    }
}

/// Run one facility's week and return its physical output.
fn run_facility(
    world: &mut World,
    dataset: &Dataset,
    handle: Handle,
    power_ratio_bps: i32,
    _power: &BTreeMap<Handle, i32>,
    utility: bool,
) -> MilliLots {
    let facility = match world.facilities.get(handle) {
        Some(facility) => facility.clone(),
        None => return MilliLots::ZERO,
    };
    let firm_handle = facility.firm;
    let Some(firm) = world.firms.get(firm_handle) else {
        return MilliLots::ZERO;
    };
    if !firm.operative() {
        return MilliLots::ZERO;
    }
    let recipe = &dataset.recipes[facility.recipe as usize];

    // Staffing limit: the minimum tier ratio, capped at full capacity.
    let mut staff_ratio = 10_000i32;
    for tier in 0..3 {
        let need = recipe.workers_low_mid_high[tier] * facility.blocks as i64;
        if need > 0 {
            let ratio = (facility.workers[tier] * 10_000 / need).min(10_000);
            staff_ratio = staff_ratio.min(ratio as i32);
        }
    }
    let mut ratio = facility.target_bps.min(staff_ratio).max(0);
    if !utility {
        ratio = ratio.min(power_ratio_bps);
    }
    if ratio <= 0 {
        update_facility_result(world, handle, MilliLots::ZERO, staff_ratio, true);
        return MilliLots::ZERO;
    }

    // Input limits, excluding electricity which is a service allocated above.
    let mut input_limit = ratio;
    for (product_id, weekly) in &recipe.weekly_inputs_lots {
        if product_id == "electricity" {
            continue;
        }
        let code = dataset.product_index[product_id];
        let need = weekly * facility.blocks as i64 * ratio as i64 / 10_000;
        if need <= 0 {
            continue;
        }
        let available = world
            .firms
            .get(firm_handle)
            .map(|f| f.stock(code, facility.region).0)
            .unwrap_or(0);
        if available < need {
            let attainable = (available * 10_000 / (weekly * facility.blocks as i64).max(1)) as i32;
            input_limit = input_limit.min(attainable);
        }
    }
    ratio = ratio.min(input_limit).max(0);

    // Ore deposits are finite.
    if facility.deposit_remaining.0 > 0 {
        let full = MilliLots::from_lots(recipe.weekly_output_lots * facility.blocks as i64);
        let wanted = full.0 * ratio as i64 / 10_000;
        if facility.deposit_remaining.0 < wanted {
            ratio = (facility.deposit_remaining.0 * 10_000 / full.0.max(1)) as i32;
        }
    } else if recipe.output == "ore" {
        ratio = 0;
    }

    if ratio <= 0 {
        update_facility_result(world, handle, MilliLots::ZERO, staff_ratio, true);
        return MilliLots::ZERO;
    }

    // Consume inputs once, carrying their actual moving-average cost.
    let mut material_cost = 0i64;
    for (product_id, weekly) in &recipe.weekly_inputs_lots {
        if product_id == "electricity" {
            continue;
        }
        let code = dataset.product_index[product_id];
        let need =
            MilliLots(weekly * facility.blocks as i64 * ratio as i64 / 10_000 * 1_000 / 1_000);
        if need.0 <= 0 {
            continue;
        }
        let firm = world.firms.get_mut(firm_handle).unwrap();
        let mut carry = 0i64;
        let cost = firm
            .stock_lot_mut(code, facility.region)
            .issue(need, &mut carry);
        material_cost += cost.0;
    }
    if material_cost > 0 {
        let _ = world.firms.get_mut(firm_handle).unwrap().book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Inventory, material_cost, "materials")
                .debit(Account::CostOfGoods, material_cost, "materials"),
        );
    }

    let mut output =
        MilliLots::from_lots(recipe.weekly_output_lots * facility.blocks as i64).scale_bps(ratio);
    // Seasonal yield applies to farm output only, before input planning in a full model.
    if recipe.industry == "agriculture" {
        let season = SEASONAL_YIELD_BPS[(world.tick.quarter().saturating_sub(1) % 4) as usize];
        output = output.scale_bps(season);
    }
    if output.0 > 0 {
        let code = dataset.product_index[&recipe.output];
        let firm = world.firms.get_mut(firm_handle).unwrap();
        firm.stock_lot_mut(code, facility.region)
            .receive(output, Cents(material_cost));
        let facility = world.facilities.get_mut(handle).unwrap();
        facility.deposit_remaining = facility.deposit_remaining.saturating_sub(output);
    }
    update_facility_result(
        world,
        handle,
        output,
        staff_ratio,
        ratio < input_limit.max(0),
    );
    output
}

fn update_facility_result(
    world: &mut World,
    handle: Handle,
    output: MilliLots,
    staff_ratio: i32,
    inputs_short: bool,
) {
    if let Some(facility) = world.facilities.get_mut(handle) {
        facility.last_output = output;
        facility.last_staff_ratio_bps = staff_ratio;
        facility.last_inputs_short = inputs_short;
    }
}

/// Services expire: unsold service output cannot be carried into a later week.
pub fn expire_services(world: &mut World, dataset: &Dataset) {
    let mut expiries: Vec<(Handle, u16, u8, i64)> = Vec::new();
    for (handle, firm) in world.firms.iter() {
        for ((product, region), lot) in &firm.inventory {
            if lot.qty.0 <= 0 {
                continue;
            }
            let kind = dataset.products[*product as usize].kind;
            if kind.is_service() || kind.is_financial() {
                expiries.push((handle, *product, *region, lot.cost_cents));
            }
        }
    }
    for (handle, product, region, cost) in expiries {
        let firm = match world.firms.get_mut(handle) {
            Some(firm) => firm,
            None => continue,
        };
        let lot = firm.stock_lot_mut(product, region);
        let written_off = lot.cost_cents.min(cost);
        lot.qty = MilliLots::ZERO;
        lot.cost_cents = 0;
        if written_off > 0 {
            let _ = firm.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Inventory, written_off, "service expiry")
                    .debit(Account::OperatingExpense, written_off, "service expiry"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::economy::seed;
    use crate::state::{ScenarioKind, ScenarioMode};

    #[test]
    fn production_consumes_inputs_exactly_once() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 21, ScenarioKind::Founder, ScenarioMode::Sandbox);
        let farm = dataset.product_index["food_raw"];
        let fuel = dataset.product_index["fuel"];
        let before_food: i64 = world
            .firms
            .iter()
            .map(|(_, firm)| firm.stock(farm, firm.region).0)
            .sum();
        let before_fuel: i64 = world
            .firms
            .iter()
            .map(|(_, firm)| firm.stock(fuel, firm.region).0)
            .sum();
        // Give every facility the workers it needs so staffing is not the binding limit.
        let assignments: Vec<(Handle, [i64; 3])> = world
            .facilities
            .iter()
            .map(|(handle, facility)| (handle, facility.full_workers(&dataset)))
            .collect();
        for (handle, full) in assignments {
            if let Some(f) = world.facilities.get_mut(handle) {
                f.workers = full;
            }
        }
        produce(&mut world, &dataset);
        let after_food: i64 = world
            .firms
            .iter()
            .map(|(_, firm)| firm.stock(farm, firm.region).0)
            .sum();
        let after_fuel: i64 = world
            .firms
            .iter()
            .map(|(_, firm)| firm.stock(fuel, firm.region).0)
            .sum();
        assert!(after_food > before_food, "food output increased");
        assert!(after_fuel < before_fuel, "fuel was consumed");
        for (_, firm) in world.firms.iter() {
            for ((_, _), lot) in &firm.inventory {
                assert!(lot.qty.0 >= 0, "inventory never negative");
            }
        }
    }

    #[test]
    fn missing_electricity_stops_output() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 22, ScenarioKind::Founder, ScenarioMode::Sandbox);
        // Remove every generator and gateway electricity offer.
        let generators: Vec<Handle> = world
            .facilities
            .iter()
            .filter(|(_, f)| dataset.recipes[f.recipe as usize].output == "electricity")
            .map(|(h, _)| h)
            .collect();
        for handle in generators {
            world.facilities.remove(handle);
        }
        world
            .gateways
            .retain(|gateway| gateway.product != dataset.product_index["electricity"]);
        let assignments: Vec<(Handle, [i64; 3])> = world
            .facilities
            .iter()
            .map(|(handle, facility)| (handle, facility.full_workers(&dataset)))
            .collect();
        for (handle, full) in assignments {
            if let Some(f) = world.facilities.get_mut(handle) {
                f.workers = full;
            }
        }
        produce(&mut world, &dataset);
        for (_, facility) in world.facilities.iter() {
            assert_eq!(
                facility.last_output,
                MilliLots::ZERO,
                "no power means no output"
            );
        }
    }
}
