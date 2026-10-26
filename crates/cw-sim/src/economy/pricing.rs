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

/// Goods a firm's plants make.
pub fn outputs(world: &World, dataset: &Dataset, firm: Handle) -> BTreeSet<u16> {
    let Some(entity) = world.firms.get(firm) else {
        return BTreeSet::new();
    };
    entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .filter_map(|plant| {
            dataset
                .product_index
                .get(&dataset.recipes[plant.recipe as usize].output)
                .copied()
        })
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
    (held - weekly_need * entity.policies.input_cover_weeks.max(1) as i64).max(0)
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
    if (sold == 0 && stock > 0) || stock > sold * 4 {
        target = price * 95 / 100;
    } else if stock < sold {
        target = price * 102 / 100;
    }
    let ceiling = world
        .gateways
        .iter()
        .filter(|gateway| gateway.product == product)
        .map(|gateway| gateway.sell_price_cents)
        .filter(|price| *price > 0)
        .min()
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
                (stock > sold.max(1) * 4, sold > 0 && stock * 2 < sold * 3)
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
