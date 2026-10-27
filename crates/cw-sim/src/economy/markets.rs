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
    // Roll the gateway volume signal: last week's imports feed the smoothed average.
    for gateway in &mut world.gateways {
        gateway.smoothed_import_lots =
            (gateway.smoothed_import_lots * 3 + gateway.last_import_lots) / 4;
        gateway.last_import_lots = 0;
        gateway.last_export_lots = 0;
    }
    let opening = world.outside.gateway_book.cash().max(0);
    world.outside.gateway_floor_cents = opening - opening / 100;
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
    // Eighteen lots a week per state, as in 1.1.0 when a region was a state, split across the
    // state's regions by demand.
    const LOTS_PER_STATE: i64 = 18;
    let total_demand: i64 = world
        .regions
        .iter()
        .map(|r| r.demand_index_bps as i64)
        .sum::<i64>()
        .max(1);
    let states = dataset.states.len().max(1) as i64;
    for region in 0..world.regions.len() {
        let lots =
            (LOTS_PER_STATE * states * world.regions[region].demand_index_bps as i64 * 1_000
                / total_demand)
                .max(1_000);
        // Contractors anywhere in the state can take the region's public works.
        let sellers: Vec<(Handle, u8)> = supply_regions(dataset, region as u8, service)
            .into_iter()
            .flat_map(|from| {
                world
                    .firms
                    .iter()
                    .filter(move |(_, firm)| firm.operative() && firm.stock(service, from).0 > 0)
                    .map(move |(handle, _)| (handle, from))
            })
            .collect();
        let mut remaining = MilliLots(lots);
        for (seller, from) in sellers {
            if remaining.0 <= 0 {
                break;
            }
            let capacity = world.construction_capacity(dataset, seller);
            let committed = world.construction_orders.get(&seller).copied().unwrap_or(0);
            let free_lots = (capacity - committed).max(0);
            if free_lots <= 0 {
                continue;
            }
            let available = world
                .firm(seller)
                .map(|f| f.stock(service, from))
                .unwrap_or_default();
            let qty = available
                .min(remaining)
                .min(MilliLots::from_lots(free_lots));
            if qty.0 <= 0 {
                continue;
            }
            // Buy only what the paced outside account can pay for in full.
            let price = dataset.reference_price(service).max(1);
            let affordable = world.outside.gateway_spendable() / price * 1_000;
            let qty = qty.min(MilliLots(affordable));
            if qty.0 <= 0 {
                break;
            }
            let paid = qty.value_at(Cents(price)).unwrap_or(Cents::ZERO).0;
            if paid <= 0 {
                break;
            }
            let (removed, carrying) = remove_stock(world, seller, service, from, qty);
            if !removed {
                continue;
            }
            let received = crate::economy::fx::numeraire_to_firm(world, dataset, seller, paid);
            if let Some(firm) = world.firms.get_mut(seller) {
                let _ = firm.book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, received, "public works")
                        .credit(Account::Revenue, received, "public works")
                        .credit(Account::Inventory, carrying, "public works cogs")
                        .debit(Account::CostOfGoods, carrying, "public works cogs"),
                );
                firm.record_sale(region as u8, service, qty);
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
            let cover =
                crate::economy::storage::plant_cover_weeks(world, dataset, firm, facility, code);
            let demand = world.demand_ratio_bps(facility.region, Some(code));
            let target = weekly * facility.blocks as i64 * cover * demand / 10_000;
            // Goods held back for speculation do not count as the plant's own cover.
            let have = MilliLots(
                (firm.stock(code, facility.region).0
                    - firm.storage.held_of(code, facility.region).0)
                    .max(0),
            )
            .whole_lots_floor();
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

    // Goods clear on one corridor-wide exchange per product; services and other kinds keep
    // their regional or statewide books below.
    let mut exchange: BTreeMap<u16, Vec<(Handle, u8, i64, i64)>> = BTreeMap::new();
    needed.retain(|(region, product), buyers| {
        if dataset.products[*product as usize].kind != cw_data::ProductKind::Goods {
            return true;
        }
        let entry = exchange.entry(*product).or_default();
        for (firm, qty, budget) in buyers.iter() {
            entry.push((*firm, *region, *qty, *budget));
        }
        false
    });
    for (product, buyers) in exchange {
        clear_goods_exchange(world, dataset, product, buyers, &BTreeMap::new());
    }
    // Speculators come last: they buy what the plants left, into held stock.
    for (product, (buyers, caps)) in crate::economy::storage::order_buyers(world, dataset) {
        let before: Vec<(Handle, u8, i64)> = buyers
            .iter()
            .map(|(firm, region, _, _)| {
                (
                    *firm,
                    *region,
                    world
                        .firms
                        .get(*firm)
                        .map(|f| f.stock(product, *region).0)
                        .unwrap_or(0),
                )
            })
            .collect();
        clear_goods_exchange(world, dataset, product, buyers, &caps);
        for (firm, region, was) in before {
            let now = world
                .firms
                .get(firm)
                .map(|f| f.stock(product, region).0)
                .unwrap_or(0);
            crate::economy::storage::record_received(
                world,
                firm,
                product,
                region,
                MilliLots(now - was),
            );
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
        for from in supply_regions(dataset, region, product) {
            for (handle, firm) in world.firms.iter() {
                if !firm.operative() {
                    continue;
                }
                let cover = own_input_cover_lots(world, dataset, firm, from, product) * 1_000;
                let qty = firm.stock(product, from).0 - cover;
                if qty <= 0 {
                    continue;
                }
                let price = firm
                    .policies
                    .prices
                    .get(&product)
                    .copied()
                    .unwrap_or_else(|| dataset.reference_price(product));
                let source = firm_source(handle, from, region);
                offers.push(Offer {
                    seller: source_id(source),
                    qty,
                    price_cents: price.max(1),
                    quality_bps: 10_000,
                    brand_bps: 10_000,
                });
                seller_handles.push(source);
            }
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

/// The corridor commodity exchange for one good. Every buyer's need in any region meets every
/// seller's stock in any region, plus the import offer of the buyer's own region. Each pairing
/// is priced as delivered: the seller's asking price plus freight over the lane. Pairings fill
/// cheapest first, so local and nearby supply beat distant supply and the outside market is the
/// last resort. The seller receives its asking price; the buyer pays the freight to the
/// carrier. Exchange deliveries arrive within the week. A buyer only takes what it can pay for
/// in full.
fn clear_goods_exchange(
    world: &mut World,
    dataset: &Dataset,
    product: u16,
    buyers: Vec<(Handle, u8, i64, i64)>,
    caps: &BTreeMap<(Handle, u8), i64>,
) {
    // Merge each firm's facilities into one buyer per region.
    let mut demand: BTreeMap<(Handle, u8), (i64, i64)> = BTreeMap::new();
    for (firm, region, qty, budget) in buyers {
        let entry = demand.entry((firm, region)).or_insert((0, 0));
        entry.0 += qty;
        entry.1 += budget;
    }
    if demand.is_empty() {
        return;
    }
    // Sellers: each firm's stock by region beyond its own plants' input cover.
    let mut supply: BTreeMap<(Handle, u8), i64> = BTreeMap::new();
    for (handle, firm) in world.firms.iter() {
        if !firm.operative() {
            continue;
        }
        for ((code, from), lot) in firm.inventory.iter() {
            if *code != product || lot.qty.0 <= 0 {
                continue;
            }
            let cover = own_input_cover_lots(world, dataset, firm, *from, product) * 1_000;
            let qty = lot.qty.0 - cover;
            if qty > 0 {
                supply.insert((handle, *from), qty);
            }
        }
    }
    // The reserve's release in a spike, by region: (asking price, milli-lots).
    let reserve_offers: BTreeMap<u8, (i64, i64)> = (0..world.regions.len() as u8)
        .filter_map(|region| {
            crate::economy::reserve::release_offer(world, dataset, product, region)
                .map(|offer| (region, offer))
        })
        .collect();
    let mut reserve_left: BTreeMap<u8, i64> =
        reserve_offers.iter().map(|(r, (_, q))| (*r, *q)).collect();
    let class = dataset.products[product as usize].storage_class;
    let mut freight_table: BTreeMap<(u8, u8), i64> = BTreeMap::new();
    for (_, to) in demand.keys() {
        let froms: Vec<u8> = supply
            .keys()
            .map(|(_, from)| *from)
            .chain(reserve_offers.keys().copied())
            .collect();
        for from in froms {
            freight_table.entry((from, *to)).or_insert_with(|| {
                crate::economy::logistics::estimate_cents_per_lot(world, dataset, from, *to, class)
            });
        }
    }
    let freight_per_lot = |from: u8, to: u8| freight_table.get(&(from, to)).copied().unwrap_or(0);
    // The seller's asking price, in the money of the buyer who reads it.
    let asking = |world: &World, firm: Handle, buyer: Handle| {
        let price = world
            .firms
            .get(firm)
            .and_then(|f| f.policies.prices.get(&product).copied())
            .unwrap_or_else(|| dataset.reference_price(product))
            .max(1);
        if !world.fx.active {
            return price;
        }
        let from = crate::economy::fx::handle_currency(world, dataset, firm);
        let to = crate::economy::fx::handle_currency(world, dataset, buyer);
        world.fx.convert(price, from, to).max(1)
    };
    // Every pairing with its delivered price per lot. A seller never buys from itself.
    #[derive(Clone, Copy)]
    enum Source {
        Firm(Handle, u8),
        Gateway(usize),
        /// The strategic reserve's stock in one region, sold in that region only.
        Reserve(u8),
    }
    // (delivered price, seller key, buyer key, buyer, source)
    type Pairing = (i64, u64, u64, (Handle, u8), Source);
    let mut pairs: Vec<Pairing> = Vec::new();
    for (buyer_key, _) in demand.iter() {
        let (buyer, to) = *buyer_key;
        for (from, (ask, _)) in &reserve_offers {
            // The reserve prices in the numeraire; the buyer reads it in its own money.
            let ask = world
                .fx
                .convert(*ask, 0, crate::economy::fx::currency_of(world, buyer));
            let delivered = ask + freight_per_lot(*from, to);
            if caps.get(buyer_key).is_none_or(|cap| delivered <= *cap) {
                pairs.push((
                    delivered,
                    u64::MAX / 2 - *from as u64,
                    buyer.0 ^ ((to as u64) << 56),
                    *buyer_key,
                    Source::Reserve(*from),
                ));
            }
        }
        for (seller_key, _) in supply.iter() {
            let (seller, from) = *seller_key;
            if seller == buyer {
                continue;
            }
            let delivered = asking(world, seller, buyer) + freight_per_lot(from, to);
            if caps.get(buyer_key).is_some_and(|cap| delivered > *cap) {
                continue;
            }
            pairs.push((
                delivered,
                seller.0 ^ ((from as u64) << 56),
                buyer.0 ^ ((to as u64) << 56),
                *buyer_key,
                Source::Firm(seller, from),
            ));
        }
        for (index, gateway) in world.gateways.iter().enumerate() {
            if gateway.region == to
                && gateway.product == product
                && gateway.sell_price_cents > 0
                && caps
                    .get(buyer_key)
                    .is_none_or(|cap| gateway.sell_price_cents <= *cap)
            {
                pairs.push((
                    gateway.sell_price_cents,
                    // Each gateway is its own source, after every firm seller at equal price.
                    u64::MAX - index as u64,
                    buyer.0 ^ ((to as u64) << 56),
                    *buyer_key,
                    Source::Gateway(index),
                ));
            }
        }
    }
    // The reserve's release is an emergency sale that takes orders first, then the cheapest
    // delivered price as usual.
    pairs.sort_by_key(|(price, seller, buyer, _, source)| {
        (
            !matches!(source, Source::Reserve(_)),
            *price,
            *seller,
            *buyer,
        )
    });
    let mut import_left: BTreeMap<usize, i64> = BTreeMap::new();
    // Work through the pairings one price tier and source at a time. Buyers tied on the same
    // source at the same delivered price share it in proportion to what each still wants and
    // can pay for, so a large buyer cannot starve a small one by handle order.
    let mut index = 0;
    while index < pairs.len() {
        let (delivered, seller_key, ..) = pairs[index];
        let mut end = index;
        while end < pairs.len() && pairs[end].0 == delivered && pairs[end].1 == seller_key {
            end += 1;
        }
        let tier: Vec<((Handle, u8), Source)> =
            pairs[index..end].iter().map(|p| (p.3, p.4)).collect();
        index = end;
        let source = tier[0].1;
        let available = match source {
            Source::Firm(seller, from) => supply.get(&(seller, from)).copied().unwrap_or(0),
            Source::Gateway(gate) => *import_left
                .entry(gate)
                .or_insert(world.gateways[gate].capacity_lots * 1_000),
            Source::Reserve(region) => reserve_left.get(&region).copied().unwrap_or(0),
        };
        if available <= 0 {
            continue;
        }
        // What each tied buyer wants and can pay for at this price, in milli-lots.
        let wants: Vec<(u64, i64)> = tier
            .iter()
            .map(|(buyer_key, _)| {
                let (want, budget) = demand[buyer_key];
                let cash = world
                    .firms
                    .get(buyer_key.0)
                    .map(|f| f.book.cash())
                    .unwrap_or(0)
                    .max(0);
                let affordable =
                    (budget.min(cash).max(0) as i128 * 1_000 / delivered as i128) as i64;
                (
                    buyer_key.0.0 ^ ((buyer_key.1 as u64) << 56),
                    want.min(affordable).max(0),
                )
            })
            .collect();
        let total: i64 = wants.iter().map(|(_, w)| *w).sum();
        if total <= 0 {
            continue;
        }
        let shares: Vec<i64> = if total <= available {
            wants.iter().map(|(_, w)| *w).collect()
        } else {
            allocate_cents(Cents(available), &wants)
                .iter()
                .map(|(_, c)| c.0)
                .collect()
        };
        for ((buyer_key, source), qty) in tier.into_iter().zip(shares) {
            if qty <= 0 {
                continue;
            }
            let (buyer, to) = buyer_key;
            let qty_lots = MilliLots(qty);
            match source {
                Source::Firm(seller, from) => {
                    let price = asking(world, seller, buyer);
                    let goods = qty_lots.value_at(Cents(price)).unwrap_or(Cents::ZERO);
                    if !trade_between(
                        world,
                        dataset,
                        buyer,
                        seller,
                        product,
                        (from, to),
                        qty_lots,
                        goods,
                    ) {
                        continue;
                    }
                    if let Some(entity) = world.firms.get_mut(seller) {
                        entity.record_sale(to, product, qty_lots);
                    }
                    let freight = crate::economy::logistics::book(
                        world,
                        dataset,
                        buyer,
                        product,
                        from,
                        to,
                        qty_lots,
                        "exchange freight",
                    )
                    .cents;
                    if let Some(left) = supply.get_mut(&(seller, from)) {
                        *left -= qty;
                    }
                    let entry = demand.get_mut(&buyer_key).unwrap();
                    entry.0 -= qty;
                    entry.1 -= goods.0 + freight;
                }
                Source::Reserve(region) => {
                    let ask = reserve_offers
                        .get(&region)
                        .map(|o| {
                            world
                                .fx
                                .convert(o.0, 0, crate::economy::fx::currency_of(world, buyer))
                        })
                        .unwrap_or(delivered);
                    let paid = qty_lots.value_at(Cents(ask)).unwrap_or(Cents::ZERO).0;
                    if paid <= 0 {
                        continue;
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
                        .stock_lot_mut(product, to)
                        .receive(qty_lots, Cents(paid));
                    let paid_numeraire =
                        crate::economy::fx::firm_to_numeraire_world(world, buyer, paid);
                    crate::economy::reserve::record_sale(
                        world,
                        product,
                        region,
                        qty_lots,
                        paid_numeraire,
                    );
                    let freight = if region == to {
                        0
                    } else {
                        crate::economy::logistics::book(
                            world,
                            dataset,
                            buyer,
                            product,
                            region,
                            to,
                            qty_lots,
                            "exchange freight",
                        )
                        .cents
                    };
                    if let Some(left) = reserve_left.get_mut(&region) {
                        *left -= qty;
                    }
                    let entry = demand.get_mut(&buyer_key).unwrap();
                    entry.0 -= qty;
                    entry.1 -= paid + freight;
                }
                Source::Gateway(gate) => {
                    let paid = qty_lots.value_at(Cents(delivered)).unwrap_or(Cents::ZERO).0;
                    if paid <= 0 {
                        continue;
                    }
                    let _ = world.firms.get_mut(buyer).unwrap().book.commit(
                        world.tick,
                        Txn::new().credit(Account::Cash, paid, "import").debit(
                            Account::Inventory,
                            paid,
                            "import",
                        ),
                    );
                    world
                        .firms
                        .get_mut(buyer)
                        .unwrap()
                        .stock_lot_mut(product, to)
                        .receive(qty_lots, Cents(paid));
                    world.gateways[gate].last_import_lots += qty_lots.lots();
                    let received =
                        crate::economy::fx::region_to_numeraire(world, dataset, to, paid);
                    let _ = world.outside.gateway_book.commit(
                        world.tick,
                        Txn::new()
                            .debit(Account::Cash, received, "export receipt")
                            .credit(Account::Revenue, received, "export receipt"),
                    );
                    *import_left.get_mut(&gate).unwrap() -= qty;
                    let entry = demand.get_mut(&buyer_key).unwrap();
                    entry.0 -= qty;
                    entry.1 -= paid;
                }
            }
        }
    }
}

/// Lots of a good a firm keeps in one state as input cover for its own plants there. Stock up
/// to this level is not for sale: a plant's inputs are never offered to other buyers.
fn own_input_cover_lots(
    world: &World,
    dataset: &Dataset,
    firm: &crate::state::Firm,
    region: u8,
    product: u16,
) -> i64 {
    // Input cover plus goods held back, rounded up to whole lots.
    (crate::economy::storage::reserved_milli(world, dataset, firm, region, product) + 999) / 1_000
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
        .filter(|(_, firm)| firm.operative())
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
        let own_need = own_input_cover_lots(world, dataset, firm, region, product);
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
        // The gateway buys only what its paced cash pays for in full; a seller never hands
        // over goods for part of their price.
        let price = gateway.buy_price_cents.max(1);
        let price_numeraire =
            crate::economy::fx::region_to_numeraire(world, dataset, region, price).max(1);
        let affordable = world.outside.gateway_spendable() / price_numeraire * 1_000;
        let qty = MilliLots::from_lots(exportable)
            .min(remaining)
            .min(MilliLots(affordable));
        if qty.0 <= 0 {
            break;
        }
        let paid = qty.value_at(Cents(price)).unwrap_or(Cents::ZERO).0;
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
            firm.record_sale(region, product, qty);
        }
        let spent = crate::economy::fx::region_to_numeraire(world, dataset, region, paid);
        let _ = world.outside.gateway_book.commit(
            world.tick,
            Txn::new()
                .credit(Account::Cash, spent, "export purchase")
                .debit(Account::Inventory, spent, "export purchase"),
        );
        world.gateways[index].last_export_lots += qty.lots();
        remaining = MilliLots(remaining.0 - qty.0);
    }
}

#[derive(Clone, Copy, Debug)]
enum OfferSource {
    /// A firm's stock held in region `from`, offered under `id`.
    Firm {
        handle: Handle,
        from: u8,
        id: u64,
    },
    Gateway(usize),
}

/// Services that reach customers across a state clear statewide: a plant anywhere in the state
/// can serve any of its regions, and contractors build anywhere in their state. Goods, retail,
/// leases and electricity clear in their own region (goods move between regions by freight).
fn statewide(dataset: &Dataset, product: u16) -> bool {
    let product = &dataset.products[product as usize];
    matches!(
        product.kind,
        cw_data::ProductKind::Subscription
            | cw_data::ProductKind::Service
            | cw_data::ProductKind::LocalService
            | cw_data::ProductKind::ProjectService
    ) && product.id != "electricity"
}

/// Regions whose stock can be offered in `region`'s market, the market's own region first.
fn supply_regions(dataset: &Dataset, region: u8, product: u16) -> Vec<u8> {
    if !statewide(dataset, product) {
        return vec![region];
    }
    let state = dataset.regions[region as usize].state();
    let mut regions = vec![region];
    regions.extend(
        dataset
            .regions_in_state(state)
            .into_iter()
            .filter(|other| *other != region),
    );
    regions
}

/// A firm's offer source. Stock offered in its own region keeps the firm's handle as its id;
/// stock offered from another region of the state carries that region in bits 55 to 62, which
/// a slot generation would need eight million reuses to reach.
fn firm_source(handle: Handle, from: u8, market: u8) -> OfferSource {
    let id = if from == market {
        handle.0
    } else {
        handle.0 | ((from as u64 + 1) << 55)
    };
    OfferSource::Firm { handle, from, id }
}

/// One week's household spending budget. Disposable wage income supplies ninety percent;
/// after a shortfall against the disposable baseline, at most one percent of savings may
/// supplement the budget. The supplement is deducted when it is committed, and any unspent
/// category budget returns to savings after clearing.
fn weekly_household_budget(household: &crate::state::Household) -> (i64, i64) {
    let income = household.last_income_cents.max(0);
    let baseline = household.baseline_income_cents.max(0);
    let savings = household.savings_cents.max(0);
    let mut budget = income * 90 / 100;
    let shortfall = (baseline * 90 / 100 - budget).max(0);
    let supplement = (savings / 100).min(shortfall).max(0);
    budget += supplement;
    (budget, supplement)
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
        let (budget_total, supplement) =
            weekly_household_budget(&world.regions[region_index].households);
        if supplement > 0 {
            world.regions[region_index].households.savings_cents -= supplement;
        }
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
            let demand = world.demand_ratio_bps(region_index as u8, Some(product));
            let requested = (base * ratio / 10_000).max(0) * demand / 10_000 * 1_000;
            if requested <= 0 {
                continue;
            }
            let mut offers: Vec<Offer> = Vec::new();
            let mut sources: Vec<OfferSource> = Vec::new();
            for from in supply_regions(dataset, region_index as u8, product) {
                for (handle, firm) in world.firms.iter() {
                    if !firm.operative() {
                        continue;
                    }
                    let qty = firm.stock(product, from).0;
                    if qty <= 0 {
                        continue;
                    }
                    let price = firm
                        .policies
                        .prices
                        .get(&product)
                        .copied()
                        .unwrap_or(reference);
                    let source = firm_source(handle, from, region_index as u8);
                    offers.push(Offer {
                        seller: source_id(source),
                        qty,
                        price_cents: price.max(1),
                        quality_bps: firm.quality_bps.get(&product).copied().unwrap_or_else(|| {
                            crate::economy::research::quality(firm.tech_level, 0)
                        }) as i64
                            * (10_000
                                + crate::economy::structure::loyalty_premium_bps(
                                    world, handle, from, product,
                                ))
                            / 10_000,
                        brand_bps: firm
                            .brand_bps
                            .get(&(region_index as u8, product))
                            .copied()
                            .unwrap_or(10_000) as i64,
                    });
                    sources.push(source);
                }
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
        OfferSource::Firm { id, .. } => id,
        OfferSource::Gateway(index) => GATEWAY_ID_BASE + index as u64,
    }
}

fn clear_government(world: &mut World, dataset: &Dataset) {
    // Care episodes are funded from the healthcare budget. It is a weekly national ceiling, not
    // a per-region allowance, so spending is consumed as the regions are served. Defence runs
    // through funded procurement contracts rather than a spot book.
    let care = dataset.product_index["care"];
    let healthcare = world.government.healthcare_budget_cents.max(0);
    for (product, budget) in [(care, healthcare)] {
        let mut remaining = budget;
        for region in 0..world.regions.len() {
            if remaining <= 0 {
                break;
            }
            let mut offers = Vec::new();
            let mut sources = Vec::new();
            for from in supply_regions(dataset, region as u8, product) {
                for (handle, firm) in world.firms.iter() {
                    if !firm.operative() {
                        continue;
                    }
                    let qty = firm.stock(product, from).0;
                    if qty <= 0 {
                        continue;
                    }
                    let price = firm
                        .policies
                        .prices
                        .get(&product)
                        .copied()
                        .unwrap_or_else(|| dataset.reference_price(product));
                    let source = firm_source(handle, from, region as u8);
                    offers.push(Offer {
                        seller: source_id(source),
                        qty,
                        price_cents: price.max(1),
                        quality_bps: 10_000
                            + crate::economy::structure::loyalty_premium_bps(
                                world, handle, from, product,
                            ),
                        brand_bps: 10_000,
                    });
                    sources.push(source);
                }
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
        if let Some(OfferSource::Firm {
            handle: seller_handle,
            ..
        }) = sources
            .iter()
            .find(|source| source_id(**source) == *seller)
            .copied()
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
            OfferSource::Firm {
                handle: seller_handle,
                from,
                ..
            } => {
                let price = world
                    .firms
                    .get(seller_handle)
                    .and_then(|firm| firm.policies.prices.get(&product).copied())
                    .unwrap_or_else(|| dataset.reference_price(product));
                let price = world.fx.convert(
                    price.max(1),
                    crate::economy::fx::handle_currency(world, dataset, seller_handle),
                    crate::economy::fx::handle_currency(world, dataset, buyer_handle),
                );
                let cost = qty.value_at(Cents(price.max(1))).unwrap_or(Cents::ZERO);
                if trade_between(
                    world,
                    dataset,
                    buyer_handle,
                    seller_handle,
                    product,
                    (from, region),
                    qty,
                    cost,
                ) && let Some(seller_firm) = world.firms.get_mut(seller_handle)
                {
                    seller_firm.record_sale(region, product, qty);
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
                world.gateways[index].last_import_lots += qty.lots();
                let received =
                    crate::economy::fx::region_to_numeraire(world, dataset, region, paid);
                let _ = world.outside.gateway_book.commit(
                    world.tick,
                    Txn::new()
                        .debit(Account::Cash, received, "export receipt")
                        .credit(Account::Revenue, received, "export receipt"),
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
        if let OfferSource::Firm {
            handle: seller_handle,
            from,
            ..
        } = source
        {
            let price = world
                .firms
                .get(seller_handle)
                .and_then(|firm| firm.policies.prices.get(&product).copied())
                .unwrap_or_else(|| dataset.reference_price(product));
            let cost = qty.value_at(Cents(price.max(1))).unwrap_or(Cents::ZERO);
            // Household cash is outside cash; the sale posts revenue and removes stock.
            let (removed, carrying) = remove_stock(world, seller_handle, product, from, qty);
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
                firm.record_sale(region, product, qty);
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
        if let OfferSource::Firm {
            handle: seller_handle,
            from,
            ..
        } = source
        {
            let price = world
                .firms
                .get(seller_handle)
                .and_then(|firm| firm.policies.prices.get(&product).copied())
                .unwrap_or_else(|| dataset.reference_price(product));
            let cost = qty.value_at(Cents(price.max(1))).unwrap_or(Cents::ZERO);
            let (removed, carrying) = remove_stock(world, seller_handle, product, from, qty);
            if !removed {
                continue;
            }
            let available = world.government.book.cash().max(0);
            // The state pays in the numeraire; the seller is paid in its own money.
            let cost_numeraire =
                crate::economy::fx::firm_to_numeraire(world, dataset, seller_handle, cost.0);
            let paid_numeraire = cost_numeraire.min(available);
            if paid_numeraire <= 0 {
                continue;
            }
            let paid = if paid_numeraire == cost_numeraire {
                cost.0
            } else {
                crate::economy::fx::numeraire_to_firm(world, dataset, seller_handle, paid_numeraire)
            };
            let _ = world.government.book.commit(
                world.tick,
                Txn::new()
                    .credit(Account::Cash, paid_numeraire, "procurement")
                    .debit(Account::OperatingExpense, paid_numeraire, "procurement"),
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
                firm.record_sale(region, product, qty);
            }
            spent += paid_numeraire;
        }
    }
    spent
}

/// Move `qty` of `product` from the seller's stock in region `route.0` to the buyer's stock in
/// the market region `route.1`, for `cost` capped at the buyer's cash.
#[allow(clippy::too_many_arguments)]
fn trade_between(
    world: &mut World,
    dataset: &Dataset,
    buyer: Handle,
    seller: Handle,
    product: u16,
    route: (u8, u8),
    qty: MilliLots,
    cost: Cents,
) -> bool {
    let (from, region) = route;
    let cash = world
        .firms
        .get(buyer)
        .map(|firm| firm.book.cash())
        .unwrap_or(0);
    let paid = cost.0.min(cash.max(0));
    if paid <= 0 {
        return false;
    }
    let (removed, carrying) = remove_stock(world, seller, product, from, qty);
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
    // The buyer paid in its money; the seller is paid in its own, now or on collection.
    let buyer_currency = crate::economy::fx::handle_currency(world, dataset, buyer);
    crate::economy::fx::book_sale_proceeds(
        world,
        dataset,
        seller,
        buyer_currency,
        paid,
        carrying,
        "sale",
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
    if entity.stock(product, region).0 < qty.0 || qty.0 <= 0 {
        return (false, 0);
    }
    let cost = entity.issue_stock(product, region, qty);
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

    /// Give a firm exactly `lots` of a good in a region, booked at reference cost.
    fn stock(
        world: &mut World,
        dataset: &Dataset,
        firm: Handle,
        product: u16,
        region: u8,
        lots: i64,
    ) {
        let entity = world.firms.get_mut(firm).unwrap();
        entity.inventory.retain(|(p, _), _| *p != product);
        let cost = lots * dataset.reference_price(product);
        entity
            .stock_lot_mut(product, region)
            .receive(MilliLots::from_lots(lots), Cents(cost));
        let _ = entity.book.commit(
            world.tick,
            Txn::new()
                .debit(Account::Inventory, cost, "test stock")
                .credit(Account::ShareCapital, cost, "test stock"),
        );
    }

    #[test]
    fn the_exchange_fills_cheapest_delivered_first_and_imports_last() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            301,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        let steel = dataset.product_index["steel"];
        let firms: Vec<Handle> = world.firms.iter().map(|(h, _)| h).take(3).collect();
        let (buyer, local, remote) = (firms[0], firms[1], firms[2]);
        let to = dataset.region_index["OH-CEN"];
        let far = dataset.region_index["PA-WES"];
        for firm in [buyer, local, remote] {
            world
                .firms
                .get_mut(firm)
                .unwrap()
                .inventory
                .retain(|(p, _), _| *p != steel);
        }
        // A local seller asking $440 and a remote one asking $380 plus freight.
        stock(&mut world, &dataset, local, steel, to, 10);
        stock(&mut world, &dataset, remote, steel, far, 10);
        world
            .firms
            .get_mut(local)
            .unwrap()
            .policies
            .prices
            .insert(steel, 44_000);
        world
            .firms
            .get_mut(remote)
            .unwrap()
            .policies
            .prices
            .insert(steel, 38_000);
        let freight_lot =
            crate::economy::freight::freight_cost(&dataset, far, to, MilliLots::from_lots(100))
                / 100;
        assert!(
            38_000 + freight_lot < 44_000,
            "the remote seller is cheaper delivered"
        );
        let buyer_cash = world.firms.get(buyer).unwrap().book.cash();
        // The buyer wants 15 lots: all ten remote lots first, then five local ones.
        clear_goods_exchange(
            &mut world,
            &dataset,
            steel,
            vec![(buyer, to, 15_000, 100_000_000)],
            &BTreeMap::new(),
        );
        let held = |w: &World, f: Handle, r: u8| w.firms.get(f).unwrap().stock(steel, r).lots();
        assert_eq!(held(&world, buyer, to), 15);
        assert_eq!(held(&world, remote, far), 0);
        assert_eq!(held(&world, local, to), 5);
        // Ten lots are below the smallest rail shipment and no carrier serves the lane, so the
        // outside trucks haul them at the reference rate plus their premium (120 percent).
        let freight =
            crate::economy::freight::freight_cost(&dataset, far, to, MilliLots::from_lots(10))
                * dataset.logistics.outside_premium_bps
                / 10_000;
        assert_eq!(
            buyer_cash - world.firms.get(buyer).unwrap().book.cash(),
            10 * 38_000 + freight + 5 * 44_000,
            "goods at asking prices plus freight once"
        );
        // Eight more lots: the local seller's last five at $440, then three imports at $460.
        let imports_before: i64 = world
            .gateways
            .iter()
            .filter(|g| g.product == steel && g.region == to)
            .map(|g| g.last_import_lots)
            .sum();
        clear_goods_exchange(
            &mut world,
            &dataset,
            steel,
            vec![(buyer, to, 8_000, 100_000_000)],
            &BTreeMap::new(),
        );
        let imports_after: i64 = world
            .gateways
            .iter()
            .filter(|g| g.product == steel && g.region == to)
            .map(|g| g.last_import_lots)
            .sum();
        assert_eq!(imports_after - imports_before, 3);
        assert_eq!(held(&world, local, to), 0);
        assert_eq!(held(&world, buyer, to), 23);
    }

    #[test]
    fn tied_buyers_share_a_scarce_source_in_proportion() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            302,
            crate::state::ScenarioKind::Founder,
            crate::state::ScenarioMode::Sandbox,
        );
        let steel = dataset.product_index["steel"];
        let firms: Vec<Handle> = world.firms.iter().map(|(h, _)| h).take(3).collect();
        let (big, small, seller) = (firms[0], firms[1], firms[2]);
        let to = dataset.region_index["OH-CEN"];
        for firm in [big, small, seller] {
            world
                .firms
                .get_mut(firm)
                .unwrap()
                .inventory
                .retain(|(p, _), _| *p != steel);
        }
        // Remove the import so the single seller is the only source.
        world
            .gateways
            .retain(|g| !(g.product == steel && g.region == to));
        stock(&mut world, &dataset, seller, steel, to, 10);
        // 30 and 10 lots wanted against 10 available: 7.5 and 2.5.
        clear_goods_exchange(
            &mut world,
            &dataset,
            steel,
            vec![
                (big, to, 30_000, 100_000_000),
                (small, to, 10_000, 100_000_000),
            ],
            &BTreeMap::new(),
        );
        assert_eq!(
            world.firms.get(big).unwrap().stock(steel, to),
            MilliLots(7_500)
        );
        assert_eq!(
            world.firms.get(small).unwrap().stock(steel, to),
            MilliLots(2_500)
        );
    }

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
    fn a_wage_shortfall_can_draw_one_percent_of_savings() {
        let mut household = crate::state::Household {
            savings_cents: 1_000_000,
            last_income_cents: 0,
            baseline_income_cents: 100_000,
            weights: BTreeMap::new(),
            baseline_lots: BTreeMap::new(),
            last_sale_price_cents: BTreeMap::new(),
            spend_cents: BTreeMap::new(),
        };
        let (budget, supplement) = weekly_household_budget(&household);
        assert_eq!(supplement, 10_000, "at most one percent of savings");
        assert_eq!(budget, 10_000);
        household.savings_cents -= supplement;
        assert_eq!(household.savings_cents, 990_000);
        household.last_income_cents = 100_000;
        let (budget, supplement) = weekly_household_budget(&household);
        assert_eq!(supplement, 0, "no supplement when income is at baseline");
        assert_eq!(budget, 90_000);
        household.savings_cents = 0;
        household.last_income_cents = 0;
        let (budget, supplement) = weekly_household_budget(&household);
        assert_eq!(supplement, 0);
        assert_eq!(budget, 0);
    }

    #[test]
    fn a_week_with_no_wages_spends_at_most_one_percent_of_savings() {
        let dataset = Dataset::load().unwrap();
        let mut world = crate::economy::seed::build(
            &dataset,
            77,
            crate::state::ScenarioKind::Guided,
            crate::state::ScenarioMode::Sandbox,
        );
        for region in &mut world.regions {
            region.households.last_income_cents = 0;
            region.households.baseline_income_cents = 100_000;
            region.households.savings_cents = 1_000_000;
        }
        clear_households(&mut world, &dataset);
        for region in &world.regions {
            assert!(
                region.households.savings_cents >= 990_000,
                "a zero-income week can spend at most one percent of savings"
            );
            assert!(region.households.savings_cents <= 1_000_000);
        }
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
