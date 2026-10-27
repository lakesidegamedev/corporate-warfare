//! Corporate operations evidence: sector focus, state conditions, sprawl and strategy retooling.
//! Expected values are worked by hand from crates/cw-sim/src/economy/operations.rs rules and the
//! condition indices in content/geography/regions.json.

use cw_core::ids::Handle;
use cw_core::ledger::{Account, Txn};
use cw_data::Dataset;
use cw_sim::commands::{CommandEnvelope, CommandIntent, apply};
use cw_sim::economy::{operations, seed};
use cw_sim::state::{FacilityStatus, ScenarioKind, ScenarioMode, World};

fn world() -> (World, Dataset) {
    let dataset = Dataset::load().unwrap();
    let world = seed::build(&dataset, 901, ScenarioKind::Founder, ScenarioMode::Sandbox);
    (world, dataset)
}

fn region(dataset: &Dataset, code: &str) -> u8 {
    dataset.regions.iter().position(|r| r.code == code).unwrap() as u8
}

#[test]
fn sprawl_is_a_smooth_curve_that_logistics_offsets() {
    // 0.18 bps per division squared: no cliff at any count, and it only ever rises.
    let at = |d: i64| operations::sprawl_rate_bps(d, 0, 0, false);
    assert_eq!(at(0), 0);
    assert_eq!(at(10), 18);
    assert_eq!(at(20), 72);
    assert_eq!(at(30), 162);
    for d in 1..30 {
        assert!(at(d + 1) >= at(d), "monotone at {d}");
        assert!(at(d + 1) - at(d) <= 11, "no step at {d}");
    }
    // Strength 200 halves it; four owned logistics divisions take off 24 percent; both stack.
    assert_eq!(operations::sprawl_rate_bps(20, 200, 0, false), 36);
    assert_eq!(operations::sprawl_rate_bps(20, 0, 4, false), 54);
    assert_eq!(operations::sprawl_rate_bps(20, 200, 4, false), 27);
    // The offset caps at forty percent, and a secondary type doubles the bill.
    assert_eq!(operations::sprawl_rate_bps(20, 0, 20, false), 43);
    assert_eq!(operations::sprawl_rate_bps(20, 0, 0, true), 144);
    assert!(operations::sprawl_onset(0, 0, false) > operations::sprawl_onset(0, 4, true));
    assert!(operations::sprawl_onset(200, 4, false) > operations::sprawl_onset(0, 0, false));
}

#[test]
fn roads_move_freight_cost() {
    let dataset = Dataset::load().unwrap();
    // West Virginia roads 4800 and Kentucky 6000: average 5400, so +300 bps.
    assert_eq!(
        operations::road_bps(
            &dataset,
            region(&dataset, "WV-SOU"),
            region(&dataset, "KY-LOU")
        ),
        10_300
    );
    // Indiana 6800 and Illinois 6200: average 6500, so -250 bps.
    assert_eq!(
        operations::road_bps(
            &dataset,
            region(&dataset, "IN-CEN"),
            region(&dataset, "IL-CHI")
        ),
        9_750
    );
}

#[test]
fn state_conditions_shape_plant_output() {
    let (mut world, dataset) = world();
    let firm = world.firms.iter().next().unwrap().0;
    let recipe = dataset.recipe_index["software_studio"];
    let mut plant = world.facilities.iter().next().unwrap().1.clone();
    plant.firm = firm;
    plant.recipe = recipe;
    plant.region = region(&dataset, "NY-NYC");
    let handle = world.facilities.insert(plant.clone());
    // New York Metro: grid 9400 is 100 under 9500, costing 40 bps; skill 6949 adds 155;
    // broadband 4800 adds 280.
    assert_eq!(
        operations::condition_terms(&world, &dataset, handle, false),
        (0, 395)
    );
    assert_eq!(
        operations::output_bps(&world, &dataset, handle, false),
        10_000,
        "good conditions save inputs instead of exceeding rated capacity"
    );
    plant.region = region(&dataset, "WV-SOU");
    let wv = world.facilities.insert(plant);
    // Southern West Virginia: grid 9300 costs 80, skill 3923 costs 86, broadband 996 costs 100.
    assert_eq!(operations::output_bps(&world, &dataset, wv, false), 9_734);
    assert_eq!(
        operations::condition_terms(&world, &dataset, wv, false),
        (266, 0)
    );
}

#[test]
fn focus_sets_input_needs_and_off_focus_plants_pay_overhead() {
    let (mut world, dataset) = world();
    let (firm, plant) = world
        .firms
        .iter()
        .find_map(|(handle, firm)| firm.facilities.first().map(|p| (handle, *p)))
        .unwrap();
    assert_eq!(
        operations::focus(&world, &dataset, plant),
        operations::Focus::Primary
    );
    // Focus savings compound with any state-condition saving for the plant.
    let (_, bonus) = operations::condition_terms(&world, &dataset, plant, false);
    let with = |focus_bps: i64| (focus_bps * (10_000 - bonus as i64) / 10_000) as i32;
    assert_eq!(operations::input_bps(&world, &dataset, plant), with(9_500));
    let own = world.firm(firm).unwrap().industry;
    let other = (own + 1) % 14;
    world.firms.get_mut(firm).unwrap().secondary_industry = None;
    world.firms.get_mut(firm).unwrap().industry = other;
    assert_eq!(
        operations::focus(&world, &dataset, plant),
        operations::Focus::Off
    );
    assert_eq!(operations::input_bps(&world, &dataset, plant), with(10_000));
    world.firms.get_mut(firm).unwrap().secondary_industry = Some(own);
    assert_eq!(operations::input_bps(&world, &dataset, plant), with(9_750));
    // Off focus again: overhead is a tenth of the plant's payroll, paid to the outside sector.
    world.firms.get_mut(firm).unwrap().secondary_industry = None;
    let workers = world.facilities.get(plant).unwrap().workers;
    let wages = world.firm(firm).unwrap().policies.wage_offer;
    let payroll: i64 = (0..3).map(|t| workers[t] * wages[t]).sum();
    // Isolate the focus overhead: no logistics budget, no retail shrinkage, no sprawl.
    for (handle, entity) in world.firms.iter_mut() {
        entity.policies.logistics_budget_cents = 0;
        if handle != firm {
            entity.status = cw_sim::state::FirmStatus::Dissolved;
        }
    }
    let plants = world.firm(firm).unwrap().facilities.clone();
    let off: i64 = plants
        .iter()
        .map(|p| {
            let w = world.facilities.get(*p).unwrap().workers;
            (0..3).map(|t| w[t] * wages[t]).sum::<i64>()
        })
        .sum();
    let before = world.outside.gateway_book.balance(Account::Cash);
    operations::weekly(&mut world, &dataset);
    let received = world.outside.gateway_book.balance(Account::Cash) - before;
    assert!(payroll > 0);
    let industry = &dataset.recipes[world.facilities.get(plant).unwrap().recipe as usize].industry;
    if industry != "retail" && industry != "real_estate" {
        assert_eq!(received, off * 1_000 / 10_000);
    }
    assert!(world.firm(firm).unwrap().book.is_balanced());
}

#[test]
fn a_focus_change_transitions_for_four_weeks_and_locks_for_eight() {
    let (mut world, _) = world();
    let firm = world.firms.iter().next().unwrap().0;
    let own = world.firm(firm).unwrap().industry;
    let other = (own + 1) % 14;
    operations::set_focus(&mut world, firm, other, Some(own)).unwrap();
    let entity = world.firm(firm).unwrap();
    assert_eq!(
        (entity.industry, entity.secondary_industry),
        (other, Some(own))
    );
    assert_eq!(entity.focus_transition_until, world.tick.0 + 4);
    assert!(
        operations::set_focus(&mut world, firm, own, None).is_err(),
        "locked"
    );
    assert!(
        operations::set_focus(&mut world, firm, own, Some(own)).is_err(),
        "secondary differs"
    );
    world.tick = cw_core::calendar::Tick(world.tick.0 + 8);
    assert!(operations::set_focus(&mut world, firm, own, None).is_ok());
}

#[test]
fn logistics_strength_grows_with_the_budget_paid() {
    assert_eq!(operations::logistics_growth(400_000), 0);
    assert_eq!(operations::logistics_growth(500_000), 1);
    assert_eq!(operations::logistics_growth(2_500_000), 2);
    assert_eq!(operations::logistics_growth(10_000_000), 3);
}

fn envelope(world: &World, firm: Handle, intent: CommandIntent) -> CommandEnvelope {
    CommandEnvelope {
        command_id_high: 0x0905,
        command_id_low: world.revision,
        actor: firm.0,
        expected_revision: world.revision,
        submitted_tick: world.tick.0,
        intent,
    }
}

#[test]
fn a_retool_switches_strategy_within_the_sector_at_a_quarter_of_capex() {
    let (mut world, dataset) = world();
    let (firm, plant) = world
        .facilities
        .iter()
        .find(|(_, f)| dataset.recipes[f.recipe as usize].id == "farm_industrial")
        .map(|(h, f)| (f.firm, h))
        .unwrap();
    let blocks = world.facilities.get(plant).unwrap().blocks as i64;
    let tick = world.tick;
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .book
        .commit(
            tick,
            Txn::new()
                .debit(Account::Cash, 100_000_000_000, "fixture")
                .credit(Account::ShareCapital, 100_000_000_000, "fixture"),
        )
        .unwrap();
    // Cross-sector retool is refused.
    let mine = dataset.recipe_index["coal_mine"];
    let refused = envelope(
        &world,
        firm,
        CommandIntent::RetoolFacility {
            facility: plant,
            recipe: mine,
        },
    );
    assert!(apply(&mut world, &dataset, &refused).is_err());
    let precision = dataset.recipe_index["farm_precision"];
    let cash = world.firm(firm).unwrap().book.cash();
    let command = envelope(
        &world,
        firm,
        CommandIntent::RetoolFacility {
            facility: plant,
            recipe: precision,
        },
    );
    apply(&mut world, &dataset, &command).expect("same-sector retool");
    let template = &dataset.recipes[precision as usize];
    assert_eq!(
        cash - world.firm(firm).unwrap().book.cash(),
        template.capex_cents * blocks / 4
    );
    let weeks = (template.build_weeks / 3).max(4);
    assert_eq!(
        world.facilities.get(plant).unwrap().status,
        FacilityStatus::Retooling {
            until: cw_core::calendar::Tick(world.tick.0 + weeks)
        }
    );
}

#[test]
fn the_outside_account_draws_at_most_one_percent_a_week() {
    let (mut world, dataset) = world();
    // Leave the outside account nearly empty: the paced draw must still hold.
    let cash = world.outside.gateway_book.cash();
    let keep = 2_000_000;
    let _ = world.outside.gateway_book.commit(
        world.tick,
        Txn::new()
            .credit(Account::Cash, cash - keep, "probe transfer")
            .debit(Account::OtherAssets, cash - keep, "probe transfer"),
    );
    for _ in 0..6 {
        let before = world.outside.gateway_book.cash();
        cw_sim::tick::advance(&mut world, &dataset);
        let after = world.outside.gateway_book.cash();
        // Imports only add to the account, so the net week can never fall below the floor.
        assert!(
            after >= before - before / 100,
            "week drew {} of {before}",
            before - after
        );
        assert!(after >= 0);
    }
}

#[test]
fn a_mothballed_plant_carries_no_staff_and_reopens_for_a_twentieth_of_its_capex() {
    let (mut world, dataset) = world();
    let (firm, plant) = world
        .facilities
        .iter()
        .find(|(_, f)| f.status == FacilityStatus::Operating && f.workers.iter().sum::<i64>() > 0)
        .map(|(h, f)| (f.firm, h))
        .unwrap();
    let tick = world.tick;
    world
        .firms
        .get_mut(firm)
        .unwrap()
        .book
        .commit(
            tick,
            Txn::new()
                .debit(Account::Cash, 100_000_000_000, "fixture")
                .credit(Account::ShareCapital, 100_000_000_000, "fixture"),
        )
        .unwrap();
    // Only a mothballed plant can reopen.
    let early = envelope(
        &world,
        firm,
        CommandIntent::ReopenFacility { facility: plant },
    );
    assert!(apply(&mut world, &dataset, &early).is_err());

    let mothball = envelope(
        &world,
        firm,
        CommandIntent::MothballFacility { facility: plant },
    );
    apply(&mut world, &dataset, &mothball).expect("mothball an operating plant");
    let stored = world.facilities.get(plant).unwrap();
    assert_eq!(stored.status, FacilityStatus::Mothballed);
    assert_eq!(
        stored.workers, [0; 3],
        "a mothballed plant keeps nobody on payroll"
    );
    let again = envelope(
        &world,
        firm,
        CommandIntent::MothballFacility { facility: plant },
    );
    assert!(apply(&mut world, &dataset, &again).is_err());

    let capex = world.facilities.get(plant).unwrap().capex_cents;
    let cash = world.firm(firm).unwrap().book.cash();
    let reopen = envelope(
        &world,
        firm,
        CommandIntent::ReopenFacility { facility: plant },
    );
    apply(&mut world, &dataset, &reopen).expect("reopen a mothballed plant");
    assert_eq!(
        cash - world.firm(firm).unwrap().book.cash(),
        (capex / 20).max(100_000)
    );
    let stored = world.facilities.get(plant).unwrap();
    assert_eq!(stored.status, FacilityStatus::Operating);
    assert_eq!(stored.mothballed_since, None);
}

#[test]
fn farm_crews_follow_the_season_and_count_as_full_crews() {
    let (mut world, dataset) = world();
    let (plant, recipe) = world
        .facilities
        .iter()
        .find(|(_, f)| {
            dataset.recipes[f.recipe as usize].industry == "agriculture"
                && f.status == FacilityStatus::Operating
        })
        .map(|(h, f)| (h, f.recipe))
        .unwrap();
    let template = &dataset.recipes[recipe as usize];
    // A winter quarter: the yield is 80 percent, so is the crew.
    assert_eq!(world.tick.quarter(), 1);
    assert_eq!(
        cw_sim::economy::production::crew_need_bps(&world, template),
        8_000
    );
    let blocks = world.facilities.get(plant).unwrap().blocks as i64;
    let full: Vec<i64> = (0..3)
        .map(|t| template.workers_low_mid_high[t] * blocks)
        .collect();
    {
        let facility = world.facilities.get_mut(plant).unwrap();
        facility.target_bps = 10_000;
        facility.workers = [full[0], full[1], full[2]];
    }
    cw_sim::economy::labour::clear(&mut world, &dataset);
    let staffed = world.facilities.get(plant).unwrap().workers;
    for tier in 0..3 {
        assert_eq!(
            staffed[tier],
            full[tier] * 8_000 / 10_000,
            "tier {tier} keeps the winter crew"
        );
    }
    // Every other industry needs its whole crew in any season.
    let mill = dataset
        .recipes
        .iter()
        .find(|r| r.industry == "manufacturing")
        .unwrap();
    assert_eq!(
        cw_sim::economy::production::crew_need_bps(&world, mill),
        10_000
    );
}
