//! Worked scenario per industry (execution packet P25).
//!
//! One deterministic scenario for each of the fifteen industries in
//! `docs/planning/DATA-CONTENT.md`. Each scenario seeds the pinned world with
//! `economy::seed::build`, staffs a real facility, advances or produces, and asserts a
//! physical or accounting outcome that only that industry model can produce. Generic
//! factory behaviour (buy inputs, make output, sell it) is not evidence.
//!
//! Assertions use documented rules and independent oracles (conservation, the seasonal
//! index in CONTENT-PACING, stoichiometric ratios computed from the recipe, the premium
//! formula in FINANCE-LABOUR-AI) rather than mirroring engine internals. Every scenario
//! also checks the world-wide invariants: balanced double-entry ledgers, non-negative
//! inventory and non-negative cash.

use std::collections::BTreeMap;

use cw_core::calendar::Tick;
use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_core::money::Cents;
use cw_core::quantity::MilliLots;
use cw_data::{Dataset, ProductKind};
use cw_sim::commands::{CommandEnvelope, CommandIntent, apply};
use cw_sim::economy::{banking, freight, insurance, production, seed};
use cw_sim::state::{
    Contract, ContractStatus, Facility, FacilityStatus, HolderId, INDUSTRY_NAMES, ScenarioKind,
    ScenarioMode, World,
};
use cw_sim::tick;

// ---------------------------------------------------------------------------------------
// Fixture helpers
// ---------------------------------------------------------------------------------------

fn dataset() -> Dataset {
    Dataset::load().expect("pinned dataset validates")
}

fn seeded(seed_value: u64) -> World {
    let mut world = seed::build(
        &dataset(),
        seed_value,
        ScenarioKind::Founder,
        ScenarioMode::Sandbox,
    );
    // Industry scenarios test one rule in isolation. The seeded baseline contracts belong to
    // the corridor scenario and would otherwise deliver unrelated stock into these fixtures.
    let handles: Vec<_> = world.contracts.iter().map(|(handle, _)| handle).collect();
    for handle in handles {
        world.contracts.remove(handle);
    }
    for (_, firm) in world.firms.iter_mut() {
        firm.contracts_buy.clear();
        firm.contracts_sell.clear();
    }
    // Disable the gateway bid so a scenario observes the goods it produced rather than their
    // immediate export. The import offer stays available for power and inputs.
    for gateway in world.gateways.iter_mut() {
        gateway.buy_price_cents = 0;
    }
    world
}

fn firms_in(world: &World, industry: &str) -> Vec<Handle> {
    let index = INDUSTRY_NAMES
        .iter()
        .position(|name| *name == industry)
        .expect("known industry") as u8;
    world
        .firms
        .iter()
        .filter(|(_, firm)| firm.industry == index)
        .map(|(handle, _)| handle)
        .collect()
}

fn one_firm(world: &World, industry: &str) -> Handle {
    firms_in(world, industry)
        .into_iter()
        .next()
        .expect("seeded firm for industry")
}

fn facility_of(world: &World, firm: Handle, recipe_id: &str, dataset: &Dataset) -> Handle {
    world
        .firm(firm)
        .expect("firm")
        .facilities
        .iter()
        .copied()
        .find(|handle| {
            world
                .facilities
                .get(*handle)
                .map(|facility| dataset.recipes[facility.recipe as usize].id == recipe_id)
                .unwrap_or(false)
        })
        .expect("facility with recipe")
}

/// Employ every worker the recipe needs, so staffing is never the binding constraint.
fn staff(world: &mut World, dataset: &Dataset, firm: Handle) {
    let handles = world.firm(firm).expect("firm").facilities.clone();
    for handle in handles {
        let full = world
            .facilities
            .get(handle)
            .expect("facility")
            .full_workers(dataset);
        if let Some(facility) = world.facilities.get_mut(handle) {
            facility.workers = full;
        }
    }
}

/// Owner capital injection, posted as real paid-in equity so the ledger stays balanced.
fn fund(world: &mut World, firm: Handle, cents: i64) {
    let mut book = world.firm(firm).expect("firm").book.clone();
    book.commit(
        world.tick,
        Txn::new()
            .debit(Account::Cash, cents, "test capital")
            .credit(Account::ShareCapital, cents, "test capital"),
    )
    .expect("capital injection posts");
    world.firms.get_mut(firm).unwrap().book = book;
}

/// Set a firm's physical stock of one product in one region to an exact quantity.
fn set_stock(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    product: &str,
    region: u8,
    lots: i64,
) {
    let code = dataset.product_code(product).expect("known product");
    let current = world.firm(firm).expect("firm").stock(code, region);
    let target = MilliLots::from_lots(lots);
    if target.0 > current.0 {
        let extra = MilliLots(target.0 - current.0);
        let cost = extra
            .value_at(Cents(dataset.reference_price(code)))
            .expect("cost fits")
            .0;
        let entity = world.firms.get_mut(firm).unwrap();
        entity
            .book
            .commit(
                world.tick,
                Txn::new().credit(Account::Cash, cost, "test stock").debit(
                    Account::Inventory,
                    cost,
                    "test stock",
                ),
            )
            .expect("stock purchase posts");
        entity
            .stock_lot_mut(code, region)
            .receive(extra, Cents(cost));
    } else if target.0 < current.0 {
        let remove = MilliLots(current.0 - target.0);
        let entity = world.firms.get_mut(firm).unwrap();
        let mut carry = 0;
        let cost = entity
            .stock_lot_mut(code, region)
            .issue(remove, &mut carry)
            .0;
        if cost > 0 {
            entity
                .book
                .commit(
                    world.tick,
                    Txn::new()
                        .credit(Account::Inventory, cost, "test spoilage")
                        .debit(Account::OperatingExpense, cost, "test spoilage"),
                )
                .expect("spoilage posts");
        }
    }
}

/// Add an operating facility owned by an existing firm, for multi-plant chain scenarios.
fn add_facility(
    world: &mut World,
    dataset: &Dataset,
    firm: Handle,
    recipe_id: &str,
    blocks: u8,
) -> Handle {
    let recipe_index = dataset.recipe_index[recipe_id];
    let recipe = &dataset.recipes[recipe_index as usize];
    let region = world.firm(firm).expect("firm").region;
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
        target_bps: 8_500,
        quality_bps: 10_000,
        deposit_remaining: MilliLots::ZERO,
        last_output: MilliLots::ZERO,
        last_inputs_short: false,
        last_staff_ratio_bps: 10_000,
        mothballed_since: None,
    };
    let handle = world.facilities.insert(facility);
    world
        .firms
        .get_mut(firm)
        .expect("firm")
        .facilities
        .push(handle);
    handle
}

fn facility_output(world: &World, handle: Handle) -> MilliLots {
    world.facilities.get(handle).expect("facility").last_output
}

/// Revenue recognised so far: closed weeks plus the open week's postings.
fn lifetime_revenue(world: &World, firm: Handle) -> i64 {
    let book = &world.firm(firm).expect("firm").book;
    book.lifetime().revenue - book.balance(Account::Revenue)
}

/// Every ledger balances, assets equal liabilities plus equity, cash and stock are finite.
fn assert_invariants(world: &World) {
    for (_, firm) in world.firms.iter() {
        assert!(firm.book.is_balanced(), "{} ledger balanced", firm.name);
        // The balance sheet identity holds once the week's profit-and-loss accounts are
        // closed into retained earnings; an unclosed week offsets it exactly.
        let profit_and_loss: i64 = Account::ALL
            .iter()
            .filter(|account| account.is_profit_and_loss())
            .map(|account| firm.book.balance(*account))
            .sum();
        assert_eq!(
            firm.book.total_assets() - firm.book.total_liabilities() - firm.book.total_equity(),
            -profit_and_loss,
            "{} balance sheet identity",
            firm.name
        );
        assert!(firm.book.cash() >= 0, "{} cash non-negative", firm.name);
        let carrying: i64 = firm.inventory.values().map(|lot| lot.cost_cents).sum();
        assert_eq!(
            firm.book.balance(Account::Inventory),
            carrying,
            "{} inventory asset equals the lots' carrying cost",
            firm.name
        );
        for ((_, _), lot) in &firm.inventory {
            assert!(lot.qty.0 >= 0, "{} inventory non-negative", firm.name);
            assert!(
                lot.cost_cents >= 0,
                "{} carrying cost non-negative",
                firm.name
            );
        }
    }
    assert!(world.investor.book.is_balanced(), "investor ledger");
    assert!(world.government.book.is_balanced(), "government ledger");
    assert!(world.outside.gateway_book.is_balanced(), "gateway ledger");
    assert!(world.outside.advocacy_book.is_balanced(), "advocacy ledger");
    assert!(
        world.outside.legal_services_book.is_balanced(),
        "legal services ledger"
    );
    for region in &world.regions {
        assert!(
            region.households.savings_cents >= 0,
            "household savings never negative"
        );
    }
}

// ---------------------------------------------------------------------------------------
// Agriculture: seasonal crop capacity
// ---------------------------------------------------------------------------------------

#[test]
fn agriculture_output_follows_the_documented_seasonal_index() {
    let dataset = dataset();
    // Produce inside weeks 13, 26, 39 and 52, one world per quarter so only the quarter
    // differs between runs.
    let mut quarterly: Vec<MilliLots> = Vec::new();
    for week in [12u32, 25, 38, 51] {
        let mut world = seeded(101);
        let farm = one_firm(&world, "agriculture");
        staff(&mut world, &dataset, farm);
        let facility = facility_of(&world, farm, "farm", &dataset);
        world.tick = Tick(week);
        tick::advance(&mut world, &dataset);
        assert_eq!(world.tick, Tick(week + 1));
        quarterly.push(facility_output(&world, facility));
        assert_invariants(&world);
    }

    // CONTENT-PACING: farm yield index by quarter is [8000, 11000, 13000, 8000] bps.
    let base = quarterly[0].0;
    assert!(base > 0, "farm produced in quarter one");
    for (index, documented) in [8_000i64, 11_000, 13_000, 8_000].iter().enumerate() {
        let observed = quarterly[index].0;
        let left = observed as i128 * 8_000;
        let right = base as i128 * (*documented as i128);
        let tolerance = right.abs() / 200; // 50 bps, for milli-lot rounding
        assert!(
            (left - right).abs() <= tolerance,
            "quarter {} output {} should follow index {} against base {}",
            index + 1,
            observed,
            documented,
            base
        );
    }
    assert!(quarterly[2] > quarterly[1] && quarterly[1] > quarterly[0]);
    assert_eq!(quarterly[3], quarterly[0], "quarters one and four match");
}

// ---------------------------------------------------------------------------------------
// Extraction: finite deposit depletion
// ---------------------------------------------------------------------------------------

#[test]
fn extraction_depletes_a_finite_deposit_and_never_goes_negative() {
    let dataset = dataset();
    let mut world = seeded(102);
    let miner = one_firm(&world, "extraction");
    staff(&mut world, &dataset, miner);
    let facility = facility_of(&world, miner, "mine", &dataset);
    let ore = dataset.product_code("ore").unwrap();
    let region = world.firm(miner).unwrap().region;
    set_stock(&mut world, &dataset, miner, "fuel", region, 200);
    let ore_before = world.firm(miner).unwrap().stock(ore, region);

    // A deposit of 100 lots is smaller than one week's planned output (400 lots per block).
    world
        .facilities
        .get_mut(facility)
        .unwrap()
        .deposit_remaining = MilliLots::from_lots(100);

    production::produce(&mut world, &dataset);
    let first = facility_output(&world, facility);
    let deposit = world.facilities.get(facility).unwrap().deposit_remaining;
    assert_eq!(
        first,
        MilliLots::from_lots(100),
        "output is deposit-limited"
    );
    assert_eq!(deposit, MilliLots::ZERO, "deposit fully extracted");
    assert_eq!(
        world.firm(miner).unwrap().stock(ore, region).0 - ore_before.0,
        first.0,
        "ore stock rose by exactly the extracted quantity"
    );

    production::produce(&mut world, &dataset);
    assert_eq!(
        facility_output(&world, facility),
        MilliLots::ZERO,
        "an exhausted mine produces nothing"
    );
    assert_eq!(
        world.facilities.get(facility).unwrap().deposit_remaining,
        MilliLots::ZERO
    );
    assert_eq!(
        world.firm(miner).unwrap().stock(ore, region).0 - ore_before.0,
        first.0,
        "no ore appears from an exhausted deposit"
    );

    // A one milli-lot remainder cannot be turned into negative stock or negative deposit.
    world
        .facilities
        .get_mut(facility)
        .unwrap()
        .deposit_remaining = MilliLots(1);
    production::produce(&mut world, &dataset);
    assert_eq!(facility_output(&world, facility), MilliLots::ZERO);
    assert!(world.facilities.get(facility).unwrap().deposit_remaining.0 >= 0);
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Energy: dispatch and allocated electricity service
// ---------------------------------------------------------------------------------------

#[test]
fn energy_generators_are_dispatched_and_power_is_a_weekly_allocated_service() {
    let dataset = dataset();
    let mut world = seeded(103);
    let electricity = dataset.product_code("electricity").unwrap();

    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }

    production::produce(&mut world, &dataset);

    // Generators run from opening fuel stock without needing their own output, and are paid
    // for the electricity they place into their region.
    let mut generator_output = 0i64;
    for firm in firms_in(&world, "energy") {
        let facility = facility_of(&world, firm, "generator", &dataset);
        let output = facility_output(&world, facility);
        assert!(
            output.is_positive(),
            "a fully staffed generator produces electricity"
        );
        let capacity = MilliLots::from_lots(
            dataset.recipes[world.facilities.get(facility).unwrap().recipe as usize]
                .weekly_output_lots,
        )
        .scale_bps(8_500);
        assert!(output <= capacity, "output within planned capacity");
        generator_output += output.0;
        assert!(
            lifetime_revenue(&world, firm) > 0,
            "a generator earns power sales revenue"
        );
    }
    assert!(generator_output > 0);

    // Electricity is a weekly service. Any surplus waits only for the finite gateway export
    // and is then expired: it is never carried into a later week as usable stock.
    for (_, firm) in world.firms.iter() {
        assert!(
            firm.stock(electricity, firm.region).0 >= 0,
            "{} never holds negative electricity stock",
            firm.name
        );
    }
    production::expire_services(&mut world, &dataset);
    for (_, firm) in world.firms.iter() {
        assert_eq!(
            firm.stock(electricity, firm.region),
            MilliLots::ZERO,
            "{} holds no electricity after the weekly expiry",
            firm.name
        );
    }

    // Consumers that need electricity do produce while supply exists.
    let farm = one_firm(&world, "agriculture");
    let farm_facility = facility_of(&world, farm, "farm", &dataset);
    assert!(facility_output(&world, farm_facility).is_positive());

    // Withdraw every generator and the electricity gateway: dispatch leaves consumers
    // unserved and planned output falls to zero before other inputs are consumed.
    let mut world = seeded(104);
    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }
    let generators: Vec<Handle> = world
        .facilities
        .iter()
        .filter(|(_, facility)| dataset.recipes[facility.recipe as usize].output == "electricity")
        .map(|(handle, _)| handle)
        .collect();
    for handle in generators {
        world.facilities.remove(handle);
    }
    world
        .gateways
        .retain(|gateway| gateway.product != electricity);
    production::produce(&mut world, &dataset);
    for (_, facility) in world.facilities.iter() {
        let recipe = &dataset.recipes[facility.recipe as usize];
        if recipe.weekly_inputs_lots.contains_key("electricity") {
            assert_eq!(
                facility.last_output,
                MilliLots::ZERO,
                "unserved electricity means no output from {}",
                recipe.id
            );
        }
    }
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Chemical industries: stoichiometric multi-input conversion
// ---------------------------------------------------------------------------------------

#[test]
fn chemical_industries_consumes_each_input_in_recipe_proportion() {
    let dataset = dataset();
    let mut world = seeded(105);
    let plant = one_firm(&world, "chemical_industries");
    // The plant converts fuel + ore + purchased electricity into chemicals.
    let facility = facility_of(&world, plant, "chemical_plant", &dataset);
    let blocks = world.facilities.get(facility).unwrap().blocks as i64;
    assert!((1..=4).contains(&blocks), "seeded plant has real capacity");
    staff(&mut world, &dataset, plant);
    let region = world.firm(plant).unwrap().region;

    // Make ore the binding input: half of the recipe's ten lots per block.
    let ore_lots_per_block = 10;
    set_stock(
        &mut world,
        &dataset,
        plant,
        "ore",
        region,
        ore_lots_per_block * blocks / 2,
    );
    set_stock(&mut world, &dataset, plant, "fuel", region, 200);

    let chemicals = dataset.product_code("chemicals").unwrap();
    let ore = dataset.product_code("ore").unwrap();
    let fuel = dataset.product_code("fuel").unwrap();
    let chemicals_before = world.firm(plant).unwrap().stock(chemicals, region).0;
    let ore_before = world.firm(plant).unwrap().stock(ore, region).0;
    let fuel_before = world.firm(plant).unwrap().stock(fuel, region).0;

    production::produce(&mut world, &dataset);

    // Half of full capacity is attainable, so every input is consumed at 50 percent and
    // output is exactly half the recipe quantity. There is no free conversion.
    let expected_ratio_bps = 5_000i64;
    let expected_output = 200 * blocks * expected_ratio_bps / 10_000;
    let expected_ore = ore_lots_per_block * blocks * expected_ratio_bps / 10_000;
    let expected_fuel = 100 * blocks * expected_ratio_bps / 10_000;
    assert_eq!(
        facility_output(&world, facility),
        MilliLots::from_lots(expected_output)
    );
    assert_eq!(
        chemicals_before + MilliLots::from_lots(expected_output).0,
        world.firm(plant).unwrap().stock(chemicals, region).0
    );
    assert_eq!(
        ore_before - MilliLots::from_lots(expected_ore).0,
        world.firm(plant).unwrap().stock(ore, region).0,
        "ore consumed in recipe proportion"
    );
    assert_eq!(
        fuel_before - MilliLots::from_lots(expected_fuel).0,
        world.firm(plant).unwrap().stock(fuel, region).0,
        "fuel consumed in recipe proportion"
    );
    assert_eq!(
        world
            .firm(plant)
            .unwrap()
            .stock(dataset.product_code("electricity").unwrap(), region),
        MilliLots::ZERO,
        "electricity is a service, never stockpiled"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Manufacturing: ore to steel to components to finished goods
// ---------------------------------------------------------------------------------------

#[test]
fn manufacturing_chain_converts_each_stage_from_opening_stock_only() {
    let dataset = dataset();
    let mut world = seeded(106);
    let firm = one_firm(&world, "manufacturing");
    // Replace the seeded plant with an explicit one-block chain, created in process order so
    // an order-dependent engine would wrongly let later stages run on this week's output.
    let seeded: Vec<Handle> = world.firm(firm).unwrap().facilities.clone();
    for handle in seeded {
        world.facilities.remove(handle);
    }
    world.firms.get_mut(firm).unwrap().facilities.clear();
    let metal = add_facility(&mut world, &dataset, firm, "metal_works", 1);
    let components = add_facility(&mut world, &dataset, firm, "component_works", 1);
    let assembly = add_facility(&mut world, &dataset, firm, "assembly", 1);
    let region = world.firm(firm).unwrap().region;
    set_stock(&mut world, &dataset, firm, "ore", region, 1_000);
    set_stock(&mut world, &dataset, firm, "fuel", region, 200);
    set_stock(&mut world, &dataset, firm, "chemicals", region, 300);

    let steel = dataset.product_code("steel").unwrap();
    let component_code = dataset.product_code("components").unwrap();
    let goods = dataset.product_code("goods").unwrap();

    // Week one: only the first stage has physical input at the opening bell.
    production::produce(&mut world, &dataset);
    assert!(facility_output(&world, metal).is_positive(), "steel made");
    assert_eq!(
        facility_output(&world, components),
        MilliLots::ZERO,
        "a component plant cannot use steel produced in the same week"
    );
    assert_eq!(
        facility_output(&world, assembly),
        MilliLots::ZERO,
        "an assembly plant cannot use components produced in the same week"
    );
    let steel_after_week_one = world.firm(firm).unwrap().stock(steel, region);
    assert!(steel_after_week_one.is_positive());

    // Week two: opening steel feeds components, assembly is still starved.
    production::produce(&mut world, &dataset);
    assert!(facility_output(&world, components).is_positive());
    assert_eq!(
        world.firm(firm).unwrap().stock(component_code, region),
        MilliLots::from_lots(facility_output(&world, components).lots()),
        "components produced are carried into next week"
    );
    assert_eq!(
        facility_output(&world, assembly),
        MilliLots::ZERO,
        "assembly still cannot see this week's components"
    );
    let components_after_week_two = world.firm(firm).unwrap().stock(component_code, region);
    assert!(components_after_week_two.is_positive());

    // Week three: the finished-goods stage finally runs on opening components.
    production::produce(&mut world, &dataset);
    let components_made = facility_output(&world, components);
    let goods_made = facility_output(&world, assembly);
    assert!(goods_made.is_positive(), "goods made");
    assert!(world.firm(firm).unwrap().stock(goods, region).is_positive());
    assert_eq!(
        components_after_week_two.0 + components_made.0 - goods_made.0,
        world.firm(firm).unwrap().stock(component_code, region).0,
        "assembly consumed exactly its components, one lot of components per lot of goods"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Construction: escrowed capex and phased completion
// ---------------------------------------------------------------------------------------

#[test]
fn construction_capex_is_escrowed_and_released_on_completion() {
    let dataset = dataset();
    let mut world = seeded(107);
    let builder = one_firm(&world, "construction");
    staff(&mut world, &dataset, builder);
    let contractor = facility_of(&world, builder, "contractor", &dataset);
    let region = world.firm(builder).unwrap().region;

    // Project-service output has no buyer, so it cannot be carried past the week.
    tick::advance(&mut world, &dataset);
    assert!(facility_output(&world, contractor).is_positive());
    assert_eq!(
        world
            .firm(builder)
            .unwrap()
            .stock(dataset.product_code("construction").unwrap(), region),
        MilliLots::ZERO,
        "unsold construction work expires instead of being stockpiled"
    );

    // A new project is paid from the builder's own cash into construction escrow.
    let farm_recipe = dataset.recipe_index["farm"];
    let capex = dataset.recipes[farm_recipe as usize].capex_cents;
    let build_weeks = dataset.recipes[farm_recipe as usize].build_weeks;
    fund(&mut world, builder, capex);
    let cash_before = world.firm(builder).unwrap().book.cash();
    let wip_before = world
        .firm(builder)
        .unwrap()
        .book
        .balance(Account::WorkInProgress);
    let assets_before = world
        .firm(builder)
        .unwrap()
        .book
        .balance(Account::FixedAssets);

    let revision = world.revision;
    let submitted = world.tick.0;
    let receipt = apply(
        &mut world,
        &dataset,
        &CommandEnvelope {
            command_id_high: 1,
            command_id_low: 1,
            actor: builder.0,
            expected_revision: revision,
            submitted_tick: submitted,
            intent: CommandIntent::BuildFacility {
                firm: builder,
                region,
                recipe: farm_recipe,
                blocks: 1,
            },
        },
    )
    .expect("construction command accepted");
    assert_eq!(receipt.accepted_tick, world.tick.0);

    let project = world
        .firm(builder)
        .unwrap()
        .facilities
        .iter()
        .copied()
        .find(|handle| world.facilities.get(*handle).unwrap().status == FacilityStatus::Building)
        .expect("project under construction");
    assert_eq!(
        world.firm(builder).unwrap().book.cash(),
        cash_before - capex,
        "capex left cash exactly once"
    );
    assert_eq!(
        world
            .firm(builder)
            .unwrap()
            .book
            .balance(Account::WorkInProgress),
        wip_before + capex * 40 / 100,
        "the construction service share is carried as work in progress"
    );
    assert_eq!(
        world.facilities.get(project).unwrap().escrow_cents,
        capex - capex * 40 / 100,
        "escrow holds the equipment share until delivery"
    );
    assert_eq!(
        world.facilities.get(project).unwrap().completes,
        Tick(world.tick.0 + build_weeks)
    );
    assert_eq!(
        world
            .firm(builder)
            .unwrap()
            .book
            .balance(Account::FixedAssets),
        assets_before,
        "an unfinished project is not yet a fixed asset"
    );

    for week in 1..build_weeks {
        tick::advance(&mut world, &dataset);
        assert_eq!(
            world.facilities.get(project).unwrap().status,
            FacilityStatus::Building,
            "still building at week {week}"
        );
    }
    tick::advance(&mut world, &dataset);
    let facility = world.facilities.get(project).expect("completed project");
    assert_eq!(facility.status, FacilityStatus::Operating);
    assert_eq!(facility.escrow_cents, 0, "escrow released");
    assert_eq!(
        world
            .firm(builder)
            .unwrap()
            .book
            .balance(Account::WorkInProgress),
        wip_before,
        "work in progress cleared"
    );
    assert_eq!(
        world
            .firm(builder)
            .unwrap()
            .book
            .balance(Account::FixedAssets),
        assets_before + capex,
        "completed work becomes a fixed asset exactly once"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Logistics: freight revenue and shipment lead time
// ---------------------------------------------------------------------------------------

#[test]
fn logistics_carriers_earn_freight_and_delivery_waits_for_the_lane_lead_time() {
    let dataset = dataset();
    let mut world = seeded(108);
    // Produce one week so a seller holds real output stock in its own region.
    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }
    tick::advance(&mut world, &dataset);

    let seller = one_firm(&world, "agriculture");
    let buyer = one_firm(&world, "financial");
    let product = dataset.product_code("food_raw").unwrap();
    let from_region = world.firm(seller).unwrap().region;
    let to_region = world.firm(buyer).unwrap().region;
    assert_ne!(from_region, to_region, "an inter-region lane is required");
    let quantity = MilliLots::from_lots(100);
    assert!(world.firm(seller).unwrap().stock(product, from_region) >= quantity);

    let price = dataset.reference_price(product);
    let contract = world.contracts.insert(Contract {
        buyer,
        seller,
        product,
        from_region,
        to_region,
        quantity_weekly: quantity,
        price_cents: price,
        penalty_bps: 500,
        escrow_cents: 0,
        remaining_weeks: 1,
        next_delivery: world.tick,
        status: ContractStatus::Active,
        shortfalls: MilliLots::ZERO,
        auto_renew: false,
    });

    let km = dataset
        .lane_km(
            &dataset.regions[from_region as usize].code,
            &dataset.regions[to_region as usize].code,
        )
        .expect("validated lane");
    let lead = freight::lead_time_weeks(km);
    let expected_freight = freight::freight_cost(&dataset, from_region, to_region, quantity);
    let carrier_revenue_before: i64 = firms_in(&world, "logistics")
        .iter()
        .map(|firm| lifetime_revenue(&world, *firm))
        .sum();
    let gateway_revenue_before = -world.outside.gateway_book.balance(Account::Revenue);
    let seller_stock_before = world.firm(seller).unwrap().stock(product, from_region);
    let buyer_stock_before = world.firm(buyer).unwrap().stock(product, to_region);
    let seller_cash_before = world.firm(seller).unwrap().book.cash();
    let buyer_cash_before = world.firm(buyer).unwrap().book.cash();

    freight::settle(&mut world, &dataset);

    let carrier_revenue_after: i64 = firms_in(&world, "logistics")
        .iter()
        .map(|firm| lifetime_revenue(&world, *firm))
        .sum();
    let carriers_paid = carrier_revenue_after - carrier_revenue_before;
    // Freight is charged once: named carriers take what they can, the finite outside carrier
    // sector takes the exact remainder.
    let gateway_revenue =
        -world.outside.gateway_book.balance(Account::Revenue) - gateway_revenue_before;
    assert!(carriers_paid > 0, "named carriers earn freight");
    assert!(
        carriers_paid <= expected_freight,
        "carriers cannot be paid more than the freight bill"
    );
    assert_eq!(
        carriers_paid + gateway_revenue,
        expected_freight,
        "the whole freight bill is settled exactly once"
    );

    // Freight is charged to the buyer once; the seller receives only the goods value.
    let goods_value = quantity.value_at(Cents(price)).expect("goods value fits").0;
    assert_eq!(
        buyer_cash_before - world.firm(buyer).unwrap().book.cash(),
        goods_value + expected_freight,
        "the buyer pays goods plus one freight charge"
    );
    assert_eq!(
        world.firm(seller).unwrap().book.cash() - seller_cash_before,
        goods_value,
        "the seller is paid the goods value, not the freight"
    );

    assert_eq!(
        world.firm(seller).unwrap().stock(product, from_region).0,
        seller_stock_before.0 - quantity.0,
        "seller stock left the origin region"
    );
    assert_eq!(
        world.firm(buyer).unwrap().stock(product, to_region),
        buyer_stock_before,
        "stock never teleports: nothing arrives in the delivery week"
    );
    let shipment = world
        .shipments
        .iter()
        .find(|s| s.product == product)
        .expect("a shipment is in transit");
    assert_eq!(shipment.arrive, Tick(world.tick.0 + lead));
    assert_eq!(shipment.quantity, quantity);
    assert_eq!(
        world.contracts.get(contract).unwrap().status,
        ContractStatus::Completed
    );

    for _ in 1..lead {
        tick::advance(&mut world, &dataset);
        assert_eq!(
            world.firm(buyer).unwrap().stock(product, to_region),
            buyer_stock_before,
            "still in transit"
        );
    }
    tick::advance(&mut world, &dataset);
    assert_eq!(
        world.firm(buyer).unwrap().stock(product, to_region).0,
        buyer_stock_before.0 + quantity.0,
        "delivered after the lane lead time"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Logistics: spot import settlement
// ---------------------------------------------------------------------------------------

#[test]
fn spot_imports_charge_goods_and_freight_exactly_once() {
    let dataset = dataset();
    let mut world = seeded(120);
    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }
    tick::advance(&mut world, &dataset);

    // Steel is produced only in the metal-works region, so the seller is unambiguous.
    let product = dataset.product_code("steel").unwrap();
    let price = dataset.reference_price(product);
    let buyer = one_firm(&world, "telecommunications");
    let buyer_region = world.firm(buyer).unwrap().region;
    let seller = world
        .firms
        .iter()
        .find(|(_, firm)| {
            firm.operative()
                && firm.region != buyer_region
                && firm.stock(product, firm.region).is_positive()
        })
        .map(|(handle, _)| handle)
        .expect("one steel supplier in another region");
    let seller_region = world.firm(seller).unwrap().region;
    assert_ne!(seller_region, buyer_region);

    let quantity = MilliLots::from_lots(10);
    let goods_value = quantity.value_at(Cents(price)).expect("value fits").0;
    let freight = freight::freight_cost(&dataset, seller_region, buyer_region, quantity);
    let lead = freight::lead_time_weeks(
        dataset
            .lane_km(
                &dataset.regions[seller_region as usize].code,
                &dataset.regions[buyer_region as usize].code,
            )
            .expect("validated lane"),
    );
    let buyer_cash_before = world.firm(buyer).unwrap().book.cash();
    let buyer_inventory_before = world.firm(buyer).unwrap().book.balance(Account::Inventory);
    let seller_cash_before = world.firm(seller).unwrap().book.cash();
    let seller_stock_before = world.firm(seller).unwrap().stock(product, seller_region);
    let carriers_before: i64 = firms_in(&world, "logistics")
        .iter()
        .map(|firm| lifetime_revenue(&world, *firm))
        .sum();
    let gateway_before = -world.outside.gateway_book.balance(Account::Revenue);

    let revision = world.revision;
    let submitted = world.tick.0;
    apply(
        &mut world,
        &dataset,
        &CommandEnvelope {
            command_id_high: 2,
            command_id_low: 1,
            actor: buyer.0,
            expected_revision: revision,
            submitted_tick: submitted,
            intent: CommandIntent::PlaceSpotOrder {
                firm: buyer,
                product,
                region: buyer_region,
                quantity_lots: 10,
                max_price_cents: price,
            },
        },
    )
    .expect("spot import accepted");

    assert_eq!(
        buyer_cash_before - world.firm(buyer).unwrap().book.cash(),
        goods_value + freight,
        "the buyer pays goods plus one freight charge"
    );
    assert_eq!(
        world.firm(buyer).unwrap().book.balance(Account::Inventory) - buyer_inventory_before,
        goods_value,
        "the buyer books the goods it paid for"
    );
    assert_eq!(
        world.firm(seller).unwrap().book.cash() - seller_cash_before,
        goods_value,
        "the seller is paid the goods value only"
    );
    assert_eq!(
        seller_stock_before.0 - world.firm(seller).unwrap().stock(product, seller_region).0,
        quantity.0
    );
    let carriers_after: i64 = firms_in(&world, "logistics")
        .iter()
        .map(|firm| lifetime_revenue(&world, *firm))
        .sum();
    let gateway_after = -world.outside.gateway_book.balance(Account::Revenue);
    assert_eq!(
        (carriers_after - carriers_before) + (gateway_after - gateway_before),
        freight,
        "carrier revenue equals the freight bill exactly"
    );
    assert_eq!(
        world
            .firm(buyer)
            .unwrap()
            .book
            .balance(Account::RestrictedCash),
        0
    );

    let shipment = world
        .shipments
        .iter()
        .find(|shipment| shipment.buyer == buyer && shipment.product == product)
        .expect("import in transit");
    assert_eq!(shipment.arrive, Tick(world.tick.0 + lead));
    assert_eq!(
        world.firm(buyer).unwrap().stock(product, buyer_region),
        MilliLots::ZERO
    );
    for _ in 0..lead {
        tick::advance(&mut world, &dataset);
    }
    assert_eq!(
        world.firm(buyer).unwrap().stock(product, buyer_region),
        quantity,
        "the import arrives after the lane lead time"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Retail: final demand through households
// ---------------------------------------------------------------------------------------

#[test]
fn retail_sells_consumer_goods_to_households_and_producers_do_not() {
    let dataset = dataset();
    let mut world = seeded(109);
    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }
    let grocer = one_firm(&world, "retail");
    let region = world.firm(grocer).unwrap().region;
    let delivered = dataset.product_code("food_retail").unwrap();
    let raw = dataset.product_code("food_raw").unwrap();
    assert_eq!(
        dataset.products[delivered as usize].kind,
        ProductKind::ConsumerGoods
    );

    tick::advance(&mut world, &dataset);

    let household = &world.regions[region as usize].households;
    let spend_food = household.spend_cents.get(&delivered).copied().unwrap_or(0);
    assert!(spend_food > 0, "households bought delivered food");
    assert!(
        !household.weights.contains_key(&raw),
        "household demand is for delivered retail output, not farm output"
    );
    assert!(
        !household.spend_cents.contains_key(&raw),
        "households never buy farm output directly"
    );

    // The grocery chain is the only path from farm output to household food demand.
    let grocer_firm = world.firm(grocer).unwrap();
    assert!(grocer_firm.book.lifetime().revenue > 0);
    assert!(
        grocer_firm.trailing_units(region, delivered) > 0,
        "retail sales are tracked as unit sales"
    );

    // A producer with raw food stock earns nothing from household demand directly.
    let farm = one_firm(&world, "agriculture");
    let farm_region = world.firm(farm).unwrap().region;
    assert!(
        world
            .firm(farm)
            .unwrap()
            .stock(raw, farm_region)
            .is_positive()
    );

    // Household budgets are finite: spending never exceeds the disposable budget.
    let income = world.regions[region as usize]
        .households
        .last_income_cents
        .max(0);
    let spent: i64 = household.spend_cents.values().sum();
    assert!(
        spent <= income * 90 / 100,
        "household spend {spent} within the 90 percent disposable budget of {income}"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Healthcare: care paid by households and the government budget
// ---------------------------------------------------------------------------------------

#[test]
fn healthcare_care_is_bought_by_households_and_the_healthcare_budget() {
    let dataset = dataset();
    let mut world = seeded(110);
    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }
    let clinic = one_firm(&world, "healthcare");
    let region = world.firm(clinic).unwrap().region;
    let care = dataset.product_code("care").unwrap();
    assert_eq!(
        dataset.products[care as usize].kind,
        ProductKind::LocalService
    );

    // Isolate the healthcare payer: no defence procurement and no transfer support, so every
    // government operating expense this week is a care purchase.
    world.government.procurement_budget_cents = 0;
    world.government.training_budget_cents = 0;
    let healthcare_budget = world.government.healthcare_budget_cents;
    let government_cash_before = world.government.book.cash();
    let government_expense_before = world.government.book.balance(Account::OperatingExpense);

    tick::advance(&mut world, &dataset);

    let care_spend = world.regions[region as usize]
        .households
        .spend_cents
        .get(&care)
        .copied()
        .unwrap_or(0);
    assert!(care_spend > 0, "households pay part of the care bill");
    let government_expense =
        world.government.book.balance(Account::OperatingExpense) - government_expense_before;
    assert!(government_expense > 0, "the government payer bought care");
    assert!(
        government_expense <= healthcare_budget,
        "government care spending stays inside the healthcare budget"
    );
    assert_eq!(
        government_cash_before - world.government.book.cash(),
        government_expense,
        "government care spending has a cash counterparty"
    );
    assert!(
        lifetime_revenue(&world, clinic) > 0,
        "the clinic is paid by both payer groups"
    );
    assert_eq!(
        world.firm(clinic).unwrap().stock(care, region),
        MilliLots::ZERO,
        "care is a service: unsold capacity expires"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Financial: banking
// ---------------------------------------------------------------------------------------

#[test]
fn financial_bank_deposits_are_liabilities_and_lending_obeys_limits() {
    let mut world = seeded(111);
    let bank = firms_in(&world, "financial")
        .into_iter()
        .find(|firm| world.firm(*firm).unwrap().bank.is_some())
        .expect("one seeded funded bank");
    let insurer = firms_in(&world, "financial")
        .into_iter()
        .find(|firm| world.firm(*firm).unwrap().insurer.is_some())
        .expect("one seeded insurer");
    assert_ne!(bank, insurer);
    // A cash-rich non-bank firm provides the outside deposit.
    let depositor = one_firm(&world, "retail");
    assert!(world.firm(depositor).unwrap().book.cash() >= 200_000_000);
    let borrower = one_firm(&world, "manufacturing");

    // Lending moves bank cash to the borrower and creates a matching asset and debt.
    let bank_cash_before = world.firm(bank).unwrap().book.cash();
    let borrower_cash_before = world.firm(borrower).unwrap().book.cash();
    let loans_before = world.firm(bank).unwrap().book.balance(Account::Receivables);
    let borrower_loans_before = world.firm(borrower).unwrap().book.balance(Account::Loans);
    banking::approve_loan(&mut world, bank, borrower, 10_000_000).expect("funded loan");
    assert_eq!(
        world.firm(bank).unwrap().book.cash(),
        bank_cash_before - 10_000_000
    );
    assert_eq!(
        world.firm(borrower).unwrap().book.cash(),
        borrower_cash_before + 10_000_000
    );
    assert_eq!(
        world.firm(bank).unwrap().book.balance(Account::Receivables),
        loans_before + 10_000_000,
        "loan principal is a bank asset"
    );
    assert_eq!(
        world.firm(borrower).unwrap().book.balance(Account::Loans),
        borrower_loans_before - 10_000_000,
        "the same principal is a new borrower liability"
    );

    // A deposit is a bank liability and a depositor claim; outside cash is not duplicated.
    let total_cash_before: i64 = world.firms.iter().map(|(_, firm)| firm.book.cash()).sum();
    banking::deposit(&mut world, bank, HolderId::Firm(depositor), 200_000_000)
        .expect("deposit accepted");
    let total_cash_after: i64 = world.firms.iter().map(|(_, firm)| firm.book.cash()).sum();
    assert_eq!(total_cash_before, total_cash_after, "cash exists once");
    assert_eq!(banking::deposits(&world, bank), 200_000_000);
    assert_eq!(
        world.firm(bank).unwrap().book.balance(Account::Deposits),
        -200_000_000,
        "deposits are liabilities, not assets"
    );
    assert_eq!(
        world
            .firm(depositor)
            .unwrap()
            .book
            .balance(Account::Receivables),
        200_000_000,
        "the depositor holds a claim, not cash"
    );
    assert_invariants(&world);

    // Second bank: deposits fund the book, then the owner is paid out. With no capital the
    // bank cannot support risk-weighted lending even though it still holds customer cash.
    let mut world = seeded(119);
    let bank = firms_in(&world, "financial")
        .into_iter()
        .find(|firm| world.firm(*firm).unwrap().bank.is_some())
        .expect("one seeded funded bank");
    let depositor = one_firm(&world, "retail");
    let borrower = one_firm(&world, "manufacturing");
    banking::deposit(&mut world, bank, HolderId::Firm(depositor), 200_000_000)
        .expect("deposit accepted");
    let mut book = world.firm(bank).unwrap().book.clone();
    book.commit(
        world.tick,
        Txn::new()
            .debit(Account::RetainedEarnings, 400_000_000, "capital return")
            .credit(Account::Cash, 400_000_000, "capital return"),
    )
    .expect("capital return posts");
    world.firms.get_mut(bank).unwrap().book = book;
    assert_eq!(world.firm(bank).unwrap().book.total_equity(), 0);
    assert!(world.firm(bank).unwrap().book.cash() >= 50_000_000);

    let capital_error = banking::approve_loan(&mut world, bank, borrower, 5_000_000)
        .expect_err("no capital, no loan");
    assert!(matches!(
        capital_error,
        cw_core::error::DomainError::SolvencyRestriction {
            reason: cw_core::error::SolvencyReason::CapitalTooLow,
            ..
        }
    ));
    // The same bank also cannot lend below the 20 percent liquidity floor on deposits.
    let liquidity_error = banking::approve_loan(&mut world, bank, borrower, 45_000_000)
        .expect_err("illiquid, no loan");
    assert!(matches!(
        liquidity_error,
        cw_core::error::DomainError::SolvencyRestriction {
            reason: cw_core::error::SolvencyReason::LiquidityTooLow,
            ..
        }
    ));

    // A withdrawal is limited by the cash the bank actually holds; unpaid claims remain.
    let cash_available = world.firm(bank).unwrap().book.cash();
    let paid = banking::withdraw(&mut world, bank, HolderId::Firm(depositor), 200_000_000)
        .expect("partial withdrawal");
    assert_eq!(paid, cash_available, "paid from real cash only");
    let still_owed = banking::deposits(&world, bank);
    assert_eq!(still_owed, 200_000_000 - paid);
    assert!(still_owed > 0, "unpaid deposit claims remain");

    // A run requests 25 percent of deposits and fails honestly when cash is gone.
    let paid = banking::run(&mut world, bank);
    assert_eq!(paid, 0);
    assert!(matches!(
        world.firm(bank).unwrap().status,
        cw_sim::state::FirmStatus::PastDue { .. }
    ));
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Financial: insurance
// ---------------------------------------------------------------------------------------

#[test]
fn financial_insurer_reserves_claims_and_pays_from_real_cash() {
    let dataset = dataset();
    let mut world = seeded(112);
    let insurer = firms_in(&world, "financial")
        .into_iter()
        .find(|firm| world.firm(*firm).unwrap().insurer.is_some())
        .expect("seeded insurer");
    let insured = one_firm(&world, "manufacturing");
    let facility = facility_of(&world, insured, "metal_works", &dataset);
    let region = world.facilities.get(facility).unwrap().region;

    // Specialise the underwriter so a single-region book is legal while this fixture tests
    // premiums, reserves and claims rather than concentration limits.
    world
        .firm_mut(insurer)
        .unwrap()
        .insurer
        .as_mut()
        .unwrap()
        .specialist_concentration = true;

    let limit = 100_000_000;
    let deductible = 1_000_000;
    let premium = insurance::weekly_premium(limit);
    // Documented quote: 200 bps expected loss times 1.25, paid weekly.
    let annual = limit * 200 / 10_000 * 125 / 100;
    assert!(
        (premium * 52 - annual).abs() <= 52,
        "weekly premium {premium} annualises to the quoted premium {annual}"
    );

    let policy = insurance::bind(&mut world, insurer, insured, facility, limit, deductible)
        .expect("policy bound");
    let insured_cash_before = world.firm(insured).unwrap().book.cash();
    let insurer_cash_before = world.firm(insurer).unwrap().book.cash();

    insurance::weekly(&mut world);
    // A small deterministic idiosyncratic hazard can fire in the same week; account for any
    // claim from the damage recorded on the asset.
    let first_week_loss = world
        .facilities
        .get(facility)
        .unwrap()
        .accumulated_depreciation;
    let first_week_claim = if first_week_loss > 0 {
        (first_week_loss - deductible).clamp(0, limit)
    } else {
        0
    };
    assert_eq!(
        insured_cash_before - world.firm(insured).unwrap().book.cash(),
        premium + first_week_claim,
        "the insured pays the weekly premium (plus any claim received)"
    );
    assert_eq!(
        world.firm(insurer).unwrap().book.cash() - insurer_cash_before,
        premium - first_week_claim,
        "the insurer receives exactly the premium and pays any claim"
    );
    assert!(
        world.insurance.get(policy).unwrap().earned_premium_cents >= premium,
        "earned premium is recognised as coverage elapses"
    );

    // One correlated catastrophe applies to every policy in the region at once.
    let facility_value = world.facilities.get(facility).unwrap().capex_cents
        - world
            .facilities
            .get(facility)
            .unwrap()
            .accumulated_depreciation;
    let insurer_cash_before = world.firm(insurer).unwrap().book.cash();
    let insured_cash_before = world.firm(insured).unwrap().book.cash();
    insurance::catastrophe(&mut world, region);
    let expected_loss = facility_value * 30 / 100;
    let expected_claim = (expected_loss - deductible).clamp(0, limit);
    assert!(expected_claim > 0);
    let damage = world
        .facilities
        .get(facility)
        .unwrap()
        .accumulated_depreciation
        - (world.facilities.get(facility).unwrap().capex_cents - facility_value);
    assert_eq!(
        damage, expected_loss,
        "physical damage is recorded once on the asset"
    );
    let insurer_delta = world.firm(insurer).unwrap().book.cash() - insurer_cash_before;
    let paid = expected_claim.min(insurer_cash_before);
    assert_eq!(insurer_delta, -paid, "the insurer pays from real cash");
    assert_eq!(
        world.firm(insured).unwrap().book.cash() - insured_cash_before,
        paid,
        "claim compensation equals the insurer's payout"
    );
    assert_eq!(
        -world
            .firm(insurer)
            .unwrap()
            .book
            .balance(Account::ClaimLiabilities),
        expected_claim - paid,
        "only an unpaid claim stays reserved as a liability"
    );
    assert!(
        world
            .regions
            .get(region as usize)
            .and_then(|state| state.catastrophe_until)
            .is_some(),
        "the catastrophe is recorded for the region"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Technology: software subscription and service expiry
// ---------------------------------------------------------------------------------------

#[test]
fn technology_software_is_sold_as_a_subscription_and_expires_unsold() {
    let dataset = dataset();
    let mut world = seeded(113);
    let studio = one_firm(&world, "technology");
    staff(&mut world, &dataset, studio);
    let facility = facility_of(&world, studio, "software_studio", &dataset);
    let region = world.firm(studio).unwrap().region;
    let software = dataset.product_code("software").unwrap();
    assert_eq!(
        dataset.products[software as usize].kind,
        ProductKind::Subscription
    );
    assert!(dataset.products[software as usize].kind.is_service());

    production::produce(&mut world, &dataset);
    assert!(
        facility_output(&world, facility).is_positive(),
        "licences made"
    );
    assert!(
        world
            .firm(studio)
            .unwrap()
            .stock(software, region)
            .is_positive()
    );
    let cash_before = world.firm(studio).unwrap().book.cash();
    let expense_before = world
        .firm(studio)
        .unwrap()
        .book
        .balance(Account::OperatingExpense);

    production::expire_services(&mut world, &dataset);
    assert_eq!(
        world.firm(studio).unwrap().stock(software, region),
        MilliLots::ZERO,
        "an unsold licence cannot be carried into another week"
    );
    assert_eq!(
        world.firm(studio).unwrap().book.cash(),
        cash_before,
        "expiry moves no cash; it is not a sale"
    );
    assert!(
        world
            .firm(studio)
            .unwrap()
            .book
            .balance(Account::OperatingExpense)
            >= expense_before,
        "expiry can only write off carrying cost"
    );

    // Sold this week as a service to households: revenue is real, stock is still weekly.
    tick::advance(&mut world, &dataset);
    assert!(lifetime_revenue(&world, studio) > 0, "licence revenue");
    assert_eq!(
        world.firm(studio).unwrap().stock(software, region),
        MilliLots::ZERO,
        "no subscription is stockpiled after settlement"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Media: advertising has no independent demand and expires each week
// ---------------------------------------------------------------------------------------

#[test]
fn media_advertising_is_weekly_capacity_and_earns_nothing_unsold() {
    let dataset = dataset();
    let mut world = seeded(114);
    let studio = one_firm(&world, "media");
    staff(&mut world, &dataset, studio);
    let facility = facility_of(&world, studio, "media_studio", &dataset);
    let region = world.firm(studio).unwrap().region;
    let advertising = dataset.product_code("advertising").unwrap();
    assert_eq!(
        dataset.products[advertising as usize].kind,
        ProductKind::Service
    );

    // No household, government or factory book buys advertising: CONTENT-PACING says demand
    // comes from firm marketing budgets, which are not wired into clearing yet (see report).
    for state in &world.regions {
        assert!(!state.households.weights.contains_key(&advertising));
        assert!(!state.households.baseline_lots.contains_key(&advertising));
    }
    for recipe in &dataset.recipes {
        assert!(
            !recipe.weekly_inputs_lots.contains_key("advertising"),
            "no factory consumes advertising as a material input"
        );
    }

    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }
    tick::advance(&mut world, &dataset);

    // Capacity was produced and then expired; unsold reach is not revenue.
    assert!(
        facility_output(&world, facility).is_positive(),
        "the studio produced advertising capacity"
    );
    assert_eq!(
        world.firm(studio).unwrap().stock(advertising, region),
        MilliLots::ZERO,
        "advertising is a weekly service"
    );
    assert_eq!(
        lifetime_revenue(&world, studio),
        0,
        "no infinite media revenue without an advertiser budget"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Telecommunications: finite subscription capacity
// ---------------------------------------------------------------------------------------

#[test]
fn telecommunications_installed_capacity_is_finite_and_sells_subscriptions() {
    let dataset = dataset();
    let mut world = seeded(115);
    let operator = one_firm(&world, "telecommunications");
    staff(&mut world, &dataset, operator);
    let network = facility_of(&world, operator, "network", &dataset);
    let region = world.firm(operator).unwrap().region;
    let telecom = dataset.product_code("telecom").unwrap();
    let blocks = world.facilities.get(network).unwrap().blocks as i64;
    assert_eq!(
        dataset.products[telecom as usize].kind,
        ProductKind::Subscription
    );

    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }
    tick::advance(&mut world, &dataset);

    let output = facility_output(&world, network);
    let installed_capacity = MilliLots::from_lots(1_000 * blocks).scale_bps(8_500);
    assert!(output.is_positive());
    assert!(
        output <= installed_capacity,
        "a network cannot sell more subscription blocks than it installed"
    );
    let sold = world
        .firm(operator)
        .unwrap()
        .trailing_units(region, telecom);
    assert!(sold > 0, "households subscribe");
    assert!(
        sold <= output.lots(),
        "sales are capped by installed weekly capacity"
    );
    assert!(
        world.regions[region as usize]
            .households
            .spend_cents
            .get(&telecom)
            .copied()
            .unwrap_or(0)
            > 0
    );
    assert_eq!(
        world.firm(operator).unwrap().stock(telecom, region),
        MilliLots::ZERO,
        "subscriptions renew weekly instead of being stockpiled"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Real estate: lease service with vacancy
// ---------------------------------------------------------------------------------------

#[test]
fn real_estate_leases_earn_on_occupied_blocks_and_nothing_when_vacant() {
    let dataset = dataset();
    let lease = dataset.product_code("lease").unwrap();
    assert_eq!(
        dataset.products[lease as usize].kind,
        ProductKind::LeaseService
    );

    // Occupied: households rent the available blocks.
    let mut world = seeded(116);
    let landlord = one_firm(&world, "real_estate");
    staff(&mut world, &dataset, landlord);
    let property = facility_of(&world, landlord, "rental_property", &dataset);
    let region = world.firm(landlord).unwrap().region;
    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }
    tick::advance(&mut world, &dataset);
    assert!(facility_output(&world, property).is_positive());
    let rent_paid = world.regions[region as usize]
        .households
        .spend_cents
        .get(&lease)
        .copied()
        .unwrap_or(0);
    assert!(rent_paid > 0, "occupied blocks earn rent");
    assert!(lifetime_revenue(&world, landlord) > 0);
    assert_invariants(&world);

    // Vacant: the same blocks with no tenant demand earn exactly nothing, while upkeep is
    // still paid. Lease capacity cannot be stored until a tenant appears.
    let mut world = seeded(117);
    let landlord = one_firm(&world, "real_estate");
    staff(&mut world, &dataset, landlord);
    let property = facility_of(&world, landlord, "rental_property", &dataset);
    let region = world.firm(landlord).unwrap().region;
    for industry in INDUSTRY_NAMES {
        for firm in firms_in(&world, industry) {
            staff(&mut world, &dataset, firm);
        }
    }
    world.regions[region as usize]
        .households
        .weights
        .remove(&lease);
    world.regions[region as usize]
        .households
        .baseline_lots
        .remove(&lease);
    tick::advance(&mut world, &dataset);
    assert!(
        facility_output(&world, property).is_positive(),
        "the property is still available for lease"
    );
    assert_eq!(
        lifetime_revenue(&world, landlord),
        0,
        "vacant capacity earns no rent"
    );
    assert!(
        world
            .firm(landlord)
            .unwrap()
            .book
            .lifetime()
            .operating_expense
            > 0,
        "maintenance is still paid on vacant space"
    );
    assert_eq!(
        world.firm(landlord).unwrap().stock(lease, region),
        MilliLots::ZERO,
        "unlet space expires rather than accruing rent later"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Defense: procurement against a finite government budget
// ---------------------------------------------------------------------------------------

#[test]
fn defense_procurement_is_capped_by_the_government_budget() {
    let dataset = dataset();
    let mut world = seeded(118);
    let prime = one_firm(&world, "defense");
    staff(&mut world, &dataset, prime);
    let plant = facility_of(&world, prime, "defense_plant", &dataset);
    let region = world.firm(prime).unwrap().region;
    let defense = dataset.product_code("defense").unwrap();
    assert_eq!(
        dataset.products[defense as usize].kind,
        ProductKind::ContractGoods
    );

    // No budget, no procurement: output waits as work in progress, not as a fabricated sale.
    world.government.procurement_budget_cents = 0;
    world.government.healthcare_budget_cents = 0;
    world.government.training_budget_cents = 0;
    let government_expense_before = world.government.book.balance(Account::OperatingExpense);
    tick::advance(&mut world, &dataset);
    assert!(facility_output(&world, plant).is_positive());
    assert!(
        world
            .firm(prime)
            .unwrap()
            .stock(defense, region)
            .is_positive()
    );
    assert_eq!(
        world.firm(prime).unwrap().trailing_units(region, defense),
        0,
        "an unfunded delivery is never sold"
    );
    assert_eq!(
        world.government.book.balance(Account::OperatingExpense),
        government_expense_before,
        "the government spent nothing without a procurement budget"
    );

    // A funded budget buys only what it can pay for, and it is a national weekly ceiling.
    let price = dataset.reference_price(defense);
    let budget = price * 4;
    world.government.procurement_budget_cents = budget;
    let defense_firms = firms_in(&world, "defense");
    let stock_in = |world: &World, firms: &[Handle]| -> i64 {
        firms
            .iter()
            .map(|firm| {
                let region = world.firm(*firm).unwrap().region;
                world.firm(*firm).unwrap().stock(defense, region).0
            })
            .sum()
    };
    let stock_before = stock_in(&world, &defense_firms);
    let government_cash_before = world.government.book.cash();
    tick::advance(&mut world, &dataset);
    let produced: i64 = defense_firms
        .iter()
        .flat_map(|firm| world.firm(*firm).unwrap().facilities.clone())
        .map(|facility| facility_output(&world, facility).0)
        .sum();
    let sold: i64 = defense_firms
        .iter()
        .map(|firm| {
            let region = world.firm(*firm).unwrap().region;
            world.firm(*firm).unwrap().trailing_units(region, defense)
        })
        .sum();
    assert_eq!(sold, budget / price, "the government bought what it funded");
    assert_eq!(
        (stock_before + produced - stock_in(&world, &defense_firms)) / 1_000,
        budget / price,
        "exactly the funded deliveries left the plants"
    );
    assert_eq!(
        government_cash_before - world.government.book.cash(),
        budget,
        "procurement spending is capped and has a cash counterparty"
    );
    assert_invariants(&world);
}

// ---------------------------------------------------------------------------------------
// Cross-industry coverage checks
// ---------------------------------------------------------------------------------------

#[test]
fn every_industry_has_a_distinct_operating_model() {
    let dataset = dataset();
    let mut outputs: BTreeMap<&str, Vec<(&str, ProductKind)>> = BTreeMap::new();
    for recipe in &dataset.recipes {
        let product = dataset
            .product(&recipe.output)
            .expect("recipe output is a product");
        assert_ne!(
            product.kind,
            ProductKind::FinancialContract,
            "financial claims never come out of a factory"
        );
        outputs
            .entry(recipe.industry.as_str())
            .or_default()
            .push((recipe.output.as_str(), product.kind));
    }
    let mut product_families: BTreeMap<&str, usize> = BTreeMap::new();
    for (industry, rows) in &outputs {
        assert!(!rows.is_empty(), "{industry} has an operating template");
        for (_, kind) in rows {
            *product_families.entry(kind_code(*kind)).or_default() += 1;
        }
    }
    assert!(
        product_families.len() >= 6,
        "the fifteen industries use more than one generic product kind"
    );
    for industry in INDUSTRY_NAMES {
        if industry == "financial" {
            assert!(
                dataset.seed_firms.iter().any(|firm| firm.recipe == "bank"),
                "financial industry has a bank model"
            );
            assert!(
                dataset
                    .seed_firms
                    .iter()
                    .any(|firm| firm.recipe == "property_insurer"),
                "financial industry has an insurer model"
            );
            assert!(
                !dataset.recipes.iter().any(|r| r.industry == "financial"),
                "financial products are not manufactured"
            );
        } else {
            assert!(
                outputs.contains_key(industry),
                "{industry} has an operating template"
            );
        }
    }
}

fn kind_code(kind: ProductKind) -> &'static str {
    match kind {
        ProductKind::Goods => "goods",
        ProductKind::LocalService => "local_service",
        ProductKind::ProjectService => "project_service",
        ProductKind::TransportService => "transport_service",
        ProductKind::ConsumerGoods => "consumer_goods",
        ProductKind::Subscription => "subscription",
        ProductKind::Service => "service",
        ProductKind::LeaseService => "lease_service",
        ProductKind::ContractGoods => "contract_goods",
        ProductKind::FinancialContract => "financial_contract",
    }
}
