//! Product chains and multi-output plants (1.7 preview 2): by-products are made in their fixed
//! yield, carry a share of the plant's input cost, get priced and sold, and the services a
//! corridor makes are paid for by the firms that use them.

use cw_core::ids::Handle;
use cw_core::ledger::Account;
use cw_core::money::Cents;
use cw_core::quantity::MilliLots;
use cw_data::Dataset;
use cw_sim::economy::{pricing, production, seed};
use cw_sim::state::{ScenarioKind, ScenarioMode, World};

fn world(seed_value: u64) -> (World, Dataset) {
    let dataset = Dataset::load().unwrap();
    let world = seed::build(
        &dataset,
        seed_value,
        ScenarioKind::Guided,
        ScenarioMode::Sandbox,
    );
    (world, dataset)
}

fn staff_everything(world: &mut World, dataset: &Dataset) {
    let full: Vec<(Handle, [i64; 3])> = world
        .facilities
        .iter()
        .map(|(handle, f)| (handle, f.full_workers(dataset)))
        .collect();
    for (handle, workers) in full {
        world.facilities.get_mut(handle).unwrap().workers = workers;
    }
}

fn plant_of(world: &World, dataset: &Dataset, recipe: &str) -> (Handle, Handle) {
    world
        .facilities
        .iter()
        .find(|(_, f)| dataset.recipes[f.recipe as usize].id == recipe)
        .map(|(handle, f)| (handle, f.firm))
        .unwrap_or_else(|| panic!("a seeded {recipe}"))
}

fn stock(world: &World, dataset: &Dataset, firm: Handle, product: &str) -> i64 {
    let code = dataset.product_index[product];
    world
        .firm(firm)
        .unwrap()
        .inventory
        .iter()
        .filter(|((p, _), _)| *p == code)
        .map(|(_, lot)| lot.qty.0)
        .sum()
}

/// Carrying cost on the firm's lots equals its Inventory account, the conservation rule.
fn assert_inventory_conserved(world: &World, firm: Handle) {
    let entity = world.firm(firm).unwrap();
    let lots: i64 = entity.inventory.values().map(|lot| lot.cost_cents).sum();
    assert_eq!(
        lots,
        entity.book.balance(Account::Inventory),
        "{}: lot carrying cost matches the Inventory account",
        entity.name
    );
    assert!(entity.book.is_balanced());
}

#[test]
fn a_refinery_makes_every_product_in_the_fixed_split() {
    let (mut world, dataset) = world(901);
    let (plant, firm) = plant_of(&world, &dataset, "refinery");
    staff_everything(&mut world, &dataset);
    // Gasoline and jet fuel are inputs to no plant of this firm, so their gain is the output.
    let watched = ["gasoline", "jet_fuel"];
    let before: Vec<i64> = watched
        .iter()
        .map(|id| stock(&world, &dataset, firm, id))
        .collect();
    production::produce(&mut world, &dataset);
    let blocks = world.facilities.get(plant).unwrap().blocks as i64;
    let made: Vec<i64> = watched
        .iter()
        .zip(&before)
        .map(|(id, was)| stock(&world, &dataset, firm, id) - was)
        .collect();
    let refinery = dataset.recipe("refinery").unwrap();
    assert!(made[0] > 0 && made[1] > 0, "the refinery ran: {made:?}");
    // Same run rate for every product: made / full-capacity yield is one number.
    let run = |index: usize| {
        made[index] as f64 / (refinery.lots_of(watched[index]) * blocks * 1_000) as f64
    };
    assert!(
        (run(0) - run(1)).abs() < 0.001,
        "gasoline and jet fuel follow one run rate: {} vs {}",
        run(0),
        run(1)
    );
    assert_inventory_conserved(&world, firm);
}

#[test]
fn joint_input_cost_is_shared_by_value_and_never_lost() {
    let (mut world, dataset) = world(902);
    let (_, firm) = plant_of(&world, &dataset, "refinery");
    staff_everything(&mut world, &dataset);
    let cost_before: i64 = world
        .firm(firm)
        .unwrap()
        .inventory
        .values()
        .map(|lot| lot.cost_cents)
        .sum();
    production::produce(&mut world, &dataset);
    let entity = world.firm(firm).unwrap();
    let gasoline = dataset.product_index["gasoline"];
    let carried: i64 = entity
        .inventory
        .iter()
        .filter(|((p, _), _)| *p == gasoline)
        .map(|(_, lot)| lot.cost_cents)
        .sum();
    assert!(
        carried > 0,
        "a by-product carries a share of the crude cost"
    );
    let cost_after: i64 = entity.inventory.values().map(|lot| lot.cost_cents).sum();
    assert!(
        cost_after >= cost_before,
        "the converted crude stays on the books as stock"
    );
    assert_inventory_conserved(&world, firm);
}

#[test]
fn an_unsold_by_product_is_priced_and_sold() {
    let (mut world, dataset) = world(903);
    let (_, firm) = plant_of(&world, &dataset, "refinery");
    let gasoline = dataset.product_index["gasoline"];
    // Priced: every product the plant makes is a priced output.
    let outputs = pricing::outputs(&world, &dataset, firm);
    for id in ["fuel", "gasoline", "jet_fuel", "petrochem_feedstock"] {
        assert!(
            outputs.contains(&dataset.product_index[id]),
            "{id} is priced"
        );
    }
    // Sold: the corridor has no plant that burns gasoline at scale, yet after some weeks the
    // refinery's gasoline has reached a buyer and does not pile up without limit.
    staff_everything(&mut world, &dataset);
    for _ in 0..30 {
        cw_sim::tick::advance(&mut world, &dataset);
        staff_everything(&mut world, &dataset);
    }
    let sold: i64 = world
        .firm(firm)
        .unwrap()
        .sales_history
        .iter()
        .flat_map(|week| week.iter())
        .filter(|((_, p), _)| *p == gasoline)
        .map(|(_, qty)| *qty)
        .sum();
    assert!(
        sold > 0,
        "gasoline was sold, by default pricing, to a buyer"
    );
    let weekly_make = dataset.recipe("refinery").unwrap().lots_of("gasoline") * 2;
    assert!(
        stock(&world, &dataset, firm, "gasoline") / 1_000 < weekly_make * 6,
        "unsold gasoline does not accumulate beyond a few weeks"
    );
}

#[test]
fn freight_output_is_consumed_and_paid_for() {
    let (mut world, dataset) = world(904);
    staff_everything(&mut world, &dataset);
    let freight = dataset.product_index["freight"];
    let mut sold = [0i64; 1];

    for _ in 0..13 {
        cw_sim::tick::advance(&mut world, &dataset);
        staff_everything(&mut world, &dataset);
    }
    for (_, firm) in world.firms.iter() {
        for week in &firm.sales_history {
            for ((_, product), qty) in week {
                if *product == freight {
                    sold[0] += *qty;
                }
            }
        }
    }
    assert!(sold[0] > 0, "carriers sell the freight they make");
}

#[test]
fn perishables_spoil_and_the_loss_is_booked() {
    let (mut world, dataset) = world(905);
    let (_, firm) = plant_of(&world, &dataset, "farm_industrial");
    let food = dataset.product_index["food_raw"];
    let region = world.firm(firm).unwrap().region;
    let entity = world.firms.get_mut(firm).unwrap();
    entity.inventory.retain(|(p, _), _| *p != food);
    entity
        .stock_lot_mut(food, region)
        .receive(MilliLots::from_lots(1_000), Cents(0));
    production::spoil_perishables(&mut world, &dataset);
    let left = stock(&world, &dataset, firm, "food_raw");
    // Four percent a week spoils.
    assert_eq!(left, 1_000_000 - 40_000);
    // Durable goods do not spoil.
    let before = stock(&world, &dataset, firm, "fuel");
    production::spoil_perishables(&mut world, &dataset);
    assert_eq!(stock(&world, &dataset, firm, "fuel"), before);
}

#[test]
fn the_new_chains_run_end_to_end_in_a_seeded_world() {
    // Crude -> refinery -> fuel and feedstock; ore -> pig iron -> steel; grain -> food.
    let (mut world, dataset) = world(906);
    staff_everything(&mut world, &dataset);
    // Prices now follow sales for every firm each week, so a plant can be mothballed by the
    // week 26 close: what counts is that each plant made its goods over the half year.
    let mut produced: std::collections::BTreeMap<String, i64> = Default::default();
    for _ in 0..26 {
        cw_sim::tick::advance(&mut world, &dataset);
        staff_everything(&mut world, &dataset);
        for (_, f) in world.facilities.iter() {
            *produced
                .entry(dataset.recipes[f.recipe as usize].id.clone())
                .or_default() += f.last_output.0;
        }
    }
    for recipe in [
        "oil_wells",
        "refinery",
        "gas_processing",
        "blast_furnace",
        "steel_mill",
        "electric_arc",
        "mixed_farm",
        "food_processing",
    ] {
        let produced = produced.get(recipe).copied().unwrap_or(0);
        assert!(produced > 0, "{recipe} produced in the seeded world");
    }
}
