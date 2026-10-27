//! Pricing and production planning from sell-through, shared by rival AI and the player's
//! automatic operations.
//!
//! A firm sets one price per good it makes. Weekly sales are its closed thirteen-week sales of
//! the good in every region divided by thirteen; stock is everything it holds of the good. More
//! than four weeks of stock (or none sold while stock sits) cuts the price five percent; selling
//! out, with less than a week of stock, raises it two percent; otherwise it holds. The result
//! stays within half and twice reference and never above the cheapest import price in the
//! corridor, above which buyers import instead (services, with no import quote, stop at fifteen
//! percent over reference). Stock a firm keeps as input cover for its own plants is not
//! counted as unsold.

use std::collections::BTreeSet;

use cw_core::ids::Handle;
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

/// Average weekly units of a good sold over the closed thirteen weeks.
pub fn weekly_sold(world: &World, firm: Handle, product: u16) -> i64 {
    world
        .firms
        .get(firm)
        .map(|entity| {
            entity
                .sales_history
                .iter()
                .flat_map(|week| week.iter())
                .filter(|((_, p), _)| *p == product)
                .map(|(_, qty)| *qty)
                .sum::<i64>()
                / 13
        })
        .unwrap_or(0)
}

/// Lots of a good the firm holds beyond what its own plants keep as input cover. Input cover is
/// not unsold output: a steelworks that also assembles goods keeps steel for its own plants.
pub fn free_stock(world: &World, dataset: &Dataset, firm: Handle, product: u16) -> i64 {
    let Some(entity) = world.firms.get(firm) else {
        return 0;
    };
    let held: i64 = entity
        .inventory
        .iter()
        .filter(|((p, _), _)| *p == product)
        .map(|(_, lot)| lot.qty.lots())
        .sum();
    let id = &dataset.products[product as usize].id;
    let weekly_need: i64 = entity
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
        })
        .sum();
    (held - weekly_need * cover_weeks(entity, dataset, product)).max(0)
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

/// Lots of a good the corridor imported through the gateways last week.
fn imported_last_week(world: &World, product: u16) -> i64 {
    world
        .gateways
        .iter()
        .filter(|gateway| gateway.product == product)
        .map(|gateway| gateway.last_import_lots)
        .sum()
}

/// The price the sell-through rule sets this week for one good.
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
    let sold = weekly_sold(world, firm, product);
    let stock = free_stock(world, dataset, firm, product);
    let mut target = price;
    if (sold == 0 && stock > 0) || stock > sold * glut_weeks(dataset, product) {
        target = price * 95 / 100;
    } else if stock < sold {
        // Selling out only lifts the price while the corridor is still importing the good: with
        // no imports, local supply already covers demand, so a price above reference eases back
        // by a percent a week instead of climbing to the import ceiling and staying there.
        if imported_last_week(world, product) > 0 || price < reference {
            target = price * 102 / 100;
        } else if price > reference {
            target = price * 99 / 100;
        }
    }
    // A local seller stays five percent under the cheapest import, so its goods still beat
    // imports once freight over a normal haul is added; asking the full import price made the
    // delivered price higher than imports and the corridor imported what it made.
    let ceiling = world
        .gateways
        .iter()
        .filter(|gateway| gateway.product == product)
        .map(|gateway| gateway.sell_price_cents)
        .filter(|price| *price > 0)
        .min()
        .map(|import| import * 95 / 100)
        .unwrap_or(reference + reference * 15 / 100);
    target
        .clamp(reference / 2, reference * 2)
        .min(ceiling)
        .max(1)
}

/// Production planning from sell-through, plant by plant: each plant follows the good it makes.
/// Goods: more than four weeks of unsold stock (beyond the firm's own input cover) cuts fifteen
/// points, down to thirty percent; under a week and a half raises fifteen points, up to full
/// capacity. Services keep no stock: sales under half of the firm's capacity for the service
/// cut, above ninety percent raise. Otherwise the plant holds. A plant whose good has not sold
/// yet, such as a new plant still waiting for inputs, holds too.
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
            let (glut, tight) = if dataset.products[product as usize].kind.is_service() {
                (sold * 2 < full, sold * 10 >= full * 9)
            } else {
                let stock = free_stock(world, dataset, firm, product);
                (
                    stock > sold.max(1) * glut_weeks(dataset, product),
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

/// A good's corridor price this week: the median asking price of the firms that make it, or
/// the cheapest import quote when nobody does, or the reference price.
pub fn corridor_price(world: &World, dataset: &Dataset, product: u16) -> i64 {
    let id = &dataset.products[product as usize].id;
    let mut asks: Vec<i64> = world
        .firms
        .iter()
        .filter(|(_, entity)| entity.operative())
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
    if !asks.is_empty() {
        asks.sort_unstable();
        return asks[asks.len() / 2].max(1);
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
