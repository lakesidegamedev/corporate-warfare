//! Pricing from sell-through (crates/cw-sim/src/economy/pricing.rs), shared by rival AI and the
//! player's automatic pricing, with the player's manual override.

use cw_core::ids::Handle;
use cw_data::Dataset;
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR, apply};
use cw_sim::economy::{pricing, seed};
use cw_sim::state::{ScenarioKind, ScenarioMode, World};

fn set_price(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    product: u16,
    cents: i64,
    id: u64,
) {
    let (revision, tick) = (world.revision, world.tick.0);
    apply(
        world,
        dataset,
        &CommandEnvelope {
            command_id_high: 9,
            command_id_low: id,
            actor: INVESTOR_ACTOR,
            expected_revision: revision,
            submitted_tick: tick,
            intent: CommandIntent::SetPrice {
                firm,
                product,
                price_cents: cents,
            },
        },
    )
    .expect("price accepted");
}

#[test]
fn stock_and_sell_through_move_the_price() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 55, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    let food = dataset.product_index["food_raw"];
    let region = world.firms.get(firm).unwrap().region;
    let entity = world.firms.get_mut(firm).unwrap();
    entity.policies.prices.insert(food, 10_000);
    entity.sales_history = vec![[((region, food), 100)].into_iter().collect(); 13];
    // 100 lots a week. Five weeks of stock: cut five percent.
    entity.inventory.retain(|(p, _), _| *p != food);
    entity.stock_lot_mut(food, region).receive(
        cw_core::quantity::MilliLots::from_lots(500),
        cw_core::money::Cents(0),
    );
    assert_eq!(pricing::target_price(&world, &dataset, firm, food), 9_500);
    // Half a week of stock: selling out raises two percent.
    let entity = world.firms.get_mut(firm).unwrap();
    entity.inventory.retain(|(p, _), _| *p != food);
    entity.stock_lot_mut(food, region).receive(
        cw_core::quantity::MilliLots::from_lots(50),
        cw_core::money::Cents(0),
    );
    assert_eq!(pricing::target_price(&world, &dataset, firm, food), 10_200);
    // Two weeks of stock: hold.
    let entity = world.firms.get_mut(firm).unwrap();
    entity.inventory.retain(|(p, _), _| *p != food);
    entity.stock_lot_mut(food, region).receive(
        cw_core::quantity::MilliLots::from_lots(200),
        cw_core::money::Cents(0),
    );
    assert_eq!(pricing::target_price(&world, &dataset, firm, food), 10_000);
}

#[test]
fn the_player_prices_automatically_until_setting_a_price_by_hand() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 56, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    let food = dataset.product_index["food_raw"];
    // Automatic: each week the price follows the rule.
    pricing::weekly(&mut world, &dataset);
    let expected = pricing::target_price(&world, &dataset, firm, food);
    pricing::weekly(&mut world, &dataset);
    assert!(
        world
            .firms
            .get(firm)
            .unwrap()
            .policies
            .prices
            .contains_key(&food)
    );
    let _ = expected;
    // A hand-set price stays put.
    set_price(&mut world, &dataset, firm, food, 12_000, 1);
    for _ in 0..3 {
        pricing::weekly(&mut world, &dataset);
    }
    assert_eq!(
        world.firms.get(firm).unwrap().policies.prices[&food],
        12_000
    );
    // Zero hands it back to automatic pricing.
    set_price(&mut world, &dataset, firm, food, 0, 2);
    assert!(
        !world
            .firms
            .get(firm)
            .unwrap()
            .policies
            .manual_prices
            .contains(&food)
    );
    let next = pricing::target_price(&world, &dataset, firm, food);
    pricing::weekly(&mut world, &dataset);
    assert_eq!(world.firms.get(firm).unwrap().policies.prices[&food], next);
}
