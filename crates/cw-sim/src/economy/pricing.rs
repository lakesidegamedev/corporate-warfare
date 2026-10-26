//! Pricing from sell-through, shared by rival AI and the player's automatic pricing.
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
    }
}
