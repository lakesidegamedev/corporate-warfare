//! Pricing and production planning from sell-through, shared by rival AI and the player's
//! automatic operations.
//!
//! A firm sets one price per good it makes, every week, rival or player's automatic company.
//! Weekly sales are its closed thirteen-week sales of the good in every region divided by
//! thirteen; stock is everything it holds of the good beyond its own plants' input cover. Where
//! plants buy the good, buyers left short of their input cover lift the price and the seller's
//! unsold stock beyond its target cover cuts it, six percent of reference for every week; a good
//! only households or the state buy reads the seller's cover against its target both ways.
//! Services read how much of their installed capacity sold, so spare capacity cuts the price.
//! The firm then moves a sixth of the way to the price it wants, at most two percent in a week.
//! The result stays within half and twice reference and under the cheapest import price in the
//! corridor, above which buyers import instead; where imports run a quarter or more over home
//! output the seller asks just under the import (services, with no import quote, stop at fifteen
//! percent over reference).

use std::collections::BTreeSet;

use cw_core::ids::Handle;
use cw_core::quantity::MilliLots;
use cw_data::Dataset;

use crate::state::World;

/// Goods a firm's plants make, by-products included: a by-product is priced and sold like any
/// other good, so nothing a plant makes sits without a price.
pub fn outputs(world: &World, dataset: &Dataset, firm: Handle) -> BTreeSet<u16> {
    let Some(entity) = world.firms.get(firm) else {
        return BTreeSet::new();
    };
    entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .flat_map(|plant| dataset.recipes[plant.recipe as usize].outputs())
        .filter_map(|(id, _)| dataset.product_index.get(id).copied())
        .collect()
}

/// Average weekly units of a good sold over the closed weeks, up to thirteen. A firm with fewer
/// weeks on the books averages over the weeks it has, so a young firm's sales are not read as a
/// fraction of what they are.
pub fn weekly_sold(world: &World, firm: Handle, product: u16) -> i64 {
    world
        .firms
        .get(firm)
        .map(|entity| {
            let weeks = entity.sales_history.len().clamp(1, 13) as i64;
            entity
                .sales_history
                .iter()
                .flat_map(|week| week.iter())
                .filter(|((_, p), _)| *p == product)
                .map(|(_, qty)| *qty)
                .sum::<i64>()
                / weeks
        })
        .unwrap_or(0)
}

/// Lots of a good the firm holds beyond what its own plants keep as input cover. Input cover is
/// not unsold output: a steelworks that also assembles goods keeps steel for its own plants.
pub fn free_stock(world: &World, dataset: &Dataset, firm: Handle, product: u16) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let held_all: i64 = entity
        .inventory
        .iter()
        .filter(|((p, _), _)| *p == product)
        .map(|(_, lot)| lot.qty.lots())
        .sum();
    let id = &dataset.products[product as usize].id;
    // Lots the firm's own plants keep as input cover, each at its own stock target.
    let cover: i64 = entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .filter(|plant| plant.status == crate::state::FacilityStatus::Operating)
        .map(|plant| {
            dataset.recipes[plant.recipe as usize]
                .weekly_inputs_lots
                .get(id)
                .copied()
                .unwrap_or(0)
                * plant.blocks as i64
                * crate::economy::storage::plant_cover_weeks(world, dataset, entity, plant, product)
        })
        .sum();
    // Goods held back on purpose are stored, not unsold.
    let held: i64 = entity
        .storage
        .held
        .iter()
        .filter(|((p, _), _)| *p == product)
        .map(|(_, qty)| qty.lots())
        .sum();
    (held_all - cover - held).max(0)
}

/// Weeks of sales a firm may hold of a good before it counts as a glut. Perishables spoil each
/// week they sit, so they count as a glut sooner.
pub fn glut_weeks(dataset: &Dataset, product: u16) -> i64 {
    if dataset.products[product as usize].spoilage_bps_weekly >= 200 {
        2
    } else {
        4
    }
}

/// Extra weeks of unsold stock a harvest tolerates. In the harvest quarter a grower keeps what
/// the season's glut brings (up to its storage) for the lean quarters instead of cutting price
/// and output; this is how storage smooths farm seasonality. Perishables get nothing extra.
pub fn harvest_glut_extra(world: &World, dataset: &Dataset, product: u16) -> i64 {
    if world.tick.quarter() == crate::economy::storage::HARVEST_QUARTER
        && dataset.products[product as usize].spoilage_bps_weekly < 200
        && crate::economy::storage::is_seasonal_good(dataset, product)
    {
        6
    } else {
        0
    }
}

/// Weeks of a plant's need a firm keeps on hand as input cover. Perishable inputs are bought for
/// at most two weeks, since stock that sits spoils.
pub fn cover_weeks(firm: &crate::state::Firm, dataset: &Dataset, product: u16) -> i64 {
    let weeks = firm.policies.input_cover_weeks.max(1) as i64;
    if dataset.products[product as usize].spoilage_bps_weekly >= 200 {
        weeks.min(2)
    } else {
        weeks
    }
}

/// Weeks of sales a firm is content to hold. Stock below this raises the price it asks, stock
/// above it cuts it. Perishables spoil on the shelf, so they aim lower.
pub fn cover_target_hundredths(dataset: &Dataset, product: u16) -> i64 {
    if dataset.products[product as usize].spoilage_bps_weekly >= 200 {
        100
    } else {
        200
    }
}

/// Each hundredth of a week of cover away from the target moves the price the firm wants by this
/// many basis points of reference (six percent for every whole week).
const WANT_BPS_PER_HUNDREDTH_WEEK: i64 = 6;
/// A price closes this fraction of the gap to the price its firm wants each week, so a change in
/// the market is followed over about six weeks and never overshot in one step.
const PRICE_LAG_WEEKS: i64 = 6;
/// No firm moves a price more than this share in a week, in basis points.
const MAX_WEEKLY_MOVE_BPS: i64 = 200;
/// A carrier running its lanes under this share full cuts its run rate, in basis points.
pub const CARRIER_CUT_BPS: i32 = 6_000;
/// A carrier running its lanes at least this full raises its run rate, in basis points.
pub const CARRIER_RAISE_BPS: i32 = 9_000;
/// Weeks a price that has no sales signal takes to ease back toward reference.
const IDLE_REVERSION_WEEKS: i64 = 26;

/// What the firm wants to ask for a good, in basis points of reference, from how much of it is
/// left on the shelf, how short its buyers are and how much of its capacity sold. None when
/// there is nothing to read (nothing sold and nothing held, as at a plant still starting up).
///
/// Goods: see `goods_wanted_bps`. Services keep no stock, so they read the share of installed
/// capacity that sold (see `service_wanted_bps`). Both are continuous, with no dead band, so a
/// price moves as the market moves.
fn wanted_bps(world: &World, dataset: &Dataset, firm: Handle, product: u16) -> Option<i64> {
    let sold = weekly_sold(world, firm, product);
    if dataset.products[product as usize].kind.is_service() {
        let capacity = service_capacity(world, dataset, firm, product);
        if capacity <= 0 {
            return None;
        }
        let used_bps = (sold.saturating_mul(10_000) / capacity).clamp(0, 10_000);
        return Some(service_wanted_bps(used_bps));
    }
    let stock = free_stock(world, dataset, firm, product);
    if sold <= 0 {
        return (stock > 0).then_some(7_000);
    }
    let cover = stock.saturating_mul(100) / sold;
    let target = cover_target_hundredths(dataset, product)
        + harvest_glut_extra(world, dataset, product) * 100;
    let (mut short, mut need) = buyer_shortfall(world, dataset, product);
    // Households keep no stock: a week of their demand that found nothing on the shelf counts
    // as being short the seller's whole target cover, so a good households cannot find reads as
    // scarce the way an empty shelf used to.
    let (unmet, wanted) = household_unmet(world, product);
    if wanted > 0 {
        need = need.saturating_add(wanted);
        short = short.saturating_add(unmet.saturating_mul(target) / 100);
    }
    // A good no plant or household at home buys, that the corridor exports, has an empty shelf
    // because its surplus left at the export price, not because buyers went without: it does
    // not read as scarce.
    let cover = if need <= 0 && exported(world, product) {
        cover.max(target)
    } else {
        cover
    };
    Some(goods_wanted_bps(cover, target, short, need))
}

/// Whole lots of a good households across the corridor found no seller for last week, and the
/// lots they asked for. Zero and zero for a good households do not buy.
pub fn household_unmet(world: &World, product: u16) -> (i64, i64) {
    let (mut unmet, mut wanted) = (0i64, 0i64);
    for region in &world.regions {
        let households = &region.households;
        unmet = unmet.saturating_add(households.unmet_milli.get(&product).copied().unwrap_or(0));
        wanted =
            wanted.saturating_add(households.wanted_milli.get(&product).copied().unwrap_or(0));
    }
    (unmet / 1_000, wanted / 1_000)
}

/// Whether any of the good left the corridor through a gateway last week.
pub fn exported(world: &World, product: u16) -> bool {
    world
        .gateways
        .iter()
        .any(|gateway| gateway.product == product && gateway.last_export_lots > 0)
}

/// Whether some plant or household at home buys the good. A good with no home buyer that the
/// corridor exports is worth its export bid to the plant that makes it, whatever it asks.
pub fn home_buyers(world: &World, dataset: &Dataset, product: u16) -> bool {
    buyer_shortfall(world, dataset, product).1 > 0 || household_unmet(world, product).1 > 0
}

/// The price a seller of a good wants, in basis points of reference, from its weeks of unsold
/// stock (`cover`, hundredths of a week) against the cover it aims for (`target`), and, where
/// plants buy the good, the lots those plants are short of their input cover after this week's
/// trade (`short`) against the lots they use in a week (`need`).
///
/// Where plants buy the good, the stock that matters is the whole corridor's: buyers keep weeks
/// of input cover in their own yards and top it up every week, so a seller's shelf is nearly
/// empty after trade even when every buyer is full. Reading that empty shelf as scarcity pinned
/// prices at the top of the band while stock piled up at the buyers. Here only buyers left short
/// lift the price, six percent for every week of their use they lack, and only a seller's unsold
/// stock beyond its target cuts it, six percent for every week over. A seller that sells what it
/// makes to buyers who are full asks reference. A good no plant buys (sold to households, the
/// state or abroad) reads its own cover against the target both ways, as before. Either way the
/// gap is held within five weeks, so the wanted price stays within 30 percent of reference.
pub fn goods_wanted_bps(cover: i64, target: i64, short: i64, need: i64) -> i64 {
    let gap = if need > 0 {
        let short_weeks = short.max(0).saturating_mul(100) / need;
        short_weeks - (cover - target).max(0)
    } else {
        target - cover
    };
    10_000 + gap.clamp(-500, 500) * WANT_BPS_PER_HUNDREDTH_WEEK
}

/// The price a seller of a service wants, in basis points of reference, from the share of its
/// installed capacity that sold. A service cannot be stored, so spare capacity is its unsold
/// stock: at 85 percent used it asks reference; every point above adds six hundredths of a
/// percent (nine percent over when sold out) and every point below takes the same off, down to
/// twenty percent under reference, where an idle plant still wins work from its rivals.
pub fn service_wanted_bps(used_bps: i64) -> i64 {
    (10_000 + (used_bps.clamp(0, 10_000) - 8_500) * 6 / 10).clamp(8_000, 11_500)
}

/// Lots that the operating plants buying a good are short of their input cover after this
/// week's trade, and the lots those plants use in a week, over the whole corridor. A firm's
/// plants in one state share their stock, so each firm and state is counted once against the
/// sum of its plants' targets. Speculative held stock is not cover.
pub fn buyer_shortfall(world: &World, dataset: &Dataset, product: u16) -> (i64, i64) {
    let id = &dataset.products[product as usize].id;
    let mut targets: std::collections::BTreeMap<(Handle, u8), i64> = Default::default();
    let mut need = 0i64;
    for (_, plant) in world.facilities.iter() {
        if plant.status != crate::state::FacilityStatus::Operating {
            continue;
        }
        let Some(weekly) = dataset.recipes[plant.recipe as usize]
            .weekly_inputs_lots
            .get(id)
            .copied()
        else {
            continue;
        };
        let Some(entity) = world.firms.get(plant.firm).filter(|f| f.operative()) else {
            continue;
        };
        let use_lots = weekly.saturating_mul(plant.blocks as i64);
        let cover =
            crate::economy::storage::plant_cover_weeks(world, dataset, entity, plant, product);
        let demand = world.demand_ratio_bps(plant.region, Some(product));
        need = need.saturating_add(use_lots);
        *targets.entry((plant.firm, plant.region)).or_default() +=
            use_lots.saturating_mul(cover).saturating_mul(demand) / 10_000;
    }
    let short = targets
        .into_iter()
        .map(|((firm, region), target)| {
            let have = world
                .firms
                .get(firm)
                .map(|f| {
                    MilliLots(
                        (f.stock(product, region).0 - f.storage.held_of(product, region).0).max(0),
                    )
                    .whole_lots_floor()
                })
                .unwrap_or(0);
            (target - have).max(0)
        })
        .sum();
    (short, need)
}

/// Imports of a good in basis points of what the corridor's own plants made last week, from
/// the gateways' smoothed import lots. None when nothing is imported. 10,000 means imports
/// match home output; a good imported with no home output reads as the cap, 1,000,000.
pub fn import_dependence_bps(world: &World, dataset: &Dataset, product: u16) -> Option<i64> {
    let imported: i64 = world
        .gateways
        .iter()
        .filter(|gateway| gateway.product == product)
        .map(|gateway| gateway.smoothed_import_lots.max(0))
        .sum();
    if imported <= 0 {
        return None;
    }
    let id = &dataset.products[product as usize].id;
    let made: i64 = world
        .facilities
        .iter()
        .map(|(_, plant)| plant.last_made(dataset, id).lots())
        .sum();
    Some(dependence_bps(imported, made))
}

/// Imports against home output in basis points, capped at 1,000,000 (a hundred times).
pub fn dependence_bps(imported: i64, made: i64) -> i64 {
    if made <= 0 {
        return 1_000_000;
    }
    (imported.saturating_mul(10_000) / made).min(1_000_000)
}

/// Imports at least this share of home output make a good import dependent: home sellers price
/// near the import and rivals weigh building to replace it.
pub const IMPORT_DEPENDENT_BPS: i64 = 12_500;

/// Lots a firm's operating plants could make of a service in a week at full run. Capacity a firm
/// has idled by planning is still capacity it could sell, so it is spare, not used.
fn service_capacity(world: &World, dataset: &Dataset, firm: Handle, product: u16) -> i64 {
    let id = &dataset.products[product as usize].id;
    world
        .firms
        .get(firm)
        .map(|entity| {
            entity
                .facilities
                .iter()
                .filter_map(|handle| world.facilities.get(*handle))
                .filter(|plant| plant.status == crate::state::FacilityStatus::Operating)
                .map(|plant| {
                    dataset.recipes[plant.recipe as usize].lots_of(id) * plant.blocks as i64
                })
                .sum()
        })
        .unwrap_or(0)
}

/// The price the sell-through rule sets this week for one good.
///
/// The firm works out the price it wants from its stock cover (see `wanted_bps`), then moves
/// a sixth of the way there, never more than two percent in a week. The price is kept within
/// half and twice reference and under the cheapest import, above which buyers import instead.
pub fn target_price(world: &World, dataset: &Dataset, firm: Handle, product: u16) -> i64 {
    let reference = dataset.reference_price(product).max(1);
    let Some(entity) = world.firms.get(firm) else {
        return reference;
    };
    let price = entity
        .policies
        .prices
        .get(&product)
        .copied()
        .unwrap_or(reference);
    // A local seller stays five percent under the cheapest import, so its goods still beat
    // imports once freight over a normal haul is added; asking the full import price made the
    // delivered price higher than imports and the corridor imported what it made.
    let import = world
        .gateways
        .iter()
        .filter(|gateway| gateway.product == product)
        .map(|gateway| gateway.sell_price_cents)
        .filter(|price| *price > 0)
        .min();
    let ceiling = import
        .map(|import| import * 95 / 100)
        .unwrap_or(reference + reference * 15 / 100);
    let signal = wanted_bps(world, dataset, firm, product);
    // Where imports run well over what the corridor makes, the corridor is short at any home
    // price under the import, so home sellers ask just under it (the ceiling) whatever their
    // own shelf says: buyers pay the import price for the rest anyway.
    let dependent = import.is_some()
        && import_dependence_bps(world, dataset, product)
            .is_some_and(|bps| bps >= IMPORT_DEPENDENT_BPS);
    let wanted = signal
        .map(|bps| {
            let own = reference * bps / 10_000;
            if dependent { own.max(ceiling) } else { own }
                .clamp(reference / 2, reference * 2)
                .min(ceiling)
        })
        .unwrap_or(reference);
    let lag = if signal.is_some() {
        PRICE_LAG_WEEKS
    } else {
        IDLE_REVERSION_WEEKS
    };
    let mut step = (wanted - price) / lag;
    if step == 0 && wanted != price {
        step = (wanted - price).signum();
    }
    let cap = (price * MAX_WEEKLY_MOVE_BPS / 10_000).max(1);
    let mut priced = (price + step.clamp(-cap, cap))
        .clamp(reference / 2, reference * 2)
        .min(ceiling)
        .max(1);
    // A state firm has one aim: cover its costs. It asks cost plus five percent and no more.
    if entity.state_owned_since.is_some()
        && let Some(cost) = unit_cost(world, dataset, firm, product)
    {
        priced = priced.min((cost * 105 / 100).max(1));
    }
    priced
}

/// Production planning from sell-through, plant by plant: each plant follows the good it makes.
/// Goods: more than four weeks of unsold stock (beyond the firm's own input cover) cuts fifteen
/// points, down to thirty percent; under a week and a half raises fifteen points, up to full
/// capacity. Services keep no stock: the plant moves fifteen points a week toward what the firm
/// sold of the service plus a tenth, between ten percent and full. Otherwise the
/// plant holds. A plant whose good has not sold yet, such as a new plant still waiting for
/// inputs, holds too.
pub fn plant_targets(world: &World, dataset: &Dataset, firm: Handle) -> Vec<(Handle, i32)> {
    let Some(entity) = world.firms.get(firm) else {
        return Vec::new();
    };
    let mut capacity: std::collections::BTreeMap<u16, i64> = std::collections::BTreeMap::new();
    let mut plants: Vec<(Handle, u16, i32)> = Vec::new();
    for handle in &entity.facilities {
        let Some(plant) = world.facilities.get(*handle) else {
            continue;
        };
        if plant.status != crate::state::FacilityStatus::Operating {
            continue;
        }
        let recipe = &dataset.recipes[plant.recipe as usize];
        if recipe.storage_class.is_some() {
            continue;
        }
        if let Some(code) = dataset.product_index.get(&recipe.output) {
            *capacity.entry(*code).or_default() += recipe.weekly_output_lots * plant.blocks as i64;
            plants.push((*handle, *code, plant.target_bps));
        }
    }
    plants
        .into_iter()
        .map(|(handle, product, current)| {
            let sold = weekly_sold(world, firm, product);
            let full = capacity[&product];
            let carrier = world
                .facilities
                .get(handle)
                .is_some_and(|p| dataset.recipes[p.recipe as usize].transport_mode.is_some());
            let (glut, tight) = if carrier {
                // A carrier follows how full its lanes ran: under 60 percent cuts, from 90
                // raises. Unused carrying capacity expires each week like any service, so a
                // carrier no longer idles at half full.
                match crate::economy::logistics::carrier_utilisation_bps(world, handle) {
                    Some(use_bps) => (use_bps < CARRIER_CUT_BPS, use_bps >= CARRIER_RAISE_BPS),
                    None => (false, false),
                }
            } else if dataset.products[product as usize].kind.is_service() {
                // A service that goes unsold expires, so the plant is run toward what sold plus
                // a tenth, fifteen points a week, rather than held anywhere between half and
                // nine-tenths of capacity where it could waste a third of its output.
                let wanted = (sold * 11_000 / full.max(1)).clamp(1_000, 10_000) as i32;
                let target = if wanted < current {
                    (current - 1_500).max(wanted)
                } else {
                    (current + 1_500).min(wanted)
                };
                return (handle, target);
            } else {
                let stock = free_stock(world, dataset, firm, product);
                (
                    stock
                        > sold.max(1)
                            * (glut_weeks(dataset, product)
                                + harvest_glut_extra(world, dataset, product)),
                    sold > 0 && stock * 2 < sold * 3,
                )
            };
            let target = if glut {
                (current - 1_500).max(3_000)
            } else if tight {
                (current + 1_500).min(10_000)
            } else {
                current
            };
            (handle, target)
        })
        .collect()
}

/// Apply this week's plant-by-plant plan. The firm's headline target becomes the
/// capacity-weighted average of its plants.
pub fn plan(world: &mut World, dataset: &Dataset, firm: Handle) {
    for (handle, target) in plant_targets(world, dataset, firm) {
        if let Some(plant) = world.facilities.get_mut(handle) {
            plant.target_bps = target;
        }
    }
    let Some(entity) = world.firms.get(firm) else {
        return;
    };
    let (mut weighted, mut total) = (0i64, 0i64);
    for plant in entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .filter(|plant| plant.status == crate::state::FacilityStatus::Operating)
    {
        if dataset.recipes[plant.recipe as usize]
            .storage_class
            .is_some()
        {
            continue;
        }
        let full = dataset.recipes[plant.recipe as usize].weekly_output_lots * plant.blocks as i64;
        weighted += full * plant.target_bps as i64;
        total += full;
    }
    if total > 0 {
        let average = (weighted / total) as i32;
        world
            .firms
            .get_mut(firm)
            .unwrap()
            .policies
            .production_target_bps = average;
    }
}

/// The market price of a good, as a seller sees it: the median asking price of the other firms
/// that make it, or the reference price when nobody else does.
pub fn market_price(world: &World, dataset: &Dataset, firm: Handle, product: u16) -> i64 {
    let id = &dataset.products[product as usize].id;
    let mut asks: Vec<i64> = world
        .firms
        .iter()
        .filter(|(handle, entity)| *handle != firm && entity.operative())
        .filter(|(_, entity)| {
            entity.facilities.iter().any(|plant| {
                world
                    .facilities
                    .get(*plant)
                    .is_some_and(|plant| dataset.recipes[plant.recipe as usize].makes(id))
            })
        })
        .filter_map(|(_, entity)| entity.policies.prices.get(&product).copied())
        .collect();
    if asks.is_empty() {
        return dataset.reference_price(product).max(1);
    }
    asks.sort_unstable();
    asks[asks.len() / 2].max(1)
}

/// What one lot of a good costs the firm to make at its plants' full run rate this season:
/// inputs at reference, its own wage offers for the crew, maintenance, land and depreciation,
/// over the lots those plants make. None when the firm has no operating plant for the good.
pub fn unit_cost(world: &World, dataset: &Dataset, firm: Handle, product: u16) -> Option<i64> {
    let entity = world.firms.get(firm)?;
    let id = &dataset.products[product as usize].id;
    let (mut cost, mut lots) = (0i64, 0i64);
    for plant in entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .filter(|plant| plant.status == crate::state::FacilityStatus::Operating)
    {
        let recipe = &dataset.recipes[plant.recipe as usize];
        if !recipe.makes(id) {
            continue;
        }
        let blocks = plant.blocks as i64;
        let crew_bps = crate::economy::production::crew_need_bps(world, recipe);
        let inputs: i64 = recipe
            .weekly_inputs_lots
            .iter()
            .filter_map(|(input, qty)| {
                dataset
                    .product_index
                    .get(input)
                    .map(|code| qty * dataset.reference_price(*code))
            })
            .sum();
        let wages: i64 = (0..3)
            .map(|tier| {
                recipe.workers_low_mid_high[tier] * entity.policies.wage_offer[tier] * crew_bps
                    / 10_000
            })
            .sum();
        let fixed = recipe.weekly_maintenance(dataset.maintenance_annual_bps)
            + dataset.fixture_land_rent_cents_weekly
            + recipe.weekly_depreciation();
        // A joint plant's cost is shared among its products by value at reference prices.
        let value_of = |product: &str| {
            dataset
                .product_index
                .get(product)
                .map(|code| dataset.reference_price(*code))
                .unwrap_or(0)
        };
        let total_value = recipe.output_value(value_of).max(1);
        let own_value = recipe.lots_of(id) * value_of(id);
        cost += (inputs + wages + fixed) * blocks * own_value / total_value;
        lots += recipe.lots_of(id) * blocks;
    }
    (lots > 0).then(|| (cost / lots).max(1))
}

/// The price a posture sets this week, within half and twice reference. None when the good has
/// no posture, or a cost-plus posture has no cost to mark up.
pub fn posture_price(world: &World, dataset: &Dataset, firm: Handle, product: u16) -> Option<i64> {
    let posture = *world
        .firms
        .get(firm)?
        .policies
        .price_postures
        .get(&product)?;
    let base = match posture {
        crate::state::PricePosture::Market { bps } => {
            market_price(world, dataset, firm, product) * (10_000 + bps as i64) / 10_000
        }
        crate::state::PricePosture::CostPlus { bps } => {
            unit_cost(world, dataset, firm, product)? * (10_000 + bps as i64) / 10_000
        }
    };
    let reference = dataset.reference_price(product).max(1);
    Some(base.clamp(reference / 2, reference * 2).max(1))
}

/// The player's automatic pricing: every good a player company makes, unless the player set
/// its price by hand, follows the sell-through rule each week.
pub fn weekly(world: &mut World, dataset: &Dataset) {
    // Rivals always plan production this way; the player's companies do unless set by hand.
    let rivals: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(handle, firm)| {
            firm.operative() && !crate::economy::events_engine::player_side(world, *handle)
        })
        .map(|(handle, _)| handle)
        .collect();
    for firm in rivals {
        // Rivals ask the same sell-through price every week, state firms capped at cost.
        for product in outputs(world, dataset, firm) {
            let target = target_price(world, dataset, firm, product);
            if let Some(entity) = world.firms.get_mut(firm) {
                entity.policies.prices.insert(product, target);
            }
        }
        plan(world, dataset, firm);
    }
    let firms: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(handle, firm)| {
            firm.operative() && crate::economy::events_engine::player_side(world, *handle)
        })
        .map(|(handle, _)| handle)
        .collect();
    for firm in firms {
        for product in outputs(world, dataset, firm) {
            // A posture sets the price from the market or the firm's own cost.
            if let Some(price) = posture_price(world, dataset, firm, product) {
                if let Some(entity) = world.firms.get_mut(firm) {
                    entity.policies.prices.insert(product, price);
                }
                continue;
            }
            let manual = world
                .firms
                .get(firm)
                .is_some_and(|f| f.policies.manual_prices.contains(&product));
            if manual {
                continue;
            }
            let target = target_price(world, dataset, firm, product);
            if let Some(entity) = world.firms.get_mut(firm) {
                entity.policies.prices.insert(product, target);
            }
        }
        // Automatic production follows sell-through too, unless the player set it by hand.
        let manual = world
            .firms
            .get(firm)
            .is_some_and(|f| f.policies.manual_production);
        if !manual {
            plan(world, dataset, firm);
        }
    }
}

/// Apply a firm-wide production target to the firm and every plant it runs.
pub fn set_target(world: &mut World, firm: Handle, target_bps: i32) {
    let Some(entity) = world.firms.get_mut(firm) else {
        return;
    };
    entity.policies.production_target_bps = target_bps;
    let plants = entity.facilities.clone();
    for handle in plants {
        if let Some(plant) = world.facilities.get_mut(handle) {
            plant.target_bps = target_bps;
        }
    }
}

/// A good's corridor price this week: the asking prices of the firms that make it, each counted
/// by the lots it sells (and one, so a plant that has not sold yet still counts), or the cheapest
/// import quote when nobody does, or the reference price. Weighting by sales keeps the price
/// steady when a small seller joins, leaves or is restructured, which a plain median does not.
pub fn corridor_price(world: &World, dataset: &Dataset, product: u16) -> i64 {
    use crate::state::FirmStatus;
    let id = &dataset.products[product as usize].id;
    // A firm in restructuring still holds its plants and asks, so it stays in the average: a
    // firm that drops out for a week between states would make the price jump and jump back.
    let (mut total, mut weight) = (0i128, 0i128);
    for (handle, entity) in world.firms.iter().filter(|(_, entity)| {
        matches!(
            entity.status,
            FirmStatus::Operating | FirmStatus::PastDue { .. } | FirmStatus::Restructuring { .. }
        )
    }) {
        let makes = entity.facilities.iter().any(|plant| {
            world
                .facilities
                .get(*plant)
                .is_some_and(|plant| dataset.recipes[plant.recipe as usize].makes(id))
        });
        let Some(ask) = entity
            .policies
            .prices
            .get(&product)
            .copied()
            .filter(|_| makes)
        else {
            continue;
        };
        let lots = weekly_sold(world, handle, product) as i128 + 1;
        total += ask as i128 * lots;
        weight += lots;
    }
    if weight > 0 {
        return ((total + weight / 2) / weight).max(1) as i64;
    }
    world
        .gateways
        .iter()
        .filter(|gateway| gateway.product == product && gateway.sell_price_cents > 0)
        .map(|gateway| gateway.sell_price_cents)
        .min()
        .unwrap_or_else(|| dataset.reference_price(product))
        .max(1)
}

/// Close the week's prices into the bounded history: every priced good and every operating
/// firm's share price.
pub fn record_closing_prices(world: &mut World, dataset: &Dataset) {
    let goods = (0..dataset.products.len() as u16)
        .filter(|code| dataset.products[*code as usize].reference_price_cents > 0)
        .map(|code| (code, corridor_price(world, dataset, code)))
        .collect();
    let shares = world
        .firms
        .iter()
        .filter(|(_, entity)| entity.operative() && entity.shares.issued > 0)
        .map(|(handle, entity)| (handle, entity.shares.mark_cents.max(1)))
        .collect();
    world.price_history.push(crate::state::PricePoint {
        week: world.tick.0,
        goods,
        shares,
    });
    let excess = world
        .price_history
        .len()
        .saturating_sub(crate::state::PRICE_HISTORY_WEEKS);
    world.price_history.drain(..excess);
}
