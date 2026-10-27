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

/// The crew a plant needs this week, in basis points of its full crew. Farms run seasonal crews:
/// in the low-yield quarters they keep only the share of hands the yield needs, so wages follow
/// the harvest instead of staying fixed while output falls. Every other plant needs its full crew.
pub fn crew_need_bps(world: &World, recipe: &cw_data::Recipe) -> i64 {
    if recipe.industry == "agriculture" {
        let season = SEASONAL_YIELD_BPS[(world.tick.quarter().saturating_sub(1) % 4) as usize];
        season.min(10_000) as i64
    } else {
        10_000
    }
}

/// Produce the whole week: utilities first, then goods and services.
pub fn produce(world: &mut World, dataset: &Dataset) {
    let power = dispatch_electricity(world, dataset);
    // Goods produced this week are never usable this week, so production draws on the stock
    // that existed when production began. Without this snapshot, a facility earlier in the
    // slab could feed a later facility of the same firm inside one week.
    let mut opening = opening_stock(world);
    produce_goods(world, dataset, &power, &mut opening);
}

/// Physical input stock available to this week's production, keyed by firm, product and region.
fn opening_stock(world: &World) -> BTreeMap<(Handle, u16, u8), i64> {
    let mut stock = BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        for ((product, region), lot) in &firm.inventory {
            if lot.qty.0 > 0 {
                stock.insert((handle, *product, *region), lot.qty.0);
            }
        }
    }
    stock
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
        let produced = run_facility(world, dataset, handle, 10_000, &BTreeMap::new(), true, None);
        if produced.is_positive() {
            let facility = world.facilities.get(handle).expect("facility");
            // Payment must reach the owning firm; facility and firm handles are separate
            // slabs and their numeric values overlap.
            supply_by_region
                .entry(facility.region)
                .or_default()
                .push((facility.firm, produced));
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
            let firm = match world.firms.get(facility.firm) {
                Some(firm) if firm.operative() => firm,
                _ => continue,
            };
            // Planned demand in milli-lots, like every other physical quantity in the trade.
            let planned = MilliLots::from_lots(need * facility.blocks as i64)
                .scale_bps(facility.target_bps)
                .0;
            if planned <= 0 {
                continue;
            }
            // A buyer can only reserve what it actually holds in free cash.
            let affordable = if price > 0 {
                firm.book.cash().max(0) / price
            } else {
                planned
            };
            let requested = planned.min(affordable.saturating_mul(1_000)).max(0);
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
        let total_granted: i64 = allocation.iter().map(|(_, qty)| qty.0).sum();
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
        // Remove exactly the power consumers bought from producer stock. What is left is the
        // week's surplus, which the gateway may export instead of expiring unsold.
        if total_granted > 0 && !local_supply.is_empty() {
            let weights: Vec<(u64, i64)> = local_supply
                .iter()
                .map(|(handle, qty)| (handle.0, qty.0))
                .collect();
            let split = MilliLots::allocate(MilliLots(total_granted), &weights);
            for (index, (seller, _)) in local_supply.iter().enumerate() {
                let qty = split[index].1;
                if qty.0 <= 0 {
                    continue;
                }
                if let Some(firm) = world.firms.get_mut(*seller) {
                    firm.issue_stock(electricity, region_index as u8, qty);
                    // Power dispatched to the grid is a sale, for planning and the board.
                    firm.record_sale(region_index as u8, electricity, qty);
                }
            }
        }
    }
    result
}

fn produce_goods(
    world: &mut World,
    dataset: &Dataset,
    power: &BTreeMap<Handle, i32>,
    opening: &mut BTreeMap<(Handle, u16, u8), i64>,
) {
    let mut handles: Vec<Handle> = world.facilities.iter().map(|(handle, _)| handle).collect();
    // Services expire each week and are bought in the week they are used, so the plants that
    // make them run before the plants that use them: service makers with no service inputs
    // first, then service makers that use services, then everything else.
    let tier = |handle: &Handle| -> u8 {
        let Some(facility) = world.facilities.get(*handle) else {
            return 2;
        };
        let recipe = &dataset.recipes[facility.recipe as usize];
        let makes_service = dataset
            .product_index
            .get(&recipe.output)
            .is_some_and(|code| dataset.products[*code as usize].kind.is_service());
        if !makes_service {
            return 2;
        }
        let uses_service = recipe.weekly_inputs_lots.keys().any(|id| {
            id != "electricity"
                && dataset
                    .product_index
                    .get(id)
                    .is_some_and(|code| dataset.products[*code as usize].kind.is_service())
        });
        if uses_service { 1 } else { 0 }
    };
    handles.sort_by_key(|handle| (tier(handle), *handle));
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
        let produced = run_facility(
            world,
            dataset,
            handle,
            power_ratio,
            power,
            false,
            Some(opening),
        );
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
    mut opening: Option<&mut BTreeMap<(Handle, u16, u8), i64>>,
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
    // A storage facility holds goods and makes none; its crew and power are paid elsewhere.
    if recipe.storage_class.is_some() {
        update_facility_result(world, handle, MilliLots::ZERO, 10_000, false);
        return MilliLots::ZERO;
    }

    // Staffing limit: the minimum tier ratio, capped at full capacity. A seasonal farm crew is a
    // full crew for its season.
    let crew_bps = crew_need_bps(world, recipe);
    let mut staff_ratio = 10_000i32;
    for tier in 0..3 {
        let need = recipe.workers_low_mid_high[tier] * facility.blocks as i64 * crew_bps / 10_000;
        if need > 0 {
            let ratio = (facility.workers[tier] * 10_000 / need).min(10_000);
            staff_ratio = staff_ratio.min(ratio as i32);
        }
    }
    let mut ratio = facility.target_bps.min(staff_ratio).max(0);
    if let Some((cap, until)) = facility.outage
        && world.tick.0 < until.0
    {
        ratio = ratio.min(cap);
    }
    if !utility {
        ratio = ratio.min(power_ratio_bps);
    }
    // Adverse state conditions (grid reliability, low skill or broadband) are a capacity limit,
    // applied before inputs and deposits so the plant consumes and extracts only what it makes.
    ratio = (ratio as i64
        * crate::economy::operations::output_bps(world, dataset, handle, utility) as i64
        / 10_000) as i32;
    if ratio <= 0 {
        update_facility_result(world, handle, MilliLots::ZERO, staff_ratio, true);
        return MilliLots::ZERO;
    }

    // Sector focus: plants of the owner's primary or secondary type need fewer inputs.
    let focus_bps = crate::economy::operations::input_bps(world, dataset, handle);
    // Input limits, excluding electricity which is a service allocated above.
    let mut input_limit = ratio;
    for (product_id, weekly) in &recipe.weekly_inputs_lots {
        if product_id == "electricity" {
            continue;
        }
        let code = dataset.product_index[product_id];
        if dataset.products[code as usize].kind.is_service() {
            // Services cannot be stockpiled: they expire each week and markets clear after
            // production, so a non-electric service input is bought in the week it is used
            // from the finite outside service sector at the reference price. Cash is the limit.
            let unit_cost =
                weekly * facility.blocks as i64 * dataset.reference_price(code) * focus_bps as i64
                    / 10_000;
            if unit_cost > 0 {
                let cash = world
                    .firms
                    .get(firm_handle)
                    .map(|f| f.book.cash())
                    .unwrap_or(0)
                    .max(0);
                let attainable = (cash * 10_000 / unit_cost).min(10_000) as i32;
                input_limit = input_limit.min(attainable);
            }
            continue;
        }
        let need = MilliLots::from_lots(weekly * facility.blocks as i64)
            .scale_bps(focus_bps)
            .scale_bps(ratio);
        if need.0 <= 0 {
            continue;
        }
        let available = match opening.as_ref() {
            Some(stock) => stock
                .get(&(firm_handle, code, facility.region))
                .copied()
                .unwrap_or(0),
            None => world
                .firms
                .get(firm_handle)
                .map(|f| f.stock(code, facility.region).0)
                .unwrap_or(0),
        };
        if available < need.0 {
            let full = MilliLots::from_lots(weekly * facility.blocks as i64)
                .scale_bps(focus_bps)
                .0
                .max(1);
            input_limit = input_limit.min((available * 10_000 / full) as i32);
        }
    }
    ratio = ratio.min(input_limit).max(0);

    // Deposits are finite. When the deposit binds, the run rate rounds up and output is then
    // capped at exactly what remains, so a mine extracts its last lots instead of stranding a
    // rounding remainder underground.
    let mut deposit_cap: Option<MilliLots> = None;
    if facility.deposit_remaining.0 > 0 {
        let full = MilliLots::from_lots(recipe.weekly_output_lots * facility.blocks as i64);
        let wanted = full.0 * ratio as i64 / 10_000;
        if facility.deposit_remaining.0 < wanted {
            let denominator = full.0.max(1);
            let rounded_up =
                (facility.deposit_remaining.0 * 10_000 + denominator - 1) / denominator;
            ratio = ratio.min(rounded_up as i32);
            deposit_cap = Some(facility.deposit_remaining);
        }
    } else if recipe.depletes() {
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
        // Recipe quantities are whole lots; one lot is 1000 milli-lots. Availability and
        // consumption must use the same unit or the plant consumes a thousandth of its recipe.
        let need = MilliLots::from_lots(weekly * facility.blocks as i64)
            .scale_bps(focus_bps)
            .scale_bps(ratio);
        if need.0 <= 0 {
            continue;
        }
        if dataset.products[code as usize].kind.is_service() {
            // Same-week service purchase, expensed as incurred like electricity.
            let cost = need
                .value_at(Cents(dataset.reference_price(code)))
                .unwrap_or(Cents::ZERO)
                .0;
            let cash = world
                .firms
                .get(firm_handle)
                .map(|f| f.book.cash())
                .unwrap_or(0)
                .max(0);
            let paid = cost.min(cash);
            if paid > 0 {
                let _ = world.firms.get_mut(firm_handle).unwrap().book.commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Cash, paid, "service input")
                        .debit(Account::OperatingExpense, paid, "service input"),
                );
                // The corridor's own service producers are paid first; the outside sector
                // supplies only what they cannot.
                crate::economy::services::pay_for_service(
                    world,
                    dataset,
                    Some(firm_handle),
                    code,
                    paid,
                    "service input",
                );
            }
            continue;
        }
        if let Some(stock) = opening.as_deref_mut() {
            let entry = stock
                .entry((firm_handle, code, facility.region))
                .or_insert(0);
            *entry = (*entry - need.0).max(0);
        }
        let firm = world.firms.get_mut(firm_handle).unwrap();
        let cost = firm.issue_stock(code, facility.region, need);
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
    if let Some(cap) = deposit_cap {
        output = output.min(cap);
    }
    // Seasonal yield applies to farm output only, before input planning in a full model.
    let season = if recipe.industry == "agriculture" {
        SEASONAL_YIELD_BPS[(world.tick.quarter().saturating_sub(1) % 4) as usize]
    } else {
        10_000
    };
    output = output.scale_bps(season);
    // Supply-side events (crop loss, outages) reduce physical output, not household demand.
    if let Some(code) = dataset.product_index.get(&recipe.output) {
        output = output.scale_bps(world.supply_ratio_bps(facility.region, *code) as i32);
    }
    // Every product the plant makes, in its fixed yield split: the primary output first, then
    // each by-product at the same run rate, season and supply conditions.
    let mut made: Vec<(u16, MilliLots)> = Vec::new();
    if output.0 > 0 {
        made.push((dataset.product_index[&recipe.output], output));
    }
    for by in &recipe.by_products {
        let Some(code) = dataset.product_index.get(&by.product).copied() else {
            continue;
        };
        let qty = MilliLots::from_lots(by.weekly_lots * facility.blocks as i64)
            .scale_bps(ratio)
            .scale_bps(season)
            .scale_bps(world.supply_ratio_bps(facility.region, code) as i32);
        // A deposit that runs out stops the by-product with the primary.
        if qty.0 > 0 && output.0 > 0 {
            made.push((code, qty));
        }
    }
    if output.0 > 0 {
        // Utilities book output too, at zero carrying cost because fuel was expensed as
        // produced. Dispatch then removes exactly what consumers bought, leaving the surplus
        // available for the finite gateway export.
        if utility {
            let firm = world.firms.get_mut(firm_handle).unwrap();
            for (code, qty) in &made {
                firm.stock_lot_mut(*code, facility.region)
                    .receive(*qty, Cents(0));
            }
        } else {
            let shares = joint_cost_shares(dataset, &made, material_cost);
            let firm = world.firms.get_mut(firm_handle).unwrap();
            for ((code, qty), share) in made.iter().zip(shares) {
                firm.stock_lot_mut(*code, facility.region)
                    .receive(*qty, Cents(share));
            }
            // Capitalise the converted inputs into the new stock. The consumption posting
            // above already expensed them, so this reverses that expense; without it the
            // Inventory account and the lots' carrying cost drift apart and stock issues
            // drive the asset negative.
            if material_cost > 0 {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Inventory, material_cost, "capitalised production")
                        .credit(
                            Account::CostOfGoods,
                            material_cost,
                            "capitalised production",
                        ),
                );
            }
        }
        // Only a deposit-bearing facility (ore) consumes recoverable stock. Every other recipe
        // starts at zero and must stay at zero; subtracting output unconditionally drove the
        // field negative on ordinary plants and made the conservation check meaningless.
        let facility = world.facilities.get_mut(handle).unwrap();
        if !facility.deposit_remaining.is_zero() {
            facility.deposit_remaining =
                facility.deposit_remaining.max_zero().saturating_sub(output);
        }
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

/// Split the converted inputs' carrying cost across a plant's joint outputs by their value at
/// reference prices (the usual joint-cost rule). The shares always sum to the cost: the rounding
/// remainder goes to the primary output, the first entry.
fn joint_cost_shares(dataset: &Dataset, made: &[(u16, MilliLots)], cost: i64) -> Vec<i64> {
    if made.len() <= 1 || cost <= 0 {
        let mut shares = vec![0; made.len()];
        if let Some(first) = shares.first_mut() {
            *first = cost.max(0);
        }
        return shares;
    }
    let values: Vec<i64> = made
        .iter()
        .map(|(code, qty)| {
            qty.value_at(Cents(dataset.reference_price(*code)))
                .unwrap_or(Cents::ZERO)
                .0
                .max(0)
        })
        .collect();
    let total: i64 = values.iter().sum();
    if total <= 0 {
        let mut shares = vec![0; made.len()];
        shares[0] = cost;
        return shares;
    }
    let mut shares: Vec<i64> = values
        .iter()
        .map(|value| round_even_div(cost as i128 * *value as i128, total as i128) as i64)
        .collect();
    let assigned: i64 = shares.iter().sum();
    shares[0] += cost - assigned;
    shares
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

/// Perishable goods lose a share of the unsold stock each week, written off at carrying cost.
/// Spoilage destroys stock, never cash, and the loss is an operating expense. Stock that sits in
/// the firm's own dedicated storage in that region spoils at a fifth of the rate; the milli-lots lost
/// are recorded on the firm's storage book.
pub fn spoil_perishables(world: &mut World, dataset: &Dataset) {
    let rates: Vec<(u16, u32)> = dataset
        .products
        .iter()
        .enumerate()
        .filter(|(_, product)| product.spoilage_bps_weekly > 0)
        .map(|(code, product)| (code as u16, product.spoilage_bps_weekly))
        .collect();
    if rates.is_empty() {
        return;
    }
    let handles: Vec<Handle> = world.firms.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        // Share of each class's stock that sits in owned, dedicated space, by region.
        let owned: BTreeMap<(u8, cw_data::StorageClass), (i64, i64)> = match world.firms.get(handle)
        {
            Some(firm) => crate::economy::storage::uses(world, dataset, firm)
                .into_iter()
                .map(|u| ((u.region, u.class), (u.owned_milli, u.used_milli)))
                .collect(),
            None => continue,
        };
        let Some(firm) = world.firms.get_mut(handle) else {
            continue;
        };
        let mut written_off = 0i64;
        let mut lost_milli = 0i64;
        for (code, bps) in &rates {
            let class = dataset.storage_class_of(*code);
            let keys: Vec<u8> = firm
                .inventory
                .iter()
                .filter(|((p, _), lot)| p == code && lot.qty.0 > 0)
                .map(|((_, region), _)| *region)
                .collect();
            for region in keys {
                let held = firm.stock(*code, region);
                let (own, used) = class
                    .and_then(|c| owned.get(&(region, c)).copied())
                    .unwrap_or((0, 0));
                let rate = crate::economy::storage::spoilage_bps(*bps, own, used);
                // At least one milli-lot spoils while any remains, so small piles do not linger.
                let lost = MilliLots((held.0 * rate / 10_000).max(1)).min(held);
                written_off += firm.issue_stock(*code, region, lost).0;
                lost_milli += lost.0;
            }
        }
        firm.storage.last.spoiled_cents += written_off;
        firm.storage.last.spoiled_milli += lost_milli;
        firm.storage.spoiled_total_milli += lost_milli;
        if written_off > 0 {
            let _ = firm.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Inventory, written_off, "spoilage")
                    .debit(Account::OperatingExpense, written_off, "spoilage"),
            );
        }
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
        let fuel = dataset.product_index["fuel"];
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
        // Farm output is the physical production of this pass; the whole-world stock also
        // falls when a grocer consumes raw food in the same week.
        let food_produced: i64 = world
            .facilities
            .iter()
            .filter(|(_, facility)| dataset.recipes[facility.recipe as usize].output == "food_raw")
            .map(|(_, facility)| facility.last_output.0)
            .sum();
        let after_fuel: i64 = world
            .firms
            .iter()
            .map(|(_, firm)| firm.stock(fuel, firm.region).0)
            .sum();
        assert!(food_produced > 0, "food output increased");
        assert!(after_fuel < before_fuel, "fuel was consumed");
        for (_, firm) in world.firms.iter() {
            for ((_, _), lot) in &firm.inventory {
                assert!(lot.qty.0 >= 0, "inventory never negative");
            }
        }
    }

    #[test]
    fn only_ore_facilities_consume_a_deposit() {
        let dataset = Dataset::load().unwrap();
        let mut world = seed::build(&dataset, 23, ScenarioKind::Founder, ScenarioMode::Sandbox);
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
            if dataset.recipes[facility.recipe as usize].depletes() {
                assert!(
                    facility.deposit_remaining.0 >= 0,
                    "a mine cannot extract more than its deposit"
                );
            } else {
                assert_eq!(
                    facility.deposit_remaining,
                    MilliLots::ZERO,
                    "only a depleting extraction plant has a deposit"
                );
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
        // Every plant whose strategy draws electricity stops; strategies that use none (for
        // example infrastructure buildout or timber) are unaffected.
        let mut checked = 0;
        for (_, facility) in world.facilities.iter() {
            let recipe = &dataset.recipes[facility.recipe as usize];
            if recipe.weekly_inputs_lots.contains_key("electricity") {
                checked += 1;
                assert_eq!(
                    facility.last_output,
                    MilliLots::ZERO,
                    "no power means no output for {}",
                    recipe.id
                );
            }
        }
        assert!(checked > 20, "most strategies draw power");
    }
}
