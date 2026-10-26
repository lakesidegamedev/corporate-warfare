//! Pricing and production planning from sell-through, shared by rival AI and the player's
//! automatic operations.
//!
//! A firm sets one price per good it makes. Weekly sales are its closed thirteen-week sales of
//! the good in every region divided by thirteen; stock is everything it holds of the good. More
//! than four weeks of stock (or none sold while stock sits) cuts the price five percent; selling
//! out, with less than a week of stock, raises it two percent; otherwise it holds. The result
//! stays within half and twice reference and never above the cheapest import price in the
//! corridor, above which buyers import instead (services, with no import quote, stop at fifteen
//! percent over reference).

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
    let stock: i64 = entity
        .inventory
        .iter()
        .filter(|((p, _), _)| *p == product)
        .map(|(_, lot)| lot.qty.lots())
        .sum();
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

/// Production target in basis points from sell-through. Goods: more than four weeks of unsold
/// stock in any good cuts fifteen points, down to thirty percent; under a week and a half of
/// stock in every good raises fifteen points, up to full capacity. Services keep no stock:
/// sales under half of full capacity cut, above ninety percent raise. Otherwise it holds.
pub fn production_target(world: &World, dataset: &Dataset, firm: Handle) -> i32 {
    let Some(entity) = world.firms.get(firm) else {
        return 10_000;
    };
    let current = entity.policies.production_target_bps;
    let mut capacity: std::collections::BTreeMap<u16, i64> = std::collections::BTreeMap::new();
    for plant in entity
        .facilities
        .iter()
        .filter_map(|handle| world.facilities.get(*handle))
        .filter(|plant| plant.status == crate::state::FacilityStatus::Operating)
    {
        let recipe = &dataset.recipes[plant.recipe as usize];
        if let Some(code) = dataset.product_index.get(&recipe.output) {
            *capacity.entry(*code).or_default() += recipe.weekly_output_lots * plant.blocks as i64;
        }
    }
    if capacity.is_empty() {
        return current;
    }
    let mut glut = false;
    let mut tight = true;
    for (product, full) in &capacity {
        let sold = weekly_sold(world, firm, *product);
        if dataset.products[*product as usize].kind.is_service() {
            if sold * 2 < *full {
                glut = true;
            }
            if sold * 10 < *full * 9 {
                tight = false;
            }
            continue;
        }
        let stock: i64 = entity
            .inventory
            .iter()
            .filter(|((p, _), _)| p == product)
            .map(|(_, lot)| lot.qty.lots())
            .sum();
        if stock > sold.max(1) * 4 {
            glut = true;
        }
        if stock * 2 >= sold * 3 {
            tight = false;
        }
    }
    if glut {
        (current - 1_500).max(3_000)
    } else if tight {
        (current + 1_500).min(10_000)
    } else {
        current
    }
}

/// The player's automatic pricing: every good a player company makes, unless the player set
/// its price by hand, follows the sell-through rule each week.
pub fn weekly(world: &mut World, dataset: &Dataset) {
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
            let target = production_target(world, dataset, firm);
            set_target(world, firm, target);
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
