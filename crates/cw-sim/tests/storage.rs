//! Storage (1.7 preview 2): finite regional space by class, outside storage and its fee, holding
//! cost, spoilage inside owned cold storage, stock targets, speculative buy orders and the rule
//! that storage moves money but never creates or destroys stock except recorded spoilage.

use cw_core::ids::Handle;
use cw_core::ledger::Account;
use cw_core::money::Cents;
use cw_core::quantity::MilliLots;
use cw_data::{Dataset, StorageClass};
use cw_sim::commands::{CommandEnvelope, CommandIntent, INVESTOR_ACTOR, apply};
use cw_sim::economy::{production, seed, storage};
use cw_sim::state::{Facility, FacilityStatus, ScenarioKind, ScenarioMode, World};

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

fn add_plant(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    region: u8,
    recipe_id: &str,
    blocks: u8,
) -> Handle {
    let recipe_index = dataset.recipe_index[recipe_id];
    let recipe = &dataset.recipes[recipe_index as usize];
    let facility = Facility {
        firm,
        region,
        recipe: recipe_index,
        blocks,
        status: FacilityStatus::Operating,
        started: world.tick,
        completes: world.tick,
        escrow_cents: 0,
        capex_cents: recipe.capex_cents * blocks as i64,
        accumulated_depreciation: 0,
        workers: recipe
            .workers_low_mid_high
            .map(|workers| workers * blocks as i64),
        target_bps: 10_000,
        quality_bps: 10_000,
        deposit_remaining: MilliLots::ZERO,
        last_output: MilliLots::ZERO,
        last_inputs_short: false,
        last_staff_ratio_bps: 10_000,
        mothballed_since: None,
        equipment_due_cents: 0,
        outage: None,
        stock_plan: Default::default(),
        ship: Default::default(),
        serves: Vec::new(),
    };
    let handle = world.facilities.insert(facility);
    world.firms.get_mut(firm).unwrap().facilities.push(handle);
    handle
}

fn give(world: &mut World, firm: Handle, product: u16, region: u8, lots: i64) {
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .stock_lot_mut(product, region)
        .receive(MilliLots::from_lots(lots), Cents(0));
}

fn command(world: &mut World, dataset: &Dataset, intent: CommandIntent, id: u64) {
    let (revision, tick) = (world.revision, world.tick.0);
    apply(
        world,
        dataset,
        &CommandEnvelope {
            command_id_high: 77,
            command_id_low: id,
            actor: INVESTOR_ACTOR,
            expected_revision: revision,
            submitted_tick: tick,
            intent,
        },
    )
    .expect("command accepted");
}

fn total_cash(world: &World) -> i64 {
    world.firms.iter().map(|(_, f)| f.book.cash()).sum::<i64>() + world.outside.gateway_book.cash()
}

fn total_stock(world: &World, product: u16) -> i64 {
    world
        .firms
        .iter()
        .flat_map(|(_, f)| f.inventory.iter())
        .filter(|((p, _), _)| *p == product)
        .map(|(_, lot)| lot.qty.0)
        .sum()
}

/// Two distinct operating firms and a region both are free to use.
fn two_firms(world: &World) -> (Handle, Handle, u8) {
    let mut firms = world
        .firms
        .iter()
        .filter(|(_, f)| f.operative())
        .map(|(h, f)| (h, f.region));
    let (a, region) = firms.next().unwrap();
    let (b, _) = firms.next().unwrap();
    (a, b, region)
}

#[test]
fn owned_storage_adds_space_in_its_class_and_region() {
    let (mut world, dataset) = world(1101);
    let (firm, _, region) = two_firms(&world);
    let before = storage::space(&world, &dataset, world.firm(firm).unwrap());
    let cold_before = before
        .get(&(region, StorageClass::Cold))
        .copied()
        .unwrap_or_default();
    add_plant(&mut world, &dataset, firm, region, "cold_store", 2);
    let after = storage::space(&world, &dataset, world.firm(firm).unwrap());
    let cold = after[&(region, StorageClass::Cold)];
    assert_eq!(cold.owned, cold_before.owned + 2 * 5_000 * 1_000);
    assert_eq!(cold.yard, cold_before.yard, "a store adds no yard");
    // Another class is untouched.
    assert_eq!(
        after
            .get(&(region, StorageClass::Tank))
            .copied()
            .unwrap_or_default(),
        before
            .get(&(region, StorageClass::Tank))
            .copied()
            .unwrap_or_default()
    );
}

#[test]
fn overflow_pays_the_fee_to_a_firm_with_spare_space_and_money_is_conserved() {
    let (mut world, dataset) = world(1102);
    let (payer, landlord, region) = two_firms(&world);
    let steel = dataset.product_index["steel"];
    add_plant(&mut world, &dataset, landlord, region, "warehouse", 3);
    let space = storage::space(&world, &dataset, world.firm(payer).unwrap());
    let capacity = space
        .get(&(region, StorageClass::Dry))
        .copied()
        .unwrap_or_default()
        .total()
        / 1_000;
    let used_before = storage::uses(&world, &dataset, world.firm(payer).unwrap())
        .iter()
        .find(|u| u.region == region && u.class == StorageClass::Dry)
        .map(|u| u.used_milli / 1_000)
        .unwrap_or(0);
    // Fill the payer's dry space and put 400 lots beyond it.
    let extra = capacity - used_before + 400;
    give(&mut world, payer, steel, region, extra);
    world
        .firms
        .get_mut(payer)
        .unwrap()
        .book
        .commit(
            world.tick,
            cw_core::ledger::Txn::new()
                .debit(Account::Cash, 50_000_000, "test")
                .credit(Account::ShareCapital, 50_000_000, "test"),
        )
        .ok();
    let cash = total_cash(&world);
    let payer_cash = world.firm(payer).unwrap().book.cash();
    let landlord_cash = world.firm(landlord).unwrap().book.cash();
    let stock_before = total_stock(&world, steel);

    storage::weekly(&mut world, &dataset);

    let week = world.firm(payer).unwrap().storage.last;
    assert_eq!(week.outside_lots, 400);
    assert_eq!(week.fee_cents, 400 * StorageClass::Dry.outside_fee_cents());
    assert!(
        week.holding_cents > 0,
        "stock beyond the yard carries a holding cost"
    );
    assert_eq!(
        payer_cash - world.firm(payer).unwrap().book.cash(),
        week.fee_cents + week.holding_cents
    );
    assert_eq!(
        world.firm(landlord).unwrap().book.cash() - landlord_cash,
        week.fee_cents,
        "the fee is the landlord's rent"
    );
    assert_eq!(
        world.firm(landlord).unwrap().storage.last.rent_cents,
        week.fee_cents
    );
    assert_eq!(
        total_cash(&world),
        cash,
        "storage moves money, it makes none"
    );
    assert_eq!(
        total_stock(&world, steel),
        stock_before,
        "and moves no stock"
    );
    for (_, f) in world.firms.iter() {
        assert!(f.book.is_balanced());
    }
    assert!(world.outside.gateway_book.is_balanced());
}

#[test]
fn with_no_landlord_the_fee_goes_to_the_outside_sector() {
    let (mut world, dataset) = world(1103);
    let (payer, _, region) = two_firms(&world);
    let steel = dataset.product_index["steel"];
    give(&mut world, payer, steel, region, 60_000);
    let outside = world.outside.gateway_book.cash();
    let payer_cash = world.firm(payer).unwrap().book.cash();
    storage::weekly(&mut world, &dataset);
    let week = world.firm(payer).unwrap().storage.last;
    assert!(week.fee_cents > 0 && week.outside_lots > 0);
    let paid = payer_cash - world.firm(payer).unwrap().book.cash();
    assert_eq!(paid, week.fee_cents + week.holding_cents);
    assert_eq!(world.outside.gateway_book.cash() - outside, paid);
}

#[test]
fn a_firm_with_room_in_its_yard_pays_nothing() {
    let (mut world, dataset) = world(1104);
    for (_, f) in world.firms.iter_mut() {
        f.inventory.clear();
    }
    let cash = total_cash(&world);
    storage::weekly(&mut world, &dataset);
    assert_eq!(total_cash(&world), cash);
    for (_, f) in world.firms.iter() {
        assert_eq!(f.storage.last.fee_cents + f.storage.last.holding_cents, 0);
    }
}

#[test]
fn spoilage_is_cut_hard_inside_owned_cold_storage_and_recorded() {
    let (mut world, dataset) = world(1105);
    let food = dataset.product_index["food_raw"];
    let (firm, other, region) = two_firms(&world);
    for f in [firm, other] {
        world
            .firms
            .get_mut(f)
            .unwrap()
            .inventory
            .retain(|(p, _), _| *p != food);
    }
    // Both hold 1,000 lots; only one has a cold store that covers it.
    give(&mut world, firm, food, region, 1_000);
    give(&mut world, other, food, region, 1_000);
    add_plant(&mut world, &dataset, firm, region, "cold_store", 1);
    let pair = |w: &World| {
        [firm, other]
            .iter()
            .map(|f| w.firm(*f).unwrap().stock(food, region).0)
            .sum::<i64>()
    };
    let total_before = pair(&world);
    production::spoil_perishables(&mut world, &dataset);
    let lost_owner = world.firm(firm).unwrap().storage.last.spoiled_milli;
    let lost_other = world.firm(other).unwrap().storage.last.spoiled_milli;
    // Food spoils 4 percent a week: 40 lots in the open, 8 in a cold store.
    assert_eq!(lost_other, 40_000);
    assert_eq!(lost_owner, 8_000);
    assert_eq!(
        total_before - pair(&world),
        lost_owner + lost_other,
        "the only stock destroyed is the stock recorded as spoiled"
    );
    assert_eq!(
        world.firm(firm).unwrap().storage.spoiled_total_milli,
        lost_owner
    );
    for f in [firm, other] {
        let e = world.firm(f).unwrap();
        assert!(e.storage.last.spoiled_cents >= 0);
        assert!(e.book.is_balanced());
        let lots: i64 = e.inventory.values().map(|l| l.cost_cents).sum();
        assert_eq!(lots, e.book.balance(Account::Inventory));
    }
}

#[test]
fn a_stock_target_sets_input_cover_and_the_harvest_builds_grain_ahead() {
    let (mut world, dataset) = world(1106);
    let grain = dataset.product_index["grain"];
    let fuel = dataset.product_index["fuel"];
    let food_raw = dataset.product_index["food_raw"];
    let firm = world.investor.primary_group.unwrap();
    let region = world.firm(firm).unwrap().region;
    let plant = add_plant(&mut world, &dataset, firm, region, "food_processing", 1);
    let entity = world.firm(firm).unwrap();
    let facility = world.facilities.get(plant).unwrap();
    // Default cover is the firm's policy; perishable inputs are capped at two weeks.
    let default = entity.policies.input_cover_weeks as i64;
    assert_eq!(
        storage::plant_cover_weeks(&world, &dataset, entity, facility, fuel),
        default
    );
    assert_eq!(
        storage::plant_cover_weeks(&world, &dataset, entity, facility, food_raw),
        2
    );
    // Outside the harvest quarter grain follows the default; in it, eight weeks more.
    world.tick = cw_core::calendar::Tick(2);
    let entity = world.firm(firm).unwrap();
    let facility = world.facilities.get(plant).unwrap();
    assert_eq!(
        storage::plant_cover_weeks(&world, &dataset, entity, facility, grain),
        default
    );
    world.tick = cw_core::calendar::Tick(30);
    assert_eq!(world.tick.quarter(), storage::HARVEST_QUARTER);
    let entity = world.firm(firm).unwrap();
    let facility = world.facilities.get(plant).unwrap();
    assert_eq!(
        storage::plant_cover_weeks(&world, &dataset, entity, facility, grain),
        default + storage::SEASON_EXTRA_WEEKS
    );
    // The player's target replaces the default, and switching the seasonal build off removes it.
    command(
        &mut world,
        &dataset,
        CommandIntent::SetStockTarget {
            facility: plant,
            weeks: 10,
            seasonal: false,
        },
        1,
    );
    let entity = world.firm(firm).unwrap();
    let facility = world.facilities.get(plant).unwrap();
    assert_eq!(
        storage::plant_cover_weeks(&world, &dataset, entity, facility, grain),
        10
    );
    assert_eq!(
        storage::plant_cover_weeks(&world, &dataset, entity, facility, fuel),
        10
    );
}

#[test]
fn a_buy_order_adds_to_held_stock_which_is_not_offered_for_sale_until_released() {
    let (mut world, dataset) = world(1107);
    let firm = world.investor.primary_group.unwrap();
    let steel = dataset.product_index["steel"];
    let region = world.firm(firm).unwrap().region;
    command(
        &mut world,
        &dataset,
        CommandIntent::BuyToStore {
            firm,
            product: steel,
            region,
            lots: 120,
            limit_bps: 20_000,
        },
        2,
    );
    assert_eq!(world.firm(firm).unwrap().storage.orders.len(), 1);
    let cash_start = total_cash(&world);
    for _ in 0..6 {
        cw_sim::tick::advance(&mut world, &dataset);
        let f = world.firm(firm).unwrap();
        assert!(
            f.storage.held_of(steel, region).0 <= f.stock(steel, region).0,
            "held goods are always on hand"
        );
        for (_, other) in world.firms.iter() {
            assert!(other.book.is_balanced());
        }
    }
    let held = world.firm(firm).unwrap().storage.held_of(steel, region);
    assert!(held.0 > 0, "the order filled from the exchange");
    assert!(held.lots() <= 120, "never more than the order asked");
    assert_ne!(
        total_cash(&world),
        cash_start + 1,
        "money moved only by trades"
    );
    // Held goods count as reserved, so no one can buy them off the firm.
    let entity = world.firm(firm).unwrap();
    let reserved = storage::reserved_milli(&world, &dataset, entity, region, steel);
    assert!(reserved >= held.0);
    // Releasing makes them ordinary stock again and cancels what is left of the order.
    command(
        &mut world,
        &dataset,
        CommandIntent::SellFromStore {
            firm,
            product: steel,
            region,
            lots: 0,
        },
        3,
    );
    let entity = world.firm(firm).unwrap();
    assert_eq!(entity.storage.held_of(steel, region).0, 0);
    assert!(entity.storage.orders.is_empty());
}

#[test]
fn a_year_with_storage_activity_keeps_every_book_and_lot_balanced() {
    let (mut world, dataset) = world(1108);
    let firm = world.investor.primary_group.unwrap();
    let region = world.firm(firm).unwrap().region;
    let steel = dataset.product_index["steel"];
    let grain = dataset.product_index["grain"];
    add_plant(&mut world, &dataset, firm, region, "grain_silo", 1);
    for (id, product) in [(4u64, steel), (5, grain)] {
        command(
            &mut world,
            &dataset,
            CommandIntent::BuyToStore {
                firm,
                product,
                region,
                lots: 200,
                limit_bps: 12_000,
            },
            id,
        );
    }
    for week in 0..52 {
        cw_sim::tick::advance(&mut world, &dataset);
        for (_, f) in world.firms.iter() {
            assert!(f.book.is_balanced(), "week {week}: {} balanced", f.name);
            for ((product, r), held) in &f.storage.held {
                assert!(held.0 <= f.stock(*product, *r).0, "held within stock");
            }
            // The recorded spoilage can never exceed what was ever lost.
            assert!(f.storage.recent(13).spoiled_milli <= f.storage.spoiled_total_milli);
        }
        assert!(world.outside.gateway_book.is_balanced());
        let entity = world.firm(firm).unwrap();
        let lots: i64 = entity.inventory.values().map(|l| l.cost_cents).sum();
        assert_eq!(
            lots,
            entity.book.balance(Account::Inventory),
            "week {week}: the player's lot cost equals Inventory"
        );
    }
}

#[test]
fn bad_orders_are_refused() {
    let (mut world, dataset) = world(1109);
    let firm = world.investor.primary_group.unwrap();
    let region = world.firm(firm).unwrap().region;
    let electricity = dataset.product_index["electricity"];
    let steel = dataset.product_index["steel"];
    let bad = |intent: CommandIntent, world: &mut World| {
        let (revision, tick) = (world.revision, world.tick.0);
        apply(
            world,
            &dataset,
            &CommandEnvelope {
                command_id_high: 78,
                command_id_low: world.revision + 1_000,
                actor: INVESTOR_ACTOR,
                expected_revision: revision,
                submitted_tick: tick,
                intent,
            },
        )
        .is_err()
    };
    // Services cannot be stored, nor can limits or sizes leave their ranges.
    assert!(bad(
        CommandIntent::BuyToStore {
            firm,
            product: electricity,
            region,
            lots: 10,
            limit_bps: 10_000
        },
        &mut world
    ));
    assert!(bad(
        CommandIntent::BuyToStore {
            firm,
            product: steel,
            region,
            lots: 0,
            limit_bps: 10_000
        },
        &mut world
    ));
    assert!(bad(
        CommandIntent::BuyToStore {
            firm,
            product: steel,
            region,
            lots: 10,
            limit_bps: 90_000
        },
        &mut world
    ));
    let plant = world.firm(firm).unwrap().facilities[0];
    assert!(bad(
        CommandIntent::SetStockTarget {
            facility: plant,
            weeks: 99,
            seasonal: true
        },
        &mut world
    ));
}
