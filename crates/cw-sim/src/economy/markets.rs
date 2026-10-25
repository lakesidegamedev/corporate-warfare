//! One-pass simultaneous market clearing.
//!
//! Books are per destination region, product and clearing stage. Buyers allocate desired
//! quantity across eligible offers by delivered-price/quality weight, reserve cash rounded
//! up to whole cents, and each scarce seller rations its stock pro rata with largest
//! remainders. There is no second pass, so no firm-order advantage and no solver
//! convergence risk. Returned cash is released after settlement; unfilled demand is
//! explained to the player rather than silently re-spent.

use std::collections::BTreeMap;

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::{Cents, allocate_cents, round_even_div};
use cw_core::quantity::MilliLots;
use cw_data::Dataset;

use crate::state::{FacilityStatus, World};

/// Seller offer used by the allocation core.
#[derive(Clone, Debug)]
pub struct Offer {
    pub seller: u64,
    pub qty: i64,
    pub price_cents: i64,
    pub quality_bps: i64,
    pub brand_bps: i64,
}

/// Buyer request used by the allocation core.
#[derive(Clone, Debug)]
pub struct Request {
    pub buyer: u64,
    pub qty: i64,
    pub budget_cents: i64,
    pub brand_sensitive: bool,
}

/// One filled trade: (buyer, seller, milli-lots).
pub type Fill = (u64, u64, i64);

/// Core allocation. `reference_price_cents` is the pinned positive reference price.
pub fn allocate(reference_price_cents: i64, offers: &[Offer], requests: &[Request]) -> Vec<Fill> {
    let mut result: Vec<Fill> = Vec::new();
    let reference = reference_price_cents.max(1);
    // Bids per (buyer index, seller index) in milli-lots.
    let mut bids: Vec<Vec<i64>> = vec![vec![0; offers.len()]; requests.len()];
    for (buyer_index, request) in requests.iter().enumerate() {
        if request.qty <= 0 {
            continue;
        }
        let mut weights: Vec<i64> = Vec::with_capacity(offers.len());
        for offer in offers {
            if offer.qty <= 0 || offer.price_cents <= 0 {
                weights.push(0);
                continue;
            }
            let brand = if request.brand_sensitive {
                offer.brand_bps
            } else {
                10_000
            };
            let raw = round_even_div(
                offer.quality_bps as i128 * brand as i128 * reference as i128,
                offer.price_cents as i128 * 10_000,
            ) as i64;
            weights.push(raw.clamp(1_000, 100_000));
        }
        let total_weight: i64 = weights.iter().sum();
        if total_weight <= 0 {
            continue;
        }
        let shares = allocate_cents(
            Cents(request.qty),
            &offers
                .iter()
                .enumerate()
                .map(|(index, offer)| (offer.seller ^ (index as u64) << 32, weights[index]))
                .collect::<Vec<_>>(),
        );
        for (index, (_, qty)) in shares.iter().enumerate() {
            bids[buyer_index][index] = qty.0.max(0);
        }
        // Cash cap: reserve the sum of per-fill costs rounded up to whole cents.
        let reserved: i64 = (0..offers.len())
            .map(|index| ceil_cost(offers[index].price_cents, bids[buyer_index][index]))
            .sum();
        if reserved > request.budget_cents && reserved > 0 {
            // Scale every bid by the affordable fraction, then trim rounding dust.
            for bid in bids[buyer_index].iter_mut() {
                *bid = (*bid as i128 * request.budget_cents as i128 / reserved as i128) as i64;
            }
            let mut guard = offers.len() + 2;
            loop {
                if guard == 0 {
                    break;
                }
                let total: i64 = (0..offers.len())
                    .map(|index| ceil_cost(offers[index].price_cents, bids[buyer_index][index]))
                    .sum();
                if total <= request.budget_cents {
                    break;
                }
                let Some(victim) = (0..offers.len())
                    .filter(|index| bids[buyer_index][*index] > 0)
                    .max_by_key(|index| {
                        ceil_cost(offers[*index].price_cents, bids[buyer_index][*index])
                    })
                else {
                    break;
                };
                bids[buyer_index][victim] -= 1;
                guard -= 1;
            }
        }
    }

    // Seller-side rationing: each seller distributes its stock across its bids.
    for (seller_index, offer) in offers.iter().enumerate() {
        let total_bids: i64 = bids.iter().map(|row| row[seller_index]).sum();
        if total_bids <= 0 {
            continue;
        }
        if total_bids <= offer.qty {
            for (buyer_index, row) in bids.iter().enumerate() {
                if row[seller_index] > 0 {
                    result.push((requests[buyer_index].buyer, offer.seller, row[seller_index]));
                }
            }
            continue;
        }
        let weights: Vec<(u64, i64)> = bids
            .iter()
            .enumerate()
            .map(|(buyer_index, row)| (requests[buyer_index].buyer, row[seller_index]))
            .collect();
        let rationed = allocate_cents(Cents(offer.qty), &weights);
        for (buyer_index, (_, qty)) in rationed.iter().enumerate() {
            if qty.0 > 0 {
                result.push((requests[buyer_index].buyer, offer.seller, qty.0));
            }
        }
    }
    result.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    result
}

fn ceil_cost(price_cents: i64, milli_lots: i64) -> i64 {
    if milli_lots <= 0 {
        return 0;
    }
    let numerator = price_cents as i128 * milli_lots as i128;
    ((numerator + 999) / 1000) as i64
}

/// Clear every spot book for the week: industrial inputs, household demand, procurement.
pub fn clear_week(world: &mut World, dataset: &Dataset) {
    clear_intermediate(world, dataset);
    clear_households(world, dataset);
    clear_government(world, dataset);
    clear_public_works(world, dataset);
}

/// Background public works buy a finite weekly volume of construction service at the reference
/// price. Without it, project firms have no demand between private builds, pay full payroll and
/// fail. The buyer is the finite outside account, never minted money.
fn clear_public_works(world: &mut World, dataset: &Dataset) {
    let Some(service) = dataset.product_code("construction") else {
        return;
    };
    const LOTS_PER_REGION: i64 = 18;
    for region in 0..world.regions.len() {
        let sellers: Vec<Handle> = world
            .firms
            .iter()
            .filter(|(_, firm)| firm.region == region as u8 && firm.operative())
            .filter(|(_, firm)| firm.stock(service, region as u8).0 > 0)
            .map(|(handle, _)| handle)
            .collect();
        let mut remaining = MilliLots::from_lots(LOTS_PER_REGION);
        for seller in sellers {
            if remaining.0 <= 0 {
                break;
            }
            let available = world
                .firm(seller)
                .map(|f| f.stock(service, region as u8))
                .unwrap_or_default();
            let qty = available.min(remaining);
            if qty.0 <= 0 {
                continue;
            }
            let cost = qty
                .value_at(Cents(dataset.reference_price(service)))
                .unwrap_or(Cents::ZERO)
                .0;
            let cash = world.outside.gateway_book.cash().max(0);
            let paid = cost.min(cash);
            if paid <= 0 {
                break;
            }
            let (removed, carrying) = remove_stock(world, seller, service, region as u8, qty);
            if !removed {
                continue;
            }
            if let Some(firm) = world.firms.get_mut(seller) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, paid, "public works")
                        .credit(Account::Revenue, paid, "public works")
                        .credit(Account::Inventory, carrying, "public works cogs")
                        .debit(Account::CostOfGoods, carrying, "public works cogs"),
                );
                firm.last_week_units_sold
                    .insert((region as u8, service), qty);
            }
            let _ = world.outside.gateway_book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, paid, "public works")
                    .debit(Account::OperatingExpense, paid, "public works"),
            );
            remaining = MilliLots(remaining.0 - qty.0);
        }
    }
}

fn clear_intermediate(world: &mut World, dataset: &Dataset) {
    type BuyerNeed = Vec<(Handle, i64, i64)>;
    let mut needed: BTreeMap<(u8, u16), BuyerNeed> = BTreeMap::new();
    for (_facility_handle, facility) in world.facilities.iter() {
        if facility.status != FacilityStatus::Operating {
            continue;
        }
        let firm_handle = facility.firm;
        let Some(firm) = world.firms.get(firm_handle) else {
            continue;
        };
        if !firm.operative() {
            continue;
        }
        let recipe = &dataset.recipes[facility.recipe as usize];
        for (product_id, weekly) in &recipe.weekly_inputs_lots {
            if product_id == "electricity" {
                continue;
            }
            let code = dataset.product_index[product_id];
            let cover = firm.policies.input_cover_weeks.max(1) as i64;
            let target = weekly * facility.blocks as i64 * cover;
            let have = firm.stock(code, facility.region).whole_lots_floor();
            let want = (target - have).max(0);
            if want <= 0 {
                continue;
            }
            let price = firm
                .policies
                .prices
                .get(&code)
                .copied()
                .unwrap_or_else(|| dataset.reference_price(code));
            let budget = (firm.book.cash().max(0) - firm.reserved_cash_cents)
                .max(0)
                .min(want * price * 2);
            if budget <= 0 {
                continue;
            }
            needed.entry((facility.region, code)).or_default().push((
                firm_handle,
                want * 1_000,
                budget,
            ));
        }
    }

    for ((region, product), buyers) in needed {
        // Combine multiple facilities of one firm into one buyer.
        let mut merged: BTreeMap<Handle, (i64, i64)> = BTreeMap::new();
        for (firm, qty, budget) in buyers {
            let entry = merged.entry(firm).or_insert((0, 0));
            entry.0 += qty;
            entry.1 += budget;
        }
        let requests: Vec<Request> = merged
            .iter()
            .map(|(firm, (qty, budget))| Request {
                buyer: firm.0,
                qty: *qty,
                budget_cents: *budget,
                brand_sensitive: false,
            })
            .collect();

        let mut offers: Vec<Offer> = Vec::new();
        let mut seller_handles: Vec<OfferSource> = Vec::new();
        for (handle, firm) in world.firms.iter() {
            if firm.region != region || !firm.operative() {
                continue;
            }
            let qty = firm.stock(product, region).0;
            if qty <= 0 {
                continue;
            }
            let price = firm
                .policies
                .prices
                .get(&product)
                .copied()
                .unwrap_or_else(|| dataset.reference_price(product));
            offers.push(Offer {
                seller: handle.0,
                qty,
                price_cents: price.max(1),
                quality_bps: 10_000,
                brand_bps: 10_000,
            });
            seller_handles.push(OfferSource::Firm(handle));
        }
        for (index, gateway) in world.gateways.iter().enumerate() {
            if gateway.region != region || gateway.product != product {
                continue;
            }
            offers.push(Offer {
                seller: GATEWAY_ID_BASE + index as u64,
                qty: gateway.capacity_lots * 1_000,
                price_cents: gateway.sell_price_cents,
                quality_bps: 10_000,
                brand_bps: 10_000,
            });
            seller_handles.push(OfferSource::Gateway(index));
        }
        if offers.is_empty() {
            continue;
        }
        let fills = allocate(dataset.reference_price(product), &offers, &requests);
        settle_fills(world, dataset, product, region, &fills, &seller_handles);
    }
    // Residual surplus can leave the corridor through the gateway bid even when no local buyer
    // exists, but only above the seller's own input cover. This is the export channel.
    let gateways: Vec<(u8, u16)> = world
        .gateways
        .iter()
        .map(|gateway| (gateway.region, gateway.product))
        .collect();
    for (region, product) in gateways {
        sell_surplus_to_gateway(world, dataset, region, product);
    }
}

fn sell_surplus_to_gateway(world: &mut World, dataset: &Dataset, region: u8, product: u16) {
    let Some(index) = world
        .gateways
        .iter()
        .position(|gateway| gateway.region == region && gateway.product == product)
    else {
        return;
    };
    let gateway = world.gateways[index].clone();
    if gateway.buy_price_cents <= 0 {
        return;
    }
    let mut remaining = MilliLots::from_lots(gateway.capacity_lots);
    if remaining.0 <= 0 {
        return;
    }
    let sellers: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(_, firm)| firm.region == region && firm.operative())
        .filter(|(_, firm)| firm.stock(product, region).0 > 0)
        .map(|(handle, _)| handle)
        .collect();
    for seller in sellers {
        if remaining.0 <= 0 {
            break;
        }
        let Some(firm) = world.firms.get(seller) else {
            continue;
        };
        let cover_weeks = firm.policies.input_cover_weeks.max(1) as i64;
        let mut own_need = 0i64;
        for facility_handle in &firm.facilities {
            let Some(facility) = world.facilities.get(*facility_handle) else {
                continue;
            };
            if facility.region != region {
                continue;
            }
            let recipe = &dataset.recipes[facility.recipe as usize];
            if let Some(weekly) = recipe
                .weekly_inputs_lots
                .iter()
                .find(|(id, _)| dataset.product_index[*id] == product)
                .map(|(_, qty)| *qty)
            {
                own_need += weekly * facility.blocks as i64 * cover_weeks;
            }
        }
        // Stock promised to active sell contracts is not surplus and cannot be exported.
        let committed: i64 = world
            .contracts
            .iter()
            .filter(|(_, contract)| {
                contract.seller == seller
                    && contract.product == product
                    && contract.from_region == region
                    && contract.status == crate::state::ContractStatus::Active
            })
            .map(|(_, contract)| contract.quantity_weekly.lots())
            .sum();
        let available = firm.stock(product, region).lots();
        let exportable = (available - own_need - committed).max(0);
        if exportable <= 0 {
            continue;
        }
        let qty = MilliLots::from_lots(exportable).min(remaining);
        let cost = qty
            .value_at(Cents(gateway.buy_price_cents.max(1)))
            .unwrap_or(Cents::ZERO)
            .0;
        let gateway_cash = world.outside.gateway_book.cash().max(0);
        let paid = cost.min(gateway_cash);
        if paid <= 0 {
            break;
        }
        let (removed, carrying) = remove_stock(world, seller, product, region, qty);
        if !removed {
            continue;
        }
        if let Some(firm) = world.firms.get_mut(seller) {
            let _ = firm.book.commit(
                world.tick,
                Txn::new()
                    .debit(Account::Cash, paid, "export")
                    .credit(Account::Revenue, paid, "export")
                    .credit(Account::Inventory, carrying, "export cogs")
                    .debit(Account::CostOfGoods, carrying, "export cogs"),
            );
            firm.last_week_units_sold.insert((region, product), qty);
        }
        let _ = world.outside.gateway_book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, paid, "export purchase")
                .debit(Account::Inventory, paid, "export purchase"),
        );
        remaining = MilliLots(remaining.0 - qty.0);
    }
}

#[derive(Clone, Copy, Debug)]
enum OfferSource {
    Firm(Handle),
    Gateway(usize),
}

fn clear_households(world: &mut World, dataset: &Dataset) {
    let consumer_ids = [
        "food_retail",
        "goods_retail",
        "care",
        "software",
        "telecom",
        "lease",
    ];
    for region_index in 0..world.regions.len() {
        let income = world.regions[region_index]
            .households
            .last_income_cents
            .max(0);
        let budget_total = income * 90 / 100;
        let weights = world.regions[region_index].households.weights.clone();
        let baseline = world.regions[region_index].households.baseline_lots.clone();
        let last_prices = world.regions[region_index]
            .households
            .last_sale_price_cents
            .clone();
        for id in consumer_ids {
            let product = dataset.product_index[id];
            let weight = weights.get(&product).copied().unwrap_or(0);
            if weight <= 0 {
                continue;
            }
            let category_budget = budget_total * weight / 10_000;
            if category_budget <= 0 {
                continue;
            }
            let reference = dataset.reference_price(product).max(1);
            let last = last_prices
                .get(&product)
                .copied()
                .unwrap_or(reference)
                .max(1);
            let ratio = (reference * 10_000 / last).clamp(5_000, 20_000);
            let base = baseline.get(&product).copied().unwrap_or(0);
            let requested = (base * ratio / 10_000).max(0) * 1_000;
            if requested <= 0 {
                continue;
            }
            let mut offers: Vec<Offer> = Vec::new();
            let mut sources: Vec<OfferSource> = Vec::new();
            for (handle, firm) in world.firms.iter() {
                if firm.region != region_index as u8 || !firm.operative() {
                    continue;
                }
                let qty = firm.stock(product, region_index as u8).0;
                if qty <= 0 {
                    continue;
                }
                let price = firm
                    .policies
                    .prices
                    .get(&product)
                    .copied()
                    .unwrap_or(reference);
                offers.push(Offer {
                    seller: handle.0,
                    qty,
                    price_cents: price.max(1),
                    quality_bps: firm
                        .quality_bps
                        .get(&product)
                        .copied()
                        .unwrap_or_else(|| crate::economy::research::quality(firm.tech_level, 0))
                        as i64,
                    brand_bps: firm
                        .brand_bps
                        .get(&(region_index as u8, product))
                        .copied()
                        .unwrap_or(10_000) as i64,
                });
                sources.push(OfferSource::Firm(handle));
            }
            if offers.is_empty() {
                // Unspent budget stays with the household, never with a phantom seller.
                continue;
            }
            let requests = vec![Request {
                buyer: u64::MAX,
                qty: requested,
                budget_cents: category_budget,
                brand_sensitive: true,
            }];
            let fills = allocate(reference, &offers, &requests);
            let spent = settle_household_fills(
                world,
                product,
                region_index as u8,
                &fills,
                &sources,
                dataset,
            );
            let unspent = (category_budget - spent).max(0);
            let region = &mut world.regions[region_index];
            region.households.savings_cents += unspent;
            let mut weight_sum = 0i64;
            let mut price_sum = 0i64;
            for (_, seller, qty) in &fills {
                if let Some(offer) = offers.iter().find(|offer| offer.seller == *seller) {
                    weight_sum += qty;
                    price_sum += qty * offer.price_cents;
                }
            }
            if weight_sum > 0 {
                let average = price_sum / weight_sum;
                region
                    .households
                    .last_sale_price_cents
                    .insert(product, average.max(1));
                region.households.spend_cents.insert(product, spent);
            } else {
                region.households.spend_cents.insert(product, 0);
            }
        }
    }
}

fn source_id(source: OfferSource) -> u64 {
    match source {
        OfferSource::Firm(handle) => handle.0,
        OfferSource::Gateway(index) => GATEWAY_ID_BASE + index as u64,
    }
}

fn clear_government(world: &mut World, dataset: &Dataset) {
    // Defence procurement and care episodes are funded from fiscal budgets. Each budget is a
    // weekly national ceiling, not a per-region allowance, so spending is consumed as the
    // regions are served.
    let defence = dataset.product_index["defense"];
    let care = dataset.product_index["care"];
    let procurement = world.government.procurement_budget_cents.max(0);
    let healthcare = world.government.healthcare_budget_cents.max(0);
    for (product, budget) in [(defence, procurement), (care, healthcare)] {
        let mut remaining = budget;
        for region in 0..world.regions.len() {
            if remaining <= 0 {
                break;
            }
            let mut offers = Vec::new();
            let mut sources = Vec::new();
            for (handle, firm) in world.firms.iter() {
                if firm.region != region as u8 || !firm.operative() {
                    continue;
                }
                let qty = firm.stock(product, region as u8).0;
                if qty <= 0 {
                    continue;
                }
                let price = firm
                    .policies
                    .prices
                    .get(&product)
                    .copied()
                    .unwrap_or_else(|| dataset.reference_price(product));
                offers.push(Offer {
                    seller: handle.0,
                    qty,
                    price_cents: price.max(1),
                    quality_bps: 10_000,
                    brand_bps: 10_000,
                });
                sources.push(OfferSource::Firm(handle));
            }
            if offers.is_empty() || remaining <= 0 {
                continue;
            }
            let reference = dataset.reference_price(product);
            let affordable = remaining / reference.max(1);
            if affordable <= 0 {
                continue;
            }
            let requests = vec![Request {
                buyer: u64::MAX - 1,
                qty: affordable * 1_000,
                budget_cents: remaining,
                brand_sensitive: false,
            }];
            let fills = allocate(reference, &offers, &requests);
            let spent =
                settle_government_fills(world, product, region as u8, &fills, &sources, dataset);
            remaining = (remaining - spent).max(0);
        }
    }
}

fn settle_fills(
    world: &mut World,
    dataset: &Dataset,
    product: u16,
    region: u8,
    fills: &[Fill],
    sources: &[OfferSource],
) {
    for (buyer, seller, qty) in fills {
        let Some(buyer_handle) = handle_of(*buyer) else {
            continue;
        };
        if let OfferSource::Firm(seller_handle) = sources
            .iter()
            .find(|source| source_id(**source) == *seller)
            .copied()
            .unwrap_or(OfferSource::Firm(Handle::INVALID))
            && seller_handle == buyer_handle
        {
            continue;
        }
        let Some(source) = sources
            .iter()
            .find(|source| source_id(**source) == *seller)
            .copied()
        else {
            continue;
        };
        let qty = MilliLots(*qty);
        if qty.0 <= 0 {
            continue;
        }
        match source {
            OfferSource::Firm(seller_handle) => {
                let price = world
                    .firms
                    .get(seller_handle)
                    .and_then(|firm| firm.policies.prices.get(&product).copied())
                    .unwrap_or_else(|| dataset.reference_price(product));
                let cost = qty.value_at(Cents(price.max(1))).unwrap_or(Cents::ZERO);
                if trade_between(
                    world,
                    buyer_handle,
                    seller_handle,
                    product,
                    region,
                    qty,
                    cost,
                ) && let Some(seller_firm) = world.firms.get_mut(seller_handle)
                {
                    seller_firm
                        .last_week_units_sold
                        .insert((region, product), qty);
                }
            }
            OfferSource::Gateway(index) => {
                let gateway = world.gateways[index].clone();
                let cost = qty
                    .value_at(Cents(gateway.sell_price_cents.max(1)))
                    .unwrap_or(Cents::ZERO);
                let cash = world
                    .firms
                    .get(buyer_handle)
                    .map(|f| f.book.cash())
                    .unwrap_or(0);
                let paid = cost.0.min(cash.max(0));
                if paid <= 0 {
                    continue;
                }
                let _ = world.firms.get_mut(buyer_handle).unwrap().book.commit(
                    world.tick,
                    Txn::new().credit(Account::Cash, paid, "import").debit(
                        Account::Inventory,
                        paid,
                        "import",
                    ),
                );
                world
                    .firms
                    .get_mut(buyer_handle)
                    .unwrap()
                    .stock_lot_mut(product, region)
                    .receive(qty, Cents(paid));
                let _ = world.outside.gateway_book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, paid, "export receipt")
                        .credit(Account::Revenue, paid, "export receipt"),
                );
            }
        }
    }
}

fn settle_household_fills(
    world: &mut World,
    product: u16,
    region: u8,
    fills: &[Fill],
    sources: &[OfferSource],
    dataset: &Dataset,
) -> i64 {
    let mut spent = 0i64;
    for (_, seller, qty) in fills {
        let Some(source) = sources
            .iter()
            .find(|source| source_id(**source) == *seller)
            .copied()
        else {
            continue;
        };
        let qty = MilliLots(*qty);
        if qty.0 <= 0 {
            continue;
        }
        if let OfferSource::Firm(seller_handle) = source {
            let price = world
                .firms
                .get(seller_handle)
                .and_then(|firm| firm.policies.prices.get(&product).copied())
                .unwrap_or_else(|| dataset.reference_price(product));
            let cost = qty.value_at(Cents(price.max(1))).unwrap_or(Cents::ZERO);
            // Household cash is outside cash; the sale posts revenue and removes stock.
            let (removed, carrying) = remove_stock(world, seller_handle, product, region, qty);
            if !removed {
                continue;
            }
            if let Some(firm) = world.firms.get_mut(seller_handle) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, cost.0, "household sale")
                        .credit(Account::Revenue, cost.0, "household sale")
                        .credit(Account::Inventory, carrying, "cogs")
                        .debit(Account::CostOfGoods, carrying, "cogs"),
                );
                firm.last_week_units_sold.insert((region, product), qty);
            }
            spent += cost.0;
        }
    }
    spent
}

fn settle_government_fills(
    world: &mut World,
    product: u16,
    region: u8,
    fills: &[Fill],
    sources: &[OfferSource],
    dataset: &Dataset,
) -> i64 {
    let mut spent = 0i64;
    for (_, seller, qty) in fills {
        let Some(source) = sources
            .iter()
            .find(|source| source_id(**source) == *seller)
            .copied()
        else {
            continue;
        };
        let qty = MilliLots(*qty);
        if qty.0 <= 0 {
            continue;
        }
        if let OfferSource::Firm(seller_handle) = source {
            let price = world
                .firms
                .get(seller_handle)
                .and_then(|firm| firm.policies.prices.get(&product).copied())
                .unwrap_or_else(|| dataset.reference_price(product));
            let cost = qty.value_at(Cents(price.max(1))).unwrap_or(Cents::ZERO);
            let (removed, carrying) = remove_stock(world, seller_handle, product, region, qty);
            if !removed {
                continue;
            }
            let available = world.government.book.cash().max(0);
            let paid = cost.0.min(available);
            if paid <= 0 {
                continue;
            }
            let _ = world.government.book.commit(
                world.tick,
                Txn::new().credit(Account::Cash, paid, "procurement").debit(
                    Account::OperatingExpense,
                    paid,
                    "procurement",
                ),
            );
            if let Some(firm) = world.firms.get_mut(seller_handle) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, paid, "government sale")
                        .credit(Account::Revenue, paid, "government sale")
                        .credit(Account::Inventory, carrying, "cogs")
                        .debit(Account::CostOfGoods, carrying, "cogs"),
                );
                // Procurement is a real sale and belongs in the unit-sales record.
                firm.last_week_units_sold.insert((region, product), qty);
            }
            spent += paid;
        }
    }
    spent
}

fn trade_between(
    world: &mut World,
    buyer: Handle,
    seller: Handle,
    product: u16,
    region: u8,
    qty: MilliLots,
    cost: Cents,
) -> bool {
    let cash = world
        .firms
        .get(buyer)
        .map(|firm| firm.book.cash())
        .unwrap_or(0);
    let paid = cost.0.min(cash.max(0));
    if paid <= 0 {
        return false;
    }
    let (removed, carrying) = remove_stock(world, seller, product, region, qty);
    if !removed {
        return false;
    }
    let _ = world.firms.get_mut(buyer).unwrap().book.commit(
        world.tick,
        Txn::new().credit(Account::Cash, paid, "purchase").debit(
            Account::Inventory,
            paid,
            "purchase",
        ),
    );
    world
        .firms
        .get_mut(buyer)
        .unwrap()
        .stock_lot_mut(product, region)
        .receive(qty, Cents(paid));
    let _ = world.firms.get_mut(seller).unwrap().book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, paid, "sale")
            .credit(Account::Revenue, paid, "sale")
            .credit(Account::Inventory, carrying, "cogs")
            .debit(Account::CostOfGoods, carrying, "cogs"),
    );
    true
}

fn remove_stock(
    world: &mut World,
    firm: Handle,
    product: u16,
    region: u8,
    qty: MilliLots,
) -> (bool, i64) {
    let Some(entity) = world.firms.get_mut(firm) else {
        return (false, 0);
    };
    let lot = entity.stock_lot_mut(product, region);
    if lot.qty.0 < qty.0 || qty.0 <= 0 {
        return (false, 0);
    }
    let mut carry = 0i64;
    let cost = lot.issue(qty, &mut carry);
    (true, cost.0)
}

/// Gateway sellers are outside the entity namespace. Real handles never set the high bit
/// because a slot generation would have to reach two billion reuses.
const GATEWAY_ID_BASE: u64 = 1 << 63;

fn handle_of(raw: u64) -> Option<Handle> {
    if raw == 0 || raw >= GATEWAY_ID_BASE {
        None
    } else {
        Some(Handle(raw))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offer(seller: u64, qty: i64, price: i64) -> Offer {
        Offer {
            seller,
            qty,
            price_cents: price,
            quality_bps: 10_000,
            brand_bps: 10_000,
        }
    }

    #[test]
    fn worked_example_a_rationing_is_order_independent() {
        // One seller with 100 lots; two equally priced buyers request 80 and 40.
        let offers = vec![offer(1, 100_000, 1_000)];
        let requests = vec![
            Request {
                buyer: 10,
                qty: 80_000,
                budget_cents: 80_000,
                brand_sensitive: false,
            },
            Request {
                buyer: 20,
                qty: 40_000,
                budget_cents: 40_000,
                brand_sensitive: false,
            },
        ];
        let fills = allocate(1_000, &offers, &requests);
        let total: i64 = fills.iter().map(|(_, _, qty)| qty).sum();
        assert_eq!(total, 100_000);
        let first = fills.iter().find(|(buyer, _, _)| *buyer == 10).unwrap().2;
        let second = fills.iter().find(|(buyer, _, _)| *buyer == 20).unwrap().2;
        // 66.667 and 33.333 lots; residual milli-lot goes to the lower stable buyer ID.
        assert_eq!(first, 66_667);
        assert_eq!(second, 33_333);
        // Reverse the request order: identical result.
        let mut reversed = requests;
        reversed.reverse();
        let fills = allocate(1_000, &offers, &reversed);
        assert_eq!(
            fills.iter().find(|(buyer, _, _)| *buyer == 10).unwrap().2,
            66_667
        );
    }

    #[test]
    fn worked_example_b_equal_sellers_split_the_budget() {
        let offers = vec![offer(1, 100_000, 1_000), offer(2, 100_000, 1_000)];
        let requests = vec![Request {
            buyer: 100,
            qty: 100_000,
            budget_cents: 100_000,
            brand_sensitive: true,
        }];
        let fills = allocate(1_000, &offers, &requests);
        assert_eq!(fills.len(), 2);
        for (_, _, qty) in &fills {
            assert_eq!(*qty, 50_000);
        }
        let revenue: i64 = fills.iter().map(|(_, _, qty)| qty * 1_000 / 1_000).sum();
        assert_eq!(revenue, 100_000);
    }

    #[test]
    fn worked_example_c_budget_caps_the_buy() {
        let offers = vec![offer(1, 100_000, 1_000), offer(2, 100_000, 1_000)];
        let requests = vec![Request {
            buyer: 100,
            qty: 100_000,
            budget_cents: 60_000,
            brand_sensitive: true,
        }];
        let fills = allocate(1_000, &offers, &requests);
        let total: i64 = fills.iter().map(|(_, _, qty)| qty).sum();
        assert_eq!(total, 60_000);
        for (_, _, qty) in &fills {
            assert_eq!(*qty, 30_000);
        }
    }

    #[test]
    fn worked_example_d_no_affordable_offer_means_no_trade() {
        let offers = vec![offer(1, 100_000, 1_000)];
        let requests = vec![Request {
            buyer: 100,
            qty: 100_000,
            budget_cents: 0,
            brand_sensitive: true,
        }];
        let fills = allocate(1_000, &offers, &requests);
        assert!(fills.is_empty(), "no budget dust trade");
    }

    #[test]
    fn generators_can_export_surplus_power() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            195,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        crate::economy::production::produce(&mut world, &dataset);
        let electricity = dataset.product_index["electricity"];
        let generator = world
            .firms
            .iter()
            .find(|(_, firm)| firm.industry == 2 && firm.stock(electricity, firm.region).0 > 0)
            .map(|(handle, _)| handle)
            .unwrap_or_else(|| {
                let rows: Vec<String> = world
                    .firms
                    .iter()
                    .map(|(_, firm)| {
                        format!(
                            "{} ind={} stock={} out={:?}",
                            firm.name,
                            firm.industry,
                            firm.stock(electricity, firm.region).lots(),
                            firm.facilities
                                .iter()
                                .filter_map(|h| world.facilities.get(*h))
                                .map(|f| f.last_output.lots())
                                .collect::<Vec<_>>()
                        )
                    })
                    .collect();
                panic!("a generator with output: {}", rows.join(" | "));
            });
        let region = world.firm(generator).unwrap().region;
        let before = world.firm(generator).unwrap().stock(electricity, region);
        let revenue_before = -world
            .firm(generator)
            .unwrap()
            .book
            .balance(Account::Revenue);
        sell_surplus_to_gateway(&mut world, &dataset, region, electricity);
        let after = world.firm(generator).unwrap().stock(electricity, region);
        let revenue_after = -world
            .firm(generator)
            .unwrap()
            .book
            .balance(Account::Revenue);
        assert!(after.0 < before.0, "surplus power leaves");
        assert!(revenue_after > revenue_before, "the gateway pays for it");
    }

    #[test]
    fn gateway_export_never_sells_stock_promised_to_contracts() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            193,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        let product = dataset.product_index["food_raw"];
        let seller = world
            .firms
            .iter()
            .find(|(_, firm)| firm.stock(product, firm.region).0 > 4_000)
            .map(|(handle, _)| handle)
            .expect("a producer with stock");
        let region = world.firm(seller).unwrap().region;
        let available = world.firm(seller).unwrap().stock(product, region);
        // Commit half the stock to a real contract, then export the surplus.
        let buyer = world
            .firms
            .iter()
            .find(|(handle, _)| *handle != seller)
            .map(|(handle, _)| handle)
            .unwrap();
        let committed = MilliLots(available.0 / 2);
        let contract = crate::state::Contract {
            buyer,
            seller,
            product,
            from_region: region,
            to_region: world.firm(buyer).unwrap().region,
            quantity_weekly: committed,
            price_cents: dataset.reference_price(product),
            penalty_bps: 500,
            escrow_cents: 0,
            remaining_weeks: 4,
            next_delivery: cw_core::calendar::Tick(world.tick.0 + 1),
            status: crate::state::ContractStatus::Active,
            shortfalls: MilliLots::ZERO,
            auto_renew: false,
        };
        world.contracts.insert(contract);
        sell_surplus_to_gateway(&mut world, &dataset, region, product);
        assert!(
            world.firm(seller).unwrap().stock(product, region).0 >= committed.0,
            "committed stock is protected from export"
        );
    }

    #[test]
    fn brand_and_quality_shift_household_demand_between_equal_prices() {
        // Two sellers, same price and stock. The branded one must win share without any free
        // sales: total quantity is conserved and revenue still comes from fulfilled lots.
        let mut branded = offer(1, 100_000, 1_000);
        branded.brand_bps = 15_000;
        let mut generic = offer(2, 100_000, 1_000);
        generic.brand_bps = 7_500;
        let offers = vec![branded, generic];
        let requests = vec![Request {
            buyer: 100,
            qty: 100_000,
            budget_cents: 100_000,
            brand_sensitive: true,
        }];
        let fills = allocate(1_000, &offers, &requests);
        let branded_qty: i64 = fills
            .iter()
            .filter(|(_, seller, _)| *seller == 1)
            .map(|(_, _, q)| q)
            .sum();
        let generic_qty: i64 = fills
            .iter()
            .filter(|(_, seller, _)| *seller == 2)
            .map(|(_, _, q)| q)
            .sum();
        assert_eq!(branded_qty + generic_qty, 100_000, "demand is conserved");
        assert!(branded_qty > generic_qty, "the stronger brand wins share");
        let revenue: i64 = fills.iter().map(|(_, _, q)| q).sum::<i64>() * 1_000 / 1_000;
        assert_eq!(
            revenue, 100_000,
            "revenue is exact fulfilled quantity times price"
        );
    }

    #[test]
    fn industrial_buyers_ignore_brand() {
        let mut branded = offer(1, 100_000, 1_000);
        branded.brand_bps = 15_000;
        let mut generic = offer(2, 100_000, 1_000);
        generic.brand_bps = 7_500;
        let requests = vec![Request {
            buyer: 100,
            qty: 100_000,
            budget_cents: 100_000,
            brand_sensitive: false,
        }];
        let fills = allocate(1_000, &[branded, generic], &requests);
        let first: i64 = fills
            .iter()
            .filter(|(_, seller, _)| *seller == 1)
            .map(|(_, _, q)| q)
            .sum();
        let second: i64 = fills
            .iter()
            .filter(|(_, seller, _)| *seller == 2)
            .map(|(_, _, q)| q)
            .sum();
        assert_eq!(
            first, second,
            "industrial buyers value delivered cost and quality, not brand"
        );
    }

    #[test]
    fn zero_supply_clears_nothing_and_keeps_cash() {
        let offers = vec![offer(1, 0, 10_000)];
        let requests = vec![Request {
            buyer: 100,
            qty: 100_000,
            budget_cents: 100_000,
            brand_sensitive: true,
        }];
        assert!(allocate(10_000, &offers, &requests).is_empty());
    }
}
