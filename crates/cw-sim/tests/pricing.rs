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

#[test]
fn production_follows_stock_against_sales_plant_by_plant() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 57, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    let food = dataset.product_index["food_raw"];
    let region = world.firms.get(firm).unwrap().region;
    let plant = *world
        .firms
        .get(firm)
        .unwrap()
        .facilities
        .iter()
        .find(|h| {
            let f = world.facilities.get(**h).unwrap();
            dataset.recipes[f.recipe as usize].output == "food_raw"
        })
        .unwrap();
    let set_stock = |world: &mut World, lots: i64| {
        let entity = world.firms.get_mut(firm).unwrap();
        entity.inventory.retain(|(p, _), _| *p != food);
        entity.stock_lot_mut(food, region).receive(
            cw_core::quantity::MilliLots::from_lots(lots),
            cw_core::money::Cents(0),
        );
    };
    let planned = |world: &mut World, current: i32| {
        world.facilities.get_mut(plant).unwrap().target_bps = current;
        pricing::plant_targets(world, &dataset, firm)
            .into_iter()
            .find(|(h, _)| *h == plant)
            .unwrap()
            .1
    };
    world.firms.get_mut(firm).unwrap().sales_history =
        vec![[((region, food), 100)].into_iter().collect(); 13];
    // Five weeks of stock: cut fifteen points.
    set_stock(&mut world, 500);
    assert_eq!(planned(&mut world, 10_000), 8_500);
    // One week of stock at 85 percent: raise fifteen points back to full.
    set_stock(&mut world, 100);
    assert_eq!(planned(&mut world, 8_500), 10_000);
    // A deep glut stops at thirty percent.
    set_stock(&mut world, 5_000);
    assert_eq!(planned(&mut world, 3_500), 3_000);
    // Nothing sold yet and nothing in stock, as at a new plant waiting for inputs: hold.
    world.firms.get_mut(firm).unwrap().sales_history.clear();
    set_stock(&mut world, 0);
    assert_eq!(planned(&mut world, 7_000), 7_000);
}

#[test]
fn input_cover_for_the_firms_own_plants_is_not_unsold_stock() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 57, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    // The farm burns fuel; whatever fuel it holds up to its cover weeks is its own input.
    let fuel = dataset.product_index["fuel"];
    let entity = world.firms.get(firm).unwrap();
    let weekly: i64 = entity
        .facilities
        .iter()
        .map(|h| world.facilities.get(*h).unwrap())
        .filter(|f| f.status == cw_sim::state::FacilityStatus::Operating)
        .map(|f| {
            dataset.recipes[f.recipe as usize]
                .weekly_inputs_lots
                .get("fuel")
                .copied()
                .unwrap_or(0)
                * f.blocks as i64
        })
        .sum();
    assert!(weekly > 0, "the guided farm uses fuel");
    let cover = weekly * entity.policies.input_cover_weeks.max(1) as i64;
    let region = entity.region;
    let entity = world.firms.get_mut(firm).unwrap();
    entity.inventory.retain(|(p, _), _| *p != fuel);
    entity.stock_lot_mut(fuel, region).receive(
        cw_core::quantity::MilliLots::from_lots(cover + 7),
        cw_core::money::Cents(0),
    );
    assert_eq!(pricing::free_stock(&world, &dataset, firm, fuel), 7);
}

fn posture(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    product: u16,
    mode: u8,
    bps: i32,
) -> bool {
    let (revision, tick) = (world.revision, world.tick.0);
    apply(
        world,
        dataset,
        &CommandEnvelope {
            command_id_high: 10,
            command_id_low: revision,
            actor: INVESTOR_ACTOR,
            expected_revision: revision,
            submitted_tick: tick,
            intent: CommandIntent::SetPricePosture {
                firm,
                product,
                mode,
                bps,
            },
        },
    )
    .is_ok()
}

#[test]
fn a_market_posture_prices_against_the_other_sellers_median() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 61, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    let food = dataset.product_index["food_raw"];
    // Every other farm asks a known price: $90, $100 and $120 in turn.
    let others: Vec<Handle> = world
        .firms
        .iter()
        .filter(|(h, f)| {
            *h != firm
                && f.facilities.iter().any(|p| {
                    world
                        .facilities
                        .get(*p)
                        .is_some_and(|p| dataset.recipes[p.recipe as usize].output == "food_raw")
                })
        })
        .map(|(h, _)| h)
        .collect();
    assert!(!others.is_empty(), "rival farms exist");
    let asks = [9_000i64, 10_000, 12_000];
    for (index, other) in others.iter().enumerate() {
        world
            .firms
            .get_mut(*other)
            .unwrap()
            .policies
            .prices
            .insert(food, asks[index % 3]);
    }
    let mut sorted: Vec<i64> = (0..others.len()).map(|i| asks[i % 3]).collect();
    sorted.sort();
    let median = sorted[sorted.len() / 2];
    // Ten percent under the market.
    assert!(posture(&mut world, &dataset, firm, food, 1, -1_000));
    let price = world.firms.get(firm).unwrap().policies.prices[&food];
    assert_eq!(price, median * 9 / 10);
    // The weekly review keeps it there while the market holds.
    pricing::weekly(&mut world, &dataset);
    assert_eq!(
        world.firms.get(firm).unwrap().policies.prices[&food],
        median * 9 / 10
    );
    // Out of range is refused; mode 0 hands the good back to automatic pricing.
    assert!(!posture(&mut world, &dataset, firm, food, 1, 2_500));
    assert!(!posture(&mut world, &dataset, firm, food, 2, -100));
    assert!(posture(&mut world, &dataset, firm, food, 0, 0));
    let policies = &world.firms.get(firm).unwrap().policies;
    assert!(policies.price_postures.is_empty());
    assert!(!policies.manual_prices.contains(&food));
}

#[test]
fn a_cost_plus_posture_marks_up_the_cost_of_a_lot() {
    let dataset = Dataset::load().unwrap();
    let mut world = seed::build(&dataset, 61, ScenarioKind::Guided, ScenarioMode::Sandbox);
    let firm = world.investor.primary_group.unwrap();
    let food = dataset.product_index["food_raw"];
    // Hand-worked cost of a lot from the farm's recipe, the firm's wage offers and this
    // season's crew.
    let entity = world.firms.get(firm).unwrap();
    let crew_bps = {
        let quarter = world.tick.quarter().saturating_sub(1) % 4;
        [8_000i64, 10_000, 10_000, 8_000][quarter as usize]
    };
    let (mut cost, mut lots) = (0i64, 0i64);
    for plant in entity
        .facilities
        .iter()
        .filter_map(|h| world.facilities.get(*h))
    {
        let recipe = &dataset.recipes[plant.recipe as usize];
        if recipe.output != "food_raw" || plant.status != cw_sim::state::FacilityStatus::Operating {
            continue;
        }
        let inputs: i64 = recipe
            .weekly_inputs_lots
            .iter()
            .map(|(id, q)| q * dataset.reference_price(dataset.product_index[id]))
            .sum();
        let wages: i64 = (0..3)
            .map(|t| {
                recipe.workers_low_mid_high[t] * entity.policies.wage_offer[t] * crew_bps / 10_000
            })
            .sum();
        let fixed = recipe.capex_cents * dataset.maintenance_annual_bps as i64 / 10_000 / 52
            + dataset.fixture_land_rent_cents_weekly
            + recipe.capex_cents / (recipe.life_years as i64 * 52);
        cost += (inputs + wages + fixed) * plant.blocks as i64;
        lots += recipe.weekly_output_lots * plant.blocks as i64;
    }
    assert!(lots > 0, "the guided firm farms");
    let unit = cost / lots;
    // Rounding in the recipe helpers is at most a cent a plant.
    let engine = pricing::unit_cost(&world, &dataset, firm, food).unwrap();
    assert!((engine - unit).abs() <= 1, "engine {engine} vs hand {unit}");
    assert!(posture(&mut world, &dataset, firm, food, 2, 2_000));
    let reference = dataset.reference_price(food);
    let expected = (engine * 12_000 / 10_000).clamp(reference / 2, reference * 2);
    assert_eq!(
        world.firms.get(firm).unwrap().policies.prices[&food],
        expected
    );
}
